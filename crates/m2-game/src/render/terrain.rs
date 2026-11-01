//! Single-pass, opaque terrain. Splat blending happens in the shader, never
//! between coplanar meshes in the scene's transparent queue.
use bevy::asset::{RenderAssetUsages, embedded_asset};
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::pbr::{Material, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, ShaderType, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
use bevy::shader::ShaderRef;
use m2_formats::{map::{Terrain, TerrainTexture}, pack::PackManager};
use m2_render::{Fog, FogMode};

const MAX_LAYERS: usize = 256;

#[derive(Clone, ShaderType, Debug)]
pub struct TerrainUniform {
    pub transforms: [Vec4; MAX_LAYERS],
    pub layers: [UVec4; MAX_LAYERS / 4],
    pub count: UVec4,
    pub fog_color: Vec4,
    /// x = fog start, y = fog end, z = density.
    pub fog_params: Vec4,
    /// x = fog mode (0 off, 1 linear, 2 exp, 3 exp2).
    pub fog_mode: UVec4,
    /// xyz = direction the light travels (Bevy space), normalized; w unused.
    pub light_direction: Vec4,
    /// `material.diffuse * light.diffuse`, gamma space.
    pub light_diffuse: Vec4,
    /// `material.ambient * light.ambient + material.emissive`, gamma space.
    pub light_ambient: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct TerrainMaterial {
    #[uniform(0)]
    pub params: TerrainUniform,
    #[texture(1, dimension = "2d_array")]
    #[sampler(2)]
    pub ground: Handle<Image>,
    #[texture(3, dimension = "2d_array")]
    #[sampler(4)]
    pub masks: Handle<Image>,
}

impl Material for TerrainMaterial {
    fn vertex_shader() -> ShaderRef { "embedded://m2_game/render/terrain.wesl".into() }
    fn fragment_shader() -> ShaderRef { "embedded://m2_game/render/terrain.wesl".into() }
    fn enable_prepass() -> bool { false }
    fn enable_shadows() -> bool { false }
}

/// The terrain material and its shader.
pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "terrain.wesl");
        app.add_plugins(MaterialPlugin::<TerrainMaterial>::default());
    }
}

/// The per-patch uniform: every texture-set layer's UV transform, and the
/// layers this patch actually uses (its non-zero tile counts).
pub fn patch_uniform(textures: &[TerrainTexture], counts: &[u32; 256]) -> TerrainUniform {
    let mut params = TerrainUniform {
        transforms: [Vec4::ZERO; MAX_LAYERS],
        layers: [UVec4::ZERO; MAX_LAYERS / 4],
        count: UVec4::ZERO,
        fog_color: Vec4::ZERO,
        fog_params: Vec4::ZERO,
        fog_mode: UVec4::ZERO,
        // A sun-like key and neutral fill, for a caller that builds terrain
        // without a map environment. Any map overwrites these with
        // `set_light`.
        light_direction: Vec4::new(0.35, -0.85, 0.4, 0.0),
        light_diffuse: Vec4::new(0.95, 0.93, 0.88, 0.0),
        light_ambient: Vec4::new(0.35, 0.35, 0.35, 0.0),
    };
    for (id, texture) in textures.iter().take(MAX_LAYERS).enumerate().skip(1) {
        params.transforms[id] = Vec4::new(texture.u_scale * 0.2, texture.v_scale * 0.2, texture.u_offset, -texture.v_offset);
        if counts[id] > 0 && !texture.file_name.is_empty() {
            let index = params.count.x as usize;
            params.layers[index / 4][index % 4] = id as u32;
            params.count.x += 1;
        }
    }
    // Slice zero is an explicit fallback for terrain with no texture data.
    params.count.x = params.count.x.max(1);
    params
}

/// `D3DRS_FOG*` for the terrain pass. The shader mirrors the fixed-function
/// material's per-fragment fog exactly.
pub fn set_fog(params: &mut TerrainUniform, fog: Option<Fog>) {
    let Some(fog) = fog.filter(|f| f.is_active()) else {
        params.fog_mode = UVec4::ZERO;
        return;
    };
    params.fog_color = fog.color;
    params.fog_mode = match fog.mode {
        FogMode::Linear { start, end } => {
            params.fog_params = Vec4::new(start, end, 0.0, 0.0);
            UVec4::new(1, 0, 0, 0)
        }
        FogMode::Exp { density } => {
            params.fog_params = Vec4::new(0.0, 0.0, density, 0.0);
            UVec4::new(2, 0, 0, 0)
        }
        FogMode::Exp2 { density } => {
            params.fog_params = Vec4::new(0.0, 0.0, density, 0.0);
            UVec4::new(3, 0, 0, 0)
        }
    };
}

/// The map environment's directional light, as
/// `CTerrainPatch::SoftwareTransformPatch_UpdateTerrainLighting`
/// (`client/src/game/TerrainPatch.cpp`) bakes it into the terrain's vertex
/// colours: `light.diffuse * material.diffuse` and
/// `light.ambient * material.ambient + material.emissive`, evaluated against
/// `abs(dot(normal, direction))` in the shader.
pub fn set_light(params: &mut TerrainUniform, light: &crate::render::lighting::SceneLight) {
    let (direction, diffuse, ambient) = light.terrain_colours();
    params.light_direction = direction;
    params.light_diffuse = diffuse;
    params.light_ambient = ambient;
}

/// One terrain's splat alpha masks, one 256×256 array slice per texture-set entry.
pub fn masks(terrain: &Terrain, texture_count: usize) -> Image {
    let count = texture_count.clamp(1, MAX_LAYERS);
    let mut pixels = vec![0; 256 * 256 * count];
    for splat in m2_world::splat::generate_splats(terrain, count) {
        pixels[splat.texture * 256 * 256..(splat.texture + 1) * 256 * 256].copy_from_slice(&splat.alpha);
    }
    array_image(256, 256, count, pixels, TextureFormat::R8Unorm, false)
}

/// The texture set as one mipmapped 2D array (missing textures become a flat
/// grass colour), plus the decode errors. Slow: build it off the main thread.
pub fn ground(packs: &PackManager, textures: &[TerrainTexture]) -> (Image, Vec<String>) {
    let mut errors = Vec::new();
    let decoded: Vec<_> = textures.iter().take(MAX_LAYERS).map(|texture| {
        if texture.file_name.is_empty() { return None; }
        let result = packs.get(&texture.file_name).map_err(|e| e.to_string())
            .and_then(|bytes| m2_assets::texture::decode(&bytes, &Default::default()));
        match result {
            Ok(image) => Some(image),
            Err(error) => { errors.push(format!("Terrain texture {}: {error}", String::from_utf8_lossy(&texture.file_name))); None }
        }
    }).collect();
    let width = decoded.iter().flatten().map(Image::width).max().unwrap_or(1);
    let height = decoded.iter().flatten().map(Image::height).max().unwrap_or(1);
    let count = decoded.len().max(1);
    let mut pixels = Vec::new();
    let mut mip_count = 1;
    for index in 0..count {
        let (source, sw, sh) = decoded.get(index).and_then(Option::as_ref)
            .map_or((&[107, 117, 82, 255][..], 1, 1), |image| (image.data.as_deref().unwrap(), image.width(), image.height()));
        let mut level = resample_rgba(source, sw, sh, width, height);
        let (mut w, mut h) = (width, height);
        pixels.extend_from_slice(&level);
        let mut levels = 1;
        while w > 1 || h > 1 {
            let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
            level = resample_rgba(&level, w, h, nw, nh);
            pixels.extend_from_slice(&level);
            (w, h) = (nw, nh);
            levels += 1;
        }
        mip_count = levels;
    }
    // Data is layer-major, each slice followed by its complete mip chain.
    let mut image = array_image(width, height, count, Vec::new(), TextureFormat::Rgba8Unorm, true);
    image.data = Some(pixels);
    image.texture_descriptor.mip_level_count = mip_count;
    (image, errors)
}

fn array_image(width: u32, height: u32, count: usize, data: Vec<u8>, format: TextureFormat, repeat: bool) -> Image {
    let mut image = Image::new_uninit(Extent3d { width, height, depth_or_array_layers: count as u32 }, TextureDimension::D2, format, RenderAssetUsages::default());
    image.data = Some(data);
    image.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::D2Array), ..Default::default() });
    let address = if repeat { ImageAddressMode::Repeat } else { ImageAddressMode::ClampToEdge };
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor { address_mode_u: address, address_mode_v: address, ..ImageSamplerDescriptor::linear() });
    image
}

/// Bilinear resize for texture-array slices and their mip levels. Equal-sized
/// textures preserve their level-zero bytes exactly; coordinates wrap at edges.
fn resample_rgba(src: &[u8], sw: u32, sh: u32, width: u32, height: u32) -> Vec<u8> {
    if (sw, sh) == (width, height) { return src[..(sw * sh * 4) as usize].to_vec(); }
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        let sy = (y as f32 + 0.5) * sh as f32 / height as f32 - 0.5;
        let fy = sy - sy.floor();
        for x in 0..width {
            let sx = (x as f32 + 0.5) * sw as f32 / width as f32 - 0.5;
            let fx = sx - sx.floor();
            let sample = |dx: i32, dy: i32, channel: usize| {
                let xx = (sx.floor() as i32 + dx).rem_euclid(sw as i32) as u32;
                let yy = (sy.floor() as i32 + dy).rem_euclid(sh as i32) as u32;
                src[((yy * sw + xx) * 4) as usize + channel] as f32
            };
            for c in 0..4 {
                let top = sample(0,0,c) * (1.0-fx) + sample(1,0,c) * fx;
                let bottom = sample(0,1,c) * (1.0-fx) + sample(1,1,c) * fx;
                out.push((top * (1.0-fy) + bottom * fy).round() as u8);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn patch_uses_texture_order_and_skips_missing_entries() {
        let mut textures = vec![TerrainTexture::default(); 5];
        textures[1].file_name = b"grass.dds".to_vec();
        textures[3].file_name = b"sand.dds".to_vec();
        let mut counts = [0; 256];
        counts[1] = 10; counts[2] = 10; counts[3] = 10;
        let params = patch_uniform(&textures, &counts);
        assert_eq!(params.count.x, 2);
        assert_eq!(params.layers[0], UVec4::new(1, 3, 0, 0));
        assert_eq!(patch_uniform(&[], &counts).layers[0], UVec4::ZERO);
    }
    #[test]
    fn resize_preserves_texels_and_averages_mips() {
        let pixels = [0,0,0,255, 100,100,100,255, 200,200,200,255, 100,100,100,255];
        assert_eq!(resample_rgba(&pixels, 2,2,2,2), pixels);
        assert_eq!(resample_rgba(&pixels, 2,2,1,1), [100,100,100,255]);
    }
    #[test]
    fn the_environment_light_is_the_terrain_vertex_lighting() {
        let light = crate::render::lighting::SceneLight {
            direction: Vec3::new(0.0, 1.0, 0.0),
            diffuse: Vec4::new(1.0, 0.5, 0.0, 1.0),
            ambient: Vec4::splat(0.1),
            material_diffuse: Vec4::new(1.0, 1.0, 0.5, 1.0),
            material_ambient: Vec4::splat(0.8),
            material_emissive: Vec4::ZERO,
            global_ambient: Vec4::ZERO,
        };
        let mut params = patch_uniform(&[], &[0; 256]);
        set_light(&mut params, &light);
        assert_eq!(params.light_direction, Vec4::new(0.0, 1.0, 0.0, 0.0));
        // material.diffuse * light.diffuse
        assert_eq!(params.light_diffuse, Vec4::new(1.0, 0.5, 0.0, 1.0));
        // material.ambient * light.ambient + material.emissive
        assert!((params.light_ambient - Vec4::splat(0.08)).abs().max_element() < 1e-6);
    }
    #[test]
    fn terrain_is_opaque_and_writes_depth_without_custom_sorting() {
        use bevy::material::AlphaMode;
        let material = TerrainMaterial { params: patch_uniform(&[], &[0;256]), ground: Handle::default(), masks: Handle::default() };
        assert_eq!(material.alpha_mode(), AlphaMode::Opaque);
        assert_eq!(material.depth_bias(), 0.0);
    }
}
