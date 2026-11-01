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
}

/// `playersettingmodule.py`'s `LoadGameData("INIT")`: warrior (m), assassin (f),
/// sura (m), shaman (f), then warrior (f), assassin (m), sura (f), shaman (m).
const RACES: [RaceFiles; 8] = [
    RaceFiles { msm: "warrior_m.msm", intro: "d:/ymir work/pc/warrior/intro/" },
    RaceFiles { msm: "assassin_w.msm", intro: "d:/ymir work/pc/assassin/intro/" },
    RaceFiles { msm: "sura_m.msm", intro: "d:/ymir work/pc/sura/intro/" },
    RaceFiles { msm: "shaman_w.msm", intro: "d:/ymir work/pc/shaman/intro/" },
    RaceFiles { msm: "warrior_w.msm", intro: "d:/ymir work/pc2/warrior/intro/" },
    RaceFiles { msm: "assassin_m.msm", intro: "d:/ymir work/pc2/assassin/intro/" },
    RaceFiles { msm: "sura_w.msm", intro: "d:/ymir work/pc2/sura/intro/" },
    RaceFiles { msm: "shaman_m.msm", intro: "d:/ymir work/pc2/shaman/intro/" },
];

/// The files of player race `race` (0..8).
pub fn race_files(race: u8) -> Option<RaceFiles> {
    RACES.get(usize::from(race)).copied()
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

struct Motion {
    msa: Handle<RaceMotionAsset>,
    clip: Option<Handle<AnimationClip>>,
    repeat: bool,
    started: bool,
}

/// One player character in the world.
pub struct Character {
    /// Owned by the caller: place it with its `Transform`, despawn it to remove the
    /// character. The model hangs under it once loaded.
    pub root: Entity,
    pub look: Look,
    stage: Stage,
    motion: Option<Motion>,
    /// Why the character cannot be drawn (unknown race, a missing file).
    pub error: Option<String>,
}

impl Character {
    /// Starts loading `look`; the root is spawned at `transform` under `parent`.
    pub fn spawn(world: &mut World, look: Look, transform: Transform, parent: Option<Entity>) -> Self {
        let root = world.spawn((Name::new(format!("character race {}", look.race)), transform, Visibility::Visible)).id();
        if let Some(parent) = parent {
            world.entity_mut(root).insert(ChildOf(parent));
        }
        let (stage, error) = match race_files(look.race) {
            Some(files) => (Stage::Race(world.resource::<AssetServer>().load(paths::asset_source_uri(files.msm))), None),
            None => (Stage::Failed, Some(format!("race {} has no race data", look.race))),
        };
        Character { root, look, stage, motion: None, error }
    }

    /// The model is spawned.
    pub fn is_ready(&self) -> bool {
        matches!(self.stage, Stage::Spawned(_))
    }

    /// Plays a `.msa` motion, looping or once (then holding its last frame).
    pub fn play(&mut self, world: &mut World, msa: &str, repeat: bool) {
        let msa = world.resource::<AssetServer>().load(paths::asset_source_uri(msa));
        self.motion = Some(Motion { msa, clip: None, repeat, started: false });
    }

    /// Advances loading and starts the requested motion once everything is in.
    pub fn update(&mut self, world: &mut World) {
        if let Err(error) = self.advance(world) {
            warn!("character race {}: {error}", self.look.race);
            self.error = Some(error);
            self.stage = Stage::Failed;
        }
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
                let (model, skins) = match race.shapes.get(&self.look.shape) {
                    Some(shape) if !shape.model.is_empty() => (shape.model.clone(), shape.skins.clone()),
                    Some(shape) => (race.base_model.clone(), shape.skins.clone()),
                    None => {
                        warn!("race {} has no shape {}; using its base model", self.look.race, self.look.shape);
                        (race.base_model.clone(), Vec::new())
                    }
                };
                // `SetHair` (`HAIR_COLOR_ENABLE`): a hair the race does not define is none.
                let hair = race
                    .hairs
                    .get(&self.look.hair)
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
}
