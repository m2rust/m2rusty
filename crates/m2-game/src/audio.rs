//! Where 3D sounds are heard from: `CPythonApplication::__UpdateCamera` gives
//! the sound manager the camera's centre (`SetPosition`) and its view and up
//! directions (`SetDirection`) every frame.

use bevy::prelude::*;
use m2_audio::SoundListener;

use crate::SceneCamera;
use crate::ingame::{GameControl, GameWorld, PlayerControl};

/// Keeps the [`SoundListener`] on the scene camera's centre.
pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, follow_camera.after(GameControl));
    }
}

/// In game the centre is the point the camera orbits, `distance` ahead of
/// the eye; before that (select screen) the eye itself.
fn follow_camera(
    camera: Query<&Transform, (With<SceneCamera>, Without<SoundListener>)>,
    control: Res<PlayerControl>,
    world: Res<GameWorld>,
    mut listener: Query<&mut Transform, With<SoundListener>>,
) {
    let Ok(eye) = camera.single() else { return };
    let centre = if world.is_ready() { eye.translation + eye.forward() * control.distance } else { eye.translation };
    for mut transform in &mut listener {
        *transform = Transform::from_translation(centre).with_rotation(eye.rotation);
    }
}
