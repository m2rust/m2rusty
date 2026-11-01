//! Differential tests: the Rust map readers against the C++ loaders
//! (`tools/differential/map`, which documents the dump format and which
//! parts are real C++). Every map directory, texture set and atlas file
//! registered by the client's index is loaded on both sides.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io::Write;
use std::path::Path;

use m2_formats::map::{
    Area, MapProperty, MapSetting, MonsterAreaKind, TERRAIN_TEX_COORD_BASE, Terrain, TextureSet, area_dir_name,
    parse_area_ambience_data, parse_area_data, parse_atlas_info, parse_monster_areas, splat_alpha_matrix,
    static_shadow_matrix,
};
use m2_formats::pack::{PackManager, SearchMode, load_index};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const INDEX: &str = "pack/Index.dev";

fn manager(client: &Path) -> PackManager {
    let mut manager = PackManager::new(client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", INDEX).unwrap();
    manager
}

fn cpp(client: &Path, mode: &str) -> Vec<u8> {
    run(&cpp_tool("map"), &[client.as_os_str(), INDEX.as_ref(), mode.as_ref()])
}

fn fnv(data: impl IntoIterator<Item = u8>) -> u64 {
    data.into_iter()
        .fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

fn base_name(name: &[u8]) -> &[u8] {
    name.rsplit(|&b| b == b'/').next().unwrap()
}

fn put_f(out: &mut Vec<u8>, f: f32) {
    write!(out, "\t{:08x}", f.to_bits()).unwrap();
}

fn put_matrix(out: &mut Vec<u8>, m: &[f32; 16]) {
    m.iter().for_each(|&f| put_f(out, f));
}

fn line(out: &mut Vec<u8>, head: &str, bytes: &[u8]) {
    out.extend_from_slice(head.as_bytes());
    out.extend_from_slice(bytes);
    out.push(b'\n');
}

fn dump_texture_set(out: &mut Vec<u8>, set: &TextureSet, ok: bool) {
    write!(out, "T\t{}\t", u8::from(ok)).unwrap();
    out.extend_from_slice(&set.file_name);
    writeln!(out, "\t{}", set.textures.len()).unwrap();
    for (i, t) in set.textures.iter().enumerate() {
        write!(out, "TT\t{i}\t").unwrap();
        out.extend_from_slice(&t.file_name);
        for f in [t.u_scale, t.v_scale, t.u_offset, t.v_offset] {
            put_f(out, f);
        }
        write!(out, "\t{}\t{}\t{}", u8::from(t.splat), t.begin, t.end).unwrap();
        put_matrix(out, &t.transform);
        out.push(b'\n');
    }
}

#[derive(Default)]
struct Counts(BTreeMap<&'static str, usize>);

impl Counts {
    fn add(&mut self, kind: &'static str, ok: bool) {
        *self.0.entry(kind).or_default() += usize::from(ok);
    }
    fn report(&self, what: &str) {
        let parts: Vec<String> = self.0.iter().map(|(k, n)| format!("{k}: {n}")).collect();
        eprintln!("{what}: {}", parts.join(", "));
    }
}

fn dump_area(out: &mut Vec<u8>, manager: &PackManager, dir: &[u8], x: u16, y: u16, registered: &HashSet<u32>, counts: &mut Counts) {
    let area_dir = area_dir_name(dir, x, y);
    let read = |file: &str| {
        let mut name = area_dir.clone();
        name.extend_from_slice(file.as_bytes());
        manager.get(name).ok()
    };
    let is_registered = |crc| registered.contains(&crc);
    let objects = read("AreaData.txt").map(|d| parse_area_data(&d, is_registered));
    let ambience = read("AreaAmbienceData.txt").map(|d| parse_area_ambience_data(&d, is_registered));
    let ok = |r: &Option<Result<_, _>>| r.as_ref().is_some_and(Result::is_ok);
    counts.add("AreaData.txt", ok(&objects));
    counts.add("AreaAmbienceData.txt", ok(&ambience));

    let mut all = Vec::new();
    for r in [&objects, &ambience] {
        if let Some(Ok(v)) = r {
            all.extend_from_slice(v);
        }
    }
    writeln!(out, "A\t{}\t{}\t{}", u8::from(ok(&objects)), u8::from(ok(&ambience)), all.len()).unwrap();
    for o in &all {
        out.extend_from_slice(b"AO");
        o.position.iter().for_each(|&f| put_f(out, f));
        write!(out, "\t{}", o.crc).unwrap();
        for f in [o.yaw, o.pitch, o.roll, o.height_bias] {
            put_f(out, f);
        }
        for id in o.portal_ids {
            write!(out, "\t{id}").unwrap();
        }
        write!(out, "\t{}", o.range).unwrap();
        put_f(out, o.max_volume_area_percentage);
        out.push(b'\n');
    }

    *counts.0.entry("objects").or_default() += all.len();
    let area = Area::load(manager, &area_dir, is_registered);
    out.extend_from_slice(b"AS");
    for o in &area.objects {
        write!(out, "\t{}", o.crc).unwrap();
    }
    out.push(b'\n');
}

fn dump_terrain(out: &mut Vec<u8>, manager: &PackManager, dir: &[u8], x: u16, y: u16, height_scale: f32, counts: &mut Counts) {
    let (terrain, load) = match Terrain::load(manager, dir, x, y, height_scale) {
        Ok(loaded) => loaded,
        Err(_) => {
            out.extend_from_slice(b"TP\t0\t\n");
            counts.add("AreaProperty.txt", false);
            return;
        }
    };
    counts.add("AreaProperty.txt", true);
    for (kind, ok) in [
        ("water.wtr", load.water),
        ("height.raw", load.height),
        ("attr.atr", load.attr),
        ("tile.raw", load.tile),
        ("shadowmap.raw", load.shadow),
    ] {
        counts.add(kind, ok);
    }
    line(out, "TP\t1\t", &terrain.name);
    let b = u8::from;
    writeln!(out, "TL\t{}\t{}\t{}\t{}\t{}", b(load.water), b(load.height), b(load.attr), b(load.tile), b(load.shadow)).unwrap();
    let words = |v: &[u16]| fnv(v.iter().flat_map(|w| w.to_le_bytes()));
    writeln!(
        out,
        "TH\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{:016x}\t{}\t{:016x}\t{:016x}\t{:016x}",
        words(&terrain.height_map),
        fnv(terrain.normal_map.iter().map(|&c| c as u8)),
        fnv(terrain.attr_map.iter().copied()),
        fnv(terrain.tile_map.iter().copied()),
        fnv(terrain.water_map.iter().copied()),
        terrain.num_water,
        fnv(terrain.water_heights.iter().flat_map(|h| h.to_le_bytes())),
        words(&terrain.shadow_map),
        fnv(terrain.patch_tile_count.iter().flatten().flat_map(|c| c.to_le_bytes())),
    )
    .unwrap();
    for (i, &n) in terrain.tile_count.iter().enumerate() {
        if n != 0 {
            writeln!(out, "TC\t{i}\t{n}").unwrap();
        }
    }
}

fn dump_map(out: &mut Vec<u8>, manager: &PackManager, name: &[u8], files: &BTreeSet<Vec<u8>>, registered: &HashSet<u32>, counts: &mut Counts) {
    line(out, "M\t", name);
    let property = MapProperty::load(manager, name);
    counts.add("MapProperty.txt", property.error.is_none());
    write!(out, "MP\t{}\t{}\t", u8::from(property.error.is_none()), property.map_type as i32).unwrap();
    line(out, "", &property.parent_map_name);

    let dir = property.data_directory(name).to_vec();
    let setting = MapSetting::load(manager, &dir);
    counts.add("Setting.txt", setting.error.is_none());
    write!(
        out,
        "S\t{}\t{}\t{}",
        u8::from(setting.error.is_none()),
        u8::from(setting.terrain_visible),
        setting.view_radius
    )
    .unwrap();
    put_f(out, setting.height_scale);
    let (cx, cy) = setting.terrain_count;
    let (bx, by) = setting.base;
    write!(out, "\t{cx}\t{cy}\t{bx}\t{by}\t").unwrap();
    line(out, "", &setting.environment);
    if setting.error.is_none() {
        out.extend_from_slice(b"SM");
        put_matrix(out, &splat_alpha_matrix());
        put_matrix(out, &static_shadow_matrix());
        out.push(b'\n');
    }
    dump_texture_set(out, &setting.texture_set, true);
    let env = setting.environment_file_name(name, &dir, |n| manager.exists(n));
    line(out, "E\t", &env);

    let mut regen_name = dir.clone();
    regen_name.extend_from_slice(b"\\regen.txt");
    let areas = manager.get(&regen_name).map(|d| parse_monster_areas(&d));
    counts.add("regen.txt", areas.is_ok());
    let areas = areas.as_deref().unwrap_or_default();
    writeln!(out, "R\t{}\t{}", u8::from(manager.exists(&regen_name)), areas.len()).unwrap();
    for a in areas {
        let (l, t, r, b) = a.bounds();
        let (vid, group, kind) = match a.kind {
            MonsterAreaKind::Monster => (a.id, 0, 1),
            MonsterAreaKind::Group => (0, a.id, 2),
        };
        writeln!(
            out,
            "RA\t{kind}\t{}\t{}\t{}\t{}\t{l}\t{t}\t{r}\t{b}\t{vid}\t{group}\t{}\t{}",
            a.origin.0, a.origin.1, a.size.0, a.size.1, a.count, a.direction
        )
        .unwrap();
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
    for id in ids {
        let (x, y) = ((id / 1000) as u16, (id % 1000) as u16);
        writeln!(out, "D\t{id:06}").unwrap();
        dump_area(out, manager, &dir, x, y, registered, counts);
        dump_terrain(out, manager, &dir, x, y, setting.height_scale, counts);
    }
}

fn registered_properties(client: &Path) -> HashSet<u32> {
    let props = cpp(client, "props");
    let set: HashSet<u32> = props
        .split(|&b| b == b'\n')
        .filter_map(|l| l.strip_prefix(b"P\t"))
        .map(|crc| std::str::from_utf8(crc).unwrap().parse().unwrap())
        .collect();
    assert!(!set.is_empty(), "no properties registered");
    set
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_maps_match_cpp() {
    let client = client_dir();
    let registered = registered_properties(&client);
    let manager = manager(&client);
    let files: BTreeSet<Vec<u8>> = manager.file_names().map(<[u8]>::to_vec).collect();
    let dirs: BTreeSet<&[u8]> = files
        .iter()
        .filter_map(|f| {
            let base = base_name(f);
            (matches!(base, b"setting.txt" | b"mapproperty.txt") && f.len() > base.len())
                .then(|| &f[..f.len() - base.len() - 1])
        })
        .collect();

    let mut out = Vec::new();
    let mut counts = Counts::default();
    for dir in &dirs {
        dump_map(&mut out, &manager, dir, &files, &registered, &mut counts);
    }
    let lines = assert_same_lines(&cpp(&client, "maps"), &out);
    eprintln!("maps: {} directories, {lines} lines identical", dirs.len());
    counts.report("loaded OK");
    assert!(counts.0.get("height.raw").is_some_and(|&n| n > 0), "no terrains loaded");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_texture_sets_match_cpp() {
    let client = client_dir();
    let manager = manager(&client);
    let mut names: Vec<&[u8]> = manager
        .file_names()
        .filter(|n| n.starts_with(b"textureset/") && n.ends_with(b".txt"))
        .collect();
    names.sort_unstable();
    let mut out = Vec::new();
    let mut ok = 0;
    for name in &names {
        match TextureSet::load(&manager, name, TERRAIN_TEX_COORD_BASE) {
            Ok(set) => {
                ok += 1;
                dump_texture_set(&mut out, &set, true);
            }
            Err(_) => dump_texture_set(&mut out, &TextureSet::default(), false),
        }
    }
    let lines = assert_same_lines(&cpp(&client, "textures"), &out);
    eprintln!("texture sets: {} files, {ok} loaded, {lines} lines identical", names.len());
    assert!(ok > 0);
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_atlas_info_matches_cpp() {
    let client = client_dir();
    let manager = manager(&client);
    let mut names: Vec<&[u8]> = manager.file_names().filter(|n| base_name(n) == b"atlasinfo.txt").collect();
    names.sort_unstable();
    let mut out = Vec::new();
    let mut entries = 0;
    for name in &names {
        line(&mut out, "F\t", name);
        for e in parse_atlas_info(&manager.get(name).unwrap()) {
            entries += 1;
            line(&mut out, "I\t", &e.name);
            out.pop();
            writeln!(out, "\t{}\t{}\t{}\t{}\t{}\t{}", e.base_x, e.base_y, e.size_x, e.size_y, e.end_x, e.end_y).unwrap();
        }
    }
    let lines = assert_same_lines(&cpp(&client, "atlas"), &out);
    eprintln!("atlas: {} files, {entries} entries, {lines} lines identical", names.len());
    assert!(entries > 0);
}
