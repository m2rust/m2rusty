//! Object placements of a terrain cell (`CArea::Load`,
//! `client/src/game/Area.cpp`): `AreaData.txt` (buildings, trees, effects,
//! dungeon blocks) and `AreaAmbienceData.txt` (sounds).
//!
//! ```text
//! AreaDataFile
//! ObjectCount 1
//! Start Object000
//!     20801.746094 -13522.107422 17875.500000   (position)
//!     569394331                                 (property CRC)
//!     0.000000#0.000000#180.000000              (yaw#pitch#roll, degrees)
//!     -40.000000                                (height bias)
//!                                               (portal ids…)
//! End Object
//! ```
//!
//! Ambience objects are `x y z crc range [max volume area %]`.
//!
//! Each object names a property by CRC; C++ drops the ones the property
//! manager does not know, so the loaders take a `is_registered` predicate
//! (the property set is another module's).

use crate::pack::{self, PackManager};
use crate::script::{TokenVectorMap, atof_f32, atoi, parse_multiple_text_data};

use super::token;

/// `PORTAL_ID_MAX_NUM` (`neoeter/include/Neoeter/Render/GrpObjectInstance.h`).
pub const PORTAL_ID_MAX_NUM: usize = 8;

/// `CArea::TObjectData`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ObjectData {
    pub position: [f32; 3],
    /// CRC of the property file (`CProperty::GetCRC`).
    pub crc: u32,
    pub portal_ids: [u8; PORTAL_ID_MAX_NUM],
    /// Degrees, read as integers (`atoi`) into floats.
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Added to `position.z` when the instance is placed.
    pub height_bias: f32,
    /// Ambience only: sound range.
    pub range: u32,
    /// Ambience only.
    pub max_volume_area_percentage: f32,
}

/// Why `CArea::__Load_LoadObject`/`__Load_LoadAmbience` returned false
/// (nothing is added then).
#[derive(Debug)]
pub enum AreaFileError {
    Pack(pack::Error),
    /// `AreaDataFile`/`AreaAmbienceDataFile` or `ObjectCount` is missing.
    MissingKey(&'static str),
}

/// The `object%03d` entries of `map` for `i < count`, in `i` order.
/// C++ loops `i` over `0..count` (a `DWORD`); this visits only the keys
/// that exist, which is the same thing without four billion lookups.
fn objects(map: &TokenVectorMap, count: u32) -> Vec<&[Vec<u8>]> {
    let mut found: Vec<(u32, &[Vec<u8>])> = map
        .iter()
        .filter_map(|(key, tokens)| {
            let digits = key.strip_prefix(b"object")?;
            let text = std::str::from_utf8(digits).ok()?;
            // %03d of the DWORD index passed as int.
            let i = text.parse::<i32>().ok()? as u32;
            (i < count && format!("{:03}", i as i32) == text).then_some((i, tokens.as_slice()))
        })
        .collect();
    found.sort_by_key(|&(i, _)| i);
    found.into_iter().map(|(_, tokens)| tokens).collect()
}

/// The key checks both loaders start with; returns `ObjectCount`.
fn object_count(map: &TokenVectorMap, file_key: &'static str) -> Result<u32, AreaFileError> {
    if !map.contains_key(file_key.as_bytes()) {
        return Err(AreaFileError::MissingKey(file_key));
    }
    let Some(count) = map.get(b"objectcount".as_slice()) else {
        return Err(AreaFileError::MissingKey("objectcount"));
    };
    Ok(atoi(token(count, 0)) as u32)
}

/// The rotation token: `yaw#pitch#roll`, or just a roll.
///
/// C++ takes `substr(0, s - 1)` for the yaw, dropping the byte before the
/// first `#` (harmless for `0.000000#…`, which `atoi` stops reading at the
/// dot anyway); a leading `#` makes that the whole token, and a missing
/// second `#` makes the pitch the rest of the token and the roll the whole
/// token. Mirrored.
fn rotation(t: &[u8]) -> (f32, f32, f32) {
    let Some(s) = t.iter().position(|&b| b == b'#') else {
        return (0.0, 0.0, atoi(t) as f32);
    };
    let yaw = if s == 0 { atoi(t) } else { atoi(&t[..s - 1]) };
    let p = s + 1;
    let s2 = t[p..].iter().position(|&b| b == b'#').map(|o| p + o);
    let (pitch, roll) = match s2 {
        Some(s2) => (atoi(&t[p..s2]), atoi(&t[s2 + 1..])),
        None => (atoi(&t[p..]), atoi(t)),
    };
    (yaw as f32, pitch as f32, roll as f32)
}

/// `CArea::__Load_LoadObject` after reading the file: the objects whose
/// CRC `is_registered`, in index order.
///
/// Deviation: a block with fewer than four values reads the missing ones as
/// empty tokens (undefined behaviour in C++).
pub fn parse_area_data(data: &[u8], is_registered: impl Fn(u32) -> bool) -> Result<Vec<ObjectData>, AreaFileError> {
    let map = parse_multiple_text_data(data);
    let count = object_count(&map, "areadatafile")?;
    let mut out = Vec::new();
    for v in objects(&map, count) {
        let mut object = ObjectData {
            position: [atof_f32(token(v, 0)), atof_f32(token(v, 1)), atof_f32(token(v, 2))],
            crc: atoi(token(v, 3)) as u32,
            ..ObjectData::default()
        };
        if v.len() > 4 {
            (object.yaw, object.pitch, object.roll) = rotation(&v[4]);
        }
        if v.len() > 5 {
            object.height_bias = atof_f32(&v[5]);
        }
        for (i, id) in v.iter().skip(6).take(PORTAL_ID_MAX_NUM).enumerate() {
            object.portal_ids[i] = atoi(id) as u8;
        }
        if is_registered(object.crc) {
            out.push(object);
        }
    }
    Ok(out)
}

/// `CArea::__Load_LoadAmbience` after reading the file.
///
/// Deviation: as [`parse_area_data`], for blocks with fewer than five values.
pub fn parse_area_ambience_data(
    data: &[u8],
    is_registered: impl Fn(u32) -> bool,
) -> Result<Vec<ObjectData>, AreaFileError> {
    let map = parse_multiple_text_data(data);
    let count = object_count(&map, "areaambiencedatafile")?;
    let mut out = Vec::new();
    for v in objects(&map, count) {
        let object = ObjectData {
            position: [atof_f32(token(v, 0)), atof_f32(token(v, 1)), atof_f32(token(v, 2))],
            crc: atoi(token(v, 3)) as u32,
            range: atoi(token(v, 4)) as u32,
            max_volume_area_percentage: if v.len() >= 6 { atof_f32(&v[5]) } else { 0.0 },
            ..ObjectData::default()
        };
        if is_registered(object.crc) {
            out.push(object);
        }
    }
    Ok(out)
}

type ObjectsResult = Result<Vec<ObjectData>, AreaFileError>;

/// A loaded `CArea`: the object data of both files.
#[derive(Debug, Default)]
pub struct Area {
    /// `m_ObjectDataVector`: `AreaData.txt` objects, then ambience objects,
    /// then sorted by CRC.
    ///
    /// Deviation: C++ uses `std::ranges::sort`, which does not keep the
    /// order of objects with the same CRC; this sort is stable, so equal
    /// CRCs stay in file order.
    pub objects: Vec<ObjectData>,
    /// Errors of the two files (C++ only traces them).
    pub object_error: Option<AreaFileError>,
    pub ambience_error: Option<AreaFileError>,
}

impl Area {
    /// `CArea::Load` minus the instance building: reads `AreaData.txt` and
    /// `AreaAmbienceData.txt` from `dir` (an [`super::area_dir_name`]).
    pub fn load(manager: &PackManager, dir: &[u8], is_registered: impl Fn(u32) -> bool) -> Self {
        let read = |file: &str, parse: &dyn Fn(&[u8]) -> ObjectsResult| {
            let mut name = dir.to_vec();
            name.extend_from_slice(file.as_bytes());
            manager.get(&name).map_err(AreaFileError::Pack).and_then(|data| parse(&data))
        };
        let mut area = Self::default();
        match read("AreaData.txt", &|d| parse_area_data(d, &is_registered)) {
            Ok(objects) => area.objects.extend(objects),
            Err(e) => area.object_error = Some(e),
        }
        match read("AreaAmbienceData.txt", &|d| parse_area_ambience_data(d, &is_registered)) {
            Ok(objects) => area.objects.extend(objects),
            Err(e) => area.ambience_error = Some(e),
        }
        area.objects.sort_by_key(|o| o.crc);
        area
    }
}
