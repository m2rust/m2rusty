//! Race data: `.msm` ([`RaceAsset`]) and motion attribute `.msa`
//! ([`RaceMotionAsset`]) loaders.
//!
//! `.msm` is the `RaceDataScript` (model file names, shapes and hair with their
//! skin replacements, attachments); `.msa` is the `MotionData` script of one
//! motion (its `.gr2` animation, duration, loop and the event list). Which
//! motion belongs to which mode/index is registered by the Python scripts
//! (`playersettingmodule.py`) and is **not** reproduced here:
//! [`list_motion_files`] gives the viewer the `.msa` files of a race folder
//! instead. Effects (`.mse`) and sound scripts are Phase 3 and not loaded.
//!
//! All names are UTF-8 canonical asset paths (see [`crate::paths`]), converted
//! lossily from the legacy bytes; positions are in Bevy space.

use std::collections::BTreeMap;

use bevy::asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy::math::Vec3;
use bevy::reflect::TypePath;
use m2_formats::race as fmt;
use m2_formats::script;

use crate::source::DirectoryIndex;
use crate::{Error, coords, paths};

fn name(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn path(bytes: &[u8]) -> String {
    paths::canonical_asset_path(&name(bytes))
}

/// `SourceSkin` / `TargetSkin`: texture `src` of the model is replaced by `dst`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaceSkin {
    pub part: i32,
    pub src: String,
    pub dst: String,
}

/// A body shape or hair: a model and its texture replacements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RaceShape {
    pub model: String,
    pub skins: Vec<RaceSkin>,
}

/// What an attachment is.
#[derive(Debug, Clone, PartialEq)]
pub enum RaceAttachmentKind {
    None,
    /// Collision spheres (`radius`, centre).
    Collision { collision_type: i32, spheres: Vec<(f32, Vec3)> },
    /// An effect script (`.mse`), with its offset and rotation (degrees as stored).
    Effect { file: String, position: Vec3, rotation: [f32; 3] },
    /// A model/object file.
    Object { file: String },
}

/// `AttachingData`: something fixed to a bone of a model.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceAttachment {
    pub is_attaching: bool,
    pub model_index: u32,
    pub bone: String,
    pub kind: RaceAttachmentKind,
}

/// A `.msm` race data script.
#[derive(Asset, TypePath, Debug, Clone, Default)]
pub struct RaceAsset {
    /// `BaseModelFileName`: the `.gr2` skeleton + body.
    pub base_model: String,
    /// `TreeFileName` (an `.spt`, for races that are trees).
    pub tree: String,
    pub attribute: String,
    pub smoke_bone: String,
    /// `MotionListFileName` (`motlist.txt` by default).
    pub motion_list: String,
    pub shapes: BTreeMap<u32, RaceShape>,
    pub hairs: BTreeMap<u32, RaceShape>,
    pub attachments: Vec<RaceAttachment>,
}

impl RaceAsset {
    /// The directory of the base model, where the race's motions normally live.
    pub fn race_dir(&self) -> &str {
        paths::parent(&self.base_model)
    }
}

fn skins(s: &[fmt::Skin]) -> Vec<RaceSkin> {
    s.iter().map(|k| RaceSkin { part: k.part, src: path(&k.src_file_name), dst: path(&k.dst_file_name) }).collect()
}

fn attachment(a: &fmt::AttachingData) -> RaceAttachment {
    let kind = match &a.payload {
        fmt::AttachingPayload::None => RaceAttachmentKind::None,
        fmt::AttachingPayload::Collision(c) => RaceAttachmentKind::Collision {
            collision_type: c.collision_type,
            spheres: c.spheres.iter().map(|s| (s.radius, coords::vec3(s.position))).collect(),
        },
        fmt::AttachingPayload::Effect(e) => RaceAttachmentKind::Effect {
            file: path(&e.file_name),
            position: coords::vec3(e.position),
            rotation: e.rotation,
        },
        fmt::AttachingPayload::Object(o) => RaceAttachmentKind::Object { file: path(&o.file_name) },
    };
    RaceAttachment {
        is_attaching: a.is_attaching != 0,
        model_index: a.attaching_model_index,
        bone: name(&a.attaching_bone_name),
        kind,
    }
}

/// Converts parsed race data.
pub fn race_from_data(d: &fmt::RaceData) -> RaceAsset {
    RaceAsset {
        base_model: path(&d.base_model_file_name),
        tree: path(&d.tree_file_name),
        attribute: path(&d.attribute_file_name),
        smoke_bone: name(&d.smoke_bone_name),
        motion_list: path(&d.motion_list_file_name),
        shapes: d
            .shapes
            .iter()
            .map(|(&i, s)| (i, RaceShape { model: path(&s.model_file_name), skins: skins(&s.skins) }))
            .collect(),
        hairs: d
            .hairs
            .iter()
            .map(|(&i, h)| (i, RaceShape { model: path(&h.model_file_name), skins: skins(&h.skins) }))
            .collect(),
        attachments: d.attaching_data.iter().map(attachment).collect(),
    }
}

/// Loads `.msm` files into [`RaceAsset`].
#[derive(Default, TypePath)]
pub struct RaceLoader;

impl AssetLoader for RaceLoader {
    type Asset = RaceAsset;
    type Settings = ();
    type Error = Error;

    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<RaceAsset, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let script = script::parse(&bytes);
        let mut data = fmt::RaceData::default();
        // Smoke effects are Phase 3: accept every file name.
        if !data.load_script(&script, |_| true) {
            return Err(Error::Decode(format!("{}: invalid race data script", load_context.path())));
        }
        Ok(race_from_data(&data))
    }

    fn extensions(&self) -> &[&str] {
        &["msm"]
    }
}

/// The type-specific part of a motion event.
#[derive(Debug, Clone, PartialEq)]
pub enum RaceMotionEventKind {
    Effect {
        file: String,
        /// Attached to `bone` (when `is_attaching`) at `position`.
        bone: Option<String>,
        position: Vec3,
        following: bool,
        independent: bool,
    },
    EffectToTarget { file: String, position: Vec3, following: bool },
    Sound { file: String },
    ScreenWaving { power: i32, range: i32 },
    /// Special attack (`MOTION_EVENT_TYPE_SPECIAL_ATTACKING`): a splash area
    /// that hits for the event's duration. Kept in Metin2 space, as the
    /// battle code works there.
    SpecialAttacking(MotionSplash),
    Fly { file: String, bone: Option<String>, position: Vec3 },
    CharacterShow,
    CharacterHide,
    Warp,
}

/// One entry of a motion's event list.
#[derive(Debug, Clone, PartialEq)]
pub struct RaceMotionEvent {
    /// Start in 60 FPS frames.
    pub frame: u32,
    /// Start in seconds.
    pub start: f32,
    pub duration: f32,
    pub kind: RaceMotionEventKind,
}

/// `NRaceData::SAttackData`, the part of an attack the victim feels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MotionAttack {
    /// `iAttackType` (0 splash, 1 snipe).
    pub attack_type: i32,
    /// `iHittingType` (`HIT_TYPE_NONE`/`GREAT`/`GOOD`).
    pub hitting_type: i32,
    pub stiffen_time: f32,
    pub invisible_time: f32,
    pub external_force: f32,
    /// `iHitLimitCount`; 0 means the default limit.
    pub hit_limit_count: i32,
}

impl From<&fmt::AttackData> for MotionAttack {
    fn from(a: &fmt::AttackData) -> Self {
        MotionAttack {
            attack_type: a.attack_type,
            hitting_type: a.hitting_type,
            stiffen_time: a.stiffen_time,
            invisible_time: a.invisible_time,
            external_force: a.external_force,
            hit_limit_count: a.hit_limit_count,
        }
    }
}

/// One `NRaceData::SHitData`: a weapon sweep between two times, sampled as
/// `HitPosition` segments (actor-local, Metin2 space, before rotation).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionHit {
    pub start: f32,
    pub end: f32,
    /// `mapHitPosition`: ascending times, each with the sweep's previous and
    /// current point.
    pub positions: Vec<(f32, [f32; 3], [f32; 3])>,
}

/// `NRaceData::SMotionAttackData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionAttackData {
    pub attack: MotionAttack,
    /// `iMotionType` (`MOTION_TYPE_NORMAL`/`COMBO`/`SKILL`).
    pub motion_type: i32,
    pub hits: Vec<MotionHit>,
}

/// A `SpecialAttacking` event: spheres around the actor (Metin2 space,
/// actor-local) and the attack they deal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionSplash {
    pub attack: MotionAttack,
    /// `(radius, position)`.
    pub spheres: Vec<(f32, [f32; 3])>,
    /// `isEnableHitProcess`: whether a hit sends `CG_ATTACK`.
    pub enable_hit_process: bool,
}

/// `CRaceMotionData::SComboInputData`, in motion seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ComboInput {
    /// `PreInputTime`: input from here on is remembered.
    pub input_start: f32,
    /// `DirectInputTime`: the next combo step may start.
    pub next_combo: f32,
    /// `InputLimitTime`: input after this is ignored.
    pub input_end: f32,
}

/// A `.msa` motion script.
#[derive(Asset, TypePath, Debug, Clone, Default)]
pub struct RaceMotionAsset {
    /// The `.gr2` animation (canonical asset path).
    pub motion_file: String,
    pub duration: f32,
    pub is_loop: bool,
    /// `-1` loops forever.
    pub loop_count: i32,
    pub loop_start: f32,
    pub loop_end: f32,
    /// Root motion accumulated over the motion (`Accumulation`), Bevy space.
    pub accumulation: Option<Vec3>,
    pub is_combo: bool,
    pub is_attacking: bool,
    pub events: Vec<RaceMotionEvent>,
    /// `ComboInputData`, when the motion is a combo step.
    pub combo_input: Option<ComboInput>,
    /// `AttackingData`, when the motion attacks.
    pub attack: Option<MotionAttackData>,
    /// `LoopCancelEnable` (`m_bCancelEnableSkill`).
    pub cancel_enable_skill: bool,
    /// The sound script (`sound/<motion>.mss`): `(60 FPS frame, file)`.
    pub sounds: Vec<(u32, String)>,
}

fn opt(bytes: &[u8], attaching: bool) -> Option<String> {
    (attaching && !bytes.is_empty()).then(|| name(bytes))
}

fn event(e: &fmt::MotionEvent) -> Option<RaceMotionEvent> {
    use fmt::MotionEventData as D;
    let kind = match &e.data {
        D::Effect(x) => RaceMotionEventKind::Effect {
            file: path(&x.effect_file_name),
            bone: opt(&x.attaching_bone_name, x.is_attaching != 0),
            position: coords::vec3(x.effect_position),
            following: x.is_following != 0,
            independent: x.is_independent != 0,
        },
        D::EffectToTarget(x) => RaceMotionEventKind::EffectToTarget {
            file: path(&x.effect_file_name),
            position: coords::vec3(x.effect_position),
            following: x.is_following != 0,
        },
        D::Sound(x) => RaceMotionEventKind::Sound { file: path(&x.sound_file_name) },
        D::ScreenWaving(x) => RaceMotionEventKind::ScreenWaving { power: x.power, range: x.affecting_range },
        D::SpecialAttacking(x) => RaceMotionEventKind::SpecialAttacking(MotionSplash {
            attack: MotionAttack::from(&x.attack),
            spheres: x.collision.spheres.iter().map(|s| (s.radius, s.position)).collect(),
            enable_hit_process: x.is_enable_hit_process != 0,
        }),
        D::Fly(x) => RaceMotionEventKind::Fly {
            file: path(&x.fly_file_name),
            bone: opt(&x.attaching_bone_name, x.is_attaching != 0),
            position: coords::vec3(x.fly_position),
        },
        D::CharacterShow => RaceMotionEventKind::CharacterShow,
        D::CharacterHide => RaceMotionEventKind::CharacterHide,
        D::Warp => RaceMotionEventKind::Warp,
    };
    Some(RaceMotionEvent { frame: e.frame, start: e.starting_time, duration: e.duration_time, kind })
}

/// Converts a parsed motion script.
pub fn motion_from_data(m: &fmt::RaceMotionData) -> RaceMotionAsset {
    RaceMotionAsset {
        motion_file: path(&m.motion_file_name),
        duration: m.motion_duration,
        is_loop: m.is_loop_motion,
        loop_count: m.loop_count,
        loop_start: m.loop_start_time,
        loop_end: m.loop_end_time,
        accumulation: m.is_accumulation_motion.then(|| coords::vec3(m.accumulation_position)),
        is_combo: m.is_combo_motion,
        is_attacking: m.is_attacking_motion,
        events: m.events.iter().filter_map(event).collect(),
        combo_input: m.is_combo_motion.then_some(ComboInput {
            input_start: m.combo_input.input_start_time,
            next_combo: m.combo_input.next_combo_time,
            input_end: m.combo_input.input_end_time,
        }),
        attack: m.is_attacking_motion.then(|| MotionAttackData {
            attack: MotionAttack::from(&m.motion_attack_data.attack),
            motion_type: m.motion_attack_data.motion_type,
            hits: m
                .motion_attack_data
                .hit_data
                .iter()
                .map(|hit| MotionHit {
                    start: hit.attack_start_time,
                    end: hit.attack_end_time,
                    positions: hit.hit_positions.iter().map(|(t, p)| (*t, p.last_position, p.position)).collect(),
                })
                .collect(),
        }),
        cancel_enable_skill: m.cancel_enable_skill != 0,
        sounds: m.sound_instances.iter().map(|s| (s.frame, path(&s.file_name))).collect(),
    }
}

/// Loads `.msa` files into [`RaceMotionAsset`].
#[derive(Default, TypePath)]
pub struct RaceMotionLoader;

impl AssetLoader for RaceMotionLoader {
    type Asset = RaceMotionAsset;
    type Settings = ();
    type Error = Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<RaceMotionAsset, Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let script = script::parse(&bytes);
        let mut data = fmt::RaceMotionData::from_script(&script)
            .ok_or_else(|| Error::Decode(format!("{}: invalid motion script", load_context.path())))?;
        // `LoadMotionData` -> `LoadSoundScriptData`: `sound/<motion>.mss`,
        // named from the `d:/ymir work/...` spelling; a missing script is fine.
        let motion = format!("d:/{}", paths::canonical_asset_path(&load_context.path().path().to_string_lossy()));
        if let Some(sound) = fmt::sound_script_file_name(motion.as_bytes())
            && let Some(sound) = paths::asset_path(&sound)
        {
            let sound_path = load_context.path().resolve(&bevy::asset::AssetPath::parse(&format!("/{sound}")));
            if let Ok(bytes) = load_context.read_asset_bytes(sound_path).await
                && let Some(sounds) = fmt::sound::parse_sound_information(&script::parse(&bytes), None)
            {
                fmt::sound::data_to_instance(&sounds, &mut data.sound_instances);
            }
        }
        Ok(motion_from_data(&data))
    }

    fn extensions(&self) -> &[&str] {
        &["msa"]
    }
}

/// The `.msa` files under `race_dir` (canonical asset path of a directory),
/// including sub-folders such as `horse/`, sorted. For the viewer: it shows a
/// race's motions without the Python registration tables.
pub fn list_motion_files(index: &DirectoryIndex, race_dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![paths::canonical_asset_path(race_dir).trim_matches('/').to_owned()];
    while let Some(dir) = stack.pop() {
        let Some(entries) = index.list(&dir) else { continue };
        for (entry, is_dir) in entries {
            let full = paths::join(&dir, entry);
            if is_dir {
                stack.push(full);
            } else if entry.ends_with(".msa") {
                out.push(full);
            }
        }
    }
    out.sort();
    out
}
