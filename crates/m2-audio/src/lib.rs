//! The client's sound (`neoeter/src/audio`) on `bevy_audio`.
//!
//! | C++ | here |
//! |-----|------|
//! | `CSoundManager` | [`SoundManager`] (system param) and [`SoundState`] (resource) |
//! | `CSoundManager2D` / `CSoundInstance2D` | [`Sound2d`] entities, at most [`MAX_2D_SOUNDS`] |
//! | `CSoundManager3D` / `CSoundInstance3D` | [`Sound3d`] entities, at most [`MAX_3D_SOUNDS`], heard by the [`SoundListener`] |
//! | `CSoundManagerStream` / `CSoundInstanceStream` | [`Music`] entities, one per slot of [`MusicSlots`] |
//! | `CSoundData`, `CSoundBase` (file cache, Miles callbacks) | [`M2Sound`] assets through the `m2://` source |
//! | `NSound` (`Type.cpp`, `.mss` scripts) | `m2_formats::race::sound` (already ported) |
//! | `MSSCompat.h` (Miles on miniaudio) | `bevy_audio`/rodio; [`falloff_compensation`] keeps its distance curve |
//!
//! # Setup
//!
//! ```no_run
//! use bevy::prelude::*;
//! use m2_assets::M2AssetsPlugin;
//! use m2_audio::{AudioConfig, M2AudioPlugin, SoundManager};
//!
//! fn click(mut sounds: SoundManager) {
//!     sounds.play_2d("sound/ui/click.wav");
//! }
//!
//! App::new()
//!     .add_plugins(M2AssetsPlugin::new("bin"))
//!     .add_plugins(DefaultPlugins)
//!     // After DefaultPlugins: it needs AssetPlugin and AudioPlugin.
//!     .add_plugins(M2AudioPlugin::new(AudioConfig::default()))
//!     .add_systems(Startup, click)
//!     .run();
//! ```
//!
//! The game moves the [`SoundListener`] entity to the camera's centre each
//! frame (`CPythonApplication` calls `SetPosition`/`SetDirection` from its
//! camera update).
//!
//! # Differences from C++
//!
//! - The music fades step by `speed` per 60 FPS frame of elapsed time instead
//!   of per rendered frame, so they last the same on any frame rate.
//! - Sounds load asynchronously: a sound starts once its file is in (C++
//!   blocks on the first play), and a missing file frees its instance when the
//!   load fails instead of when it is asked for.
//! - `Pause`/`Resume` and velocities (Doppler) are not used by the client and
//!   are not ported; control a playing sound through its `AudioSink`.

pub mod manager;
pub mod music;
pub mod source;
pub mod volume;

use bevy::audio::{AddAudioSource, SpatialListener};
use bevy::prelude::*;
use bevy::transform::TransformSystems;

pub use manager::{
    MAX_2D_SOUNDS, MAX_3D_SOUNDS, Music, Sound2d, Sound3d, SoundListener, SoundManager, SoundState, SoundVolume,
    falloff_compensation,
};
pub use music::{MusicSlots, MusicState};
pub use source::{M2Sound, M2SoundLoader};
pub use volume::{AudioConfig, Volumes};

/// Registers [`M2Sound`], the sound systems and the [`SoundListener`]. Add it
/// after `DefaultPlugins`.
pub struct M2AudioPlugin {
    config: AudioConfig,
}

impl M2AudioPlugin {
    /// Starts with `config`'s volumes (`CPythonApplication::Create`:
    /// `SetMusicVolume` and `SetSoundVolumeGrade` from `metin2.cfg`).
    pub fn new(config: AudioConfig) -> Self {
        Self { config }
    }
}

impl Plugin for M2AudioPlugin {
    fn build(&self, app: &mut App) {
        let mut state = SoundState::default();
        state.volumes.set_music(self.config.music_volume);
        state.volumes.set_sound(volume::grade_to_volume(self.config.sound_grade));

        app.add_audio_source::<M2Sound>()
            .register_asset_loader(M2SoundLoader)
            .insert_resource(state)
            .add_systems(Startup, spawn_listener)
            .add_systems(First, manager::reset_frame_counts)
            .add_systems(
                PostUpdate,
                (manager::update_music, manager::replay_finished, manager::drop_unplayable, manager::apply_falloff)
                    .after(TransformSystems::Propagate),
            );
    }
}

fn spawn_listener(mut commands: Commands) {
    commands.spawn((Name::new("sound listener"), SoundListener, SpatialListener::new(manager::EAR_GAP), Transform::default()));
}
