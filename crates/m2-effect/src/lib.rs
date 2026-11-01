//! Effect runtime: the simulation behind `neoeter/src/effect`, without
//! Bevy or any GPU.
//!
//! Load an effect with `m2_formats::effect::load_effect`, wrap it in an
//! `Arc`, create an [`EffectInstance`], step it with [`EffectInstance::update`]
//! (or [`EffectInstance::update_to`] with the client timer) and a shared
//! [`GlibcRandom`], then ask it what to draw: [`EffectInstance::render`] gives
//! particle quads (corners, colour, texture frame, blend state), mesh
//! draws (world matrix, vertices, colour) and [`EffectInstance::lights`] the
//! dynamic lights. Textures stay names.
//!
//! Everything is deterministic and bit-identical to the C++ for a given
//! seed and step sequence (see `tests/differential_effect_runtime.rs`).

pub mod decorator;
pub mod element;
pub mod instance;
pub mod light;
pub mod manager;
pub mod math;
pub mod mesh;
pub mod particle;
pub mod particle_system;
pub mod rng;

pub use element::{ElementState, FrameController};
pub use instance::{EffectDraw, EffectInstance, MeshSource, NoMeshes};
pub use light::{LightInstance, LightState};
pub use manager::EffectManager;
pub use math::{Mat4, Quat, Vec3};
pub use mesh::{EffectMeshInstance, MeshDraw};
pub use particle::{Camera, Particle};
pub use particle_system::{ParticleQuad, ParticleSystemDraw, ParticleSystemInstance};
pub use rng::GlibcRandom;
