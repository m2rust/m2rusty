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

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy::window::PrimaryWindow;
use m2_assets::{M2Packs, paths};
use m2_formats::proto::parse_mob_proto;
use m2_net::phase::{ActorMove, NetEvent};

use crate::SceneCamera;
use crate::battle::{self, DynamicSphere};
use crate::combat::{self, ActorCombat, AttackContext, SkillCheck, SkillError, SkillExt};
use crate::fx::{self, CombatFx};
use crate::net::{Net, NetMessage};
use crate::player::{ItemCatalog, PlayerState};
use crate::render::character::{ArmorShapes, Character, FishingMotion, Look, MOUNT_HEIGHT, Motion, MotionTick};
use crate::render::{environment_fog, soften_fog};
use crate::world::{MapScene, WarpedMap, prepare_map_at};

/// `LocaleService_GetLocalePath`'s default (`Locale.cpp`); used for
/// `AtlasInfo.txt` and `item_proto`.
pub const LOCALE_PATH: &str = "locale/en";

/// `CInstanceBase::FUNC_WAIT` / `FUNC_MOVE`.
pub const FUNC_WAIT: u8 = 0;
pub const FUNC_MOVE: u8 = 1;
/// `FUNC_ATTACK`, `FUNC_COMBO`, `FUNC_MOB_SKILL` and the `FUNC_SKILL` flag.
pub const FUNC_ATTACK: u8 = 2;
pub const FUNC_COMBO: u8 = 3;
pub const FUNC_MOB_SKILL: u8 = 4;
pub const FUNC_SKILL: u8 = 0x80;

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
/// `CActorInstance::__AccumulationMovement` fallback; the shipped player run
/// motions also accumulate 450 units per second at 100% movement speed.
const RUN_SPEED: f32 = 450.0;
/// `CPythonPlayerEventHandler::OnMoving`: a position packet every 300 ms.
const MOVE_PACKET_SECONDS: f32 = 0.3;
const ATTACK_COOLDOWN: f32 = 0.9;
/// `c_fDefaultRotationSpeed` (`InstanceBase.cpp`): degrees per second. A
/// correction wider than 45 degrees uses the full speed, a smaller one 5/12.
const MAX_ROTATION_SPEED: f32 = 1200.0;
/// Melee reach and how far away an actor is offered as a target.
pub const ATTACK_RANGE: f32 = 320.0;
/// How long the scene waits for placed objects before the unattended run calls
/// it loaded (buildings and trees load asynchronously).
const OBJECTS_WAIT: f32 = 8.0;
/// How long a `--demo` run lasts (move, attack, stand).
const DEMO_SECONDS: f32 = 4.9;

/// Duration of each half of the map-transfer curtain.
pub const WARP_FADE_SECONDS: f32 = 0.35;

#[derive(Resource, Default)]
pub struct WarpTransition {
    started: Option<Instant>,
    cleared: bool,
    fading_in: Option<Instant>,
}

impl WarpTransition {
    pub(crate) fn ready_to_connect(&self) -> bool {
        self.cleared
    }

    pub fn alpha(&self) -> f32 {
        if let Some(start) = self.fading_in {
            (1.0 - start.elapsed().as_secs_f32() / WARP_FADE_SECONDS).clamp(0.0, 1.0)
        } else if let Some(start) = self.started {
            (start.elapsed().as_secs_f32() / WARP_FADE_SECONDS).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

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
    pub weapon: u16,
    pub hair: u16,
    pub move_speed: u8,
    pub attack_speed: u8,
    walk_mode: bool,
    /// Global position.
    pub x: i32,
    pub y: i32,
    pub rotation: f32,
    rendered_rotation: f32,
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
    moving: bool,
    rendered_moving: Option<bool>,
    /// Action to perform after reaching the position reported by the server.
    move_action: Option<ActorAction>,
    /// The `CInstanceBase` command queue and movement state.
    remote: RemoteMotion,
}

/// `CInstanceBase::m_kQue_kCmdNew` and the `StateProcess`/`MovementProcess`
/// state of a remote player: commands wait until `dwCmdTime +
/// m_nAverageNetworkGap`, then the actor walks from `move_from` towards
/// `move_to` at its run speed.
#[derive(Clone, Debug, Default, PartialEq)]
struct RemoteMotion {
    /// `(m_dwChkTime, command)`, oldest first.
    commands: VecDeque<(u32, ActorMove)>,
    /// `m_nAverageNetworkGap` in milliseconds.
    network_gap: i32,
    /// `m_isGoing`.
    going: bool,
    /// `m_kMovAfterFunc.eFunc == FUNC_MOVE`: keep walking past `move_to`.
    after_move: bool,
    /// `m_fDstRot`.
    dst_rotation: f32,
    /// The exact position (`x`/`y` are rounded copies).
    position: Vec2,
    /// Monsters walk at the speed their move packet implies (distance over
    /// `dwDuration`); players at their run or walk speed.
    speed: Option<f32>,
    /// `CPhysicsObject`'s push: an offset blended onto the position on top
    /// of any movement (hits with an external force, `GC_SYNC_POSITION`).
    push: Option<PushBlend>,
}

/// `CPhysicsObject::SetLastPosition` + its `CEaseOutInterpolation`s: `total`
/// is added to the actor over [`PUSH_SECONDS`], eased out.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PushBlend {
    total: Vec2,
    applied: Vec2,
    elapsed: f32,
    /// Where the push ends (its start plus `total`), to recognise its echo.
    end: Vec2,
}

impl PushBlend {
    fn new(from: Vec2, to: Vec2) -> Self {
        Self { total: to - from, applied: Vec2::ZERO, elapsed: 0.0, end: to }
    }
    /// Advances the blend; returns this frame's share of the offset.
    fn step(&mut self, delta: f32) -> Vec2 {
        self.elapsed = (self.elapsed + delta).min(PUSH_SECONDS);
        let t = self.elapsed / PUSH_SECONDS;
        let offset = self.total * (1.0 - (1.0 - t) * (1.0 - t));
        let step = offset - self.applied;
        self.applied = offset;
        step
    }
    fn done(&self) -> bool {
        self.elapsed >= PUSH_SECONDS
    }
}

impl Actor {
    /// A player drawn as a player: `CActorInstance::IsPoly` is a PC whose race
    /// is a monster's (`m_eRace >= MAIN_RACE_MAX_NUM`), drawn with that race.
    pub fn has_player_body(&self) -> bool {
        self.kind == m2_net::phase::ACTOR_TYPE_PC && self.race < MAIN_RACE_MAX_NUM
    }

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
    /// Battle state per VID (`CActorInstance`'s attack, hit and death state).
    fights: HashMap<u32, ActorFight>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ShotTarget {
    Actor(u32),
    Position(Vec3),
}

#[derive(Resource, Default)]
struct FlyTargets(HashMap<u32, VecDeque<ShotTarget>>);

impl FlyTargets {
    fn set(&mut self, shooter: u32, target: ShotTarget, additional: bool) {
        let targets = self.0.entry(shooter).or_default();
        if !additional {
            targets.clear();
        }
        targets.push_back(target);
    }

    fn peek(&self, shooter: u32) -> Option<ShotTarget> {
        self.0.get(&shooter)?.front().copied()
    }

    fn shot(&mut self, shooter: u32) -> Option<ShotTarget> {
        let targets = self.0.get_mut(&shooter)?;
        if targets.len() > 1 { targets.pop_front() } else { targets.front().copied() }
    }
}

/// What an actor does in a fight beyond moving.
#[derive(Debug, Default)]
struct ActorFight {
    /// A motion to start on the actor's character this frame.
    pending: Option<ActorAction>,
    /// Its attack motion is running: the hits are tested against the main
    /// character (`CInstanceBase::AttackProcess` of every instance).
    attacking: bool,
    using_skill: bool,
    skill_cancelable: bool,
    knocked_down: bool,
    splash: Option<battle::SplashArea>,
    hits: battle::HitTracker,
    /// `m_fInvisibleTime`: cannot be hit again until then.
    invisible_until: f32,
    /// `m_dwShakeTime`: shaken by a hit until then.
    shake_until: f32,
    /// `Die()` ran (`m_isRealDead`).
    dead: bool,
    /// `IsFishing`: the line is out (`StartFishing` until the cast ends).
    fishing: bool,
    /// `Stun()`: the next hit kills it.
    stunned: bool,
    /// `m_dwOwnerVID`/`m_fOwnerBaseTime` (`GC_OWNERSHIP`): who syncs its position, since when.
    owner: u32,
    owner_since: f32,
}

/// The once motions an actor plays in a fight.
#[derive(Debug, Clone, Copy, PartialEq)]
enum ActorAction {
    /// `FUNC_ATTACK`: `NAME_NORMAL_ATTACK`.
    Attack,
    /// `FUNC_COMBO`: a PC's `NAME_COMBO_ATTACK_*` motion index.
    Combo(u32),
    /// `FUNC_MOB_SKILL`: `NAME_SPECIAL_1 + n`.
    Special(u8),
    /// `FUNC_SKILL | n`: a PC's `NAME_SKILL + n`.
    Skill { index: u16, loops: u8, moving: bool },
    /// `__HitGood`: `NAME_DAMAGE` or `NAME_DAMAGE_BACK`.
    Damage { back: bool },
    /// `__HitGreate`: `NAME_DAMAGE_FLYING(_BACK)`, then standing up.
    Knockdown { back: bool },
    /// `Die()`: `NAME_DEAD`, held at its last frame.
    Dead,
    /// `RecvDigMotionPacket`: `NAME_DIG`, `count` times.
    Dig(u8),
    /// `RecvFishing`: a once motion, then `NAME_FISHING_WAIT` while the
    /// line is out (`fishing`), else `NAME_WAIT`.
    Fishing { motion: FishingMotion, fishing: bool },
}

/// `uiTarget.TargetBoard`: who is targeted and, for monsters, the HP the
/// server reported (`GC_TARGET`, `SetHPTargetBoard`).
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct TargetBoard {
    pub vid: u32,
    /// `"Lv.N (grade) name"` for monsters (`SetEnemyVID`), the name for PCs.
    pub name: String,
    /// `bHPPercent`; `None` until the server reported it (no gauge yet).
    pub hp_percent: Option<u8>,
    pub visible: bool,
}

/// `CInstanceBase::AddDamageEffect`: a damage number over an actor.
#[derive(Message, Clone, Debug)]
pub struct DamageShown {
    /// Bevy position above the actor's head.
    pub position: Vec3,
    pub damage: i32,
    /// `DAMAGE_*` flags (`DAMAGE_DODGE` 2, `DAMAGE_BLOCK` 4, `DAMAGE_CRITICAL` 32...).
    pub flag: u8,
    /// The main character took it (`bSelf`).
    pub own: bool,
    /// The actor is the current target (`bTarget`).
    pub target: bool,
}

/// The target board's close button: `player.ClearTarget()`.
#[derive(Message, Clone, Copy, Debug)]
pub struct ClearTarget;

/// `TARGET_LEVEL_*` of `locale_game.txt`, by mob rank (`PAWN`..`KING`).
#[derive(Resource, Default, Clone, Debug)]
pub struct GradeNames(pub [String; 6]);

/// The client mob_proto data needed to name and render a server NPC/monster.
#[derive(Resource, Default)]
pub struct MobCatalog {
    by_race: HashMap<u16, MobAppearance>,
}

#[derive(Clone, Debug)]
struct MobAppearance {
    name: String,
    level: u8,
    /// `TMobTable::bRank` (`nonplayer.GetGradeByVID`).
    rank: u8,
    race_file: Option<String>,
    motions: ActorMotions,
}

/// The `GENERAL` motions of an NPC race (`motlist.txt`, the names of
/// `CRaceManager::__LoadRaceData`'s table).
#[derive(Clone, Debug, Default)]
struct ActorMotions {
    wait: Option<String>,
    walk: Option<String>,
    run: Option<String>,
    /// `NAME_NORMAL_ATTACK` (`NORMAL_ATTACK`, `NORMAL_ATTACK1`...).
    attack: Vec<String>,
    /// `NAME_DAMAGE` / `NAME_DAMAGE_BACK`.
    damage: Option<String>,
    damage_back: Option<String>,
    /// `NAME_DAMAGE_FLYING(_BACK)` and `NAME_STAND_UP(_BACK)`.
    knockdown: Option<String>,
    knockdown_back: Option<String>,
    stand_up: Option<String>,
    stand_up_back: Option<String>,
    /// `NAME_DEAD` / `NAME_DEAD_BACK`.
    dead: Option<String>,
    dead_back: Option<String>,
    /// `NAME_SPECIAL_1..6` (`SPECIAL`, `SPECIAL1`..), the mob skills.
    special: [Option<String>; 6],
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
    /// `GC_DEAD`: the death motion, no control until revived.
    Dead,
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
    /// `CPythonPlayer::m_dwTargetEndTime`: until then a hit does not take the
    /// target away from the current one (seconds of `Time::elapsed`).
    pub target_end: f32,
    /// `m_isSmtMov`: the left button was pressed on the world and is held.
    pub smart_held: bool,
    /// Where the cursor is pinned during a camera drag, and how far the drag
    /// turned (`m_fPitchSum`/`m_fRollSum`).
    pub camera_anchor: Option<Vec2>,
    pub camera_dragged: f32,
    /// The polymorph race the main body was built for (0: the player race).
    pub poly_race: u16,
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
    /// `MODE_CLICK_ACTOR`: request this NPC's quest script once in reach.
    reserved_talk: Option<u32>,
    /// `MODE_CLICK_ITEM`: the ground item walked to, picked on arrival.
    reserved_item: Option<u32>,
    /// `m_kVctkVictim`: victims pushed this frame, `(vid, global x, global y)`.
    victims: Vec<(u32, i32, i32)>,
    /// `m_isPreInput`: space was pressed inside the combo's pre-input window.
    pub pre_input: bool,
    /// `m_wcurComboType` (`SetComboType`, the combo skill); 0 is the basic combo.
    pub combo_type: u32,
    /// `m_fAtkDirRot`: the heading the combo attacks towards.
    attack_rotation: f32,
    /// `m_HitDataMap` of the running attack motion.
    hits: battle::HitTracker,
    /// `m_kSplashArea` of the running skill motion.
    splash: Option<battle::SplashArea>,
    /// `m_kCurMotNode.uSkill`: the skill whose motion runs (0 for attacks).
    pub motion_skill: u32,
    /// The main character died (`GC_DEAD`); controls are off until revived.
    pub dead: bool,
    /// The actor under the pointer (`chr.Pick`), for its name tail.
    pub hover: Option<u32>,
    /// The main VID and map origin of this frame, for code that runs while
    /// [`GameWorld`] is out of the world.
    main_vid: u32,
    base: (u32, u32),
    /// `m_dwShakeTime` of the main character.
    shake_until: f32,
    /// The shake added to the drawn position this frame.
    shake_offset: Vec3,
}

impl Default for PlayerControl {
    fn default() -> Self {
        PlayerControl {
            yaw: 0.0,
            target_end: 0.0,
            smart_held: false,
            camera_anchor: None,
            camera_dragged: 0.0,
            poly_race: 0,
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
            reserved_talk: None,
            reserved_item: None,
            victims: Vec::new(),
            pre_input: false,
            combo_type: 0,
            attack_rotation: 0.0,
            hits: battle::HitTracker::default(),
            splash: None,
            motion_skill: 0,
            dead: false,
            hover: None,
            main_vid: 0,
            base: (0, 0),
            shake_until: 0.0,
            shake_offset: Vec3::ZERO,
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

    /// Destination terrain, streamed objects, and the player's model are ready.
    pub fn assets_ready(&self) -> bool {
        matches!(&self.state, Some(State::Ready { scene, character, .. })
            if scene.pending_objects() == 0 && (character.is_ready() || character.error.is_some()))
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
        self.scene().map_or(0.0, |scene| scene.ground_height(x, y))
    }

    /// The map's local extent in world units.
    pub fn extent(&self) -> Vec2 {
        self.scene().map_or(Vec2::ZERO, |scene| {
            let (x, y) = scene.map.map.terrain_count;
            Vec2::new(f32::from(x) * TERRAIN_WORLD, f32::from(y) * TERRAIN_WORLD)
        })
    }
}

#[derive(Resource, Default)]
struct ScreenWave { until: f32, power: f32 }

pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameWorld>()
            .init_resource::<WarpTransition>()
            .init_resource::<PlayerControl>()
            .init_resource::<Actors>()
            .init_resource::<FlyTargets>()
            .init_resource::<MobCatalog>()
            .init_resource::<ArmorShapes>()
            .init_resource::<UiFocus>()
            .init_resource::<TargetBoard>()
            .init_resource::<GradeNames>()
            .init_resource::<CombatFx>()
            .init_resource::<ScreenWave>()
            .add_message::<EnterWorld>()
            .add_message::<combat::UseSkillRequest>()
            .add_message::<DamageShown>()
            .add_message::<ClearTarget>()
            .add_systems(
                Update,
                (
                    update_warp_transition,
                    clear_world_on_warp,
                    start_entering,
                    track_actors,
                    track_battle,
                    sync_actor_entities,
                    drive_world.in_set(GameControl),
                    apply_stealth,
                    show_damage,
                    CombatFx::update,
                    fx::play_sounds.run_if(resource_exists::<m2_audio::SoundState>),
                )
                    .chain(),
            )
            .init_resource::<NameTags>()
            .init_resource::<MiniMap>()
            .add_systems(PostUpdate, (update_name_tags.after(bevy::transform::TransformSystems::Propagate), update_minimap));
    }
}

fn update_warp_transition(mut transition: ResMut<WarpTransition>, mut messages: MessageReader<NetMessage>, game: Res<GameWorld>) {
    for message in messages.read() {
        match message {
            NetMessage::Game(NetEvent::Warp { .. }) => {
                transition.started = Some(Instant::now());
                transition.cleared = false;
                transition.fading_in = None;
            }
            NetMessage::ConnectFailed { .. } | NetMessage::Disconnected { .. } | NetMessage::Game(NetEvent::LoginFailure { .. })
                if transition.started.is_some() => transition.fading_in = Some(Instant::now()),
            _ => {}
        }
    }
    if transition.started.is_some() && transition.cleared && transition.fading_in.is_none()
        && (game.assets_ready() || game.error().is_some())
    {
        transition.fading_in = Some(Instant::now());
    }
    if transition.fading_in.is_some_and(|start| start.elapsed() >= Duration::from_secs_f32(WARP_FADE_SECONDS)) {
        *transition = WarpTransition::default();
    }
}

/// Discard the previous map and actors once the curtain is opaque.
fn clear_world_on_warp(world: &mut World) {
    let transition = world.resource::<WarpTransition>();
    if transition.cleared || transition.fading_in.is_some() || !transition.started.is_some_and(|start| start.elapsed() >= Duration::from_secs_f32(WARP_FADE_SECONDS)) {
        return;
    }
    world.resource_mut::<WarpTransition>().cleared = true;
    let mut game = world.remove_resource::<GameWorld>().unwrap();
    if let Some(State::Ready { scene, character, .. }) = game.state.take() {
        world.despawn(scene.root);
        world.despawn(character.root);
    }
    world.insert_resource(GameWorld::default());
    let mut actors = world.remove_resource::<Actors>().unwrap();
    for (_, entity) in actors.entities.drain() {
        world.despawn(entity);
    }
    for (_, character) in actors.characters.drain() {
        if world.get_entity(character.root).is_ok() {
            world.despawn(character.root);
        }
    }
    world.insert_resource(Actors::default());
    world.resource_mut::<FlyTargets>().0.clear();
    world.resource_mut::<CombatFx>().clear_scene();
    crate::ground::GroundItems::clear_scene(world);
    *world.resource_mut::<PlayerControl>() = PlayerControl::default();
    *world.resource_mut::<TargetBoard>() = TargetBoard::default();
    crate::render::sky::clear_sky(world);
}

/// Starts loading the map for an [`EnterWorld`] request.
#[allow(clippy::too_many_arguments)]
fn start_entering(
    mut commands: Commands,
    mut messages: MessageReader<EnterWorld>,
    mut world: ResMut<GameWorld>,
    packs: Option<Res<M2Packs>>,
    mut mobs: ResMut<MobCatalog>,
    mut armor_shapes: ResMut<ArmorShapes>,
    mut actors: ResMut<Actors>,
    mut grades: ResMut<GradeNames>,
    transition: Res<WarpTransition>,
    mut pending: Local<Option<EnterWorld>>,
) {
    let Some(request) = messages.read().last().copied().or_else(|| pending.take()) else { return };
    if world.state.is_some() {
        if transition.started.is_some() {
            *pending = Some(request);
        }
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
        commands.entity(character.root).try_despawn();
    }
    if let Some(entity) = actors.entities.remove(&request.vid) {
        commands.entity(entity).try_despawn();
    }
    info!("entering the world at {},{}", request.x, request.y);
    let packs = packs.packs.clone();
    match ArmorShapes::load(&packs, LOCALE_PATH) {
        Ok(shapes) => *armor_shapes = shapes,
        Err(error) => warn!("could not load {LOCALE_PATH}/item_proto for actor armor: {error}"),
    }
    // `localeInfo.TARGET_LEVEL_*` (`locale_game.txt`): the target board's
    // grade names, by `nonplayer.PAWN`..`KING`.
    if let Ok(data) = packs.get(format!("{LOCALE_PATH}/locale_game.txt").as_bytes()) {
        let keys = ["PAWN", "S_PAWN", "KNIGHT", "S_KNIGHT", "BOSS", "KING"];
        for line in String::from_utf8_lossy(&data).lines() {
            let mut fields = line.splitn(2, '\t');
            let (Some(key), Some(value)) = (fields.next(), fields.next()) else { continue };
            if let Some(grade) = key.strip_prefix("TARGET_LEVEL_").and_then(|k| keys.iter().position(|x| *x == k)) {
                grades.0[grade] = value.trim().to_owned();
            }
        }
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
                        mob.rank = table.rank;
                    })
                    .or_insert_with(|| MobAppearance {
                        name: proto_text(&table.locale_name),
                        level: table.level,
                        rank: table.rank,
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
            (race, MobAppearance { name, level: 0, rank: 0, race_file: file, motions })
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
        let file = paths::join(folder, file);
        // `NAME`, `NAME1`, `NAME2`...: the same motion with variants.
        let base = kind.trim_end_matches(|c: char| c.is_ascii_digit());
        let target = match base {
            "WAIT" => &mut motions.wait,
            "WALK" => &mut motions.walk,
            "RUN" => &mut motions.run,
            "NORMAL_ATTACK" => {
                motions.attack.push(file);
                continue;
            }
            "FRONT_DAMAGE" => &mut motions.damage,
            "BACK_DAMAGE" => &mut motions.damage_back,
            "FRONT_KNOCKDOWN" => &mut motions.knockdown,
            "BACK_KNOCKDOWN" => &mut motions.knockdown_back,
            "FRONT_STANDUP" => &mut motions.stand_up,
            "BACK_STANDUP" => &mut motions.stand_up_back,
            "FRONT_DEAD" | "DEAD" => &mut motions.dead,
            "BACK_DEAD" => &mut motions.dead_back,
            "SPECIAL" => {
                // `SPECIAL` is NAME_SPECIAL_1, `SPECIAL1` NAME_SPECIAL_2...
                let index = kind[7..].parse::<usize>().unwrap_or(0);
                if let Some(slot) = motions.special.get_mut(index) {
                    slot.get_or_insert(file);
                }
                continue;
            }
            _ => continue,
        };
        target.get_or_insert(file);
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
    time: Res<Time>,
    net: Option<Res<Net>>,
) {
    // `ELTimer_GetServerFrameMSec`.
    let server_now = net.as_ref().map(|net| net.server_ms());
    // Actors appended before `mob_proto` loaded get its name and level now.
    if !mobs.by_race.is_empty() {
        for actor in actors.by_vid.values_mut().filter(|actor| actor.name.is_empty() && actor.kind != m2_net::phase::ACTOR_TYPE_PC) {
            if let Some(mob) = mobs.by_race.get(&actor.race) {
                actor.name.clone_from(&mob.name);
                if actor.level == 0 {
                    actor.level = u32::from(mob.level);
                }
            }
        }
    }
    for message in messages.read() {
        let NetMessage::Game(event) = message else { continue };
        match event {
            NetEvent::ActorAppend(actor) => {
                if actor.vid == game.main_vid {
                    continue;
                }
                if let Some(entity) = actors.entities.remove(&actor.vid) {
                    commands.entity(entity).try_despawn();
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
                // `CInstanceBase::Create` of a dead actor: `DieEnd`.
                let dead = actor.state_flags & combat::STATE_DEAD != 0;
                actors.fights.insert(
                    actor.vid,
                    ActorFight { dead, pending: dead.then_some(ActorAction::Dead), ..Default::default() },
                );
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
                        rendered_rotation: actor.rotation,
                        armor: actor.armor,
                        weapon: actor.weapon,
                        hair: actor.hair,
                        move_speed: actor.move_speed,
                        attack_speed: actor.attack_speed,
                        walk_mode: false,
                        empire: actor.empire,
                        pk_mode: actor.pk_mode,
                        guild_id: actor.guild_id,
                        alignment: actor.alignment,
                        state_flags: actor.state_flags,
                        affect_flags: actor.affect_flags,
                        mount_vnum: actor.mount_vnum,
                        move_from: Vec2::new(actor.x as f32, actor.y as f32),
                        move_to: Vec2::new(actor.x as f32, actor.y as f32),
                        moving: false,
                        rendered_moving: None,
                        move_action: None,
                        remote: RemoteMotion { position: Vec2::new(actor.x as f32, actor.y as f32), ..RemoteMotion::default() },
                    },
                );
            }
            NetEvent::ActorMove(motion) => {
                if let Some(actor) = actors.by_vid.get_mut(&motion.vid) {
                    // `CInstanceBase::PushTCPState` (every actor but the main one): replayed at the sender's
                    // time plus the smoothed network gap (`StateProcess`).
                    let remote = &mut actor.remote;
                    let check = match server_now {
                        Some(now) => {
                            let gap = now.wrapping_sub(motion.time) as i32;
                            remote.network_gap = (remote.network_gap * 70 + gap * 30) / 100;
                            motion.time.wrapping_add(remote.network_gap as u32)
                        }
                        None => 0,
                    };
                    remote.commands.push_back((check, motion.clone()));
                }
            }
            NetEvent::ActorOwner { owner_vid, victim_vid } => {
                // `RecvOwnership` -> `NEW_SetOwner`.
                let fight = actors.fights.entry(*victim_vid).or_default();
                fight.owner = *owner_vid;
                fight.owner_since = time.elapsed_secs();
            }
            NetEvent::Packet(m2_net::wire::GcPacket::WalkMode(packet)) => {
                if let Some(actor) = actors.by_vid.get_mut(&packet.vid) {
                    let walking = packet.mode != 0;
                    if actor.walk_mode != walking {
                        actor.walk_mode = walking;
                        actor.rendered_moving = None;
                    }
                }
            }
            NetEvent::ActorSync { vid, x, y } => {
                // `SyncActor` -> `NEW_SyncPixelPosition` -> `TEMP_Push`/`__Push`:
                // the actor is blended to the synced position like a push (eased
                // out over the physics blend time), not snapped there. A push in
                // progress towards the same point (our own hit's sync echoed
                // back) keeps running.
                if let Some(actor) = actors.by_vid.get_mut(vid) {
                    let target = Vec2::new(*x as f32, *y as f32);
                    if actor.remote.push.is_none_or(|push| push.end.distance(target) >= 1.0) {
                        actor.remote.push = Some(PushBlend::new(actor.remote.position, target));
                    }
                }
            }
            NetEvent::ActorUpdate(update) => {
                let appearance_changed = if let Some(actor) = actors.by_vid.get_mut(&update.vid) {
                    let changed = actor.kind == m2_net::phase::ACTOR_TYPE_PC
                        && (actor.armor != update.armor || actor.hair != update.hair);
                    actor.armor = update.armor;
                    actor.weapon = update.weapon;
                    actor.hair = update.hair;
                    actor.move_speed = update.move_speed;
                    actor.attack_speed = update.attack_speed;
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
                        commands.entity(character.root).try_despawn();
                    }
                    actors.failed_visuals.remove(&update.vid);
                }
            }
            NetEvent::ActorRemove { vid } => {
                actors.by_vid.remove(vid);
                actors.fights.remove(vid);
                actors.characters.remove(vid);
                actors.failed_visuals.remove(vid);
                if let Some(entity) = actors.entities.remove(vid) {
                    commands.entity(entity).try_despawn();
                }
            }
            NetEvent::Packet(m2_net::wire::GcPacket::Mount(packet)) => {
                if let Some(actor) = actors.by_vid.get_mut(&packet.vid) {
                    actor.mount_vnum = packet.mount_vid;
                }
            }
            _ => {}
        }
    }
}

/// How far above an actor's feet damage numbers start (`GetHeight`, roughly a
/// character's bounding box).
const DAMAGE_HEIGHT: f32 = 190.0;

/// The server's battle packets: `RecvTargetPacket` (the target board),
/// `RecvDamageInfoPacket` (damage numbers), `RecvDeadPacket` and
/// `RecvStunPacket`; and the target board's close button.
#[allow(clippy::too_many_arguments)]
fn track_battle(
    mut messages: MessageReader<NetMessage>,
    mut clear: MessageReader<ClearTarget>,
    mut actors: ResMut<Actors>,
    mut control: ResMut<PlayerControl>,
    mut board: ResMut<TargetBoard>,
    mut damage: MessageWriter<DamageShown>,
    game: Res<GameWorld>,
    mobs: Res<MobCatalog>,
    grades: Res<GradeNames>,
    mut net: Option<ResMut<Net>>,
    mut fx: ResMut<CombatFx>,
    mut fly_targets: ResMut<FlyTargets>,
    packs: Option<Res<M2Packs>>,
) {
    use m2_net::wire::GcPacket;
    if clear.read().last().is_some() {
        control.target = None;
        board.visible = false;
        if let Some(net) = net.as_deref_mut() {
            net.send_target(0);
        }
    }
    let base = (game.base.0 as i32, game.base.1 as i32);
    for message in messages.read() {
        let NetMessage::Game(NetEvent::Packet(packet)) = message else { continue };
        match packet {
            GcPacket::PlayerPointChange(p) if p.Type == combat::point::LEVEL as u8 || p.Type == 25 => {
                // `ShowPointEffect`: level and level-step packets display
                // effects at the actor; neither one initiates a map transfer.
                let vid = if p.dwVID == u32::MAX { game.main_vid } else { p.dwVID };
                let local = if vid == game.main_vid {
                    game.local.map(|(x, y)| Vec2::new(x as f32, y as f32))
                } else {
                    actors.by_vid.get_mut(&vid).map(|actor| {
                        if p.Type == combat::point::LEVEL as u8 {
                            actor.level = p.value.max(0) as u32;
                        }
                        Vec2::new((actor.x - base.0) as f32, (actor.y - base.1) as f32)
                    })
                };
                if let Some(local) = local {
                    let file = if p.Type == combat::point::LEVEL as u8 {
                        "d:/ymir work/effect/etc/levelup_1/level_up.mse"
                    } else {
                        "d:/ymir work/effect/etc/skillup/skillup_1.mse"
                    };
                    let at = Vec3::new(local.x, -local.y, game.terrain_height(local.x, local.y));
                    fx.spawn(packs.as_deref(), file, at, Vec3::ZERO);
                }
            }
            GcPacket::FlyTargeting(p) | GcPacket::AddFlyTargeting(p) => {
                let target = if p.dwTargetVID != 0 && (p.dwTargetVID == game.main_vid || actors.by_vid.contains_key(&p.dwTargetVID)) {
                    ShotTarget::Actor(p.dwTargetVID)
                } else {
                    let x = (p.lX - base.0) as f32;
                    let y = (p.lY - base.1) as f32;
                    ShotTarget::Position(Vec3::new(x, -y, game.terrain_height(x, y) + 60.0))
                };
                fly_targets.set(p.dwShooterVID, target, matches!(packet, GcPacket::AddFlyTargeting(_)));
            }
            GcPacket::Target(p) => {
                let actor = actors.by_vid.get(&p.dwVID);
                let alive = actor.is_some_and(|actor| {
                    actor.state_flags & combat::STATE_DEAD == 0 && !actors.fights.get(&p.dwVID).is_some_and(|f| f.dead)
                });
                match actor.filter(|_| alive) {
                    Some(actor) => {
                        let view = actor.combat(p.dwVID, game.base);
                        if view.is_pc() || view.is_building() {
                            // `CloseTargetBoardIfDifferent`.
                            if board.vid != p.dwVID {
                                board.visible = false;
                            }
                        } else if view.is_stone() || view.is_enemy() || view.is_door() {
                            // `CanViewTargetHP` -> `SetHPTargetBoard`.
                            if board.vid != p.dwVID || board.hp_percent.is_none() {
                                let mob = mobs.by_race.get(&actor.race);
                                let mut name = format!("Lv.{} ", actor.level);
                                if let Some(grade) = mob.and_then(|mob| grades.0.get(usize::from(mob.rank)))
                                    && !grade.is_empty()
                                {
                                    name.push_str(&format!("({grade}) "));
                                }
                                name.push_str(&actor.name);
                                *board = TargetBoard { vid: p.dwVID, name, hp_percent: None, visible: true };
                            }
                            board.hp_percent = Some(p.bHPPercent.min(100));
                            board.visible = true;
                        } else {
                            board.visible = false;
                        }
                    }
                    None => board.visible = false,
                }
            }
            GcPacket::SepcialEffect(p) => {
                // `RecvSpecialEffect`: the registered effect at the actor; the
                // potions also play `USESOUND_POTION` for the main character.
                let Some((file, potion)) = fx::special_effect(p.r#type) else { continue };
                let own = p.vid == game.main_vid;
                let local = if own {
                    game.local.map(|(x, y)| Vec2::new(x as f32, y as f32))
                } else {
                    actors
                        .by_vid
                        .get(&p.vid)
                        .map(|actor| Vec2::new((actor.x - base.0) as f32, (actor.y - base.1) as f32))
                };
                let Some(local) = local else { continue };
                let at = Vec3::new(local.x, -local.y, game.terrain_height(local.x, local.y));
                fx.spawn(packs.as_deref(), file, at, Vec3::ZERO);
                if potion && own {
                    fx.sound(fx::bevy_position(at), fx::USESOUND_POTION, false);
                }
            }
            GcPacket::CreateFly(p) => {
                // `RecvCreateFlyPacket` -> `CreateIndexedFly`: from the start
                // actor's feet towards the end actor (the experience orbs of
                // a kill fly from the monster to the killer).
                let Some(file) = fx::indexed_fly(p.bType) else { continue };
                let at = |vid: u32| {
                    let local = if vid == game.main_vid {
                        game.local.map(|(x, y)| Vec2::new(x as f32, y as f32))
                    } else {
                        actors
                            .by_vid
                            .get(&vid)
                            .map(|actor| Vec2::new((actor.x - base.0) as f32, (actor.y - base.1) as f32))
                    }?;
                    Some(Vec3::new(local.x, -local.y, game.terrain_height(local.x, local.y)))
                };
                let (Some(start), Some(end)) = (at(p.dwStartVID), at(p.dwEndVID)) else { continue };
                fx.fly(packs.as_deref(), file, start, p.dwEndVID, end + Vec3::Z * fx::FLY_TARGET_HEIGHT);
            }
            GcPacket::DamageInfo(p) => {
                if p.damage < 0 {
                    continue;
                }
                let own = p.dwVID == game.main_vid;
                let local = if own {
                    game.local.map(|(x, y)| Vec2::new(x as f32, y as f32))
                } else {
                    actors
                        .by_vid
                        .get(&p.dwVID)
                        .map(|actor| Vec2::new((actor.x - base.0) as f32, (actor.y - base.1) as f32))
                };
                let Some(local) = local else { continue };
                let height = game.terrain_height(local.x, local.y) + DAMAGE_HEIGHT;
                damage.write(DamageShown {
                    position: local_to_world(local.x, local.y, height),
                    damage: p.damage,
                    flag: p.flag,
                    own,
                    target: board.vid == p.dwVID,
                });
            }
            // `RecvDigMotionPacket`: face the vein, then dig `count` times.
            GcPacket::DigMotion(p) => {
                let target = actors.by_vid.get(&p.target_vid).map(|actor| Vec2::new(actor.x as f32, actor.y as f32));
                if let Some(actor) = actors.by_vid.get_mut(&p.vid) {
                    if let Some(target) = target {
                        actor.rotation = facing_degrees(target - Vec2::new(actor.x as f32, actor.y as f32));
                    }
                    actors.fights.entry(p.vid).or_default().pending = Some(ActorAction::Dig(p.count));
                }
            }
            // `RecvFishing`: `StartFishing` turns to `dir * 5` degrees and
            // throws; the others end the cast (`IsFishing` guards stop/react).
            GcPacket::Fishing(p) if p.subheader != crate::social::fishing::FISH => {
                use crate::social::fishing;
                let actors = &mut *actors;
                let Some(actor) = actors.by_vid.get_mut(&p.info) else { continue };
                let fight = actors.fights.entry(p.info).or_default();
                let fishing_now = fight.fishing;
                let action = match p.subheader {
                    fishing::START => {
                        actor.rotation = (f32::from(p.dir) * 5.0).rem_euclid(360.0);
                        fight.fishing = true;
                        Some((FishingMotion::Throw, true))
                    }
                    fishing::STOP if fishing_now => Some((FishingMotion::Stop, false)),
                    fishing::REACT if fishing_now => Some((FishingMotion::React, true)),
                    fishing::SUCCESS => Some((FishingMotion::Catch, false)),
                    fishing::FAIL => Some((FishingMotion::Fail, false)),
                    _ => None,
                };
                if let Some((motion, fishing)) = action {
                    fight.fishing = fishing;
                    fight.pending = Some(ActorAction::Fishing { motion, fishing });
                }
            }
            // `RecvSpecificEffect`: an effect file attached to the actor.
            GcPacket::SpecificEffect(p) => {
                let local = if p.vid == game.main_vid {
                    game.local.map(|(x, y)| Vec2::new(x as f32, y as f32))
                } else {
                    actors.by_vid.get(&p.vid).map(|actor| Vec2::new((actor.x - base.0) as f32, (actor.y - base.1) as f32))
                };
                let file = m2_net::wire::cstr_to_string(&p.effect_file);
                if let Some(local) = local
                    && !file.is_empty()
                {
                    let at = Vec3::new(local.x, -local.y, game.terrain_height(local.x, local.y));
                    fx.spawn(packs.as_deref(), &file, at, Vec3::ZERO);
                }
            }
            GcPacket::Dead(p) => {
                if p.vid == game.main_vid {
                    // `OnGameOver` + `NotifyDeadMainCharacter`.
                    control.dead = true;
                    control.reserved_attack = None;
                    control.reserved_skill = None;
                    control.destination = None;
                    continue;
                }
                let fight = actors.fights.entry(p.vid).or_default();
                if !fight.dead {
                    fight.dead = true;
                    fight.attacking = false;
                    fight.pending = Some(ActorAction::Dead);
                }
                if let Some(actor) = actors.by_vid.get_mut(&p.vid) {
                    actor.state_flags |= combat::STATE_DEAD;
                    actor.moving = false;
                }
                // `NotifyCharacterDead`: a dead target is dropped.
                if control.target == Some(p.vid) {
                    control.target = None;
                    board.visible = false;
                    if let Some(net) = net.as_deref_mut() {
                        net.send_target(0);
                    }
                }
            }
            GcPacket::Stun(p) => {
                if p.vid == game.main_vid {
                    control.dead = true;
                } else {
                    actors.fights.entry(p.vid).or_default().stunned = true;
                }
            }
            _ => {}
        }
    }
    // `SetTarget` on a PC opens the board with its name (`SetPCTargetBoard`);
    // losing the target closes it.
    match control.target {
        None => board.visible = false,
        Some(vid) if vid != board.vid => {
            if let Some(actor) = actors.by_vid.get(&vid).filter(|actor| actor.kind == m2_net::phase::ACTOR_TYPE_PC) {
                *board = TargetBoard { vid, name: actor.name.clone(), hp_percent: None, visible: true };
            } else {
                board.visible = false;
            }
        }
        Some(_) => {}
    }
}

/// `DAMAGE_DODGE` / `DAMAGE_BLOCK` (`CInstanceBase::EDamage`).
const DAMAGE_DODGE: u8 = 1 << 2;
const DAMAGE_BLOCK: u8 = 1 << 3;
/// `FONT_WIDTH` of `ProcessDamage`: the spacing of the damage digits.
const DAMAGE_FONT_WIDTH: f32 = 30.0;
const DAMAGE_VALUE_DIR: &str = "d:/ymir work/effect/affect/damagevalue/";

/// `CInstanceBase::ProcessDamage`: misses show `miss`/`target_miss`, damage
/// one `damage`/`target` effect per digit (units first, each further digit
/// 30 units to the camera's left) drawing that digit's texture. Damage to
/// actors other than the main character and its target is not shown.
fn show_damage(
    mut shown: MessageReader<DamageShown>,
    mut fx: ResMut<CombatFx>,
    packs: Option<Res<M2Packs>>,
    camera: Query<&GlobalTransform, With<SceneCamera>>,
    mut alternate: Local<bool>,
) {
    let Ok(camera) = camera.single() else {
        shown.clear();
        return;
    };
    let right = Vec3::from_array(m2_assets::coords::vec3_back(*camera.right()));
    let forward = Vec3::from_array(m2_assets::coords::vec3_back(*camera.forward()));
    // `GetDegreeFromPosition2(target, eye)`: the effect faces the camera.
    let angle = (-forward.x).atan2(forward.y).to_degrees();
    let rotation = Vec3::new(0.0, 0.0, angle);
    for damage in shown.read() {
        let position = Vec3::from_array(m2_assets::coords::vec3_back(damage.position));
        if damage.flag & (DAMAGE_DODGE | DAMAGE_BLOCK) != 0 {
            let file = if damage.own { "miss.mse" } else { "target_miss.mse" };
            fx.spawn(packs.as_deref(), &format!("{DAMAGE_VALUE_DIR}{file}"), position, rotation);
            continue;
        }
        let (prefix, effect) = if damage.own {
            *alternate = !*alternate;
            ("damage_", if *alternate { "damage.mse" } else { "damage_1.mse" })
        } else if damage.target {
            ("target_", "target.mse")
        } else {
            continue;
        };
        let mut value = damage.damage.max(0) as u32;
        let mut index = 0;
        while value > 0 && index <= 7 {
            let digit = value % 10;
            value /= 10;
            let texture = format!("{DAMAGE_VALUE_DIR}{prefix}{digit}.dds");
            let at = position - right * DAMAGE_FONT_WIDTH * index as f32;
            fx.spawn_with_texture(packs.as_deref(), &format!("{DAMAGE_VALUE_DIR}{effect}"), &texture, at, rotation);
            index += 1;
        }
    }
}

/// Keeps the Bevy actor transforms in map-local coordinates as server movement
/// and synchronization packets arrive. Height is sampled from the loaded map.
fn sync_actor_entities(world: &mut World) {
    let delta = world.resource::<Time>().delta_secs().min(0.1);
    let server_now = world.get_resource::<Net>().map(Net::server_ms);
    advance_actor_states(&mut world.resource_mut::<Actors>(), delta, server_now);
    if !world.resource::<GameWorld>().is_ready() {
        return;
    }
    // Borrow the catalog rather than copying every race's motion paths on
    // every frame, including races that are not present in the scene.
    world.resource_scope(|world, mobs: Mut<MobCatalog>| {
        sync_actor_visuals(world, &mobs.by_race);
    });
}

/// `CInstanceBase::Update` for the remote actors: the rotation blend, the
/// physics push, `StateProcess` (due commands) and `MovementProcess`.
fn advance_actor_states(actors: &mut Actors, delta: f32, server_now: Option<u32>) {
    let mut arrived_actions = Vec::new();
    for (&vid, actor) in &mut actors.by_vid {
        let turn = signed_difference(actor.rotation, actor.rendered_rotation);
        let rate = if turn.abs() > 45.0 { MAX_ROTATION_SPEED } else { MAX_ROTATION_SPEED * 5.0 / 12.0 };
        actor.rendered_rotation = (actor.rendered_rotation + turn.clamp(-rate * delta, rate * delta)).rem_euclid(360.0);
        // `CPhysicsObject::Update`: the push moves the actor on its own,
        // whatever it is doing (`m_x += GetXMovement()`).
        if let Some(push) = &mut actor.remote.push {
            let step = push.step(delta);
            let done = push.done();
            actor.remote.position += step;
            actor.x = actor.remote.position.x.round() as i32;
            actor.y = actor.remote.position.y.round() as i32;
            if done {
                actor.remote.push = None;
            }
        }
        // `__CanProcessNetworkStatePacket`: the dead keep their commands queued.
        let locked = actors.fights.get(&vid).is_some_and(|fight| {
            fight.dead || (fight.using_skill && !fight.skill_cancelable) || fight.knocked_down || matches!(fight.pending, Some(ActorAction::Knockdown { .. }))
        });
        if !locked {
            while let Some((check, _)) = actor.remote.commands.front() {
                if server_now.is_some_and(|now| (now.wrapping_sub(*check) as i32) < 0) {
                    break;
                }
                let (_, command) = actor.remote.commands.pop_front().expect("front exists");
                if let Some(action) = remote_state_process(actor, &command) {
                    arrived_actions.push((vid, action));
                }
            }
        }
        if !locked && let Some(action) = remote_movement_process(actor, delta) {
            arrived_actions.push((vid, action));
        }
    }
    for (vid, action) in arrived_actions {
        let fight = actors.fights.entry(vid).or_default();
        if !fight.dead {
            fight.pending = Some(action);
        }
    }
}

fn sync_actor_visuals(world: &mut World, mobs: &HashMap<u16, MobAppearance>) {
    let now = world.resource::<Time>().elapsed_secs();
    let delta = world.resource::<Time>().delta_secs().min(0.1);
    world.resource_scope(|world, mut actors: Mut<Actors>| {
        let motions = world.resource::<Assets<m2_assets::race::RaceMotionAsset>>();
        let steps: Vec<_> = actors.characters.iter().filter_map(|(&vid, character)| {
            let fight = actors.fights.get(&vid)?;
            if !fight.attacking || fight.dead || (character.is_mounted() && !fight.using_skill) { return None; }
            let elapsed = character.motion_elapsed(now)?;
            let motion = motions.get(character.motion_handle()?)?;
            let accumulation = motion.accumulation?;
            if motion.duration <= 0.0 || elapsed >= motion.duration { return None; }
            let rotation = actors.by_vid.get(&vid)?.rotation;
            let step = Vec3::from_array(m2_assets::coords::vec3_back(accumulation)) * delta.min(elapsed) / motion.duration;
            let step = battle::rotate_local(step, rotation);
            Some((vid, Vec2::new(step.x, -step.y)))
        }).collect();
        for (vid, step) in steps {
            if let Some(actor) = actors.by_vid.get_mut(&vid) {
                actor.remote.position += step;
                actor.x = actor.remote.position.x.round() as i32;
                actor.y = actor.remote.position.y.round() as i32;
            }
        }
    });
    let (base, heights) = {
        let actors = world.resource::<Actors>();
        let game = world.resource::<GameWorld>();
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
        (base, heights)
    };
    world.resource_scope(|world, actors: Mut<Actors>| {
        let mut query = world.query::<(&mut ActorEntity, &mut Transform)>();
        for (mut actor_entity, mut transform) in query.iter_mut(world) {
            let Some(actor) = actors.by_vid.get(&actor_entity.vid) else { continue };
            let local_x = (actor.x - base.0 as i32) as f32;
            let local_y = (actor.y - base.1 as i32) as f32;
            let height = heights.get(&actor_entity.vid).copied().unwrap_or_default();
            let translation = local_to_world(local_x, local_y, height);
            let rotation = Quat::from_rotation_y(-actor.rendered_rotation.to_radians());
            if transform.translation != translation || transform.rotation != rotation {
                transform.translation = translation;
                transform.rotation = rotation;
            }
            if actor_entity.name != actor.name || actor_entity.armor != actor.armor || actor_entity.hair != actor.hair {
                actor_entity.name.clone_from(&actor.name);
                actor_entity.armor = actor.armor;
                actor_entity.hair = actor.hair;
            }
        }
    });

    let pending: Vec<(u32, Entity, Actor)> = {
        let actors = world.resource::<Actors>();
        actors.entities.iter()
            .filter(|(vid, _)| !actors.characters.contains_key(*vid) && !actors.failed_visuals.contains(*vid))
            .filter_map(|(&vid, &entity)| actors.by_vid.get(&vid).cloned().map(|actor| (vid, entity, actor)))
            .collect()
    };
    for (vid, parent, actor) in pending {
        let character = if actor.has_player_body() {
            let shapes = world.resource::<ArmorShapes>();
            let (shape, specular) = (shapes.shape(u32::from(actor.armor)), shapes.specular(u32::from(actor.armor)));
            let mut character = Character::spawn(
                world,
                Look { race: actor.race as u8, shape, hair: u32::from(actor.hair) },
                Transform::IDENTITY,
                Some(parent),
            );
            character.set_armor_specular(specular);
            character
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

    world.resource_scope(|_world, mut actors: Mut<Actors>| {
        let rates: Vec<_> = actors.by_vid.iter().map(|(&vid, actor)| (vid, actor.move_speed)).collect();
        for (vid, speed) in rates {
            let rate = if speed == 0 { 1.0 } else { f32::from(speed) / 100.0 };
            if actors.characters.get_mut(&vid).is_some_and(|character| character.set_movement_rate(rate))
                && let Some(actor) = actors.by_vid.get_mut(&vid) && actor.moving
            {
                actor.rendered_moving = None;
            }
        }
    });

    sync_remote_equipment(world, mobs);
    let rendered: HashSet<u32> = world.resource::<Actors>().characters.keys().copied().collect();
    let now = world.resource::<Time>().elapsed_secs();

    // Battle motions: a pending action plays once, then the actor's wait or
    // run; a death holds its last frame.
    let actions: Vec<(u32, ActorAction, Actor)> = {
        let mut actors = world.resource_mut::<Actors>();
        let actors = &mut *actors;
        actors
            .fights
            .iter_mut()
            .filter(|(vid, _)| rendered.contains(*vid))
            .filter_map(|(vid, fight)| {
                let action = fight.pending.take()?;
                let actor = actors.by_vid.get_mut(vid)?;
                fight.attacking = matches!(action, ActorAction::Attack | ActorAction::Combo(_) | ActorAction::Special(_) | ActorAction::Skill { .. });
                fight.using_skill = matches!(action, ActorAction::Skill { .. });
                fight.skill_cancelable = false;
                fight.knocked_down = matches!(action, ActorAction::Knockdown { .. });
                if matches!(action, ActorAction::Skill { moving: true, .. }) {
                    actor.moving = true;
                    actor.remote.going = true;
                    actor.remote.after_move = true;
                    actor.remote.dst_rotation = actor.rotation;
                    actor.move_from = actor.remote.position;
                    let heading = actor.rotation.to_radians();
                    actor.move_to = actor.move_from + Vec2::new(heading.sin(), heading.cos()) * RUN_SPEED;
                }
                fight.splash = None;
                fight.hits.clear();
                actor.rendered_moving = Some(actor.moving);
                Some((*vid, action, actor.clone()))
            })
            .collect()
    };
    for (vid, action, actor) in actions {
        world.resource_scope(|world, mut actors: Mut<Actors>| {
            let Some(character) = actors.characters.get_mut(&vid) else { return };
            let pc = actor.has_player_body();
            let fallback = ActorMotions::default();
            let motions = mobs.get(&actor.race).map(|mob| &mob.motions).unwrap_or(&fallback);
            let idle = if pc {
                character.motion_path(if actor.moving { if actor.walk_mode { Motion::Walk } else { Motion::Run } } else { Motion::Wait })
            } else if actor.moving {
                motions.run.clone().or(motions.walk.clone())
            } else {
                motions.wait.clone()
            };
            let pick = |list: &[String]| (!list.is_empty()).then(|| list[(now * 1000.0) as usize % list.len()].clone());
            let once = match action {
                ActorAction::Attack if pc => character.motion_path(Motion::Attack(1)),
                ActorAction::Attack => pick(&motions.attack),
                // `NAME_COMBO_ATTACK_1` is motion 14.
                ActorAction::Combo(motion) if pc => {
                    character.motion_path(Motion::Attack(motion.saturating_sub(13).max(1)))
                }
                ActorAction::Combo(_) => pick(&motions.attack),
                ActorAction::Special(n) => motions.special.get(usize::from(n)).cloned().flatten(),
                ActorAction::Skill { index, .. } if pc => crate::render::character::skill_motion_file_in(
                    actor.race as u8,
                    character.motion_mode(),
                    index,
                ),
                ActorAction::Skill { .. } => None,
                ActorAction::Damage { back } if pc => character.motion_path(Motion::Damage { back }),
                ActorAction::Damage { back } => {
                    if back { motions.damage_back.clone().or(motions.damage.clone()) } else { motions.damage.clone() }
                }
                ActorAction::Knockdown { back } if pc => character.motion_path(Motion::DamageFlying { back }),
                ActorAction::Knockdown { back } => {
                    if back { motions.knockdown_back.clone().or(motions.knockdown.clone()).or(motions.damage.clone()) } else { motions.knockdown.clone().or(motions.damage.clone()) }
                }
                ActorAction::Dead if pc => character.motion_path(Motion::Dead),
                ActorAction::Dead => motions.dead.clone(),
                ActorAction::Dig(_) if pc => character.motion_path(Motion::Dig),
                ActorAction::Fishing { motion, .. } if pc => character.motion_path(Motion::Fishing(motion)),
                ActorAction::Dig(_) | ActorAction::Fishing { .. } => None,
            };
            let Some(once) = once else { return };
            // A rider attacks or casts from a standing horse.
            if matches!(action, ActorAction::Attack | ActorAction::Combo(_) | ActorAction::Skill { moving: false, .. }) {
                character.idle_mount(world);
            }
            match action {
                ActorAction::Skill { loops, .. } => {
                    match idle.as_deref() {
                        Some(idle) => character.play_then(world, &once, idle),
                        None => character.play(world, &once, false),
                    }
                    let targets = world.get_resource::<FlyTargets>().and_then(|targets| targets.0.get(&vid)).map_or(1, VecDeque::len) as u32;
                    character.set_motion_loop_count(if targets > 1 { u32::from(loops).max(1) + targets - 1 } else { u32::from(loops) });
                }
                ActorAction::Dead => character.play(world, &once, false),
                ActorAction::Dig(count) => {
                    let mut then: Vec<(&str, bool)> = vec![(once.as_str(), false); usize::from(count.max(1)) - 1];
                    if let Some(idle) = idle.as_deref() {
                        then.push((idle, true));
                    }
                    character.play_sequence(world, &once, &then);
                }
                ActorAction::Fishing { fishing, .. } => {
                    let wait = if fishing { character.motion_path(Motion::Fishing(FishingMotion::Wait)) } else { idle.clone() };
                    match wait.as_deref() {
                        Some(wait) => character.play_then(world, &once, wait),
                        None => character.play(world, &once, false),
                    }
                }
                ActorAction::Knockdown { back } => {
                    let stand = if pc {
                        character.motion_path(Motion::StandUp { back })
                    } else if back {
                        motions.stand_up_back.clone().or(motions.stand_up.clone())
                    } else {
                        motions.stand_up.clone()
                    };
                    let mut then: Vec<(&str, bool)> = Vec::new();
                    if let Some(stand) = stand.as_deref() {
                        then.push((stand, false));
                    }
                    if let Some(idle) = idle.as_deref() {
                        then.push((idle, true));
                    }
                    character.play_sequence(world, &once, &then);
                }
                _ => match idle.as_deref() {
                    Some(idle) => character.play_then(world, &once, idle),
                    None => character.play(world, &once, false),
                },
            }
        });
    }

    // Wait/run follow the server's movement, unless a once motion runs or the
    // actor is dead.
    let motion_updates: Vec<(u32, Actor)> = world.resource_scope(|world, mut actors: Mut<Actors>| {
        let actors = &mut *actors;
        let busy: HashSet<u32> = actors
            .characters
            .iter()
            .filter(|(vid, character)| {
                character.is_acting(world) || actors.fights.get(*vid).is_some_and(|fight| fight.dead)
            })
            .map(|(vid, _)| *vid)
            .collect();
        for (vid, fight) in &mut actors.fights {
            if fight.using_skill {
                fight.skill_cancelable = actors.characters.get(vid).and_then(Character::motion_handle)
                    .and_then(|handle| world.resource::<Assets<m2_assets::race::RaceMotionAsset>>().get(handle))
                    .is_some_and(|motion| motion.cancel_enable_skill);
            }
            if !busy.contains(vid) {
                fight.attacking = false;
                fight.using_skill = false;
                fight.knocked_down = false;
                fight.splash = None;
            }
        }
        actors
            .by_vid
            .iter_mut()
            .filter_map(|(&vid, actor)| {
                if actor.rendered_moving == Some(actor.moving) || !rendered.contains(&vid) || busy.contains(&vid) {
                    return None;
                }
                actor.rendered_moving = Some(actor.moving);
                Some((vid, actor.clone()))
            })
            .collect()
    });
    for (vid, actor) in motion_updates {
        world.resource_scope(|world, mut actors: Mut<Actors>| {
            let Some(character) = actors.characters.get_mut(&vid) else { return };
            if actor.has_player_body() {
                character.play_motion(world, if actor.moving { if actor.walk_mode { Motion::Walk } else { Motion::Run } } else { Motion::Wait }, true);
            } else if let Some(motions) = mobs.get(&actor.race).map(|mob| &mob.motions) {
                let motion = if actor.moving {
                    if actor.walk_mode { motions.walk.as_ref().or(motions.run.as_ref()) } else { motions.run.as_ref().or(motions.walk.as_ref()) }
                } else { motions.wait.as_ref() };
                if let Some(motion) = motion {
                    character.play(world, motion, true);
                }
            }
        });
    }

    // `ShakeProcess`: a hit victim jitters by up to 10 units for 100 ms.
    {
        let shaking: Vec<(Entity, Vec3)> = {
            let actors = world.resource::<Actors>();
            actors
                .fights
                .iter()
                .filter(|(_, fight)| fight.shake_until > now)
                .filter_map(|(vid, fight)| {
                    let character = actors.characters.get(vid)?;
                    let seed = (now * 1000.0) as u32 ^ (fight.shake_until * 977.0) as u32 ^ vid;
                    Some((character.root, rest_offset(character) + shake_offset(seed)))
                })
                .collect()
        };
        let resting: Vec<(Entity, Vec3)> = world
            .resource::<Actors>()
            .characters
            .iter()
            .filter(|(vid, _)| !world.resource::<Actors>().fights.get(*vid).is_some_and(|fight| fight.shake_until > now))
            .map(|(_, character)| (character.root, rest_offset(character)))
            .collect();
        for (entity, offset) in shaking {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation = offset;
            }
        }
        for (entity, rest) in resting {
            if let Some(mut transform) = world.get_mut::<Transform>(entity)
                && transform.translation != rest
            {
                transform.translation = rest;
            }
        }
    }

    // Motion sounds and effects of the other actors (`SoundEventProcess` with
    // the frequency check, `MotionEventProcess`).
    let ticks: Vec<(u32, MotionTick)> = world.resource_scope(|world, mut actors: Mut<Actors>| {
        actors.characters.iter_mut().map(|(vid, character)| (*vid, character.take_motion_tick(world, now))).collect()
    });
    for (vid, tick) in ticks {
        let Some(mut actor) = world.resource::<Actors>().by_vid.get(&vid).cloned() else { continue };
        let local = Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32);
        let height = heights.get(&vid).copied().unwrap_or_default();
        let mut metin2 = Vec3::new(local.x, -local.y, height);
        let target = shot_target(world, vid, None, base, height, &tick);
        if tick.events.iter().any(|event| matches!(event.kind, m2_assets::race::RaceMotionEventKind::Warp))
            && let Some((_, target)) = target
        {
            let destination = skill_warp_position(metin2, target);
            let at = Vec2::new(destination.x, -destination.y);
            let game = world.resource::<GameWorld>();
            if !game.scene().is_some_and(|scene| scene.is_attr_on(at, ATTR_BLOCK)) {
                metin2 = Vec3::new(at.x, -at.y, game.terrain_height(at.x, at.y));
                let global = at + Vec2::new(base.0 as f32, base.1 as f32);
                actor.x = global.x.round() as i32;
                actor.y = global.y.round() as i32;
                actor.rotation = facing_degrees(Vec2::new(target.x - metin2.x, metin2.y - target.y));
                let entity = world.resource::<Actors>().entities.get(&vid).copied();
                if let Some(entity) = entity && let Some(mut transform) = world.get_mut::<Transform>(entity) {
                    transform.translation = fx::bevy_position(metin2);
                }
                if let Some(current) = world.resource_mut::<Actors>().by_vid.get_mut(&vid) {
                    current.x = actor.x;
                    current.y = actor.y;
                    current.rotation = actor.rotation;
                    current.remote.position = global;
                }
            }
        }
        let position = fx::bevy_position(metin2);
        for event in &tick.events {
            if let m2_assets::race::RaceMotionEventKind::SpecialAttacking(splash) = &event.kind {
                world.resource_mut::<Actors>().fights.entry(vid).or_default().splash = Some(battle::SplashArea::new(
                    splash, metin2, actor.rotation, now, event.duration, 0,
                ));
            }
        }
        world.resource_scope(|world, mut actors: Mut<Actors>| {
            let Some(character) = actors.characters.get_mut(&vid) else { return };
            motion_tick_effects(world, character, &tick, position, metin2, actor.rotation, true, target);
        });
    }

    world.resource_scope(|world, mut actors: Mut<Actors>| {
        for character in actors.characters.values_mut() {
            character.update(world);
        }
    });
}

/// `CInstanceBase::SetWeapon` / `ChangeWeapon` and mount changes for remote PCs.
fn sync_remote_equipment(world: &mut World, mobs: &HashMap<u16, MobAppearance>) {
    let equipment: Vec<(u32, u16, u32)> = {
        let actors = world.resource::<Actors>();
        actors.by_vid.iter().filter(|(_, actor)| actor.has_player_body())
            .map(|(&vid, actor)| (vid, actor.weapon, actor.mount_vnum)).collect()
    };
    for (vid, weapon, mount) in equipment {
        let needs_change = world.resource::<Actors>().characters.get(&vid).is_some_and(|character| {
            character.weapon_vnum() != u32::from(weapon) || character.mount_vnum() != mount
        });
        if !needs_change { continue; }
        let visual = if weapon == 0 { None } else {
            world.get_resource::<M2Packs>().and_then(|packs| {
                world.get_resource::<ItemCatalog>().and_then(|catalog| catalog.weapon_visual(&packs.packs, u32::from(weapon)))
            })
        };
        let mount_info = mobs.get(&(mount as u16));
        world.resource_scope(|world, mut actors: Mut<Actors>| {
            let Some(character) = actors.characters.get_mut(&vid) else { return };
            let mut changed = if weapon == 0 {
                character.clear_weapon(world)
            } else if let Some(visual) = &visual {
                character.set_weapon(world, visual.vnum, &visual.model, &visual.sub_model, visual.weapon_type, f32::from(visual.specular) / 100.0)
            } else {
                false
            };
            if mount == 0 {
                changed |= character.clear_mount(world);
            } else if let Some(mob) = mount_info && let Some(file) = mob.race_file.as_deref() {
                changed |= character.set_mount(world, mount, file, mob.motions.wait.as_deref(), mob.motions.run.as_deref());
            }
            if changed && let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                transform.translation.y = if character.is_mounted() { MOUNT_HEIGHT } else { 0.0 };
            }
            if changed && let Some(actor) = actors.by_vid.get_mut(&vid) {
                actor.rendered_moving = None;
            }
        });
    }
}

/// Where an actor's character root rests under its entity: lifted onto the
/// saddle height while mounted.
fn rest_offset(character: &Character) -> Vec3 {
    if character.is_mounted() { Vec3::Y * MOUNT_HEIGHT } else { Vec3::ZERO }
}

/// `ShakeProcess`'s random offset (−9..9 on each axis), as a Bevy vector.
fn shake_offset(seed: u32) -> Vec3 {
    let mut state = seed.wrapping_mul(0x9E37_79B9).wrapping_add(0x7F4A_7C15);
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state % 10) as f32 * if state & 0x100 == 0 { 1.0 } else { -1.0 }
    };
    let (x, y, z) = (next(), next(), next());
    fx::bevy_position(Vec3::new(x, y, z))
}

/// Resolve an actor's current server-provided fly target when its motion fires.
fn shot_target(world: &mut World, shooter: u32, fallback: Option<u32>, base: (u32, u32), height: f32, tick: &MotionTick) -> Option<(u32, Vec3)> {
    let shoots = tick.events.iter().any(|event| matches!(event.kind, m2_assets::race::RaceMotionEventKind::Fly { .. }));
    let target = if shoots { world.resource_mut::<FlyTargets>().shot(shooter) }
        else { world.resource::<FlyTargets>().peek(shooter) }
        .or(fallback.map(ShotTarget::Actor))?;
    match target {
        ShotTarget::Position(at) => Some((0, at)),
        ShotTarget::Actor(vid) => {
            let game = world.get_resource::<GameWorld>();
            let local = if game.is_some_and(|game| game.main_vid == vid) {
                let (x, y) = game?.local?;
                Vec2::new(x as f32, y as f32)
            } else {
                let actors = world.resource::<Actors>();
                let actor = actors.by_vid.get(&vid)?;
                Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32)
            };
            let ground = game.map_or(height, |game| game.terrain_height(local.x, local.y));
            Some((vid, Vec3::new(local.x, -local.y, ground + fx::FLY_TARGET_HEIGHT)))
        }
    }
}

/// Plays a motion tick's sounds and effect events for a character at
/// `position` (Bevy) / `metin2` with heading `rotation`.
/// `ProcessMotionEventEffectEvent`: independent and non-attached effects are
/// placed at the actor (offset by the event position), attached ones at their
/// bone.
fn motion_tick_effects(
    world: &mut World,
    character: &Character,
    tick: &MotionTick,
    position: Vec3,
    metin2: Vec3,
    _rotation: f32,
    check_frequency: bool,
    fly_target: Option<(u32, Vec3)>,
) {
    use m2_assets::race::RaceMotionEventKind;
    world.resource_scope(|world, mut fx: Mut<CombatFx>| {
        for sound in &tick.sounds {
            fx.sound(position, sound, check_frequency);
        }
        let packs = world.get_resource::<M2Packs>().cloned();
        for event in &tick.events {
            match &event.kind {
                RaceMotionEventKind::Effect { file, bone, position: offset, following, independent } => {
                    let entity = bone.as_deref().and_then(|name| character.bone_entity(name)).unwrap_or(character.root);
                    if !independent && (bone.is_none() || *following) {
                        fx.spawn_following(packs.as_ref(), file, entity, *offset);
                    } else {
                        let transform = world.get::<GlobalTransform>(if *independent { character.root } else { entity })
                            .map(GlobalTransform::to_matrix).unwrap_or(Mat4::from_translation(position));
                        let matrix = transform * Mat4::from_translation(*offset);
                        let swap = Mat4::from_mat3(m2_assets::coords::SWAP_YZ);
                        fx.spawn_matrix(packs.as_ref(), file, m2_effect::Mat4::from_array((swap * matrix * swap).to_cols_array()));
                    }
                }
                RaceMotionEventKind::EffectToTarget { file, position: offset, following } => {
                    let Some((vid, at)) = fly_target else { continue };
                    let entity = if vid != 0 {
                        world.get_resource::<Actors>().and_then(|actors| actors.characters.get(&vid)).map(|chr| chr.root)
                            .or_else(|| (vid == world.get_resource::<PlayerState>()?.vid).then_some(character.root))
                    } else { None };
                    if *following && let Some(entity) = entity {
                        fx.spawn_following(packs.as_ref(), file, entity, *offset);
                    } else {
                        let at = at + Vec3::from_array(m2_assets::coords::vec3_back(*offset));
                        fx.spawn(packs.as_ref(), file, at, Vec3::ZERO);
                    }
                }
                RaceMotionEventKind::ScreenWaving { power, range } => {
                    let center = world.get_resource::<crate::render::shadow::SceneCameraTarget>().and_then(|target| target.0);
                    if center.is_some_and(|center| center.distance_squared(position) <= (*range as f32).powi(2)) {
                        let now = world.resource::<Time>().elapsed_secs();
                        world.insert_resource(ScreenWave { until: now + event.duration, power: *power as f32 });
                    }
                }
                RaceMotionEventKind::CharacterHide => { world.entity_mut(character.root).insert(crate::render::character::MotionHidden); }
                RaceMotionEventKind::CharacterShow => { world.entity_mut(character.root).remove::<crate::render::character::MotionHidden>(); }
                RaceMotionEventKind::Sound { file } => fx.sound(position, file, false),
                RaceMotionEventKind::Fly { file, bone, position: offset } => {
                    let Some((target, target_at)) = fly_target else { continue };
                    let offset = Vec3::from_array(m2_assets::coords::vec3_back(Vec3::from_array(offset.to_array())));
                    let start = bone.as_deref().and_then(|name| character.bone_transform(world, name))
                        .map(|matrix| Vec3::from_array(m2_assets::coords::vec3_back(matrix.translation())))
                        .unwrap_or(metin2) + offset;
                    fx.fly(packs.as_ref(), file, start, target, target_at);
                }
                _ => {}
            }
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
                    let packs = std::sync::Arc::clone(&world.resource::<M2Packs>().packs);
                    let mut scene = MapScene::spawn_streamed(world, warped.prepared, fog, packs);
                    scene.update_streamed(world, Vec2::new(warped.local_x as f32, warped.local_y as f32));
                    let far = (scene.map.map.view_radius as f32 * m2_world::CELLSCALE as f32).max(1_000.0);
                    if let Ok(mut projection) = world.query_filtered::<&mut Projection, With<SceneCamera>>().single_mut(world)
                        && let Projection::Perspective(perspective) = &mut *projection
                    {
                        perspective.far = far;
                    }
                    world.resource_mut::<CombatFx>().set_fog(fog);
                    let height = scene.ground_height(warped.local_x as f32, warped.local_y as f32);
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
        let mut focus = *world.resource::<UiFocus>();
        if world.resource::<WarpTransition>().started.is_some() {
            focus.pointer = true;
            focus.keyboard = true;
        }
        let keys = world.resource::<ButtonInput<KeyCode>>().clone();
        let buttons = world.resource::<ButtonInput<MouseButton>>().clone();
        let scroll = *world.resource::<AccumulatedMouseScroll>();
        let mouse_motion = world.get_resource::<bevy::input::mouse::AccumulatedMouseMotion>().map_or(Vec2::ZERO, |motion| motion.delta);
        let cursor = world
            .query_filtered::<&Window, With<PrimaryWindow>>()
            .single(world)
            .ok()
            .and_then(|window| window.cursor_position());
        let base = game.base;
        let main_vid = game.main_vid;
        let last_pointer: Option<Vec2>;
        let mut attack_ready = game.attack_ready;
        let mut next_move_packet = game.next_move_packet;
        let (count_x, count_y) = scene.map.map.terrain_count;
        let extent = Vec2::new(f32::from(count_x) * TERRAIN_WORLD, f32::from(count_y) * TERRAIN_WORLD);

        let mut control = world.remove_resource::<PlayerControl>().unwrap();
        control.main_vid = main_vid;
        control.base = base;
        {
            // Last frame's hit shake is only drawn, never walked from.
            let position = world.get::<Transform>(character.root).map_or(Vec3::ZERO, |t| t.translation) - control.shake_offset;
            let mut local = Vec2::new(position.x, -position.z);

            // Camera (`MBF_CAMERA` on the right and middle buttons,
            // `NEW_SetMouseCameraState`): a press starts a drag and hides the
            // cursor, which stays where it was while the mouse turns the camera
            // (`CCamera::BeginDrag`/`Drag` warp it back); the release shows it
            // again. A release that did not turn the camera (`EndDrag` under 1)
            // targets the actor under the cursor (`__ChangeTargetToPickedInstance`).
            let camera_buttons = [MouseButton::Right, MouseButton::Middle];
            let mut camera_click = false;
            if !focus.pointer
                && control.camera_anchor.is_none()
                && camera_buttons.iter().any(|button| buttons.just_pressed(*button))
                && let Some(cursor) = cursor
            {
                control.camera_anchor = Some(cursor);
                control.camera_dragged = 0.0;
                set_cursor_lock(world, Some(cursor));
            }
            if let Some(anchor) = control.camera_anchor {
                if camera_buttons.iter().any(|button| buttons.pressed(*button)) {
                    let drag = mouse_motion;
                    control.yaw -= drag.x * 0.006;
                    control.pitch = (control.pitch + drag.y * 0.005).clamp(CAMERA_PITCH_MIN, CAMERA_PITCH_MAX);
                    control.camera_dragged += drag.length();
                    set_cursor_lock(world, Some(anchor));
                } else {
                    camera_click = control.camera_dragged < 1.0;
                    control.camera_anchor = None;
                    set_cursor_lock(world, None);
                }
            }
            let cursor = if control.camera_anchor.is_some() { None } else { cursor };
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
            if character.set_movement_rate(if points.move_speed > 0 { points.move_speed as f32 / 100.0 } else { 1.0 })
                && control.state == MoveState::Moving
            {
                character.play_motion(world, Motion::Run, true);
            }

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
            let attackable_me = me;
            let attackable = |vid: u32| {
                candidates
                    .iter()
                    .find(|(candidate, _)| *candidate == vid)
                    .is_some_and(|(_, candidate)| combat::is_attackable(&combat_world, &attackable_me, candidate))
            };
            let distance_to = |vid: u32, local: Vec2| {
                candidates.iter().find(|(candidate, _)| *candidate == vid).map(|(_, candidate)| {
                    (Vec2::new(candidate.x as f32, candidate.y as f32) - local).length()
                })
            };

            // `CPythonPlayer::SetTarget` is set by a click or a hit (`OnHit`);
            // a target that can no longer be picked (gone, hidden) is dropped.
            let still_targetable = control.target.and_then(|vid| candidates.iter().find(|(candidate, _)| *candidate == vid))
                .is_some_and(|(_, actor)| actor.can_pick());
            let new_target = if still_targetable { control.target } else { None };
            if new_target != control.target {
                control.target = new_target;
                if let Some(mut net) = world.get_resource_mut::<Net>() {
                    net.send_target(new_target.unwrap_or(0));
                }
            }
            control.target_distance = control.target.and_then(|vid| distance_to(vid, local));

            // The left button is `MBF_SMART` (`NEW_SetMouseSmartState`): a
            // press runs `__OnPressSmart`, which `NEW_RefreshMouseWalkingDirection`
            // repeats every frame while the button is held (`m_isSmtMov`), and
            // the release runs `__OnClickSmart`. Ctrl turns it into `NEW_Attack`.
            let mut click_attack = false;
            let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
            let left_press = !focus.pointer && buttons.just_pressed(MouseButton::Left);
            if left_press {
                control.smart_held = !ctrl;
            }
            let left_hold = control.smart_held && buttons.pressed(MouseButton::Left) && !left_press;
            let left_release = control.smart_held && buttons.just_released(MouseButton::Left);
            if !buttons.pressed(MouseButton::Left) {
                control.smart_held = false;
            }
            let bow_mode = matches!(
                character.motion_mode(),
                crate::render::character::MotionMode::Bow | crate::render::character::MotionMode::HorseBow
            );
            let reach = if bow_mode { bow_range(&points) } else { ATTACK_RANGE };
            let actor_at = |vid: u32| {
                candidates.iter().find(|(candidate, _)| *candidate == vid).map(|(_, actor)| *actor).unwrap_or_default()
            };
            if left_press && ctrl {
                click_attack = true;
            } else if (left_press || left_hold)
                && let Some(cursor) = cursor
            {
                if let Some(vid) = pick_actor(world, cursor, base).filter(|vid| *vid != main_vid) {
                    // `__OnPressActor`: `__ChangeTargetToPickedInstance` (`SetTarget`
                    // with `bForceChange`), then attack it in reach or walk to it.
                    if control.target == Some(vid) {
                        control.target_end = now + 1.0;
                    } else {
                        control.target = Some(vid);
                        control.target_end = now + 2.0;
                        if let Some(mut net) = world.get_resource_mut::<Net>() {
                            net.send_target(vid);
                        }
                    }
                    control.target_distance = distance_to(vid, local);
                    control.reserved_talk = None;
                    control.reserved_attack = None;
                    control.reserved_item = None;
                    let actor = actor_at(vid);
                    let in_reach = control.target_distance.is_some_and(|distance| distance <= reach);
                    if attackable(vid) {
                        if in_reach {
                            control.destination = None;
                            click_attack = true;
                        } else {
                            control.destination = Some(Vec2::new(actor.x as f32, actor.y as f32));
                            control.reserved_attack = Some(vid);
                        }
                    } else if control.target_distance.is_some_and(|distance| distance > 250.0) {
                        // `__ReserveClickActor`: walk into click range; the
                        // release (or the arrival) talks to it.
                        let destination = Vec2::new(actor.x as f32, actor.y as f32);
                        let direction = (destination - local).normalize_or_zero();
                        control.destination = Some(destination - direction * 180.0);
                        control.reserved_talk = actor.is_npc().then_some(vid);
                    }
                } else if let Some(item) = crate::ground::pick_item(world, cursor).filter(|_| left_press) {
                    // `__OnPressItem`: pick it at once within reach, else
                    // `__ReserveClickItem` and walk there first.
                    control.reserved_talk = None;
                    control.reserved_attack = None;
                    let at = world.resource::<crate::ground::GroundItems>().position(base, item);
                    if let Some(at) = at {
                        if (at - local).length() <= crate::ground::CLICKABLE_DISTANCE {
                            control.destination = None;
                            control.reserved_item = None;
                            crate::ground::send_click_item(world, item, now);
                        } else {
                            control.destination = Some(at);
                            control.reserved_item = Some(item);
                        }
                    }
                } else if control.reserved_item.is_none() || left_press {
                    // `__OnPressGround`: run to the point under the cursor,
                    // following it while the button is held.
                    control.reserved_talk = None;
                    control.reserved_attack = None;
                    control.reserved_item = None;
                    let height = |x: f32, y: f32| scene.ground_height(x, y);
                    if let Some(point) = pick_terrain(world, cursor, height, extent) {
                        control.destination = Some(point);
                    }
                }
            } else if left_release
                && let Some(cursor) = cursor
            {
                if crate::ground::pick_item(world, cursor).is_some() {
                    // `__OnClickItem` does nothing.
                } else if let Some(vid) = pick_actor(world, cursor, base) {
                    // `__OnClickActor`: talk to an NPC in click range (else
                    // walk there first), then `NEW_Stop`.
                    let actor = actor_at(vid);
                    let distance = distance_to(vid, local);
                    if !attackable(vid) && actor.is_npc() {
                        if distance.is_some_and(|distance| distance <= 250.0) {
                            control.destination = None;
                            if let Some(mut net) = world.get_resource_mut::<Net>()
                                && net.send_on_click(vid)
                            {
                                info!("requested NPC dialog from {vid}");
                            }
                        } else {
                            let destination = Vec2::new(actor.x as f32, actor.y as f32);
                            let direction = (destination - local).normalize_or_zero();
                            control.destination = Some(destination - direction * 180.0);
                            control.reserved_talk = Some(vid);
                        }
                    } else if control.reserved_attack.is_none() {
                        control.destination = None;
                    }
                } else {
                    let height = |x: f32, y: f32| scene.ground_height(x, y);
                    if let Some(point) = pick_terrain(world, cursor, height, extent) {
                        // `__OnClickGround`: the move and `__ShowPickedEffect`.
                        control.destination = Some(point);
                        let at = Vec3::new(point.x, -point.y, scene.ground_height(point.x, point.y));
                        let packs = world.get_resource::<M2Packs>().cloned();
                        world.resource_mut::<CombatFx>().spawn(packs.as_ref(), fx::EFFECT_PICK, at, Vec3::ZERO);
                    } else {
                        control.destination = None;
                    }
                }
            }
            if camera_click
                && let Some(cursor) = cursor
                && let Some(vid) = pick_actor(world, cursor, base)
            {
                // The right button's click: `__ChangeTargetToPickedInstance`
                // (`OpenCharacterMenu` is the PC target board, drawn for any target).
                if control.target != Some(vid) {
                    control.target = Some(vid);
                    if let Some(mut net) = world.get_resource_mut::<Net>() {
                        net.send_target(vid);
                    }
                }
                control.target_end = now + 2.0;
            }

            // `CInstanceBase::OnTargeted` attaches the registered
            // `EFFECT_TARGET` (`click_glow_select.mse`) and follows the actor.
            let hover_vid = (!focus.pointer)
                .then_some(cursor)
                .flatten()
                .and_then(|cursor| pick_actor(world, cursor, base));
            control.hover = hover_vid;
            let hover_vid = hover_vid
                .filter(|vid| Some(*vid) != control.target);
            let marker_position = |vid: u32| {
                let actor = world.resource::<Actors>().by_vid.get(&vid)?;
                let x = (actor.x - base.0 as i32) as f32;
                let y = (actor.y - base.1 as i32) as f32;
                Some(Vec3::new(x, -y, scene.ground_height(x, y)))
            };
            let target_marker = control.target.and_then(|vid| marker_position(vid).map(|position| (vid, position)));
            let hover_marker = hover_vid.and_then(|vid| marker_position(vid).map(|position| (vid, position)));
            let packs = world.get_resource::<M2Packs>().cloned();
            let mut fx = world.resource_mut::<CombatFx>();
            fx.set_target(packs.as_ref(), target_marker);
            fx.set_hover(packs.as_ref(), hover_marker);
            let fly_targets = fx.fly_targets();

            // Flying objects home on their targets; affect loops follow their
            // actors (`__SetAffect` -> `__AttachEffect(EFFECT_AFFECT + n)`).
            let (targets, affects, oriented) = {
                let actors = world.resource::<Actors>();
                let actor_local = |actor: &Actor| Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32);
                let feet = |p: Vec2| Vec3::new(p.x, -p.y, scene.ground_height(p.x, p.y));
                let targets: HashMap<u32, Vec3> = fly_targets
                    .into_iter()
                    .filter_map(|vid| {
                        let at = if vid == main_vid { feet(local) } else { feet(actor_local(actors.by_vid.get(&vid)?)) };
                        Some((vid, at + Vec3::Z * fx::FLY_TARGET_HEIGHT))
                    })
                    .collect();
                let main_flags = world.get_resource::<PlayerState>().map_or(0, |player| player.affect_flags);
                let mut affects = Vec::new();
                let mut oriented = Vec::new();
                let mut attach = |vid: u32, flags: u64, walking: bool, at: Vec3, chr: Option<&Character>| {
                    // Others under stealth hide their effects too (`HideAllAttachingEffect`).
                    let hidden_other = vid != main_vid
                        && (combat::has_affect(flags, combat::AFFECT_EUNHYEONG)
                            || combat::has_affect(flags, combat::AFFECT_REVIVE_INVISIBILITY));
                    if combat::has_affect(flags, combat::AFFECT_INVISIBILITY) || hidden_other {
                        return;
                    }
                    if combat::has_affect(flags, combat::AFFECT_GEOMGYEONG)
                        && let Some(chr) = chr
                        && let Some(bone) = chr.bone_transform(world, "equip_right_hand")
                    {
                        let two_handed = chr.weapon_type() == Some(combat::weapon::TWO_HANDED);
                        oriented.push(((vid, 0x10000 + u32::from(two_handed)), fx::sword_aura(two_handed), bone.to_matrix()));
                    }
                    for affect in 0..64 {
                        if !combat::has_affect(flags, affect) || (fx::affect_effect_needs_walking(affect) && !walking) {
                            continue;
                        }
                        let Some((bone, file)) = fx::affect_effect(affect) else { continue };
                        let position = (!bone.is_empty())
                            .then(|| chr?.bone_transform(world, bone))
                            .flatten()
                            .map_or(at, |t| Vec3::from_array(m2_assets::coords::vec3_back(t.translation())));
                        affects.push(((vid, affect), file, position));
                    }
                };
                attach(main_vid, main_flags, control.state == MoveState::Moving, feet(local), Some(&*character));
                for (&vid, actor) in &actors.by_vid {
                    if vid != main_vid && actor.affect_flags != 0 {
                        attach(vid, actor.affect_flags, actor.moving, feet(actor_local(actor)), actors.characters.get(&vid));
                    }
                }
                let catalog = world.get_resource::<ItemCatalog>();
                let mut refine = |vid: u32, vnum: u32, at: Vec3, chr: &Character| {
                    let Some(item) = catalog.and_then(|catalog| catalog.get(vnum)) else { return };
                    for (key, attachment, file) in fx::refined_effects(vnum, item) {
                        let (right, left) = crate::render::character::weapon_bones(chr.look.race);
                        let bone = match attachment { "right" => right, "left" => left, other => Some(other) };
                        let position = bone.and_then(|bone| chr.bone_transform(world, bone))
                            .map_or(at, |t| Vec3::from_array(m2_assets::coords::vec3_back(t.translation())));
                        affects.push(((vid, key), file, position));
                    }
                };
                let player = world.get_resource::<PlayerState>();
                if let Some(player) = player {
                    refine(main_vid, player.weapon().map_or(0, |item| item.vnum), feet(local), character);
                    if let Some((armor, _)) = player.appearance {
                        refine(main_vid, u32::from(armor), feet(local), character);
                    }
                }
                for (&vid, actor) in &actors.by_vid {
                    if actor.kind != m2_net::phase::ACTOR_TYPE_PC { continue; }
                    if let Some(chr) = actors.characters.get(&vid) {
                        let at = feet(actor_local(actor));
                        refine(vid, u32::from(actor.weapon), at, chr);
                        refine(vid, u32::from(actor.armor), at, chr);
                    }
                }
                if let Some(social) = world.get_resource::<crate::social::SocialState>() {
                    for (&id, target) in &social.targets {
                        if let Some(at) = quest_target_position(target, &actors, base) {
                            let at = local_to_world(at.x, at.y, scene.ground_height(at.x, at.y));
                            oriented.push(((id as u32, 0x20000), fx::EFFECT_QUEST_TARGET, Mat4::from_translation(at)));
                        }
                    }
                }
                (targets, affects, oriented)
            };
            let mut fx = world.resource_mut::<CombatFx>();
            let impacts = fx.update_flies(packs.as_ref(), delta, |vid| targets.get(&vid).copied());
            fx.sync_attached(packs.as_ref(), &affects);
            fx.sync_oriented(packs.as_ref(), &oriented);
            drop(fx);
            // `CFlyingInstance::Update` calls OnShootDamage on an object target.
            for vid in impacts {
                if vid == main_vid {
                    control.shake_until = now + 0.1;
                    if control.state == MoveState::Idle {
                        if let (Some(damage), Some(wait)) = (character.motion_path(Motion::Damage { back: false }), character.motion_path(Motion::Wait)) {
                            character.play_then(world, &damage, &wait);
                        }
                    }
                } else {
                    let mut actors = world.resource_mut::<Actors>();
                    let fight = actors.fights.entry(vid).or_default();
                    if fight.dead { continue; }
                    fight.shake_until = now + 0.1;
                    if fight.stunned {
                        fight.dead = true;
                        fight.pending = Some(ActorAction::Dead);
                    } else if !fight.attacking && !fight.knocked_down {
                        fight.pending = Some(ActorAction::Damage { back: false });
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
                    let victim = nearest_attackable(&candidates, &attackable, local, ATTACK_RANGE).unwrap_or(0);
                    face_actor(world, &mut control, base, local, victim);
                    control.attack_rotation = control.advancing;
                    control.combo = 0;
                    input_combo_attack(world, &mut control, character, now, base, local);
                }
            }

            let mut moved = false;
            let mut direction = None;
            if control.state == MoveState::Attacking && !focus.keyboard {
                let mut input = Vec2::ZERO;
                if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) { input.y += 1.0; }
                if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) { input.y -= 1.0; }
                if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) { input.x += 1.0; }
                if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) { input.x -= 1.0; }
                if input != Vec2::ZERO {
                    let forward = Vec2::new(-control.yaw.sin(), control.yaw.cos());
                    let right = Vec2::new(control.yaw.cos(), control.yaw.sin());
                    let heading = (forward * input.y + right * input.x).normalize();
                    control.attack_rotation = facing_degrees(heading);
                    if character.is_mounted() { direction = Some(heading); }
                }
            }
            let moving_skill = control.state == MoveState::Casting && world.get_resource::<combat::SkillCatalog>()
                .and_then(|catalog| catalog.get(control.motion_skill)).is_some_and(SkillExt::is_moving_skill);
            if (!matches!(control.state, MoveState::Attacking | MoveState::Casting) || moving_skill) && !control.dead {
                let mut input = Vec2::ZERO;
                if !focus.keyboard {
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
                }
                // `NEW_SetSingleDirKeyState` -> `__ClearReservedAction`: a
                // direction key takes over from a click move.
                if input != Vec2::ZERO && control.destination.is_some() {
                    control.destination = None;
                    control.reserved_attack = None;
                    control.reserved_talk = None;
                    control.reserved_skill = None;
                    control.reserved_item = None;
                }
                let mut wanted = None;
                let run_speed = remote_run_speed(points.move_speed.clamp(0, 255) as u8);
                if let Some(destination) = control.destination {
                    let (next, arrived) = advance_towards(local, destination, run_speed * delta);
                    wanted = Some(next);
                    if arrived {
                        info!("destination reached at {:.0},{:.0}", next.x, next.y);
                        control.destination = None;
                    }
                } else if input != Vec2::ZERO {
                    let forward = Vec2::new(-control.yaw.sin(), control.yaw.cos());
                    let right = Vec2::new(control.yaw.cos(), control.yaw.sin());
                    let map_direction = (forward * input.y + right * input.x).normalize();
                    wanted = Some(clamp_to_map(local + map_direction * run_speed * delta, extent));
                }
                if let Some(wanted) = wanted
                    && (wanted - local).length() > f32::EPSILON
                {
                    // The character turns to where it wants to go even when
                    // a wall stops it.
                    direction = Some((wanted - local).normalize());
                    let next = check_advancing(world, scene, character, local, wanted, base, main_vid, false, false);
                    if (next - local).length() > f32::EPSILON {
                        local = next;
                        moved = true;
                    } else if control.destination.is_some() && scene.contains_position(wanted) {
                        // `BlockMovement`: a click-move into a wall stops.
                        info!("blocked at {:.0},{:.0}", local.x, local.y);
                        control.destination = None;
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

            // `__ReserveClickActor`: stop within the NPC's clickable distance,
            // then ask the server for the quest script with `HEADER_CG_ON_CLICK`.
            if let Some(vid) = control.reserved_talk {
                if let Some(distance) = distance_to(vid, local) {
                    if distance <= 250.0 {
                        control.reserved_talk = None;
                        control.destination = None;
                        if let Some(mut net) = world.get_resource_mut::<Net>() {
                            if net.send_on_click(vid) {
                                info!("requested NPC dialog from {vid}");
                            } else {
                                warn!("could not send NPC click for {vid}");
                            }
                        }
                    }
                } else {
                    control.reserved_talk = None;
                }
            }

            // `MODE_CLICK_ITEM`: pick the reserved item on arrival.
            if let Some(item) = control.reserved_item {
                match world.resource::<crate::ground::GroundItems>().position(base, item) {
                    Some(at) if (at - local).length() < crate::ground::RESERVED_PICK_DISTANCE || control.destination.is_none() => {
                        control.reserved_item = None;
                        control.destination = None;
                        if (at - local).length() <= crate::ground::CLICKABLE_DISTANCE {
                            crate::ground::send_click_item(world, item, now);
                        }
                    }
                    Some(_) => {}
                    None => control.reserved_item = None,
                }
            }
            // `game.py`'s Z / grave: `PickCloseItem`, the nearest item within
            // `__GetPickableDistance` (500 mounted, else 300).
            if !focus.keyboard && (keys.just_pressed(KeyCode::KeyZ) || keys.just_pressed(KeyCode::Backquote)) {
                let reach = if character.is_mounted() { 500.0 } else { 300.0 };
                let close = world.resource::<crate::ground::GroundItems>().closest(base, local, reach);
                if let Some(item) = close {
                    crate::ground::send_click_item(world, item, now);
                }
            }

            me.x = local.x.round() as i32;
            me.y = local.y.round() as i32;
            // `MODE_USE_SKILL`: keep running towards the reserved target and
            // cast as soon as the skill's range is reached.
            if let Some((vnum, vid, range)) = control.reserved_skill {
                if let Some((_, actor)) = candidates.iter().find(|(candidate, _)| *candidate == vid) {
                    if skill_distance(&me, actor) < range as f32 {
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
            // key is down. Melee feeds the combo (`InputComboAttackCommand`),
            // which chains the steps on the motions' combo timings; the bow
            // shoots once per motion.
            let attack_key = !focus.keyboard && keys.pressed(KeyCode::Space);
            if (attack_key || click_attack) && !control.dead && control.state != MoveState::Casting {
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
                    if control.state == MoveState::Attacking || now < attack_ready {
                        Ok(())
                    } else {
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
                            control.attack_rotation = control.advancing;
                            control.combo = 0;
                            run_next_combo(world, &mut control, character, now, base, local);
                            if control.target != Some(victim) {
                                control.target = Some(victim);
                                if let Some(mut net) = world.get_resource_mut::<Net>() {
                                    net.send_target(victim);
                                }
                            }
                            world.resource_mut::<FlyTargets>().set(control.main_vid, ShotTarget::Actor(victim), false);
                            if let Some(mut net) = world.get_resource_mut::<Net>() {
                                if let Some((_, actor)) = candidates.iter().find(|(candidate, _)| *candidate == victim) {
                                    net.send_fly_targeting(victim, actor.x + base.0 as i32, actor.y + base.1 as i32, false);
                                }
                            }
                            let duration = character.motion_duration();
                            attack_ready = now + if duration > 0.05 { duration * 0.9 } else { ATTACK_COOLDOWN };
                        })
                    }
                } else {
                    context.can_attack().and_then(|()| context.mounted_hand_attack()).map(|()| {
                        // `m_fAtkDirRot`: towards the target or the nearest
                        // attackable actor in reach, else straight ahead.
                        let victim = control
                            .target
                            .filter(|vid| {
                                attackable(*vid) && distance_to(*vid, local).is_some_and(|distance| distance <= ATTACK_RANGE)
                            })
                            .or_else(|| nearest_attackable(&candidates, &attackable, local, ATTACK_RANGE));
                        if control.state != MoveState::Attacking {
                            if let Some(victim) = victim {
                                face_actor(world, &mut control, base, local, victim);
                            }
                            control.attack_rotation = control.advancing;
                        } else if direction.is_some() {
                            control.attack_rotation = facing_degrees(direction.unwrap());
                        }
                        input_combo_attack(world, &mut control, character, now, base, local);
                    })
                };
                if let Err(error) = result
                    && keys.just_pressed(KeyCode::Space)
                    && let Some(mut player) = world.get_resource_mut::<PlayerState>()
                {
                    player.line(1, format!("Cannot attack: {}", error.reason()));
                }
            }
            if moving_skill && moved && now >= next_move_packet {
                next_move_packet = now + MOVE_PACKET_SECONDS;
                send_move(world, base, local, control.advancing);
            }
            // `ComboProcess`: a remembered input fires at `DirectInputTime`.
            combo_process(world, &mut control, character, now, base, local);

            // `__AccumulationMovement`: attack and skill motions with an
            // `Accumulation` step the character forward over the motion
            // (`accum · dt / duration`, turned by the advancing rotation),
            // through the same collision as walking.
            if matches!(control.state, MoveState::Attacking | MoveState::Casting)
                && !(character.is_mounted() && control.state == MoveState::Attacking)
                && let Some(elapsed) = character.motion_elapsed(now)
                && let Some((accumulation, duration)) = character
                    .motion_handle()
                    .and_then(|handle| world.resource::<Assets<m2_assets::race::RaceMotionAsset>>().get(handle))
                    .and_then(|asset| asset.accumulation.map(|a| (a, asset.duration)))
                && duration > 0.0
                && elapsed < duration
            {
                let step = delta.min(duration - elapsed) / duration;
                let accum = Vec3::from_array(m2_assets::coords::vec3_back(accumulation)) * step;
                let world_delta = battle::rotate_local(accum, control.advancing);
                let wanted = clamp_to_map(local + Vec2::new(world_delta.x, -world_delta.y), extent);
                if (wanted - local).length() > f32::EPSILON {
                    local = check_advancing(world, scene, character, local, wanted, base, main_vid, true, control.state == MoveState::Casting);
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
                    if character.is_mounted() && control.state == MoveState::Attacking {
                        control.attack_rotation = target;
                    }
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
            let mut height = scene.ground_height(local.x, local.y);
            if character.is_mounted() {
                height += MOUNT_HEIGHT;
            }
            // `ShakeProcess` of the main character when hit.
            let shake = if control.shake_until > now {
                shake_offset((now * 1000.0) as u32 ^ main_vid)
            } else {
                Vec3::ZERO
            };
            if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                transform.translation = local_to_world(local.x, local.y, height) + shake;
            }
            control.shake_offset = shake;
            game.local = Some((local.x.round() as i32, local.y.round() as i32));

            // Polymorph: the main actor re-appended with a monster race is
            // rebuilt as that monster (`CInstanceBase::SetRace`, `IsPoly`), and
            // back as the player race when it ends.
            let poly = world.get_resource::<PlayerState>().map_or(0, |player| player.poly_race);
            if poly != control.poly_race {
                let transform = world.get::<Transform>(character.root).copied().unwrap_or_default();
                let replacement = if poly != 0 {
                    world.get_resource::<MobCatalog>().and_then(|mobs| mobs.by_race.get(&poly)).cloned().and_then(|mob| {
                        let file = mob.race_file.clone()?;
                        let mut body = Character::spawn_actor(world, poly, &file, transform, None);
                        body.set_mob_motions(mob_motion_files(&mob.motions));
                        Some(body)
                    })
                } else {
                    let (race, hair, armor) = world.get_resource::<PlayerState>().map_or((0, 0, 0), |player| {
                        let (armor, hair) = player.appearance.unwrap_or_default();
                        (player.race as u8, u32::from(hair), u32::from(armor))
                    });
                    let shape = world.resource::<ArmorShapes>().shape(armor);
                    Some(Character::spawn(world, Look { race, shape, hair }, transform, None))
                };
                // The monster's data may still be loading: retry next frame.
                if let Some(mut body) = replacement {
                    if world.get_entity(character.root).is_ok() {
                        world.despawn(character.root);
                    }
                    body.play_motion(world, if control.state == MoveState::Moving { Motion::Run } else { Motion::Wait }, true);
                    **character = body;
                    control.poly_race = poly;
                }
            }

            // `CInstanceBase::ChangeWeapon`/`MountHorse`: the server's
            // equipment and mount cells drive the model, motion mode and the
            // wait/run motion. A polymorphed body has neither.
            if let Some((armor, hair)) = world.get_resource::<PlayerState>().and_then(|player| player.appearance) {
                let shapes = world.resource::<ArmorShapes>();
                let (shape, specular) = (shapes.shape(u32::from(armor)), shapes.specular(u32::from(armor)));
                let look = Look { race: character.look.race, shape, hair: u32::from(hair) };
                character.change_look(world, look, specular);
            }
            if !character.is_poly() && sync_main_weapon(world, character) {
                match control.state {
                    MoveState::Idle => character.play_motion(world, Motion::Wait, true),
                    MoveState::Moving => character.play_motion(world, Motion::Run, true),
                    MoveState::Attacking | MoveState::Casting | MoveState::Dead => {}
                }
            }
            if !character.is_poly() && sync_main_mount(world, character) {
                match control.state {
                    MoveState::Idle => character.play_motion(world, Motion::Wait, true),
                    MoveState::Moving => character.play_motion(world, Motion::Run, true),
                    MoveState::Attacking | MoveState::Casting | MoveState::Dead => {}
                }
            }

            // The fight this frame: the main character's motion events and
            // hits, then the other actors' swings at it.
            let ground = scene.ground_height(local.x, local.y);
            let mut me_position = Vec3::new(local.x, -local.y, ground);
            let heights = |x: f32, y: f32| scene.ground_height(x, y);
            let blocked = |p: Vec2| map_attr(scene, p, ATTR_BLOCK);
            let warped = main_motion_events(world, &mut control, character, me_position, now, &blocked);
            if warped != me_position {
                local = Vec2::new(warped.x, -warped.y);
                me_position = Vec3::new(local.x, -local.y, scene.ground_height(local.x, local.y));
                game.local = Some((local.x.round() as i32, local.y.round() as i32));
                if let Some(mut transform) = world.get_mut::<Transform>(character.root) {
                    transform.translation = fx::bevy_position(me_position);
                }
            }
            main_attack_process(world, &mut control, character, &candidates, &attackable, me_position, now, delta, &heights, &blocked);
            actor_attacks_on_main(world, &mut control, character, &candidates, &me, &combat_world, base, me_position, now, delta, &heights);
            actor_attacks_on_actors(world, &candidates, &combat_world, now, delta, &heights);

            // `GC_DEAD`: the death motion once, then nothing until revived
            // (the server restores the HP on restart).
            let hp = world.get_resource::<PlayerState>().map_or(1, |player| player.points[POINT_HP]);
            if control.dead && control.state != MoveState::Dead {
                control.state = MoveState::Dead;
                control.combo = 0;
                control.pre_input = false;
                control.action_started = now;
                if let Some(file) = character.motion_path(Motion::Dead) {
                    character.play(world, &file, false);
                }
            } else if control.state == MoveState::Dead && hp > 0 && now - control.action_started > 1.0 {
                control.dead = false;
                control.state = MoveState::Idle;
                character.play_motion(world, Motion::Wait, true);
            }

            // Motions and position packets.
            match control.state {
                MoveState::Dead => {}
                MoveState::Attacking | MoveState::Casting => {
                    if (character.motion_started() && character.motion_finished(world))
                        || (!character.motion_started() && now - control.action_started > 4.0)
                    {
                        // Back to wait: a non-combo motion resets the combo
                        // (`SetMotion` -> `m_dwcurComboIndex = 0`).
                        control.state = MoveState::Idle;
                        control.combo = 0;
                        control.pre_input = false;
                        control.combo_time = now;
                        control.splash = None;
                        control.motion_skill = 0;
                        character.play_motion(world, Motion::Wait, true);
                        send_stop(world, base, local, control.advancing);
                    }
                }
                _ if moved => {
                    if control.state != MoveState::Moving {
                        control.state = MoveState::Moving;
                        character.play_motion(world, Motion::Run, true);
                        next_move_packet = now + MOVE_PACKET_SECONDS;
                        send_move(world, base, local, control.advancing);
                    }
                    else if now >= next_move_packet {
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
        let position = world.get::<Transform>(character.root).map_or(Vec3::ZERO, |t| t.translation);
        scene.update_streamed(world, Vec2::new(position.x, -position.z));
        character.update(world);
    });
}

/// `CRaceMotionData::NAME_COMBO_ATTACK_1`: the motion index of combo motion 1.
const NAME_COMBO_ATTACK_1: u32 = 14;
/// Camera dragging uses relative mouse motion with a locked pointer. Bevy
/// falls back to confinement on X11. Wayland forbids ordinary pointer warps,
/// so do not call Window::set_cursor_position here (even on the first frame
/// when the lock request has not reached the compositor yet).
fn set_cursor_lock(world: &mut World, anchor: Option<Vec2>) {
    use bevy::window::{CursorGrabMode, CursorOptions};
    let Ok(mut options) = world.query_filtered::<&mut CursorOptions, With<PrimaryWindow>>().single_mut(world) else { return };
    let (visible, grab) = if anchor.is_some() { (false, CursorGrabMode::Locked) } else { (true, CursorGrabMode::None) };
    if options.visible != visible { options.visible = visible; }
    if options.grab_mode != grab { options.grab_mode = grab; }
}

/// A monster's motion files for a polymorphed player body.
fn mob_motion_files(motions: &ActorMotions) -> crate::render::character::MobMotionFiles {
    crate::render::character::MobMotionFiles {
        wait: motions.wait.clone(),
        walk: motions.walk.clone(),
        run: motions.run.clone(),
        attack: motions.attack.clone(),
        damage: motions.damage.clone(),
        dead: motions.dead.clone(),
    }
}

/// `MAIN_RACE_MAX_NUM`: the eight player races.
pub const MAIN_RACE_MAX_NUM: u16 = 8;

/// `IS_HUGE_RACE`: reached from further away, not pushed.
const HUGE_RACE: u16 = 2493;

/// The once motion a state command plays (`StateProcess`'s function cases).
fn command_action(motion: &ActorMove) -> Option<ActorAction> {
    match motion.func {
        FUNC_ATTACK => Some(ActorAction::Attack),
        FUNC_COMBO => Some(ActorAction::Combo(u32::from(motion.arg))),
        FUNC_MOB_SKILL => Some(ActorAction::Special(motion.arg)),
        func if func & FUNC_SKILL != 0 => Some(ActorAction::Skill { index: u16::from(func & !FUNC_SKILL), loops: motion.arg & 15, moving: motion.arg & 16 != 0 }),
        _ => None,
    }
}

/// `CInstanceBase::StateProcess` for one due command of a remote PC. Returns
/// the once motion to play now (a function close enough to run in place).
fn remote_state_process(actor: &mut Actor, command: &ActorMove) -> Option<ActorAction> {
    let current = actor.remote.position;
    let destination = Vec2::new(command.x as f32, command.y as f32);
    let distance = current.distance(destination);
    let action = command_action(command);
    // `FUNC_WAIT` walks over 1 unit, the functions over 50; `FUNC_MOVE` always.
    let walk = match command.func {
        FUNC_MOVE => true,
        FUNC_WAIT => distance > 1.0,
        _ if action.is_some() => distance >= 50.0,
        _ => return None,
    };
    // A monster's pace is the server's: the distance over the packet's duration.
    actor.remote.speed = (actor.kind != m2_net::phase::ACTOR_TYPE_PC && command.duration > 0)
        .then(|| distance / (command.duration as f32 / 1000.0))
        .filter(|speed| speed.is_finite() && *speed > 0.0);
    if walk {
        // `NEW_SetSrcPixelPosition(cur)`, `NEW_SetDstPixelPosition(dst)`,
        // `m_isGoing`, `m_kMovAfterFunc`, `StartWalking`.
        actor.move_from = current;
        actor.move_to = destination;
        actor.remote.position = current;
        actor.remote.dst_rotation = command.rotation;
        actor.remote.going = true;
        actor.remote.after_move = command.func == FUNC_MOVE;
        actor.move_action = action;
        actor.moving = true;
        None
    } else {
        // In place: `SCRIPT_SetPixelPosition(dst)`, the rotation, then the function.
        actor.remote.going = false;
        actor.move_action = None;
        actor.moving = false;
        actor.x = command.x;
        actor.y = command.y;
        actor.remote.position = destination;
        actor.rotation = command.rotation;
        action
    }
}

/// The remote half of `CInstanceBase::MovementProcess`: walk along the source
/// to destination direction at the run speed; a `FUNC_MOVE` keeps walking
/// through its destination (re-aiming once it is 100 units past), the others
/// stop on it and run their function. Returns that function's motion.
fn remote_movement_process(actor: &mut Actor, delta: f32) -> Option<ActorAction> {
    if !actor.remote.going || !actor.moving {
        return None;
    }
    let step = actor.remote.speed.unwrap_or_else(|| remote_move_speed(actor.move_speed, actor.walk_mode)) * delta;
    let direction = |from: Vec2, to: Vec2, rotation: f32| {
        (to - from).try_normalize().unwrap_or(Vec2::new(rotation.to_radians().sin(), rotation.to_radians().cos()))
    };
    let current = actor.remote.position;
    let mut heading = direction(actor.move_from, actor.move_to, actor.remote.dst_rotation);
    let mut next = current + heading * step;
    let current_length = current.distance(actor.move_from);
    let next_length = next.distance(actor.move_from);
    let total_length = actor.move_to.distance(actor.move_from);
    let mut action = None;
    if total_length - current_length < -100.0 {
        // Latency let it run too far: aim back at the destination and stop
        // there unless another move arrives first.
        actor.move_from = current;
        heading = direction(current, actor.move_to, actor.remote.dst_rotation);
        next = current + heading * step;
        actor.remote.after_move = false;
    } else if current_length <= total_length && total_length <= next_length && !actor.remote.after_move {
        // Arrived: `SCRIPT_SetPixelPosition(dst)`, the rotation, `EndWalking`.
        next = actor.move_to;
        actor.remote.going = false;
        actor.moving = false;
        actor.rotation = actor.remote.dst_rotation;
        action = actor.move_action.take();
    }
    if actor.remote.going {
        // `SetAdvancingRotation`: face the way it walks.
        actor.rotation = facing_degrees(heading);
    }
    actor.remote.position = next;
    actor.x = next.x.round() as i32;
    actor.y = next.y.round() as i32;
    action
}

/// Remote PCs walk using their synchronized movement-speed percentage.
fn remote_run_speed(speed: u8) -> f32 {
    RUN_SPEED * if speed == 0 { 1.0 } else { f32::from(speed) / 100.0 }
}

fn remote_move_speed(speed: u8, walking: bool) -> f32 {
    remote_run_speed(speed) * if walking { 220.0 / RUN_SPEED } else { 1.0 }
}
/// `gc_fActorSlideMoveSpeed`: the most a collision slide may move per frame.
const ACTOR_SLIDE_MOVE_SPEED: f32 = 30.0;
/// The body sphere of an actor whose race has none (players' `.msm` give 40).
const DEFAULT_BODY_RADIUS: f32 = 40.0;
/// `ATTRIBUTE_BLOCK`.
const ATTR_BLOCK: u8 = 1 << 0;
/// `POINT_HP`.
const POINT_HP: usize = 5;

/// What the main character's combo code needs from the playing motion.
struct MotionInfo {
    elapsed: f32,
    duration: f32,
    combo: Option<m2_assets::race::ComboInput>,
}

fn motion_info(world: &World, character: &Character, now: f32) -> Option<MotionInfo> {
    let elapsed = character.motion_elapsed(now)?;
    let asset = world.resource::<Assets<m2_assets::race::RaceMotionAsset>>().get(character.motion_handle()?)?;
    Some(MotionInfo { elapsed, duration: asset.duration, combo: asset.combo_input })
}

/// `__GetCurrentComboType`: hand, bow and mounted modes have only the basic
/// combo.
fn current_combo_type(control: &PlayerControl, character: &Character) -> u32 {
    use crate::render::character::MotionMode;
    let mode = character.motion_mode();
    if matches!(mode, MotionMode::General | MotionMode::Bow | MotionMode::HorseBow) || character.is_mounted() {
        0
    } else {
        control.combo_type
    }
}

/// `CActorInstance::__RunNextCombo`: the next step of the combo, facing
/// `m_fAtkDirRot`. Past the last step nothing happens (the C++ traces an
/// error) until the motion ends and the combo resets.
fn run_next_combo(
    world: &mut World,
    control: &mut PlayerControl,
    character: &mut Character,
    now: f32,
    base: (u32, u32),
    local: Vec2,
) -> bool {
    control.combo += 1;
    let sequence = character.motion_mode().combo_sequence(current_combo_type(control, character));
    let Some(&motion) = sequence.get(control.combo as usize - 1) else { return false };
    if !character.play_combo_motion(world, motion) {
        return false;
    }
    // `ComboAttack`: `BlendRotation` + `SetAdvancingRotation` + the motion.
    set_advancing_rotation(control, control.attack_rotation);
    control.state = MoveState::Attacking;
    control.destination = None;
    control.action_started = now;
    control.hits.clear();
    control.splash = None;
    control.motion_skill = 0;
    // `__OnAttack` -> `OnAttack`: `FUNC_COMBO` with the motion index.
    let (x, y) = global_of(base, local);
    let motion_index = NAME_COMBO_ATTACK_1 + motion - 1;
    if let Some(mut net) = world.get_resource_mut::<Net>() {
        net.send_move(FUNC_COMBO, motion_index as u8, control.attack_rotation, x, y);
    }
    // `__OnEndCombo`: mounted combos go on from the first step.
    if control.combo as usize == sequence.len() && character.is_mounted() {
        control.combo = 1;
    }
    debug!("combo step {} (motion {motion})", control.combo);
    true
}

/// `CActorInstance::InputComboAttackCommand`: one frame of the attack key.
/// The first press starts the combo; while a step runs, input before
/// `PreInputTime` is ignored, input in the pre-input window is remembered,
/// input after `DirectInputTime` starts the next step at once and input after
/// `InputLimitTime` is dropped. A motion without combo data takes the next
/// input after 90% of its length.
fn input_combo_attack(
    world: &mut World,
    control: &mut PlayerControl,
    character: &mut Character,
    now: f32,
    base: (u32, u32),
    local: Vec2,
) -> bool {
    if control.pre_input {
        return false;
    }
    if control.combo == 0 || control.state != MoveState::Attacking {
        control.combo = 0;
        return run_next_combo(world, control, character, now, base, local);
    }
    let Some(info) = motion_info(world, character, now) else { return false };
    match info.combo {
        Some(combo) => {
            if info.elapsed > combo.input_end {
                false
            } else if info.elapsed > combo.next_combo {
                run_next_combo(world, control, character, now, base, local)
            } else {
                if info.elapsed > combo.input_start {
                    control.pre_input = true;
                }
                false
            }
        }
        None if info.elapsed > info.duration * 0.9 => run_next_combo(world, control, character, now, base, local),
        None => false,
    }
}

/// `CActorInstance::ComboProcess`: a remembered input starts the next step
/// once `DirectInputTime` is reached.
fn combo_process(
    world: &mut World,
    control: &mut PlayerControl,
    character: &mut Character,
    now: f32,
    base: (u32, u32),
    local: Vec2,
) {
    if control.combo == 0 {
        control.pre_input = false;
        return;
    }
    if !control.pre_input || control.state != MoveState::Attacking {
        return;
    }
    let Some(info) = motion_info(world, character, now) else { return };
    if info.combo.is_some_and(|combo| info.elapsed > combo.next_combo) {
        control.pre_input = false;
        run_next_combo(world, control, character, now, base, local);
    }
}

/// The defending spheres of a tracked actor: its race's, or one sphere at its
/// chest when the race has none (or is not drawn).
fn victim_spheres(world: &World, actors: &mut Actors, vid: u32, position: Vec3) -> Vec<DynamicSphere> {
    let spheres = actors
        .characters
        .get_mut(&vid)
        .filter(|character| character.has_spheres(m2_formats::race::COLLISION_TYPE_DEFENDING))
        .map(|character| character.spheres(world, m2_formats::race::COLLISION_TYPE_DEFENDING))
        .unwrap_or_default();
    if spheres.is_empty() { vec![DynamicSphere::still(position + Vec3::Z * 100.0, 70.0)] } else { spheres }
}

/// Where a hit lands and how the victim reacts: `__ProcessDataAttackSuccess`
/// for the main character's hits (hit spark, `__HitGood`/`__HitGreate`,
/// invisibility) and `OnHit` (target, `CG_ATTACK`).
#[allow(clippy::too_many_arguments)]
fn attack_success(
    world: &mut World,
    control: &mut PlayerControl,
    attack: &m2_assets::race::MotionAttack,
    attacker: Vec3,
    attacker_rotation: f32,
    victim: &ActorCombat,
    victim_position: Vec3,
    skill: u32,
    send_packet: bool,
    blocked: &dyn Fn(Vec2) -> bool,
    now: f32,
) {
    if attack.hitting_type == battle::HIT_TYPE_NONE {
        return;
    }
    attack_feedback(world, attack, attacker, attacker_rotation, victim, victim_position, now);
    let huge = victim.race == HUGE_RACE;
    // `__CanPushDestActor` + `__PushCircle` + `IncreaseExternalForce`: a hit
    // with an external force (the combo's last steps) pushes a victim the
    // server made ours to sync (`GC_OWNERSHIP`, under 3 s ago) or a stunned
    // one away from the attacker.
    let pushable = {
        let fight = world.resource::<Actors>().fights.get(&victim.vid);
        let owned = fight.is_some_and(|fight| {
            fight.stunned || (fight.owner == control.main_vid && now - fight.owner_since <= 3.0)
        });
        !(victim.is_building() || victim.is_door() || victim.is_stone() || victim.is_npc() || huge) && owned
    };
    if attack.external_force > 0.0
        && pushable
        && let Some(movement) = external_force_movement((victim_position - attacker).truncate(), attack.external_force)
        && movement.length() > 1.0
    {
        let from = Vec2::new(victim_position.x, -victim_position.y);
        let to = from + Vec2::new(movement.x, -movement.y);
        let steps = (movement.length() / 10.0).ceil() as i32;
        let clear = (1..=steps).all(|i| !blocked(from.lerp(to, i as f32 / steps as f32)));
        if clear {
            let base = control.base;
            let mut actors = world.resource_mut::<Actors>();
            if let Some(actor) = actors.by_vid.get_mut(&victim.vid) {
                let global = Vec2::new(to.x + base.0 as f32, to.y + base.1 as f32);
                // `IncreaseExternalForce`: the offset blends in over the
                // physics time while the victim keeps its own movement.
                let from = actor.remote.position;
                actor.remote.push = Some(PushBlend::new(from, global));
                control.victims.push((victim.vid, global.x as i32, global.y as i32));
            }
        }
    }
    // `OnHit` -> `SetTarget(victim, FALSE)`: hitting the target keeps it for
    // another second; another victim takes over only once that has run out,
    // so a swing through a group does not flip the target between them.
    if control.target == Some(victim.vid) {
        control.target_end = now + 1.0;
    } else if control.target_end <= now && control.target != Some(victim.vid) {
        control.target = Some(victim.vid);
        control.target_end = now + 1.0;
        if let Some(mut net) = world.get_resource_mut::<Net>() {
            net.send_target(victim.vid);
        }
    }
    if send_packet && let Some(mut net) = world.get_resource_mut::<Net>() {
        net.send_skill_attack(skill, victim.vid);
    }
    debug!("hit {} (skill {skill})", victim.vid);
}

/// Shared `__ProcessDataAttackSuccess` feedback. Remote attackers use this
/// without invoking the main player's targeting, damage or position packets.
#[allow(clippy::too_many_arguments)]
fn attack_feedback(
    world: &mut World,
    attack: &m2_assets::race::MotionAttack,
    attacker: Vec3,
    attacker_rotation: f32,
    victim: &ActorCombat,
    victim_position: Vec3,
    now: f32,
) {
    if attack.hitting_type == battle::HIT_TYPE_NONE { return; }
    let (rotation, huge) = {
        let actors = world.resource::<Actors>();
        let actor = actors.by_vid.get(&victim.vid);
        (actor.map_or(0.0, |a| a.rotation), victim.race == HUGE_RACE)
    };
    // Hit spark: at the victim, turned to face the attacker.
    let spark = if huge { (attacker + victim_position) * 0.5 } else { victim_position };
    let height = (attacker.x - spark.x).atan2(spark.y - attacker.y).to_degrees();
    let packs = world.get_resource::<M2Packs>().cloned();
    {
        let mut fx = world.resource_mut::<CombatFx>();
        if victim.is_building() || victim.is_door() {
            let towards = (spark - attacker).normalize_or_zero() * 30.0;
            fx.spawn(packs.as_ref(), fx::EFFECT_HIT, attacker + towards, Vec3::ZERO);
        } else {
            fx.spawn(packs.as_ref(), fx::EFFECT_HIT, spark, Vec3::new(0.0, 0.0, height));
        }
    }
    // Strong Body and mounted actors resist falling (`IsResistFallen`).
    let resist_fallen = combat::has_affect(victim.affect_flags, combat::AFFECT_CHEONGEUN)
        || world.resource::<Actors>().by_vid.get(&victim.vid).is_some_and(|actor| actor.mount_vnum != 0);
    // Reaction.
    {
        let mut actors = world.resource_mut::<Actors>();
        let fight = actors.fights.entry(victim.vid).or_default();
        fight.invisible_until = now + attack.invisible_time;
        let facing = |r: f32| Vec2::new(r.to_radians().sin(), r.to_radians().cos());
        let back = facing(attacker_rotation).dot(facing(rotation)) >= 0.0;
        if victim.is_building() {
        } else if victim.is_stone() || victim.is_door() {
            // `__HitStone`.
            if fight.stunned {
                fight.dead = true;
                fight.pending = Some(ActorAction::Dead);
            } else {
                fight.shake_until = now + 0.1;
            }
        } else if !fight.dead && !fight.knocked_down {
            fight.shake_until = now + 0.1;
            if fight.stunned {
                fight.dead = true;
                fight.pending = Some(if attack.hitting_type == battle::HIT_TYPE_GREAT && !resist_fallen {
                    ActorAction::Knockdown { back }
                } else {
                    ActorAction::Dead
                });
            } else if !fight.using_skill && (!fight.attacking || attack.hitting_type == battle::HIT_TYPE_GREAT && !resist_fallen) {
                // Great hits interrupt attacks; ordinary hits respect `isLock`.
                fight.pending = Some(if attack.hitting_type == battle::HIT_TYPE_GREAT && !resist_fallen {
                    ActorAction::Knockdown { back }
                } else {
                    ActorAction::Damage { back }
                });
            }
        }
    }
    if matches!(world.resource::<Actors>().fights.get(&victim.vid).and_then(|fight| fight.pending), Some(ActorAction::Knockdown { .. }))
        && let Some(actor) = world.resource_mut::<Actors>().by_vid.get_mut(&victim.vid)
    {
        actor.remote.going = false;
        actor.moving = false;
        actor.move_action = None;
    }
}

/// `CInstanceBase::AttackProcess` for the main character: the playing attack
/// or skill motion's weapon sweep and splash area against every attackable
/// actor near enough.
#[allow(clippy::too_many_arguments)]
fn main_attack_process(
    world: &mut World,
    control: &mut PlayerControl,
    character: &mut Character,
    candidates: &[(u32, ActorCombat)],
    attackable: &impl Fn(u32) -> bool,
    position: Vec3,
    now: f32,
    frame: f32,
    heights: &dyn Fn(f32, f32) -> f32,
    blocked: &dyn Fn(Vec2) -> bool,
) {
    if !matches!(control.state, MoveState::Attacking | MoveState::Casting) {
        return;
    }
    let Some(elapsed) = character.motion_elapsed(now) else { return };
    let attack = {
        let Some(asset) = character
            .motion_handle()
            .and_then(|handle| world.resource::<Assets<m2_assets::race::RaceMotionAsset>>().get(handle))
        else {
            return;
        };
        asset.attack.clone()
    };
    let rotation = control.facing;
    let skill = control.motion_skill;
    for (vid, actor) in candidates {
        if !attackable(*vid) || actor.is_dead() {
            continue;
        }
        let victim_position = Vec3::new(actor.x as f32, -(actor.y as f32), heights(actor.x as f32, actor.y as f32));
        let reach = if actor.race == HUGE_RACE { 500.0 } else { 300.0 };
        if (victim_position - position).truncate().length_squared() >= reach * reach {
            continue;
        }
        let spheres = world.resource_scope(|world, mut actors: Mut<Actors>| {
            if actors.fights.get(vid).is_some_and(|fight| fight.invisible_until > now || fight.dead) {
                return None;
            }
            Some(victim_spheres(world, &mut actors, *vid, victim_position))
        });
        let Some(spheres) = spheres else { continue };
        // `__SplashAttackProcess` first, then `__NormalAttackProcess`.
        if let Some(area) = &mut control.splash
            && area.hit(*vid, &spheres, now, control.target)
        {
            let (data, enable) = (area.splash.attack, area.splash.enable_hit_process);
            attack_success(world, control, &data, position, rotation, actor, victim_position, skill, enable, blocked, now);
            continue;
        }
        if let Some(attack) = &attack
            && battle::normal_attack_hit(attack, &mut control.hits, elapsed, frame, position, rotation, combat::attack_reach(world.get_resource::<PlayerState>().map_or(0, |player| player.affect_flags)), *vid, &spheres, now)
        {
            attack_success(world, control, &attack.attack, position, rotation, actor, victim_position, skill, true, blocked, now);
        }
    }
    // `FlushVictimList`: where the pushed victims will stand.
    if !control.victims.is_empty() {
        let victims = std::mem::take(&mut control.victims);
        if let Some(mut net) = world.get_resource_mut::<Net>() {
            net.game.send_sync_positions(&victims);
        }
    }
}

/// The main character's own motion events: sounds (no frequency check, as
/// `SoundEventProcess(!IsMain)`), effects, and the `SpecialAttacking` splash
/// areas of skills.
fn main_motion_events(world: &mut World, control: &mut PlayerControl, character: &mut Character, mut position: Vec3, now: f32, blocked: &dyn Fn(Vec2) -> bool) -> Vec3 {
    let tick = character.take_motion_tick(world, now);
    let own_target = world.resource::<FlyTargets>().peek(control.main_vid) == Some(ShotTarget::Actor(control.main_vid));
    let target = if own_target { Some((control.main_vid, position + Vec3::Z * fx::FLY_TARGET_HEIGHT)) }
        else { shot_target(world, control.main_vid, control.target, control.base, position.z, &tick) };
    for event in &tick.events {
        if matches!(event.kind, m2_assets::race::RaceMotionEventKind::Warp)
            && let Some((_, target)) = target
        {
            let destination = skill_warp_position(position, target);
            if !blocked(Vec2::new(destination.x, -destination.y)) {
                position = destination;
                let heading = Vec2::new(target.x - position.x, position.y - target.y);
                set_advancing_rotation(control, facing_degrees(heading));
                send_stop(world, control.base, Vec2::new(position.x, -position.y), control.advancing);
            }
        }
        if let m2_assets::race::RaceMotionEventKind::SpecialAttacking(splash) = &event.kind {
            control.splash = Some(battle::SplashArea::new(
                splash,
                position,
                control.facing,
                now,
                event.duration,
                control.motion_skill,
            ));
        }
    }
    dispatch_motion_shots(world, &tick, control.motion_skill, target.is_some());
    motion_tick_effects(world, character, &tick, fx::bevy_position(position), position, control.facing, false, target);
    position
}

/// `CNormalBowAttack_FlyEventHandler::OnShoot`, exactly once per fly event.
fn dispatch_motion_shots(world: &mut World, tick: &MotionTick, skill: u32, has_target: bool) {
    if !has_target { return; }
    for event in &tick.events {
        if matches!(event.kind, m2_assets::race::RaceMotionEventKind::Fly { .. })
            && let Some(mut net) = world.get_resource_mut::<Net>()
        {
            net.send_shoot(skill);
        }
    }
}

fn skill_warp_position(from: Vec3, target: Vec3) -> Vec3 {
    target - (target - from).normalize_or_zero() * 270.0
}

/// Monsters' and other players' swings against the main character
/// (`AttackProcess` of every instance, without a packet): the victim shakes,
/// shows the hit and, when standing, plays its damage motion.
///
/// `AttackProcess` only tests victims the attacker `IsAttackableInstance`s,
/// and `CheckAttacking` skips both sides standing in a safe zone; a remote
/// instance has no duel mode of its own (`SetDuelMode` is main-only).
#[allow(clippy::too_many_arguments)]
fn actor_attacks_on_main(
    world: &mut World,
    control: &mut PlayerControl,
    character: &mut Character,
    candidates: &[(u32, ActorCombat)],
    me: &ActorCombat,
    combat_world: &combat::CombatWorld,
    base: (u32, u32),
    position: Vec3,
    now: f32,
    frame: f32,
    heights: &dyn Fn(f32, f32) -> f32,
) {
    let main_vid = me.vid;
    if me.in_safe {
        return;
    }
    let remote_world = combat::CombatWorld { observer: false, duel_mode: combat::DUEL_NONE, ..combat_world.clone() };
    let may_hit = |vid: u32| {
        candidates
            .iter()
            .find(|(candidate, _)| *candidate == vid)
            .is_some_and(|(_, attacker)| !attacker.in_safe && combat::is_attackable(&remote_world, attacker, me))
    };
    let attackers: Vec<(u32, Vec3, f32, f32, f32, Option<m2_assets::race::MotionAttackData>)> = {
        let actors = world.resource::<Actors>();
        let motions = world.resource::<Assets<m2_assets::race::RaceMotionAsset>>();
        actors
            .fights
            .iter()
            .filter(|(vid, fight)| fight.attacking && !fight.dead && may_hit(**vid))
            .filter_map(|(vid, _)| {
                let actor = actors.by_vid.get(vid)?;
                let character = actors.characters.get(vid)?;
                let elapsed = character.motion_elapsed(now)?;
                let attack = motions.get(character.motion_handle()?)?.attack.clone();
                let local = Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32);
                let at = Vec3::new(local.x, -local.y, heights(local.x, local.y));
                ((at - position).truncate().length_squared() < 300.0 * 300.0 || actors.fights.get(vid)?.splash.is_some())
                    .then_some((*vid, at, actor.rotation, elapsed, combat::attack_reach(actor.affect_flags), attack))
            })
            .collect()
    };
    if attackers.is_empty() || control.dead {
        return;
    }
    let mut spheres = character.spheres(world, m2_formats::race::COLLISION_TYPE_DEFENDING);
    if spheres.is_empty() {
        spheres.push(DynamicSphere::still(position + Vec3::Z * 100.0, 70.0));
    }
    for (vid, at, rotation, elapsed, reach, attack) in attackers {
        let hit = world.resource_scope(|_, mut actors: Mut<Actors>| {
            let fight = actors.fights.entry(vid).or_default();
            if let Some(splash) = &mut fight.splash && splash.hit(main_vid, &spheres, now, None) {
                return Some(splash.splash.attack);
            }
            if let Some(attack) = &attack
                && battle::normal_attack_hit(attack, &mut fight.hits, elapsed, frame, at, rotation, reach, main_vid, &spheres, now)
            { return Some(attack.attack); }
            None
        });
        let Some(attack) = hit.filter(|attack| attack.hitting_type != battle::HIT_TYPE_NONE) else { continue };
        let height = (at.x - position.x).atan2(position.y - at.y).to_degrees();
        let packs = world.get_resource::<M2Packs>().cloned();
        world.resource_mut::<CombatFx>().spawn(packs.as_ref(), fx::EFFECT_HIT, position, Vec3::new(0.0, 0.0, height));
        control.shake_until = now + 0.1;
        if control.state == MoveState::Idle && !control.dead {
            let facing = |r: f32| Vec2::new(r.to_radians().sin(), r.to_radians().cos());
            let back = facing(rotation).dot(facing(control.facing)) >= 0.0;
            let motion = if attack.hitting_type == battle::HIT_TYPE_GREAT && !character.is_mounted() && !combat::has_affect(me.affect_flags, combat::AFFECT_CHEONGEUN) {
                Motion::DamageFlying { back }
            } else {
                Motion::Damage { back }
            };
            if let (Some(once), Some(wait)) = (character.motion_path(motion), character.motion_path(Motion::Wait)) {
                if let Motion::DamageFlying { back } = motion
                    && let Some(stand) = character.motion_path(Motion::StandUp { back })
                {
                    character.play_sequence(world, &once, &[(&stand, false), (&wait, true)]);
                } else {
                    character.play_then(world, &once, &wait);
                }
            }
        }
    }
}

/// Every remote instance also tests its swings against other remote actors.
/// This produces local feedback only; damage remains the server's decision.
#[allow(clippy::too_many_arguments)]
fn actor_attacks_on_actors(
    world: &mut World,
    candidates: &[(u32, ActorCombat)],
    combat_world: &combat::CombatWorld,
    now: f32,
    frame: f32,
    heights: &dyn Fn(f32, f32) -> f32,
) {
    let remote_world = combat::CombatWorld { observer: false, duel_mode: combat::DUEL_NONE, ..combat_world.clone() };
    let attackers: Vec<_> = {
        let actors = world.resource::<Actors>();
        let motions = world.resource::<Assets<m2_assets::race::RaceMotionAsset>>();
        candidates.iter().filter_map(|(vid, attacker)| {
            let fight = actors.fights.get(vid)?;
            if !fight.attacking || fight.dead || attacker.in_safe { return None; }
            let character = actors.characters.get(vid)?;
            let elapsed = character.motion_elapsed(now)?;
            let attack = motions.get(character.motion_handle()?)?.attack.clone();
            let rotation = actors.by_vid.get(vid)?.rotation;
            Some((*vid, *attacker, rotation, elapsed, attack))
        }).collect()
    };
    for (vid, attacker, rotation, elapsed, attack) in attackers {
        let at = Vec3::new(attacker.x as f32, -attacker.y as f32, heights(attacker.x as f32, attacker.y as f32));
        let has_splash = world.resource::<Actors>().fights.get(&vid).is_some_and(|fight| fight.splash.is_some());
        for (victim_vid, victim) in candidates {
            if *victim_vid == vid || victim.in_safe || !combat::is_attackable(&remote_world, &attacker, victim) { continue; }
            let position = Vec3::new(victim.x as f32, -victim.y as f32, heights(victim.x as f32, victim.y as f32));
            let normal_reach = if victim.race == HUGE_RACE { 500.0 } else { 300.0 };
            if !has_splash && (position - at).truncate().length_squared() >= normal_reach * normal_reach { continue; }
            let hit = world.resource_scope(|world, mut actors: Mut<Actors>| {
                if actors.fights.get(victim_vid).is_some_and(|fight| fight.dead || fight.invisible_until > now) { return None; }
                let spheres = victim_spheres(world, &mut actors, *victim_vid, position);
                let fight = actors.fights.entry(vid).or_default();
                if let Some(splash) = &mut fight.splash
                    && splash.hit(*victim_vid, &spheres, now, None)
                {
                    return Some(splash.splash.attack);
                }
                if (position - at).truncate().length_squared() < normal_reach * normal_reach
                    && let Some(attack) = &attack
                    && battle::normal_attack_hit(attack, &mut fight.hits, elapsed, frame, at, rotation,
                        combat::attack_reach(attacker.affect_flags), *victim_vid, &spheres, now)
                {
                    return Some(attack.attack);
                }
                None
            });
            if let Some(attack) = hit {
                attack_feedback(world, &attack, at, rotation, victim, position, now);
            }
        }
    }
}

/// `CInstanceBase::CheckAdvancing` + `CPythonBackground::CheckAdvancing` for
/// the main character's move from `from` to `to` (map-local): blocked map
/// cells stop it, buildings and trees slide it round or stop it, and other
/// actors' bodies stop it. Returns where it ends up.
#[allow(clippy::too_many_arguments)]
fn check_advancing(
    world: &mut World,
    scene: &MapScene,
    character: &mut Character,
    from: Vec2,
    to: Vec2,
    base: (u32, u32),
    main_vid: u32,
    attacking: bool,
    using_skill: bool,
) -> Vec2 {
    if to == from {
        return to;
    }
    if !scene.contains_position(to) {
        // The current sector may still be loading; only block crossing into
        // another sector until its terrain and collision objects are ready.
        let size = TERRAIN_WORLD;
        if (from.x / size).floor() != (to.x / size).floor() || (from.y / size).floor() != (to.y / size).floor() { return from; }
    }
    let height = |p: Vec2| scene.ground_height(p.x, p.y);
    // Body sphere of the main character (`m_BodyPointInstanceList`); it is
    // re-placed for the move, as `UpdateAdvancingPointInstance` does.
    let body_type = if attacking { m2_formats::race::COLLISION_TYPE_DEFENDING } else { m2_formats::race::COLLISION_TYPE_BODY };
    let mut bodies = character.spheres(world, body_type);
    let me = Vec3::new(from.x, -from.y, height(from));
    if bodies.is_empty() {
        bodies.push(DynamicSphere::still(me + Vec3::Z * 100.0, DEFAULT_BODY_RADIUS));
    }
    let step = Vec3::new(to.x - from.x, -(to.y - from.y), 0.0);
    for body in &mut bodies {
        let offset = body.position - me;
        body.last = me + offset;
        body.position = me + offset + step;
    }

    // Other actors: `TestActorCollision` on body spheres (defending ones while
    // attacking); two collisions or one that sliding does not resolve stop
    // the move.
    let others: Vec<(u32, Vec3)> = world
        .resource::<Actors>()
        .by_vid
        .iter()
        // `IsUsingSkill`: a skill motion goes through every actor.
        .filter(|_| !using_skill)
        .filter(|(vid, actor)| **vid != main_vid && actor.state_flags & combat::STATE_DEAD == 0)
        .map(|(vid, actor)| {
            let local = Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32);
            (*vid, Vec3::new(local.x, -local.y, height(local)))
        })
        .filter(|(_, p)| (*p - me).truncate().length_squared() <= 800.0 * 800.0)
        .collect();
    for (vid, at) in others {
        let other = world.resource_scope(|world, mut actors: Mut<Actors>| {
            if actors.fights.get(&vid).is_some_and(|fight| fight.dead) {
                return Vec::new();
            }
            let spheres = actors
                .characters
                .get_mut(&vid)
                .filter(|c| c.has_spheres(body_type))
                .map(|c| c.spheres(world, body_type))
                .unwrap_or_default();
            if spheres.is_empty() { vec![DynamicSphere::still(at + Vec3::Z * 100.0, DEFAULT_BODY_RADIUS)] } else { spheres }
        });
        if battle::actor_blocks(&bodies, &other) {
            return from;
        }
    }

    // Map attributes: `ATTRIBUTE_BLOCK` every 10 units along the move and at
    // its end.
    let length = (to - from).length();
    let steps = (length / 10.0) as i32;
    for i in 1..=steps {
        let p = from + (to - from) * (i as f32 / steps as f32);
        if map_attr(scene, p, ATTR_BLOCK) {
            return from;
        }
    }
    if map_attr(scene, to, ATTR_BLOCK) {
        return from;
    }

    // Buildings and trees: `AvoidObject` slides the move, `IsBlockObject`
    // stops it when the slide still collides.
    let Some(body) = bodies.first().copied() else { return to };
    let shapes = scene.collisions_near(body.position, body.radius + length + 50.0);
    if shapes.is_empty() {
        return to;
    }
    let (movement, blocked) = battle::adjust_movement(&shapes, body, ACTOR_SLIDE_MOVE_SPEED.max(length));
    if blocked {
        return from;
    }
    let slid = from + Vec2::new(movement.x, -movement.y);
    if map_attr(scene, slid, ATTR_BLOCK) { from } else { slid }
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
        Some(visual) => character.set_weapon(world, visual.vnum, &visual.model, &visual.sub_model, visual.weapon_type, f32::from(visual.specular) / 100.0),
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
        let cooldown = combat::skill_cooldown(&data, combat::skill_power(level, grade), points, points.casting_speed);
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
    // `__UseSkill`'s toggle branch: an active toggle only sends the packet to
    // turn it off, and does not play the motion again.
    let toggle_affect = match vnum { 63 => Some(19), 78 => Some(26), 79 => Some(25), _ => None };
    let toggle_active = toggle_affect.is_some_and(|affect| combat::has_affect(me.affect_flags, affect));
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
    if control.state == MoveState::Casting { return; }
    let check = SkillCheck {
        level,
        grade,
        level_limit_enforced: true,
        mounted: mount,
        in_safe: me.in_safe,
        fishing: false,
        weapon_type: character.weapon_type().unwrap_or(combat::weapon::NONE),
        arrows,
        cool_time_enforced: true,
        cool_time_left: cool_left,
        empty_bottles: world.get_resource::<PlayerState>().map_or(0, |player| u32::from(player.has_item(27995))),
        poison_bottles: world.get_resource::<PlayerState>().map_or(0, |player| u32::from(player.has_item(27996))),
        affected_by_dash: combat::has_affect(me.affect_flags, combat::AFFECT_DASH),
        is_charge_reserved: control.reserved_skill.is_some_and(|(skill, _, _)| skill == vnum),
    };
    if let Err(error) = combat::check_skill_usable(&data, &check, points) {
        fail(world, error);
        return;
    }

    let candidate = |vid: u32| if vid == me.vid { Some(*me) } else { candidates.iter().find(|(candidate, _)| *candidate == vid).map(|(_, actor)| *actor) };
    let needs_target = data.needs_target() || data.can_change_direction() || data.is_auto_search_target();
    let mut target = needs_target.then(|| control.target.and_then(candidate)).flatten();
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
                        if data.is_charge() && !check.is_charge_reserved {
                            if let Some(mut net) = world.get_resource_mut::<Net>() { net.send_use_skill(vnum, 0); }
                            let cooldown = combat::skill_cooldown(&data, combat::skill_power(level, grade), points, points.casting_speed);
                            if let Some(mut state) = world.get_resource_mut::<combat::SkillState>() { state.run_cool_time(vnum, cooldown, now); }
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

    if let Some(actor) = target {
        world.resource_mut::<FlyTargets>().set(control.main_vid, ShotTarget::Actor(actor.vid), false);
        if let Some(mut net) = world.get_resource_mut::<Net>() {
            net.send_fly_targeting(actor.vid, actor.x + base.0 as i32, actor.y + base.1 as i32, false);
        }
    }

    let mut extra_shots = 0;
    // Fan/circle multi-target skills (`GetTargetCount`).
    if let Some(actor) = target {
        let count = combat::skill_target_count(&data, combat::skill_power(level, grade), points);
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
                if let Some(other) = candidate(vid) {
                    world.resource_mut::<FlyTargets>().set(control.main_vid, ShotTarget::Actor(vid), true);
                    extra_shots += 1;
                    if let Some(mut net) = world.get_resource_mut::<Net>() {
                        net.send_fly_targeting(vid, other.x + base.0 as i32, other.y + base.1 as i32, true);
                    }
                }
            }
            while extra_shots + 1 < count.min(16) {
                let offset = ((now * 977.0 + extra_shots as f32 * 137.5).sin()) * 10.0;
                let heading = (control.advancing + offset).to_radians();
                let at = local + Vec2::new(heading.sin(), heading.cos()) * 8000.0;
                world.resource_mut::<FlyTargets>().set(control.main_vid, ShotTarget::Position(Vec3::new(at.x, -at.y, 0.0)), true);
                if let Some(mut net) = world.get_resource_mut::<Net>() {
                    net.send_fly_targeting(0, at.x as i32 + base.0 as i32, at.y as i32 + base.1 as i32, true);
                }
                extra_shots += 1;
            }
        }
    }

    // `NEW_UseSkill`: the skill's registered motion, unless the data says none.
    let motion = (!data.no_motion).then(|| combat::skill_motion_index(&data, grade));
    if let Some(motion) = motion
        && !character.play_skill(world, motion)
    {
        trace!("skill {vnum}: no motion for index {motion}");
    }
    if motion.is_some() {
        let loops = combat::skill_motion_loop_count(&data, combat::skill_power(level, grade), points);
        character.set_motion_loop_count(if extra_shots > 0 { loops.max(1) + extra_shots } else { loops });
    }
    control.reserved_skill = None;
    control.reserved_attack = None;
    control.reserved_talk = None;
    control.reserved_item = None;
    control.smart_held = false;
    if !data.is_moving_skill() { control.destination = None; }
    if motion.is_some() {
        control.combo = 0;
        control.pre_input = false;
        control.state = MoveState::Casting;
        control.action_started = now;
        // `m_kCurMotNode.uSkill`: the motion's hits are this skill's.
        control.motion_skill = vnum;
        control.hits.clear();
        control.splash = None;
    }
    if let Some(mut net) = world.get_resource_mut::<Net>() {
        // `NEW_UseSkill` -> `__OnUseSkill` -> `OnUseSkill`: `FUNC_SKILL |
        // motion` with the loop count (and bit 4 for the moving skill, horse
        // dash 137), then `__SendUseSkill`.
        if let Some(motion) = motion {
            let mut arg = combat::skill_motion_loop_count(&data, combat::skill_power(level, grade), points).min(15) as u8;
            if data.is_moving_skill() {
                arg |= 1 << 4;
            }
            let (x, y) = global_of(base, local);
            net.send_move(FUNC_SKILL | (motion as u8 & 0x7f), arg, control.advancing, x, y);
        }
        net.send_use_skill(vnum, target_vid);
    }
    let cooldown = combat::skill_cooldown(&data, combat::skill_power(level, grade), points, points.casting_speed);
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
    control.reserved_attack = None;
    control.reserved_talk = None;
    control.reserved_item = None;
    control.smart_held = false;
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
    let mob = world
        .get_resource::<MobCatalog>()
        .and_then(|mobs| mobs.by_race.get(&(vnum as u16)))
        .cloned();
    match mob {
        Some(mob) if mob.race_file.is_some() => character.set_mount(world, vnum, mob.race_file.as_deref().unwrap(), mob.motions.wait.as_deref(), mob.motions.run.as_deref()),
        _ => {
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
    scene.is_attr_on(local, flag)
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
    let actors: Vec<(u32, Entity)> = world
        .get_resource::<Actors>()
        .map(|actors| {
            actors.entities.iter()
                .filter(|(vid, _)| actors.by_vid.get(*vid).is_some_and(|actor| actor.combat(**vid, base).can_pick()))
                .map(|(&vid, &entity)| (vid, entity))
                .collect()
        })
        .unwrap_or_default();
    let camera = {
        let mut query = world.query_filtered::<(&Camera, &GlobalTransform), With<SceneCamera>>();
        query.single(world).ok().map(|(camera, transform)| (camera.clone(), *transform))
    };
    let (camera, transform) = camera?;
    let mut best: Option<(f32, u32)> = None;
    for (vid, entity) in actors {
        let Some(transform_actor) = world.get::<Transform>(entity) else { continue };
        let root = transform_actor.translation;
        // Test several points up the actor's rendered height. The old fixed
        // map-origin point missed actors on raised terrain and taller NPCs.
        for height in [40.0, 100.0, 170.0, 250.0] {
            let position = root + Vec3::Y * height;
            let Ok(screen) = camera.world_to_viewport(&transform, position) else { continue };
            let distance = screen.distance(cursor);
            if distance <= 78.0 && best.is_none_or(|(best_distance, _)| distance < best_distance) {
                best = Some((distance, vid));
            }
        }
    }
    best.map(|(_, vid)| vid)
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
    let now = world.get_resource::<Time>().map_or(0.0, Time::elapsed_secs);
    let shake = world.get_resource::<ScreenWave>().filter(|wave| wave.until > now)
        .map_or(Vec3::ZERO, |wave| Vec3::new((now * 97.0).sin(), (now * 131.0).sin(), 0.0) * wave.power);
    let eye = target + offset + shake;
    if let Some(mut shadow_target) = world.get_resource_mut::<crate::render::shadow::SceneCameraTarget>() {
        shadow_target.0 = Some(target);
    }
    let mut camera = world.query_filtered::<&mut Transform, With<SceneCamera>>();
    if let Ok(mut transform) = camera.single_mut(world) {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fly_target_packets_keep_shooters_and_position_targets_separate() {
        let mut targets = FlyTargets::default();
        targets.set(5, ShotTarget::Actor(10), false);
        targets.set(5, ShotTarget::Position(Vec3::new(30.0, -40.0, 60.0)), true);
        targets.set(6, ShotTarget::Actor(11), false);
        assert_eq!(targets.shot(5), Some(ShotTarget::Actor(10)));
        assert_eq!(targets.shot(5), Some(ShotTarget::Position(Vec3::new(30.0, -40.0, 60.0))));
        assert_eq!(targets.shot(6), Some(ShotTarget::Actor(11)));
        targets.set(5, ShotTarget::Actor(12), false);
        assert_eq!(targets.shot(5), Some(ShotTarget::Actor(12)));
    }

    #[test]
    fn warp_curtain_stays_opaque_while_loading_and_fades_back_in() {
        let start = Instant::now() - Duration::from_secs_f32(WARP_FADE_SECONDS * 2.0);
        let mut transition = WarpTransition { started: Some(start), cleared: true, fading_in: None };
        assert_eq!(transition.alpha(), 1.0);
        transition.fading_in = Some(start);
        assert_eq!(transition.alpha(), 0.0);
    }

    #[test]
    fn warp_keeps_old_scene_until_faded_and_waits_for_destination() {
        let mut app = App::new();
        app.add_message::<NetMessage>()
            .insert_resource(GameWorld { state: Some(State::Failed("old scene".into())), ..default() })
            .init_resource::<WarpTransition>()
            .init_resource::<Actors>()
            .init_resource::<FlyTargets>()
            .init_resource::<CombatFx>()
            .init_resource::<PlayerControl>()
            .init_resource::<TargetBoard>()
            .add_systems(Update, (update_warp_transition, clear_world_on_warp).chain());
        app.world_mut().write_message(NetMessage::Game(NetEvent::Warp { x: 1, y: 2, addr: 0, port: 1 }));
        app.update();
        assert_eq!(app.world().resource::<GameWorld>().error(), Some("old scene"));
        assert!(!app.world().resource::<WarpTransition>().ready_to_connect());
        app.world_mut().resource_mut::<WarpTransition>().started = Some(Instant::now() - Duration::from_secs_f32(WARP_FADE_SECONDS * 2.0));
        app.update();
        assert!(app.world().resource::<WarpTransition>().ready_to_connect());
        assert!(app.world().resource::<GameWorld>().state.is_none());
        app.update();
        assert!(app.world().resource::<WarpTransition>().fading_in.is_none());
        app.world_mut().resource_mut::<GameWorld>().state = Some(State::Failed("destination failed".into()));
        app.update();
        assert!(app.world().resource::<WarpTransition>().fading_in.is_some());
    }

    #[test]
    fn enemy_motion_fly_event_launches_the_targeted_arrow() {
        use m2_assets::race::{RaceMotionEvent, RaceMotionEventKind};
        use std::sync::Arc;
        let dir = std::env::var_os("M2_CLIENT_DIR").map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"));
        if !dir.join("pack/Index.dev").exists() {
            eprintln!("SKIP: client data not found (set M2_CLIENT_DIR)");
            return;
        }
        let mut manager = m2_formats::pack::PackManager::new(&dir);
        m2_formats::pack::load_index(&mut manager, "pack", "pack/Index.dev").unwrap();
        let packs = Arc::new(manager);
        let mut world = World::new();
        world.insert_resource(M2Packs { reader: m2_assets::source::PackAssetReader::new(packs.clone()), packs });
        world.insert_resource(CombatFx::default());
        world.insert_resource(Actors::default());
        world.insert_resource(GameWorld { main_vid: 7, local: Some((500, 400)), ..default() });
        let mut targets = FlyTargets::default();
        targets.set(3, ShotTarget::Actor(7), false);
        world.insert_resource(targets);
        let character = Character::spawn_actor(&mut world, 1, "", Transform::IDENTITY, None);
        let tick = MotionTick { events: vec![RaceMotionEvent {
            frame: 10, start: 0.0, duration: 0.0,
            kind: RaceMotionEventKind::Fly { file: "d:/ymir work/pc/assassin/effect/arrow_01_fast.msf".into(), bone: None, position: Vec3::ZERO },
        }], ..default() };
        let target = shot_target(&mut world, 3, None, (0, 0), 0.0, &tick);
        assert_eq!(target, Some((7, Vec3::new(500.0, -400.0, fx::FLY_TARGET_HEIGHT))));
        motion_tick_effects(&mut world, &character, &tick, Vec3::ZERO, Vec3::ZERO, 0.0, true, target);
        assert_eq!(world.resource::<CombatFx>().fly_targets(), vec![7]);
    }

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
    fn packed_riding_horses_have_the_saddle_bone() {
        use m2_formats::pack::{PackManager, load_index};
        let dir = std::env::var_os("M2_CLIENT_DIR").map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"));
        if !dir.join("pack/Index.dev").exists() { return; }
        let mut packs = PackManager::new(&dir);
        load_index(&mut packs, "pack", "pack/Index.dev").unwrap();
        let index = m2_assets::DirectoryIndex::build(&packs);
        let races = load_race_registrations(&packs, &index);
        for race in [20104, 20107] {
            let file = races[&race].race_file.as_deref().unwrap();
            let mut race_data = m2_formats::race::RaceData::default();
            assert!(race_data.load(&packs, paths::virtual_name(file), |_| true));
            let model_file = proto_text(&race_data.base_model_file_name);
            let model = m2_formats::gr2::parse(&packs.get(paths::virtual_name(&model_file)).unwrap()).unwrap();
            let skeleton = model.models[0].skeleton.as_ref().unwrap();
            assert!(skeleton.bones.iter().any(|bone| bone.name.eq_ignore_ascii_case(b"saddle")), "race {race} has no saddle bone");
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
                name: String::new(), race: 0, kind, level: 0, armor: 0, weapon: 0, hair: 0, move_speed: 100, attack_speed: 100, walk_mode: false,
                empire: 0, pk_mode: 0, guild_id: 0, alignment: 0, state_flags: 0, affect_flags: 0, mount_vnum: 0,
                x: 0, y: 0, rotation: 0.0, rendered_rotation: 0.0, move_from: Vec2::ZERO, move_to: Vec2::ZERO,
                moving: false, rendered_moving: Some(false), move_action: None, remote: RemoteMotion::default(),
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
        let mut armed = update(11, 0);
        if let NetMessage::Game(NetEvent::ActorUpdate(data)) = &mut armed {
            data.weapon = 19;
            data.move_speed = 120;
        }
        app.world_mut().write_message(armed);
        app.update();
        assert!(app.world().resource::<Actors>().characters.contains_key(&10));
        assert!(app.world().resource::<Actors>().characters.contains_key(&11));
        assert_eq!(app.world().resource::<Actors>().by_vid[&11].weapon, 19);
        assert_eq!(app.world().resource::<Actors>().by_vid[&11].move_speed, 120);
        let mut catalog = ItemCatalog::default();
        catalog.0.items.insert(19, m2_formats::proto::ItemData {
            model_file_name: b"d:/ymir work/item/weapon/00019.gr2".to_vec(),
            table: m2_formats::proto::ItemTable { sub_type: 0, ..default() },
            ..default()
        });
        app.insert_resource(catalog);
        sync_remote_equipment(app.world_mut(), &HashMap::new());
        {
            let actors = app.world().resource::<Actors>();
            let character = actors.characters.get(&11).unwrap();
            assert_eq!(character.weapon_vnum(), 19);
            assert_eq!(character.weapon_type(), Some(0));
            assert_eq!(character.motion_mode(), crate::render::character::MotionMode::OneHandSword);
        }
        app.world_mut().write_message(update(11, 0));
        app.update();
        sync_remote_equipment(app.world_mut(), &HashMap::new());
        assert_eq!(app.world().resource::<Actors>().characters[&11].weapon_type(), None);
        app.world_mut().resource_scope(|world, mut actors: Mut<Actors>| {
            let character = actors.characters.get_mut(&11).unwrap();
            let root = character.root;
            assert!(character.change_look(world, Look { race: 0, shape: 3, hair: 2 }, 0.0));
            assert_eq!(character.root, root);
            assert_eq!((character.look.shape, character.look.hair), (3, 2));
        });

        app.world_mut().write_message(update(11, 1));
        app.update();
        let actors = app.world().resource::<Actors>();
        assert!(!actors.characters.contains_key(&11));
        assert_eq!(actors.by_vid[&11].rendered_moving, None);
    }

    #[test]
    fn remote_player_follows_move_and_zero_duration_stop_without_snapping() {
        use m2_net::phase::{ActorData, ActorMove, ACTOR_TYPE_PC};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).init_resource::<Actors>().init_resource::<GameWorld>()
            .init_resource::<MobCatalog>().add_message::<NetMessage>().add_systems(Update, track_actors);
        let actor = ActorData {
            vid: 9, kind: ACTOR_TYPE_PC, race: 0, move_speed: 100, attack_speed: 100, state_flags: 0,
            rotation: 0.0, x: 1000, y: 2000, affect_flags: 0, name: Some("rucadi".into()),
            guild_id: 0, level: 1, alignment: 0, pk_mode: 0, empire: 2, armor: 0, weapon: 19, hair: 0, mount_vnum: 0,
        };
        app.world_mut().write_message(NetMessage::Game(NetEvent::ActorAppend(actor)));
        app.update();
        let movement = |func, x, duration| NetMessage::Game(NetEvent::ActorMove(ActorMove {
            vid: 9, func, arg: 0, rotation: 90.0, x, y: 2000, time: 0, duration,
        }));
        app.world_mut().write_message(movement(FUNC_MOVE, 1100, 0));
        app.update();
        assert_eq!(app.world().resource::<Actors>().by_vid[&9].x, 1000);
        advance_actor_states(&mut app.world_mut().resource_mut::<Actors>(), 0.1, None);
        assert_eq!(app.world().resource::<Actors>().by_vid[&9].x, 1045);
        app.world_mut().write_message(movement(FUNC_WAIT, 1150, 0));
        app.update();
        assert_eq!(app.world().resource::<Actors>().by_vid[&9].x, 1045);
        advance_actor_states(&mut app.world_mut().resource_mut::<Actors>(), 0.1, None);
        assert_eq!(app.world().resource::<Actors>().by_vid[&9].x, 1090);
        advance_actor_states(&mut app.world_mut().resource_mut::<Actors>(), 0.1, None);
        advance_actor_states(&mut app.world_mut().resource_mut::<Actors>(), 0.1, None);
        let actors = app.world().resource::<Actors>();
        assert_eq!(actors.by_vid[&9].x, 1150);
        assert_eq!(actors.by_vid[&9].weapon, 19);
    }

    pub(super) fn remote_pc(x: i32) -> Actor {
        Actor {
            name: String::new(), race: 0, kind: m2_net::phase::ACTOR_TYPE_PC, level: 0, armor: 0, weapon: 0, hair: 0, move_speed: 100, attack_speed: 100, walk_mode: false,
            empire: 0, pk_mode: 0, guild_id: 0, alignment: 0, state_flags: 0, affect_flags: 0, mount_vnum: 0,
            x, y: 0, rotation: 0.0, rendered_rotation: 0.0, move_from: Vec2::ZERO, move_to: Vec2::ZERO,
            moving: false, rendered_moving: None, move_action: None,
            remote: RemoteMotion::default(),
        }
    }

    pub(super) fn command(func: u8, x: i32, time: u32) -> ActorMove {
        ActorMove { vid: 9, func, arg: 0, rotation: 90.0, x, y: 0, time, duration: 0 }
    }

    #[test]
    fn remote_move_runs_on_then_returns_to_its_last_position() {
        let mut actors = Actors::default();
        let mut actor = remote_pc(0);
        actor.remote.commands.push_back((0, command(FUNC_MOVE, 100, 0)));
        actors.by_vid.insert(9, actor);
        // `MovementProcess` keeps a MOVE walking until 100 units past its target...
        let mut furthest = 0;
        for _ in 0..40 {
            advance_actor_states(&mut actors, 0.05, None);
            furthest = furthest.max(actors.by_vid[&9].x);
        }
        assert!((200..=225).contains(&furthest), "ran to {furthest}");
        // ...then aims back at it and stops there.
        assert_eq!(actors.by_vid[&9].x, 100);
        assert!(!actors.by_vid[&9].moving);
    }

    #[test]
    fn remote_commands_wait_for_their_check_time() {
        let mut actors = Actors::default();
        let mut actor = remote_pc(0);
        actor.remote.commands.push_back((1_000, command(FUNC_MOVE, 500, 900)));
        actors.by_vid.insert(9, actor);
        advance_actor_states(&mut actors, 0.1, Some(990));
        assert_eq!((actors.by_vid[&9].x, actors.by_vid[&9].moving), (0, false));
        advance_actor_states(&mut actors, 0.1, Some(1_000));
        assert_eq!((actors.by_vid[&9].x, actors.by_vid[&9].moving), (45, true));
    }

    #[test]
    fn a_push_keeps_blending_while_the_actor_moves() {
        let mut actors = Actors::default();
        let mut actor = remote_pc(0);
        actor.kind = 0; // `TYPE_ENEMY`
        actor.remote.push = Some(PushBlend::new(Vec2::ZERO, Vec2::new(0.0, 100.0)));
        // A monster's move to x 1000 over four seconds starts with the push.
        actor.remote.commands.push_back((0, ActorMove { duration: 4000, ..command(FUNC_WAIT, 1000, 0) }));
        actors.by_vid.insert(9, actor);
        for _ in 0..41 {
            advance_actor_states(&mut actors, 0.05, None);
        }
        let actor = &actors.by_vid[&9];
        // Both happened: about half the walk at the packet's pace and the whole push.
        assert!((495..=520).contains(&actor.x), "x {}", actor.x);
        assert!((95..=100).contains(&actor.y), "y {}", actor.y);
        assert!(actor.remote.push.is_none());
    }

    #[test]
    fn a_close_attack_command_plays_in_place() {
        let mut actors = Actors::default();
        let mut actor = remote_pc(0);
        actor.remote.commands.push_back((0, command(FUNC_ATTACK, 30, 0)));
        actors.by_vid.insert(9, actor);
        advance_actor_states(&mut actors, 0.016, None);
        assert_eq!(actors.by_vid[&9].x, 30);
        assert_eq!(actors.fights[&9].pending, Some(ActorAction::Attack));
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
                    weapon: 0,
                    hair: 0,
                    move_speed: 100,
                    attack_speed: 100,
                    walk_mode: false,
                    empire: 0, pk_mode: 0, guild_id: 0, alignment: 0, state_flags: 0, affect_flags: 0, mount_vnum: 0,
                    x,
                    y,
                    rotation: 0.0,
                    rendered_rotation: 0.0,
                    move_from: Vec2::new(x as f32, y as f32),
                    move_to: Vec2::new(x as f32, y as f32),
                    moving: false,
                    rendered_moving: None,
                    move_action: None,
                    remote: RemoteMotion::default(),
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

/// `CPhysicsObject`'s blend of a push: `100 · c_fFrameTime`.
const PUSH_SECONDS: f32 = 2.0;

/// `CPhysicsObject::IncreaseExternalForce` (mass 1, friction 0.3): the push
/// `force` gives along `direction` (Metin2 x/y), stepped like `Accumulate`
/// for at most 100 frames. `None` without a direction.
fn external_force_movement(direction: Vec2, force: f32) -> Option<Vec2> {
    const FRICTION: f32 = 0.3;
    const EPSILON: f32 = 0.001;
    let mut direction = direction.try_normalize()?;
    let mut velocity = direction * force;
    let mut movement = Vec2::ZERO;
    for _ in 0..100 {
        // `Accumulate`: the z velocity is always 0, so friction always acts.
        velocity -= direction * FRICTION;
        if velocity.x * direction.x < EPSILON {
            velocity.x = 0.0;
            direction.x = 0.0;
        }
        if velocity.y * direction.y < EPSILON {
            velocity.y = 0.0;
            direction.y = 0.0;
        }
        movement += velocity;
        if velocity.x.abs() < EPSILON && velocity.y.abs() < EPSILON {
            break;
        }
    }
    Some(movement)
}

/// One character's `CPythonTextTail` tail: the name (in its name colour), the
/// level and alignment title left of it and the guild name above, at the
/// projected point `GetHeight() + 10` above the feet.
#[derive(Clone, Debug)]
pub struct NameTag {
    pub vid: u32,
    /// Viewport position (logical pixels) of the bottom centre of the name.
    pub position: Vec2,
    pub name: String,
    pub color: [u8; 3],
    /// `UpdateTextTailLevel` ("Lv %d"), for players.
    pub level: Option<u32>,
    /// `RefreshTextTail`'s alignment title and its colour.
    pub title: Option<(&'static str, [u8; 3])>,
    pub guild: Option<String>,
    /// The colour of the line above the name: white for a guild, yellow for
    /// a ground item's owner (`SetItemTextTailOwner`).
    pub guild_color: [u8; 3],
}

/// The tails to draw this frame (`ShowAllTextTail` within 35 m while names
/// are always shown, the C++ default; else the hovered and targeted ones).
#[derive(Resource, Default)]
pub struct NameTags {
    pub tags: Vec<NameTag>,
    /// `systemSetting.IsAlwaysShowName()` (`DEFAULT_VALUE_ALWAYS_SHOW_NAME`).
    pub always_show: bool,
    initialised: bool,
}

/// `colorInfo.CHR_NAME_RGB_*`.
const NAME_RGB_PC: [u8; 3] = [255, 215, 76];
const NAME_RGB_NPC: [u8; 3] = [122, 231, 93];
const NAME_RGB_MOB: [u8; 3] = [235, 22, 9];
/// `CPythonTextTail::ShowAllTextTail`'s range.
const NAME_TAG_RANGE: f32 = 3500.0;

/// `GetAlignmentGrade` -> `g_TitleNameMap` (`PVP_LEVEL0..8`) and
/// `colorInfo.TITLE_RGB_*`; grade 4 (`TITLE_NONE`, neutral) has no title.
fn alignment_title(alignment: i32) -> Option<(&'static str, [u8; 3])> {
    let grade = match alignment {
        12000.. => 0,
        8000.. => 1,
        4000.. => 2,
        1000.. => 3,
        0.. => 4,
        -3999.. => 5,
        -7999.. => 6,
        -11999.. => 7,
        _ => 8,
    };
    const TITLES: [(&str, [u8; 3]); 9] = [
        ("Chivalric", [0, 204, 255]),
        ("Noble", [0, 144, 255]),
        ("Good", [92, 110, 255]),
        ("Friendly", [155, 155, 255]),
        ("Neutral", [255, 255, 255]),
        ("Aggressive", [207, 117, 0]),
        ("Fraudulent", [235, 83, 0]),
        ("Malicious", [227, 0, 0]),
        ("Cruel", [255, 0, 0]),
    ];
    (grade != 4).then(|| TITLES[grade])
}

/// `CPythonTextTail::UpdateAllTextTail` + `UpdateShowingTextTail`: which tails
/// show and where, for the UI to draw.
fn update_name_tags(world: &mut World) {
    if !world.resource::<NameTags>().initialised {
        let mut tags = world.resource_mut::<NameTags>();
        tags.always_show = true;
        tags.initialised = true;
    }
    let camera = {
        let mut query = world.query_filtered::<(&Camera, &GlobalTransform), With<SceneCamera>>();
        query.single(world).ok().map(|(camera, transform)| (camera.clone(), *transform))
    };
    let game = world.resource::<GameWorld>();
    let (Some((camera, eye)), Some(State::Ready { character, .. })) = (camera, game.state.as_ref()) else {
        world.resource_mut::<NameTags>().tags.clear();
        return;
    };
    let main_vid = game.main_vid;
    let control = world.resource::<PlayerControl>();
    let (hover, target) = (control.hover, control.target);
    let show_all = world.resource::<NameTags>().always_show
        || world.resource::<ButtonInput<KeyCode>>().pressed(KeyCode::AltLeft);
    let head = |chr: &Character| {
        let root = world.get::<GlobalTransform>(chr.root)?.translation();
        // `GetHeight() + 10`: the head bone stands in for the bounding box top.
        let top = chr.bone_transform(world, "Bip01 Head").map_or(root.y + 180.0, |bone| bone.translation().y + 30.0);
        Some(Vec3::new(root.x, top + 10.0, root.z))
    };
    let main_at = world.get::<GlobalTransform>(character.root).map(|t| t.translation());
    let mut tags = Vec::new();
    let mut add = |vid: u32, at: Option<Vec3>, tag: NameTag| {
        let Some(at) = at else { return };
        let near = main_at.is_some_and(|me| (me - at).xz().length() < NAME_TAG_RANGE);
        if !((show_all && near) || Some(vid) == hover || Some(vid) == target || vid == main_vid && show_all) {
            return;
        }
        if let Ok(position) = camera.world_to_viewport(&eye, at) {
            tags.push(NameTag { position, ..tag });
        }
    };
    // The main character.
    if let Some(player) = world.get_resource::<PlayerState>() {
        let level = player.points.get(1).copied().filter(|level| *level > 0).map(|level| level as u32);
        let guild = world
            .get_resource::<Net>()
            .and_then(|net| net.game.guild_names().iter().find(|name| !name.is_empty()).cloned())
            .filter(|_| player.guild_id != 0);
        add(
            main_vid,
            head(character),
            NameTag {
                vid: main_vid,
                position: Vec2::ZERO,
                name: player.name.clone(),
                color: NAME_RGB_PC,
                level,
                title: alignment_title(i32::from(player.alignment)),
                guild,
                guild_color: [255, 255, 255],
            },
        );
    }
    let actors = world.resource::<Actors>();
    let mobs = world.resource::<MobCatalog>();
    for (&vid, actor) in &actors.by_vid {
        // Monsters appended before `mob_proto` loaded have no name yet.
        let name = if actor.name.is_empty() {
            mobs.by_race.get(&actor.race).map(|mob| mob.name.clone()).unwrap_or_default()
        } else {
            actor.name.clone()
        };
        if vid == main_vid || name.is_empty() {
            continue;
        }
        let Some(chr) = actors.characters.get(&vid) else { continue };
        let combat = actor.combat(vid, game.base);
        if !combat.can_pick() || actors.fights.get(&vid).is_some_and(|fight| fight.dead) {
            continue;
        }
        let pc = combat.is_pc();
        let color = if pc {
            NAME_RGB_PC
        } else if combat.is_npc() {
            NAME_RGB_NPC
        } else {
            NAME_RGB_MOB
        };
        add(
            vid,
            head(chr),
            NameTag {
                vid,
                position: Vec2::ZERO,
                name,
                color,
                level: (pc && actor.level > 0).then_some(actor.level),
                title: if pc { alignment_title(i32::from(actor.alignment)) } else { None },
                guild: None,
                guild_color: [255, 255, 255],
            },
        );
    }
    // `RegisterItemTextTail`: ground items show their name (`c_TextTail_Item_Color`,
    // white) within the same range, and `SetItemTextTailOwner`'s "<name>'s" in yellow.
    if show_all
        && let (Some(items), Some(catalog)) = (world.get_resource::<crate::ground::GroundItems>(), world.get_resource::<ItemCatalog>())
    {
        for item in items.by_vid.values() {
            let Some(at) = item.entity().and_then(|entity| world.get::<GlobalTransform>(entity)).map(GlobalTransform::translation) else { continue };
            if !main_at.is_some_and(|me| (me - at).xz().length() < NAME_TAG_RANGE) {
                continue;
            }
            let Some(data) = catalog.get(item.vnum) else { continue };
            if let Ok(position) = camera.world_to_viewport(&eye, at + Vec3::Y * 20.0) {
                tags.push(NameTag {
                    vid: 0,
                    position,
                    name: data.table.locale_name_utf8(1252).into_owned(),
                    color: [255, 255, 255],
                    level: None,
                    title: None,
                    guild: (!item.owner.is_empty()).then(|| format!("{}'s", item.owner)),
                    guild_color: [255, 255, 0],
                });
            }
        }
    }
    world.resource_mut::<NameTags>().tags = tags;
}

/// `CActorInstance::TYPE_WARP`.
const ACTOR_TYPE_WARP: u8 = 3;

/// What a `CPythonMiniMap` mark stands for (its `whitemark.sub` colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniMapMark {
    Pc,
    Npc,
    Monster,
    Warp,
}

/// The minimap's input (`CPythonMiniMap::Update`): the map, where the main
/// character stands and faces, the camera's heading and the characters near.
#[derive(Resource, Default)]
pub struct MiniMap {
    /// The map's folder (`metin2_map_b1`), whose terrains hold `minimap.dds`.
    pub map: Option<String>,
    /// Map-local position of the main character, Metin2 units.
    pub center: Vec2,
    /// Global position, for the coordinates line.
    pub global: (i32, i32),
    /// The main character's heading and the camera's yaw, degrees clockwise
    /// from north as the map draws it.
    pub facing: f32,
    pub camera: f32,
    /// Map-local positions of the other characters.
    pub marks: Vec<(Vec2, MiniMapMark)>,
    pub targets: Vec<Vec2>,
}

/// Target updates carry global map coordinates; actor targets follow the
/// visible actor, falling back to their last server-provided position.
fn quest_target_position(target: &crate::social::QuestTarget, actors: &Actors, base: (u32, u32)) -> Option<Vec2> {
    let global = actors.by_vid.get(&target.vid).map(|actor| (actor.x, actor.y)).or(target.position)?;
    Some(Vec2::new((global.0 - base.0 as i32) as f32, (global.1 - base.1 as i32) as f32))
}

fn update_minimap(world: &mut World) {
    let game = world.resource::<GameWorld>();
    let (Some(State::Ready { .. }), Some(map), Some(local)) = (game.state.as_ref(), game.map.clone(), game.local) else {
        world.resource_mut::<MiniMap>().map = None;
        return;
    };
    let base = game.base;
    let main_vid = game.main_vid;
    let center = Vec2::new(local.0 as f32, local.1 as f32);
    let control = world.resource::<PlayerControl>();
    let (facing, camera) = (control.facing, -control.yaw.to_degrees());
    let actors = world.resource::<Actors>();
    let marks = actors
        .by_vid
        .iter()
        .filter(|(vid, actor)| **vid != main_vid && actor.state_flags & combat::STATE_DEAD == 0)
        .filter_map(|(&vid, actor)| {
            let combat = actor.combat(vid, base);
            if !combat.can_pick() || actors.fights.get(&vid).is_some_and(|fight| fight.dead) {
                return None;
            }
            let kind = if combat.is_pc() {
                MiniMapMark::Pc
            } else if actor.kind == ACTOR_TYPE_WARP {
                MiniMapMark::Warp
            } else if combat.is_npc() {
                MiniMapMark::Npc
            } else if combat.is_building() || combat.is_door() {
                return None;
            } else {
                MiniMapMark::Monster
            };
            Some((Vec2::new((actor.x - base.0 as i32) as f32, (actor.y - base.1 as i32) as f32), kind))
        })
        .collect();
    let targets = world.get_resource::<crate::social::SocialState>().map(|social| {
        social.targets.values().filter_map(|target| quest_target_position(target, actors, base)).collect()
    }).unwrap_or_default();
    let mut minimap = world.resource_mut::<MiniMap>();
    minimap.targets = targets;
    minimap.map = Some(map);
    minimap.center = center;
    minimap.global = (local.0 + base.0 as i32, local.1 + base.1 as i32);
    minimap.facing = facing;
    minimap.camera = camera;
    minimap.marks = marks;
}

#[cfg(test)]
mod push_tests {
    use super::*;

    #[test]
    fn external_force_slows_by_the_friction_each_frame() {
        // Force 3: velocities 2.7, 2.4, ... 0.3 then 0 (≈ 13.5 units).
        let movement = external_force_movement(Vec2::new(1.0, 0.0), 3.0).unwrap();
        assert!((movement.x - 13.5).abs() < 0.01, "{movement}");
        assert_eq!(movement.y, 0.0);
        assert!(external_force_movement(Vec2::ZERO, 3.0).is_none());
    }
}

/// A mesh part's own material while [`apply_stealth`] has it drawn see-through.
#[derive(Component)]
struct OpaqueMaterial(Handle<m2_render::FixedFunctionMaterial>);

/// `__Assassin_SetEunhyeongAffect` / `__SetReviveInvisibilityAffect` and
/// `AFFECT_INVISIBILITY`: under stealth the main character is drawn at half
/// opacity (`BlendAlphaValue(0.5)`), everyone else not at all (alpha 0 with
/// their effects hidden). The C++ blends the alpha over a second; here it
/// switches at once.
fn apply_stealth(world: &mut World, mut applied: Local<HashMap<Entity, f32>>) {
    let stealthy = |flags: u64| {
        combat::has_affect(flags, combat::AFFECT_EUNHYEONG) || combat::has_affect(flags, combat::AFFECT_REVIVE_INVISIBILITY)
    };
    let mut wanted: Vec<(Entity, f32)> = Vec::new();
    if let Some(State::Ready { character, .. }) = world.resource::<GameWorld>().state.as_ref() {
        let flags = world.get_resource::<PlayerState>().map_or(0, |player| player.affect_flags);
        wanted.push((character.root, if world.get::<crate::render::character::MotionHidden>(character.root).is_some() { 0.0 } else if stealthy(flags) { 0.5 } else { 1.0 }));
    }
    {
        let actors = world.resource::<Actors>();
        for (vid, character) in &actors.characters {
            let flags = actors.by_vid.get(vid).map_or(0, |actor| actor.affect_flags);
            let hidden = world.get::<crate::render::character::MotionHidden>(character.root).is_some() || stealthy(flags) || combat::has_affect(flags, combat::AFFECT_INVISIBILITY);
            wanted.push((character.root, if hidden { 0.0 } else { 1.0 }));
        }
    }
    applied.retain(|root, _| wanted.iter().any(|(entity, _)| entity == root));
    for (root, alpha) in wanted {
        // Opaque characters that were never touched need no walk.
        if alpha >= 1.0 && applied.get(&root).is_none_or(|previous| *previous >= 1.0) {
            continue;
        }
        if world.get_entity(root).is_err() {
            continue;
        }
        let visibility = if alpha <= 0.0 { Visibility::Hidden } else { Visibility::Inherited };
        if world.get::<Visibility>(root).is_some_and(|current| *current != visibility) {
            world.entity_mut(root).insert(visibility);
        }
        // New parts (a weapon, a respawned body) appear later, so the parts
        // are walked every frame while the alpha is not 1.
        let mut parts = Vec::new();
        let mut stack = vec![root];
        while let Some(entity) = stack.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                stack.extend(children.iter());
            }
            if world.get::<MeshMaterial3d<m2_render::FixedFunctionMaterial>>(entity).is_some() {
                parts.push(entity);
            }
        }
        for part in parts {
            let original = world.get::<OpaqueMaterial>(part).map(|opaque| opaque.0.clone());
            if alpha >= 1.0 || alpha <= 0.0 {
                if let Some(original) = original {
                    world.entity_mut(part).insert(MeshMaterial3d(original)).remove::<OpaqueMaterial>();
                }
                continue;
            }
            if original.is_some() {
                continue;
            }
            let current = world.get::<MeshMaterial3d<m2_render::FixedFunctionMaterial>>(part).expect("part has a material").0.clone();
            let Some(mut material) = world.resource::<Assets<m2_render::FixedFunctionMaterial>>().get(&current).cloned() else {
                continue;
            };
            // The diffuse alpha scales the texture's (`MODULATE(TEXTURE, DIFFUSE)`).
            material.diffuse.w = alpha;
            material.stages[0].alpha_op = m2_render::TextureOp::Modulate;
            material.stages[0].alpha_arg1 = m2_render::TextureArg::TEXTURE;
            material.stages[0].alpha_arg2 = m2_render::TextureArg::DIFFUSE;
            material.blend = Some(m2_render::Blend::ALPHA);
            let translucent = crate::render::cached_material(world, material);
            world.entity_mut(part).insert((MeshMaterial3d(translucent), OpaqueMaterial(current)));
        }
        applied.insert(root, alpha);
    }
}

#[cfg(test)]
#[path = "skill_tests.rs"]
mod skill_tests;
