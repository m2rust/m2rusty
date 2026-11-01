//! Terrain patch geometry: `CTerrain::_CalculateTerrainPatch`
//! (`AreaTerrain.cpp`), `CTerrainPatch` (`TerrainPatch.cpp`) and the shared
//! index buffers of `CMapOutdoor::SetIndexBuffer`
//! (`MapOutdoorIndexBuffer.cpp`), plus the LOD thresholds of
//! `MapOutdoorRenderSTP.cpp`.
//!
//! Positions are in world units with `y` negated (`kPosition.y = -fY`), as
//! the C++ vertex buffers hold them. A terrain is 8x8 patches of 16x16
//! cells; a patch has 17x17 vertices.

use m2_formats::map::{PATCH_COUNT, Terrain, WATERMAP_SIZE};

use crate::outdoor::float_to_long;

/// `CTerrainImpl::PATCH_XSIZE`.
pub const PATCH_SIZE: usize = 16;
/// `CTerrainPatch::TERRAIN_VERTEX_COUNT`.
pub const TERRAIN_VERTEX_COUNT: usize = (PATCH_SIZE + 1) * (PATCH_SIZE + 1);
/// `m_pwaIndices` capacity / `TERRAINPATCH_LODMAX`.
pub const LOD_COUNT: usize = 3;
/// `m_fOpaqueWaterDepth` set by `CMapOutdoor::Initialize`.
pub const DEFAULT_OPAQUE_WATER_DEPTH: f32 = 400.0;

/// `HardwareTransformPatch_SSourceVertex`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatchVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

/// `SWaterVertex`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterVertex {
    pub position: [f32; 3],
    /// ARGB; only the alpha varies.
    pub diffuse: u32,
}

/// `CTerrainPatch::PATCH_TYPE_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchType {
    Plain = 0,
    Hill = 1,
    Cliff = 2,
}

/// One built `CTerrainPatch`.
#[derive(Debug, Clone)]
pub struct PatchMesh {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub patch_type: PatchType,
    pub water_exist: bool,
    /// 17x17 vertices, row-major.
    pub vertices: Vec<PatchVertex>,
    /// Triangle list (6 vertices per wet cell); empty without water.
    pub water_vertices: Vec<WaterVertex>,
}

/// `PR_FLOAT_TO_INT` into a `DWORD`.
fn float_to_dword(f: f32) -> u32 {
    float_to_long(f) as u32
}

/// `CTerrain::_CalculateTerrainPatch` for the patch `(px, py)` of the
/// terrain at `(tx, ty)`.
pub fn build_patch(terrain: &Terrain, tx: u16, ty: u16, px: usize, py: usize, opaque_water_depth: f32) -> PatchMesh {
    let oo_depth = 1.0f32 / opaque_water_depth;
    let transparent = 0.8f32 * opaque_water_depth;
    let (start_x, start_y) = (px * PATCH_SIZE, py * PATCH_SIZE);
    let cs = crate::CELLSCALE as f32;
    let orig_x = (i32::from(tx) * 128 * crate::CELLSCALE) as f32 + (start_x as i32 * crate::CELLSCALE) as f32;
    let mut f_y = (i32::from(ty) * 128 * crate::CELLSCALE) as f32 + (start_y as i32 * crate::CELLSCALE) as f32;
    let mut patch = PatchMesh {
        min_x: orig_x,
        max_x: orig_x + (PATCH_SIZE as i32 * crate::CELLSCALE) as f32,
        min_y: f_y,
        max_y: f_y + (PATCH_SIZE as i32 * crate::CELLSCALE) as f32,
        min_z: 0.0,
        max_z: 0.0,
        patch_type: PatchType::Plain,
        water_exist: false,
        vertices: Vec::with_capacity(TERRAIN_VERTEX_COUNT),
        water_vertices: Vec::new(),
    };
    let (mut min_z, mut max_z) = (999_999.0f32, -999_999.0f32);
    let (mut plain, mut hill, mut cliff) = (0u16, 0u16, 0u16);
    let hs = terrain.height_scale;
    let h_at = |x: usize, y: usize| terrain.height(x as i32, y as i32);
    for dy in start_y..=start_y + PATCH_SIZE {
        let mut f_x = orig_x;
        for dx in start_x..=start_x + PATCH_SIZE {
            let hgt = h_at(dx, dy);
            let ni = (dy * 129 + dx) * 3;
            let n = &terrain.normal_map[ni..ni + 3];
            let normal = [
                (-i32::from(n[0])) as f32 * 0.0078740f32,
                f32::from(n[1]) * 0.0078740f32,
                f32::from(n[2]) * 0.0078740f32,
            ];
            let position = [f_x, -f_y, f32::from(hgt) * hs];
            patch.vertices.push(PatchVertex { position, normal });
            if 0.5f32 > normal[2] {
                cliff += 1;
            } else if 0.866_025_4_f32 > normal[2] {
                hill += 1;
            } else {
                plain += 1;
            }
            max_z = if max_z < position[2] { position[2] } else { max_z };
            min_z = if position[2] < min_z { position[2] } else { min_z };

            if dx < 128 && dy < 128 && dx != start_x + PATCH_SIZE && dy != start_y + PATCH_SIZE {
                let layer = terrain.water_map[dy * WATERMAP_SIZE + dx];
                if layer != 0xFF {
                    let water = i64::from(terrain.water_heights[usize::from(layer)]);
                    if water != -1 {
                        let diff = |h: u16| {
                            let d = (water - i64::from(h)) as f32;
                            let d = if d < transparent { d } else { transparent };
                            if 0.0f32 > d { 0.0 } else { d }
                        };
                        let a = [
                            float_to_dword(diff(hgt) * oo_depth * 255.0f32),
                            float_to_dword(diff(h_at(dx, dy + 1)) * oo_depth * 255.0f32),
                            float_to_dword(diff(h_at(dx + 1, dy)) * oo_depth * 255.0f32),
                            float_to_dword(diff(h_at(dx + 1, dy + 1)) * oo_depth * 255.0f32),
                        ];
                        let key = (a[0] << 24) | (a[1] << 16) | (a[2] << 8) | a[3];
                        if key != 0 {
                            let z = water as f32 * hs;
                            let d = |alpha: u32, low: u32| ((alpha << 24) & 0xFF00_0000) | low;
                            let (x0, x1, y0, y1) = (f_x, f_x + cs, -f_y, -f_y - cs);
                            for (x, y, diffuse) in [
                                (x0, y0, d(a[0], 0x0000_00FF)),
                                (x0, y1, d(a[1], 0x00FF_FFFF)),
                                (x1, y0, d(a[2], 0x00FF_FFFF)),
                                (x1, y0, d(a[2], 0x00FF_FFFF)),
                                (x0, y1, d(a[1], 0x00FF_FFFF)),
                                (x1, y1, d(a[3], 0x00FF_FFFF)),
                            ] {
                                patch.water_vertices.push(WaterVertex { position: [x, y, z], diffuse });
                            }
                            patch.water_exist = true;
                        }
                    }
                }
            }
            f_x += cs;
        }
        f_y += cs;
    }
    if plain <= hill.max(cliff) {
        patch.patch_type = if cliff <= hill { PatchType::Hill } else { PatchType::Cliff };
    }
    patch.min_z = min_z;
    patch.max_z = max_z;
    patch
}

/// `CTerrain::CalculateTerrainPatch`: all 8x8 patches, row-major.
pub fn build_terrain_patches(terrain: &Terrain, tx: u16, ty: u16, opaque_water_depth: f32) -> Vec<PatchMesh> {
    (0..PATCH_COUNT)
        .flat_map(|py| (0..PATCH_COUNT).map(move |px| (px, py)))
        .map(|(px, py)| build_patch(terrain, tx, ty, px, py, opaque_water_depth))
        .collect()
}

/// `p(cur, row, col)`: `cur + row * 17 + col` stored as a `WORD`.
fn at(cur: u16, row: i32, col: i32) -> u16 {
    (i32::from(cur) + row * 17 + col) as u16
}

fn tris(out: &mut Vec<u16>, cur: u16, list: &[[(i32, i32); 3]]) {
    for t in list {
        for &(r, c) in t {
            out.push(at(cur, r, c));
        }
    }
}

type T = [(i32, i32); 3];
const L1_TL: [T; 6] = [
    [(0, 0), (1, 0), (1, 1)],
    [(1, 0), (2, 0), (1, 1)],
    [(1, 1), (2, 0), (2, 2)],
    [(1, 1), (2, 2), (0, 2)],
    [(1, 1), (0, 2), (0, 1)],
    [(1, 1), (0, 1), (0, 0)],
];
const L1_T: [T; 5] = [
    [(0, 0), (2, 0), (1, 1)],
    [(2, 0), (2, 2), (1, 1)],
    [(2, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 1), (1, 1)],
    [(0, 1), (0, 0), (1, 1)],
];
const L1_TR: [T; 6] = [
    [(0, 0), (2, 0), (1, 1)],
    [(2, 0), (2, 2), (1, 1)],
    [(2, 2), (1, 2), (1, 1)],
    [(1, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 1), (1, 1)],
    [(0, 1), (0, 0), (1, 1)],
];
const L1_L: [T; 5] = [
    [(0, 0), (1, 0), (1, 1)],
    [(1, 0), (2, 0), (1, 1)],
    [(2, 0), (2, 2), (1, 1)],
    [(2, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 0), (1, 1)],
];
const L1_R: [T; 5] = [
    [(0, 0), (2, 0), (1, 1)],
    [(2, 0), (2, 2), (1, 1)],
    [(2, 2), (1, 2), (1, 1)],
    [(1, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 0), (1, 1)],
];
const L1_BL: [T; 6] = [
    [(0, 0), (1, 0), (1, 1)],
    [(1, 0), (2, 0), (1, 1)],
    [(1, 1), (2, 0), (2, 1)],
    [(1, 1), (2, 1), (2, 2)],
    [(2, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 0), (1, 1)],
];
const L1_B: [T; 5] = [
    [(0, 0), (2, 0), (1, 1)],
    [(2, 0), (2, 1), (1, 1)],
    [(2, 1), (2, 2), (1, 1)],
    [(2, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 0), (1, 1)],
];
const L1_BR: [T; 6] = [
    [(0, 0), (2, 0), (1, 1)],
    [(2, 0), (2, 1), (1, 1)],
    [(2, 1), (2, 2), (1, 1)],
    [(2, 2), (1, 2), (1, 1)],
    [(1, 2), (0, 2), (1, 1)],
    [(0, 2), (0, 0), (1, 1)],
];
const L1_M: [T; 2] = [[(0, 0), (2, 0), (0, 2)], [(2, 0), (2, 2), (0, 2)]];

/// A level-1 block kind (`ADDLvl1*`).
#[derive(Clone, Copy)]
enum Edge {
    TL,
    T,
    TR,
    L,
    R,
    BL,
    B,
    BR,
    M,
}

fn lvl1(out: &mut Vec<u16>, cur: u16, e: Edge) {
    match e {
        Edge::TL => tris(out, cur, &L1_TL),
        Edge::T => tris(out, cur, &L1_T),
        Edge::TR => tris(out, cur, &L1_TR),
        Edge::L => tris(out, cur, &L1_L),
        Edge::R => tris(out, cur, &L1_R),
        Edge::BL => tris(out, cur, &L1_BL),
        Edge::B => tris(out, cur, &L1_B),
        Edge::BR => tris(out, cur, &L1_BR),
        Edge::M => tris(out, cur, &L1_M),
    }
}

fn lvl2(out: &mut Vec<u16>, cur: u16, e: Edge) {
    let c = |r, k| at(cur, r, k);
    match e {
        Edge::TL => {
            lvl1(out, cur, Edge::TL);
            lvl1(out, c(0, 2), Edge::T);
            lvl1(out, c(2, 0), Edge::L);
            lvl1(out, c(2, 2), Edge::M);
        }
        Edge::T => {
            lvl1(out, cur, Edge::T);
            lvl1(out, c(0, 2), Edge::T);
            tris(out, cur, &[[(2, 0), (4, 0), (2, 2)], [(4, 0), (4, 4), (2, 2)], [(4, 4), (2, 4), (2, 2)]]);
        }
        Edge::TR => {
            lvl1(out, cur, Edge::T);
            lvl1(out, c(0, 2), Edge::TR);
            lvl1(out, c(2, 0), Edge::M);
            lvl1(out, c(2, 2), Edge::R);
        }
        Edge::L => {
            lvl1(out, cur, Edge::L);
            lvl1(out, c(2, 0), Edge::L);
            tris(out, cur, &[[(0, 2), (2, 2), (0, 4)], [(0, 4), (2, 2), (4, 4)], [(4, 4), (2, 2), (4, 2)]]);
        }
        Edge::R => {
            lvl1(out, c(0, 2), Edge::R);
            lvl1(out, c(2, 2), Edge::R);
            tris(out, cur, &[[(0, 2), (0, 0), (2, 2)], [(0, 0), (4, 0), (2, 2)], [(2, 2), (4, 0), (4, 2)]]);
        }
        Edge::BL => {
            lvl1(out, cur, Edge::L);
            lvl1(out, c(0, 2), Edge::M);
            lvl1(out, c(2, 0), Edge::BL);
            lvl1(out, c(2, 2), Edge::B);
        }
        Edge::B => {
            tris(out, cur, &[[(2, 0), (2, 2), (0, 0)], [(0, 0), (2, 2), (0, 4)], [(0, 4), (2, 2), (2, 4)]]);
            lvl1(out, c(2, 0), Edge::B);
            lvl1(out, c(2, 2), Edge::B);
        }
        Edge::BR => {
            lvl1(out, cur, Edge::M);
            lvl1(out, c(0, 2), Edge::R);
            lvl1(out, c(2, 0), Edge::B);
            lvl1(out, c(2, 2), Edge::BR);
        }
        Edge::M => tris(out, cur, &[[(0, 0), (4, 0), (0, 4)], [(4, 0), (4, 4), (0, 4)]]),
    }
}

/// `CMapOutdoor::SetIndexBuffer`: the three LOD index buffers shared by
/// every patch. LOD 0 is a triangle strip, 1 and 2 are triangle lists.
pub fn build_index_buffers() -> [Vec<u16>; LOD_COUNT] {
    const W: i32 = 17;
    let mut idx: [Vec<u16>; LOD_COUNT] = Default::default();
    let mut c2 = [0u16; LOD_COUNT];
    let edge_of = |y: usize, x: usize, last: usize, step: usize| -> Option<Edge> {
        // The `0 == y` / `last == y` / `0 == y % step` ladder of the C++.
        if y == 0 {
            Some(if x == 0 {
                Edge::TL
            } else if x == last {
                Edge::TR
            } else {
                Edge::T
            })
        } else if y == last {
            Some(if x == 0 {
                Edge::BL
            } else if x == last {
                Edge::BR
            } else {
                Edge::B
            })
        } else if y % step == 0 {
            Some(if x == 0 {
                Edge::L
            } else if x == last {
                Edge::R
            } else {
                Edge::M
            })
        } else {
            None
        }
    };
    for y in 0..PATCH_SIZE {
        let c = c2[0];
        if y % 2 == 0 {
            idx[0].push(c);
            idx[0].push(at(c, 1, 0));
        } else {
            idx[0].push(at(c, 1, 0));
            idx[0].push(c);
        }
        for x in 0..PATCH_SIZE {
            let c = c2[0];
            if y % 2 == 0 {
                idx[0].push(at(c, 0, 1));
                idx[0].push(at(c, 1, 1));
                c2[0] = c.wrapping_add(1);
            } else {
                idx[0].push(at(c, 1, -1));
                idx[0].push(at(c, 0, -1));
                c2[0] = c.wrapping_sub(1);
            }
            if x % 2 == 0 {
                if let Some(e) = edge_of(y, x, PATCH_SIZE - 2, 2) {
                    lvl1(&mut idx[1], c2[1], e);
                }
                c2[1] = c2[1].wrapping_add(2);
            }
            if x % 4 == 0 {
                if let Some(e) = edge_of(y, x, PATCH_SIZE - 4, 4) {
                    lvl2(&mut idx[2], c2[2], e);
                }
                c2[2] = c2[2].wrapping_add(4);
            }
        }
        if y < PATCH_SIZE - 1 {
            let c = c2[0];
            idx[0].push(at(c, 1, 0));
            idx[0].push(at(c, 1, 0));
            c2[0] = c.wrapping_add(W as u16);
            if y % 2 == 0 {
                c2[1] = c2[1].wrapping_add(2);
            }
            if y % 4 == 0 {
                c2[2] = c2[2].wrapping_add(4);
            }
        }
    }
    idx
}

/// `CMapOutdoor::SelectIndexBuffer`: primitive count and whether LOD
/// `lod` is a strip (`true`) or a list.
pub fn primitive_count(lod: usize, num_indices: usize) -> (u16, bool) {
    if lod == 0 { ((num_indices as u16).wrapping_sub(2), true) } else { ((num_indices as u16) / 3, false) }
}

/// The two distances (`__GetNoFogDistance`, `__GetFogDistance`) where
/// patches drop to LOD 1 and 2, for a map's view radius in cells.
pub fn lod_distances(view_radius: i64) -> (f32, f32) {
    let base = (i64::from(crate::CELLSCALE) * view_radius) as f32;
    (base * 0.5, base * 0.75)
}

/// The LOD step of `__RenderTerrain_RenderSoftwareTransformPatch`: patches
/// are visited nearest first and the level only ever grows.
pub fn next_lod(current: u8, distance: f32, distances: (f32, f32)) -> u8 {
    if current == 0 && distances.0 <= distance {
        1
    } else if current == 1 && distances.1 <= distance {
        2
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_buffers_have_expected_shape() {
        let [l0, l1, l2] = build_index_buffers();
        assert_eq!(l0.len(), 16 * 34 + 15 * 2);
        assert!(l1.len() % 3 == 0 && l2.len() % 3 == 0);
        assert!(l0.iter().chain(&l1).chain(&l2).all(|&i| usize::from(i) < TERRAIN_VERTEX_COUNT));
    }

    #[test]
    fn lod_steps_grow() {
        let d = lod_distances(60);
        assert_eq!(d, (6000.0, 9000.0));
        assert_eq!(next_lod(0, 100.0, d), 0);
        assert_eq!(next_lod(0, 7000.0, d), 1);
        assert_eq!(next_lod(1, 7000.0, d), 1);
        assert_eq!(next_lod(1, 9000.0, d), 2);
    }
}
