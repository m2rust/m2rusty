//! The game-side Bevy library: what both the client and the asset viewer need
//! to put the game world on screen.
//!
//! - [`render`]: models, SpeedTrees, effects, terrain, sky and clouds drawn
//!   with the client's fixed-function render states (`m2-render`).
//! - [`net`]: the server connection ([`net::NetPlugin`]): login through the
//!   account server, then the game connection, as messages.
//! - [`world`]: an outdoor map loaded by [`m2_world::load_map`], prepared off
//!   the main thread ([`world::prepare_map`]) and spawned as one scene
//!   ([`world::MapScene`]).
//! - [`ingame`]: the in-game scene (map, player character, third-person
//!   camera) for the position the server warps to.
//! - [`audio`]: keeps the `m2-audio` sound listener on the camera centre.
//!
//! Everything spawned here hangs under a root entity the caller owns, so the
//! caller tags and despawns that root. The camera the scene is drawn for
//! carries [`SceneCamera`]: animated models are culled against it, clouds
//! follow it, effects billboard towards it and the sky is installed on it.

pub mod audio;
pub mod battle;
pub mod combat;
pub mod fx;
pub mod ground;
pub mod ingame;
pub mod net;
pub mod player;
pub mod render;
pub mod social;
pub mod world;

use bevy::prelude::*;

/// Marks the camera the world is drawn for. Exactly one should exist.
#[derive(Component, Default, Debug, Clone, Copy)]
pub struct SceneCamera;

/// Registers the game's materials and their per-frame systems.
pub struct M2GamePlugin;

impl Plugin for M2GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            render::terrain::TerrainPlugin,
            render::cloud::CloudPlugin,
            render::lighting::LightingPlugin,
            render::model::ModelCullPlugin,
            render::character::MotionBlendPlugin,
            ground::GroundItemPlugin,
            render::shadow::CharacterShadowPlugin,
            render::sun::SunPlugin,
            ingame::InGamePlugin,
            audio::GameAudioPlugin,
        ));
    }
}
