//! `regen.txt`: monster spawn areas (`CMapOutdoor::LoadMonsterAreaInfo`,
//! `client/src/game/MapOutdoorLoad.cpp`, into `CMonsterAreaInfo`).
//!
//! ```text
//! //type cx  cy  sx sy z dir time percent count vnum
//! m      334 539 18 18 0 0   0    100     5     112
//! ```
//!
//! `m` lines spawn a monster vnum, `g` lines a group; other lines are
//! ignored.

use crate::script::atoi;
use crate::text::{self, DEFAULT_DELIMITERS};

use super::atol;

/// `CMonsterAreaInfo::EMonsterAreaInfoType` of a loaded area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterAreaKind {
    /// `m`: `id` is a monster vnum (`SetMonsterVID`).
    Monster,
    /// `g`: `id` is a group id (`SetMonsterGroupID`).
    Group,
}

/// What `LoadMonsterAreaInfo` stores in a `CMonsterAreaInfo`. The spawn
/// positions and direction vector C++ derives are random, so not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterArea {
    pub kind: MonsterAreaKind,
    /// Center (`cx`, `cy`) and half size (`sx`, `sy`), `atol` (64-bit).
    pub origin: (i64, i64),
    pub size: (i64, i64),
    pub id: u32,
    pub count: u32,
    /// `EMonsterDir`: 0 random, 1..=8 north clockwise (any `atoi` value is
    /// stored as is).
    pub direction: i32,
}

impl MonsterArea {
    /// `SetLRTB`: left, top, right, bottom (origin ∓ size).
    pub fn bounds(&self) -> (i64, i64, i64, i64) {
        let (ox, oy) = self.origin;
        let (sx, sy) = self.size;
        (
            ox.wrapping_sub(sx),
            oy.wrapping_sub(sy),
            ox.wrapping_add(sx),
            oy.wrapping_add(sy),
        )
    }
}

/// `CMapOutdoor::LoadMonsterAreaInfo` after reading the file. Lines whose
/// first token (lowercased) is `m` or `g` with at least 11 tokens become
/// areas; `z`, `time` and `percent` are parsed by C++ and dropped.
pub fn parse_monster_areas(data: &[u8]) -> Vec<MonsterArea> {
    let mut areas = Vec::new();
    for line in text::lines(data) {
        let Some(tokens) = text::split_line(line, DEFAULT_DELIMITERS) else {
            continue;
        };
        let kind = match tokens[0].to_ascii_lowercase().as_slice() {
            b"m" => MonsterAreaKind::Monster,
            b"g" => MonsterAreaKind::Group,
            _ => continue,
        };
        if tokens.len() < 11 {
            continue;
        }
        areas.push(MonsterArea {
            kind,
            origin: (atol(tokens[1]), atol(tokens[2])),
            size: (atol(tokens[3]), atol(tokens[4])),
            direction: atoi(tokens[6]),
            count: atoi(tokens[9]) as u32,
            id: atoi(tokens[10]) as u32,
        });
    }
    areas
}
