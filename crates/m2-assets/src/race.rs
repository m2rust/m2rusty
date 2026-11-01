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
    /// Special attack: damage window starting at the event.
    SpecialAttacking,
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
        D::SpecialAttacking(_) => RaceMotionEventKind::SpecialAttacking,
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
        let data = fmt::RaceMotionData::from_script(&script)
            .ok_or_else(|| Error::Decode(format!("{}: invalid motion script", load_context.path())))?;
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
