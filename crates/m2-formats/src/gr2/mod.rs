//! Granny `.gr2` files: port of `GrannyParseGR2FromMemory`
//! (`neoeter/src/model/grn/Gr2Reader.cpp`) and its Oodle1 decompressor.
//!
//! The C++ reader is not a general Granny loader: it follows fixed record
//! offsets (Granny 2 files exported for Metin2) and pulls out the fields the
//! engine uses through `GrannyCompat.h` — models with their skeleton and
//! meshes (vertices, indices, triangle groups, materials, bone bindings) and
//! animations (track groups, transform tracks, curves). [`parse`] returns the
//! same data as plain structs.
//!
//! Where the C++ structs pair a count with a pointer that can be null while
//! the count is not, both are kept here: the count as C++ reports it, and a
//! `Vec` that is empty for the null pointer.
//!
//! Deviations from the C++ code, all on malformed input only:
//! - Bounds checks like `off + 84 > size` are computed without 32-bit
//!   wraparound; where C++ wraps, passes the check and reads out of bounds,
//!   the record is skipped here.
//! - A relocation into a section index past the section table is treated as
//!   missing (C++ indexes its fixup vector out of bounds).
//! - The root type's member names are read up to the end of their section
//!   (C++ reads on until a NUL).
//! - Vertices are decoded only as far as their section reaches (C++ hands
//!   out a raw pointer and the engine reads `VertexCount` records from it).
//! - Total section size and the memory spent on records are capped
//!   ([`MAX_TOTAL_SECTION_BYTES`], [`Error::TooLarge`]), so a small hostile
//!   file cannot make the parser allocate or loop without bound.
//! - Every animation owns its track groups. In C++ they point into one shared
//!   vector that later animations reallocate (a dangling pointer for files
//!   with more than one animation; the client's files have at most one).
//! - Decompression fails cleanly where the C code would divide by zero.

mod decompress;

use std::collections::HashMap;
use std::fmt;

pub use decompress::{DecompressError, decompress};

/// The 16-byte magic every file starts with (little-endian, 32-bit Granny).
pub const MAGIC: [u8; 16] = [
    0xb8, 0x67, 0xb0, 0xca, 0xf8, 0x6d, 0xb1, 0x0f, 0x84, 0x72, 0x8c, 0x7e, 0x5e, 0x19, 0x00, 0x1e,
];

/// `kMaxSections`.
const MAX_SECTIONS: u32 = 4096;
/// `kMaxSectionBytes`.
const MAX_SECTION_BYTES: u32 = 256 << 20;
/// `kMaxRecords`: larger counts fail the whole file.
const MAX_RECORDS: i32 = 200_000;
/// Model and animation counts above this are read as 0.
const MAX_ROOT_COUNT: i32 = 2000;
/// Not in C++: the sum of all section sizes.
pub const MAX_TOTAL_SECTION_BYTES: u64 = 256 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Shorter than the 88-byte header.
    TooShort,
    BadMagic,
    /// Section count 0 or above 4096.
    SectionCount(u32),
    /// The section table does not fit in the file.
    SectionTable,
    /// Section `n` is too large, of an unknown format, or its data or
    /// relocations do not fit in the file.
    Section(u32),
    /// Section `n` does not decompress.
    Decompress(u32),
    /// The root node is outside its section.
    RootNode,
    /// A record count above 200000.
    TooManyRecords(i32),
    /// Not in C++: the file needs more memory than its size justifies.
    TooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooShort => write!(f, "shorter than the gr2 header"),
            Error::BadMagic => write!(f, "not a gr2 file"),
            Error::SectionCount(n) => write!(f, "bad section count {n}"),
            Error::SectionTable => write!(f, "section table outside the file"),
            Error::Section(n) => write!(f, "section {n} is malformed"),
            Error::Decompress(n) => write!(f, "section {n} does not decompress"),
            Error::RootNode => write!(f, "root node outside its section"),
            Error::TooManyRecords(n) => write!(f, "record count {n} too large"),
            Error::TooLarge => write!(f, "file too large to load"),
        }
    }
}

impl std::error::Error for Error {}

/// `granny_transform`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Transform {
    /// `GrannyHasPosition | GrannyHasOrientation | GrannyHasScaleShear`.
    pub flags: u32,
    pub position: [f32; 3],
    /// Quaternion x, y, z, w.
    pub orientation: [f32; 4],
    pub scale_shear: [[f32; 3]; 3],
}

impl Transform {
    /// Size in the file (and of the C struct).
    pub const SIZE: usize = 68;

    fn read(b: &[u8]) -> Self {
        let f = |i: usize| f32_at(b, 4 + i * 4);
        Transform {
            flags: u32_at(b, 0),
            position: [f(0), f(1), f(2)],
            orientation: [f(3), f(4), f(5), f(6)],
            scale_shear: [[f(7), f(8), f(9)], [f(10), f(11), f(12)], [f(13), f(14), f(15)]],
        }
    }
}

/// A parsed file: `granny_file_info`'s models and animations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct File {
    /// `ModelCount`: `models.len()`, except when the models array has no
    /// relocation, where C++ keeps the stored count with a null `Models`.
    pub model_count: i32,
    pub models: Vec<Model>,
    /// `AnimationCount`, like `model_count`.
    pub animation_count: i32,
    pub animations: Vec<Animation>,
}

/// `granny_model`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Model {
    pub name: Vec<u8>,
    pub skeleton: Option<Skeleton>,
    pub initial_placement: Transform,
    /// `MeshBindingCount` as stored (negative counts read as 0).
    pub mesh_binding_count: i32,
    /// `MeshBindings`: empty when null, else `mesh_binding_count` entries,
    /// `None` where the mesh could not be resolved.
    pub mesh_bindings: Vec<Option<Box<Mesh>>>,
}

/// `granny_skeleton`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Skeleton {
    pub name: Vec<u8>,
    pub bone_count: i32,
    /// Empty when C++ leaves `Bones` null; bones outside their section are
    /// all-zero.
    pub bones: Vec<Bone>,
}

/// `granny_bone`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bone {
    pub name: Vec<u8>,
    /// -1 for a root bone.
    pub parent_index: i32,
    pub local_transform: Transform,
    pub inverse_world: [[f32; 4]; 4],
}

/// `granny_mesh`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub name: Vec<u8>,
    pub vertex_data: Option<VertexData>,
    pub topology: Option<TriTopology>,
    pub material_binding_count: i32,
    /// Empty when null; `None` where the material could not be resolved.
    pub material_bindings: Vec<Option<Material>>,
    pub bone_binding_count: i32,
    /// Empty when null; bindings outside their section are all-zero.
    pub bone_bindings: Vec<BoneBinding>,
}

impl Mesh {
    /// `GrannyMeshIsRigid`: at most one bone binding. Decides the vertex
    /// layout the reader assumes.
    pub fn is_rigid(&self) -> bool {
        self.bone_binding_count <= 1
    }
}

/// `granny_vertex_data`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VertexData {
    /// `VertexCount` as stored (not validated by C++).
    pub vertex_count: i32,
    /// `None` when `Vertices` is null. The layout is not read from the file:
    /// C++ assumes PNT332 for rigid meshes and PWNT3432 otherwise.
    pub vertices: Option<Vertices>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Vertices {
    /// `GrannyPNT332VertexType`.
    Rigid(Vec<RigidVertex>),
    /// `GrannyPWNT3432VertexType`.
    Skinned(Vec<SkinnedVertex>),
}

impl Vertices {
    pub fn len(&self) -> usize {
        match self {
            Vertices::Rigid(v) => v.len(),
            Vertices::Skinned(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `granny_pnt332_vertex`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RigidVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl RigidVertex {
    pub const SIZE: usize = 32;

    fn read(b: &[u8]) -> Self {
        let f = |i: usize| f32_at(b, i * 4);
        RigidVertex {
            position: [f(0), f(1), f(2)],
            normal: [f(3), f(4), f(5)],
            uv: [f(6), f(7)],
        }
    }
}

/// `granny_pwnt3432_vertex`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub bone_weights: [u8; 4],
    pub bone_indices: [u8; 4],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl SkinnedVertex {
    pub const SIZE: usize = 40;

    fn read(b: &[u8]) -> Self {
        let f = |at: usize| f32_at(b, at);
        SkinnedVertex {
            position: [f(0), f(4), f(8)],
            bone_weights: [b[12], b[13], b[14], b[15]],
            bone_indices: [b[16], b[17], b[18], b[19]],
            normal: [f(20), f(24), f(28)],
            uv: [f(32), f(36)],
        }
    }
}

/// `granny_tri_topology`, as far as the reader fills it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TriTopology {
    /// `Groups` (`GroupCount` is their number; 0 when not loaded).
    pub groups: Vec<TriMaterialGroup>,
    /// `Indices16` or `Indices` (`IndexCount` is their number).
    pub indices: Indices,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Indices {
    #[default]
    None,
    U16(Vec<u16>),
    I32(Vec<i32>),
}

impl Indices {
    pub fn len(&self) -> usize {
        match self {
            Indices::None => 0,
            Indices::U16(v) => v.len(),
            Indices::I32(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// `granny_tri_material_group`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TriMaterialGroup {
    pub material_index: i32,
    pub tri_first: i32,
    pub tri_count: i32,
}

/// `granny_material`. Mesh materials have a name and maps; the materials the
/// maps point to have only a texture (that is all C++ reads of them).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Material {
    pub name: Vec<u8>,
    pub map_count: i32,
    /// Empty when `Maps` is null.
    pub maps: Vec<MaterialMap>,
    pub texture: Option<Texture>,
}

/// `granny_material_map`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialMap {
    /// E.g. `Diffuse Color` (see `GrannyGetMaterialTextureByType`).
    pub usage: Vec<u8>,
    pub material: Option<Box<Material>>,
}

/// `granny_texture`; only the file name is read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Texture {
    pub from_file_name: Vec<u8>,
}

/// `granny_bone_binding`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoneBinding {
    pub bone_name: Vec<u8>,
    pub obb_min: [f32; 3],
    pub obb_max: [f32; 3],
}

/// `granny_animation`. (C++ also sets `Oversampling`, always to 0.)
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Animation {
    pub name: Vec<u8>,
    pub duration: f32,
    pub time_step: f32,
    /// The groups that resolved (`TrackGroupCount` is their number).
    pub track_groups: Vec<TrackGroup>,
}

/// `granny_track_group`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackGroup {
    pub name: Vec<u8>,
    pub loop_translation: [f32; 3],
    /// `TransformTracks` (`TransformTrackCount` is their number). Tracks from
    /// the first one outside its section on are all-zero, as in C++.
    pub transform_tracks: Vec<TransformTrack>,
}

/// `granny_transform_track`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TransformTrack {
    pub name: Vec<u8>,
    pub position: Curve,
    pub orientation: Curve,
    pub scale_shear: Curve,
}

/// `granny_curve` (the old Granny 2 B-spline layout).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Curve {
    pub degree: i32,
    /// `KnotCount` (0 for negative or too-large stored counts).
    pub knot_count: i32,
    /// Empty when `Knots` is null.
    pub knots: Vec<f32>,
    pub control_count: i32,
    pub controls: Vec<f32>,
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn i32_at(b: &[u8], at: usize) -> i32 {
    u32_at(b, at) as i32
}

fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_bits(u32_at(b, at))
}

/// Whether `len` bytes at `off` fit in `section` (without the u32
/// wraparound of the C++ checks).
fn fits(section: &[u8], off: u32, len: u64) -> bool {
    u64::from(off) + len <= section.len() as u64
}

/// Section contents with their relocations ("fixups"): what a pointer field
/// at a given section and offset points to.
struct Sections {
    data: Vec<Vec<u8>>,
    fixups: Vec<HashMap<u32, (u32, u32)>>,
    /// Bytes left to spend on records (see [`Error::TooLarge`]).
    budget: u64,
}

impl Sections {
    /// The relocation of the pointer at `sec:off`.
    fn fixup(&self, sec: u32, off: u32) -> Option<(u32, u32)> {
        self.fixups.get(sec as usize)?.get(&off).copied()
    }

    /// A relocation target and its section, if `len` bytes fit there.
    fn record(&self, sec: u32, off: u32, len: u64) -> Option<&[u8]> {
        let data = self.data.get(sec as usize)?;
        fits(data, off, len).then(|| &data[off as usize..])
    }

    /// `ResolveString`: the NUL-terminated string the pointer at `sec:off`
    /// points to, or empty (no relocation, target outside its section, or no
    /// NUL before the section ends).
    fn string(&mut self, sec: u32, off: u32) -> Result<Vec<u8>, Error> {
        let Some((to_sec, to_off)) = self.fixup(sec, off) else {
            return Ok(Vec::new());
        };
        let Some(data) = self.data.get(to_sec as usize) else {
            return Ok(Vec::new());
        };
        let Some(tail) = data.get(to_off as usize..).filter(|t| !t.is_empty()) else {
            return Ok(Vec::new());
        };
        let Some(len) = tail.iter().position(|&b| b == 0) else {
            return Ok(Vec::new());
        };
        let s = tail[..len].to_vec();
        self.spend(s.len() as u64)?;
        Ok(s)
    }

    /// Charges `bytes` of memory or work against the budget.
    fn spend(&mut self, bytes: u64) -> Result<(), Error> {
        self.budget = self.budget.checked_sub(bytes).ok_or(Error::TooLarge)?;
        Ok(())
    }

    /// Charges `count` records of `T`, plus one unit per record for the
    /// loop that visits them.
    fn spend_records<T>(&mut self, count: i32) -> Result<(), Error> {
        self.spend(count.max(0) as u64 * (size_of::<T>() as u64 + 1))
    }
}

/// Fails the file for counts above `kMaxRecords`.
fn check_count(count: i32) -> Result<(), Error> {
    if count > MAX_RECORDS {
        Err(Error::TooManyRecords(count))
    } else {
        Ok(())
    }
}

/// `GrannyParseGR2FromMemory`.
pub fn parse(buf: &[u8]) -> Result<File, Error> {
    // `size` is an int in C++; anything at or above 2 GiB is unrepresentable.
    if buf.len() < 88 {
        return Err(Error::TooShort);
    }
    if buf.len() > i32::MAX as usize {
        return Err(Error::TooLarge);
    }
    if buf[..16] != MAGIC {
        return Err(Error::BadMagic);
    }
    let size = buf.len() as u64;
    let header = |at: usize| u32_at(buf, 32 + at);
    let (sec_offset, num_sec) = (header(12), header(16));
    let (root_type_sec, root_type_off) = (header(20), header(24));
    let (root_node_sec, root_node_off) = (header(28), header(32));

    if num_sec == 0 || num_sec > MAX_SECTIONS {
        return Err(Error::SectionCount(num_sec));
    }
    let table = 32 + u64::from(sec_offset);
    if u64::from(sec_offset) > size || table > size || u64::from(num_sec) * 44 > size - table {
        return Err(Error::SectionTable);
    }

    let mut data = Vec::with_capacity(num_sec as usize);
    let mut fixups = Vec::with_capacity(num_sec as usize);
    let mut total = 0u64;
    for i in 0..num_sec {
        let entry = &buf[table as usize + i as usize * 44..][..44];
        let field = |at: usize| u32_at(entry, at);
        let (fmt, off, comp_size, uncomp_size) = (field(0), field(4), field(8), field(12));
        let (step1, step2, reloc_off, num_reloc) = (field(20), field(24), field(28), field(32));

        if uncomp_size > MAX_SECTION_BYTES || (fmt != 0 && fmt != 2) {
            return Err(Error::Section(i));
        }
        if comp_size > 0 && u64::from(off) + u64::from(comp_size) > size {
            return Err(Error::Section(i));
        }
        total += u64::from(uncomp_size);
        if total > MAX_TOTAL_SECTION_BYTES {
            return Err(Error::TooLarge);
        }

        let mut section = vec![0u8; uncomp_size as usize];
        if comp_size > 0 && uncomp_size > 0 {
            let src = &buf[off as usize..][..comp_size as usize];
            if fmt == 0 {
                let n = comp_size.min(uncomp_size) as usize;
                section[..n].copy_from_slice(&src[..n]);
            } else {
                decompress(src, step1, step2, &mut section).map_err(|_| Error::Decompress(i))?;
            }
        }
        data.push(section);

        let mut map = HashMap::new();
        if num_reloc > 0 {
            if u64::from(reloc_off) > size || u64::from(num_reloc) > (size - u64::from(reloc_off)) / 12 {
                return Err(Error::Section(i));
            }
            map.reserve(num_reloc as usize);
            for r in buf[reloc_off as usize..][..num_reloc as usize * 12]
                .as_chunks::<12>()
                .0
                .iter()
            {
                // Later relocations for the same offset win, as in the C++ map.
                map.insert(u32_at(r, 0), (u32_at(r, 4), u32_at(r, 8)));
            }
        }
        fixups.push(map);
    }

    let mut s = Sections {
        data,
        fixups,
        budget: 16 * total + (16 << 20),
    };
    if s.record(root_node_sec, root_node_off, 16).is_none() {
        return Err(Error::RootNode);
    }

    let (models_off, anims_off) = root_members(&s, root_type_sec, root_type_off, root_node_off);
    let root = &s.data[root_node_sec as usize];
    let root_count = |member: Option<u32>| match member {
        Some(off) if fits(root, off, 8) => {
            let n = i32_at(root, off as usize);
            if (0..=MAX_ROOT_COUNT).contains(&n) { n } else { 0 }
        }
        _ => 0,
    };
    let model_count = root_count(models_off);
    let anim_count = root_count(anims_off);

    let mut file = File {
        model_count,
        animation_count: anim_count,
        ..File::default()
    };

    if let Some(models_off) = models_off.filter(|_| model_count > 0)
        && let Some((arr_sec, arr_off)) = s.fixup(root_node_sec, models_off.wrapping_add(4))
    {
        s.spend(model_count as u64)?;
        for m in 0..model_count as u32 {
            let Some((sec, off)) = s.fixup(arr_sec, arr_off.wrapping_add(m * 4)) else {
                continue;
            };
            if let Some(model) = read_model(&mut s, sec, off)? {
                file.models.push(model);
            }
        }
        file.model_count = file.models.len() as i32;
    }

    if let Some(anims_off) = anims_off.filter(|_| anim_count > 0)
        && let Some((arr_sec, arr_off)) = s.fixup(root_node_sec, anims_off.wrapping_add(4))
    {
        s.spend(anim_count as u64)?;
        for a in 0..anim_count as u32 {
            let Some((sec, off)) = s.fixup(arr_sec, arr_off.wrapping_add(a * 4)) else {
                continue;
            };
            if let Some(anim) = read_animation(&mut s, sec, off)? {
                file.animations.push(anim);
            }
        }
        file.animation_count = file.animations.len() as i32;
    }

    Ok(file)
}

/// Walks the root type definition (32-byte member records) to find where
/// the `Models` and `Animations` members sit in the root node. The last
/// member of each name wins.
fn root_members(s: &Sections, type_sec: u32, type_off: u32, node_off: u32) -> (Option<u32>, Option<u32>) {
    let (mut models, mut anims) = (None, None);
    let Some(types) = s.data.get(type_sec as usize) else {
        return (models, anims);
    };
    let (mut type_off, mut data_off) = (type_off, node_off);
    while fits(types, type_off, 32) {
        let member_type = u32_at(types, type_off as usize);
        if member_type == 0 {
            break; // GrannyEndMember
        }
        let name = s
            .fixup(type_sec, type_off.wrapping_add(4))
            .and_then(|(sec, off)| s.data.get(sec as usize)?.get(off as usize..).filter(|t| !t.is_empty()))
            .map_or(&[][..], |tail| {
                &tail[..tail.iter().position(|&b| b == 0).unwrap_or(tail.len())]
            });

        let member_size = match member_type {
            1 => 0,
            3..=5 => 8,
            7 => 12,
            9 => 68,
            11 | 12 => 1,
            13 | 14 => 2,
            _ => 4,
        };
        match name {
            b"Models" => models = Some(data_off),
            b"Animations" => anims = Some(data_off),
            _ => {}
        }
        data_off = data_off.wrapping_add(member_size);
        type_off += 32;
    }
    (models, anims)
}

/// One model record (84 bytes); `None` when it is outside its section.
fn read_model(s: &mut Sections, sec: u32, off: u32) -> Result<Option<Model>, Error> {
    let Some(rec) = s.record(sec, off, 84) else {
        return Ok(None);
    };
    let initial_placement = Transform::read(&rec[8..]);
    let mb_count = i32_at(rec, 76);
    s.spend(size_of::<Model>() as u64)?;
    let mut model = Model {
        initial_placement,
        ..Model::default()
    };
    model.name = s.string(sec, off)?;

    if let Some((sk_sec, sk_off)) = s.fixup(sec, off.wrapping_add(4)) {
        model.skeleton = read_skeleton(s, sk_sec, sk_off)?;
    }

    check_count(mb_count)?;
    model.mesh_binding_count = mb_count.max(0);
    if mb_count > 0
        && let Some((arr_sec, arr_off)) = s.fixup(sec, off.wrapping_add(80))
    {
        s.spend_records::<Option<Box<Mesh>>>(mb_count)?;
        model.mesh_bindings = vec![None; mb_count as usize];
        for b in 0..mb_count as u32 {
            if let Some((mesh_sec, mesh_off)) = s.fixup(arr_sec, arr_off.wrapping_add(b * 4)) {
                model.mesh_bindings[b as usize] = read_mesh(s, mesh_sec, mesh_off)?.map(Box::new);
            }
        }
    }
    Ok(Some(model))
}

/// A skeleton (12 bytes) and its bones (156 bytes each).
fn read_skeleton(s: &mut Sections, sec: u32, off: u32) -> Result<Option<Skeleton>, Error> {
    let Some(rec) = s.record(sec, off, 12) else {
        return Ok(None);
    };
    let bone_count = i32_at(rec, 4);
    let mut skel = Skeleton {
        name: s.string(sec, off)?,
        ..Skeleton::default()
    };
    check_count(bone_count)?;
    skel.bone_count = bone_count.max(0);

    if bone_count > 0
        && let Some((bn_sec, bn_off)) = s.fixup(sec, off.wrapping_add(8))
    {
        s.spend_records::<Bone>(bone_count)?;
        skel.bones = vec![Bone::default(); bone_count as usize];
        for (b, bone) in (0u32..).zip(skel.bones.iter_mut()) {
            let b_off = bn_off.wrapping_add(b * 156);
            let Some(rec) = s.record(bn_sec, b_off, 140) else {
                continue;
            };
            bone.parent_index = i32_at(rec, 4);
            bone.local_transform = Transform::read(&rec[8..]);
            for (r, row) in bone.inverse_world.iter_mut().enumerate() {
                for (c, v) in row.iter_mut().enumerate() {
                    *v = f32_at(rec, 76 + (r * 4 + c) * 4);
                }
            }
            bone.name = s.string(bn_sec, b_off)?;
        }
    }
    Ok(Some(skel))
}

/// A mesh record (36 bytes) with its vertex data, topology, materials and
/// bone bindings.
fn read_mesh(s: &mut Sections, sec: u32, off: u32) -> Result<Option<Mesh>, Error> {
    let Some(rec) = s.record(sec, off, 36) else {
        return Ok(None);
    };
    let (mat_count, bb_count) = (i32_at(rec, 20), i32_at(rec, 28));
    s.spend(size_of::<Mesh>() as u64)?;
    let mut mesh = Mesh {
        name: s.string(sec, off)?,
        ..Mesh::default()
    };

    // Vertex data: the layout is decided below, from the bone bindings.
    let mut vertices_at = None;
    if let Some((vtx_sec, vtx_off)) = s.fixup(sec, off.wrapping_add(4))
        && let Some(vrec) = s.record(vtx_sec, vtx_off, 12)
    {
        let vertex_count = i32_at(vrec, 4);
        vertices_at = s
            .fixup(vtx_sec, vtx_off.wrapping_add(8))
            .filter(|&(d_sec, d_off)| s.data.get(d_sec as usize).is_some_and(|d| (d_off as usize) < d.len()));
        mesh.vertex_data = Some(VertexData {
            vertex_count,
            vertices: None,
        });
    }

    if let Some((topo_sec, topo_off)) = s.fixup(sec, off.wrapping_add(16)) {
        mesh.topology = read_topology(s, topo_sec, topo_off)?;
    }

    check_count(mat_count)?;
    mesh.material_binding_count = mat_count.max(0);
    if mat_count > 0
        && let Some((arr_sec, arr_off)) = s.fixup(sec, off.wrapping_add(24))
    {
        s.spend_records::<Option<Material>>(mat_count)?;
        mesh.material_bindings = vec![None; mat_count as usize];
        for m in 0..mat_count as u32 {
            if let Some((mat_sec, mat_off)) = s.fixup(arr_sec, arr_off.wrapping_add(m * 4)) {
                mesh.material_bindings[m as usize] = read_material(s, mat_sec, mat_off)?;
            }
        }
    }

    check_count(bb_count)?;
    mesh.bone_binding_count = bb_count.max(0);
    if bb_count > 0
        && let Some((arr_sec, arr_off)) = s.fixup(sec, off.wrapping_add(32))
    {
        s.spend_records::<BoneBinding>(bb_count)?;
        mesh.bone_bindings = vec![BoneBinding::default(); bb_count as usize];
        for (b, binding) in (0u32..).zip(mesh.bone_bindings.iter_mut()) {
            let b_off = arr_off.wrapping_add(b * 36);
            let Some(rec) = s.record(arr_sec, b_off, 28) else {
                continue;
            };
            binding.obb_min = [f32_at(rec, 4), f32_at(rec, 8), f32_at(rec, 12)];
            binding.obb_max = [f32_at(rec, 16), f32_at(rec, 20), f32_at(rec, 24)];
            binding.bone_name = s.string(arr_sec, b_off)?;
        }
    }

    if let (Some(vd), Some((d_sec, d_off))) = (&mut mesh.vertex_data, vertices_at) {
        let rigid = mesh.bone_binding_count <= 1; // Mesh::is_rigid
        let stride = if rigid { RigidVertex::SIZE } else { SkinnedVertex::SIZE };
        let available = s.data[d_sec as usize].len() - d_off as usize;
        let n = (vd.vertex_count.max(0) as usize).min(available / stride);
        s.spend((n * stride) as u64)?;
        let bytes = &s.data[d_sec as usize][d_off as usize..];
        let records = bytes.chunks_exact(stride).take(n);
        vd.vertices = Some(if rigid {
            Vertices::Rigid(records.map(RigidVertex::read).collect())
        } else {
            Vertices::Skinned(records.map(SkinnedVertex::read).collect())
        });
    }
    Ok(Some(mesh))
}

/// A topology record (24 bytes): triangle groups and 16- or 32-bit indices.
fn read_topology(s: &mut Sections, sec: u32, off: u32) -> Result<Option<TriTopology>, Error> {
    let Some(rec) = s.record(sec, off, 24) else {
        return Ok(None);
    };
    let (grp_count, idx_count, idx_count16) = (i32_at(rec, 0), i32_at(rec, 8), i32_at(rec, 16));
    check_count(grp_count)?;
    check_count(idx_count)?;
    check_count(idx_count16)?;
    let mut topo = TriTopology::default();

    // 16-bit indices take precedence; when their pointer resolves but they do
    // not fit, there are no indices at all (no fallback to 32-bit).
    if idx_count16 > 0
        && let Some((i_sec, i_off)) = s.fixup(sec, off.wrapping_add(20))
    {
        if let Some(b) = s.record(i_sec, i_off, idx_count16 as u64 * 2) {
            topo.indices = Indices::U16(
                b.as_chunks::<2>()
                    .0
                    .iter()
                    .take(idx_count16 as usize)
                    .map(|c| u16::from_le_bytes(*c))
                    .collect(),
            );
        }
    } else if idx_count > 0
        && let Some((i_sec, i_off)) = s.fixup(sec, off.wrapping_add(12))
        && let Some(b) = s.record(i_sec, i_off, idx_count as u64 * 4)
    {
        topo.indices = Indices::I32(
            b.as_chunks::<4>()
                .0
                .iter()
                .take(idx_count as usize)
                .map(|c| i32_at(c, 0))
                .collect(),
        );
    }

    if grp_count > 0
        && let Some((g_sec, g_off)) = s.fixup(sec, off.wrapping_add(4))
        && let Some(b) = s.record(g_sec, g_off, grp_count as u64 * 12)
    {
        topo.groups = b
            .as_chunks::<12>()
            .0
            .iter()
            .take(grp_count as usize)
            .map(|g| TriMaterialGroup {
                material_index: i32_at(g, 0),
                tri_first: i32_at(g, 4),
                tri_count: i32_at(g, 8),
            })
            .collect();
    }
    // Each copy is bounded by its section, but many meshes can share one.
    s.spend(topo.indices.len() as u64 * 4 + topo.groups.len() as u64 * 12)?;
    Ok(Some(topo))
}

/// A mesh material (12 bytes) and its maps (8 bytes each: usage string and
/// material pointer; of that material only the texture at +12 is read).
fn read_material(s: &mut Sections, sec: u32, off: u32) -> Result<Option<Material>, Error> {
    let Some(rec) = s.record(sec, off, 12) else {
        return Ok(None);
    };
    let map_count = i32_at(rec, 4);
    s.spend(size_of::<Material>() as u64)?;
    let mut mat = Material {
        name: s.string(sec, off)?,
        ..Material::default()
    };
    check_count(map_count)?;
    mat.map_count = map_count.max(0);

    if map_count > 0
        && let Some((map_sec, map_off)) = s.fixup(sec, off.wrapping_add(8))
    {
        s.spend_records::<MaterialMap>(map_count)?;
        mat.maps.reserve_exact(map_count as usize);
        for mp in 0..map_count as u32 {
            let cur = map_off.wrapping_add(mp * 8);
            let mut map = MaterialMap {
                usage: s.string(map_sec, cur)?,
                material: None,
            };
            if let Some((sub_sec, sub_off)) = s.fixup(map_sec, cur.wrapping_add(4)) {
                let mut sub = Material::default();
                if let Some((tex_sec, tex_off)) = s.fixup(sub_sec, sub_off.wrapping_add(12)) {
                    sub.texture = Some(Texture {
                        from_file_name: s.string(tex_sec, tex_off)?,
                    });
                }
                s.spend(size_of::<Material>() as u64)?;
                map.material = Some(Box::new(sub));
            }
            mat.maps.push(map);
        }
    }
    Ok(Some(mat))
}

/// An animation record (16 bytes) and its track groups.
fn read_animation(s: &mut Sections, sec: u32, off: u32) -> Result<Option<Animation>, Error> {
    let Some(rec) = s.record(sec, off, 16) else {
        return Ok(None);
    };
    let (duration, time_step, tg_count) = (f32_at(rec, 4), f32_at(rec, 8), i32_at(rec, 12));
    s.spend(size_of::<Animation>() as u64)?;
    let mut anim = Animation {
        name: s.string(sec, off)?,
        duration,
        time_step,
        track_groups: Vec::new(),
    };
    check_count(tg_count)?;

    if tg_count > 0
        && let Some((arr_sec, arr_off)) = s.fixup(sec, off.wrapping_add(16))
    {
        s.spend(tg_count as u64)?;
        for t in 0..tg_count as u32 {
            let Some((tg_sec, tg_off)) = s.fixup(arr_sec, arr_off.wrapping_add(t * 4)) else {
                continue;
            };
            if let Some(group) = read_track_group(s, tg_sec, tg_off)? {
                anim.track_groups.push(group);
            }
        }
    }
    Ok(Some(anim))
}

/// A track group record (at least 20 bytes; the loop translation at +104
/// when the section reaches +116) and its transform tracks (64 bytes each).
fn read_track_group(s: &mut Sections, sec: u32, off: u32) -> Result<Option<TrackGroup>, Error> {
    let Some(rec) = s.record(sec, off, 20) else {
        return Ok(None);
    };
    let tt_count = i32_at(rec, 12);
    let loop_translation = if rec.len() >= 116 {
        [f32_at(rec, 104), f32_at(rec, 108), f32_at(rec, 112)]
    } else {
        [0.0; 3]
    };
    s.spend(size_of::<TrackGroup>() as u64)?;
    let mut group = TrackGroup {
        name: s.string(sec, off)?,
        loop_translation,
        transform_tracks: Vec::new(),
    };
    check_count(tt_count)?;

    if tt_count > 0
        && let Some((tt_sec, tt_off)) = s.fixup(sec, off.wrapping_add(16))
    {
        s.spend_records::<TransformTrack>(tt_count)?;
        group.transform_tracks = vec![TransformTrack::default(); tt_count as usize];
        for t in 0..tt_count as u32 {
            let cur = tt_off.wrapping_add(t * 64);
            let Some(rec) = s.record(tt_sec, cur, 64) else {
                break;
            };
            let curves = [4u32, 24, 44].map(|c| {
                let r = &rec[c as usize..];
                (i32_at(r, 0), i32_at(r, 4), i32_at(r, 12), cur.wrapping_add(c))
            });
            let name = s.string(tt_sec, cur)?;
            let [position, orientation, scale_shear] = curves;
            group.transform_tracks[t as usize] = TransformTrack {
                name,
                position: read_curve(s, tt_sec, position)?,
                orientation: read_curve(s, tt_sec, orientation)?,
                scale_shear: read_curve(s, tt_sec, scale_shear)?,
            };
        }
    }
    Ok(Some(group))
}

/// The `parseCurve` lambda: a 20-byte curve (degree, knot count and
/// pointer, control count and pointer) at `curve_off` in section `sec`,
/// given as `(degree, knot_count, control_count, curve_off)`.
fn read_curve(
    s: &mut Sections,
    sec: u32,
    (degree, knot_count, control_count, curve_off): (i32, i32, i32, u32),
) -> Result<Curve, Error> {
    // Too-large counts read as 0 here instead of failing the file.
    let clamp = |n: i32| if n > MAX_RECORDS { 0 } else { n.max(0) };
    let (knot_count, control_count) = (clamp(knot_count), clamp(control_count));
    let mut floats = |count: i32, ptr_off: u32| -> Result<Vec<f32>, Error> {
        if count == 0 {
            return Ok(Vec::new());
        }
        let Some((f_sec, f_off)) = s.fixup(sec, ptr_off) else {
            return Ok(Vec::new());
        };
        let Some(b) = s.record(f_sec, f_off, count as u64 * 4) else {
            return Ok(Vec::new());
        };
        let v: Vec<f32> = b
            .as_chunks::<4>()
            .0
            .iter()
            .take(count as usize)
            .map(|c| f32_at(c, 0))
            .collect();
        s.spend(v.len() as u64 * 4)?;
        Ok(v)
    };
    let knots = floats(knot_count, curve_off.wrapping_add(8))?;
    let controls = floats(control_count, curve_off.wrapping_add(16))?;
    Ok(Curve {
        degree,
        knot_count,
        knots,
        control_count,
        controls,
    })
}
