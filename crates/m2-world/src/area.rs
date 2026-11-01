//! Area objects: what `CArea::Load` + `__Load_BuildObjectInstances`
//! (`Area.cpp`) decide for every object of a terrain cell, minus creating
//! the graphic instances.
//!
//! The object list (`AreaData.txt`, `AreaAmbienceData.txt`) names a property
//! by CRC. The property's `PropertyType` selects tree, building, effect,
//! ambience or dungeon block, and its fields give the model/tree/effect
//! files. [`place_objects`] resolves that and returns plain descriptions;
//! loading the models, trees and effects (which can still fail in C++ and
//! then leave an empty instance) is the renderer's job.
//!
//! Names are raw bytes (CP949 pack keys, D5), not UTF-8 strings.

use m2_formats::effect::case_crc32;
use m2_formats::map::{ObjectData, PORTAL_ID_MAX_NUM, parse_area_ambience_data, parse_area_data};
use m2_formats::pack::PackManager;
use m2_formats::property::{Property, PropertyManager};
use m2_formats::race::no_extension;
use m2_formats::script::{atof_f32, atoi};

use crate::sort::sort_by_less;

/// `CArea::TAmbienceInstance::Update` choice, from `PlayType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbienceUpdate {
    Once,
    Step,
    Loop,
    /// Unknown play type: no update function.
    None,
}

/// What an object turns into.
#[derive(Debug, Clone, PartialEq)]
pub enum PlacedKind {
    /// `PROPERTY_TYPE_TREE`: a SpeedTree (`TreeFile`).
    Tree { file: Vec<u8> },
    /// `PROPERTY_TYPE_BUILDING`: a model with collision data.
    Building {
        file: Vec<u8>,
        /// `<file without extension>.mdatr`.
        attribute_file: Vec<u8>,
        attribute_exists: bool,
        shadow_flag: i32,
        /// `<file>_lod_01.gr2`… while they exist (at most 3).
        lod_files: Vec<Vec<u8>>,
    },
    /// `PROPERTY_TYPE_EFFECT`.
    Effect {
        file: Vec<u8>,
        /// `GetCaseCRC32` of the file name, the effect manager key.
        effect_id: u32,
        /// Global matrix: yaw/pitch/roll rotation, translation in `_41.._43`
        /// (row-major).
        matrix: [f32; 16],
    },
    /// `PROPERTY_TYPE_AMBIENCE`: a positional sound.
    Ambience {
        name: Vec<u8>,
        play_type: Vec<u8>,
        update: AmbienceUpdate,
        play_interval: f32,
        play_interval_variation: f32,
        /// The property's own `MaxVolumeAreaPercentage` (the instance uses
        /// the object's, in [`PlacedObject::max_volume_area_percentage`]).
        property_max_volume_area_percentage: f32,
        sounds: Vec<Vec<u8>>,
    },
    /// `PROPERTY_TYPE_DUNGEON_BLOCK`.
    DungeonBlock { file: Vec<u8>, attribute_file: Vec<u8>, attribute_exists: bool },
}

/// One placed object.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedObject {
    /// Property CRC of the object.
    pub crc: u32,
    /// `Position`, with `z` already raised by the height bias
    /// (`Position.z + m_fHeightBias`).
    pub position: [f32; 3],
    /// Yaw, pitch, roll in degrees.
    pub rotation: [f32; 3],
    pub portal_ids: [u8; PORTAL_ID_MAX_NUM],
    /// Ambience only.
    pub range: u32,
    /// Ambience only (the object's value).
    pub max_volume_area_percentage: f32,
    pub kind: PlacedKind,
}

/// `CArea::Load` object part: both files, concatenated, and sorted by CRC
/// with libstdc++'s unstable `std::ranges::sort`. Objects whose property the
/// manager does not know are dropped while loading.
pub fn load_objects(manager: &PackManager, dir: &[u8], props: &PropertyManager) -> Vec<ObjectData> {
    let known = |crc: u32| props.get(crc).is_some();
    let read = |file: &str| {
        let mut name = dir.to_vec();
        name.extend_from_slice(file.as_bytes());
        manager.get(&name).ok()
    };
    let mut objects = Vec::new();
    if let Some(Ok(o)) = read("AreaData.txt").map(|d| parse_area_data(&d, known)) {
        objects.extend(o);
    }
    if let Some(Ok(o)) = read("AreaAmbienceData.txt").map(|d| parse_area_ambience_data(&d, known)) {
        objects.extend(o);
    }
    sort_objects(&mut objects);
    objects
}

/// `std::ranges::sort(m_ObjectDataVector, ObjectDataComp())`.
pub fn sort_objects(objects: &mut [ObjectData]) {
    sort_by_less(objects, |a, b| a.crc < b.crc);
}

type Mat = [f32; 16];

fn identity() -> Mat {
    let mut m = [0.0; 16];
    for i in 0..4 {
        m[i * 5] = 1.0;
    }
    m
}

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut out = [0.0; 16];
    for i in 0..4 {
        for j in 0..4 {
            out[i * 4 + j] = (a[i * 4] * b[j]) + (a[i * 4 + 1] * b[4 + j]) + (a[i * 4 + 2] * b[8 + j]) + (a[i * 4 + 3] * b[12 + j]);
        }
    }
    out
}

/// `D3DXMatrixRotationYawPitchRoll` of the compatibility layer
/// (`roll * pitch * yaw`, row-vector convention), angles in radians.
pub fn rotation_yaw_pitch_roll(yaw: f32, pitch: f32, roll: f32) -> [f32; 16] {
    let (s, c) = (roll.sin(), roll.cos());
    let mut z = identity();
    (z[0], z[1], z[4], z[5]) = (c, s, -s, c);
    let (s, c) = (pitch.sin(), pitch.cos());
    let mut x = identity();
    (x[5], x[6], x[9], x[10]) = (c, s, -s, c);
    let (s, c) = (yaw.sin(), yaw.cos());
    let mut y = identity();
    (y[0], y[2], y[8], y[10]) = (c, -s, s, c);
    mul(&mul(&z, &x), &y)
}

/// `D3DXToRadian`.
fn to_radian(degree: f32) -> f32 {
    degree * (std::f32::consts::PI / 180.0f32)
}

fn building_like(p: &Property, want: &[u8]) -> Option<()> {
    (p.get_string("PropertyType")? == want).then_some(())?;
    p.get_string("PropertyName")?;
    Some(())
}

/// `CArea::__SetObjectInstance`: resolves one object. `exists` tells whether
/// a file is in the packs (`CResourceManager::IsFileExist`). `None` when the
/// property is unknown, has an unknown type, or does not parse.
pub fn place_object(o: &ObjectData, props: &PropertyManager, exists: &dyn Fn(&[u8]) -> bool) -> Option<PlacedObject> {
    let p = props.get(o.crc)?;
    let position = [o.position[0], o.position[1], o.position[2] + o.height_bias];
    let kind = match p.get_string("PropertyType")? {
        b"Tree" => PlacedKind::Tree { file: p.get_string("TreeFile")?.to_vec() },
        b"Building" => {
            building_like(p, b"Building")?;
            let file = p.get_string("BuildingFile")?.to_vec();
            let attribute_file = [no_extension(&file), b".mdatr"].concat();
            let shadow_flag = p.get_string("ShadowFlag").map_or(0, atoi);
            let mut lod_files = Vec::new();
            for lod in 1..=3 {
                let name = [no_extension(&file), format!("_lod_{lod:02}.gr2").as_bytes()].concat();
                if !exists(&name) {
                    break;
                }
                lod_files.push(name);
            }
            PlacedKind::Building { attribute_exists: exists(&attribute_file), file, attribute_file, shadow_flag, lod_files }
        }
        b"Effect" => {
            building_like(p, b"Effect")?;
            let file = p.get_string("EffectFile")?.to_vec();
            let mut matrix = rotation_yaw_pitch_roll(to_radian(o.yaw), to_radian(o.pitch), to_radian(o.roll));
            (matrix[12], matrix[13], matrix[14]) = (position[0], position[1], position[2]);
            PlacedKind::Effect { effect_id: case_crc32(&file), file, matrix }
        }
        b"Ambience" => {
            building_like(p, b"Ambience")?;
            let play_type = p.get_string("PlayType")?.to_vec();
            let interval = atof_f32(p.get_string("PlayInterval")?);
            let variation = atof_f32(p.get_string("PlayIntervalVariation")?);
            let max_volume = p.get_string("MaxVolumeAreaPercentage").map_or(0.0, atof_f32);
            let sounds = p.get_vector("AmbienceSoundVector")?.to_vec();
            let update = match &play_type[..] {
                b"ONCE" => AmbienceUpdate::Once,
                b"STEP" => AmbienceUpdate::Step,
                b"LOOP" => AmbienceUpdate::Loop,
                _ => AmbienceUpdate::None,
            };
            PlacedKind::Ambience {
                name: p.get_string("PropertyName")?.to_vec(),
                play_type,
                update,
                play_interval: interval,
                play_interval_variation: variation,
                property_max_volume_area_percentage: max_volume,
                sounds,
            }
        }
        b"DungeonBlock" => {
            building_like(p, b"DungeonBlock")?;
            let file = p.get_string("dungeonblockfile")?.to_vec();
            let attribute_file = [no_extension(&file), b".mdatr"].concat();
            PlacedKind::DungeonBlock { attribute_exists: exists(&attribute_file), file, attribute_file }
        }
        _ => return None,
    };
    Some(PlacedObject {
        crc: o.crc,
        position,
        rotation: [o.yaw, o.pitch, o.roll],
        portal_ids: o.portal_ids,
        range: o.range,
        max_volume_area_percentage: o.max_volume_area_percentage,
        kind,
    })
}

/// [`place_object`] for a whole (sorted) list, skipping the ones that do not
/// resolve.
pub fn place_objects(objects: &[ObjectData], props: &PropertyManager, exists: &dyn Fn(&[u8]) -> bool) -> Vec<PlacedObject> {
    objects.iter().filter_map(|o| place_object(o, props, exists)).collect()
}
