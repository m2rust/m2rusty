//! Differential test: m2-world against the real C++ client code
//! (`tools/differential/world`, which documents the dump format and what is
//! real C++). Every map of the client's index is loaded on both sides; the
//! dumps (hashes of meshes, splats and area placements, hashes of ~20k
//! queries per player position) must be identical.
//!
//! Needs the client and the C++ tool. Run with
//! `M2_CLIENT_DIR=… M2_DIFF_BUILD=… cargo test --release -p m2-world --test
//! differential_world -- --ignored --nocapture` (`scripts/differential.sh`
//! builds the tool into `build/differential`).

mod support;

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use m2_formats::map::{
    MapProperty, MapSetting, TERRAIN_TEX_COORD_BASE, Terrain, splat_alpha_matrix, terrain_id,
};
use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::property::PropertyManager;
use m2_world::area::{AmbienceUpdate, PlacedKind, load_objects, place_objects};
use m2_world::lighting::{Light, Material, vertex_diffuse, vertex_uvs};
use m2_world::mesh::{build_index_buffers, build_terrain_patches, primitive_count, DEFAULT_OPAQUE_WATER_DEPTH};
use m2_world::splat::{generate_splats, mip_chain};
use m2_world::{OutdoorMap, TerrainCell, float_to_long};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const INDEX: &str = "pack/Index.dev";

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ u64::from(x)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn f(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn gen_points(ox: i64, oy: i64, rng: &mut Rng) -> Vec<(f32, f32)> {
    let mut pts = Vec::new();
    for j in 0..105i64 {
        for i in 0..105i64 {
            pts.push(((ox - 28800 + 800 * i) as f32, (oy - 28800 + 800 * j) as f32));
        }
    }
    let (mut bx, mut by) = (Vec::new(), Vec::new());
    for k in 0..27i64 {
        for d in -1..=1i64 {
            bx.push((ox - 28800 + 3200 * k + d) as f32);
            by.push((oy - 28800 + 3200 * k + d) as f32);
        }
    }
    for &x in &bx {
        for &y in &by {
            pts.push((x, y));
        }
    }
    for _ in 0..4000 {
        let kind = rng.next() % 4;
        if kind == 0 || kind == 2 {
            let x = (ox - 60000) as f32 + (rng.next() % 2_720_000) as f32 / 16.0;
            let mut y = (oy - 60000) as f32 + (rng.next() % 2_720_000) as f32 / 16.0;
            if if kind == 0 { rng.next() & 1 != 0 } else { true } {
                y = -y;
            }
            pts.push((x, y));
        } else if kind == 1 {
            let ax = (rng.next() % 384) as i64;
            let bx2 = (rng.next() % 3) as i64 - 1;
            let ay = (rng.next() % 384) as i64;
            let by2 = (rng.next() % 3) as i64 - 1;
            let flip = rng.next() & 1 != 0;
            let cx = ox + ax * 200 - 12800 + bx2;
            let cy = oy + ay * 200 - 12800 + by2;
            let y = cy as f32;
            pts.push((cx as f32, if flip { -y } else { y }));
        } else {
            let x = ((rng.next() % 40_000_000) as i64 - 10_000_000) as f32;
            let y = ((rng.next() % 40_000_000) as i64 - 10_000_000) as f32;
            pts.push((x, y));
        }
    }
    let specials = [0.0f32, -0.0, 1e-30, 25599.99, 25600.0, -1.0, 1e10, -1e10, 4294967296.0, 3e9, f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
    for &x in &specials {
        for &y in &specials {
            pts.push((x, y));
        }
    }
    pts
}

#[derive(Default)]
struct Acc {
    h: Option<Fnv>,
    count: u64,
    hits: u64,
}

impl Acc {
    fn h(&mut self) -> &mut Fnv {
        self.h.get_or_insert_with(Fnv::new)
    }
    fn line(&mut self, out: &mut Vec<u8>, kind: &str) {
        let hash = self.h().0;
        writeln!(out, "Q\t{kind}\t{hash:016x}\t{}\t{}", self.count, self.hits).unwrap();
    }
}

fn run_queries(out: &mut Vec<u8>, map: &OutdoorMap, pos_index: usize) {
    let (cx, cy) = map.cur;
    let (ox, oy) = (i64::from(cx) * 25600, i64::from(cy) * 25600);
    let mut rng = Rng(((cx as u64) << 32 | u64::from(cy as u32)).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(pos_index as u64));
    let pts = gen_points(ox, oy, &mut rng);
    let (mut height, mut normal, mut water, mut attr, mut getattr, mut shadow, mut num, mut coords, mut pick) =
        (Acc::default(), Acc::default(), Acc::default(), Acc::default(), Acc::default(), Acc::default(), Acc::default(), Acc::default(), Acc::default());
    let flags = [1u8, 2, 4, 3, 8, 16, 32, 48, 255];
    for &(fx, fy) in &pts {
        let ix = float_to_long(fx) as i32;
        let iy = float_to_long(fy) as i32;
        let th = map.terrain_height(fx, fy);
        height.h().f(th);
        height.count += 1;
        height.hits += u64::from(th != 0.0);

        let n = map.normal(ix, iy);
        normal.h().u8(u8::from(n.is_some()));
        if let Some(n) = n {
            n.iter().for_each(|&f| normal.h().f(f));
        }
        normal.count += 1;
        normal.hits += u64::from(n.is_some());

        let w = map.water_height(ix, iy);
        water.h().u8(u8::from(w.is_some()));
        if let Some(w) = w {
            water.h().u64(w as u64);
        }
        water.count += 1;
        water.hits += u64::from(w.is_some());

        let mut bits = 0u32;
        for (k, &f) in flags.iter().enumerate() {
            bits |= u32::from(map.is_attr_on_f(fx, fy, f)) << k;
        }
        attr.h().u32(bits);
        attr.count += 1;
        attr.hits += u64::from(bits != 0);

        let a = map.attr_f(fx, fy);
        getattr.h().u8(u8::from(a.is_some()));
        getattr.h().u8(a.unwrap_or(0));
        getattr.count += 1;
        getattr.hits += u64::from(a.is_some());

        let sc = map.shadow_map_color(fx, fy);
        shadow.h().u32(sc);
        shadow.count += 1;
        shadow.hits += u64::from(sc != 0xFFFF_FFFF);

        let (ok, tn) = map.terrain_num_raw(fx, fy);
        num.h().u8(u8::from(ok));
        num.h().u8(tn);
        num.count += 1;
        num.hits += u64::from(ok);

        if fx.is_finite() && fy.is_finite() && fx.abs() < 1e8 && fy.abs() < 1e8 {
            let c = map.convert_to_map_coords(fx, fy);
            coords.h().u32(c.cell_x as u32);
            coords.h().u32(c.cell_y as u32);
            coords.h().u8(c.sub_cell_x);
            coords.h().u8(c.sub_cell_y);
            coords.h().u32(u32::from(c.terrain_x));
            coords.h().u32(u32::from(c.terrain_y));
            coords.count += 1;
        }
    }
    for _ in 0..40 {
        let sx = (ox - 25600) as f32 + (rng.next() % 76800) as f32;
        let mut sy = (oy - 25600) as f32 + (rng.next() % 76800) as f32;
        if rng.next() & 1 != 0 {
            sy = -sy;
        }
        let sz = (rng.next() % 8000) as f32;
        let dx = ((rng.next() % 2001) as i32 - 1000) as f32 / 1000.0;
        let dy = ((rng.next() % 2001) as i32 - 1000) as f32 / 1000.0;
        let dz = -((((rng.next() % 951) as i32 + 50) as f32) / 1000.0);
        let range = (1000 + rng.next() % 39000) as f32;
        let hit = map.pick_terrain([sx, sy, sz], [dx, dy, dz], range);
        pick.h().u8(u8::from(hit.is_some()));
        if let Some(h) = hit {
            h.iter().for_each(|&f| pick.h().f(f));
        }
        pick.count += 1;
        pick.hits += u64::from(hit.is_some());
    }
    let slots: String = map.window.iter().map(|s| if s.is_some() { '1' } else { '0' }).collect();
    writeln!(out, "W\t{pos_index}\t{cx}\t{cy}\t{slots}\t{}", pts.len()).unwrap();
    height.line(out, "height");
    normal.line(out, "normal");
    water.line(out, "water");
    attr.line(out, "attr");
    getattr.line(out, "getattr");
    shadow.line(out, "shadow");
    num.line(out, "terrainnum");
    coords.line(out, "mapcoords");
    pick.line(out, "pick");
}

fn put_f(out: &mut Vec<u8>, f: f32) {
    write!(out, "\t{:08x}", f.to_bits()).unwrap();
}

fn hex(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

fn dump_terrain_mesh(out: &mut Vec<u8>, cell: &TerrainCell, texture_count: usize) {
    let patches = build_terrain_patches(&cell.terrain, cell.x, cell.y, DEFAULT_OPAQUE_WATER_DEPTH);
    for (i, p) in patches.iter().enumerate() {
        let mut hv = Fnv::new();
        for v in &p.vertices {
            v.position.iter().chain(&v.normal).for_each(|&f| hv.f(f));
            hv.u32(0xFFFF_FFFF);
        }
        let mut hw = Fnv::new();
        for v in &p.water_vertices {
            v.position.iter().for_each(|&f| hw.f(f));
            hw.u32(v.diffuse);
        }
        write!(out, "PA\t{}\t{}\t{}\t{}", i % 8, i / 8, p.patch_type as i32, u8::from(p.water_exist)).unwrap();
        for f in [p.min_x, p.max_x, p.min_y, p.max_y, p.min_z, p.max_z] {
            put_f(out, f);
        }
        writeln!(out, "\t{:016x}\t{}\t{:016x}", hv.0, p.water_vertices.len(), hw.0).unwrap();
    }
    for s in generate_splats(&cell.terrain, texture_count) {
        let mut h0 = Fnv::new();
        h0.bytes(&s.alpha);
        write!(out, "SP\t{}\t1\t{:016x}\t1", s.texture, h0.0).unwrap();
        for level in mip_chain(&s.alpha) {
            let mut h = Fnv::new();
            h.bytes(&level);
            write!(out, "\t{:016x}", h.0).unwrap();
        }
        out.push(b'\n');
    }
    let mut h = Fnv::new();
    let matrix = splat_alpha_matrix();
    for p in &patches {
        for uv in vertex_uvs(&p.vertices, u32::from(cell.x), u32::from(cell.y), TERRAIN_TEX_COORD_BASE, &matrix) {
            [uv.tile, uv.alpha, uv.static_shadow].iter().flatten().for_each(|&f| h.f(f));
        }
    }
    writeln!(out, "UV\t{:016x}", h.0).unwrap();
    // direction, ambient, diffuse, material ambient, diffuse, emissive
    let cfgs: [[[f32; 3]; 6]; 4] = [
        [[0.3, -0.5, -0.8], [0.5, 0.5, 0.5], [0.8, 0.8, 0.8], [1.0; 3], [1.0; 3], [0.0; 3]],
        [[-0.6, 0.2, 0.7], [0.9; 3], [0.9; 3], [0.9; 3], [1.0; 3], [0.0; 3]],
        [[0.1, 0.9, -0.4], [0.2, 0.3, 0.7], [0.7, 0.4, 0.1], [0.8, 0.6, 0.4], [0.5, 0.9, 0.7], [0.05, 0.0, 0.1]],
        [[0.0, 0.0, -1.0], [1.2, 1.1, 0.3], [0.9; 3], [1.0; 3], [1.0; 3], [0.0, 0.2, 0.0]],
    ];
    for (c, cfg) in cfgs.iter().enumerate() {
        let light = Light { direction: cfg[0], ambient: cfg[1], diffuse: cfg[2] };
        let mtrl = Material { ambient: cfg[3], diffuse: cfg[4], emissive: cfg[5] };
        let mut h = Fnv::new();
        for p in &patches {
            vertex_diffuse(&p.vertices, &light, &mtrl).iter().for_each(|&d| h.u32(d));
        }
        writeln!(out, "LT\t{c}\t{:016x}", h.0).unwrap();
    }
}

fn dump_area(out: &mut Vec<u8>, manager: &PackManager, dir: &[u8], x: u16, y: u16, props: &PropertyManager) {
    let area_dir = m2_formats::map::area_dir_name(dir, x, y);
    let objects = load_objects(manager, &area_dir, props);
    let exists = |n: &[u8]| manager.exists(n);
    let placed = place_objects(&objects, props, &exists);
    writeln!(out, "AN\t{}", placed.len()).unwrap();
    for o in &placed {
        let head = {
            let mut s = format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                o.crc,
                hex(o.position[0]),
                hex(o.position[1]),
                hex(o.position[2]),
                hex(o.rotation[0]),
                hex(o.rotation[1]),
                hex(o.rotation[2])
            );
            for p in o.portal_ids {
                s += &format!("\t{p}");
            }
            s + &format!("\t{}\t{}", o.range, hex(o.max_volume_area_percentage))
        };
        out.extend_from_slice(b"AP\t");
        let join = |out: &mut Vec<u8>, b: &[u8]| {
            out.push(b'\t');
            out.extend_from_slice(b);
        };
        match &o.kind {
            PlacedKind::Tree { file } => {
                write!(out, "tree\t{head}").unwrap();
                join(out, file);
            }
            PlacedKind::Building { file, attribute_file, attribute_exists, shadow_flag, lod_files } => {
                write!(out, "building\t{head}").unwrap();
                join(out, file);
                join(out, attribute_file);
                write!(out, "\t{}\t{shadow_flag}\t{}", u8::from(*attribute_exists), lod_files.len()).unwrap();
                lod_files.iter().for_each(|l| join(out, l));
            }
            PlacedKind::Effect { file, effect_id, matrix } => {
                write!(out, "effect\t{head}").unwrap();
                join(out, file);
                write!(out, "\t{effect_id}").unwrap();
                matrix.iter().for_each(|&f| put_f(out, f));
            }
            PlacedKind::Ambience {
                name,
                play_type,
                update,
                play_interval,
                play_interval_variation,
                property_max_volume_area_percentage,
                sounds,
            } => {
                write!(out, "ambience\t{head}").unwrap();
                join(out, name);
                join(out, play_type);
                let u = match update {
                    AmbienceUpdate::Once => 0,
                    AmbienceUpdate::Step => 1,
                    AmbienceUpdate::Loop => 2,
                    AmbienceUpdate::None => 3,
                };
                write!(out, "\t{u}").unwrap();
                for f in [play_interval, play_interval_variation, property_max_volume_area_percentage] {
                    put_f(out, *f);
                }
                write!(out, "\t{}", sounds.len()).unwrap();
                sounds.iter().for_each(|s| join(out, s));
            }
            PlacedKind::DungeonBlock { file, attribute_file, attribute_exists } => {
                write!(out, "dungeon\t{head}").unwrap();
                join(out, file);
                join(out, attribute_file);
                write!(out, "\t{}", u8::from(*attribute_exists)).unwrap();
            }
        }
        out.push(b'\n');
    }
}

fn dump_map(out: &mut Vec<u8>, manager: &PackManager, name: &[u8], files: &BTreeSet<Vec<u8>>, props: &PropertyManager) {
    out.extend_from_slice(b"M\t");
    out.extend_from_slice(name);
    out.push(b'\n');
    let property = MapProperty::load(manager, name);
    let dir = property.data_directory(name).to_vec();
    let setting = MapSetting::load(manager, &dir);
    let ok = setting.error.is_none();
    let (cx, cy) = setting.terrain_count;
    let texture_count = setting.texture_set.textures.len();
    write!(out, "MS\t{}\t{cx}\t{cy}\t{}", u8::from(ok), setting.view_radius).unwrap();
    put_f(out, setting.height_scale);
    put_f(out, DEFAULT_OPAQUE_WATER_DEPTH);
    writeln!(out, "\t{texture_count}").unwrap();
    if !ok {
        return;
    }
    for (lod, idx) in build_index_buffers().iter().enumerate() {
        let mut h = Fnv::new();
        idx.iter().for_each(|i| h.bytes(&i.to_le_bytes()));
        let (prims, strip) = primitive_count(lod, idx.len());
        writeln!(out, "IB\t{lod}\t{}\t{:016x}\t{prims}\t{}", idx.len(), h.0, u8::from(strip)).unwrap();
    }

    let mut prefix: Vec<u8> = dir.iter().map(|&b| if b == b'\\' { b'/' } else { b.to_ascii_lowercase() }).collect();
    prefix.push(b'/');
    let ids: BTreeSet<u32> = files
        .range(prefix.clone()..)
        .take_while(|f| f.starts_with(&prefix))
        .filter_map(|f| {
            let rest = &f[prefix.len()..];
            (rest.len() > 7 && rest[6] == b'/' && rest[..6].iter().all(u8::is_ascii_digit))
                .then(|| std::str::from_utf8(&rest[..6]).unwrap().parse().unwrap())
        })
        .collect();
    let load_cell = |x: u16, y: u16| -> Option<TerrainCell> {
        let (terrain, _) = Terrain::load(manager, &dir, x, y, setting.height_scale).ok()?;
        Some(TerrainCell { x, y, terrain })
    };
    for &id in &ids {
        let (x, y) = ((id / 1000) as u16, (id % 1000) as u16);
        let cell = load_cell(x, y);
        writeln!(out, "T\t{id:06}\t{}", u8::from(cell.is_some())).unwrap();
        if let Some(cell) = &cell {
            dump_terrain_mesh(out, cell, texture_count);
        }
        dump_area(out, manager, &dir, x, y, props);
        let _ = terrain_id(x, y);
    }

    let mut positions: Vec<(f32, f32)> = Vec::new();
    if i32::from(cx) * i32::from(cy) <= 64 {
        for j in 0..i32::from(cy) {
            for i in 0..i32::from(cx) {
                let sign = if (i + j) & 1 != 0 { -1.0f32 } else { 1.0 };
                positions.push(((i * 25600 + 12800) as f32, (j * 25600 + 12800) as f32 * sign));
            }
        }
    } else {
        for (n, &id) in ids.iter().enumerate() {
            let (px, py) = (((id / 1000) * 25600 + 12800) as f32, ((id % 1000) * 25600 + 12800) as f32);
            positions.push((px, if n & 1 != 0 { -py } else { py }));
        }
    }
    positions.push((-5000.0, -5000.0));
    positions.push((f32::from(cx) * 25600.0 + 5000.0, f32::from(cy) * 25600.0 + 5000.0));
    positions.push((1e9, -1e9));
    let mut map = OutdoorMap::new(setting.terrain_count);
    for (index, &(x, y)) in positions.iter().enumerate() {
        map.update(x, y, load_cell);
        run_queries(out, &map, index);
    }
}

fn base_name(name: &[u8]) -> &[u8] {
    name.rsplit(|&b| b == b'/').next().unwrap()
}

fn manager(client: &Path) -> PackManager {
    let mut manager = PackManager::new(client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", INDEX).unwrap();
    manager
}

#[test]
#[ignore = "needs the client data and the C++ tool (see the module docs)"]
fn world_matches_cpp() {
    let client = client_dir();
    if !client.join("pack").exists() {
        eprintln!("skipping: no client data at {}", client.display());
        return;
    }
    let only = std::env::var("M2_WORLD_MAP").ok();
    let mut cpp_args = vec![client.as_os_str().to_owned(), INDEX.into(), "maps".into()];
    if let Some(m) = &only {
        cpp_args.push(m.into());
    }
    let cpp_args: Vec<&std::ffi::OsStr> = cpp_args.iter().map(|s| s.as_os_str()).collect();
    let cpp = run(&cpp_tool("world"), &cpp_args);

    let manager = manager(&client);
    let mut props = PropertyManager::default();
    props.initialize(&manager);
    let files: BTreeSet<Vec<u8>> = manager.file_names().map(<[u8]>::to_vec).collect();
    let mut dirs = BTreeSet::new();
    for name in &files {
        let base = base_name(name);
        if (base == b"setting.txt" || base == b"mapproperty.txt") && name.len() > base.len() {
            dirs.insert(name[..name.len() - base.len() - 1].to_vec());
        }
    }
    let mut rust = Vec::new();
    for dir in &dirs {
        if only.as_ref().is_some_and(|o| o.as_bytes() != dir.as_slice()) {
            continue;
        }
        dump_map(&mut rust, &manager, dir, &files, &props);
    }
    let lines = assert_same_lines(&cpp, &rust);
    eprintln!("world: {} maps, {lines} dump lines identical", dirs.len());
}
