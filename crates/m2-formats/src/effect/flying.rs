//! `CFlyingData` (`client/src/game/FlyingData.cpp`): an `.msf` flying object
//! (arrows, spells), with the effects attached to it.

use super::{Cursor, case_crc32, resolve, string_path};
use crate::pack::PackManager;
use crate::script::{self, Script};

/// `CFlyingData::TFlyingAttachData`: one `Group AttachData`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlyingAttachData {
    /// `FLY_ATTACH_NONE`, `_EFFECT`, `_OBJECT` (0–2).
    pub kind: i32,
    /// `FLY_ATTACH_TYPE_NONE`, `_LINE`, `_MULTI_LINE`, `_SINE`, `_EXP` (0–4).
    pub fly_type: i32,
    /// The attached effect, resolved against the script's directory.
    pub file_name: Vec<u8>,
    pub has_tail: bool,
    pub tail_color: u32,
    pub tail_length: f32,
    pub tail_size: f32,
    pub rect_shape: bool,
    pub roll: f32,
    /// For `MULTI_LINE`.
    pub distance: f32,
    /// For `SINE` and `EXP`.
    pub period: f32,
    pub amplitude: f32,
}

/// `CFlyingData` after `LoadScriptFile`.
#[derive(Debug, Clone, PartialEq)]
pub struct FlyingData {
    /// `m_strFilename`: the requested name through `StringPath`.
    pub file_name: Vec<u8>,
    pub spreading: bool,
    pub maintain_parallel: bool,
    pub initial_velocity: f32,
    pub cone_angle: f32,
    pub roll_angle: f32,
    pub angular_velocity: [f32; 3],
    pub gravity: f32,
    pub bomb_range: f32,
    /// Resolved against the script's directory (not lowercased).
    pub bomb_effect_name: Vec<u8>,
    /// What `CEffectManager::RegisterEffect2` returns for it:
    /// [`case_crc32`] of the `StringPath`ed name; 0 without a bomb effect.
    pub bomb_effect_id: u32,
    pub homing: bool,
    pub homing_max_angle: f32,
    pub homing_start_time: f32,
    pub hit_on_background: bool,
    pub hit_on_another_monster: bool,
    pub pierce_count: i32,
    /// Never loaded (the line is commented out in C++).
    pub collision_sphere_radius: f32,
    pub range: f32,
    pub acceleration: [f32; 3],
    pub attach: Vec<FlyingAttachData>,
}

impl Default for FlyingData {
    /// `CFlyingData::__Initialize`.
    fn default() -> Self {
        Self {
            file_name: Vec::new(),
            spreading: false,
            maintain_parallel: false,
            initial_velocity: 200.0,
            cone_angle: 0.0,
            roll_angle: 0.0,
            angular_velocity: [0.0; 3],
            gravity: 0.0,
            bomb_range: 10.0,
            bomb_effect_name: Vec::new(),
            bomb_effect_id: 0,
            homing: false,
            homing_max_angle: 3.0,
            homing_start_time: 0.0,
            hit_on_background: false,
            hit_on_another_monster: false,
            pierce_count: 0,
            collision_sphere_radius: 0.0,
            range: 500.0,
            acceleration: [0.0; 3],
            attach: Vec::new(),
        }
    }
}

/// `CFlyingData::LoadScriptFile` on a parsed script (`name` as requested).
/// Returns the data and what C++ returns: false when `InitialVelocity` or
/// `Range` is missing, with what was read up to there.
///
/// The effects it names are not loaded here (C++ registers them with the
/// effect manager as a side effect).
pub fn parse_flying_data(name: &[u8], script: &Script) -> (FlyingData, bool) {
    let mut d = FlyingData { file_name: string_path(name), ..FlyingData::default() };
    if !script.complete {
        return (d, false);
    }
    let ok = load(&mut d, script);
    (d, ok)
}

fn load(d: &mut FlyingData, script: &Script) -> bool {
    let mut cursor = Cursor::new(&script.root);
    let node = cursor.node();
    // The loader's file name, which relative names resolve against.
    let script_name = d.file_name.clone();
    // `temp` is shared by every integer read, and keeps its value when a
    // read fails.
    let mut temp = 0i32;
    let read = |key: &str, temp: &mut i32| match node.token_int(key) {
        Some(v) => {
            *temp = v;
            true
        }
        None => false,
    };

    d.spreading = read("spreadingflag", &mut temp) && temp != 0;
    d.maintain_parallel = read("maintainparallelflag", &mut temp) && temp != 0;
    let Some(v) = node.token_float("initialvelocity") else {
        return false;
    };
    d.initial_velocity = v;
    if let Some(v) = node.token_float("coneangle") {
        d.cone_angle = v;
    }
    if let Some(v) = node.token_float("rollangle") {
        d.roll_angle = v;
    }
    if let Some(v) = node.token_vector3("angularvelocity") {
        d.angular_velocity = v;
    }
    if let Some(v) = node.token_float("gravity") {
        d.gravity = v;
    }
    d.hit_on_background = read("hitonbackground", &mut temp) && temp != 0;
    d.hit_on_another_monster = read("hitonanothermonster", &mut temp) && temp != 0;
    d.pierce_count = node.token_int("piercecount").unwrap_or(0);
    if let Some(v) = node.token_float("bombrange") {
        d.bomb_range = v;
    }
    match node.token_string("bombeffect") {
        None => d.bomb_effect_name.clear(),
        Some(b"") => d.bomb_effect_name.clear(),
        Some(bomb) => {
            d.bomb_effect_name = resolve(&script_name, bomb);
            d.bomb_effect_id = case_crc32(&string_path(&d.bomb_effect_name));
        }
    }
    d.homing = read("homingflag", &mut temp) && temp != 0;
    d.homing_start_time = node.token_float("homingstarttime").unwrap_or(0.0);
    d.homing_max_angle = node.token_float("homingmaxangle").unwrap_or(0.0);
    let Some(v) = node.token_float("range") else {
        return false;
    };
    d.range = v;
    d.acceleration = node.token_vector3("acceleration").unwrap_or([0.0; 3]);

    let mut i = 0;
    while i < cursor.child_count() {
        cursor.goto_child_index(i, |c| {
            if c.name() != Some(&b"attachdata"[..]) {
                return; // TraceError "Wrong Data"
            }
            let node = c.node();
            let mut a = FlyingAttachData::default();
            let Some(kind) = node.token_int("type") else {
                return;
            };
            a.kind = kind;
            if let Some(v) = node.token_int("flytype") {
                a.fly_type = v;
            }
            if let Some(file) = node.token_string("attachfile") {
                a.file_name = if file.is_empty() { Vec::new() } else { resolve(&script_name, file) };
            }
            if let Some(v) = node.token_int("tailflag") {
                temp = v;
            }
            if temp != 0 {
                a.has_tail = true;
                if let Some(v) = node.token_dword("tailcolor") {
                    a.tail_color = v;
                }
                if let Some(v) = node.token_float("taillength") {
                    a.tail_length = v;
                }
                if let Some(v) = node.token_float("tailsize") {
                    a.tail_size = v;
                }
                if let Some(v) = node.token_int("tailshaperect") {
                    temp = v;
                }
                a.rect_shape = temp != 0;
            }
            for (key, field) in [
                ("roll", &mut a.roll),
                ("distance", &mut a.distance),
                ("period", &mut a.period),
                ("amplitude", &mut a.amplitude),
            ] {
                if let Some(v) = node.token_float(key) {
                    *field = v;
                }
            }
            d.attach.push(a);
        });
        i += 1;
    }
    true
}

/// `CFlyingData::LoadScriptFile`: reads `name` through the pack manager.
/// A missing file returns false with the initial data.
pub fn load_flying_data(manager: &PackManager, name: impl AsRef<[u8]>) -> (FlyingData, bool) {
    let name = name.as_ref();
    let normalized = string_path(name);
    match script::load(manager, &normalized) {
        Ok(script) => parse_flying_data(name, &script),
        Err(_) => (FlyingData { file_name: normalized, ..FlyingData::default() }, false),
    }
}
