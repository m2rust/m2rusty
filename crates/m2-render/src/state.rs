//! Fixed-function render state vocabulary and its mapping to GPU state.
//!
//! The enums mirror the Direct3D 9 values the original client uses (see
//! `neoeter/include/Platform/RenderBackend.h`), restricted to what the GL
//! reference backend applies plus a few cheap D3D-correct extras listed in
//! the README. Everything here is plain data: the mapping functions are pure
//! and unit-tested without a GPU.

use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSamplerDescriptor};
use bevy::math::Vec4;
use bevy::render::render_resource::{
    BlendComponent, BlendFactor as WgpuBlendFactor, BlendOperation, BlendState, CompareFunction, Face,
};

/// `D3DBLEND_*`. `BOTHSRCALPHA`/`BOTHINVSRCALPHA`/`BLENDFACTOR` are not modelled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendFactor {
    Zero,
    #[default]
    One,
    SrcColor,
    InvSrcColor,
    SrcAlpha,
    InvSrcAlpha,
    DestAlpha,
    InvDestAlpha,
    DestColor,
    InvDestColor,
    /// `D3DBLEND_SRCALPHASAT`; the GL backend maps it to ONE, this crate honours it.
    SrcAlphaSaturate,
}

impl BlendFactor {
    pub fn to_wgpu(self) -> WgpuBlendFactor {
        match self {
            Self::Zero => WgpuBlendFactor::Zero,
            Self::One => WgpuBlendFactor::One,
            Self::SrcColor => WgpuBlendFactor::Src,
            Self::InvSrcColor => WgpuBlendFactor::OneMinusSrc,
            Self::SrcAlpha => WgpuBlendFactor::SrcAlpha,
            Self::InvSrcAlpha => WgpuBlendFactor::OneMinusSrcAlpha,
            Self::DestAlpha => WgpuBlendFactor::DstAlpha,
            Self::InvDestAlpha => WgpuBlendFactor::OneMinusDstAlpha,
            Self::DestColor => WgpuBlendFactor::Dst,
            Self::InvDestColor => WgpuBlendFactor::OneMinusDst,
            Self::SrcAlphaSaturate => WgpuBlendFactor::SrcAlphaSaturated,
        }
    }
}

/// `D3DBLENDOP_*`. The GL backend ignores the blend op (always ADD).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendOp {
    #[default]
    Add,
    Subtract,
    RevSubtract,
    Min,
    Max,
}

impl BlendOp {
    pub fn to_wgpu(self) -> BlendOperation {
        match self {
            Self::Add => BlendOperation::Add,
            Self::Subtract => BlendOperation::Subtract,
            Self::RevSubtract => BlendOperation::ReverseSubtract,
            Self::Min => BlendOperation::Min,
            Self::Max => BlendOperation::Max,
        }
    }
}

/// `D3DRS_ALPHABLENDENABLE` with `SRCBLEND`/`DESTBLEND`/`BLENDOP`. As in the
/// GL backend the same factors apply to colour and alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Blend {
    pub src: BlendFactor,
    pub dst: BlendFactor,
    pub op: BlendOp,
}

impl Blend {
    pub const fn new(src: BlendFactor, dst: BlendFactor) -> Self {
        Self { src, dst, op: BlendOp::Add }
    }

    /// `SRCALPHA`, `INVSRCALPHA`: the client's default blend.
    pub const ALPHA: Self = Self::new(BlendFactor::SrcAlpha, BlendFactor::InvSrcAlpha);
    /// `SRCALPHA`, `ONE`: additive effects.
    pub const ADDITIVE: Self = Self::new(BlendFactor::SrcAlpha, BlendFactor::One);

    pub fn to_wgpu(self) -> BlendState {
        let component = BlendComponent {
            src_factor: self.src.to_wgpu(),
            dst_factor: self.dst.to_wgpu(),
            operation: self.op.to_wgpu(),
        };
        BlendState { color: component, alpha: component }
    }
}

/// `D3DCMP_*`, used for the depth function and the alpha test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CompareFunc {
    Never = 1,
    Less = 2,
    Equal = 3,
    #[default]
    LessEqual = 4,
    Greater = 5,
    NotEqual = 6,
    GreaterEqual = 7,
    Always = 8,
}

impl CompareFunc {
    /// The depth comparison for Bevy's reverse-Z depth buffer: a smaller D3D
    /// depth (nearer) is a larger reverse-Z value, so the order flips.
    pub fn to_reverse_z_wgpu(self) -> CompareFunction {
        match self {
            Self::Never => CompareFunction::Never,
            Self::Less => CompareFunction::Greater,
            Self::Equal => CompareFunction::Equal,
            Self::LessEqual => CompareFunction::GreaterEqual,
            Self::Greater => CompareFunction::Less,
            Self::NotEqual => CompareFunction::NotEqual,
            Self::GreaterEqual => CompareFunction::LessEqual,
            Self::Always => CompareFunction::Always,
        }
    }
}

/// `D3DRS_CULLMODE`. The face is judged by its winding on the final image:
/// `Cw` culls clockwise triangles (the D3D default, so front faces are
/// counter-clockwise like Bevy's), `Ccw` culls counter-clockwise ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    #[default]
    Cw,
    Ccw,
}

impl CullMode {
    /// Bevy's pipelines use counter-clockwise front faces.
    pub fn to_wgpu(self) -> Option<Face> {
        match self {
            Self::None => None,
            Self::Cw => Some(Face::Back),
            Self::Ccw => Some(Face::Front),
        }
    }
}

/// `D3DRS_ZENABLE`, `ZWRITEENABLE`, `ZFUNC`. D3D and GL both skip depth
/// writes when the test is disabled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DepthState {
    pub test: bool,
    pub write: bool,
    pub func: CompareFunc,
}

impl Default for DepthState {
    /// The client's `SetDefaultState`: enabled, writing, `LESSEQUAL`.
    fn default() -> Self {
        Self { test: true, write: true, func: CompareFunc::LessEqual }
    }
}

impl DepthState {
    pub const DISABLED: Self = Self { test: false, write: false, func: CompareFunc::Always };

    pub fn compare(self) -> CompareFunction {
        if self.test { self.func.to_reverse_z_wgpu() } else { CompareFunction::Always }
    }

    pub fn writes(self) -> bool {
        self.test && self.write
    }
}

/// `D3DRS_ALPHATESTENABLE` with `ALPHAFUNC` and `ALPHAREF` (0..=255).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AlphaTest {
    pub func: CompareFunc,
    pub reference: u8,
}

impl AlphaTest {
    pub const fn greater(reference: u8) -> Self {
        Self { func: CompareFunc::Greater, reference }
    }
    pub const fn greater_equal(reference: u8) -> Self {
        Self { func: CompareFunc::GreaterEqual, reference }
    }
}

/// `D3DRS_FOGVERTEXMODE`/`FOGTABLEMODE` with start/end/density. The fog
/// coordinate is the distance along the view axis, as in the GL backend.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FogMode {
    Linear { start: f32, end: f32 },
    Exp { density: f32 },
    Exp2 { density: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub mode: FogMode,
    /// Gamma-space colour; alpha is ignored.
    pub color: Vec4,
}

impl Fog {
    pub fn linear(color: Vec4, start: f32, end: f32) -> Self {
        Self { mode: FogMode::Linear { start, end }, color }
    }
    pub fn exp(color: Vec4, density: f32) -> Self {
        Self { mode: FogMode::Exp { density }, color }
    }
    pub fn exp2(color: Vec4, density: f32) -> Self {
        Self { mode: FogMode::Exp2 { density }, color }
    }
    /// The GL backend disables linear fog whose end is not past its start.
    pub fn is_active(self) -> bool {
        match self.mode {
            FogMode::Linear { start, end } => end > start,
            _ => true,
        }
    }
}

/// `D3DTOP_*` values (the numbering is shared with the shader).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TextureOp {
    Disable = 1,
    SelectArg1 = 2,
    SelectArg2 = 3,
    #[default]
    Modulate = 4,
    Modulate2x = 5,
    Modulate4x = 6,
    Add = 7,
    AddSigned = 8,
    AddSigned2x = 9,
    Subtract = 10,
    AddSmooth = 11,
    BlendDiffuseAlpha = 12,
    BlendTextureAlpha = 13,
    BlendFactorAlpha = 14,
    BlendTextureAlphaPm = 15,
    BlendCurrentAlpha = 16,
    /// `Arg1.rgb + Arg1.a * Arg2.rgb` (`CGrannyMaterial::__ApplySpecularRenderState`).
    ModulateAlphaAddColor = 18,
}

/// `D3DTA_*` source.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArgSource {
    #[default]
    Diffuse = 0,
    Current = 1,
    Texture = 2,
    TFactor = 3,
}

/// A texture-stage argument: source plus the `D3DTA_COMPLEMENT` and
/// `D3DTA_ALPHAREPLICATE` modifiers (replicate only affects colour ops).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureArg {
    pub source: ArgSource,
    pub complement: bool,
    pub alpha_replicate: bool,
}

impl TextureArg {
    pub const DIFFUSE: Self = Self::of(ArgSource::Diffuse);
    pub const CURRENT: Self = Self::of(ArgSource::Current);
    pub const TEXTURE: Self = Self::of(ArgSource::Texture);
    pub const TFACTOR: Self = Self::of(ArgSource::TFactor);

    const fn of(source: ArgSource) -> Self {
        Self { source, complement: false, alpha_replicate: false }
    }

    pub const fn complement(mut self) -> Self {
        self.complement = true;
        self
    }

    pub const fn alpha_replicate(mut self) -> Self {
        self.alpha_replicate = true;
        self
    }

    /// Packed as the shader expects: source | complement << 8 | replicate << 9.
    pub fn pack(self) -> u32 {
        self.source as u32 | (self.complement as u32) << 8 | (self.alpha_replicate as u32) << 9
    }
}

/// One texture stage: `COLOROP`/`COLORARG1/2` and `ALPHAOP`/`ALPHAARG1/2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureStage {
    pub color_op: TextureOp,
    pub color_arg1: TextureArg,
    pub color_arg2: TextureArg,
    pub alpha_op: TextureOp,
    pub alpha_arg1: TextureArg,
    pub alpha_arg2: TextureArg,
}

impl TextureStage {
    /// Stage 0 as the client leaves it: colour `MODULATE(TEXTURE, DIFFUSE)`,
    /// alpha `SELECTARG1(TEXTURE)`.
    pub const MODULATE: Self = Self {
        color_op: TextureOp::Modulate,
        color_arg1: TextureArg::TEXTURE,
        color_arg2: TextureArg::DIFFUSE,
        alpha_op: TextureOp::SelectArg1,
        alpha_arg1: TextureArg::TEXTURE,
        alpha_arg2: TextureArg::DIFFUSE,
    };
    /// Stage 1 as the client leaves it.
    pub const DISABLED: Self = Self {
        color_op: TextureOp::Disable,
        color_arg1: TextureArg::TEXTURE,
        color_arg2: TextureArg::CURRENT,
        alpha_op: TextureOp::Disable,
        alpha_arg1: TextureArg::TEXTURE,
        alpha_arg2: TextureArg::CURRENT,
    };
    /// Colour and alpha both `SELECTARG1(TEXTURE)`: the texel unchanged.
    pub const REPLACE: Self = Self {
        color_op: TextureOp::SelectArg1,
        alpha_op: TextureOp::SelectArg1,
        ..Self::MODULATE
    };
}

impl Default for TextureStage {
    fn default() -> Self {
        Self::MODULATE
    }
}

/// `D3DTEXF_*` min/mag filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    Point,
    #[default]
    Linear,
}

/// `D3DSAMP_MIPFILTER`. `None` samples level 0 only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MipFilter {
    None,
    Point,
    #[default]
    Linear,
}

/// `D3DTADDRESS_*`. `Border` is not available on WebGPU/GLES-class limits and
/// is clamped; `Mirror` is an extension (the GL backend repeats instead).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Address {
    #[default]
    Wrap,
    Mirror,
    Clamp,
}

/// `D3DSAMP_*` for one stage. The default is the client's `SetDefaultState`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerState {
    pub min: Filter,
    pub mag: Filter,
    pub mip: MipFilter,
    pub address_u: Address,
    pub address_v: Address,
}

impl SamplerState {
    pub const NEAREST: Self = Self {
        min: Filter::Point,
        mag: Filter::Point,
        mip: MipFilter::Point,
        address_u: Address::Wrap,
        address_v: Address::Wrap,
    };

    pub const fn clamped(mut self) -> Self {
        self.address_u = Address::Clamp;
        self.address_v = Address::Clamp;
        self
    }

    /// The GL backend lets the mip filter follow the min filter; here the mip
    /// filter is explicit, `Point` min with a `Linear` mip is allowed.
    pub fn descriptor(self) -> ImageSamplerDescriptor {
        let filter = |f: Filter| match f {
            Filter::Point => ImageFilterMode::Nearest,
            Filter::Linear => ImageFilterMode::Linear,
        };
        let address = |a: Address| match a {
            Address::Wrap => ImageAddressMode::Repeat,
            Address::Mirror => ImageAddressMode::MirrorRepeat,
            Address::Clamp => ImageAddressMode::ClampToEdge,
        };
        ImageSamplerDescriptor {
            address_mode_u: address(self.address_u),
            address_mode_v: address(self.address_v),
            address_mode_w: ImageAddressMode::ClampToEdge,
            mag_filter: filter(self.mag),
            min_filter: filter(self.min),
            mipmap_filter: match self.mip {
                MipFilter::Linear => ImageFilterMode::Linear,
                MipFilter::None | MipFilter::Point => ImageFilterMode::Nearest,
            },
            lod_max_clamp: if self.mip == MipFilter::None { 0.0 } else { 32.0 },
            ..Default::default()
        }
    }
}

/// Fixed-function lighting, an extension beyond the GL reference backend
/// (which ignores `SetLight`/`LightEnable` and never enables `GL_LIGHTING`).
/// One directional light plus ambient, evaluated per vertex (Gouraud) with
/// the D3D9 equation; the material colours replace vertex colours
/// (`D3DMCS_MATERIAL`, as `CStateManager::SetDefaultState` sets).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lighting {
    /// World-space direction the light travels (`D3DLIGHT9::Direction`).
    pub direction: bevy::math::Vec3,
    pub light_diffuse: Vec4,
    pub light_ambient: Vec4,
    /// `D3DRS_AMBIENT`.
    pub global_ambient: Vec4,
    pub material_ambient: Vec4,
    pub material_emissive: Vec4,
}

impl Lighting {
    pub fn directional(direction: bevy::math::Vec3, diffuse: Vec4, ambient: Vec4) -> Self {
        Self {
            direction,
            light_diffuse: diffuse,
            light_ambient: ambient,
            global_ambient: Vec4::ZERO,
            material_ambient: Vec4::ONE,
            material_emissive: Vec4::ZERO,
        }
    }
}

/// The discrete state that needs a different render pipeline. It is the
/// material's specialisation key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PipelineKey {
    pub blend: Option<Blend>,
    pub cull: CullMode,
    pub depth: DepthState,
}

/// The GPU pipeline state a [`PipelineKey`] maps to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipelineStates {
    pub blend: Option<BlendState>,
    pub cull_mode: Option<Face>,
    pub depth_write: bool,
    pub depth_compare: CompareFunction,
}

impl PipelineKey {
    pub fn states(self) -> PipelineStates {
        PipelineStates {
            blend: self.blend.map(Blend::to_wgpu),
            cull_mode: self.cull.to_wgpu(),
            depth_write: self.depth.writes(),
            depth_compare: self.depth.compare(),
        }
    }
}

/// Converts a `D3DCOLOR` (`0xAARRGGBB`) to the gamma-space vector the shader
/// uses; no sRGB conversion, matching the original.
pub fn d3d_color(argb: u32) -> Vec4 {
    let [b, g, r, a] = argb.to_le_bytes();
    rgba8(r, g, b, a)
}

/// Gamma-space colour from 8-bit channels (`value / 255`, no sRGB decode).
pub fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Vec4 {
    Vec4::new(r as f32, g as f32, b as f32, a as f32) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_funcs_flip_for_reverse_z() {
        assert_eq!(CompareFunc::Less.to_reverse_z_wgpu(), CompareFunction::Greater);
        assert_eq!(CompareFunc::LessEqual.to_reverse_z_wgpu(), CompareFunction::GreaterEqual);
        assert_eq!(CompareFunc::Greater.to_reverse_z_wgpu(), CompareFunction::Less);
        assert_eq!(CompareFunc::GreaterEqual.to_reverse_z_wgpu(), CompareFunction::LessEqual);
        assert_eq!(CompareFunc::Equal.to_reverse_z_wgpu(), CompareFunction::Equal);
        assert_eq!(CompareFunc::NotEqual.to_reverse_z_wgpu(), CompareFunction::NotEqual);
        assert_eq!(CompareFunc::Never.to_reverse_z_wgpu(), CompareFunction::Never);
        assert_eq!(CompareFunc::Always.to_reverse_z_wgpu(), CompareFunction::Always);
    }

    #[test]
    fn disabled_depth_test_never_writes() {
        let state = DepthState { test: false, write: true, func: CompareFunc::Less };
        assert!(!state.writes());
        assert_eq!(state.compare(), CompareFunction::Always);
        let default = DepthState::default();
        assert!(default.writes());
        assert_eq!(default.compare(), CompareFunction::GreaterEqual);
    }

    #[test]
    fn cull_modes_follow_screen_winding() {
        assert_eq!(CullMode::None.to_wgpu(), None);
        assert_eq!(CullMode::Cw.to_wgpu(), Some(Face::Back));
        assert_eq!(CullMode::Ccw.to_wgpu(), Some(Face::Front));
        assert_eq!(CullMode::default(), CullMode::Cw);
    }

    #[test]
    fn blend_maps_to_wgpu_with_same_factors_for_alpha() {
        let state = Blend::ALPHA.to_wgpu();
        assert_eq!(state.color, state.alpha);
        assert_eq!(state.color.src_factor, WgpuBlendFactor::SrcAlpha);
        assert_eq!(state.color.dst_factor, WgpuBlendFactor::OneMinusSrcAlpha);
        assert_eq!(state.color.operation, BlendOperation::Add);
        let additive = Blend::ADDITIVE.to_wgpu();
        assert_eq!(additive.color.dst_factor, WgpuBlendFactor::One);
        let sub = Blend { src: BlendFactor::One, dst: BlendFactor::One, op: BlendOp::RevSubtract }.to_wgpu();
        assert_eq!(sub.color.operation, BlendOperation::ReverseSubtract);
    }

    #[test]
    fn pipeline_key_maps_every_discrete_state() {
        let key = PipelineKey {
            blend: Some(Blend::ALPHA),
            cull: CullMode::Ccw,
            depth: DepthState { test: true, write: false, func: CompareFunc::Less },
        };
        let states = key.states();
        assert_eq!(states.blend, Some(Blend::ALPHA.to_wgpu()));
        assert_eq!(states.cull_mode, Some(Face::Front));
        assert!(!states.depth_write);
        assert_eq!(states.depth_compare, CompareFunction::Greater);

        let opaque = PipelineKey { blend: None, cull: CullMode::None, depth: DepthState::DISABLED }.states();
        assert_eq!(opaque.blend, None);
        assert_eq!(opaque.cull_mode, None);
        assert!(!opaque.depth_write);
        assert_eq!(opaque.depth_compare, CompareFunction::Always);
    }

    #[test]
    fn distinct_pipeline_states_have_distinct_keys() {
        use std::collections::HashSet;
        let base = PipelineKey { blend: None, cull: CullMode::None, depth: DepthState::default() };
        let mut set = HashSet::new();
        set.insert(base);
        set.insert(PipelineKey { blend: Some(Blend::ALPHA), ..base });
        set.insert(PipelineKey { cull: CullMode::Cw, ..base });
        set.insert(PipelineKey { depth: DepthState::DISABLED, ..base });
        set.insert(base);
        assert_eq!(set.len(), 4);
    }

    #[test]
    fn sampler_state_maps_filters_and_addressing() {
        let d = SamplerState::NEAREST.descriptor();
        assert_eq!(d.mag_filter, ImageFilterMode::Nearest);
        assert_eq!(d.min_filter, ImageFilterMode::Nearest);
        assert_eq!(d.address_mode_u, ImageAddressMode::Repeat);
        let d = SamplerState { address_u: Address::Mirror, address_v: Address::Clamp, ..Default::default() }.descriptor();
        assert_eq!(d.address_mode_u, ImageAddressMode::MirrorRepeat);
        assert_eq!(d.address_mode_v, ImageAddressMode::ClampToEdge);
        assert_eq!(d.mag_filter, ImageFilterMode::Linear);
        let none = SamplerState { mip: MipFilter::None, ..Default::default() }.descriptor();
        assert_eq!(none.lod_max_clamp, 0.0);
    }

    #[test]
    fn texture_args_pack_for_the_shader() {
        assert_eq!(TextureArg::TEXTURE.pack(), 2);
        assert_eq!(TextureArg::DIFFUSE.pack(), 0);
        assert_eq!(TextureArg::TFACTOR.complement().pack(), 3 | 0x100);
        assert_eq!(TextureArg::TEXTURE.alpha_replicate().pack(), 2 | 0x200);
    }

    #[test]
    fn d3d_colors_are_not_srgb_converted() {
        let c = d3d_color(0x80FF8000);
        assert!((c.w - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(c.x, 1.0);
        assert!((c.y - 128.0 / 255.0).abs() < 1e-6);
        assert_eq!(c.z, 0.0);
    }

    #[test]
    fn linear_fog_needs_end_past_start() {
        assert!(Fog::linear(Vec4::ONE, 3.0, 12.0).is_active());
        assert!(!Fog::linear(Vec4::ONE, 12.0, 12.0).is_active());
        assert!(Fog::exp2(Vec4::ONE, 0.1).is_active());
    }
}
