//! Fixed-function (Direct3D 9 style) rendering for the Metin2 port.
//!
//! Bevy's `StandardMaterial` cannot reproduce the original client's
//! fixed-function pipeline. [`FixedFunctionMaterial`] does, through one WGSL
//! (WESL) shader driven by a small uniform plus a few pipeline-specialisation
//! keys. See the README for the supported state table and the conventions
//! mesh and image producers must follow.

mod material;
mod projection;
mod state;

use bevy::app::{App, Plugin};
use bevy::asset::embedded_asset;
use bevy::camera::ClearColorConfig;
use bevy::render::view::Msaa;
use bevy::color::{Color, LinearRgba};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::pbr::MaterialPlugin;
use bevy::prelude::Camera3d;

pub use material::{FixedFunctionMaterial, FixedFunctionUniform, SHADER_PATH};
pub use projection::FixedFunctionProjection;
pub use state::*;

/// Registers [`FixedFunctionMaterial`] and its embedded shader.
#[derive(Default)]
pub struct M2RenderPlugin;

impl Plugin for M2RenderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "fixed_function.wesl");
        app.add_plugins(MaterialPlugin::<FixedFunctionMaterial>::default());
    }
}

/// The components that make a Bevy 3D camera render in the original's
/// colour space: no tonemapping, no dithering, no MSAA. Spawn it together with
/// a [`FixedFunctionProjection`] and a render target in a non-sRGB format
/// (`Rgba8Unorm`), because the shader writes gamma-space values that must be
/// stored and blended unchanged.
pub fn gamma_space_camera() -> impl bevy::ecs::bundle::Bundle {
    (Camera3d::default(), Tonemapping::None, DebandDither::Disabled, Msaa::Off)
}

/// A clear colour that is written verbatim: gamma-space `0..=1` values, no
/// sRGB conversion (a plain `Color::srgb` would be decoded to linear).
pub fn clear_color(color: bevy::math::Vec4) -> ClearColorConfig {
    ClearColorConfig::Custom(Color::LinearRgba(LinearRgba::new(color.x, color.y, color.z, color.w)))
}
