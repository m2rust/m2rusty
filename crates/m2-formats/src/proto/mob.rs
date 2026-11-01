//! `CPythonNonPlayer::LoadNonPlayerData` (`client/src/ui/PythonNonPlayer.cpp`):
//! mob_proto.

use std::collections::BTreeMap;

use super::{Error, Reader, le32};
use crate::crypt::{self, MOB_PROTO_KEY};

/// `sizeof(CPythonNonPlayer::TMobTable)`, packed.
pub const MOB_TABLE_SIZE: usize = 255;

const CHARACTER_NAME_LEN: usize = 24 + 1;

/// `CPythonNonPlayer::TMobSkillLevel`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MobSkillLevel {
    pub vnum: u32,
    pub level: u8,
}

/// `CPythonNonPlayer::TMobTable` (`client/include/Client/UI/PythonNonPlayer.h`),
/// one mob_proto record. Names and the folder are the raw `char` buffers.
#[derive(Debug, Clone, PartialEq)]
pub struct MobTable {
    pub vnum: u32,
    pub name: [u8; CHARACTER_NAME_LEN],
    pub locale_name: [u8; CHARACTER_NAME_LEN],
    pub mob_type: u8,
    pub rank: u8,
    pub battle_type: u8,
    pub level: u8,
    pub size: u8,
    pub gold_min: u32,
    pub gold_max: u32,
    pub exp: u32,
    pub max_hp: u32,
    pub regen_cycle: u8,
    pub regen_percent: u8,
    pub def: u16,
    pub ai_flag: u32,
    pub race_flag: u32,
    pub immune_flag: u32,
    pub str: u8,
    pub dex: u8,
    pub con: u8,
    pub int: u8,
    pub damage_range: [u32; 2],
    pub attack_speed: i16,
    pub moving_speed: i16,
    pub aggressive_hp_pct: u8,
    pub aggressive_sight: u16,
    pub attack_range: u16,
    pub enchants: [i8; 6],
    pub resists: [i8; 11],
    pub resurrection_vnum: u32,
    pub drop_item_vnum: u32,
    pub mount_capacity: u8,
    pub on_click_type: u8,
    pub empire: u8,
    pub folder: [u8; 65],
    pub dam_multiply: f32,
    pub summon_vnum: u32,
    pub drain_sp: u32,
    pub monster_color: u32,
    pub polymorph_item_vnum: u32,
    pub skills: [MobSkillLevel; 5],
    pub berserk_point: u8,
    pub stone_skin_point: u8,
    pub god_speed_point: u8,
    pub death_blow_point: u8,
    pub revive_point: u8,
}

impl MobTable {
    /// Decodes one packed record.
    ///
    /// # Panics
    /// If `record` is shorter than [`MOB_TABLE_SIZE`].
    pub fn parse(record: &[u8]) -> Self {
        assert!(record.len() >= MOB_TABLE_SIZE);
        let mut r = Reader::new(record);
        Self {
            vnum: r.u32(),
            name: r.bytes(),
            locale_name: r.bytes(),
            mob_type: r.u8(),
            rank: r.u8(),
            battle_type: r.u8(),
            level: r.u8(),
            size: r.u8(),
            gold_min: r.u32(),
            gold_max: r.u32(),
            exp: r.u32(),
            max_hp: r.u32(),
            regen_cycle: r.u8(),
            regen_percent: r.u8(),
            def: r.u16(),
            ai_flag: r.u32(),
            race_flag: r.u32(),
            immune_flag: r.u32(),
            str: r.u8(),
            dex: r.u8(),
            con: r.u8(),
            int: r.u8(),
            damage_range: [r.u32(), r.u32()],
            attack_speed: r.i16(),
            moving_speed: r.i16(),
            aggressive_hp_pct: r.u8(),
            aggressive_sight: r.u16(),
            attack_range: r.u16(),
            enchants: std::array::from_fn(|_| r.i8()),
            resists: std::array::from_fn(|_| r.i8()),
            resurrection_vnum: r.u32(),
            drop_item_vnum: r.u32(),
            mount_capacity: r.u8(),
            on_click_type: r.u8(),
            empire: r.u8(),
            folder: r.bytes(),
            dam_multiply: r.f32(),
            summon_vnum: r.u32(),
            drain_sp: r.u32(),
            monster_color: r.u32(),
            polymorph_item_vnum: r.u32(),
            skills: std::array::from_fn(|_| MobSkillLevel {
                vnum: r.u32(),
                level: r.u8(),
            }),
            berserk_point: r.u8(),
            stone_skin_point: r.u8(),
            god_speed_point: r.u8(),
            death_blow_point: r.u8(),
            revive_point: r.u8(),
        }
    }
}

/// Decodes a mob_proto file: `MMPT`, elements, data size, then a
/// `CLZObject` encrypted with [`MOB_PROTO_KEY`] whose size must be a
/// multiple of the record. C++ reads `elements` records whatever the
/// decompressed size; here a file claiming more than it holds is rejected.
pub fn parse_mob_proto(data: &[u8]) -> Result<Vec<MobTable>, Error> {
    let fourcc = le32(data, 0).ok_or(Error::Truncated)?;
    if fourcc != u32::from_le_bytes(*b"MMPT") {
        return Err(Error::BadFourCC(fourcc));
    }
    let elements = le32(data, 4).ok_or(Error::Truncated)? as usize;
    let data_size = le32(data, 8).ok_or(Error::Truncated)? as usize;
    let blob = data.get(12..12 + data_size).ok_or(Error::Truncated)?;

    let records = crypt::lz_decompress(blob, Some(&MOB_PROTO_KEY))?;
    // CLZObject::GetSize() is the real size for a decompressed object.
    if !records.len().is_multiple_of(MOB_TABLE_SIZE) {
        return Err(Error::BadSize(records.len() as u32));
    }
    if elements.checked_mul(MOB_TABLE_SIZE).is_none_or(|n| n > records.len()) {
        return Err(Error::Truncated);
    }
    Ok(records
        .as_chunks::<MOB_TABLE_SIZE>()
        .0
        .iter()
        .take(elements)
        .map(|r| MobTable::parse(r))
        .collect())
}

/// `CPythonNonPlayer::m_NonPlayerDataMap`.
#[derive(Debug, Clone, Default)]
pub struct MobTables {
    pub mobs: BTreeMap<u32, MobTable>,
}

impl MobTables {
    /// `LoadNonPlayerData` after decoding: the first record of a vnum wins
    /// (`std::map::insert`).
    pub fn insert_all(&mut self, tables: Vec<MobTable>) {
        for table in tables {
            self.mobs.entry(table.vnum).or_insert(table);
        }
    }
}
