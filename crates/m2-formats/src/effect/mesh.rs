//! `EffectMesh.cpp`: `CEffectMeshScript` (the `Group Mesh` element of an
//! effect script) and `CEffectMesh` (the binary `.mde` mesh it names).

use super::{Cursor, ElementBase, Error, TimeEventTableFloat, resolve, token_time_event_float};
use crate::pack::{self, PackManager};
use crate::text;

/// `D3DBLEND_ONE`.
const D3DBLEND_ONE: u8 = 2;
/// `D3DBLEND_SRCCOLOR`.
const D3DBLEND_SRCCOLOR: u8 = 3;
/// `D3DTOP_MODULATE`.
const D3DTOP_MODULATE: u8 = 4;

/// More mesh elements than this are refused (C++ would try to allocate
/// them; real scripts have a handful).
const MAX_MESH_ELEMENTS: u32 = 1 << 16;

/// `CEffectMeshScript::TMeshData`: how one mesh of the `.mde` is drawn.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MeshData {
    /// `MESH_BILLBOARD_TYPE_*` (`Type.h`).
    pub billboard_type: u8,
    pub blending_enable: i32,
    pub blending_src_type: u8,
    pub blending_dest_type: u8,
    /// Never read from the script (`TRUE`/`FALSE` only via
    /// `ReserveMeshData`, which loading does not call).
    pub texture_alpha_enable: i32,
    pub color_operation_type: u8,
    /// r, g, b, a.
    pub color_factor: [f32; 4],
    pub texture_animation_loop_enable: i32,
    pub texture_animation_frame_delay: f32,
    pub texture_animation_start_frame: u32,
    pub alpha: TimeEventTableFloat,
}

/// `CEffectMeshScript`: one `Group Mesh` of an effect.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectMeshScript {
    pub base: ElementBase,
    pub mesh_animation_loop: i32,
    pub mesh_animation_loop_count: i32,
    pub mesh_animation_frame_delay: f32,
    /// One per mesh of the `.mde` (`MeshElementCount`). Entries after a
    /// failed one keep the zeroed `TMeshData()` defaults.
    pub mesh_data: Vec<MeshData>,
    /// `m_strMeshFileName`: resolved against the script's directory.
    pub mesh_file_name: Vec<u8>,
}

impl Default for EffectMeshScript {
    /// `CEffectMeshScript::Clear`.
    fn default() -> Self {
        Self {
            base: ElementBase::default(),
            mesh_animation_loop: 0,
            mesh_animation_loop_count: 0,
            mesh_animation_frame_delay: 0.02,
            mesh_data: Vec::new(),
            mesh_file_name: Vec::new(),
        }
    }
}

impl EffectMeshScript {
    /// `CEffectElementBase::LoadScript` + `CEffectMeshScript::OnLoadScript`
    /// on a cleared element; what was read before a failure stays.
    pub(crate) fn load(&mut self, cursor: &mut Cursor, script_name: &[u8]) -> Result<(), Error> {
        if self.base.load(cursor)? {
            self.load_mesh(cursor, script_name)?;
        }
        Ok(())
    }

    fn load_mesh(&mut self, cursor: &mut Cursor, script_name: &[u8]) -> Result<bool, Error> {
        let node = cursor.node();
        let Some(file) = node.token_string("meshfilename") else {
            return Ok(false);
        };
        self.mesh_file_name = resolve(script_name, file);

        let Some(looping) = node.token_int("meshanimationloopenable") else {
            return Ok(false);
        };
        self.mesh_animation_loop = looping;
        self.mesh_animation_loop_count = node.token_int("meshanimationloopcount").unwrap_or(0);
        let Some(delay) = node.token_float("meshanimationframedelay") else {
            return Ok(false);
        };
        self.mesh_animation_frame_delay = delay;
        let Some(count) = node.token_dword("meshelementcount") else {
            return Ok(false);
        };
        if count > MAX_MESH_ELEMENTS {
            // Deviation: C++ resizes to it (std::bad_alloc for huge counts).
            return Err(Error::Throws);
        }

        self.mesh_data = vec![MeshData::default(); count as usize];
        for i in 0..self.mesh_data.len() {
            let ok = cursor.goto_child_index(i, |c| load_mesh_data(c.node(), &mut self.mesh_data[i]))?;
            if !ok {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// One `Group MeshDataNN`; `Ok(false)` where C++ returns `FALSE`.
fn load_mesh_data(node: &crate::script::Node, d: &mut MeshData) -> Result<bool, Error> {
    macro_rules! required {
        ($field:expr, $value:expr) => {
            match $value {
                Some(v) => $field = v,
                None => return Ok(false),
            }
        };
    }
    required!(d.billboard_type, node.token_byte("billboardtype"));
    required!(d.blending_enable, node.token_int("blendingenable"));
    required!(d.blending_src_type, node.token_byte("blendingsrctype"));
    required!(d.blending_dest_type, node.token_byte("blendingdesttype"));
    required!(d.texture_animation_loop_enable, node.token_int("textureanimationloopenable"));
    required!(d.texture_animation_frame_delay, node.token_float("textureanimationframedelay"));
    d.texture_animation_start_frame = node.token_dword("textureanimationstartframe").unwrap_or(0);
    d.color_operation_type = node.token_byte("coloroperationtype").unwrap_or(D3DTOP_MODULATE);
    d.color_factor = node.token_vector4("colorfactor").unwrap_or([1.0; 4]);
    d.alpha = token_time_event_float(node, b"timeeventalpha")?.unwrap_or_default();
    Ok(true)
}

/// `CEffectMeshScript::ReserveMeshData`'s defaults, for tools that build
/// mesh data without a script.
impl MeshData {
    pub fn reserved() -> Self {
        Self {
            billboard_type: 0,
            blending_enable: 1,
            blending_src_type: D3DBLEND_SRCCOLOR,
            blending_dest_type: D3DBLEND_ONE,
            texture_alpha_enable: 0,
            color_operation_type: D3DTOP_MODULATE,
            color_factor: [1.0; 4],
            texture_animation_loop_enable: 1,
            texture_animation_frame_delay: 0.02,
            texture_animation_start_frame: 0,
            alpha: Vec::new(),
        }
    }
}

/// `TPTVertex`: position and texture coordinate (with `v` negated).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PtVertex {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
}

/// `CEffectMesh::TEffectFrameData`: one animation frame of a mesh.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectMeshFrame {
    /// Only in version 2 files; 0 in version 1.
    pub changed_frame: u8,
    pub visibility: f32,
    pub vertex_count: u32,
    pub texture_vertex_count: u32,
    pub index_count: u32,
    /// The triangle list: one vertex per index.
    pub vertices: Vec<PtVertex>,
}

/// `CEffectMesh::TEffectMeshData`: one mesh (geom) of an `.mde`.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectMeshElement {
    /// `szObjectName`, all 32 bytes as stored.
    pub object_name: [u8; 32],
    /// `szDiffuseMapFileName`, all 128 bytes as stored.
    pub diffuse_map_file_name: [u8; 128],
    pub frames: Vec<EffectMeshFrame>,
    /// `pImageVector`: the requested texture names, one per animation frame
    /// of an `.ifl` diffuse map, or the diffuse map itself.
    pub textures: Vec<Vec<u8>>,
}

/// The bytes of a fixed `char[]` up to its first NUL.
fn c_str(bytes: &[u8]) -> &[u8] {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    &bytes[..end]
}

impl EffectMeshElement {
    /// `szObjectName` as a C string.
    pub fn object_name(&self) -> &[u8] {
        c_str(&self.object_name)
    }

    /// `szDiffuseMapFileName` as a C string.
    ///
    /// Deviation: without a NUL C++ `strlen` runs into the next fields; here
    /// the name stops at 128 bytes.
    pub fn diffuse_map_file_name(&self) -> &[u8] {
        c_str(&self.diffuse_map_file_name)
    }
}

/// `CEffectMesh` after `Load`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EffectMesh {
    pub geom_count: i32,
    pub frame_count: i32,
    pub meshes: Vec<EffectMeshElement>,
}

/// Why an `.mde` did not load.
#[derive(Debug)]
pub enum MeshError {
    /// The file is not registered (`OnLoad(0, nullptr)` fails).
    Pack(pack::Error),
    /// Neither `EffectData` nor `MDEData002` (`OnLoad` returns false).
    BadHeader,
    /// C++ would read past the buffer or past its vertex arrays, or
    /// allocate a negative count (undefined behaviour or a throw); refused.
    Malformed,
}

impl std::fmt::Display for MeshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pack(e) => write!(f, "{e}"),
            Self::BadHeader => f.write_str("not an effect mesh"),
            Self::Malformed => f.write_str("truncated or inconsistent effect mesh"),
        }
    }
}

impl std::error::Error for MeshError {}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], MeshError> {
        if self.buf.len() - self.pos < n {
            return Err(MeshError::Malformed);
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, MeshError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, MeshError> {
        Ok(f32::from_bits(self.u32()?))
    }

    #[allow(clippy::chunks_exact_to_as_chunks)]
    fn u32s(&mut self, n: u32) -> Result<Vec<u32>, MeshError> {
        let bytes = self.take(n as usize * 4)?;
        Ok(bytes.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect())
    }

    #[allow(clippy::chunks_exact_to_as_chunks)]
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, MeshError> {
        let bytes = self.take(n * 4)?;
        Ok(bytes.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap())).collect())
    }
}

/// One frame's arrays and the triangle list built from them.
fn read_frame(r: &mut Reader, frame: &mut EffectMeshFrame) -> Result<(), MeshError> {
    let positions = r.f32s(frame.vertex_count as usize * 3)?;
    let indices = r.u32s(frame.index_count)?;
    let tex = r.f32s(frame.texture_vertex_count as usize * 2)?;
    let tex_indices = r.u32s(frame.index_count)?;
    frame.vertices = indices
        .iter()
        .zip(&tex_indices)
        .map(|(&i, &t)| {
            // C++ only asserts these (reads out of bounds in release).
            if i >= frame.vertex_count || t >= frame.texture_vertex_count {
                return Err(MeshError::Malformed);
            }
            let (i, t) = (i as usize, t as usize);
            Ok(PtVertex {
                position: [positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]],
                // `texCoord.y *= -1`, which gcc compiles as a negation.
                tex_coord: [tex[t * 2], -tex[t * 2 + 1]],
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(())
}

/// `GetFileExtension` + `stl_lowers`: the lowercased text after the last
/// `.` of the last path component.
fn extension(name: &[u8]) -> Vec<u8> {
    for (i, &b) in name.iter().enumerate().rev() {
        match b {
            b'.' => return name[i + 1..].to_ascii_lowercase(),
            b'/' | b'\\' => break,
            _ => {}
        }
    }
    Vec::new()
}

/// The texture requests for a diffuse map: each non-empty line of an
/// `.ifl` (prefixed by the `.ifl`'s directory), or the map itself. A missing
/// `.ifl` gives none.
fn diffuse_textures(manager: &PackManager, diffuse: &[u8]) -> Vec<Vec<u8>> {
    if extension(diffuse) != b"ifl" {
        return vec![diffuse.to_vec()];
    }
    let Ok(data) = manager.get(diffuse) else {
        return Vec::new();
    };
    let dir = super::only_path_name(diffuse);
    text::lines(&data).into_iter().filter(|line| !line.is_empty()).map(|line| [dir, line].concat()).collect()
}

/// `CEffectMesh::OnLoad`: parses an `.mde`. `manager` reads `.ifl` texture
/// lists.
pub fn parse_effect_mesh(manager: &PackManager, data: &[u8]) -> Result<EffectMesh, MeshError> {
    let mut r = Reader { buf: data, pos: 0 };
    let header = r.take(11)?;
    let v2 = match header {
        b"EffectData\0" => false,
        b"MDEData002\0" => true,
        _ => return Err(MeshError::BadHeader),
    };
    let geom_count = r.u32()? as i32;
    let frame_count = r.u32()? as i32;
    // resize() of a negative int throws; the `short` loop counter never
    // reaches counts past 0x7fff.
    if geom_count < 0 || frame_count < 0 || geom_count > 0x7fff {
        return Err(MeshError::Malformed);
    }

    let mut meshes = Vec::with_capacity(geom_count as usize);
    for _ in 0..geom_count {
        let object_name: [u8; 32] = r.take(32)?.try_into().unwrap();
        let diffuse_map_file_name: [u8; 128] = r.take(128)?.try_into().unwrap();

        let mut shared = EffectMeshFrame::default();
        if !v2 {
            // __LoadData_Ver001: one set of counts for every frame.
            shared.vertex_count = r.u32()?;
            shared.index_count = r.u32()?;
            shared.texture_vertex_count = r.u32()?;
        }
        let mut frames = Vec::new();
        for _ in 0..frame_count {
            let mut frame = EffectMeshFrame::default();
            if v2 {
                frame.changed_frame = r.take(1)?[0];
                frame.visibility = r.f32()?;
                frame.vertex_count = r.u32()?;
                frame.index_count = r.u32()?;
                frame.texture_vertex_count = r.u32()?;
            } else {
                frame.vertex_count = shared.vertex_count;
                frame.index_count = shared.index_count;
                frame.texture_vertex_count = shared.texture_vertex_count;
                frame.visibility = r.f32()?;
            }
            read_frame(&mut r, &mut frame)?;
            frames.push(frame);
        }

        let mut mesh = EffectMeshElement { object_name, diffuse_map_file_name, frames, textures: Vec::new() };
        mesh.textures = diffuse_textures(manager, mesh.diffuse_map_file_name());
        meshes.push(mesh);
    }

    Ok(EffectMesh { geom_count, frame_count, meshes })
}

/// `CEffectMesh(name).Load()`: reads `name` through the pack manager.
pub fn load_effect_mesh(manager: &PackManager, name: impl AsRef<[u8]>) -> Result<EffectMesh, MeshError> {
    let data = manager.get(name).map_err(MeshError::Pack)?;
    parse_effect_mesh(manager, &data)
}
