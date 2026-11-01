//! `CMapOutdoor` queries on the 3x3 terrain window, and `CTerrain`'s own
//! height, normal, attribute, water and shadow lookups.
//!
//! The C++ keeps every query relative to the window around the player
//! (`m_pTerrain[9]`, `m_CurCoordinate`); [`OutdoorMap::update`] moves it as
//! `CMapOutdoor::Update` does. Integer widths, `BYTE`/`WORD` wrap-arounds
//! and the odd index arithmetic are mirrored on purpose.

use std::collections::BTreeMap;

use m2_formats::map::{ATTRMAP_SIZE, NORMALMAP_SIZE, SHADOWMAP_SIZE, Terrain, WATERMAP_SIZE};

use crate::{AROUND_AREA_NUM, CELLSCALE, TERRAIN_WORLD};

/// `PR_FLOAT_TO_INT` / `static_cast<long>(std::floor(f))` as x86-64 computes
/// it: `cvttss2si` gives `i64::MIN` for NaN and out-of-range values.
pub fn float_to_long(f: f32) -> i64 {
    let f = f.floor();
    if f.is_nan() || f >= 9.223_372e18 || f < -9.223_372e18 { i64::MIN } else { f as i64 }
}

/// `PR_FLOAT_TO_INT` into an `int`.
fn float_to_int(f: f32) -> i32 {
    float_to_long(f) as i32
}

/// A loaded `CTerrain`: its coordinate and data.
#[derive(Debug, Clone)]
pub struct TerrainCell {
    /// `m_wX`, `m_wY`.
    pub x: u16,
    pub y: u16,
    pub terrain: Terrain,
}

impl TerrainCell {
    /// `CTerrain::GetHeight(int, int)`: height at a world position, 0 outside
    /// this terrain.
    pub fn height(&self, x: i32, y: i32) -> f32 {
        let x = x.wrapping_sub(i32::from(self.x) * 128 * CELLSCALE);
        let y = y.wrapping_sub(i32::from(self.y) * 128 * CELLSCALE);
        if x < 0 || y < 0 || x > TERRAIN_WORLD || y > TERRAIN_WORLD {
            return 0.0;
        }
        let xdist = i64::from(x % CELLSCALE);
        let ydist = i64::from(y % CELLSCALE);
        let ooscale = 1.0f32 / CELLSCALE as f32;
        let (x, y) = (x / CELLSCALE, y / CELLSCALE);
        let hs = self.terrain.height_scale;
        let h = |x: i32, y: i32| f32::from(self.terrain.height(x, y)) * hs;
        let h1 = h(x, y);
        let h2 = h(x + 1, y + 1);
        if xdist <= ydist {
            let h3 = h(x, y + 1);
            let xslope = (h2 - h3) * ooscale;
            let yslope = (h3 - h1) * ooscale;
            return h1 + (xdist as f32 * xslope + ydist as f32 * yslope);
        }
        let h3 = h(x + 1, y);
        let xslope = (h3 - h1) * ooscale;
        let yslope = (h2 - h3) * ooscale;
        h1 + (xdist as f32 * xslope + ydist as f32 * yslope)
    }

    /// `CTerrain::GetNormal(int, int)`: the normal at a position inside the
    /// terrain (wrapped into `0..=25600`).
    pub fn normal(&self, mut ix: i32, mut iy: i32) -> [f32; 3] {
        while ix < 0 {
            ix += TERRAIN_WORLD;
        }
        while iy < 0 {
            iy += TERRAIN_WORLD;
        }
        while ix > TERRAIN_WORLD {
            ix -= TERRAIN_WORLD;
        }
        while iy > TERRAIN_WORLD {
            iy -= TERRAIN_WORLD;
        }
        ix /= CELLSCALE;
        iy /= CELLSCALE;
        let i = (iy as usize * NORMALMAP_SIZE + ix as usize) * 3;
        let n = &self.terrain.normal_map[i..i + 3];
        [-f32::from(n[0]) * 0.007874016, f32::from(n[1]) * 0.007874016, f32::from(n[2]) * 0.007874016]
    }

    /// `CTerrain::isAttrOn`.
    pub fn is_attr_on(&self, x: u16, y: u16, flag: u8) -> bool {
        if usize::from(x) >= ATTRMAP_SIZE || usize::from(y) >= ATTRMAP_SIZE {
            return false;
        }
        let attr = self.terrain.attr_map[usize::from(y) * ATTRMAP_SIZE + usize::from(x)];
        if flag < 16 { attr & flag != 0 } else { flag / 16 == attr / 16 }
    }

    /// `CTerrain::GetAttr`: 0 outside the attribute map.
    pub fn attr(&self, x: u16, y: u16) -> u8 {
        if usize::from(x) >= ATTRMAP_SIZE || usize::from(y) >= ATTRMAP_SIZE {
            return 0;
        }
        self.terrain.attr_map[usize::from(y) * ATTRMAP_SIZE + usize::from(x)]
    }

    /// `CTerrain::GetWaterHeight(WORD, WORD, long *)`: the value is stored
    /// as `LONG` (32 bit) and halved with C integer division.
    ///
    /// C++ bug: callers pass `(offset) / WATERMAP_XSIZE` (128) although a
    /// cell is 200 wide, so the index can pass the end of the 128x128 water
    /// map and read the bytes of the normal map that follows it in the
    /// object. Mirrored by reading the water map followed by the normal map.
    pub fn water_height(&self, x: u16, y: u16) -> Option<i64> {
        let i = usize::from(y) * WATERMAP_SIZE + usize::from(x);
        let water = &self.terrain.water_map;
        let n = if i < water.len() {
            water[i]
        } else {
            // Reinterpret the `char` normals as `BYTE`.
            self.terrain.normal_map.get(i - water.len()).map_or(0, |&b| b as u8)
        };
        if n > self.terrain.num_water {
            return None;
        }
        Some(i64::from(self.terrain.water_heights[usize::from(n)] / 2))
    }

    /// `CTerrain::GetWaterHeight(BYTE, long *)`: the raw height of a layer.
    pub fn layer_water_height(&self, layer: u8) -> Option<i32> {
        (layer <= self.terrain.num_water).then(|| self.terrain.water_heights[usize::from(layer)])
    }

    /// `CTerrainImpl::GetShadowMapColor`. C++ bug: the result repeats the
    /// green channel in the blue byte and drops blue. Mirrored.
    pub fn shadow_map_color(&self, mut fx: f32, mut fy: f32) -> u32 {
        let size = TERRAIN_WORLD as f32;
        let foo = 1.0f32 / size;
        if fx < 0.0 || fy < 0.0 || fx >= size || fy >= size {
            return 0xFFFF_FFFF;
        }
        fx = fx * foo * (SHADOWMAP_SIZE - 1) as f32;
        fy = fy * foo * (SHADOWMAP_SIZE - 1) as f32;
        let ix = float_to_int(fx);
        let iy = float_to_int(fy);
        let mut w = u32::from(self.terrain.shadow_map[(iy as usize) * SHADOWMAP_SIZE + ix as usize]);
        w >>= 5;
        let g = (w & 0x1f) << 3;
        w >>= 5;
        let r = (w & 0x1f) << 3;
        0xff00_0000 | (g << 16) | (g << 8) | r
    }
}

/// Result of `CMapOutdoor::ConvertToMapCoords`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapCoords {
    pub cell_x: i32,
    pub cell_y: i32,
    pub sub_cell_x: u8,
    pub sub_cell_y: u8,
    pub terrain_x: u16,
    pub terrain_y: u16,
}

/// The outdoor map state that queries need: terrain count, the loaded
/// terrains and the 3x3 window (`m_pTerrain`).
#[derive(Debug, Clone)]
pub struct OutdoorMap {
    /// `m_sTerrainCountX`, `m_sTerrainCountY`.
    pub terrain_count: (i16, i16),
    /// `m_CurCoordinate`.
    pub cur: (i16, i16),
    /// `m_PrevCoordinate`.
    pub prev: (i16, i16),
    /// `m_lCurCoordStartX/Y`.
    pub cur_start: (i64, i64),
    /// `m_TerrainVector`, by coordinate.
    pub terrains: BTreeMap<(u16, u16), TerrainCell>,
    /// `m_pTerrain`: the coordinate held by each window slot.
    pub window: [Option<(u16, u16)>; 9],
}

impl OutdoorMap {
    /// A freshly initialised `CMapOutdoor` with the given `MapSize`.
    pub fn new(terrain_count: (i16, i16)) -> Self {
        Self {
            terrain_count,
            cur: (-1, -1),
            prev: (-1, -1),
            cur_start: (0, 0),
            terrains: BTreeMap::new(),
            window: [None; 9],
        }
    }

    /// `CMapOutdoor::Update`, the part that moves the window. When the
    /// terrain coordinate of the player changes, every terrain of the 3x3
    /// around it is requested from `load` (skipped when already loaded) and
    /// the window is reassigned. Returns whether the window moved.
    pub fn update(&mut self, x: f32, y: f32, mut load: impl FnMut(u16, u16) -> Option<TerrainCell>) -> bool {
        let ix = float_to_int(x);
        let y = if y < 0.0 { -y } else { y };
        let iy = float_to_int(y);
        let minmax = |min: i32, v: i32, max: i32| if max < min { min.max(v) } else { max.min(min.max(v)) };
        let cx = minmax(0, ix / TERRAIN_WORLD, i32::from(self.terrain_count.0) - 1) as i16;
        let cy = minmax(0, iy / TERRAIN_WORLD, i32::from(self.terrain_count.1) - 1) as i16;
        let need_init = self.prev.0 == -1 || self.prev.1 == -1;
        if !(need_init || self.cur.0 != cx || self.cur.1 != cy) {
            return false;
        }
        self.prev = if need_init { (cx, cy) } else { self.cur };
        self.cur = (cx, cy);
        self.cur_start = (i64::from(cx) * i64::from(TERRAIN_WORLD), i64::from(cy) * i64::from(TERRAIN_WORLD));
        let min_x = (i32::from(cx) - 1).max(0);
        let max_x = (i32::from(cx) + 1).min(i32::from(self.terrain_count.0) - 1);
        let min_y = (i32::from(cy) - 1).max(0);
        let max_y = (i32::from(cy) + 1).min(i32::from(self.terrain_count.1) - 1);
        for ty in min_y..=max_y {
            for tx in min_x..=max_x {
                let key = (tx as u16, ty as u16);
                if !self.terrains.contains_key(&key) {
                    if let Some(cell) = load(key.0, key.1) {
                        self.terrains.insert(key, cell);
                    }
                }
            }
        }
        self.assign_terrain_ptr(min_x, max_x, min_y, max_y);
        true
    }

    /// `CMapOutdoor::AssignTerrainPtr`.
    fn assign_terrain_ptr(&mut self, min_x: i32, max_x: i32, min_y: i32, max_y: i32) {
        self.window = [None; 9];
        for &(tx, ty) in self.terrains.keys() {
            let (x, y) = (i32::from(tx), i32::from(ty));
            if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
                let slot = (y - i32::from(self.cur.1) + 1) * 3 + (x - i32::from(self.cur.0) + 1);
                self.window[slot as usize] = Some((tx, ty));
            }
        }
    }

    /// `CMapOutdoor::GetTerrainNumFromCoord`: the slot is a `BYTE`, so far
    /// coordinates wrap into the window; `false` above 9.
    pub fn terrain_num_from_coord(&self, x: u16, y: u16) -> Option<u8> {
        let (ok, b) = self.terrain_num_from_coord_raw(x, y);
        ok.then_some(b)
    }

    /// As [`terrain_num_from_coord`](Self::terrain_num_from_coord), also
    /// returning the (wrapped) slot C++ stores in its out-parameter on
    /// failure.
    pub fn terrain_num_from_coord_raw(&self, x: u16, y: u16) -> (bool, u8) {
        let v = (i32::from(y) - i32::from(self.cur.1) + 1) * 3 + (i32::from(x) - i32::from(self.cur.0) + 1);
        let b = v as u8;
        (i32::from(b) <= AROUND_AREA_NUM, b)
    }

    /// `CMapOutdoor::GetTerrainNum(float, float)` with the out-parameter.
    pub fn terrain_num_raw(&self, x: f32, y: f32) -> (bool, u8) {
        let y = if y < 0.0 { -y } else { y };
        let (ix, iy) = (float_to_int(x), float_to_int(y));
        self.terrain_num_from_coord_raw((ix / TERRAIN_WORLD) as u16, (iy / TERRAIN_WORLD) as u16)
    }

    /// `CMapOutdoor::GetTerrainPointer`.
    pub fn terrain_pointer(&self, num: u8) -> Option<&TerrainCell> {
        if i32::from(num) >= AROUND_AREA_NUM {
            return None;
        }
        self.terrains.get(&self.window[usize::from(num)]?)
    }

    /// `CMapOutdoor::GetTerrainNum(float, float)`.
    pub fn terrain_num(&self, x: f32, y: f32) -> Option<u8> {
        let y = if y < 0.0 { -y } else { y };
        let (ix, iy) = (float_to_int(x), float_to_int(y));
        self.terrain_num_from_coord((ix / TERRAIN_WORLD) as u16, (iy / TERRAIN_WORLD) as u16)
    }

    /// `CMapOutdoor::GetTerrainHeight`: terrain only (no object heights).
    pub fn terrain_height(&self, x: f32, y: f32) -> f32 {
        let y = if y < 0.0 { -y } else { y };
        let (lx, ly) = (float_to_long(x), float_to_long(y));
        let coord = ((lx / i64::from(TERRAIN_WORLD)) as u16, (ly / i64::from(TERRAIN_WORLD)) as u16);
        let Some(num) = self.terrain_num_from_coord(coord.0, coord.1) else {
            return 0.0;
        };
        match self.terrain_pointer(num) {
            Some(t) => t.height(lx as i32, ly as i32),
            None => 0.0,
        }
    }

    /// `CMapOutdoor::GetNormal(int, int)`.
    pub fn normal(&self, mut ix: i32, mut iy: i32) -> Option<[f32; 3]> {
        let (wx, wy) = (i32::from(self.terrain_count.0) * TERRAIN_WORLD, i32::from(self.terrain_count.1) * TERRAIN_WORLD);
        if ix <= 0 {
            ix = 0;
        } else if ix >= wx {
            ix = wx;
        }
        if iy <= 0 {
            iy = 0;
        } else if iy >= wy {
            iy = wy;
        }
        let cx = ((ix / TERRAIN_WORLD) as u16 as i32).min(i32::from(self.terrain_count.0) - 1) as u16;
        let cy = ((iy / TERRAIN_WORLD) as u16 as i32).min(i32::from(self.terrain_count.1) - 1) as u16;
        let t = self.terrain_pointer(self.terrain_num_from_coord(cx, cy)?)?;
        while ix >= TERRAIN_WORLD {
            ix -= TERRAIN_WORLD;
        }
        while iy >= TERRAIN_WORLD {
            iy -= TERRAIN_WORLD;
        }
        Some(t.normal(ix, iy))
    }

    /// The bounds test and terrain lookup shared by the attribute and water
    /// queries; returns the terrain and its coordinate in world units.
    fn locate(&self, ix: i32, iy: i32) -> Option<(&TerrainCell, i32, i32)> {
        if ix < 0
            || iy < 0
            || ix > i32::from(self.terrain_count.0) * TERRAIN_WORLD
            || iy > i32::from(self.terrain_count.1) * TERRAIN_WORLD
        {
            return None;
        }
        let (cx, cy) = ((ix / TERRAIN_WORLD) as u16, (iy / TERRAIN_WORLD) as u16);
        let t = self.terrain_pointer(self.terrain_num_from_coord(cx, cy)?)?;
        Some((t, ix - i32::from(cx) * TERRAIN_WORLD, iy - i32::from(cy) * TERRAIN_WORLD))
    }

    /// `CMapOutdoor::GetWaterHeight(int, int, long *)`.
    pub fn water_height(&self, ix: i32, iy: i32) -> Option<i64> {
        let (t, ox, oy) = self.locate(ix, iy)?;
        t.water_height((ox / WATERMAP_SIZE as i32) as u16, (oy / WATERMAP_SIZE as i32) as u16)
    }

    /// `CMapOutdoor::isAttrOn(int, int, BYTE)`.
    pub fn is_attr_on(&self, ix: i32, iy: i32, flag: u8) -> bool {
        let Some((t, ox, oy)) = self.locate(ix, iy) else {
            return false;
        };
        t.is_attr_on((ox / 100) as u16, (oy / 100) as u16, flag)
    }

    /// `CMapOutdoor::isAttrOn(float, float, BYTE)`.
    pub fn is_attr_on_f(&self, x: f32, y: f32, flag: u8) -> bool {
        self.is_attr_on(float_to_int(x), float_to_int(y), flag)
    }

    /// `CMapOutdoor::GetAttr(int, int, BYTE *)`.
    pub fn attr(&self, ix: i32, iy: i32) -> Option<u8> {
        let (t, ox, oy) = self.locate(ix, iy)?;
        Some(t.attr(((ox as u16) / 100) as u16, ((oy as u16) / 100) as u16))
    }

    /// `CMapOutdoor::GetAttr(float, float, BYTE *)`.
    pub fn attr_f(&self, x: f32, y: f32) -> Option<u8> {
        self.attr(float_to_int(x), float_to_int(y))
    }

    /// `CMapOutdoor::GetShadowMapColor`.
    pub fn shadow_map_color(&self, fx: f32, fy: f32) -> u32 {
        let fy = if fy < 0.0 { -fy } else { fy };
        let size = TERRAIN_WORLD as f32;
        let xr = fx - self.cur_start.0 as f32;
        let yr = fy - self.cur_start.1 as f32;
        // Row of the window (0..2) and column, as the C++ range checks give.
        let band = |v: f32| -> Option<usize> {
            if v < -size {
                None
            } else if v < 0.0 {
                Some(0)
            } else if v < size {
                Some(1)
            } else if v < 2.0 * size {
                Some(2)
            } else {
                None
            }
        };
        // C++ tests the row (`fYRef`) first with `>=` ... `<` ranges, and NaN
        // falls through every test to 0xFFFFFFFF.
        let (Some(row), Some(col)) = (band(yr), band(xr)) else {
            return 0xFFFF_FFFF;
        };
        let (ox, oy) = (xr - size * (col as f32 - 1.0), yr - size * (row as f32 - 1.0));
        match self.terrain_pointer((row * 3 + col) as u8) {
            Some(t) => t.shadow_map_color(ox, oy),
            None => 0xFFFF_FFFF,
        }
    }

    /// `CMapOutdoor::ConvertToMapCoords`. Loops forever for infinite input,
    /// as C++ does; callers pass finite coordinates.
    pub fn convert_to_map_coords(&self, mut fx: f32, mut fy: f32) -> MapCoords {
        if fy < 0.0 {
            fy = -fy;
        }
        let (ix, iy) = (float_to_int(fx), float_to_int(fy));
        let terrain_x = (ix / TERRAIN_WORLD) as u16;
        let terrain_y = (iy / TERRAIN_WORLD) as u16;
        let max = TERRAIN_WORLD as f32;
        while fx < 0.0 {
            fx += max;
        }
        while fy < 0.0 {
            fy += max;
        }
        while fx >= max {
            fx -= max;
        }
        while fy >= max {
            fy -= max;
        }
        let foo = 1.0f32 / 100.0;
        let cell_x = float_to_int(fx * foo);
        let cell_y = float_to_int(fy * foo);
        let ratio = 2.0f32 * foo;
        let sub_x = (float_to_long(fx * ratio) as u8) % 2;
        let sub_y = (float_to_long(fy * ratio) as u8) % 2;
        MapCoords { cell_x, cell_y, sub_cell_x: sub_x, sub_cell_y: sub_y, terrain_x, terrain_y }
    }

    /// `CMapOutdoor::GetPickingPointWithRayOnlyTerrain`: marches the ray
    /// `start + dir * range` (direction normalised as `CRay` does) in three
    /// passes of growing step and returns the first point below the ground.
    pub fn pick_terrain(&self, start: [f32; 3], dir: [f32; 3], range: f32) -> Option<[f32; 3]> {
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        let dir = if len > 1e-8 {
            let inv = 1.0f32 / len;
            [dir[0] * inv, dir[1] * inv, dir[2] * inv]
        } else {
            [0.0; 3]
        };
        let end = [start[0] + range * dir[0], start[1] + range * dir[1], start[2] + range * dir[2]];
        let mut pos = 0.0f32;
        for (step, limit) in [(5.0f32, 5000.0f32), (10.0, 10000.0), (100.0, 100000.0)] {
            if let Some(p) = self.pick_pass(&mut pos, start, end, step, range, limit) {
                return Some(p);
            }
        }
        None
    }

    /// `CMapOutdoor::__PickTerrainHeight`.
    fn pick_pass(&self, pos: &mut f32, start: [f32; 3], end: [f32; 3], step: f32, range: f32, limit: f32) -> Option<[f32; 3]> {
        let inv = 1.0f32 / range;
        while *pos < range && *pos < limit {
            let s = *pos * inv;
            let cur = [
                start[0] + (s * (end[0] - start[0])),
                start[1] + (s * (end[1] - start[1])),
                start[2] + (s * (end[2] - start[2])),
            ];
            let mut mult = 1.0f32;
            if let Some(num) = self.terrain_num(cur[0], cur[1]) {
                if let Some(t) = self.terrain_pointer(num) {
                    let ix = float_to_int(cur[0]);
                    let iy = float_to_int(cur[1].abs());
                    let h = t.height(ix, iy);
                    if h >= cur[2] {
                        return Some(cur);
                    }
                    let m = 0.01f32 * (cur[2] - h);
                    mult = if 1.0f32 > m { 1.0 } else { m };
                }
            }
            *pos += step * mult;
        }
        None
    }
}
