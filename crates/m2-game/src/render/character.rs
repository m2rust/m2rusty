//! Player characters: a race (`.msm`) dressed in a body shape and a hair, playing
//! a motion. The port of `CInstanceBase::SetRace`/`SetArmor`/`SetHair` for
//! players (`CActorInstance::SetShape`/`SetHair`).
//!
//! A [`Character`] spawns its root at once and builds itself as the assets
//! arrive; call [`Character::update`] every frame. The race files per race come
//! from `playersettingmodule.py` (the Python registered them; the port has no
//! Python, so they are here).

use std::collections::HashMap;

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle};
use bevy::animation::{AnimationClip, AnimationPlayer};
use bevy::asset::{AssetServer, Assets, LoadState, UntypedAssetId};
use bevy::prelude::*;
use m2_assets::gr2::{Gr2Model, Gr2Spawn};
use m2_assets::paths;
use m2_assets::race::{RaceAsset, RaceMotionAsset, RaceSkin};
use m2_formats::pack::PackManager;
use m2_formats::proto::parse_item_proto;

use super::model;

/// The files of one player race (`MAIN_RACE_*` order).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaceFiles {
    /// The race data script in the root pack (`chrmgr.LoadLocalRaceData`).
    pub msm: &'static str,
    /// The folder of the select-screen motions (`SetIntroMotions`).
    pub intro: &'static str,
    /// The folder of the normal motions (`SetGeneralMotions`).
    pub general: &'static str,
}

/// `playersettingmodule.py`'s `LoadGameData("INIT")`: warrior (m), assassin (f),
/// sura (m), shaman (f), then warrior (f), assassin (m), sura (f), shaman (m).
const RACES: [RaceFiles; 8] = [
    RaceFiles { msm: "warrior_m.msm", intro: "d:/ymir work/pc/warrior/intro/", general: "d:/ymir work/pc/warrior/general/" },
    RaceFiles { msm: "assassin_w.msm", intro: "d:/ymir work/pc/assassin/intro/", general: "d:/ymir work/pc/assassin/general/" },
    RaceFiles { msm: "sura_m.msm", intro: "d:/ymir work/pc/sura/intro/", general: "d:/ymir work/pc/sura/general/" },
    RaceFiles { msm: "shaman_w.msm", intro: "d:/ymir work/pc/shaman/intro/", general: "d:/ymir work/pc/shaman/general/" },
    RaceFiles { msm: "warrior_w.msm", intro: "d:/ymir work/pc2/warrior/intro/", general: "d:/ymir work/pc2/warrior/general/" },
    RaceFiles { msm: "assassin_m.msm", intro: "d:/ymir work/pc2/assassin/intro/", general: "d:/ymir work/pc2/assassin/general/" },
    RaceFiles { msm: "sura_w.msm", intro: "d:/ymir work/pc2/sura/intro/", general: "d:/ymir work/pc2/sura/general/" },
    RaceFiles { msm: "shaman_m.msm", intro: "d:/ymir work/pc2/shaman/intro/", general: "d:/ymir work/pc2/shaman/general/" },
];

/// The files of player race `race` (0..8).
pub fn race_files(race: u8) -> Option<RaceFiles> {
    RACES.get(usize::from(race)).copied()
}

/// The normal motions (`playersettingmodule.py`'s `SetGeneralMotions` plus the
/// mode's combo attack, which is what `CInstanceBase::NEW_Attack` runs for a
/// player).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Wait,
    Walk,
    Run,
    Attack,
}

/// `CRaceMotionData::EMode` as selected by `CInstanceBase::RefreshState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionMode {
    General,
    OneHandSword,
    TwoHandSword,
    DualHandSword,
    Bow,
    Fan,
    Bell,
}

impl MotionMode {
    /// The motion folder under a race's root (`playersettingmodule.py`'s
    /// `SetPathName(path + "onehand_sword/")` and friends).
    pub fn folder(self) -> &'static str {
        match self {
            MotionMode::General => "general/",
            MotionMode::OneHandSword => "onehand_sword/",
            MotionMode::TwoHandSword => "twohand_sword/",
            MotionMode::DualHandSword => "dualhand_sword/",
            MotionMode::Bow => "bow/",
            MotionMode::Fan => "fan/",
            MotionMode::Bell => "bell/",
        }
    }

    /// The attack motion registered as `MOTION_COMBO_ATTACK_1` for the mode.
    /// Only `GENERAL` registers `MOTION_NORMAL_ATTACK`; players attack with
    /// combos (`CInstanceBase::NEW_Attack` -> `InputComboAttack`).
    pub fn attack_file(self) -> &'static str {
        match self {
            MotionMode::General | MotionMode::Bow => "attack.msa",
            _ => "combo_01.msa",
        }
    }
}

impl Motion {
    /// The `.msa` of this motion for `race` in `mode`. `SetGeneralMotions`
    /// registers `wait`/`walk`/`run`; the attack is the mode's
    /// `MOTION_COMBO_ATTACK_1` file.
    pub fn file_in(self, race: u8, mode: MotionMode) -> Option<String> {
        let files = race_files(race)?;
        let root = files.general.strip_suffix("general/").unwrap_or(files.general);
        let name = match self {
            Motion::Wait => "wait.msa",
            Motion::Walk => "walk.msa",
            Motion::Run => "run.msa",
            Motion::Attack => mode.attack_file(),
        };
        Some(format!("{root}{}{name}", mode.folder()))
    }

    /// The `.msa` of this motion in the general (no weapon) mode.
    pub fn file(self, race: u8) -> Option<String> {
        self.file_in(race, MotionMode::General)
    }
}

/// The select-screen motions (`chr.MOTION_INTRO_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroMotion {
    Wait,
    Selected,
    NotSelected,
}

impl IntroMotion {
    /// The `.msa` of this motion for `race`.
    pub fn file(self, race: u8) -> Option<String> {
        let name = match self {
            IntroMotion::Wait => "wait.msa",
            IntroMotion::Selected => "selected.msa",
            IntroMotion::NotSelected => "not_selected.msa",
        };
        race_files(race).map(|r| format!("{}{name}", r.intro))
    }
}

/// `CItemData::WEAPON_*` (`client/include/Client/Game/ItemData.h`), the
/// `TItemTable::bSubType` of a weapon.
pub const WEAPON_SWORD: u8 = 0;
pub const WEAPON_DAGGER: u8 = 1;
pub const WEAPON_BOW: u8 = 2;
pub const WEAPON_TWO_HANDED: u8 = 3;
pub const WEAPON_BELL: u8 = 4;
pub const WEAPON_FAN: u8 = 5;
pub const WEAPON_ARROW: u8 = 6;
/// `CItemData::WEAPON_NUM_TYPES`; `WEAPON_NONE` is `NUM_TYPES + 1`. Any value
/// the switch does not name takes the `default` branch (general mode).
pub const WEAPON_NONE: u8 = 8;

/// Which hand(s) a weapon is drawn in: `CActorInstance::__IsRightHandWeapon` /
/// `__IsLeftHandWeapon` (`ActorInstanceAttach.cpp`). Daggers are dual-wielded,
/// a bow is held in the left hand, everything else in the right. The mounted
/// fan case is not reachable here until horses are ported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponHands {
    Right,
    Left,
    Both,
}

/// `__IsRightHandWeapon`/`__IsLeftHandWeapon` for an unmounted actor.
pub fn weapon_hands(weapon_type: u8) -> WeaponHands {
    match weapon_type {
        WEAPON_DAGGER => WeaponHands::Both,
        WEAPON_BOW => WeaponHands::Left,
        _ => WeaponHands::Right,
    }
}

/// `CInstanceBase::RefreshState`'s motion-mode switch for an unmounted actor.
pub fn weapon_motion_mode(weapon_type: Option<u8>) -> MotionMode {
    match weapon_type {
        Some(WEAPON_SWORD) => MotionMode::OneHandSword,
        Some(WEAPON_TWO_HANDED) => MotionMode::TwoHandSword,
        Some(WEAPON_DAGGER) => MotionMode::DualHandSword,
        Some(WEAPON_BOW) => MotionMode::Bow,
        Some(WEAPON_FAN) => MotionMode::Fan,
        Some(WEAPON_BELL) => MotionMode::Bell,
        Some(WEAPON_ARROW) | None => MotionMode::General,
        Some(_) => MotionMode::General,
    }
}

/// `playersettingmodule.py`'s `RegisterAttachingBoneName` calls for a player
/// race: `(right hand, left hand)`. Warriors use `equip_right_hand`; the other
/// classes `equip_right`/`equip_left` (only the dual-wield classes define the
/// left bone).
pub fn weapon_bones(race: u8) -> (Option<&'static str>, Option<&'static str>) {
    match race {
        0 | 4 => (Some("equip_right_hand"), None),
        1 | 5 | 3 | 7 => (Some("equip_right"), Some("equip_left")),
        2 | 6 => (Some("equip_right"), None),
        _ => (None, None),
    }
}

/// Armor vnum to body shape: each item's `value[3]` (`item_proto`).
#[derive(Resource, Debug, Clone, Default)]
pub struct ArmorShapes(pub HashMap<u32, u32>);

impl ArmorShapes {
    /// Reads `<locale_path>/item_proto` (`locale/en` by default, `MULTI_LOCALE_PATH`).
    pub fn load(packs: &PackManager, locale_path: &str) -> Result<Self, String> {
        let file = format!("{locale_path}/item_proto");
        let data = packs.get(file.as_bytes()).map_err(|e| format!("{file}: {e}"))?;
        let items = parse_item_proto(&data).map_err(|e| format!("{file}: {e:?}"))?;
        Ok(ArmorShapes(items.iter().map(|item| (item.vnum, item.values[3] as u32)).collect()))
    }

    /// `__ArmorVnumToShape` with `ARMOR_SPECULAR_ENABLE` (as `constInfo.py` sets it):
    /// 0 and 1 are shapes already; an item's shape is its `value[3]`; an unknown vnum
    /// is used as the shape.
    pub fn shape(&self, armor: u32) -> u32 {
        if armor <= 1 {
            return armor;
        }
        self.0.get(&armor).copied().unwrap_or(armor)
    }
}

/// What a player looks like: race, body shape and hair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    pub race: u8,
    pub shape: u32,
    pub hair: u32,
}

enum Stage {
    Race(Handle<RaceAsset>),
    Models { body: (Handle<Gr2Model>, Vec<RaceSkin>), hair: Option<(Handle<Gr2Model>, Vec<RaceSkin>)> },
    Spawned(Gr2Spawn),
    Failed,
}

struct Playing {
    msa: Handle<RaceMotionAsset>,
    /// The `.msa` path, for the log.
    file: String,
    clip: Option<Handle<AnimationClip>>,
    repeat: bool,
    started: bool,
}

/// The equipped weapon models (`CActorInstance::AttachWeapon`). The right hand
/// uses the item's main model, the left hand its sub-model; each is attached to
/// the race's weapon bone once both assets are loaded.
struct Weapon {
    vnum: u32,
    right: Option<Handle<Gr2Model>>,
    left: Option<Handle<Gr2Model>>,
    /// The spawned mesh entities, despawned when the weapon changes.
    attached: Vec<Entity>,
    ready: bool,
}

/// One renderable character in the world, whether player controlled or not.
pub struct Character {
    /// Owned by the caller: place it with its `Transform`, despawn it to remove the
    /// character. The model hangs under it once loaded.
    pub root: Entity,
    pub look: Look,
    /// The `.msm` used for this character. Non-player races are supplied by
    /// the mob race registry instead of the player race table.
    race_file: String,
    player_appearance: bool,
    stage: Stage,
    motion: Option<Playing>,
    /// `m_wcurMotionMode` (`CActorInstance::SetMotionMode`).
    motion_mode: MotionMode,
    weapon: Option<Weapon>,
    /// Why the character cannot be drawn (unknown race, a missing file).
    pub error: Option<String>,
}

impl Character {
    /// Starts loading `look`; the root is spawned at `transform` under `parent`.
    pub fn spawn(world: &mut World, look: Look, transform: Transform, parent: Option<Entity>) -> Self {
        let race_file = race_files(look.race).map(|files| files.msm).unwrap_or("");
        Self::spawn_from_race_file(world, look, race_file, true, transform, parent)
    }

    /// Spawns an actor from a registered race data file (`CRaceManager`),
    /// including NPCs and monsters whose race number is not a player race.
    pub fn spawn_actor(world: &mut World, race: u16, file: &str, transform: Transform, parent: Option<Entity>) -> Self {
        let look = Look { race: 0, shape: 0, hair: 0 };
        let mut character = Self::spawn_from_race_file(world, look, file, false, transform, parent);
        world.entity_mut(character.root).insert(Name::new(format!("actor race {race}")));
        character.race_file = format!("race {race}: {file}");
        character
    }

    fn spawn_from_race_file(
        world: &mut World,
        look: Look,
        race_file: &str,
        player_appearance: bool,
        transform: Transform,
        parent: Option<Entity>,
    ) -> Self {
        let root = world.spawn((Name::new(format!("character race {}", look.race)), transform, Visibility::Visible)).id();
        if let Some(parent) = parent {
            world.entity_mut(root).insert(ChildOf(parent));
        }
        let (stage, error) = if race_file.is_empty() {
            (Stage::Failed, Some(format!("race {} has no race data", look.race)))
        } else {
            (Stage::Race(world.resource::<AssetServer>().load(paths::asset_source_uri(race_file))), None)
        };
        Character { root, look, race_file: race_file.to_owned(), player_appearance, stage, motion: None, motion_mode: MotionMode::General, weapon: None, error }
    }

    /// The model is spawned.
    pub fn is_ready(&self) -> bool {
        matches!(self.stage, Stage::Spawned(_))
    }

    /// Plays a `.msa` motion, looping or once (then holding its last frame).
    pub fn play(&mut self, world: &mut World, msa: &str, repeat: bool) {
        let file = msa.to_owned();
        let msa = world.resource::<AssetServer>().load(paths::asset_source_uri(msa));
        self.motion = Some(Playing { msa, file, clip: None, repeat, started: false });
    }

    /// Plays one of the race's normal motions in the current weapon motion mode
    /// (`CInstanceBase::RefreshState` after `SetMotionMode`).
    pub fn play_motion(&mut self, world: &mut World, motion: Motion, repeat: bool) {
        if let Some(file) = motion.file_in(self.look.race, self.motion_mode) {
            self.play(world, &file, repeat);
        }
    }

    /// Equips `vnum`'s weapon model (`CInstanceBase::SetWeapon` /
    /// `CActorInstance::AttachWeapon`). `model` is the right-hand model,
    /// `sub_model` the left-hand one; `weapon_type` selects the hands and the
    /// motion mode. Returns whether the weapon changed, so the caller can
    /// restart the wait motion (`ChangeWeapon`).
    pub fn set_weapon(
        &mut self,
        world: &mut World,
        vnum: u32,
        model: &str,
        sub_model: &str,
        weapon_type: u8,
    ) -> bool {
        if vnum == self.weapon.as_ref().map_or(0, |weapon| weapon.vnum) {
            return false;
        }
        self.clear_weapon(world);
        self.motion_mode = weapon_motion_mode((vnum != 0).then_some(weapon_type));
        if vnum == 0 || model.is_empty() {
            return true;
        }
        let hands = weapon_hands(weapon_type);
        let server = world.resource::<AssetServer>();
        let right = matches!(hands, WeaponHands::Right | WeaponHands::Both)
            .then(|| server.load(paths::asset_source_uri(model)));
        let left = matches!(hands, WeaponHands::Left | WeaponHands::Both)
            .then(|| server.load(paths::asset_source_uri(if sub_model.is_empty() { model } else { sub_model })));
        self.weapon = Some(Weapon { vnum, right, left, attached: Vec::new(), ready: false });
        true
    }

    /// The equipped weapon's vnum, or 0.
    pub fn weapon_vnum(&self) -> u32 {
        self.weapon.as_ref().map_or(0, |weapon| weapon.vnum)
    }

    /// Removes the weapon attachment (an empty weapon cell). Returns whether a
    /// weapon was equipped.
    pub fn clear_weapon(&mut self, world: &mut World) -> bool {
        let Some(weapon) = self.weapon.take() else { return false };
        for entity in weapon.attached {
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
            }
        }
        self.motion_mode = MotionMode::General;
        true
    }

    /// The last played motion has ended. False while it is still loading.
    pub fn motion_finished(&self, world: &World) -> bool {
        match &self.stage {
            Stage::Spawned(spawned) => world.get::<AnimationPlayer>(spawned.root).is_some_and(|player| player.all_finished()),
            _ => false,
        }
    }

    /// The requested motion has started playing (its clip is loaded).
    pub fn motion_started(&self) -> bool {
        self.motion.as_ref().is_some_and(|motion| motion.started)
    }

    /// Advances loading and starts the requested motion once everything is in.
    pub fn update(&mut self, world: &mut World) {
        if let Err(error) = self.advance(world) {
            warn!("character {}: {error}", self.race_file);
            self.error = Some(error);
            self.stage = Stage::Failed;
        }
        self.advance_weapon(world);
        self.update_motion(world);
    }

    fn advance(&mut self, world: &mut World) -> Result<(), String> {
        match &self.stage {
            Stage::Race(handle) => {
                let Some(race) = world.resource::<Assets<RaceAsset>>().get(handle).cloned() else {
                    return failed(world, handle.id().untyped());
                };
                let server = world.resource::<AssetServer>().clone();
                // `SetShape`: the shape's model (the base model when it names none) with its
                // skins. C++ draws nothing for a shape the race does not define; the port
                // falls back to the bare base model so the character stays visible.
                let (model, skins) = if !self.player_appearance {
                    (race.base_model.clone(), Vec::new())
                } else {
                    match race.shapes.get(&self.look.shape) {
                        Some(shape) if !shape.model.is_empty() => (shape.model.clone(), shape.skins.clone()),
                        Some(shape) => (race.base_model.clone(), shape.skins.clone()),
                        None => {
                            warn!("race {} has no shape {}; using its base model", self.look.race, self.look.shape);
                            (race.base_model.clone(), Vec::new())
                        }
                    }
                };
                // `SetHair` (`HAIR_COLOR_ENABLE`): a hair the race does not define is none.
                let hair = self
                    .player_appearance
                    .then(|| race.hairs.get(&self.look.hair))
                    .flatten()
                    .filter(|h| !h.model.is_empty())
                    .map(|h| (server.load(paths::asset_source_uri(&h.model)), h.skins.clone()));
                self.stage = Stage::Models { body: (server.load(paths::asset_source_uri(&model)), skins), hair };
                Ok(())
            }
            Stage::Models { body, hair } => {
                let models = world.resource::<Assets<Gr2Model>>();
                let Some(body_model) = models.get(&body.0).cloned() else {
                    return failed(world, body.0.id().untyped());
                };
                let hair_model = match hair {
                    Some((handle, _)) => match models.get(handle).cloned() {
                        Some(model) => Some(model),
                        // A hair that fails to load leaves the character bald, not invisible.
                        None if matches!(world.resource::<AssetServer>().get_load_state(handle.id()), Some(LoadState::Failed(_))) => None,
                        None => return Ok(()),
                    },
                    None => None,
                };
                let (body_skins, hair_skins) = (body.1.clone(), hair.as_ref().map(|h| h.1.clone()).unwrap_or_default());
                let spawned = model::spawn_model(world, &body_model, Some(self.root), &body_skins, None);
                if let Some(hair_model) = &hair_model {
                    model::attach_model(world, hair_model, &spawned, &hair_skins, None);
                }
                // Restart the motion on the new skeleton.
                if let Some(motion) = &mut self.motion {
                    motion.started = false;
                }
                self.stage = Stage::Spawned(spawned);
                Ok(())
            }
            Stage::Spawned(_) | Stage::Failed => Ok(()),
        }
    }

    /// Waits for the weapon models and attaches them to the race's weapon bones
    /// (`CActorInstance::AttachWeapon`). The C++ right/left parts are
    /// `CRaceData::PART_WEAPON`/`PART_WEAPON_LEFT`.
    fn advance_weapon(&mut self, world: &mut World) {
        let Stage::Spawned(spawned) = &self.stage else { return };
        let Some(weapon) = &mut self.weapon else { return };
        if weapon.ready {
            return;
        }
        let (right, left) = {
            let models = world.resource::<Assets<Gr2Model>>();
            let server = world.resource::<AssetServer>();
            let load = |handle: &Option<Handle<Gr2Model>>| match handle {
                Some(handle) => match models.get(handle).cloned() {
                    Some(model) => Ok(Some(model)),
                    None if matches!(server.get_load_state(handle.id()), Some(LoadState::Failed(_))) => Ok(None),
                    None => Err(()),
                },
                None => Ok(None),
            };
            match (load(&weapon.right), load(&weapon.left)) {
                (Ok(right), Ok(left)) => (right, left),
                _ => return,
            }
        };
        let (right_bone, left_bone) = weapon_bones(self.look.race);
        let mut parts = Vec::new();
        if let (Some(model), Some(bone)) = (&right, right_bone) {
            parts.extend(model::attach_model_to_bone(world, model, spawned, bone, &[], None));
        }
        if let (Some(model), Some(bone)) = (&left, left_bone) {
            parts.extend(model::attach_model_to_bone(world, model, spawned, bone, &[], None));
        }
        weapon.attached = parts;
        weapon.ready = true;
    }

    fn update_motion(&mut self, world: &mut World) {
        let (Stage::Spawned(spawned), Some(motion)) = (&self.stage, &mut self.motion) else { return };
        if motion.started {
            return;
        }
        if motion.clip.is_none() {
            let Some(msa) = world.resource::<Assets<RaceMotionAsset>>().get(&motion.msa) else { return };
            let clip = world.resource::<AssetServer>().load(paths::asset_source_uri(&msa.motion_file));
            motion.clip = Some(clip);
        }
        let Some(clip) = motion.clip.clone() else { return };
        if world.resource::<Assets<AnimationClip>>().get(&clip).is_none() {
            return;
        }
        let (graph, node) = AnimationGraph::from_clip(clip);
        let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
        let mut root = world.entity_mut(spawned.root);
        root.insert(AnimationGraphHandle(graph));
        if let Some(mut player) = root.get_mut::<AnimationPlayer>() {
            player.stop_all();
            let active = player.play(node);
            if motion.repeat {
                active.repeat();
            }
        }
        motion.started = true;
        info!("motion {} started", motion.file);
    }
}

/// `Ok` while the asset is loading, the load error once it failed.
fn failed(world: &World, id: UntypedAssetId) -> Result<(), String> {
    match world.resource::<AssetServer>().get_load_state(id) {
        Some(LoadState::Failed(error)) => Err(error.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn race_files_follow_playersettingmodule() {
        assert_eq!(race_files(5).unwrap().msm, "assassin_m.msm");
        assert_eq!(IntroMotion::Wait.file(5).unwrap(), "d:/ymir work/pc2/assassin/intro/wait.msa");
        assert_eq!(IntroMotion::Selected.file(0).unwrap(), "d:/ymir work/pc/warrior/intro/selected.msa");
        assert!(race_files(8).is_none());
    }

    #[test]
    fn armor_vnums_map_to_shapes() {
        let shapes = ArmorShapes([(11209, 3)].into_iter().collect());
        assert_eq!(shapes.shape(0), 0);
        assert_eq!(shapes.shape(1), 1);
        assert_eq!(shapes.shape(11209), 3);
        // Unknown vnums are used as the shape.
        assert_eq!(shapes.shape(77), 77);
    }

    #[test]
    fn weapon_type_selects_hands_and_motion_mode() {
        // `__IsRightHandWeapon`/`__IsLeftHandWeapon` and `RefreshState`.
        assert_eq!(weapon_hands(WEAPON_DAGGER), WeaponHands::Both);
        assert_eq!(weapon_hands(WEAPON_BOW), WeaponHands::Left);
        assert_eq!(weapon_hands(WEAPON_SWORD), WeaponHands::Right);
        assert_eq!(weapon_motion_mode(Some(WEAPON_DAGGER)), MotionMode::DualHandSword);
        assert_eq!(weapon_motion_mode(Some(WEAPON_SWORD)), MotionMode::OneHandSword);
        assert_eq!(weapon_motion_mode(Some(WEAPON_TWO_HANDED)), MotionMode::TwoHandSword);
        assert_eq!(weapon_motion_mode(Some(WEAPON_BOW)), MotionMode::Bow);
        assert_eq!(weapon_motion_mode(Some(WEAPON_FAN)), MotionMode::Fan);
        assert_eq!(weapon_motion_mode(Some(WEAPON_BELL)), MotionMode::Bell);
        assert_eq!(weapon_motion_mode(Some(WEAPON_ARROW)), MotionMode::General);
        assert_eq!(weapon_motion_mode(None), MotionMode::General);
    }

    #[test]
    fn weapon_motions_follow_playersettingmodule() {
        // Assassin (race 5) daggers: `dualhand_sword/wait.msa` and combo_01.
        assert_eq!(
            Motion::Wait.file_in(5, MotionMode::DualHandSword).unwrap(),
            "d:/ymir work/pc2/assassin/dualhand_sword/wait.msa"
        );
        assert_eq!(
            Motion::Attack.file_in(5, MotionMode::DualHandSword).unwrap(),
            "d:/ymir work/pc2/assassin/dualhand_sword/combo_01.msa"
        );
        assert_eq!(
            Motion::Attack.file_in(0, MotionMode::OneHandSword).unwrap(),
            "d:/ymir work/pc/warrior/onehand_sword/combo_01.msa"
        );
        assert_eq!(
            Motion::Attack.file_in(5, MotionMode::Bow).unwrap(),
            "d:/ymir work/pc2/assassin/bow/attack.msa"
        );
        // No weapon keeps the general folder and `attack.msa`.
        assert_eq!(Motion::Attack.file(0).unwrap(), "d:/ymir work/pc/warrior/general/attack.msa");
    }

    #[test]
    fn weapon_bones_follow_playersettingmodule() {
        assert_eq!(weapon_bones(0), (Some("equip_right_hand"), None));
        assert_eq!(weapon_bones(5), (Some("equip_right"), Some("equip_left")));
        assert_eq!(weapon_bones(3), (Some("equip_right"), Some("equip_left")));
    }
}
