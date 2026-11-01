//! The in-game scene: the player standing in the map the server put them in,
//! with a third-person camera, click/keyboard movement and attacks.
//!
//! `introloading.py` warps by global position (`net.Warp(self.playerX,
//! self.playerY)`): `AtlasInfo.txt` names the map that holds it, the map loads
//! off the main thread ([`crate::world::prepare_map_at`]) and the character
//! spawns at its local position on the terrain height, watched by a
//! third-person camera.
//!
//! Movement follows `CPythonPlayer`/`CPythonNetworkStream`: a left click picks
//! the ground under the cursor and runs there
//! (`NEW_MoveToDestPixelPositionDirection`), `WASD`/arrows move relative to the
//! camera, and the client reports its position every 300 ms with `FUNC_MOVE`
//! and once with `FUNC_WAIT` when it stops
//! (`CPythonPlayerEventHandler::OnMoving`/`OnStop`). Space swings at the nearest
//! actor (`SendAttackPacket`); the camera orbits with a right-drag and zooms with
//! the wheel (`CCamera`).
//!
//! Actors other than the player are tracked from `ActorAppend`/`Move`/`Sync`/
//! `Remove` but not drawn yet; the next milestone draws them with their races.

use std::collections::HashMap;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy::window::PrimaryWindow;
use m2_assets::M2Packs;
use m2_net::phase::NetEvent;

use crate::SceneCamera;
use crate::net::{Net, NetMessage};
use crate::render::character::{Character, Look, Motion};
use crate::render::{environment_fog, soften_fog};
use crate::world::{MapScene, WarpedMap, prepare_map_at};

/// `LocaleService_GetLocalePath`'s default (`Locale.cpp`); used for
/// `AtlasInfo.txt` and `item_proto`.
pub const LOCALE_PATH: &str = "locale/en";

/// `CInstanceBase::FUNC_WAIT` / `FUNC_MOVE`.
pub const FUNC_WAIT: u8 = 0;
pub const FUNC_MOVE: u8 = 1;

/// `CTerrainImpl::TERRAIN_XSIZE`.
const TERRAIN_WORLD: f32 = 25600.0;

/// Camera limits and the start view (`CCamera`).
const CAMERA_MIN_DISTANCE: f32 = 150.0;
const CAMERA_MAX_DISTANCE: f32 = 1600.0;
const CAMERA_START_DISTANCE: f32 = 420.0;
const CAMERA_PITCH_MIN: f32 = -0.1;
const CAMERA_PITCH_MAX: f32 = 1.1;
const CAMERA_TARGET_UP: f32 = 85.0;

/// Run speed in world units (cm) per second (the client's `GetRunSpeed`).
const RUN_SPEED: f32 = 500.0;
/// `CPythonPlayerEventHandler::OnMoving`: a position packet every 300 ms.
const MOVE_PACKET_SECONDS: f32 = 0.3;
const ATTACK_COOLDOWN: f32 = 0.9;
/// `c_fDefaultRotationSpeed` (`InstanceBase.cpp`): degrees per second. A
/// correction wider than 45 degrees uses the full speed, a smaller one 5/12.
const MAX_ROTATION_SPEED: f32 = 1200.0;
/// Melee reach and how far away an actor is offered as a target.
pub const ATTACK_RANGE: f32 = 320.0;
const TARGET_RANGE: f32 = 1200.0;
/// How long the scene waits for placed objects before the unattended run calls
/// it loaded (buildings and trees load asynchronously).
const OBJECTS_WAIT: f32 = 8.0;
/// How long a `--demo` run lasts (move, attack, stand).
const DEMO_SECONDS: f32 = 4.9;

/// Enter the world at global position `(x, y)` with character `look`.
#[derive(Message, Clone, Copy, Debug)]
pub struct EnterWorld {
    pub x: i32,
    pub y: i32,
    pub look: Look,
    /// The main actor's VID, so it is not also tracked as another actor.
    pub vid: u32,
    /// Unattended movement/attack check: run, swing once, then stop.
    pub demo: bool,
}

/// Another actor the server told us about (`CPythonCharacterManager`).
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub name: String,
    pub race: u16,
    /// `CActorInstance::GetKind` (1 = NPC, 6 = player).
    pub kind: u8,
    pub level: u32,
    /// Global position.
    pub x: i32,
    pub y: i32,
}

/// The actors other than the main character, by VID.
#[derive(Resource, Default)]
pub struct Actors {
    pub by_vid: HashMap<u32, Actor>,
}

/// Set by the UI: when it wants the pointer or the keyboard, the game ignores
/// them (an egui window is open under the cursor).
#[derive(Resource, Default, Clone, Copy)]
pub struct UiFocus {
    pub pointer: bool,
    pub keyboard: bool,
}

/// What the character does now (`CInstanceBase::FUNC_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveState {
    Idle,
    Moving,
    Attacking,
}

/// The in-game controls and their state.
#[derive(Resource)]
pub struct PlayerControl {
    /// Camera orbit around the player.
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// The rendered rotation, in degrees (`0` faces north).
    pub facing: f32,
    /// The rotation the character turns towards (`GetAdvancingRotation`).
    pub advancing: f32,
    /// `m_fRotSpd`, degrees per second.
    rotation_speed: f32,
    pub state: MoveState,
    /// The actor space would hit, if any.
    pub target: Option<u32>,
    /// How far away that actor is, in world units.
    pub target_distance: Option<f32>,
    /// The ground point to run to, in map coordinates.
    pub destination: Option<Vec2>,
}

impl Default for PlayerControl {
    fn default() -> Self {
        PlayerControl {
            yaw: 0.0,
            pitch: 0.38,
            distance: CAMERA_START_DISTANCE,
            facing: 0.0,
            advancing: 0.0,
            rotation_speed: MAX_ROTATION_SPEED * 5.0 / 12.0,
            state: MoveState::Idle,
            target: None,
            target_distance: None,
            destination: None,
        }
    }
}

enum State {
    Loading {
        task: Task<Result<WarpedMap, String>>,
        look: Look,
        vid: u32,
        demo: bool,
    },
    Ready {
        scene: Box<MapScene>,
        character: Box<Character>,
        spawned: f32,
        demo: bool,
        demo_started: bool,
        demo_attacked: bool,
    },
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
    /// The map's origin in global coordinates.
    pub base: (u32, u32),
    /// The main actor's VID.
    pub main_vid: u32,
    last_pointer: Option<Vec2>,
    next_move_packet: f32,
    attack_ready: f32,
}

impl GameWorld {
    /// The map and the character are spawned.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, Some(State::Ready { .. }))
    }

    /// The map and the character are spawned and the placed objects had their
    /// time to load (or there are none left). A `--demo` run also waits for
    /// its scripted movement and attack.
    pub fn is_loaded(&self, now: f32) -> bool {
        match &self.state {
            Some(State::Ready { scene, spawned, demo, .. }) => {
                (!demo || now - *spawned >= DEMO_SECONDS)
                    && (scene.pending_objects() == 0 || now - *spawned >= OBJECTS_WAIT)
            }
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

    fn scene(&self) -> Option<&MapScene> {
        match &self.state {
            Some(State::Ready { scene, .. }) => Some(scene),
            _ => None,
        }
    }

    /// Terrain height at a map-local position.
    pub fn terrain_height(&self, x: f32, y: f32) -> f32 {
        self.scene().map_or(0.0, |scene| scene.map.map.terrain_height(x, y))
    }

    /// The map's local extent in world units.
    pub fn extent(&self) -> Vec2 {
        self.scene().map_or(Vec2::ZERO, |scene| {
            let (x, y) = scene.map.map.terrain_count;
            Vec2::new(f32::from(x) * TERRAIN_WORLD, f32::from(y) * TERRAIN_WORLD)
        })
    }
}

pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameWorld>()
            .init_resource::<PlayerControl>()
            .init_resource::<Actors>()
            .init_resource::<UiFocus>()
            .add_message::<EnterWorld>()
            .add_systems(Update, (start_entering, track_actors, drive_world).chain());
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
    world.main_vid = request.vid;
    info!("entering the world at {},{}", request.x, request.y);
    let packs = packs.packs.clone();
    let (x, y) = (request.x, request.y);
    let task = AsyncComputeTaskPool::get().spawn(async move { prepare_map_at(&packs, LOCALE_PATH, x, y) });
    world.state = Some(State::Loading { task, look: request.look, vid: request.vid, demo: request.demo });
}

/// Follows the actors the server adds, moves and removes.
fn track_actors(mut messages: MessageReader<NetMessage>, mut actors: ResMut<Actors>) {
    for message in messages.read() {
        let NetMessage::Game(event) = message else { continue };
        match event {
            NetEvent::ActorAppend(actor) => {
                actors.by_vid.insert(
                    actor.vid,
                    Actor {
                        name: actor.name.clone().unwrap_or_default(),
                        race: actor.race,
                        kind: actor.kind,
                        level: actor.level,
                        x: actor.x,
                        y: actor.y,
                    },
                );
            }
            NetEvent::ActorMove(motion) => {
                if let Some(actor) = actors.by_vid.get_mut(&motion.vid) {
                    actor.x = motion.x;
                    actor.y = motion.y;
                }
            }
            NetEvent::ActorSync { vid, x, y } => {
                if let Some(actor) = actors.by_vid.get_mut(vid) {
                    actor.x = *x;
                    actor.y = *y;
                }
            }
            NetEvent::ActorRemove { vid } => {
                actors.by_vid.remove(vid);
            }
            _ => {}
        }
    }
}

/// Polls the map load, then handles input, movement, the network position
/// reports and the camera every frame.
fn drive_world(world: &mut World) {
    world.resource_scope(|world, mut game: Mut<GameWorld>| {
        if let Some(State::Loading { task, look, vid, demo }) = &mut game.state {
            let Some(result) = check_ready(task) else { return };
            let (look, vid, demo) = (*look, *vid, *demo);
            match result {
                Ok(warped) => {
                    game.local = Some((warped.local_x, warped.local_y));
                    game.map = Some(warped.name.clone());
                    game.base = (warped.base_x, warped.base_y);
                    game.main_vid = vid;
                    let fog = warped.prepared.map.environment.as_ref().and_then(environment_fog).map(soften_fog);
                    let map = std::sync::Arc::new(warped.prepared);
                    let scene = MapScene::spawn_with_fog(world, map, fog);
                    let height = scene.map.map.terrain_height(warped.local_x as f32, warped.local_y as f32);
                    let position = local_to_world(warped.local_x as f32, warped.local_y as f32, height);
                    info!(
                        "map {}: local position {},{} at height {}",
                        warped.name, warped.local_x, warped.local_y, height
                    );
                    let mut character = Character::spawn(world, look, Transform::from_translation(position), None);
                    character.play_motion(world, Motion::Wait, true);
                    let spawned = world.resource::<Time>().elapsed_secs();
                    game.state = Some(State::Ready {
                        scene: Box::new(scene),
                        character: Box::new(character),
                        spawned,
                        demo,
                        demo_started: false,
                        demo_attacked: false,
                    });
                }
                Err(error) => {
                    error!("in-game scene: {error}");
                    game.state = Some(State::Failed(error));
                }
            }
        }
        let game = &mut *game;
        let Some(State::Ready { scene, character, spawned, demo, demo_started, demo_attacked }) = game.state.as_mut()
        else {
            return;
        };

        let now = world.resource::<Time>().elapsed_secs();
        let delta = world.resource::<Time>().delta_secs().min(0.1);
        let focus = *world.resource::<UiFocus>();
        let keys = world.resource::<ButtonInput<KeyCode>>().clone();
        let buttons = world.resource::<ButtonInput<MouseButton>>().clone();
        let scroll = *world.resource::<AccumulatedMouseScroll>();
        let cursor = world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(world)
            .ok()
            .and_then(|window| window.cursor_position());
        let base = game.base;
        let main_vid = game.main_vid;
        let mut last_pointer = game.last_pointer;
        let mut attack_ready = game.attack_ready;
        let mut next_move_packet = game.next_move_packet;
        let (count_x, count_y) = scene.map.map.terrain_count;
        let extent = Vec2::new(f32::from(count_x) * TERRAIN_WORLD, f32::from(count_y) * TERRAIN_WORLD);

        let mut control = world.remove_resource::<PlayerControl>().unwrap();
        {
            let position = world.get::<Transform>(character.root).map_or(Vec3::ZERO, |t| t.translation);
            let mut local = Vec2::new(position.x, -position.z);

            // Camera: right-drag orbits, the wheel zooms.
            if buttons.pressed(MouseButton::Right)
                && let (Some(cursor), Some(previous)) = (cursor, last_pointer)
            {
                let drag = cursor - previous;
                control.yaw -= drag.x * 0.006;
                control.pitch = (control.pitch + drag.y * 0.005).clamp(CAMERA_PITCH_MIN, CAMERA_PITCH_MAX);
            }
            if !focus.pointer && scroll.delta.y != 0.0 {
                control.distance =
                    (control.distance - scroll.delta.y * 60.0).clamp(CAMERA_MIN_DISTANCE, CAMERA_MAX_DISTANCE);
            }
            last_pointer = cursor;

            // Left click: run to the ground under the cursor.
            if !focus.pointer
                && buttons.just_pressed(MouseButton::Left)
                && let Some(cursor) = cursor
            {
                let height = |x: f32, y: f32| scene.map.map.terrain_height(x, y);
                if let Some(point) = pick_terrain(world, cursor, height, extent) {
                    control.destination = Some(point);
                    info!("walk to {:.0},{:.0}", point.x, point.y);
                }
            }

            let target = nearest_actor(&world.resource::<Actors>().by_vid, base, local, main_vid, TARGET_RANGE);
            control.target = target.map(|(vid, _)| vid);
            control.target_distance = target.map(|(_, distance)| distance);

            // The demo run: walk to the nearest actor, then swing at it.
            if *demo {
                let elapsed = now - *spawned;
                if !*demo_started && elapsed > 0.5 {
                    *demo_started = true;
                    let destination = nearest_actor(&world.resource::<Actors>().by_vid, base, local, main_vid, 4000.0)
                        .and_then(|(vid, _)| {
                            let actor = world.resource::<Actors>().by_vid.get(&vid).cloned()?;
                            let actor_local =
                                Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32);
                            let towards = (local - actor_local).normalize_or_zero();
                            Some(actor_local + towards * 200.0)
                        });
                    control.destination = Some(destination.unwrap_or(local + Vec2::new(400.0, 900.0)));
                }
                if !*demo_attacked && elapsed > 4.5 {
                    *demo_attacked = true;
                    let victim = attack_victim(&control);
                    face_actor(world, &mut control, base, local, victim);
                    start_attack(world, &mut control, character, victim);
                }
            }

            let mut moved = false;
            let mut direction = None;
            if control.state != MoveState::Attacking {
                if let Some(destination) = control.destination {
                    let (next, arrived) = advance_towards(local, destination, RUN_SPEED * delta);
                    if (next - local).length() > f32::EPSILON {
                        direction = Some((next - local).normalize());
                        local = next;
                        moved = true;
                    }
                    if arrived {
                        info!("destination reached at {:.0},{:.0}", local.x, local.y);
                        control.destination = None;
                    }
                } else if !focus.keyboard {
                    let mut input = Vec2::ZERO;
                    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
                        input.y += 1.0;
                    }
                    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
                        input.y -= 1.0;
                    }
                    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
                        input.x += 1.0;
                    }
                    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
                        input.x -= 1.0;
                    }
                    if input != Vec2::ZERO {
                        let forward = Vec2::new(-control.yaw.sin(), control.yaw.cos());
                        let right = Vec2::new(control.yaw.cos(), control.yaw.sin());
                        let map_direction = (forward * input.y + right * input.x).normalize();
                        local = clamp_to_map(local + map_direction * RUN_SPEED * delta, extent);
                        direction = Some(map_direction);
                        moved = true;
                    }
                }
            }

            // Space: normal attack, with the nearest actor as the victim.
            if !focus.keyboard
                && keys.just_pressed(KeyCode::Space)
                && now >= attack_ready
                && control.state != MoveState::Attacking
            {
                control.destination = None;
                attack_ready = now + ATTACK_COOLDOWN;
                let victim = attack_victim(&control);
                face_actor(world, &mut control, base, local, victim);
                start_attack(world, &mut control, character, victim);
            }

            // Face where we run, keep the feet on the ground.
            if let Some(direction) = direction {
                let target = facing_degrees(direction);
                if signed_difference(target, control.advancing).abs() > 0.01 {
                    set_advancing_rotation(&mut control, target);
                }
            }
            // `CInstanceBase::Update`: walk the rendered rotation towards the
            // advancing one at `m_fRotSpd`, taking the short way round.
            if moved || control.state == MoveState::Attacking {
                turn_towards(&mut control, delta);
                if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                    transform.rotation = Quat::from_rotation_y(-control.facing.to_radians());
                }
            }
            let height = scene.map.map.terrain_height(local.x, local.y);
            if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                transform.translation = local_to_world(local.x, local.y, height);
            }
            game.local = Some((local.x.round() as i32, local.y.round() as i32));

            // Motions and position packets.
            match control.state {
                MoveState::Attacking => {
                    if character.motion_started() && character.motion_finished(world) {
                        control.state = MoveState::Idle;
                        character.play_motion(world, Motion::Wait, true);
                        send_stop(world, base, local, control.advancing);
                    }
                }
                _ if moved => {
                    if control.state != MoveState::Moving {
                        control.state = MoveState::Moving;
                        character.play_motion(world, Motion::Run, true);
                    }
                    if now >= next_move_packet {
                        next_move_packet = now + MOVE_PACKET_SECONDS;
                        send_move(world, base, local, control.advancing);
                    }
                }
                _ => {
                    if control.state != MoveState::Idle {
                        control.state = MoveState::Idle;
                        character.play_motion(world, Motion::Wait, true);
                        send_stop(world, base, local, control.advancing);
                    }
                }
            }

            place_camera(world, local_to_world(local.x, local.y, height), &control);
        }
        world.insert_resource(control);
        game.last_pointer = last_pointer;
        game.attack_ready = attack_ready;
        game.next_move_packet = next_move_packet;
        scene.update(world);
        character.update(world);
    });
}

/// The VID an attack goes to: the target when it is within reach, else 0 (a
/// swing at nothing; the C++ client only sends a hit when it connects).
fn attack_victim(control: &PlayerControl) -> u32 {
    match (control.target, control.target_distance) {
        (Some(vid), Some(distance)) if distance <= ATTACK_RANGE => vid,
        _ => 0,
    }
}

/// Turns the character towards the actor it attacks (`CActorInstance::SetAdvancingRotation`).
fn face_actor(world: &mut World, control: &mut PlayerControl, base: (u32, u32), local: Vec2, victim: u32) {
    if victim == 0 {
        return;
    }
    let Some(actor) = world.resource::<Actors>().by_vid.get(&victim) else { return };
    let actor_local = Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32);
    let direction = actor_local - local;
    if direction == Vec2::ZERO {
        return;
    }
    set_advancing_rotation(control, facing_degrees(direction.normalize()));
}

/// Starts a normal attack: plays `attack.msa` and sends `CG_ATTACK`.
fn start_attack(world: &mut World, control: &mut PlayerControl, character: &mut Character, victim: u32) {
    control.state = MoveState::Attacking;
    control.destination = None;
    character.play_motion(world, Motion::Attack, false);
    if let Some(mut net) = world.get_resource_mut::<Net>() {
        net.send_attack(victim);
    }
    info!("attack: victim {victim}, facing {:.0}", control.advancing);
}

fn send_move(world: &mut World, base: (u32, u32), local: Vec2, facing: f32) {
    let (x, y) = global_of(base, local);
    if let Some(mut net) = world.get_resource_mut::<Net>() {
        net.send_move(FUNC_MOVE, 0, facing, x, y);
    }
}

fn send_stop(world: &mut World, base: (u32, u32), local: Vec2, facing: f32) {
    let (x, y) = global_of(base, local);
    if let Some(mut net) = world.get_resource_mut::<Net>() {
        net.send_move(FUNC_WAIT, 0, facing, x, y);
    }
}

/// `__LocalPositionToGlobalPosition`.
fn global_of(base: (u32, u32), local: Vec2) -> (i32, i32) {
    (local.x.round() as i32 + base.0 as i32, local.y.round() as i32 + base.1 as i32)
}

/// The actor nearest to `position` within `range`, excluding the main actor.
fn nearest_actor(
    actors: &HashMap<u32, Actor>,
    base: (u32, u32),
    position: Vec2,
    main_vid: u32,
    range: f32,
) -> Option<(u32, f32)> {
    let (bx, by) = (base.0 as i32, base.1 as i32);
    actors
        .iter()
        .filter(|(vid, _)| **vid != main_vid)
        .map(|(vid, actor)| {
            let local = Vec2::new((actor.x - bx) as f32, (actor.y - by) as f32);
            (*vid, (local - position).length())
        })
        .filter(|(_, distance)| *distance <= range)
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
}

/// `NEW_Goto` step: the next position towards `destination` and whether it is reached.
fn advance_towards(position: Vec2, destination: Vec2, max_step: f32) -> (Vec2, bool) {
    let delta = destination - position;
    let distance = delta.length();
    if distance <= max_step { (destination, true) } else { (position + delta / distance * max_step, false) }
}

fn clamp_to_map(position: Vec2, extent: Vec2) -> Vec2 {
    if extent == Vec2::ZERO {
        position
    } else {
        Vec2::new(position.x.clamp(0.0, extent.x), position.y.clamp(0.0, extent.y))
    }
}

/// The client's advancing rotation: `0` faces north (map `+y`).
fn facing_degrees(direction: Vec2) -> f32 {
    f32::atan2(direction.x, direction.y).to_degrees().rem_euclid(360.0)
}

/// The shortest signed turn from `current` to `target`, in `-180..180`.
fn signed_difference(target: f32, current: f32) -> f32 {
    (target - current + 180.0).rem_euclid(360.0) - 180.0
}

/// `GetDegreeDifference`: the shortest angle between two headings, `0..180`.
fn degree_difference(a: f32, b: f32) -> f32 {
    signed_difference(a, b).abs()
}

/// `CInstanceBase::SetAdvancingRotation`: the target heading and the turn
/// speed for it. A correction wider than 45 degrees uses the full speed, a
/// smaller one 5/12 of it.
fn set_advancing_rotation(control: &mut PlayerControl, target: f32) {
    control.rotation_speed = if degree_difference(control.facing, target) > 45.0 {
        MAX_ROTATION_SPEED
    } else {
        MAX_ROTATION_SPEED * 5.0 / 12.0
    };
    control.advancing = target.rem_euclid(360.0);
}

/// `CInstanceBase::Update`'s turn while walking: `facing` moves towards
/// `advancing` at `rotation_speed` degrees per second, the short way round.
fn turn_towards(control: &mut PlayerControl, delta: f32) {
    let difference = signed_difference(control.advancing, control.facing);
    let step = control.rotation_speed * delta;
    if difference.abs() <= step {
        control.facing = control.advancing;
    } else {
        control.facing = (control.facing + step * difference.signum()).rem_euclid(360.0);
    }
}

/// The Bevy position of a map-local `(x, y)` at terrain `height`. The client's
/// in-world Y is the map's stored Y negated (`coords`: terrain vertices are
/// stored `[x, -y, h]`).
fn local_to_world(local_x: f32, local_y: f32, height: f32) -> Vec3 {
    Vec3::new(local_x, height, -local_y)
}

/// Marches the camera ray through the heightfield and returns the first ground
/// point it hits, in map coordinates (`CPythonPlayer::__OnClickGround`).
fn pick_terrain(world: &mut World, cursor: Vec2, height: impl Fn(f32, f32) -> f32, extent: Vec2) -> Option<Vec2> {
    let mut camera = world.query_filtered::<(&Camera, &GlobalTransform), With<SceneCamera>>();
    let (camera, transform) = camera.single(world).ok()?;
    let ray = camera.viewport_to_world(transform, cursor).ok()?;
    let (origin, direction) = (ray.origin, *ray.direction);
    let step = 20.0;
    let mut previous_t = 0.0;
    let mut t = 0.0;
    while t < 20_000.0 {
        t += step;
        let point = origin + direction * t;
        if point.y <= height(point.x, -point.z) {
            let mut low = previous_t;
            let mut high = t;
            for _ in 0..12 {
                let middle = (low + high) * 0.5;
                let sample = origin + direction * middle;
                if sample.y <= height(sample.x, -sample.z) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            let hit = origin + direction * high;
            return Some(clamp_to_map(Vec2::new(hit.x, -hit.z), extent));
        }
        previous_t = t;
    }
    None
}

/// Third-person camera: orbits the character at `yaw`/`pitch`/`distance`.
fn place_camera(world: &mut World, position: Vec3, control: &PlayerControl) {
    let (sin_yaw, cos_yaw) = control.yaw.sin_cos();
    let (sin_pitch, cos_pitch) = control.pitch.sin_cos();
    let offset = Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch) * control.distance;
    let target = position + Vec3::Y * CAMERA_TARGET_UP;
    let eye = target + offset;
    let mut camera = world.query_filtered::<&mut Transform, With<SceneCamera>>();
    if let Ok(mut transform) = camera.single_mut(world) {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_y_is_negated_in_world_space() {
        // b1: local y 62233 is south of the map origin, so -Z is north.
        assert_eq!(local_to_world(66382.0, 62233.0, 19242.91), Vec3::new(66382.0, 19242.91, -62233.0));
    }

    #[test]
    fn the_camera_orbits_behind_and_above() {
        let mut app = App::new();
        let camera = app.world_mut().spawn((SceneCamera, Transform::default())).id();
        let control = PlayerControl::default();
        place_camera(app.world_mut(), Vec3::new(100.0, 20.0, -300.0), &control);
        let transform = app.world().get::<Transform>(camera).unwrap();
        // Default yaw 0: south of the character (+Z), above it.
        assert!(transform.translation.z > -300.0);
        assert!(transform.translation.y > 20.0);
        assert!(transform.forward().z < 0.0);
        assert!(transform.forward().y < 0.0);
    }

    #[test]
    fn movement_steps_towards_the_destination() {
        let (next, arrived) = advance_towards(Vec2::new(0.0, 0.0), Vec2::new(0.0, 10.0), 4.0);
        assert_eq!(next, Vec2::new(0.0, 4.0));
        assert!(!arrived);
        let (next, arrived) = advance_towards(Vec2::new(0.0, 8.0), Vec2::new(0.0, 10.0), 4.0);
        assert_eq!(next, Vec2::new(0.0, 10.0));
        assert!(arrived);
    }

    #[test]
    fn facing_follows_the_map_direction() {
        // North (+y) is 0 degrees, east (+x) is 90, south is 180.
        assert_eq!(facing_degrees(Vec2::new(0.0, 1.0)), 0.0);
        assert!((facing_degrees(Vec2::new(1.0, 0.0)) - 90.0).abs() < 1e-3);
        assert!((facing_degrees(Vec2::new(0.0, -1.0)) - 180.0).abs() < 1e-3);
    }

    #[test]
    fn the_character_turns_at_the_client_rotation_speed() {
        let mut control = PlayerControl::default();
        // 90 - 0 > 45: full 1200 deg/s, 0.05 s is 60 degrees.
        set_advancing_rotation(&mut control, 90.0);
        assert_eq!(control.rotation_speed, MAX_ROTATION_SPEED);
        turn_towards(&mut control, 0.05);
        assert!((control.facing - 60.0).abs() < 1e-3, "{}", control.facing);

        // A small correction turns at 5/12 of the speed and stops on the target.
        control.facing = 0.0;
        control.advancing = 0.0;
        set_advancing_rotation(&mut control, 20.0);
        assert_eq!(control.rotation_speed, MAX_ROTATION_SPEED * 5.0 / 12.0);
        turn_towards(&mut control, 0.1);
        assert_eq!(control.facing, 20.0);
    }

    #[test]
    fn turning_takes_the_short_way_round_the_compass() {
        let mut control =
            PlayerControl { facing: 350.0, advancing: 10.0, rotation_speed: MAX_ROTATION_SPEED, ..Default::default() };
        turn_towards(&mut control, 0.01); // 12 degrees, clockwise through north
        assert!((control.facing - 2.0).abs() < 1e-3, "{}", control.facing);
    }

    #[test]
    fn the_nearest_actor_inside_the_range_is_the_target() {
        let mut actors = HashMap::new();
        for (vid, x, y) in [(10, 100, 100), (11, 130, 100), (12, 2000, 2000)] {
            actors.insert(vid, Actor { name: String::new(), race: 0, kind: 1, level: 0, x, y });
        }
        let target = nearest_actor(&actors, (0, 0), Vec2::ZERO, 11, 500.0);
        assert_eq!(target.map(|(vid, _)| vid), Some(10));
        // The main actor is never its own target.
        let target = nearest_actor(&actors, (0, 0), Vec2::ZERO, 10, 500.0);
        assert_eq!(target.map(|(vid, _)| vid), Some(11));
    }
}
