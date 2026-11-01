//! Terrain texture sets (`CTextureSet::Load`, `neoeter/src/terrain/TextureSet.cpp`):
//!
//! ```text
//! TextureSet
//! TextureCount 2
//! Start Texture001
//!     "d:/ymir work/terrain/a.dds"
//!     4.0 4.0 0.0 0.0     (u/v scale, u/v offset)
//!     1                   (splat)
//!     0 65535             (begin, end height)
//! End Texture001
//! ```

use crate::pack::{self, PackManager};
use crate::script::{atof_f32, atoi, parse_multiple_text_data};

use super::{atol, token};

/// `CMapOutdoor::m_fTerrainTexCoordBase`:
/// `1.0f / (float)(PATCH_XSIZE * CELLSCALE)`, what `LoadSetting` passes to
/// `CTextureSet::Load`.
pub const TERRAIN_TEX_COORD_BASE: f32 = 1.0 / 3200.0;

/// `TTerrainTexture` without the image. Defaults are its constructor's.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainTexture {
    pub file_name: Vec<u8>,
    pub u_scale: f32,
    pub v_scale: f32,
    pub u_offset: f32,
    pub v_offset: f32,
    pub splat: bool,
    /// Height range (16-bit height map values) the texture is meant for.
    pub begin: u16,
    pub end: u16,
    /// `m_matTransform`, row-major `_11`…`_44`: identity, or what
    /// `SetTexture` computes from the scales and offsets.
    pub transform: [f32; 16],
}

const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

impl Default for TerrainTexture {
    fn default() -> Self {
        Self {
            file_name: Vec::new(),
            u_scale: 4.0,
            v_scale: 4.0,
            u_offset: 0.0,
            v_offset: 0.0,
            splat: true,
            begin: 0,
            end: 0,
            transform: IDENTITY,
        }
    }
}

impl TerrainTexture {
    /// The matrix of `CTextureSet::SetTexture`: `D3DXMatrixScaling(base *
    /// UScale, -base * VScale, 0)` with `_41 = UOffset`, `_42 = -VOffset`.
    pub fn compute_transform(&self, tex_coord_base: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        m[0] = tex_coord_base * self.u_scale;
        m[5] = -tex_coord_base * self.v_scale;
        m[10] = 0.0;
        m[12] = self.u_offset;
        m[13] = -self.v_offset;
        m
    }
}

/// A loaded `CTextureSet`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TextureSet {
    /// Index 0 is the eraser (a default texture); `texture%03d` keys fill
    /// 1..=`TextureCount`, and entries without a key stay default.
    pub textures: Vec<TerrainTexture>,
    /// `m_stFileName`: the name it was loaded from.
    pub file_name: Vec<u8>,
}

/// Why `CTextureSet::Load` returned false. The set is empty then.
#[derive(Debug)]
pub enum TextureSetError {
    Pack(pack::Error),
    /// The `TextureSet` or `TextureCount` key is missing.
    MissingKey(&'static str),
}

impl TextureSet {
    /// `CTextureSet::Load` after reading the file.
    ///
    /// Deviations (undefined behaviour in C++): a `Texture%03d` block with
    /// fewer than eight values reads the missing ones as empty tokens, and a
    /// `TextureCount` below -1 (a huge `resize`) is treated like -1, which
    /// leaves no textures at all, not even the eraser.
    pub fn parse(data: &[u8], name: &[u8], tex_coord_base: f32) -> Result<Self, TextureSetError> {
        let map = parse_multiple_text_data(data);
        if !map.contains_key(b"textureset".as_slice()) {
            return Err(TextureSetError::MissingKey("textureset"));
        }
        let Some(count) = map.get(b"texturecount".as_slice()) else {
            return Err(TextureSetError::MissingKey("texturecount"));
        };
        let count = atol(token(count, 0));

        // Create() adds the eraser, then resize(count + 1).
        let len = usize::try_from(count.saturating_add(1)).unwrap_or(0);
        let mut textures = vec![TerrainTexture::default()];
        textures.resize(len, TerrainTexture::default());

        for i in 0..count.max(0) {
            // "texture%03d" of a long passed as int.
            let key = format!("texture{:03}", (i + 1) as i32);
            let Some(v) = map.get(key.as_bytes()) else {
                continue;
            };
            let mut texture = TerrainTexture {
                file_name: token(v, 0).to_vec(),
                u_scale: atof_f32(token(v, 1)),
                v_scale: atof_f32(token(v, 2)),
                u_offset: atof_f32(token(v, 3)),
                v_offset: atof_f32(token(v, 4)),
                splat: atoi(token(v, 5)) != 0,
                begin: atoi(token(v, 6)) as u16,
                end: atoi(token(v, 7)) as u16,
                transform: IDENTITY,
            };
            texture.transform = texture.compute_transform(tex_coord_base);
            textures[(i + 1) as usize] = texture;
        }

        Ok(Self {
            textures,
            file_name: name.to_vec(),
        })
    }

    /// `CTextureSet::Load`: reads `name` through the pack manager.
    pub fn load(manager: &PackManager, name: &[u8], tex_coord_base: f32) -> Result<Self, TextureSetError> {
        let data = manager.get(name).map_err(TextureSetError::Pack)?;
        Self::parse(&data, name, tex_coord_base)
    }
}
