//! Rust counterpart of `m2diff_spt` (`tools/differential/spt/SptDump.cpp`).
//!
//!   m2-spt-dump pack <client dir> [index file, default pack/Index.dev]
//!   m2-spt-dump files <base dir> <spt file>...
//!
//! Loads every `.spt` (in the packs, sorted by name, or the given files) as
//! `CSpeedTreeWrapper::LoadTree` does, under a few seeds, positions, sizes
//! and cameras, and prints everything `GetGeometry`, `GetTextures`,
//! `GetBoundingBox` and `GetCollisionObject` report. Floats are printed as
//! the hex of their bits. Cache files are looked up like
//! `LoadBakedGeometry`, relative to the client / base dir.

use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::ExitCode;

use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::spt::{
    DEFAULT_MATERIAL, Indexed, LoadParams, TREE_CACHE_DIRS, TREE_CACHE_ENV, Tree, leaf_billboards,
    tree_cache_file_name,
};

/// One way of loading a tree; the same table is in `SptDump.cpp`.
struct Variant {
    name: &'static str,
    seed: u32,
    position: [f32; 3],
    size: Option<f32>,
    camera: [f32; 3],
}

const VARIANTS: [Variant; 5] = [
    Variant { name: "engine", seed: 1, position: [0.0; 3], size: None, camera: [0.0; 3] },
    Variant { name: "seed", seed: 12345, position: [37.5, 81.25, 3.0], size: None, camera: [500.0, -300.0, 200.0] },
    Variant {
        name: "negative",
        seed: 0xFFFF_FFFF,
        position: [-123.4, -5.5, 0.0],
        size: None,
        camera: [-123.4, -5.5, 100.0],
    },
    Variant { name: "size1000", seed: 2, position: [0.0; 3], size: Some(1000.0), camera: [0.0; 3] },
    Variant { name: "sizehalf", seed: 3, position: [0.0; 3], size: Some(0.5), camera: [1.0, 2.0, 3.0] },
];

/// The engine tree's instance (`MakeInstance` + `SetTreePosition`).
const INSTANCE_POSITION: [f32; 3] = [1000.0, 2000.0, 30.0];
const INSTANCE_CAMERA: [f32; 3] = [10.0, 20.0, 1500.0];

fn hex(values: &[f32]) -> String {
    values.iter().map(|v| format!("{:08x}", v.to_bits())).collect::<Vec<_>>().join(" ")
}

fn name_or_dash(name: Option<&[u8]>) -> &[u8] {
    name.unwrap_or(b"-")
}

fn find_cache(base: &Path, hash: u32) -> Option<Vec<u8>> {
    let file = tree_cache_file_name(hash);
    let mut dirs: Vec<_> = TREE_CACHE_DIRS.iter().map(|d| base.join(d)).collect();
    if let Some(env) = std::env::var_os(TREE_CACHE_ENV).filter(|e| !e.is_empty()) {
        dirs.push(base.join(env));
    }
    // The first file that opens is used, valid or not.
    dirs.iter().find_map(|d| std::fs::read(d.join(&file)).ok())
}

fn dump_indexed(out: &mut Vec<u8>, label: &str, part: &Indexed, is_fronds: bool) {
    let empty = part.vertices.is_empty();
    let lod = if is_fronds && empty { -1 } else { 0 };
    // GetGeometry reports one frond strip whenever there are frond vertices.
    let strips = if is_fronds { u16::from(!empty) } else { part.num_strips() };
    let len = if strips > 0 { part.strip_length() } else { 0 };
    writeln!(out, "{label}\t{}\t{lod}\t{strips}\t{}\t{len}", part.vertex_count(), hex(&[part.alpha_test])).unwrap();
    for (i, v) in part.vertices.iter().enumerate() {
        writeln!(
            out,
            "v\t{i}\t{}\t{:08x}\t{}\t{}",
            hex(&v.position),
            v.color,
            hex(&v.tex_coords0),
            hex(&v.tex_coords1)
        )
        .unwrap();
    }
    if strips > 0 {
        let indices: Vec<String> = part.strip_indices().iter().map(u16::to_string).collect();
        writeln!(out, "strip\t{}", indices.join(" ")).unwrap();
    }
}

fn dump_tree(out: &mut Vec<u8>, tree: &Tree, v: &Variant) {
    let spt = &tree.spt;
    let g = &tree.geometry;
    writeln!(out, "variant\t{}", v.name).unwrap();
    writeln!(out, "size\t{}", hex(&[spt.size, spt.size_variance])).unwrap();

    let t = &spt.textures;
    for (label, name) in [("branch", &t.branch), ("composite", &t.composite), ("shadow", &t.self_shadow)] {
        out.extend_from_slice(format!("texture\t{label}\t").as_bytes());
        out.extend_from_slice(name_or_dash(name.as_deref()));
        out.push(b'\n');
    }
    for (label, names) in [("leaf", &t.leaves), ("frond", &t.fronds)] {
        writeln!(out, "{label}_textures\t{}", names.len()).unwrap();
        for name in names {
            out.extend_from_slice(format!("{label}_texture\t").as_bytes());
            out.extend_from_slice(name);
            out.push(b'\n');
        }
    }

    writeln!(out, "bbox\t{}", hex(&g.bounding_box)).unwrap();
    for label in ["branch", "frond", "leaf"] {
        writeln!(out, "material\t{label}\t{}", hex(&DEFAULT_MATERIAL)).unwrap();
    }
    writeln!(out, "collisions\t{}", g.collisions.len()).unwrap();
    for (i, c) in g.collisions.iter().enumerate() {
        writeln!(out, "collision\t{i}\t{}\t{}\t{}", c.kind, hex(&c.position), hex(&c.dimensions)).unwrap();
    }

    dump_indexed(out, "branches", &g.branches, false);
    dump_indexed(out, "fronds", &g.fronds, true);

    let corners = leaf_billboards(&g.leaves, v.position, v.camera);
    writeln!(
        out,
        "leaves0\t{}\t{}\t0\t{}",
        u8::from(!g.leaves.is_empty()),
        hex(&[g.leaf_alpha_test]),
        g.leaves.len() as u16
    )
    .unwrap();
    for (i, (leaf, c)) in g.leaves.iter().zip(&corners).enumerate() {
        writeln!(
            out,
            "leaf\t{i}\t{}\t{:08x}\t{}\t{}\t{}\t{}",
            hex(&leaf.center),
            leaf.color,
            leaf.map_index,
            leaf.cluster_index,
            hex(&leaf.tex_coords),
            hex(c)
        )
        .unwrap();
    }
    writeln!(out, "leaves1\t0\t0\t-1").unwrap();
    writeln!(out, "billboards\t0\t0\t0").unwrap();
    writeln!(out, "triangles\t{}\t{}\t0", g.branch_triangle_count(), g.leaf_triangle_count()).unwrap();
}

fn dump_file(out: &mut Vec<u8>, base: &Path, name: &[u8], data: &[u8]) {
    out.extend_from_slice(b"file\t");
    out.extend_from_slice(name);
    writeln!(out, "\t{}", data.len()).unwrap();

    let cache = find_cache(base, m2_formats::spt::fnv1a32(data));
    let mut engine_tree = None;
    for v in &VARIANTS {
        let params = LoadParams { seed: v.seed, position: v.position, size: v.size.map(|s| (s, 0.0)) };
        match Tree::load(data, cache.as_deref(), &params) {
            Ok(tree) => {
                dump_tree(out, &tree, v);
                engine_tree.get_or_insert(tree);
            }
            Err(_) => {
                writeln!(out, "variant\t{}\tload failed", v.name).unwrap();
            }
        }
    }
    if let Some(tree) = engine_tree {
        for (i, c) in leaf_billboards(&tree.geometry.leaves, INSTANCE_POSITION, INSTANCE_CAMERA).iter().enumerate() {
            writeln!(out, "instance_leaf\t{i}\t{}", hex(c)).unwrap();
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let usage = || {
        eprintln!("usage: {0} pack <client dir> [index] | {0} files <base dir> <spt>...", args[0]);
        ExitCode::from(2)
    };
    let (Some(mode), Some(base)) = (args.get(1), args.get(2)) else {
        return usage();
    };
    let base = Path::new(base);
    let mut stdout = BufWriter::new(std::io::stdout().lock());

    match mode.as_str() {
        "pack" => {
            let index = args.get(3).map_or("pack/Index.dev", String::as_str);
            let mut manager = PackManager::new(base);
            manager.set_search_mode(SearchMode::PackFirst);
            if let Err(e) = load_index(&mut manager, "pack", index) {
                eprintln!("{index}: {e}");
                return ExitCode::FAILURE;
            }
            let mut names: Vec<&[u8]> = manager.file_names().filter(|n| n.ends_with(b".spt")).collect();
            names.sort_unstable();
            for name in names {
                let mut out = Vec::new();
                match manager.get(name) {
                    Ok(data) => dump_file(&mut out, base, name, &data),
                    Err(_) => {
                        out.extend_from_slice(b"file\t");
                        out.extend_from_slice(name);
                        out.extend_from_slice(b"\t-\n");
                    }
                }
                if stdout.write_all(&out).is_err() {
                    return ExitCode::SUCCESS;
                }
            }
        }
        "files" => {
            for file in &args[3..] {
                let data = match std::fs::read(base.join(file)) {
                    Ok(data) => data,
                    Err(e) => {
                        eprintln!("{file}: {e}");
                        return ExitCode::FAILURE;
                    }
                };
                let mut out = Vec::new();
                dump_file(&mut out, base, file.as_bytes(), &data);
                if stdout.write_all(&out).is_err() {
                    return ExitCode::SUCCESS;
                }
            }
        }
        _ => return usage(),
    }
    ExitCode::SUCCESS
}
