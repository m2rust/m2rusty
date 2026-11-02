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
//! Actors other than the player are ECS entities tracked from
//! `ActorAppend`/`Move`/`Sync`/`Remove`. Their server identity and appearance
//! data stay on the entity so character rendering can consume them.

use std::collections::{HashMap, HashSet};

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy::window::PrimaryWindow;
use m2_assets::{M2Packs, paths};
use m2_formats::proto::parse_mob_proto;
use m2_net::phase::NetEvent;

use crate::SceneCamera;
use crate::combat::{self, ActorCombat, AttackContext, SkillCheck, SkillError, SkillExt};
use crate::net::{Net, NetMessage};
use crate::player::{ItemCatalog, PlayerState};
use crate::render::character::{ArmorShapes, Character, Look, MOUNT_HEIGHT, Motion};
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
    pub armor: u16,
    pub hair: u16,
    /// Global position.
    pub x: i32,
    pub y: i32,
    pub rotation: f32,
    /// `CharacterAdditionalInfo`/`CharacterUpdate` combat metadata.
    pub empire: u8,
    pub pk_mode: u8,
    pub guild_id: u32,
    pub alignment: i16,
    pub state_flags: u8,
    pub affect_flags: u64,
    pub mount_vnum: u32,
    move_from: Vec2,
    move_to: Vec2,
    move_elapsed: f32,
    move_duration: f32,
    moving: bool,
    rendered_moving: Option<bool>,
}

impl Actor {
    /// The actor as the combat predicates see it, in map-local coordinates
    /// (`base` is the map's global origin).
    pub fn combat(&self, vid: u32, base: (u32, u32)) -> crate::combat::ActorCombat {
        crate::combat::ActorCombat {
            vid,
            kind: self.kind,
            race: self.race,
            empire: self.empire,
            pk_mode: self.pk_mode,
            guild_id: self.guild_id,
            alignment: self.alignment,
            dead: self.state_flags & crate::combat::STATE_DEAD != 0,
            state_flags: self.state_flags,
            affect_flags: self.affect_flags,
            in_safe: false,
            x: self.x - base.0 as i32,
            y: self.y - base.1 as i32,
        }
    }
}

/// The actors other than the main character, by VID.
#[derive(Resource, Default)]
pub struct Actors {
    pub by_vid: HashMap<u32, Actor>,
    /// Bevy entity for each server actor. The main character is kept separate.
    pub entities: HashMap<u32, Entity>,
    /// Rendered character roots, keyed by server VID.
    characters: HashMap<u32, Character>,
    failed_visuals: HashSet<u32>,
}

/// The client mob_proto data needed to name and render a server NPC/monster.
#[derive(Resource, Default)]
pub struct MobCatalog {
    by_race: HashMap<u16, MobAppearance>,
}

#[derive(Clone, Debug)]
struct MobAppearance {
    name: String,
    level: u8,
    race_file: Option<String>,
    motions: ActorMotions,
}

#[derive(Clone, Debug, Default)]
struct ActorMotions {
    wait: Option<String>,
    walk: Option<String>,
    run: Option<String>,
}

/// Server identity and appearance metadata attached to every non-player actor.
#[derive(Component, Clone, Debug)]
pub struct ActorEntity {
    pub vid: u32,
    pub name: String,
    pub race: u16,
    /// Server actor kind (`1` NPC, `6` player; other values include monsters).
    pub kind: u8,
    pub level: u32,
    pub armor: u16,
    pub hair: u16,
}

/// Set by the UI: when it wants the pointer or the keyboard, the game ignores
/// them (an egui window is open under the cursor).
#[derive(Resource, Default, Clone, Copy)]
pub struct UiFocus {
    pub pointer: bool,
    pub keyboard: bool,
}

/// Native UI publishes input capture before player movement runs.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameControl;

/// What the character does now (`CInstanceBase::FUNC_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveState {
    Idle,
    Moving,
    Attacking,
    Casting,
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
    /// `m_dwcurComboIndex`: the combo step reached by the last attack.
    pub combo: u32,
    /// When the last combo step ended, for the input window.
    pub combo_time: f32,
    /// When the current attack/cast started (a stuck motion falls back).
    pub action_started: f32,
    /// `MODE_USE_SKILL`: (vnum, target VID, range) while running in range.
    pub reserved_skill: Option<(u32, u32, u32)>,
    /// `MODE_CLICK_ACTOR`: attack this VID once it is in reach.
    pub reserved_attack: Option<u32>,
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
            combo: 0,
            combo_time: 0.0,
            action_started: 0.0,
            reserved_skill: None,
            reserved_attack: None,
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
            .init_resource::<MobCatalog>()
            .init_resource::<ArmorShapes>()
            .init_resource::<UiFocus>()
            .add_message::<EnterWorld>()
            .add_message::<combat::UseSkillRequest>()
            .add_systems(Update, (start_entering, track_actors, sync_actor_entities, drive_world.in_set(GameControl)).chain());
    }
}

/// Starts loading the map for an [`EnterWorld`] request.
fn start_entering(
    mut commands: Commands,
    mut messages: MessageReader<EnterWorld>,
    mut world: ResMut<GameWorld>,
    packs: Option<Res<M2Packs>>,
    mut mobs: ResMut<MobCatalog>,
    mut armor_shapes: ResMut<ArmorShapes>,
    mut actors: ResMut<Actors>,
) {
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
    actors.by_vid.remove(&request.vid);
    actors.failed_visuals.remove(&request.vid);
    if let Some(character) = actors.characters.remove(&request.vid) {
        commands.entity(character.root).despawn();
    }
    if let Some(entity) = actors.entities.remove(&request.vid) {
        commands.entity(entity).despawn();
    }
    info!("entering the world at {},{}", request.x, request.y);
    let packs = packs.packs.clone();
    match ArmorShapes::load(&packs, LOCALE_PATH) {
        Ok(shapes) => *armor_shapes = shapes,
        Err(error) => warn!("could not load {LOCALE_PATH}/item_proto for actor armor: {error}"),
    }
    let mob_file = format!("{LOCALE_PATH}/mob_proto");
    match packs.get(mob_file.as_bytes()).map_err(|error| error.to_string()).and_then(|data| {
        parse_mob_proto(&data).map_err(|error| error.to_string())
    }) {
        Ok(tables) => {
            let mut catalog = MobCatalog::default();
            let directory_index = m2_assets::DirectoryIndex::build(&packs);
            let race_registrations = load_race_registrations(&packs, &directory_index);
            let registered_count = race_registrations.len();
            let unresolved_count = race_registrations.values().filter(|race| race.race_file.is_none()).count();
            catalog.by_race.extend(race_registrations);
            for table in tables {
                let race = table.vnum as u16;
                catalog
                    .by_race
                    .entry(race)
                    .and_modify(|mob| {
                        let localized = proto_text(&table.locale_name);
                        if !localized.is_empty() {
                            mob.name = localized;
                        }
                        if mob.level == 0 {
                            mob.level = table.level;
                        }
                    })
                    .or_insert_with(|| MobAppearance {
                        name: proto_text(&table.locale_name),
                        level: table.level,
                        race_file: None,
                        motions: ActorMotions::default(),
                    });
            }
            info!("loaded {registered_count} NPC race registrations; {unresolved_count} have no matching MSM asset");
            mobs.by_race = catalog.by_race;
        }
        Err(error) => warn!("could not load {mob_file}: {error}"),
    }
    let (x, y) = (request.x, request.y);
    let task = AsyncComputeTaskPool::get().spawn(async move { prepare_map_at(&packs, LOCALE_PATH, x, y) });
    world.state = Some(State::Loading { task, look: request.look, vid: request.vid, demo: request.demo });
}

fn proto_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_end_matches('\0').trim().to_owned()
}

/// Ports `playersettingmodule.__LoadGameNPC`: vnum/name and
/// `RegisterRaceSrcName` entries from `npclist.txt`, then resolves the same
/// race paths as `CRaceManager::__LoadRaceData`.
fn load_race_registrations(
    packs: &m2_formats::pack::PackManager,
    index: &m2_assets::DirectoryIndex,
) -> HashMap<u16, MobAppearance> {
    let Ok(data) = packs.get(b"npclist.txt") else {
        warn!("could not load npclist.txt: no NPC race registrations");
        return HashMap::new();
    };
    let mut names = HashMap::<u16, String>::new();
    let mut source_names = HashMap::<String, String>::new();
    for line in String::from_utf8_lossy(&data).lines() {
        let mut fields = line.split('\t');
        let Some(vnum) = fields.next().and_then(|value| value.trim().parse::<u32>().ok()) else {
            continue;
        };
        let Some(name) = fields.next().map(str::trim).filter(|name| !name.is_empty()) else { continue };
        if vnum == 0 {
            if let Some(source) = fields.next().map(str::trim).filter(|source| !source.is_empty()) {
                source_names.entry(name.to_owned()).or_insert_with(|| source.to_owned());
            }
        } else if vnum <= u16::MAX as u32 {
            names.entry(vnum as u16).or_insert_with(|| name.to_owned());
        }
    }
    names
        .into_iter()
        .map(|(race, name)| {
            let file = resolve_registered_race_file(index, race, &name, source_names.get(&name));
            let motions = file.as_deref().map_or_else(ActorMotions::default, |file| load_actor_motions(packs, file));
            (race, MobAppearance { name, level: 0, race_file: file, motions })
        })
        .collect()
}

/// Reads the same GENERAL motion aliases that `CRaceManager` registers from a
/// race's `motlist.txt`; many monsters use numeric filenames such as `00.msa`
/// and have no literal `wait.msa` or `run.msa`.
fn load_actor_motions(packs: &m2_formats::pack::PackManager, race_file: &str) -> ActorMotions {
    let folder = paths::parent(race_file);
    let mut race_data = m2_formats::race::RaceData::default();
    let motion_list = if race_data.load(packs, paths::virtual_name(race_file), |_| true) {
        proto_text(&race_data.motion_list_file_name)
    } else {
        "motlist.txt".to_owned()
    };
    let file = paths::join(folder, &motion_list);
    let Ok(data) = packs.get(paths::virtual_name(&file)) else { return ActorMotions::default() };
    let mut motions = ActorMotions::default();
    for line in String::from_utf8_lossy(&data).lines() {
        let mut fields = line.split_whitespace();
        if !fields.next().is_some_and(|mode| mode.eq_ignore_ascii_case("general")) {
            continue;
        }
        let (Some(kind), Some(file)) = (fields.next(), fields.next()) else { continue };
        let kind = kind.to_ascii_uppercase();
        let target = if kind == "WAIT" || (kind.starts_with("WAIT") && kind[4..].parse::<u8>().is_ok()) {
            &mut motions.wait
        } else if kind == "WALK" || (kind.starts_with("WALK") && kind[4..].parse::<u8>().is_ok()) {
            &mut motions.walk
        } else if kind == "RUN" || (kind.starts_with("RUN") && kind[3..].parse::<u8>().is_ok()) {
            &mut motions.run
        } else {
            continue;
        };
        target.get_or_insert_with(|| paths::join(folder, file));
    }
    motions
}

/// Mirrors the ordered roots and explicit `#folder/shape.msm` convention in
/// `CRaceManager`. Race source names come from zero-vnum `npclist.txt` rows.
fn resolve_registered_race_file(
    index: &m2_assets::DirectoryIndex,
    race: u16,
    name: &str,
    source: Option<&String>,
) -> Option<String> {
    if let Some(local) = name.strip_prefix('#') {
        let path = paths::canonical_asset_path(&format!("{local}shape.msm"));
        return asset_exists(index, &path).then_some(path);
    }
    let categories: &[&str] = if (14000..15000).contains(&race) || race == 20043 {
        &["guild", "npc", "npc2", "monster", "monster2"]
    } else if race >= 30000 {
        &["npc2", "npc", "monster", "monster2", "guild"]
    } else if race > 9000 {
        &["npc", "npc2", "monster", "monster2", "guild"]
    } else if matches!(race, 8507 | 8510) {
        &["monster2", "monster", "npc", "npc2", "guild"]
    } else if race > 8000 {
        &["monster", "monster2", "npc", "npc2", "guild"]
    } else if race > 2000 || (1400..=1700).contains(&race) {
        &["monster2", "monster", "npc", "npc2", "guild"]
    } else {
        &["monster", "monster2", "npc", "npc2", "guild"]
    };
    for category in categories {
        let source_name = source.map_or(name, String::as_str);
        let candidate = paths::canonical_asset_path(&format!("d:/ymir work/{category}/{source_name}/{name}.msm"));
        if asset_exists(index, &candidate) {
            return Some(candidate);
        }
    }
    None
}

fn asset_exists(index: &m2_assets::DirectoryIndex, path: &str) -> bool {
    let parent = paths::parent(path);
    let file = path.rsplit('/').next().unwrap_or(path);
    index
        .list(parent)
        .is_some_and(|mut entries| entries.any(|(name, is_dir)| !is_dir && name.eq_ignore_ascii_case(file)))
}

/// Follows the actors the server adds, moves and removes.
fn track_actors(
    mut commands: Commands,
    mut messages: MessageReader<NetMessage>,
    mut actors: ResMut<Actors>,
    game: Res<GameWorld>,
    mobs: Res<MobCatalog>,
) {
    for message in messages.read() {
        let NetMessage::Game(event) = message else { continue };
        match event {
            NetEvent::ActorAppend(actor) => {
                if actor.vid == game.main_vid {
                    continue;
                }
                if let Some(entity) = actors.entities.remove(&actor.vid) {
                    commands.entity(entity).despawn();
                }
                actors.characters.remove(&actor.vid);
                actors.failed_visuals.remove(&actor.vid);
                let name = actor
                    .name
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .or_else(|| mobs.by_race.get(&actor.race).map(|mob| mob.name.as_str()))
                    .unwrap_or("");
                let level = if actor.level == 0 {
                    mobs.by_race.get(&actor.race).map_or(0, |mob| u32::from(mob.level))
                } else {
                    actor.level
                };
                let entity = commands
                    .spawn((
                        Name::new(if name.is_empty() {
                            format!("actor {}", actor.vid)
                        } else {
                            name.to_owned()
                        }),
                        ActorEntity {
                            vid: actor.vid,
                            name: name.to_owned(),
                            race: actor.race,
                            kind: actor.kind,
                            level,
                            armor: actor.armor,
                            hair: actor.hair,
                        },
                        Transform::from_translation(Vec3::new(actor.x as f32, 0.0, -(actor.y as f32)))
                            .with_rotation(Quat::from_rotation_y(-actor.rotation.to_radians())),
                        Visibility::Visible,
                    ))
                    .id();
                actors.entities.insert(actor.vid, entity);
                actors.by_vid.insert(
                    actor.vid,
                    Actor {
                        name: name.to_owned(),
                        race: actor.race,
                        kind: actor.kind,
                        level,
                        x: actor.x,
                        y: actor.y,
                        rotation: actor.rotation,
                        armor: actor.armor,
                        hair: actor.hair,
                        empire: actor.empire,
                        pk_mode: actor.pk_mode,
                        guild_id: actor.guild_id,
                        alignment: actor.alignment,
                        state_flags: actor.state_flags,
                        affect_flags: actor.affect_flags,
                        mount_vnum: actor.mount_vnum,
                        move_from: Vec2::new(actor.x as f32, actor.y as f32),
                        move_to: Vec2::new(actor.x as f32, actor.y as f32),
                        move_elapsed: 0.0,
                        move_duration: 0.0,
                        moving: false,
                        rendered_moving: None,
                    },
                );
            }
            NetEvent::ActorMove(motion) => {
                if let Some(actor) = actors.by_vid.get_mut(&motion.vid) {
                    actor.move_from = Vec2::new(actor.x as f32, actor.y as f32);
                    actor.move_to = Vec2::new(motion.x as f32, motion.y as f32);
                    actor.move_elapsed = 0.0;
                    actor.move_duration = motion.duration as f32 / 1000.0;
                    actor.moving = motion.func != FUNC_WAIT;
                    if actor.move_duration == 0.0 {
                        actor.x = motion.x;
                        actor.y = motion.y;
                    }
                    actor.rotation = motion.rotation;
                }
            }
            NetEvent::ActorSync { vid, x, y } => {
                if let Some(actor) = actors.by_vid.get_mut(vid) {
                    actor.x = *x;
                    actor.y = *y;
                    actor.move_from = Vec2::new(*x as f32, *y as f32);
                    actor.move_to = actor.move_from;
                    actor.move_elapsed = actor.move_duration;
                }
            }
            NetEvent::ActorUpdate(update) => {
                let appearance_changed = if let Some(actor) = actors.by_vid.get_mut(&update.vid) {
                    let changed = actor.kind == m2_net::phase::ACTOR_TYPE_PC
                        && (actor.armor != update.armor || actor.hair != update.hair);
                    actor.armor = update.armor;
                    actor.hair = update.hair;
                    actor.guild_id = update.guild_id;
                    actor.alignment = update.alignment;
                    actor.pk_mode = update.pk_mode;
                    actor.state_flags = update.state_flags;
                    actor.affect_flags = update.affect_flags;
                    actor.mount_vnum = update.mount_vnum;
                    if changed {
                        actor.rendered_moving = None;
                    }
                    changed
                } else {
                    false
                };
                if appearance_changed {
                    if let Some(character) = actors.characters.remove(&update.vid) {
                        commands.entity(character.root).despawn();
                    }
                    actors.failed_visuals.remove(&update.vid);
                }
            }
            NetEvent::ActorRemove { vid } => {
                actors.by_vid.remove(vid);
                actors.characters.remove(vid);
                actors.failed_visuals.remove(vid);
                if let Some(entity) = actors.entities.remove(vid) {
                    commands.entity(entity).despawn();
                }
            }
            _ => {}
        }
    }
}

/// Keeps the Bevy actor transforms in map-local coordinates as server movement
/// and synchronization packets arrive. Height is sampled from the loaded map.
fn sync_actor_entities(world: &mut World) {
    let delta = world.resource::<Time>().delta_secs().min(0.1);
    {
        let mut actors = world.resource_mut::<Actors>();
        for actor in actors.by_vid.values_mut() {
            if actor.move_duration > 0.0 {
                actor.move_elapsed = (actor.move_elapsed + delta).min(actor.move_duration);
                let position = actor.move_from.lerp(actor.move_to, actor.move_elapsed / actor.move_duration);
                actor.x = position.x.round() as i32;
                actor.y = position.y.round() as i32;
            }
        }
    }
    let (actors, entities, mobs, base, heights, ready) = {
        let actors = world.resource::<Actors>();
        let game = world.resource::<GameWorld>();
        let mobs = world.resource::<MobCatalog>().by_race.clone();
        let base = game.base;
        let heights = actors
            .by_vid
            .iter()
            .map(|(&vid, actor)| {
                let x = (actor.x - base.0 as i32) as f32;
                let y = (actor.y - base.1 as i32) as f32;
                (vid, game.terrain_height(x, y))
            })
            .collect::<HashMap<_, _>>();
        (actors.by_vid.clone(), actors.entities.clone(), mobs, base, heights, game.is_ready())
    };
    if !ready {
        return;
    }
    let mut query = world.query::<(&mut ActorEntity, &mut Transform)>();
    for (mut actor_entity, mut transform) in query.iter_mut(world) {
        let Some(actor) = actors.get(&actor_entity.vid) else { continue };
        let local_x = (actor.x - base.0 as i32) as f32;
        let local_y = (actor.y - base.1 as i32) as f32;
        let height = heights.get(&actor_entity.vid).copied().unwrap_or_default();
        transform.translation = local_to_world(local_x, local_y, height);
        transform.rotation = Quat::from_rotation_y(-actor.rotation.to_radians());
        actor_entity.name.clone_from(&actor.name);
        actor_entity.armor = actor.armor;
        actor_entity.hair = actor.hair;
    }

    let pending: Vec<(u32, Entity, Actor)> = entities
        .iter()
        .filter(|(vid, _)| {
            let actors = world.resource::<Actors>();
            !actors.characters.contains_key(*vid) && !actors.failed_visuals.contains(*vid)
        })
        .filter_map(|(&vid, &entity)| actors.get(&vid).cloned().map(|actor| (vid, entity, actor)))
        .collect();
    for (vid, parent, actor) in pending {
        let character = if actor.kind == m2_net::phase::ACTOR_TYPE_PC {
            let shape = world.resource::<ArmorShapes>().shape(u32::from(actor.armor));
            Character::spawn(
                world,
                Look { race: actor.race as u8, shape, hair: u32::from(actor.hair) },
                Transform::IDENTITY,
                Some(parent),
            )
        } else if let Some(file) = mobs.get(&actor.race).and_then(|mob| mob.race_file.as_deref()) {
            Character::spawn_actor(world, actor.race, file, Transform::IDENTITY, Some(parent))
        } else {
            if mobs.contains_key(&actor.race) {
                warn!("actor race {} has no matching registered MSM from npclist.txt", actor.race);
            } else {
                warn!("actor race {} is missing from {LOCALE_PATH}/mob_proto", actor.race);
            }
            world.resource_mut::<Actors>().failed_visuals.insert(vid);
            continue;
        };
        world.resource_mut::<Actors>().characters.insert(vid, character);
    }
    let rendered: HashSet<u32> = world.resource::<Actors>().characters.keys().copied().collect();
    let motion_updates: Vec<(u32, Actor)> = {
        let mut actors = world.resource_mut::<Actors>();
        actors
            .by_vid
            .iter_mut()
            .filter_map(|(&vid, actor)| {
                if actor.rendered_moving == Some(actor.moving) || !rendered.contains(&vid) {
                    return None;
                }
                actor.rendered_moving = Some(actor.moving);
                Some((vid, actor.clone()))
            })
            .collect()
    };
    for (vid, actor) in motion_updates {
        world.resource_scope(|world, mut actors: Mut<Actors>| {
            let Some(character) = actors.characters.get_mut(&vid) else { return };
            if actor.kind == m2_net::phase::ACTOR_TYPE_PC {
                character.play_motion(world, if actor.moving { Motion::Run } else { Motion::Wait }, true);
            } else if let Some(motions) = mobs.get(&actor.race).map(|mob| &mob.motions) {
                let motion = if actor.moving { motions.run.as_ref().or(motions.walk.as_ref()) } else { motions.wait.as_ref() };
                if let Some(motion) = motion {
                    character.play(world, motion, true);
                }
            }
        });
    }
    world.resource_scope(|world, mut actors: Mut<Actors>| {
        for character in actors.characters.values_mut() {
            character.update(world);
        }
    });
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
                    sync_main_weapon(world, &mut character);
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

            // The player's own combat state (`CPythonPlayer` + the main actor).
            let in_safe = map_attr(scene, local, ATTR_BANPK);
            let (mut me, combat_world, points, arrows, riding_grade, riding_level, mount_vnum, ) = {
                let player = world.get_resource::<PlayerState>();
                let skills = world.get_resource::<combat::SkillState>();
                let mut me = player.map(PlayerState::combat_self).unwrap_or_default();
                me.x = local.x.round() as i32;
                me.y = local.y.round() as i32;
                me.in_safe = in_safe;
                (
                    me,
                    player.map(PlayerState::combat_world).unwrap_or_default(),
                    player.map(|player| combat::Points::from_points(&player.points)).unwrap_or_default(),
                    player.map_or(0, PlayerState::arrow_count),
                    skills.map_or(0, |skills| skills.grade(109)),
                    skills.map_or(0, |skills| skills.level(109)),
                    player.map_or(0, |player| player.mount_vnum),
                )
            };
            me.in_safe = in_safe;

            // The tracked actors as combat sees them, in map-local coordinates.
            let candidates: Vec<(u32, ActorCombat)> = world
                .get_resource::<Actors>()
                .map(|actors| {
                    actors
                        .by_vid
                        .iter()
                        .map(|(&vid, actor)| {
                            let mut combat = actor.combat(vid, base);
                            combat.in_safe = map_attr(scene, Vec2::new(combat.x as f32, combat.y as f32), ATTR_BANPK);
                            (vid, combat)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let attackable = |vid: u32| {
                candidates
                    .iter()
                    .find(|(candidate, _)| *candidate == vid)
                    .is_some_and(|(_, candidate)| combat::is_attackable(&combat_world, &me, candidate))
            };
            let distance_to = |vid: u32, local: Vec2| {
                candidates.iter().find(|(candidate, _)| *candidate == vid).map(|(_, candidate)| {
                    (Vec2::new(candidate.x as f32, candidate.y as f32) - local).length()
                })
            };

            // `CPythonPlayer::SetTarget`: keep the current target while it can
            // still be picked, else take the nearest pickable actor.
            let still_targetable = control.target.and_then(|vid| candidates.iter().find(|(candidate, _)| *candidate == vid))
                .is_some_and(|(_, actor)| actor.can_pick());
            let new_target = if still_targetable {
                control.target
            } else {
                nearest_actor(&world.resource::<Actors>().by_vid, base, local, main_vid, TARGET_RANGE)
                    .map(|(vid, _)| vid)
                    .filter(|vid| candidates.iter().find(|(candidate, _)| candidate == vid).is_some_and(|(_, actor)| actor.can_pick()))
            };
            if new_target != control.target {
                control.target = new_target;
                if let Some(mut net) = world.get_resource_mut::<Net>() {
                    net.send_target(new_target.unwrap_or(0));
                }
            }
            control.target_distance = control.target.and_then(|vid| distance_to(vid, local));

            // Left click: an actor under the pointer is targeted (and attacked
            // when in reach), otherwise the ground point is a move order.
            let mut click_attack = false;
            if !focus.pointer
                && buttons.just_pressed(MouseButton::Left)
                && let Some(cursor) = cursor
            {
                if let Some(vid) = pick_actor(world, cursor, base) {
                    control.target = Some(vid);
                    control.target_distance = distance_to(vid, local);
                    if let Some(mut net) = world.get_resource_mut::<Net>() {
                        net.send_target(vid);
                    }
                    let in_reach = control.target_distance.is_some_and(|distance| {
                        let bow = matches!(character.motion_mode(), crate::render::character::MotionMode::Bow | crate::render::character::MotionMode::HorseBow);
                        distance <= if bow { bow_range(&points) } else { ATTACK_RANGE }
                    });
                    if attackable(vid) && in_reach {
                        click_attack = true;
                    } else if let Some((_, actor)) = candidates.iter().find(|(candidate, _)| *candidate == vid) {
                        let destination = Vec2::new(actor.x as f32, actor.y as f32);
                        control.destination = Some(destination);
                        control.reserved_attack = Some(vid);
                        info!("walk to actor {vid} at {:.0},{:.0}", destination.x, destination.y);
                    }
                } else {
                    let height = |x: f32, y: f32| scene.map.map.terrain_height(x, y);
                    if let Some(point) = pick_terrain(world, cursor, height, extent) {
                        control.destination = Some(point);
                        info!("walk to {:.0},{:.0}", point.x, point.y);
                    }
                }
            }

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
                    attack_ready = now + ATTACK_COOLDOWN;
                }
            }

            let mut moved = false;
            let mut direction = None;
            if !matches!(control.state, MoveState::Attacking | MoveState::Casting) {
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

            // `MODE_CLICK_ACTOR`: attack a clicked target once it is in reach.
            if let Some(vid) = control.reserved_attack {
                let in_reach = distance_to(vid, local).is_some_and(|distance| {
                    let bow = matches!(
                        character.motion_mode(),
                        crate::render::character::MotionMode::Bow | crate::render::character::MotionMode::HorseBow
                    );
                    distance <= if bow { bow_range(&points) } else { ATTACK_RANGE }
                });
                if !attackable(vid) {
                    control.reserved_attack = None;
                } else if in_reach {
                    control.reserved_attack = None;
                    click_attack = true;
                }
            }

            // `MODE_USE_SKILL`: keep running towards the reserved target and
            // cast as soon as the skill's range is reached.
            if let Some((vnum, vid, range)) = control.reserved_skill {
                if let Some((_, actor)) = candidates.iter().find(|(candidate, _)| *candidate == vid) {
                    if skill_distance(&me, actor) < range as f32 {
                        control.reserved_skill = None;
                        use_skill(
                            world,
                            &mut control,
                            character,
                            vnum,
                            &me,
                            &combat_world,
                            &points,
                            arrows,
                            (riding_grade, riding_level),
                            mount_vnum,
                            &candidates,
                            &attackable,
                            local,
                            now,
                            base,
                        );
                    } else if control.destination.is_none() {
                        control.destination = Some(Vec2::new(actor.x as f32, actor.y as f32));
                    }
                } else {
                    control.reserved_skill = None;
                }
            }

            // Horse commands (`game.py`'s Ctrl+H/J/B/F/G).
            if !focus.keyboard {
                let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
                if ctrl {
                    let command = if keys.just_pressed(KeyCode::KeyH) {
                        Some("/user_horse_ride")
                    } else if keys.just_pressed(KeyCode::KeyJ) {
                        if mount_vnum != 0 {
                            Some("/unmount")
                        } else {
                            // `game.py` Ctrl+J uses the first horse item.
                            let cell = world.get_resource::<PlayerState>().and_then(|player| {
                                player.items.iter().find(|(position, item)| {
                                    position.window == 1
                                        && matches!(item.vnum, 71114 | 71116 | 71118 | 71120)
                                }).map(|(position, _)| position.cell)
                            });
                            if let Some(cell) = cell
                                && let Some(mut net) = world.get_resource_mut::<Net>()
                            {
                                net.game.send_item_use(cell);
                            }
                            None
                        }
                    } else if keys.just_pressed(KeyCode::KeyB) {
                        Some("/user_horse_back")
                    } else if keys.just_pressed(KeyCode::KeyF) {
                        Some("/user_horse_feed")
                    } else if keys.just_pressed(KeyCode::KeyG) {
                        Some("/ride")
                    } else {
                        None
                    };
                    if let Some(command) = command
                        && let Some(mut net) = world.get_resource_mut::<Net>()
                    {
                        net.game.send_chat(command, 0);
                    }
                }
            }

            // Space (held): `CPythonPlayer::NEW_Attack` every frame the attack
            // key is down and no attack is running.
            let can_act = !matches!(control.state, MoveState::Attacking | MoveState::Casting);
            if !focus.keyboard && (keys.pressed(KeyCode::Space) || click_attack) && now >= attack_ready && can_act {
                let mode = character.motion_mode();
                let hand_mode = matches!(
                    mode,
                    crate::render::character::MotionMode::General | crate::render::character::MotionMode::Horse
                );
                let bow = matches!(
                    mode,
                    crate::render::character::MotionMode::Bow | crate::render::character::MotionMode::HorseBow
                );
                let context = AttackContext {
                    world: combat_world.clone(),
                    me,
                    mounted: mount_vnum,
                    riding_grade,
                    riding_level,
                    observer: combat_world.observer,
                    in_safe,
                    hand_mode,
                    arrows,
                };
                let result = if bow {
                    let victim = control
                        .target
                        .filter(|vid| attackable(*vid))
                        .or_else(|| nearest_attackable(&candidates, &attackable, local, bow_range(&points)));
                    match victim {
                        None => Err(combat::AttackError::NeedTarget),
                        Some(victim) => {
                            let victim_safe = candidates
                                .iter()
                                .find(|(candidate, _)| *candidate == victim)
                                .is_some_and(|(_, actor)| actor.in_safe);
                            context.can_shoot(victim_safe).and_then(|()| {
                                if distance_to(victim, local).is_some_and(|distance| distance > bow_range(&points)) {
                                    control.destination = candidates
                                        .iter()
                                        .find(|(candidate, _)| *candidate == victim)
                                        .map(|(_, actor)| Vec2::new(actor.x as f32, actor.y as f32));
                                    Err(combat::AttackError::NeedTarget)
                                } else {
                                    Ok(victim)
                                }
                            })
                        }
                    }
                    .map(|victim| {
                        face_actor(world, &mut control, base, local, victim);
                        control.destination = None;
                        control.state = MoveState::Attacking;
                        control.combo = 1;
                        control.action_started = now;
                        character.play_attack(world, 1);
                        if let Some(mut net) = world.get_resource_mut::<Net>() {
                            if let Some((_, actor)) = candidates.iter().find(|(candidate, _)| *candidate == victim) {
                                net.send_fly_targeting(victim, actor.x + base.0 as i32, actor.y + base.1 as i32, false);
                            }
                            net.send_attack(victim);
                            net.send_shoot(0);
                        }
                    })
                } else {
                    context
                        .can_attack()
                        .and_then(|()| context.mounted_hand_attack())
                        .map(|()| {
                            let victim = control
                                .target
                                .filter(|vid| attackable(*vid) && distance_to(*vid, local).is_some_and(|distance| distance <= ATTACK_RANGE))
                                .or_else(|| nearest_attackable(&candidates, &attackable, local, ATTACK_RANGE))
                                .unwrap_or(0);
                            if now - control.combo_time > 2.0 {
                                control.combo = 0;
                            }
                            control.combo = if control.combo == 0 {
                                1
                            } else {
                                control.combo % mode.combo_count().max(1) + 1
                            };
                            control.combo_time = now;
                            face_actor(world, &mut control, base, local, victim);
                            control.destination = None;
                            control.state = MoveState::Attacking;
                            control.action_started = now;
                            if !character.play_attack(world, control.combo) {
                                control.combo = 1;
                                character.play_attack(world, 1);
                            }
                            if let Some(mut net) = world.get_resource_mut::<Net>() {
                                net.send_attack(victim);
                            }
                            info!("attack: combo {}, victim {victim}, facing {:.0}", control.combo, control.advancing);
                        })
                };
                if let Err(error) = result {
                    if keys.just_pressed(KeyCode::Space)
                        && let Some(player) = world.get_resource_mut::<PlayerState>()
                    {
                        let mut player = player;
                        player.line(1, format!("Cannot attack: {}", error.reason()));
                    }
                } else {
                    let duration = character.motion_duration();
                    attack_ready = now + if duration > 0.05 { duration * 0.9 } else { ATTACK_COOLDOWN };
                }
            }

            // Skill requests from the skill page or the task bar.
            let requests: Vec<u32> = world
                .resource_mut::<Messages<combat::UseSkillRequest>>()
                .drain()
                .map(|request| request.vnum)
                .collect();
            for vnum in requests {
                use_skill(
                    world,
                    &mut control,
                    character,
                    vnum,
                    &me,
                    &combat_world,
                    &points,
                    arrows,
                    (riding_grade, riding_level),
                    mount_vnum,
                    &candidates,
                    &attackable,
                    local,
                    now,
                    base,
                );
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
            if moved || matches!(control.state, MoveState::Attacking | MoveState::Casting) {
                turn_towards(&mut control, delta);
                if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                    transform.rotation = Quat::from_rotation_y(-control.facing.to_radians());
                }
            }
            let mut height = scene.map.map.terrain_height(local.x, local.y);
            if character.is_mounted() {
                height += MOUNT_HEIGHT;
            }
            if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                transform.translation = local_to_world(local.x, local.y, height);
            }
            game.local = Some((local.x.round() as i32, local.y.round() as i32));

            // `CInstanceBase::ChangeWeapon`/`MountHorse`: the server's
            // equipment and mount cells drive the model, motion mode and the
            // wait/run motion.
            if sync_main_weapon(world, character) {
                match control.state {
                    MoveState::Idle => character.play_motion(world, Motion::Wait, true),
                    MoveState::Moving => character.play_motion(world, Motion::Run, true),
                    MoveState::Attacking | MoveState::Casting => {}
                }
            }
            if sync_main_mount(world, character) {
                match control.state {
                    MoveState::Idle => character.play_motion(world, Motion::Wait, true),
                    MoveState::Moving => character.play_motion(world, Motion::Run, true),
                    MoveState::Attacking | MoveState::Casting => {}
                }
            }

            // Motions and position packets.
            match control.state {
                MoveState::Attacking | MoveState::Casting => {
                    if (character.motion_started() && character.motion_finished(world))
                        || now - control.action_started > 4.0
                    {
                        control.state = MoveState::Idle;
                        control.combo_time = now;
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

/// Feeds the main character the weapon in the server-owned equipment cell
/// (`GC_ITEM_SET` on cell `c_Equipment_Weapon` and later changes). The client
/// does not predict equipment; `PlayerState` is the authority. Returns whether
/// the weapon changed, so the caller can restart its motion
/// (`CInstanceBase::ChangeWeapon`).
fn sync_main_weapon(world: &mut World, character: &mut Character) -> bool {
    let Some(player) = world.get_resource::<PlayerState>() else { return false };
    let vnum = player.weapon().map_or(0, |item| item.vnum);
    if vnum == character.weapon_vnum() {
        return false;
    }
    if vnum == 0 {
        return character.clear_weapon(world);
    }
    let visual = {
        let Some(packs) = world.get_resource::<M2Packs>().map(|packs| packs.packs.clone()) else { return false };
        world.get_resource::<ItemCatalog>().and_then(|catalog| catalog.weapon_visual(&packs, vnum))
    };
    match visual {
        Some(visual) => character.set_weapon(world, visual.vnum, &visual.model, &visual.sub_model, visual.weapon_type),
        // The catalog is still loading; retry next frame.
        None => false,
    }
}

/// `CPythonPlayer::__UseSkill` (`PythonPlayerSkill.cpp:475`). The UI and the
/// task bar only say *which* skill; target choice, checks, motion and the
/// packet are the C++ player's job and live here.
#[allow(clippy::too_many_arguments)]
fn use_skill(
    world: &mut World,
    control: &mut PlayerControl,
    character: &mut Character,
    vnum: u32,
    me: &ActorCombat,
    combat_world: &combat::CombatWorld,
    points: &combat::Points,
    arrows: u32,
    riding: (u8, u8),
    mount: u32,
    candidates: &[(u32, ActorCombat)],
    attackable: &impl Fn(u32) -> bool,
    local: Vec2,
    now: f32,
    base: (u32, u32),
) {
    let Some(data) = world.get_resource::<combat::SkillCatalog>().and_then(|catalog| catalog.get(vnum).cloned()) else {
        warn!("skill {vnum}: no skill data loaded");
        return;
    };
    let (level, grade, cool_left) = world
        .get_resource::<combat::SkillState>()
        .map(|state| (state.level(vnum), state.grade(vnum), state.cool_time_left(vnum, now)))
        .unwrap_or((0, 0, 0.0));
    let fail = |world: &mut World, error: SkillError| {
        if let Some(player) = world.get_resource_mut::<PlayerState>() {
            let mut player = player;
            player.line(1, format!("Cannot use skill: {}", error.reason()));
        }
    };

    // `CanAct` (`CActorInstance::CanUseSkill`): dead and stunned actors cannot
    // start a skill.
    if combat_world.observer {
        fail(world, SkillError::Observer);
        return;
    }
    if me.is_dead() {
        fail(world, SkillError::Dead);
        return;
    }
    if combat::has_affect(me.affect_flags, combat::AFFECT_STUN) {
        fail(world, SkillError::Stunned);
        return;
    }

    // `ClickSkillSlot` sends guild skills through `UseGuildSkill`, not
    // `__UseSkill`: the guild packet, cooldown and motion only.
    if data.skill_type == m2_formats::proto::SkillType::Guild as u8 {
        if cool_left > 0.0 {
            fail(world, SkillError::WaitCooltime);
            return;
        }
        if !data.no_motion {
            let motion = combat::skill_motion_index(&data, grade);
            character.play_skill(world, motion);
        }
        control.state = MoveState::Casting;
        control.action_started = now;
        if let Some(mut net) = world.get_resource_mut::<Net>() {
            net.send_guild_use_skill(vnum, 0);
        }
        let cooldown = combat::skill_cooldown(&data, f32::from(level), points, points.casting_speed);
        if let Some(mut state) = world.get_resource_mut::<combat::SkillState>() {
            state.run_cool_time(vnum, cooldown, now);
        }
        return;
    }

    // `__CanUseSkill`: the riding skill gates mounted skills.
    if mount != 0 && riding.0 < 1 && riding.1 < 20 {
        fail(world, SkillError::MountSkillLevel);
        return;
    }
    let check = SkillCheck {
        level,
        grade,
        level_limit_enforced: false,
        mounted: mount,
        in_safe: me.in_safe,
        fishing: false,
        weapon_type: character.weapon_type().unwrap_or(combat::weapon::NONE),
        arrows,
        cool_time_enforced: true,
        cool_time_left: cool_left,
        empty_bottles: world.get_resource::<PlayerState>().map_or(0, |player| u32::from(player.has_item(27995))),
        poison_bottles: world.get_resource::<PlayerState>().map_or(0, |player| u32::from(player.has_item(27996))),
        affected_by_dash: false,
        is_charge_reserved: control.reserved_skill.is_some(),
    };
    if let Err(error) = combat::check_skill_usable(&data, &check, points) {
        fail(world, error);
        return;
    }
    // `__UseSkill`'s toggle branch: an active toggle only sends the packet to
    // turn it off, and does not play the motion again.
    let toggle_active = (vnum as usize) < 255
        && world.get_resource::<combat::SkillState>().is_some_and(|state| state.active[vnum as usize]);
    if data.is_toggle() && toggle_active {
        if let Some(mut net) = world.get_resource_mut::<Net>() {
            net.send_use_skill(vnum, 0);
        }
        if (vnum as usize) < 255
            && let Some(mut state) = world.get_resource_mut::<combat::SkillState>()
        {
            state.active[vnum as usize] = false;
        }
        return;
    }

    let candidate = |vid: u32| candidates.iter().find(|(candidate, _)| *candidate == vid).map(|(_, actor)| *actor);
    let mut target = control.target.and_then(candidate);
    let needs_target = data.needs_target() || data.can_change_direction() || data.is_auto_search_target();
    if needs_target {
        if let Some(actor) = target {
            let acceptable = if data.needs_corpse() { actor.is_dead() } else { !actor.is_dead() };
            if !acceptable {
                target = None;
            }
        }
        if target.is_none() && data.is_auto_search_target() {
            let found = combat::front_target(me, combat_world, control.facing, &candidates.iter().map(|(_, actor)| *actor).collect::<Vec<_>>(), 2000.0);
            if let Some(vid) = found {
                control.target = Some(vid);
                target = candidate(vid);
            }
        }
        match target {
            Some(actor) if actor.is_dead() && !data.needs_corpse() => {
                fail(world, SkillError::NeedTarget);
                return;
            }
            Some(actor) if !actor.is_dead() && data.needs_corpse() => {
                fail(world, SkillError::OnlyForCorpse);
                return;
            }
            Some(actor) => {
                if data.only_for_alliance() {
                    if actor.vid == me.vid && !data.can_use_for_me() {
                        fail(world, SkillError::CannotUseSelf);
                        return;
                    }
                    if actor.is_pc() && !attackable(actor.vid) {
                        let range = combat::skill_target_range(&data, points.bow_distance);
                        if range > 0 && skill_distance(me, &actor) >= range as f32 {
                            reserve_skill(control, &actor, vnum, range, local);
                            return;
                        }
                    } else if data.can_use_for_me() {
                        target = candidate(me.vid);
                    } else {
                        fail(world, SkillError::OnlyForAlliance);
                        return;
                    }
                } else {
                    if actor.in_safe {
                        fail(world, SkillError::CannotAttackEnemyInSafe);
                        return;
                    }
                    if !attackable(actor.vid) {
                        fail(world, SkillError::CannotAttack);
                        return;
                    }
                    let range = combat::skill_target_range(&data, points.bow_distance);
                    if range > 0 && skill_distance(me, &actor) >= range as f32 {
                        // `__ProcessEnemySkillTargetRange`: charge skills send
                        // the packet now and then run into range.
                        if data.is_charge() && !check.is_charge_reserved
                            && let Some(mut net) = world.get_resource_mut::<Net>()
                        {
                            net.send_use_skill(vnum, actor.vid);
                        }
                        reserve_skill(control, &actor, vnum, range, local);
                        return;
                    }
                }
            }
            None => {
                if data.can_use_for_me() {
                    target = candidate(me.vid);
                } else if data.needs_corpse() {
                    fail(world, SkillError::OnlyForCorpse);
                    return;
                } else {
                    fail(world, SkillError::NeedTarget);
                    return;
                }
            }
        }
    }

    let target = target.or_else(|| data.can_use_for_me().then(|| candidate(me.vid)).flatten());
    let target_vid = target.map_or(0, |actor| actor.vid);

    // `CanChangeDirection`: look at the picked actor/target.
    if let Some(actor) = target
        && actor.vid != me.vid
    {
        face_actor(world, control, base, local, actor.vid);
    }

    // Fan/circle multi-target skills (`GetTargetCount`).
    if let Some(actor) = target {
        let count = combat::skill_target_count(&data, f32::from(level), points);
        if count > 1 {
            let range = combat::skill_target_range(&data, points.bow_distance) as f32;
            let extra = combat::skill_targets(
                me,
                combat_world,
                &candidates.iter().map(|(_, actor)| *actor).collect::<Vec<_>>(),
                (actor.x as f32, actor.y as f32),
                range,
                data.is_fan_range(),
            );
            for vid in extra.into_iter().filter(|vid| *vid != actor.vid).take(count as usize - 1) {
                if let Some(other) = candidate(vid)
                    && let Some(mut net) = world.get_resource_mut::<Net>()
                {
                    net.send_fly_targeting(vid, other.x + base.0 as i32, other.y + base.1 as i32, true);
                }
            }
        }
    }

    // `NEW_UseSkill`: the skill's registered motion, unless the data says none.
    if !data.no_motion {
        let motion = combat::skill_motion_index(&data, grade);
        if !character.play_skill(world, motion) {
            trace!("skill {vnum}: no motion for index {motion}");
        }
    }
    control.reserved_skill = None;
    control.destination = None;
    control.combo = 0;
    control.state = MoveState::Casting;
    control.action_started = now;
    if let Some(mut net) = world.get_resource_mut::<Net>() {
        net.send_use_skill(vnum, target_vid);
    }
    let cooldown = combat::skill_cooldown(&data, f32::from(level), points, points.casting_speed);
    if let Some(mut state) = world.get_resource_mut::<combat::SkillState>() {
        state.run_cool_time(vnum, cooldown, now);
        if data.is_toggle() && (vnum as usize) < state.active.len() {
            state.active[vnum as usize] = true;
        }
    }
    info!("skill {vnum} level {level} on target {target_vid} (cooldown {cooldown:.1}s)");
}

fn distance_between(a: &ActorCombat, b: &ActorCombat) -> f32 {
    ((b.x - a.x) as f32).hypot((b.y - a.y) as f32)
}

/// `IS_HUGE_RACE` (`ActorInstanceBattle.cpp`): the huge race is 2493 and
/// the client fakes 200 units of extra reach for it.
fn skill_distance(a: &ActorCombat, b: &ActorCombat) -> f32 {
    let distance = distance_between(a, b);
    if b.race == 2493 { distance - 200.0 } else { distance }
}

/// `__ReserveUseSkill`: run towards the target and cast when in range.
fn reserve_skill(control: &mut PlayerControl, target: &ActorCombat, vnum: u32, range: u32, local: Vec2) {
    control.reserved_skill = Some((vnum, target.vid, range));
    let destination = Vec2::new(target.x as f32, target.y as f32);
    let towards = (local - destination).normalize_or_zero();
    control.destination = Some(destination + towards * (range as f32 * 0.8));
}

/// `CInstanceBase::SHORSE`: the server's mount packet drives the horse model
/// and the mounted motion mode.
fn sync_main_mount(world: &mut World, character: &mut Character) -> bool {
    let Some(player) = world.get_resource::<PlayerState>() else { return false };
    let vnum = player.mount_vnum;
    if vnum == character.mount_vnum() {
        return false;
    }
    if vnum == 0 {
        return character.clear_mount(world);
    }
    let file = world
        .get_resource::<MobCatalog>()
        .and_then(|mobs| mobs.by_race.get(&(vnum as u16)))
        .and_then(|mob| mob.race_file.clone());
    match file {
        Some(file) => character.set_mount(world, vnum, &file),
        None => {
            warn!("mount vnum {vnum} has no registered MSM race");
            false
        }
    }
}



/// `CTerrainImpl::ATTRIBUTE_BANPK` (`IsInSafe`).
const ATTR_BANPK: u8 = 1 << 2;

/// `CInstanceBase::__GetBowRange`: 2500 - 100 plus `POINT_BOW_DISTANCE`.
fn bow_range(points: &combat::Points) -> f32 {
    2400.0 + points.bow_distance.max(0) as f32
}

/// `CMapOutdoor::isAttrOn` on the loaded map, in map-local coordinates.
fn map_attr(scene: &MapScene, local: Vec2, flag: u8) -> bool {
    let x = local.x.floor() as i32;
    let y = local.y.floor() as i32;
    if x < 0 || y < 0 {
        return false;
    }
    let cell = (x / TERRAIN_WORLD as i32) as u16;
    let row = (y / TERRAIN_WORLD as i32) as u16;
    scene
        .map
        .map
        .terrains
        .iter()
        .find(|terrain| terrain.x == cell && terrain.y == row)
        .is_some_and(|terrain| {
            terrain.is_attr_on(
                ((x % TERRAIN_WORLD as i32) / 100) as u16,
                ((y % TERRAIN_WORLD as i32) / 100) as u16,
                flag,
            )
        })
}

/// The nearest attackable actor within `range` of `local`.
fn nearest_attackable(
    candidates: &[(u32, ActorCombat)],
    attackable: &impl Fn(u32) -> bool,
    local: Vec2,
    range: f32,
) -> Option<u32> {
    candidates
        .iter()
        .filter(|(vid, actor)| attackable(*vid) && !actor.is_dead())
        .map(|(vid, actor)| ((Vec2::new(actor.x as f32, actor.y as f32) - local).length(), *vid))
        .filter(|(distance, _)| *distance <= range)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, vid)| vid)
}

/// The actor under the pointer: `CPythonCharacterManager::OLD_GetPickedInstance`
/// with a screen-space test against the tracked actors.
fn pick_actor(world: &mut World, cursor: Vec2, base: (u32, u32)) -> Option<u32> {
    let actors: Vec<(u32, f32, f32)> = world
        .get_resource::<Actors>()
        .map(|actors| {
            actors
                .by_vid
                .iter()
                .map(|(&vid, actor)| {
                    (vid, (actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32)
                })
                .collect()
        })
        .unwrap_or_default();
    let camera = {
        let mut query = world.query_filtered::<(&Camera, &GlobalTransform), With<SceneCamera>>();
        query.single(world).ok().map(|(camera, transform)| (camera.clone(), *transform))
    };
    let (camera, transform) = camera?;
    let mut best: Option<(f32, u32)> = None;
    for (vid, x, y) in actors {
        let position = Vec3::new(x, 80.0, -y);
        let Ok(screen) = camera.world_to_viewport(&transform, position) else { continue };
        let distance = screen.distance(cursor);
        if distance <= 60.0 && best.is_none_or(|(best_distance, _)| distance < best_distance) {
            best = Some((distance, vid));
        }
    }
    best.map(|(_, vid)| vid)
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
    control.combo = 1;
    control.action_started = world.resource::<Time>().elapsed_secs();
    character.play_attack(world, 1);
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
    fn actor_motion_list_uses_pack_virtual_paths_and_msm_override() {
        use m2_formats::pack::{Container, PackManager};
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("fixture/ymir work/npc/guard");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(
            folder.join("guard.msm"),
            b"BaseModelFileName \"d:/ymir work/npc/guard/guard.gr2\"\nMotionListFileName \"custom.txt\"\n",
        )
        .unwrap();
        std::fs::write(folder.join("custom.txt"), b"GENERAL WAIT 00.msa 100\nGENERAL RUN 03.msa 100\n").unwrap();
        let mut packs = PackManager::new(dir.path());
        packs.register("fixture", Container::Folder).unwrap();

        let motions = load_actor_motions(&packs, "ymir work/npc/guard/guard.msm");
        assert_eq!(motions.wait.as_deref(), Some("ymir work/npc/guard/00.msa"));
        assert_eq!(motions.run.as_deref(), Some("ymir work/npc/guard/03.msa"));
    }

    #[test]
    fn packed_guardians_have_idle_clips_for_their_skeletons() {
        use m2_formats::pack::{PackManager, load_index};
        let dir = std::env::var_os("M2_CLIENT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"));
        if !dir.join("pack/Index.dev").exists() {
            eprintln!("SKIP: client data not found (set M2_CLIENT_DIR)");
            return;
        }
        let mut packs = PackManager::new(&dir);
        load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        let index = m2_assets::DirectoryIndex::build(&packs);
        let races = load_race_registrations(&packs, &index);
        for race in [11000, 11002] {
            let appearance = races.get(&race).expect("guardian registration");
            let race_file = appearance.race_file.as_deref().expect("guardian race file");
            let wait = appearance.motions.wait.as_deref().expect("guardian idle motion");
            assert!(wait.ends_with("/00.msa"), "race {race}: {wait}");

            let mut race_data = m2_formats::race::RaceData::default();
            assert!(race_data.load(&packs, paths::virtual_name(race_file), |_| true));
            let model_file = proto_text(&race_data.base_model_file_name);
            let model = m2_formats::gr2::parse(&packs.get(paths::virtual_name(&model_file)).unwrap()).unwrap();
            let skeleton = model.models[0].skeleton.as_ref().expect("guardian skeleton");
            let msa = packs.get(paths::virtual_name(wait)).unwrap();
            let motion = m2_formats::race::RaceMotionData::from_script(&m2_formats::script::parse(&msa))
                .expect("guardian motion data");
            let animation_file = proto_text(&motion.motion_file_name);
            let animation = m2_formats::gr2::parse(&packs.get(paths::virtual_name(&animation_file)).unwrap()).unwrap();
            let clip = m2_assets::gr2::animation_to_clip(&animation.animations[0]);
            let animated_bones = skeleton.bones.iter().filter(|bone| {
                let name = String::from_utf8_lossy(&bone.name);
                clip.curves().contains_key(&m2_assets::gr2::bone_target_id(&name))
            }).count();
            assert!(animated_bones > 10, "race {race} has only {animated_bones} animated bones");
        }
    }

    #[test]
    fn actor_updates_keep_unchanged_visuals_and_restart_changed_player_motions() {
        use bevy::asset::AssetPlugin;
        use m2_assets::M2AssetsPlugin;
        use m2_formats::pack::PackManager;
        use m2_net::phase::{ACTOR_TYPE_NPC, ACTOR_TYPE_PC, ActorUpdate};

        let dir = tempfile::tempdir().unwrap();
        let packs = std::sync::Arc::new(PackManager::new(dir.path()));
        let mut app = App::new();
        app.add_plugins(M2AssetsPlugin::from_packs(packs))
            .add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_resource::<Actors>()
            .init_resource::<GameWorld>()
            .init_resource::<MobCatalog>()
            .add_message::<NetMessage>()
            .add_systems(Update, track_actors);
        app.finish();
        app.cleanup();

        for (vid, kind) in [(10, ACTOR_TYPE_NPC), (11, ACTOR_TYPE_PC)] {
            let actor = Actor {
                name: String::new(), race: 0, kind, level: 0, armor: 0, hair: 0,
                empire: 0, pk_mode: 0, guild_id: 0, alignment: 0, state_flags: 0, affect_flags: 0, mount_vnum: 0,
                x: 0, y: 0, rotation: 0.0, move_from: Vec2::ZERO, move_to: Vec2::ZERO,
                move_elapsed: 0.0, move_duration: 0.0, moving: false, rendered_moving: Some(false),
            };
            let character = Character::spawn(app.world_mut(), Look { race: 0, shape: 0, hair: 0 }, Transform::IDENTITY, None);
            let mut actors = app.world_mut().resource_mut::<Actors>();
            actors.by_vid.insert(vid, actor);
            actors.characters.insert(vid, character);
        }
        let update = |vid, armor| NetMessage::Game(NetEvent::ActorUpdate(ActorUpdate {
            vid, armor, guild_id: 0, move_speed: 0, attack_speed: 0, state_flags: 0,
            affect_flags: 0, alignment: 0, pk_mode: 0, weapon: 0, hair: 0, mount_vnum: 0,
        }));
        app.world_mut().write_message(update(10, 1));
        app.world_mut().write_message(update(11, 0));
        app.update();
        assert!(app.world().resource::<Actors>().characters.contains_key(&10));
        assert!(app.world().resource::<Actors>().characters.contains_key(&11));

        app.world_mut().write_message(update(11, 1));
        app.update();
        let actors = app.world().resource::<Actors>();
        assert!(!actors.characters.contains_key(&11));
        assert_eq!(actors.by_vid[&11].rendered_moving, None);
    }

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
            actors.insert(
                vid,
                Actor {
                    name: String::new(),
                    race: 0,
                    kind: 1,
                    level: 0,
                    armor: 0,
                    hair: 0,
                    empire: 0, pk_mode: 0, guild_id: 0, alignment: 0, state_flags: 0, affect_flags: 0, mount_vnum: 0,
                    x,
                    y,
                    rotation: 0.0,
                    move_from: Vec2::new(x as f32, y as f32),
                    move_to: Vec2::new(x as f32, y as f32),
                    move_elapsed: 0.0,
                    move_duration: 0.0,
                    moving: false,
                    rendered_moving: None,
                },
            );
        }
        let target = nearest_actor(&actors, (0, 0), Vec2::ZERO, 11, 500.0);
        assert_eq!(target.map(|(vid, _)| vid), Some(10));
        // The main actor is never its own target.
        let target = nearest_actor(&actors, (0, 0), Vec2::ZERO, 10, 500.0);
        assert_eq!(target.map(|(vid, _)| vid), Some(11));
    }
}
