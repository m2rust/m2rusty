//! Differential tests: the Rust SpeedTree port against `CSpeedTreeRT` on
//! real client data and on synthetic `.spt` files and geometry caches. Both
//! sides print every value `CSpeedTreeWrapper` reads back (floats as bits)
//! under several seeds, positions, sizes and cameras.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::spt::{TREE_CACHE_MAGIC, fnv1a32, tree_cache_file_name};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const RUST_SPT_DUMP: &str = env!("CARGO_BIN_EXE_m2-spt-dump");

fn assert_same_dump(args: &[&OsStr]) -> Vec<u8> {
    let cpp = run(&cpp_tool("spt"), args);
    let rust = run(Path::new(RUST_SPT_DUMP), args);
    assert_same_lines(&cpp, &rust);
    cpp
}

fn count_lines(dump: &[u8], prefix: &[u8]) -> usize {
    dump.split(|&b| b == b'\n').filter(|l| l.starts_with(prefix)).count()
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_spt_files_match_cpp() {
    let client = client_dir();
    let dump = assert_same_dump(&[OsStr::new("pack"), client.as_os_str()]);
    let files = count_lines(&dump, b"file\t");
    assert!(files > 0, "no .spt files found");
    assert_eq!(count_lines(&dump, b"variant\t"), files * 5);
    assert!(!dump.windows(11).any(|w| w == b"load failed"));
    eprintln!("{files} .spt files identical ({} dump lines)", dump.split(|&b| b == b'\n').count());
}

/// Little-endian writer for `.tree` cache fixtures (layout in
/// `TreeCacheFormat.h`).
#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn f32s(&mut self, v: &[f32]) -> &mut Self {
        for x in v {
            self.0.extend_from_slice(&x.to_le_bytes());
        }
        self
    }
}

struct Part {
    vertices: u32,
    strips: u32,
    indices: Vec<u16>,
    alpha: f32,
}

impl Part {
    fn none() -> Self {
        Part { vertices: 0, strips: 0, indices: Vec::new(), alpha: 0.0 }
    }
}

struct LeafSet {
    active: bool,
    alpha: f32,
    count: u32,
}

/// Leaf `i`'s corners: a rotated, non-square quad (or degenerate/vertical
/// ones to exercise the half-size and angle maths).
fn leaf_corners(i: u32) -> [f32; 16] {
    let f = i as f32;
    match i % 5 {
        3 => [0.0; 16],
        4 => {
            let mut c = [1.0; 16];
            c[6] = -2.5; // vertical top edge: atan2(dz, 0)
            c
        }
        _ => {
            let (w, h) = (10.0 + f, 4.0 + 0.5 * f);
            let (ca, sa) = ((0.3 * f).cos(), (0.3 * f).sin());
            let tl = [-w * ca, -w * sa, h];
            let tr = [w * ca, w * sa, h * 0.9];
            let br = [w * ca, w * sa, -h];
            let bl = [-w * ca, -w * sa, -h * 1.1];
            [tl[0], tl[1], tl[2], 1.0, tr[0], tr[1], tr[2], 1.0, br[0], br[1], br[2], 1.0, bl[0], bl[1], bl[2], 1.0]
        }
    }
}

struct Cache {
    magic: u32,
    bbox: [f32; 6],
    collisions: Vec<(i32, [f32; 6])>,
    parts: [Part; 2],
    leaves: [LeafSet; 2],
    trailer: bool,
}

impl Cache {
    fn valid() -> Self {
        Cache {
            magic: TREE_CACHE_MAGIC,
            bbox: [-10.0, -20.0, -0.5, 10.0, 20.0, 333.25],
            collisions: vec![(0, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), (2, [-1.0, 0.5, 9.0, 0.25, 7.0, 8.0])],
            parts: [
                // 10 indices in 3 strips: 3, 3 and the remaining 4.
                Part { vertices: 6, strips: 3, indices: vec![0, 1, 2, 2, 3, 4, 4, 5, 0, 1], alpha: 0.25 },
                // 2 indices in 3 strips: two empty strips, then both.
                Part { vertices: 4, strips: 3, indices: vec![3, 1], alpha: 0.75 },
            ],
            leaves: [LeafSet { active: true, alpha: 84.0, count: 7 }, LeafSet { active: true, alpha: 9.0, count: 3 }],
            trailer: true,
        }
    }

    fn bytes(&self, hash: u32) -> Vec<u8> {
        let mut w = W::default();
        w.u32(self.magic).u32(hash).f32s(&self.bbox).u32(self.collisions.len() as u32);
        for (kind, v) in &self.collisions {
            w.i32(*kind).f32s(v);
        }
        for (p, part) in self.parts.iter().enumerate() {
            w.u32(part.vertices).u32(part.strips).u16(3).f32s(&[part.alpha]).u32(part.indices.len() as u32);
            for &i in &part.indices {
                w.u16(i);
            }
            let n = part.vertices;
            let base = 100.0 * p as f32;
            for i in 0..n {
                let f = i as f32;
                w.f32s(&[base + f, -f * 0.5, f * f]);
            }
            for i in 0..n {
                w.u32(0x8040_2000 | i);
            }
            for i in 0..n {
                w.f32s(&[i as f32 / 8.0, 1.0 - i as f32 / 8.0]);
            }
            for i in 0..n {
                w.f32s(&[0.5, i as f32 * 0.125]);
            }
            for _ in 0..n * 3 {
                w.f32s(&[0.0]);
            }
        }
        for set in &self.leaves {
            w.u8(u8::from(set.active)).i32(0).f32s(&[set.alpha]).u32(set.count);
            for i in 0..set.count {
                w.f32s(&[i as f32, 2.0 * i as f32, 50.0 + i as f32]);
            }
            for i in 0..set.count {
                w.u32(0xFF00_0000 | (i * 0x10101));
            }
            for i in 0..set.count {
                w.f32s(&leaf_corners(i));
                w.f32s(&[0.0, 0.5, 0.25, 0.5, 0.25, 0.25, 0.0, 0.25]);
                w.u8(i as u8 % 3).u8(i as u8 % 4);
            }
        }
        if self.trailer {
            for active in [1u8, 0] {
                w.u8(active).f32s(&[0.5]).f32s(&[1.0; 20]);
            }
            for name in [&b"SyntheticBark.dds"[..], b"SyntheticComposite.dds", b"SyntheticShadow.dds"] {
                w.u32(name.len() as u32);
                w.0.extend_from_slice(name);
            }
        }
        w.0
    }
}

/// Synthetic `.spt` files: texture-name and size-scan edge cases.
fn synthetic_spts() -> Vec<(&'static str, Vec<u8>)> {
    let size = |v: f32| {
        let mut b = vec![0xD1, 0x07, 0x00, 0x00];
        b.extend_from_slice(&v.to_le_bytes());
        b
    };
    let mut names = b"__IdvSpt_02_\0D:\\Tree\\\\Composite\\CompositeMapB1#N.tga\0Compo\\x.tga\0".to_vec();
    names.extend_from_slice(b"CompositeShadowMap.tga\0SelfShadow.TGA\0shadow.dds\0\"Leaf_1.tga\"\0LEAVES.dds\0");
    names.extend_from_slice(b"Frondy#.tga\0#x.TGA\0bark.dds\0OakBark.tga\0trunk.tga\0x#y\0abc\0dir/\0'Fern.tga'\0");
    names.extend_from_slice(&size(15.0));
    names.extend_from_slice(&size(f32::NAN));
    names.extend_from_slice(&size(150.0));
    names.extend_from_slice(&size(900.0));

    let mut big = b"PalmBark.tga ".to_vec();
    big.extend_from_slice(&size(3999.0));
    let mut tail = b"BirchBark.tga".to_vec();
    tail.extend_from_slice(&[0xD1, 0x07, 0x00, 0x00, 0x00, 0x00, 0x96]); // 7 bytes: never read
    let mut conifer = b"\xB0\xA1JuniperBark.tga\x7F\xFFpine-leaf.tga".to_vec();
    conifer.extend_from_slice(&size(4000.0));

    vec![
        ("empty.spt", Vec::new()),
        ("names.spt", names),
        ("palm_big.spt", big),
        ("birch_tail.spt", tail),
        ("conifer.spt", conifer),
        ("baobab.spt", b"BaobabBark.tga".to_vec()),
        ("aloe.spt", b"aloe\0".to_vec()),
        ("nothing.spt", b"\x01\x02\x03".to_vec()),
    ]
}

/// Synthetic `.spt` files and geometry caches, including malformed caches
/// (which C++ ignores, generating the geometry instead) and the search
/// order of the cache directories.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_spts_and_caches_match_cpp() {
    let client = client_dir();
    let mut manager = PackManager::new(&client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", "pack/Index.dev").unwrap();
    let mut real: Vec<&[u8]> = manager.file_names().filter(|n| n.ends_with(b".spt")).collect();
    real.sort_unstable();
    // One real tree of every archetype, to base the cached variants on.
    let pick = |needle: &[u8]| -> Vec<u8> {
        let name = real.iter().find(|n| n.windows(needle.len()).any(|w| w == needle)).unwrap();
        manager.get(name).unwrap()
    };
    let palm = pick(b"palm");
    let beech = pick(b"beech");

    let root = tempfile::tempdir().unwrap();
    let base = root.path().join("base");
    let dirs = [base.join("treecache"), root.path().join("treecache"), base.join("dist/treecache")];
    for d in &dirs {
        fs::create_dir_all(d).unwrap();
    }

    let mut files: Vec<(String, Vec<u8>)> =
        synthetic_spts().into_iter().map(|(n, d)| (n.to_owned(), d)).collect();
    // (cache dir, cache contents) for a variant of a real tree.
    let mut cached = |name: &str, tree: &[u8], dir: usize, cache: Option<&dyn Fn(u32) -> Vec<u8>>| {
        let mut data = tree.to_vec();
        data.extend_from_slice(name.as_bytes());
        let hash = fnv1a32(&data);
        if let Some(cache) = cache {
            fs::write(dirs[dir].join(tree_cache_file_name(hash)), cache(hash)).unwrap();
        }
        files.push((format!("{name}.spt"), data));
    };

    cached("valid", &palm, 0, Some(&|h| Cache::valid().bytes(h)));
    cached("valid_parent_dir", &beech, 1, Some(&|h| Cache::valid().bytes(h)));
    cached("valid_dist_dir", &palm, 2, Some(&|h| Cache::valid().bytes(h)));
    cached("no_trailer", &beech, 0, Some(&|h| Cache { trailer: false, ..Cache::valid() }.bytes(h)));
    cached("first_leaf_set_inactive", &palm, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.leaves[0].active = false;
        c.bytes(h)
    }));
    cached("no_leaves", &palm, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.leaves = [LeafSet { active: true, alpha: 1.0, count: 0 }, LeafSet { active: false, alpha: 2.0, count: 0 }];
        c.collisions.clear();
        c.bytes(h)
    }));
    cached("branches_without_vertices", &beech, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.parts[0] = Part { vertices: 0, strips: 1, indices: vec![0, 1, 2], alpha: 0.5 };
        c.bytes(h)
    }));
    cached("fronds_without_indices", &beech, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.parts[1] = Part { vertices: 3, strips: 0, indices: Vec::new(), alpha: 0.5 };
        c.bytes(h)
    }));
    cached("no_parts", &beech, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.parts = [Part::none(), Part::none()];
        c.bytes(h)
    }));
    cached("long_strip", &palm, 0, Some(&|h| {
        // 70000 indices: C++ reports the length as u16 (4464).
        let mut c = Cache::valid();
        c.parts[0] = Part { vertices: 3, strips: 1, indices: (0..70000u32).map(|i| (i % 3) as u16).collect(), alpha: 1.0 };
        c.bytes(h)
    }));
    // Malformed caches: generated geometry instead.
    cached("bad_magic", &palm, 0, Some(&|h| Cache { magic: 0x1234_5678, ..Cache::valid() }.bytes(h)));
    cached("hash_mismatch", &palm, 0, Some(&|h| Cache::valid().bytes(h ^ 1)));
    cached("empty_cache", &palm, 0, Some(&|_| Vec::new()));
    cached("header_only", &palm, 0, Some(&|h| Cache::valid().bytes(h)[..30].to_vec()));
    cached("truncated_leaves", &palm, 0, Some(&|h| {
        let b = Cache { trailer: false, ..Cache::valid() }.bytes(h);
        b[..b.len() - 1].to_vec()
    }));
    cached("strips_without_count", &palm, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.parts[1] = Part { vertices: 2, strips: 0, indices: vec![0, 1], alpha: 0.5 };
        c.bytes(h)
    }));
    cached("too_many_vertices", &palm, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.parts[0] = Part { vertices: 0x10000, strips: 1, indices: vec![0], alpha: 0.5 };
        c.bytes(h)
    }));
    cached("too_many_strips", &palm, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.parts[0].strips = 0x10000;
        c.bytes(h)
    }));
    cached("too_many_leaves", &palm, 0, Some(&|h| {
        let mut c = Cache::valid();
        c.leaves[1].count = 0x10000;
        c.bytes(h)
    }));
    cached("too_many_collisions", &palm, 0, Some(&|h| {
        let mut b = Cache::valid().bytes(h);
        b[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        b
    }));
    cached("uncached", &beech, 0, None);
    // A broken cache in the first directory hides a valid one further on.
    cached("shadowed", &palm, 0, Some(&|h| Cache::valid().bytes(h)[..40].to_vec()));
    let shadowed = files.last().unwrap().1.clone();
    fs::write(dirs[2].join(tree_cache_file_name(fnv1a32(&shadowed))), Cache::valid().bytes(fnv1a32(&shadowed)))
        .unwrap();

    for (name, data) in &files {
        fs::write(base.join(name), data).unwrap();
    }
    let mut args: Vec<&OsStr> = vec![OsStr::new("files"), base.as_os_str()];
    args.extend(files.iter().map(|(n, _)| OsStr::new(n.as_str())));
    let dump = assert_same_dump(&args);
    // Only baked trees have a box collision: the 9 valid caches with
    // collisions, in all 5 variants (malformed and shadowed ones fell back).
    assert_eq!(count_lines(&dump, b"collision\t1\t2\t"), 9 * 5);
    eprintln!("{} synthetic files identical", count_lines(&dump, b"file\t"));
}
