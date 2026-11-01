//! Tests for the gr2 reader on synthetic files. The builder and the cases
//! are shared with `differential_gr2.rs`, which feeds the same files to the
//! C++ reader.

#![allow(dead_code)]

use m2_formats::gr2::{self, Error, Indices, Vertices};

/// Writes uncompressed (format 0) gr2 files: sections of raw bytes plus
/// relocations, laid out the way `GrannyParseGR2FromMemory` reads them.
pub mod fixture {
    pub struct Builder {
        pub sections: Vec<Vec<u8>>,
        relocs: Vec<Vec<(u32, u32, u32)>>,
        pub root_type: (u32, u32),
        pub root_node: (u32, u32),
    }

    /// Where a record lives: section and offset.
    pub type At = (u32, u32);

    impl Builder {
        pub fn new(sections: usize) -> Self {
            Builder {
                sections: vec![Vec::new(); sections],
                relocs: vec![Vec::new(); sections],
                root_type: (0, 0),
                root_node: (0, 0),
            }
        }

        /// Appends `bytes` (4-aligned) to `sec`.
        pub fn alloc(&mut self, sec: u32, bytes: &[u8]) -> At {
            let s = &mut self.sections[sec as usize];
            while !s.len().is_multiple_of(4) {
                s.push(0);
            }
            let off = s.len() as u32;
            s.extend_from_slice(bytes);
            (sec, off)
        }

        pub fn zeros(&mut self, sec: u32, len: usize) -> At {
            self.alloc(sec, &vec![0; len])
        }

        pub fn put_u32(&mut self, at: At, rel: u32, v: u32) {
            let o = (at.1 + rel) as usize;
            self.sections[at.0 as usize][o..o + 4].copy_from_slice(&v.to_le_bytes());
        }

        pub fn put_f32s(&mut self, at: At, rel: u32, v: &[f32]) {
            for (i, f) in v.iter().enumerate() {
                self.put_u32(at, rel + i as u32 * 4, f.to_bits());
            }
        }

        /// A relocation: the pointer at `at + rel` points to `to`.
        pub fn ptr(&mut self, at: At, rel: u32, to: At) {
            self.relocs[at.0 as usize].push((at.1 + rel, to.0, to.1));
        }

        /// A NUL-terminated string in `sec`, pointed to from `at + rel`.
        pub fn string(&mut self, at: At, rel: u32, sec: u32, s: &[u8]) {
            let to = self.alloc(sec, &[s, b"\0"].concat());
            self.ptr(at, rel, to);
        }

        pub fn build(&self) -> Vec<u8> {
            let n = self.sections.len();
            let table = 68usize;
            let mut data_off = table + n * 44;
            let mut out = vec![0u8; data_off];
            out[..16].copy_from_slice(&m2_formats::gr2::MAGIC);
            let header = [
                36u32,
                n as u32,
                self.root_type.0,
                self.root_type.1,
                self.root_node.0,
                self.root_node.1,
            ];
            for (i, v) in header.iter().enumerate() {
                out[44 + i * 4..48 + i * 4].copy_from_slice(&v.to_le_bytes());
            }
            let mut entries = Vec::new();
            for (s, relocs) in self.sections.iter().zip(&self.relocs) {
                let off = data_off;
                out.extend_from_slice(s);
                data_off += s.len();
                let reloc_off = data_off;
                for &(a, b, c) in relocs {
                    for v in [a, b, c] {
                        out.extend_from_slice(&v.to_le_bytes());
                    }
                }
                data_off += relocs.len() * 12;
                entries.push([
                    0,
                    off as u32,
                    s.len() as u32,
                    s.len() as u32,
                    4,
                    0,
                    0,
                    reloc_off as u32,
                    relocs.len() as u32,
                ]);
            }
            for (i, e) in entries.iter().enumerate() {
                for (j, v) in e.iter().enumerate() {
                    let at = table + i * 44 + j * 4;
                    out[at..at + 4].copy_from_slice(&v.to_le_bytes());
                }
            }
            out
        }

        /// Root type (`FromFileName`, `Models`, `Animations`) and root node in
        /// section 0. Returns the root node; `Models` is at +4 (count, array
        /// pointer) and `Animations` at +12.
        pub fn root(&mut self) -> At {
            let types = self.zeros(0, 4 * 32);
            for (i, (t, name)) in [(8u32, &b"FromFileName"[..]), (4, b"Models"), (4, b"Animations")]
                .iter()
                .enumerate()
            {
                let m = (0, types.1 + i as u32 * 32);
                self.put_u32(m, 0, *t);
                self.string(m, 4, 0, name);
            }
            let node = self.zeros(0, 20);
            self.root_type = types;
            self.root_node = node;
            node
        }

        /// An array of `n` pointers; returns it.
        pub fn ptr_array(&mut self, sec: u32, targets: &[Option<At>]) -> At {
            let arr = self.zeros(sec, targets.len() * 4);
            for (i, t) in targets.iter().enumerate() {
                if let Some(t) = t {
                    self.ptr(arr, i as u32 * 4, *t);
                }
            }
            arr
        }

        pub fn set_models(&mut self, node: At, count: u32, models: &[Option<At>]) {
            self.put_u32(node, 4, count);
            let arr = self.ptr_array(0, models);
            self.ptr(node, 8, arr);
        }

        pub fn set_animations(&mut self, node: At, count: u32, anims: &[Option<At>]) {
            self.put_u32(node, 12, count);
            let arr = self.ptr_array(0, anims);
            self.ptr(node, 16, arr);
        }

        /// A model record named `name`, no skeleton or meshes yet.
        pub fn model(&mut self, name: &[u8]) -> At {
            let m = self.zeros(0, 84);
            self.string(m, 0, 0, name);
            self.put_u32(m, 8, 7);
            self.put_f32s(
                m,
                12,
                &[
                    1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0,
                ],
            );
            m
        }

        /// A skeleton with `bones` (name, parent) attached to `model`.
        pub fn skeleton(&mut self, model: At, bones: &[(&[u8], i32)]) -> At {
            let sk = self.zeros(0, 12);
            self.string(sk, 0, 0, b"skel");
            self.put_u32(sk, 4, bones.len() as u32);
            let arr = self.zeros(0, bones.len() * 156);
            for (i, (name, parent)) in bones.iter().enumerate() {
                let b = (0, arr.1 + i as u32 * 156);
                self.string(b, 0, 0, name);
                self.put_u32(b, 4, *parent as u32);
                self.put_u32(b, 8, 1);
                self.put_f32s(b, 12, &[i as f32, 0.5, -0.25]);
                self.put_f32s(
                    b,
                    76,
                    &[
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.1, 0.2, 0.3, 1.0,
                    ],
                );
            }
            self.ptr(sk, 8, arr);
            self.ptr(model, 4, sk);
            sk
        }

        pub fn mesh_bindings(&mut self, model: At, count: u32, meshes: &[Option<At>]) {
            self.put_u32(model, 76, count);
            let arr = self.ptr_array(0, meshes);
            self.ptr(model, 80, arr);
        }

        pub fn mesh(&mut self, name: &[u8]) -> At {
            let m = self.zeros(0, 36);
            self.string(m, 0, 0, name);
            m
        }

        /// Vertex data with `count` and raw vertex bytes in `sec`.
        pub fn vertices(&mut self, mesh: At, count: i32, sec: u32, bytes: &[u8]) {
            let vd = self.zeros(0, 12);
            self.put_u32(vd, 4, count as u32);
            let data = self.alloc(sec, bytes);
            self.ptr(vd, 8, data);
            self.ptr(mesh, 4, vd);
        }

        pub fn topology(&mut self, mesh: At) -> At {
            let t = self.zeros(0, 24);
            self.ptr(mesh, 16, t);
            t
        }

        pub fn bone_bindings(&mut self, mesh: At, names: &[&[u8]]) {
            self.put_u32(mesh, 28, names.len() as u32);
            let arr = self.zeros(0, names.len() * 36);
            for (i, n) in names.iter().enumerate() {
                let b = (0, arr.1 + i as u32 * 36);
                self.string(b, 0, 0, n);
                self.put_f32s(b, 4, &[-1.0, -2.0, -3.0, 1.0, 2.0, 3.5]);
            }
            self.ptr(mesh, 32, arr);
        }

        /// A material with maps of (usage, Some(texture name) / None for a
        /// sub-material without texture).
        pub fn material(&mut self, name: &[u8], maps: &[(&[u8], Option<&[u8]>)]) -> At {
            let m = self.zeros(0, 16);
            self.string(m, 0, 0, name);
            self.put_u32(m, 4, maps.len() as u32);
            let arr = self.zeros(0, maps.len() * 8);
            for (i, (usage, tex)) in maps.iter().enumerate() {
                let mp = (0, arr.1 + i as u32 * 8);
                self.string(mp, 0, 0, usage);
                let sub = self.zeros(0, 16);
                if let Some(tex) = tex {
                    let t = self.zeros(0, 4);
                    self.string(t, 0, 0, tex);
                    self.ptr(sub, 12, t);
                }
                self.ptr(mp, 4, sub);
            }
            self.ptr(m, 8, arr);
            m
        }

        pub fn materials(&mut self, mesh: At, count: u32, mats: &[Option<At>]) {
            self.put_u32(mesh, 20, count);
            let arr = self.ptr_array(0, mats);
            self.ptr(mesh, 24, arr);
        }

        pub fn animation(&mut self, name: &[u8], duration: f32) -> At {
            let a = self.zeros(0, 20);
            self.string(a, 0, 0, name);
            self.put_f32s(a, 4, &[duration, 1.0 / 30.0]);
            a
        }

        pub fn track_groups(&mut self, anim: At, count: u32, groups: &[Option<At>]) {
            self.put_u32(anim, 12, count);
            let arr = self.ptr_array(0, groups);
            self.ptr(anim, 16, arr);
        }

        pub fn track_group(&mut self, name: &[u8], len: usize) -> At {
            let g = self.zeros(0, len);
            self.string(g, 0, 0, name);
            if len >= 116 {
                self.put_f32s(g, 104, &[0.5, 0.0, -0.5]);
            }
            g
        }

        /// `count` stored, `tracks` records of 64 bytes in `sec`.
        pub fn tracks(&mut self, group: At, count: u32, sec: u32, names: &[&[u8]]) -> At {
            self.put_u32(group, 12, count);
            let arr = self.zeros(sec, names.len() * 64);
            for (i, n) in names.iter().enumerate() {
                let t = (sec, arr.1 + i as u32 * 64);
                self.string(t, 0, 0, n);
            }
            self.ptr(group, 16, arr);
            arr
        }

        /// Curve at `track + rel` (4, 24 or 44).
        pub fn curve(&mut self, track: At, rel: u32, degree: i32, knots: &[f32], controls: &[f32]) {
            self.put_u32(track, rel, degree as u32);
            self.put_u32(track, rel + 4, knots.len() as u32);
            self.put_u32(track, rel + 12, controls.len() as u32);
            let k = self.zeros(0, knots.len() * 4);
            self.put_f32s(k, 0, knots);
            self.ptr(track, rel + 8, k);
            let c = self.zeros(0, controls.len() * 4);
            self.put_f32s(c, 0, controls);
            self.ptr(track, rel + 16, c);
        }
    }

    fn rigid_vertex_bytes(n: usize) -> Vec<u8> {
        (0..n * 8)
            .flat_map(|i| (i as f32 * 0.25).to_bits().to_le_bytes())
            .collect()
    }

    fn skinned_vertex_bytes(n: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for v in 0..n {
            for i in 0..3 {
                out.extend_from_slice(&((v * 3 + i) as f32).to_le_bytes());
            }
            out.extend_from_slice(&[200, 55, 0, 0, 0, 1, 0, 0]);
            for i in 0..5 {
                out.extend_from_slice(&((v + i) as f32 * -0.5).to_le_bytes());
            }
        }
        out
    }

    /// A model with a skeleton, a rigid mesh (u16 indices) and a skinned
    /// mesh (i32 indices) with materials, plus an animation, spread over
    /// three sections.
    pub fn full() -> Vec<u8> {
        let mut b = Builder::new(3);
        let node = b.root();
        let model = b.model(b"Model01");
        b.skeleton(model, &[(b"Bip01", -1), (b"Bip01 Pelvis", 0), (b"Bip01 \\Spine\t", 1)]);

        let rigid = b.mesh(b"rigid");
        b.vertices(rigid, 3, 1, &rigid_vertex_bytes(3));
        let t = b.topology(rigid);
        b.put_u32(t, 16, 3);
        let idx = b.alloc(2, &[0, 0, 1, 0, 2, 0]);
        b.ptr(t, 20, idx);
        b.put_u32(t, 0, 1);
        let g = b.alloc(0, &[0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]);
        b.ptr(t, 4, g);
        b.bone_bindings(rigid, &[b"Bip01"]);
        let mat = b.material(
            b"mat",
            &[(b"Diffuse Color", Some(b"D:\\ymir work\\a.dds")), (b"Opacity", None)],
        );
        b.materials(rigid, 2, &[Some(mat), None]);

        let skinned = b.mesh(b"skinned");
        b.vertices(skinned, 2, 1, &skinned_vertex_bytes(2));
        let t = b.topology(skinned);
        b.put_u32(t, 8, 3);
        let idx = b.alloc(2, &[2, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        b.ptr(t, 12, idx);
        b.bone_bindings(skinned, &[b"Bip01", b"Bip01 Pelvis"]);
        b.materials(skinned, 1, &[Some(mat)]);

        b.mesh_bindings(model, 3, &[Some(rigid), Some(skinned), None]);
        b.set_models(node, 1, &[Some(model)]);

        let anim = b.animation(b"walk", 1.5);
        let g = b.track_group(b"Model01", 120);
        let tracks = b.tracks(g, 2, 0, &[b"Bip01", b"Bip01 Pelvis"]);
        b.curve(
            tracks,
            4,
            2,
            &[0.0, 0.5, 1.0],
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        );
        b.curve(tracks, 24, 1, &[0.0], &[0.0, 0.0, 0.0, 1.0]);
        let second = (tracks.0, tracks.1 + 64);
        b.curve(second, 44, 0, &[], &[]);
        b.track_groups(anim, 2, &[Some(g), None]);
        b.set_animations(node, 1, &[Some(anim)]);
        b.build()
    }

    /// Records that are missing, out of their section or inconsistent, in
    /// every way the C++ reader handles without reading out of bounds.
    pub fn edge_cases() -> Vec<u8> {
        let mut b = Builder::new(3);
        let node = b.root();

        // Model 0: skeleton with a bone count but no bone array; a mesh
        // binding array with an unresolved entry and one past its section.
        let m0 = b.model(b"edge");
        let sk = b.zeros(0, 12);
        b.put_u32(sk, 4, 5);
        b.ptr(m0, 4, sk);
        let mesh = b.mesh(b"m");
        // 16-bit indices whose pointer resolves but whose data does not fit:
        // no fallback to the valid 32-bit ones.
        let t = b.topology(mesh);
        b.put_u32(t, 16, 1000);
        let i16s = b.alloc(1, &[1, 0]);
        b.ptr(t, 20, i16s);
        b.put_u32(t, 8, 1);
        let i32s = b.alloc(1, &[1, 0, 0, 0]);
        b.ptr(t, 12, i32s);
        // Groups that do not fit.
        b.put_u32(t, 0, 50);
        let g = b.zeros(1, 12);
        b.ptr(t, 4, g);
        // Vertex data without a vertex pointer; negative material count;
        // bone bindings without an array.
        let vd = b.zeros(0, 12);
        b.put_u32(vd, 4, 10);
        b.ptr(mesh, 4, vd);
        b.put_u32(mesh, 20, (-3i32) as u32);
        b.put_u32(mesh, 28, 4);
        let past = (1, 1 << 20);
        b.mesh_bindings(m0, 4, &[Some(mesh), None, Some(past), Some(mesh)]);

        // Model 1: bones of which the last is cut off by the section end, a
        // name without NUL, and a material map with a dangling usage.
        let m1 = b.model(b"tail");
        let sk = b.zeros(0, 12);
        b.put_u32(sk, 4, 2);
        b.ptr(m1, 4, sk);
        let bones = b.zeros(2, 156 + 100);
        b.string(bones, 0, 1, b"first");
        b.ptr(sk, 8, bones);
        let mat = b.zeros(0, 12);
        b.put_u32(mat, 4, 2);
        let maps = b.zeros(0, 16);
        b.ptr(maps, 0, (1, 1 << 20));
        b.ptr(mat, 8, maps);
        let mesh1 = b.mesh(b"");
        b.materials(mesh1, 2, &[Some(mat), Some((0, 1 << 20))]);
        b.mesh_bindings(m1, 1, &[Some(mesh1)]);
        let unterminated = b.alloc(2, b"no-nul");
        b.ptr(m1, 0, unterminated);

        b.set_models(node, 4, &[Some(m0), None, Some((0, 1 << 20)), Some(m1)]);

        // Animation: a group too short for its loop translation, with a
        // track array cut off by its section, curves with negative,
        // too-large and out-of-section counts.
        let anim = b.animation(b"edge", 0.0);
        let g = b.track_group(b"short", 24);
        let g2 = b.track_group(b"empty", 20);
        b.track_groups(anim, 3, &[Some(g), Some((1, 1 << 20)), Some(g2)]);
        // The last 64 bytes of section 1: one track of three.
        b.put_u32(g, 12, 3);
        let tracks = b.zeros(1, 64);
        b.ptr(g, 16, tracks);
        let end4 = (1, tracks.1 + 60);
        b.put_u32(tracks, 4, 3); // position: degree 3
        b.put_u32(tracks, 8, (-1i32) as u32); // knots -1
        b.put_u32(tracks, 16, 300_000); // controls above kMaxRecords
        b.put_u32(tracks, 28, 2); // orientation: 2 knots, 4 bytes left
        b.ptr(tracks, 32, end4);
        b.put_u32(tracks, 56, 1); // scale/shear: 1 control, fits
        b.ptr(tracks, 60, end4);
        // An animation record 8 bytes short of its section end.
        b.set_animations(node, 2, &[Some(anim), Some((1, tracks.1 + 56))]);
        b.build()
    }

    /// The models array pointer has no relocation: C++ keeps the count and
    /// leaves `Models` null.
    pub fn null_models() -> Vec<u8> {
        let mut b = Builder::new(1);
        let node = b.root();
        b.put_u32(node, 4, 3);
        b.put_u32(node, 12, 2000);
        b.build()
    }

    /// A bone count above `kMaxRecords`.
    pub fn too_many_bones() -> Vec<u8> {
        let mut b = Builder::new(1);
        let node = b.root();
        let model = b.model(b"big");
        let sk = b.zeros(0, 12);
        b.put_u32(sk, 4, 200_001);
        b.ptr(model, 4, sk);
        b.set_models(node, 1, &[Some(model)]);
        b.build()
    }

    /// Named synthetic inputs, all safe for the C++ reader.
    pub fn cases() -> Vec<(&'static str, Vec<u8>)> {
        let full = full();
        let mut bad_magic = full.clone();
        bad_magic[3] ^= 1;
        let mut out = vec![
            ("full", full.clone()),
            ("edge_cases", edge_cases()),
            ("null_models", null_models()),
            ("too_many_bones", too_many_bones()),
            ("bad_magic", bad_magic),
            ("short", full[..87].to_vec()),
        ];
        // Truncations cut the section data or relocations off the end.
        for (i, cut) in [88, 200, full.len() / 2, full.len() - 1].into_iter().enumerate() {
            out.push((
                ["truncated_0", "truncated_1", "truncated_2", "truncated_3"][i],
                full[..cut].to_vec(),
            ));
        }
        out
    }
}

#[test]
fn full_file() {
    let f = gr2::parse(&fixture::full()).unwrap();
    assert_eq!(
        (f.model_count, f.models.len(), f.animation_count, f.animations.len()),
        (1, 1, 1, 1)
    );
    let m = &f.models[0];
    assert_eq!(m.name, b"Model01");
    assert_eq!(m.initial_placement.flags, 7);
    assert_eq!(m.initial_placement.position, [1.0, 2.0, 3.0]);

    let sk = m.skeleton.as_ref().unwrap();
    assert_eq!(sk.name, b"skel");
    assert_eq!((sk.bone_count, sk.bones.len()), (3, 3));
    assert_eq!(sk.bones[2].name, b"Bip01 \\Spine\t");
    assert_eq!(sk.bones[2].parent_index, 1);
    assert_eq!(sk.bones[2].local_transform.position, [2.0, 0.5, -0.25]);
    assert_eq!(sk.bones[0].inverse_world[3], [0.1, 0.2, 0.3, 1.0]);

    assert_eq!((m.mesh_binding_count, m.mesh_bindings.len()), (3, 3));
    assert!(m.mesh_bindings[2].is_none());
    let rigid = m.mesh_bindings[0].as_deref().unwrap();
    assert!(rigid.is_rigid());
    let Some(Vertices::Rigid(v)) = &rigid.vertex_data.as_ref().unwrap().vertices else {
        panic!()
    };
    assert_eq!(v.len(), 3);
    assert_eq!(v[1].position, [2.0, 2.25, 2.5]);
    assert_eq!(v[1].uv, [3.5, 3.75]);
    let topo = rigid.topology.as_ref().unwrap();
    assert_eq!(topo.indices, Indices::U16(vec![0, 1, 2]));
    assert_eq!(topo.groups.len(), 1);
    assert_eq!(topo.groups[0].tri_count, 1);
    assert_eq!(rigid.material_bindings.len(), 2);
    let mat = rigid.material_bindings[0].as_ref().unwrap();
    assert_eq!(
        (mat.name.as_slice(), mat.map_count, mat.maps.len()),
        (&b"mat"[..], 2, 2)
    );
    assert_eq!(mat.maps[0].usage, b"Diffuse Color");
    let tex = mat.maps[0].material.as_ref().unwrap().texture.as_ref().unwrap();
    assert_eq!(tex.from_file_name, b"D:\\ymir work\\a.dds");
    assert!(mat.maps[1].material.as_ref().unwrap().texture.is_none());
    assert!(rigid.material_bindings[1].is_none());
    assert_eq!(rigid.bone_bindings[0].obb_max, [1.0, 2.0, 3.5]);

    let skinned = m.mesh_bindings[1].as_deref().unwrap();
    assert!(!skinned.is_rigid());
    let Some(Vertices::Skinned(v)) = &skinned.vertex_data.as_ref().unwrap().vertices else {
        panic!()
    };
    assert_eq!(v.len(), 2);
    assert_eq!(v[1].position, [3.0, 4.0, 5.0]);
    assert_eq!((v[1].bone_weights, v[1].bone_indices), ([200, 55, 0, 0], [0, 1, 0, 0]));
    assert_eq!(v[1].uv, [-2.0, -2.5]);
    assert_eq!(skinned.topology.as_ref().unwrap().indices, Indices::I32(vec![2, 1, 0]));

    let a = &f.animations[0];
    assert_eq!((a.name.as_slice(), a.duration), (&b"walk"[..], 1.5));
    assert_eq!(a.track_groups.len(), 1);
    let g = &a.track_groups[0];
    assert_eq!(g.loop_translation, [0.5, 0.0, -0.5]);
    assert_eq!(g.transform_tracks.len(), 2);
    let t = &g.transform_tracks[0];
    assert_eq!((t.position.degree, t.position.knot_count), (2, 3));
    assert_eq!(t.position.knots, [0.0, 0.5, 1.0]);
    assert_eq!(t.position.controls.len(), 9);
    assert_eq!(t.orientation.controls, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(g.transform_tracks[1].name, b"Bip01 Pelvis");
}

#[test]
fn edge_cases() {
    let f = gr2::parse(&fixture::edge_cases()).unwrap();
    // Unresolved and out-of-section models are skipped.
    assert_eq!((f.model_count, f.models.len()), (2, 2));

    let m0 = &f.models[0];
    let sk = m0.skeleton.as_ref().unwrap();
    assert_eq!((sk.bone_count, sk.bones.len()), (5, 0));
    assert_eq!(m0.mesh_bindings.len(), 4);
    assert!(m0.mesh_bindings[1].is_none() && m0.mesh_bindings[2].is_none());
    let mesh = m0.mesh_bindings[0].as_deref().unwrap();
    let topo = mesh.topology.as_ref().unwrap();
    assert_eq!(topo.indices, Indices::None);
    assert!(topo.groups.is_empty());
    let vd = mesh.vertex_data.as_ref().unwrap();
    assert_eq!((vd.vertex_count, vd.vertices.is_none()), (10, true));
    assert_eq!((mesh.material_binding_count, mesh.material_bindings.len()), (0, 0));
    assert_eq!((mesh.bone_binding_count, mesh.bone_bindings.len()), (4, 0));
    assert!(!mesh.is_rigid());

    let m1 = &f.models[1];
    assert_eq!(m1.name, b"");
    let sk = m1.skeleton.as_ref().unwrap();
    assert_eq!(sk.bones.len(), 2);
    assert_eq!(sk.bones[0].name, b"first");
    assert_eq!(sk.bones[1], gr2::Bone::default());
    let mesh = m1.mesh_bindings[0].as_deref().unwrap();
    let mat = mesh.material_bindings[0].as_ref().unwrap();
    assert_eq!((mat.map_count, mat.maps.len()), (2, 2));
    assert!(mat.maps[0].usage.is_empty() && mat.maps[0].material.is_none());
    assert!(mesh.material_bindings[1].is_none());

    assert_eq!(f.animations.len(), 1);
    let groups = &f.animations[0].track_groups;
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].loop_translation, [0.0; 3]);
    assert_eq!(groups[0].transform_tracks.len(), 3);
    let c = &groups[0].transform_tracks[0];
    assert_eq!(
        (c.position.degree, c.position.knot_count, c.position.control_count),
        (3, 0, 0)
    );
    assert_eq!((c.orientation.knot_count, c.orientation.knots.len()), (2, 0));
    assert_eq!(c.scale_shear.controls, [0.0]);
    assert_eq!(groups[0].transform_tracks[1], gr2::TransformTrack::default());
    assert!(groups[1].transform_tracks.is_empty());
}

#[test]
fn null_models_keeps_the_count() {
    let f = gr2::parse(&fixture::null_models()).unwrap();
    assert_eq!((f.model_count, f.models.len()), (3, 0));
    // Above 2000 reads as 0.
    assert_eq!((f.animation_count, f.animations.len()), (2000, 0));
}

#[test]
fn errors() {
    assert_eq!(
        gr2::parse(&fixture::too_many_bones()),
        Err(Error::TooManyRecords(200_001))
    );
    assert_eq!(gr2::parse(&[]), Err(Error::TooShort));
    let full = fixture::full();
    assert_eq!(gr2::parse(&full[..87]), Err(Error::TooShort));
    let mut bad = full.clone();
    bad[0] = 0;
    assert_eq!(gr2::parse(&bad), Err(Error::BadMagic));
    let mut bad = full.clone();
    bad[48..52].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(gr2::parse(&bad), Err(Error::SectionCount(0)));
    let mut bad = full.clone();
    bad[44..48].copy_from_slice(&0xffff_fff0u32.to_le_bytes());
    assert_eq!(gr2::parse(&bad), Err(Error::SectionTable));
    // Section 0's format.
    let mut bad = full.clone();
    bad[68..72].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(gr2::parse(&bad), Err(Error::Section(0)));
    // Section 0 compressed with garbage.
    let mut bad = full.clone();
    bad[68..72].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(gr2::parse(&bad), Err(Error::Decompress(0)));
    // Root node past its section.
    let mut bad = full;
    bad[64..68].copy_from_slice(&0xffff_fff8u32.to_le_bytes());
    assert_eq!(gr2::parse(&bad), Err(Error::RootNode));
}

#[test]
fn huge_sections_are_refused() {
    // 4096 sections of 256 MiB, zero-filled, from a ~180 KB file.
    let mut b = fixture::Builder::new(1);
    b.root();
    let mut file = b.build();
    file[48..52].copy_from_slice(&4096u32.to_le_bytes());
    let entry: [u32; 11] = [0, 0, 0, 256 << 20, 4, 0, 0, 0, 0, 0, 0];
    file.truncate(68);
    for _ in 0..4096 {
        for v in entry {
            file.extend_from_slice(&v.to_le_bytes());
        }
    }
    assert_eq!(gr2::parse(&file), Err(Error::TooLarge));
}

#[test]
fn vertices_stop_at_the_section_end() {
    // Deviation: C++ hands out the pointer and the engine reads 1000 records.
    let mut b = fixture::Builder::new(2);
    let node = b.root();
    let model = b.model(b"m");
    let mesh = b.mesh(b"mesh");
    b.vertices(mesh, 1000, 1, &[0; 70]);
    b.mesh_bindings(model, 1, &[Some(mesh)]);
    b.set_models(node, 1, &[Some(model)]);
    let f = gr2::parse(&b.build()).unwrap();
    let vd = f.models[0].mesh_bindings[0]
        .as_ref()
        .unwrap()
        .vertex_data
        .as_ref()
        .unwrap();
    assert_eq!(vd.vertex_count, 1000);
    assert_eq!(vd.vertices.as_ref().unwrap().len(), 2);
}

#[test]
fn every_animation_keeps_its_track_groups() {
    // C++ would leave the first animation's TrackGroups dangling.
    let mut b = fixture::Builder::new(1);
    let node = b.root();
    let mut anims = Vec::new();
    for name in [&b"a"[..], b"b", b"c"] {
        let a = b.animation(name, 1.0);
        let g = b.track_group(name, 116);
        b.track_groups(a, 1, &[Some(g)]);
        anims.push(Some(a));
    }
    b.set_animations(node, 3, &anims);
    let f = gr2::parse(&b.build()).unwrap();
    let names: Vec<_> = f.animations.iter().map(|a| a.track_groups[0].name.clone()).collect();
    assert_eq!(names, [b"a".to_vec(), b"b".to_vec(), b"c".to_vec()]);
}

#[test]
fn shared_records_hit_the_budget_instead_of_exploding() {
    // 2000 models, each with 200000 bindings to one mesh with 200000
    // materials: C++ would effectively never finish.
    let mut b = fixture::Builder::new(1);
    let node = b.root();
    let mesh = b.mesh(b"m");
    let mat = b.material(b"mat", &[]);
    b.put_u32(mesh, 20, 200_000);
    let arr = b.zeros(0, 4);
    b.ptr(arr, 0, mat);
    b.ptr(mesh, 24, arr);
    let model = b.model(b"m");
    b.put_u32(model, 76, 200_000);
    let marr = b.zeros(0, 4);
    b.ptr(marr, 0, mesh);
    b.ptr(model, 80, marr);
    b.set_models(node, 2000, &vec![Some(model); 2000]);
    assert_eq!(gr2::parse(&b.build()), Err(Error::TooLarge));
}

#[test]
fn truncations_and_garbage_never_panic() {
    let full = fixture::full();
    for cut in 0..full.len() {
        let _ = gr2::parse(&full[..cut]);
    }
    // Flip every byte of the header, table and relocations in turn, and
    // overwrite words with extreme values.
    for i in 0..full.len() {
        let mut f = full.clone();
        f[i] ^= 0xff;
        let _ = gr2::parse(&f);
    }
    for i in (0..full.len() - 3).step_by(4) {
        for v in [0u32, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, 0xffff_fff0] {
            let mut f = full.clone();
            f[i..i + 4].copy_from_slice(&v.to_le_bytes());
            let _ = gr2::parse(&f);
        }
    }
    let mut seed = 7u32;
    for len in [88usize, 200, 1000, 5000] {
        for _ in 0..200 {
            let mut f: Vec<u8> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (seed >> 24) as u8
                })
                .collect();
            f[..16].copy_from_slice(&gr2::MAGIC);
            f[48] = (f[48] % 8) + 1;
            f[49..52].fill(0);
            let _ = gr2::parse(&f);
        }
    }
}
