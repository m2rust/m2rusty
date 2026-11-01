//! [`FixedFunctionMaterial`]: a Bevy [`Material`] reproducing Direct3D 9
//! fixed-function state for the game's models, trees and UI quads.

use bevy::asset::{Asset, Handle};
use bevy::ecs::system::SystemParamItem;
use bevy::ecs::system::lifetimeless::SRes;
use bevy::image::Image;
use bevy::math::{UVec4, Vec4};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::material::AlphaMode;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::shader::ShaderRef;
use bevy::reflect::TypePath;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AsBindGroup, AsBindGroupError, BindGroupBuilder, BindGroupLayout, BindGroupLayoutEntry, BindingType,
    BufferBindingType, BufferInitDescriptor, BufferUsages, ColorWrites, RenderPipelineDescriptor, SamplerBindingType,
    ShaderStages, ShaderType, SpecializedMeshPipelineError, TextureSampleType, TextureViewDimension,
    UnpreparedBindingResource, encase,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::{FallbackImage, GpuImage};

use crate::state::{
    AlphaTest, Blend, BlendFactor, CullMode, DepthState, Fog, FogMode, Lighting, PipelineKey, SamplerState,
    TextureStage,
};

/// Embedded shader path, registered by [`crate::M2RenderPlugin`].
pub const SHADER_PATH: &str = "embedded://m2_render/fixed_function.wesl";

/// GPU uniform of [`FixedFunctionMaterial`] (binding 0 of the material bind
/// group). The layout is mirrored by `struct FixedFunction` in the shader.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub struct FixedFunctionUniform {
    pub fog_color: Vec4,
    pub texture_factor: Vec4,
    pub material_diffuse: Vec4,
    pub material_ambient: Vec4,
    pub material_emissive: Vec4,
    pub global_ambient: Vec4,
    pub light_direction: Vec4,
    pub light_diffuse: Vec4,
    pub light_ambient: Vec4,
    pub stage_color: [UVec4; 2],
    pub stage_alpha: [UVec4; 2],
    pub fog_params: Vec4,
    /// fog mode (0 off, 1 linear, 2 exp, 3 exp2), alpha func (0 off), lighting, texture mask.
    pub flags: UVec4,
    pub alpha_ref: UVec4,
}

/// A material whose look is defined by D3D9 fixed-function state.
///
/// Two texture stages are modelled (the GL backend applies two). A stage
/// without a texture is skipped. Colours are gamma-space; textures must be
/// created as non-sRGB formats (for example `Rgba8Unorm`) so no decode happens.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct FixedFunctionMaterial {
    pub textures: [Option<Handle<Image>>; 2],
    pub stages: [TextureStage; 2],
    pub samplers: [SamplerState; 2],
    /// `D3DRS_TEXTUREFACTOR`, gamma space.
    pub texture_factor: Vec4,
    /// Primary colour for meshes without a vertex-colour attribute, and the
    /// material diffuse colour when lighting is on. White by default.
    pub diffuse: Vec4,
    pub alpha_test: Option<AlphaTest>,
    pub blend: Option<Blend>,
    pub cull: CullMode,
    pub depth: DepthState,
    pub fog: Option<Fog>,
    pub lighting: Option<Lighting>,
    /// Draw-order nudge for the transparent phase (see `Material::depth_bias`).
    pub depth_bias: f32,
}

impl Default for FixedFunctionMaterial {
    fn default() -> Self {
        Self {
            textures: [None, None],
            stages: [TextureStage::MODULATE, TextureStage::DISABLED],
            samplers: [SamplerState::default(); 2],
            texture_factor: Vec4::ONE,
            diffuse: Vec4::ONE,
            alpha_test: None,
            blend: None,
            cull: CullMode::default(),
            depth: DepthState::default(),
            fog: None,
            lighting: None,
            depth_bias: 0.0,
        }
    }
}

impl FixedFunctionMaterial {
    /// Opaque, depth-tested, textured: stage 0 `MODULATE(TEXTURE, DIFFUSE)`.
    pub fn opaque(texture: Handle<Image>) -> Self {
        Self { textures: [Some(texture), None], ..Default::default() }
    }

    /// Without a texture: the primary (vertex or material) colour is output.
    pub fn untextured() -> Self {
        Self::default()
    }

    /// `ALPHATESTENABLE` with `GREATEREQUAL` and the given reference (0..=255),
    /// the client's usual mode. Use [`Self::alpha_test`] for another function.
    pub fn alpha_tested(mut self, reference: u8) -> Self {
        self.alpha_test = Some(AlphaTest::greater_equal(reference));
        self
    }

    pub fn alpha_test(mut self, test: AlphaTest) -> Self {
        self.alpha_test = Some(test);
        self
    }

    /// `ALPHABLENDENABLE` with the given factors and the `ADD` op.
    pub fn blended(mut self, src: BlendFactor, dst: BlendFactor) -> Self {
        self.blend = Some(Blend::new(src, dst));
        self
    }

    pub fn blend(mut self, blend: Blend) -> Self {
        self.blend = Some(blend);
        self
    }

    pub fn fog(mut self, fog: Fog) -> Self {
        self.fog = Some(fog);
        self
    }

    /// Enables fixed-function lighting (needs mesh normals).
    pub fn lit(mut self, lighting: Lighting) -> Self {
        self.lighting = Some(lighting);
        self
    }

    pub fn cull(mut self, cull: CullMode) -> Self {
        self.cull = cull;
        self
    }

    pub fn depth(mut self, depth: DepthState) -> Self {
        self.depth = depth;
        self
    }

    /// Sampler state of stage 0.
    pub fn sampler(mut self, sampler: SamplerState) -> Self {
        self.samplers[0] = sampler;
        self
    }

    /// Replaces the stage 0 operations.
    pub fn stage0(mut self, stage: TextureStage) -> Self {
        self.stages[0] = stage;
        self
    }

    /// Adds a texture on stage 1 with its operations; stage 1 sees stage 0's
    /// result as `CURRENT`.
    pub fn stage1(mut self, texture: Handle<Image>, stage: TextureStage, sampler: SamplerState) -> Self {
        self.textures[1] = Some(texture);
        self.stages[1] = stage;
        self.samplers[1] = sampler;
        self
    }

    pub fn texture_factor(mut self, factor: Vec4) -> Self {
        self.texture_factor = factor;
        self
    }

    /// Primary colour for meshes without vertex colours.
    pub fn diffuse(mut self, diffuse: Vec4) -> Self {
        self.diffuse = diffuse;
        self
    }

    pub fn depth_bias(mut self, bias: f32) -> Self {
        self.depth_bias = bias;
        self
    }

    /// The discrete state that selects the render pipeline.
    pub fn pipeline_key(&self) -> PipelineKey {
        PipelineKey { blend: self.blend, cull: self.cull, depth: self.depth }
    }

    /// The continuous state as the GPU uniform. Pure; unit-tested.
    pub fn uniform(&self) -> FixedFunctionUniform {
        let (fog_mode, fog_color, fog_params) = match self.fog.filter(|fog| fog.is_active()) {
            None => (0, Vec4::ZERO, Vec4::ZERO),
            Some(Fog { mode: FogMode::Linear { start, end }, color }) => (1, color, Vec4::new(start, end, 0.0, 0.0)),
            Some(Fog { mode: FogMode::Exp { density }, color }) => (2, color, Vec4::new(0.0, 0.0, density, 0.0)),
            Some(Fog { mode: FogMode::Exp2 { density }, color }) => (3, color, Vec4::new(0.0, 0.0, density, 0.0)),
        };
        let stage_color = self.stages.map(|s| UVec4::new(s.color_op as u32, s.color_arg1.pack(), s.color_arg2.pack(), 0));
        let stage_alpha = self.stages.map(|s| UVec4::new(s.alpha_op as u32, s.alpha_arg1.pack(), s.alpha_arg2.pack(), 0));
        let texture_mask = self.textures.iter().enumerate().fold(0u32, |mask, (i, t)| mask | (t.is_some() as u32) << i);
        let (alpha_func, alpha_ref) = match self.alpha_test {
            Some(test) => (test.func as u32, test.reference as u32),
            None => (0, 0),
        };
        let lit = self.lighting;
        FixedFunctionUniform {
            fog_color,
            texture_factor: self.texture_factor,
            material_diffuse: self.diffuse,
            material_ambient: lit.map_or(Vec4::ONE, |l| l.material_ambient),
            material_emissive: lit.map_or(Vec4::ZERO, |l| l.material_emissive),
            global_ambient: lit.map_or(Vec4::ZERO, |l| l.global_ambient),
            light_direction: lit.map_or(Vec4::ZERO, |l| l.direction.extend(0.0)),
            light_diffuse: lit.map_or(Vec4::ZERO, |l| l.light_diffuse),
            light_ambient: lit.map_or(Vec4::ZERO, |l| l.light_ambient),
            stage_color,
            stage_alpha,
            fog_params,
            flags: UVec4::new(fog_mode, alpha_func, lit.is_some() as u32, texture_mask),
            alpha_ref: UVec4::new(alpha_ref, 0, 0, 0),
        }
    }
}

impl AsBindGroup for FixedFunctionMaterial {
    type Data = PipelineKey;
    type Param = (SRes<RenderAssets<GpuImage>>, SRes<FallbackImage>);

    fn label() -> &'static str {
        "m2_fixed_function_material"
    }

    fn bind_group_data(&self) -> Self::Data {
        self.pipeline_key()
    }

    fn build_bind_group(
        &self,
        _layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (images, fallback): &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
        output: &mut BindGroupBuilder,
    ) -> Result<(), AsBindGroupError> {
        // Fallible lookups first so a missing image retries before any allocation.
        let mut views = Vec::with_capacity(2);
        for texture in &self.textures {
            let view = match texture {
                Some(handle) => images.get(handle).ok_or(AsBindGroupError::RetryNextUpdate)?.texture_view.clone(),
                None => fallback.d2.texture_view.clone(),
            };
            views.push(view);
        }

        let mut buffer = encase::UniformBuffer::new(Vec::new());
        buffer.write(&self.uniform()).expect("the uniform is a valid ShaderType");
        let uniform = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("m2_fixed_function_uniform"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            contents: buffer.as_ref(),
        });
        output.binding_resources.push((0, UnpreparedBindingResource::Buffer(uniform)));

        for (i, view) in views.into_iter().enumerate() {
            let sampler = render_device.create_sampler(&self.samplers[i].descriptor().as_wgpu());
            output
                .binding_resources
                .push((1 + 2 * i as u32, UnpreparedBindingResource::TextureView(TextureViewDimension::D2, view)));
            output
                .binding_resources
                .push((2 + 2 * i as u32, UnpreparedBindingResource::Sampler(SamplerBindingType::Filtering, sampler)));
        }
        Ok(())
    }

    fn bind_group_layout_entries(_render_device: &RenderDevice, _force_no_bindless: bool) -> Vec<BindGroupLayoutEntry> {
        let visibility = ShaderStages::VERTEX_FRAGMENT;
        let mut entries = vec![BindGroupLayoutEntry {
            binding: 0,
            visibility,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: Some(FixedFunctionUniform::min_size()),
            },
            count: None,
        }];
        for stage in 0..2u32 {
            entries.push(BindGroupLayoutEntry {
                binding: 1 + 2 * stage,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            });
            entries.push(BindGroupLayoutEntry {
                binding: 2 + 2 * stage,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            });
        }
        entries
    }
}

impl Material for FixedFunctionMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        // Picks the render phase: blended draws go to the sorted transparent phase.
        if self.blend.is_some() { AlphaMode::Blend } else { AlphaMode::Opaque }
    }

    fn depth_bias(&self) -> f32 {
        self.depth_bias
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let states = key.bind_group_data.states();
        descriptor.primitive.cull_mode = states.cull_mode;
        if let Some(depth_stencil) = descriptor.depth_stencil.as_mut() {
            depth_stencil.depth_write_enabled = Some(states.depth_write);
            depth_stencil.depth_compare = Some(states.depth_compare);
        }
        if let Some(fragment) = descriptor.fragment.as_mut() {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = states.blend;
                target.write_mask = ColorWrites::ALL;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ArgSource, CompareFunc, TextureArg, TextureOp, d3d_color};
    use bevy::math::Vec3;

    #[test]
    fn defaults_match_the_clients_state() {
        let m = FixedFunctionMaterial::untextured();
        let u = m.uniform();
        // stage 0: MODULATE(TEXTURE, DIFFUSE), alpha SELECTARG1(TEXTURE); stage 1 off.
        assert_eq!(u.stage_color[0], UVec4::new(4, 2, 0, 0));
        assert_eq!(u.stage_alpha[0], UVec4::new(2, 2, 0, 0));
        assert_eq!(u.stage_color[1].x, TextureOp::Disable as u32);
        assert_eq!(u.flags, UVec4::new(0, 0, 0, 0));
        assert_eq!(
            m.pipeline_key(),
            PipelineKey { blend: None, cull: CullMode::Cw, depth: DepthState::default() }
        );
    }

    #[test]
    fn texture_mask_follows_bound_textures() {
        let m = FixedFunctionMaterial::opaque(Handle::default());
        assert_eq!(m.uniform().flags.w, 0b01);
        let m = m.stage1(Handle::default(), TextureStage::MODULATE, SamplerState::default());
        assert_eq!(m.uniform().flags.w, 0b11);
    }

    #[test]
    fn alpha_test_packs_function_and_reference() {
        let u = FixedFunctionMaterial::untextured().alpha_test(AlphaTest::greater(128)).uniform();
        assert_eq!(u.flags.y, CompareFunc::Greater as u32);
        assert_eq!(u.alpha_ref.x, 128);
        let u = FixedFunctionMaterial::untextured().alpha_tested(1).uniform();
        assert_eq!(u.flags.y, CompareFunc::GreaterEqual as u32);
        assert_eq!(u.alpha_ref.x, 1);
    }

    #[test]
    fn fog_modes_pack_into_uniform() {
        let color = d3d_color(0xFFEBF0FF);
        let u = FixedFunctionMaterial::untextured().fog(Fog::linear(color, 3.0, 12.0)).uniform();
        assert_eq!(u.flags.x, 1);
        assert_eq!(u.fog_params, Vec4::new(3.0, 12.0, 0.0, 0.0));
        assert_eq!(u.fog_color, color);
        let u = FixedFunctionMaterial::untextured().fog(Fog::exp(color, 0.25)).uniform();
        assert_eq!((u.flags.x, u.fog_params.z), (2, 0.25));
        let u = FixedFunctionMaterial::untextured().fog(Fog::exp2(color, 0.5)).uniform();
        assert_eq!((u.flags.x, u.fog_params.z), (3, 0.5));
        // Degenerate linear fog is dropped, as in the GL backend.
        let u = FixedFunctionMaterial::untextured().fog(Fog::linear(color, 5.0, 5.0)).uniform();
        assert_eq!(u.flags.x, 0);
    }

    #[test]
    fn builder_sets_pipeline_state() {
        let m = FixedFunctionMaterial::untextured()
            .blended(BlendFactor::SrcAlpha, BlendFactor::One)
            .cull(CullMode::None)
            .depth(DepthState { test: true, write: false, func: CompareFunc::Less });
        let key = m.pipeline_key();
        assert_eq!(key.blend, Some(Blend::ADDITIVE));
        assert_eq!(key.cull, CullMode::None);
        assert!(!key.depth.write);
        assert!(matches!(m.alpha_mode(), AlphaMode::Blend));
        assert!(matches!(FixedFunctionMaterial::untextured().alpha_mode(), AlphaMode::Opaque));
    }

    #[test]
    fn lighting_packs_into_uniform() {
        let l = Lighting::directional(Vec3::NEG_Y, Vec4::new(1.0, 0.5, 0.25, 1.0), Vec4::splat(0.2));
        let u = FixedFunctionMaterial::untextured().lit(l).uniform();
        assert_eq!(u.flags.z, 1);
        assert_eq!(u.light_direction, Vec4::new(0.0, -1.0, 0.0, 0.0));
        assert_eq!(u.light_diffuse, Vec4::new(1.0, 0.5, 0.25, 1.0));
        assert_eq!(u.light_ambient, Vec4::splat(0.2));
    }

    #[test]
    fn stage_args_pack_with_modifiers() {
        let stage = TextureStage {
            color_op: TextureOp::BlendFactorAlpha,
            color_arg1: TextureArg { source: ArgSource::Texture, complement: true, alpha_replicate: false },
            color_arg2: TextureArg::TFACTOR.alpha_replicate(),
            ..TextureStage::MODULATE
        };
        let u = FixedFunctionMaterial::untextured().stage0(stage).uniform();
        assert_eq!(u.stage_color[0], UVec4::new(14, 2 | 0x100, 3 | 0x200, 0));
    }

    #[test]
    fn uniform_has_a_16_byte_aligned_size() {
        assert_eq!(FixedFunctionUniform::min_size().get() % 16, 0);
    }
}
