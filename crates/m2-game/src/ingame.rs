//! The in-game scene: the player standing in the map the server put them in.
//!
//! `introloading.py` warps by global position (`net.Warp(self.playerX,
//! self.playerY)`): `AtlasInfo.txt` names the map that holds it, the map loads
//! off the main thread ([`crate::world::prepare_map_at`]) and the character
//! spawns at its local position on the terrain height, watched by a
//! third-person camera.
//!
//! The map is spawned whole (LOD 0, no streaming); actors, movement and the
//! real motion table belong to the next milestone.

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use m2_assets::M2Packs;

use crate::SceneCamera;
use crate::render::character::{Character, IntroMotion, Look};
use crate::world::{MapScene, WarpedMap, prepare_map_at};

/// `LocaleService_GetLocalePath`'s default (`Locale.cpp`); used for
/// `AtlasInfo.txt` and `item_proto`.
pub const LOCALE_PATH: &str = "locale/en";

/// Camera distance behind the character and how far above it the camera and its
/// target sit (world units).
const CAMERA_BACK: f32 = 360.0;
const CAMERA_UP: f32 = 150.0;
const CAMERA_TARGET_UP: f32 = 85.0;
/// How long the scene waits for placed objects before the unattended run calls
/// it loaded (buildings and trees load asynchronously).
const OBJECTS_WAIT: f32 = 8.0;

/// Enter the world at global position `(x, y)` with character `look`. Sent by
/// the client when the server switches to the Game phase.
#[derive(Message, Clone, Copy, Debug)]
pub struct EnterWorld {
    pub x: i32,
    pub y: i32,
    pub look: Look,
}

enum State {
    Loading { task: Task<Result<WarpedMap, String>>, look: Look },
    Ready { scene: Box<MapScene>, character: Box<Character>, spawned: f32 },
    Failed(String),
}

/// The in-game scene. Absent until [`EnterWorld`] arrives.
#[derive(Resource, Default)]
pub struct GameWorld {
    state: Option<State>,
    /// The server position the scene was entered at, for the HUD.
    pub global: Option<(i32, i32)>,
    /// The local position inside the map.
    pub local: Option<(i32, i32)>,
    pub map: Option<String>,
}

impl GameWorld {
    /// The map and the character are spawned.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, Some(State::Ready { .. }))
    }

    /// The map and the character are spawned and the placed objects had their
    /// time to load (or there are none left).
    pub fn is_loaded(&self, now: f32) -> bool {
        match &self.state {
            Some(State::Ready { scene, spawned, .. }) => scene.pending_objects() == 0 || now - *spawned >= OBJECTS_WAIT,
            _ => false,
        }
    }

    /// Why the scene could not be built.
    pub fn error(&self) -> Option<&str> {
        match &self.state {
            Some(State::Failed(error)) => Some(error),
            _ => None,
        }
    }
}

pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameWorld>()
            .add_message::<EnterWorld>()
            .add_systems(Update, (start_entering, drive_world).chain());
    }
}

/// Starts loading the map for an [`EnterWorld`] request.
fn start_entering(mut messages: MessageReader<EnterWorld>, mut world: ResMut<GameWorld>, packs: Option<Res<M2Packs>>) {
    let Some(request) = messages.read().last() else { return };
    if world.state.is_some() {
        return;
    }
    let Some(packs) = packs else {
        world.state = Some(State::Failed("no client data: cannot load the map".into()));
        return;
    };
    world.global = Some((request.x, request.y));
    info!("entering the world at {},{}", request.x, request.y);
    let packs = packs.packs.clone();
    let (x, y) = (request.x, request.y);
    let task = AsyncComputeTaskPool::get().spawn(async move { prepare_map_at(&packs, LOCALE_PATH, x, y) });
    world.state = Some(State::Loading { task, look: request.look });
}

/// Polls the map load and, once it is in, spawns the map, the character and the
/// camera; then runs both every frame.
fn drive_world(world: &mut World) {
    world.resource_scope(|world, mut game: Mut<GameWorld>| {
        if let Some(State::Loading { task, look }) = &mut game.state {
            let Some(result) = check_ready(task) else { return };
            let look = *look;
            match result {
                Ok(warped) => {
                    game.local = Some((warped.local_x, warped.local_y));
                    game.map = Some(warped.name.clone());
                    let map = std::sync::Arc::new(warped.prepared);
                    let scene = MapScene::spawn(world, map, true);
                    let height = scene.map.map.terrain_height(warped.local_x as f32, warped.local_y as f32);
                    // The client's in-world Y is the map's stored Y negated
                    // (`coords`: terrain vertices are `[x, -y, h]`).
                    let position = local_to_world(warped.local_x, warped.local_y, height);
                    info!(
                        "map {}: local position {},{} at height {}",
                        warped.name, warped.local_x, warped.local_y, height
                    );
                    let mut character = Character::spawn(world, look, Transform::from_translation(position), None);
                    if let Some(file) = IntroMotion::Wait.file(look.race) {
                        character.play(world, &file, true);
                    }
                    place_camera(world, position);
                    let spawned = world.resource::<Time>().elapsed_secs();
                    game.state = Some(State::Ready { scene: Box::new(scene), character: Box::new(character), spawned });
                }
                Err(error) => {
                    error!("in-game scene: {error}");
                    game.state = Some(State::Failed(error));
                }
            }
        }
        let Some(State::Ready { scene, character, .. }) = game.state.as_mut() else { return };
        scene.update(world);
        character.update(world);
    });
}

/// The Bevy position of a map-local `(x, y)` at terrain `height`. The client's
/// in-world Y is the map's stored Y negated (`coords`: terrain vertices are
/// stored `[x, -y, h]`).
fn local_to_world(local_x: i32, local_y: i32, height: f32) -> Vec3 {
    Vec3::new(local_x as f32, height, -(local_y as f32))
}

/// Third-person camera behind the character, which faces -Z.
fn place_camera(world: &mut World, position: Vec3) {
    let mut camera = world.query_filtered::<&mut Transform, With<SceneCamera>>();
    if let Ok(mut transform) = camera.single_mut(world) {
        *transform = Transform::from_xyz(position.x, position.y + CAMERA_UP, position.z + CAMERA_BACK)
            .looking_at(Vec3::new(position.x, position.y + CAMERA_TARGET_UP, position.z), Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_sits_behind_the_character() {
        let mut app = App::new();
        let camera = app.world_mut().spawn((SceneCamera, Transform::default())).id();
        place_camera(app.world_mut(), Vec3::new(100.0, 20.0, 300.0));
        let transform = app.world().get::<Transform>(camera).unwrap();
        assert_eq!(transform.translation, Vec3::new(100.0, 20.0 + CAMERA_UP, 300.0 + CAMERA_BACK));
        // Looking at the character, which is in front (-Z) and below.
        assert!(transform.forward().z < 0.0);
        assert!(transform.forward().y < 0.0);
    }

    #[test]
    fn map_y_is_negated_in_world_space() {
        // b1: local y 62233 is south of the map origin, so -Z is north.
        assert_eq!(local_to_world(66382, 62233, 19242.91), Vec3::new(66382.0, 19242.91, -62233.0));
    }
}
