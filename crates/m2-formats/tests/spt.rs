//! Unit tests for the SpeedTree port that need no client data.

use m2_formats::spt::{
    CO_CYLINDER, Error, LoadParams, Shape, Spt, TREE_CACHE_MAGIC, Tree, fnv1a32, leaf_billboards, load_baked,
    tree_cache_file_name,
};

fn le(out: &mut Vec<u8>, words: &[u32]) {
    for w in words {
        out.extend_from_slice(&w.to_le_bytes());
    }
}

/// A minimal valid cache: no collisions, `vertices` branch vertices in one
/// strip of `indices` indices, no fronds, no leaves.
fn cache(hash: u32, vertices: u32, indices: u32) -> Vec<u8> {
    let mut b = Vec::new();
    le(&mut b, &[TREE_CACHE_MAGIC, hash]);
    le(&mut b, &[0; 6]); // bbox
    le(&mut b, &[0]); // collisions
    le(&mut b, &[vertices, u32::from(indices > 0)]);
    b.extend_from_slice(&0u16.to_le_bytes());
    le(&mut b, &[0.5f32.to_bits(), indices]);
    b.resize(b.len() + indices as usize * 2 + vertices as usize * 44, 0);
    le(&mut b, &[0, 0]); // fronds
    b.extend_from_slice(&0u16.to_le_bytes());
    le(&mut b, &[0, 0]);
    for _ in 0..2 {
        b.push(1);
        le(&mut b, &[0, 0, 0]);
    }
    b
}

#[test]
fn empty_spt_is_an_error() {
    assert_eq!(Spt::parse(b""), Err(Error::Empty));
    assert_eq!(Tree::load(b"", None, &LoadParams::default()), Err(Error::Empty));
}

#[test]
fn textures_and_shape() {
    let spt = Spt::parse(b"D:\\Tree\\\\PalmBark.tga\0CompositeMap#2.tga\0CompositeShadowMap.tga\0Leaf.tga\0Frond.tga")
        .unwrap();
    assert_eq!(spt.shape, Shape::Palm);
    assert_eq!(spt.textures.branch.as_deref(), Some(&b"PalmBark.tga"[..]));
    assert_eq!(spt.textures.composite.as_deref(), Some(&b"CompositeMap.tga"[..]));
    assert_eq!(spt.textures.self_shadow, None);
    assert_eq!(spt.textures.leaves, [b"Leaf.tga".to_vec()]);
    assert_eq!(spt.textures.fronds, [b"Frond.tga".to_vec()]);
    assert_eq!(spt.size, 400.0);
}

#[test]
fn size_property() {
    let mut data = b"xx".to_vec();
    data.extend_from_slice(&[0xD1, 7, 0, 0]);
    data.extend_from_slice(&50.0f32.to_le_bytes());
    assert_eq!(Spt::parse(&data).unwrap().size, 200.0);
}

#[test]
fn generated_geometry_shape() {
    let spt = Spt::parse(b"BeechBark.tga").unwrap();
    assert_eq!(spt.shape, Shape::Broadleaf);
    let g = spt.generate(1, [0.0; 3]);
    // Trunk, 5 limbs and 10 twigs, each two 14-vertex tubes.
    assert_eq!(g.branches.vertices.len(), 16 * 2 * 14);
    // Each tube after the first adds 3 stitch indices and 13 more.
    assert_eq!(g.branches.strip.len(), 14 + 31 * 16);
    assert!(g.branches.strip.iter().all(|&i| usize::from(i) < g.branches.vertices.len()));
    assert_eq!(g.leaves.len(), 96);
    assert_eq!(g.bounding_box, [-160.0, -160.0, 0.0, 160.0, 160.0, 400.0]);
    assert_eq!(g.collisions[0].kind, CO_CYLINDER);
    assert_eq!(g.fronds.vertex_count(), 0);
    assert_eq!(leaf_billboards(&g.leaves, [0.0; 3], [5.0, 5.0, 5.0]).len(), 96);
    // Same inputs, same tree; another seed moves the leaves only.
    assert_eq!(spt.generate(1, [0.0; 3]), g);
    let other = spt.generate(2, [0.0; 3]);
    assert_eq!(other.branches, g.branches);
    assert_ne!(other.leaves, g.leaves);
}

#[test]
fn baked_cache() {
    let spt = b"tree";
    let hash = fnv1a32(spt);
    assert_eq!(tree_cache_file_name(0xab), "000000ab.tree");

    let tree = Tree::load(spt, Some(&cache(hash, 3, 4)), &LoadParams::default()).unwrap();
    assert!(tree.baked);
    assert_eq!(tree.spt.size, 400.0);
    assert_eq!(tree.geometry.branches.vertices.len(), 3);
    assert_eq!(tree.geometry.branches.alpha_test, 0.5);
    assert!(tree.geometry.leaves.is_empty());
    assert_eq!(tree.geometry.leaf_alpha_test, 128.0);

    // A wrong cache is ignored.
    let tree = Tree::load(spt, Some(&cache(hash ^ 1, 3, 4)), &LoadParams::default()).unwrap();
    assert!(!tree.baked);
    assert!(matches!(load_baked(&cache(hash, 0, 0), hash ^ 1), Err(Error::HashMismatch { .. })));
}

#[test]
fn long_strip_is_reported_as_u16() {
    let g = load_baked(&cache(7, 3, 70000), 7).unwrap();
    assert_eq!(g.branches.strip.len(), 70000);
    assert_eq!(g.branches.strip_length(), (70000 % 65536) as u16);
    assert_eq!(g.branches.strip_indices().len(), 4464);
}

/// Deviation: C++ loops over the normals with an `unsigned short` and hangs
/// for more than 21845 vertices; the port reads them.
#[test]
fn many_vertices_terminate() {
    let g = load_baked(&cache(7, 30000, 3), 7).unwrap();
    assert_eq!(g.branches.vertex_count(), 30000);
}

#[test]
fn malformed_caches_are_errors() {
    let good = cache(7, 3, 4);
    for len in 0..good.len() {
        assert!(load_baked(&good[..len], 7).is_err(), "prefix of {len} bytes accepted");
    }
    let mut huge = good.clone();
    huge[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(load_baked(&huge, 7).is_err());
    let mut bad_magic = good;
    bad_magic[0] ^= 1;
    assert!(matches!(load_baked(&bad_magic, 7), Err(Error::BadMagic(_))));

    // Arbitrary bytes never panic.
    let mut seed = 1u32;
    for _ in 0..2000 {
        let len = (seed >> 20) as usize;
        let mut data = vec![0u8; len];
        for b in &mut data {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *b = (seed >> 24) as u8;
        }
        if data.len() >= 8 {
            data[..4].copy_from_slice(&TREE_CACHE_MAGIC.to_le_bytes());
            data[4..8].copy_from_slice(&7u32.to_le_bytes());
        }
        let _ = load_baked(&data, 7);
        let _ = Spt::parse(&data);
    }
}
