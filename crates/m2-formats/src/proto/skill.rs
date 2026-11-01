//! `CPythonSkill::RegisterSkillDesc` and `RegisterSkillTable`
//! (`client/src/ui/PythonSkill.cpp`): SkillDesc.txt and SkillTable.txt.

use std::collections::BTreeMap;

use super::{Error, atoi, cstr, split_line};
use crate::text;

/// `SKILL_TYPE_*`, the job column of SkillDesc.txt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SkillType {
    None = 0,
    Active = 1,
    Support = 2,
    Guild = 3,
    Horse = 4,
}

impl SkillType {
    /// `m_SkillTypeIndexMap`, keyed by the lowercased job column.
    fn from_job(job: &[u8]) -> Option<Self> {
        Some(match job {
            b"none" => SkillType::None,
            b"active" | b"warrior" | b"assassin" | b"sura" | b"shaman" | b"passive" => SkillType::Active,
            b"support" => SkillType::Support,
            b"guild" => SkillType::Guild,
            b"horse" => SkillType::Horse,
            _ => return None,
        })
    }
}

/// `m_SkillAttributeIndexMap`: `SKILL_ATTRIBUTE_*` by name.
fn attribute_flag(name: &[u8]) -> Option<u32> {
    Some(match name {
        b"NEED_TARGET" => 1 << 0,
        b"TOGGLE" => 1 << 1,
        b"WEAPON_LIMITATION" => 1 << 2,
        b"MELEE_ATTACK" => 1 << 3,
        b"USE_HP" => 1 << 4,
        b"CAN_CHANGE_DIRECTION" => 1 << 5,
        b"STANDING_SKILL" => 1 << 6,
        b"ONLY_FOR_ALLIANCE" => 1 << 7,
        b"CAN_USE_FOR_ME" => 1 << 8,
        b"NEED_CORPSE" => 1 << 9,
        b"FAN_RANGE" => 1 << 10,
        b"CAN_USE_IF_NOT_ENOUGH" => 1 << 11,
        b"NEED_EMPTY_BOTTLE" => 1 << 12,
        b"NEED_POISON_BOTTLE" => 1 << 13,
        b"ATTACK_SKILL" => 1 << 14,
        b"TIME_INCREASE_SKILL" => 1 << 15,
        b"CHARGE_ATTACK" => 1 << 16,
        b"PASSIVE" => 1 << 17,
        b"CANNOT_LEVEL_UP" => 1 << 18,
        b"ONLY_FOR_GUILD_WAR" => 1 << 19,
        b"MOVING_SKILL" => 1 << 20,
        b"HORSE_SKILL" => 1 << 21,
        b"CIRCLE_RANGE" => 1 << 22,
        b"SEARCH_TARGET" => 1 << 23,
        _ => return None,
    })
}

/// `m_SkillNeedWeaponIndexMap`: `1 << CItemData::WEAPON_*` by name.
fn need_weapon_flag(name: &[u8]) -> Option<u32> {
    Some(match name {
        b"SWORD" => 1 << 0,
        b"DAGGER" | b"DOUBLE_SWORD" => 1 << 1,
        b"BOW" => 1 << 2,
        b"TWO_HANDED" => 1 << 3,
        b"BELL" => 1 << 4,
        b"FAN" => 1 << 5,
        b"ARROW" => 1 << 6,
        b"EMPTY_HAND" => 1 << 8,
        _ => return None,
    })
}

/// `CPythonSkill::TAffectData`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AffectData {
    pub description: Vec<u8>,
    pub min_formula: Vec<u8>,
    pub max_formula: Vec<u8>,
}

/// `CPythonSkill::TGradeData` without its icon image.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GradeData {
    pub name: Vec<u8>,
    pub motion_index: u16,
}

/// `CPythonSkill::TSkillData`: the fields the two loaders set, plus the
/// constructor's defaults for the rest. Icon images are not loaded (their
/// names are derived from the job and icon columns, see `RegisterSkillDesc`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillData {
    /// `byType`, a [`SkillType`] value.
    pub skill_type: u8,
    pub skill_index: u32,
    pub max_level: u8,
    pub level_up_point: u8,
    pub level_limit: u8,
    pub no_motion: bool,
    pub name: Vec<u8>,
    pub description: Vec<u8>,
    pub conditions: Vec<Vec<u8>>,
    pub affects: Vec<AffectData>,
    pub cool_time_formula: Vec<u8>,
    pub target_count_formula: Vec<u8>,
    pub motion_loop_count_formula: Vec<u8>,
    pub need_sp_formula: Vec<u8>,
    pub continuation_sp_formula: Vec<u8>,
    pub duration: Vec<u8>,
    pub skill_attribute: u32,
    pub need_weapon: u32,
    pub target_range: u32,
    pub motion_index: u16,
    /// `SKILL_EFFECT_COUNT` grades.
    pub grades: [GradeData; 4],
}

impl Default for SkillData {
    /// `SSkillData::SSkillData`.
    fn default() -> Self {
        Self {
            skill_type: SkillType::Active as u8,
            skill_index: 0,
            max_level: 20,
            level_up_point: 1,
            level_limit: 0,
            no_motion: false,
            name: Vec::new(),
            description: Vec::new(),
            conditions: Vec::new(),
            affects: Vec::new(),
            cool_time_formula: Vec::new(),
            target_count_formula: Vec::new(),
            motion_loop_count_formula: Vec::new(),
            need_sp_formula: Vec::new(),
            continuation_sp_formula: Vec::new(),
            duration: Vec::new(),
            skill_attribute: 0,
            need_weapon: 0,
            target_range: 0,
            motion_index: 0,
            grades: Default::default(),
        }
    }
}

/// SkillDesc.txt columns (`ESkillDescTokenType`).
mod desc {
    pub const VNUM: usize = 0;
    pub const JOB: usize = 1;
    pub const NAME1: usize = 2;
    pub const NAME2: usize = 3;
    pub const NAME3: usize = 4;
    pub const DESCRIPTION: usize = 5;
    pub const CONDITION1: usize = 6;
    pub const ATTRIBUTE: usize = 10;
    pub const WEAPON: usize = 11;
    pub const MOTION_INDEX: usize = 13;
    pub const MOTION_INDEX_GRADE_NUM: usize = 14;
    pub const TARGET_COUNT_FORMULA: usize = 15;
    pub const MOTION_LOOP_COUNT_FORMULA: usize = 16;
    pub const AFFECT_DESCRIPTION_1: usize = 17;
    pub const AFFECT_MIN_1: usize = 18;
    pub const AFFECT_MAX_1: usize = 19;
    pub const LEVEL_LIMIT: usize = 26;
    pub const MAX_LEVEL: usize = 27;

    pub const CONDITION_COLUMN_COUNT: usize = 3;
    pub const AFFECT_COLUMN_COUNT: usize = 3;
    pub const AFFECT_STEP_COUNT: usize = 3;
}

/// SkillTable.txt columns (`ESkillTableTokenType`).
mod table {
    pub const VNUM: usize = 0;
    pub const MAX_LEVEL: usize = 4;
    pub const LEVEL_LIMIT: usize = 5;
    pub const POINT_POLY: usize = 7;
    pub const SP_COST_POLY: usize = 8;
    pub const DURATION_POLY: usize = 9;
    pub const DURATION_SP_COST_POLY: usize = 10;
    pub const COOLDOWN_POLY: usize = 11;
    pub const TARGET_RANGE: usize = 25;
    pub const MAX_NUM: usize = 27;
}

const SKILL_EFFECT_COUNT: i32 = 4;
const SKILL_GRADEGAP: i32 = 25;

/// `CPythonSkill::m_SkillDataMap`.
#[derive(Debug, Clone, Default)]
pub struct SkillTables {
    pub skills: BTreeMap<u32, SkillData>,
}

impl SkillTables {
    /// `CPythonSkill::RegisterSkillDesc`. A vnum seen twice updates the same
    /// entry: attributes and weapons are OR-ed in and affects appended, as
    /// in C++. A line with an unknown job keeps a default entry.
    ///
    /// Stops with [`Error::GradeLimit`] (C++ returns false) on a line with
    /// more than four motion grades, keeping the lines before it.
    ///
    /// Deviations, for lines C++ handles with undefined behaviour: columns
    /// 2..=12 are read without a length check in C++; missing ones are empty
    /// here. A negative grade count writes before `GradeData` in C++; here
    /// only the four grades are written.
    pub fn register_skill_desc(&mut self, data: &[u8]) -> Result<(), Error> {
        for line in text::lines(data) {
            let tokens = text::split_by_tab(line);
            // An empty line, or no VNUM column.
            if tokens.is_empty() {
                continue;
            }
            let column = |i: usize| tokens.get(i).copied().unwrap_or(b"");

            let index = atoi(column(desc::VNUM)) as u32;
            if index == 0 || tokens.len() <= desc::JOB {
                continue;
            }

            let skill = self.skills.entry(index).or_default();
            skill.skill_index = index;

            let Some(skill_type) = SkillType::from_job(&column(desc::JOB).to_ascii_lowercase()) else {
                continue;
            };
            skill.skill_type = skill_type as u8;

            let name1 = column(desc::NAME1);
            skill.name = name1.to_vec();
            skill.grades[0].name = name1.to_vec();
            skill.grades[1].name = column(desc::NAME2).to_vec();
            skill.grades[2].name = column(desc::NAME3).to_vec();
            for grade in &mut skill.grades[1..3] {
                if grade.name.is_empty() {
                    grade.name = name1.to_vec();
                }
            }

            skill.description = column(desc::DESCRIPTION).to_vec();

            skill.conditions = (0..desc::CONDITION_COLUMN_COUNT)
                .map(|j| column(desc::CONDITION1 + j))
                .filter(|c| !c.is_empty())
                .map(<[u8]>::to_vec)
                .collect();

            // SplitLine's result is ignored: a partial split still counts.
            // Split from c_str(), so the column ends at a NUL.
            for name in split_line(cstr(column(desc::ATTRIBUTE)), b"|").0 {
                if let Some(flag) = attribute_flag(name) {
                    skill.skill_attribute |= flag;
                }
            }
            for name in split_line(cstr(column(desc::WEAPON)), b"|").0 {
                if let Some(flag) = need_weapon_flag(name) {
                    skill.need_weapon |= flag;
                }
            }

            if tokens.len() > desc::MOTION_INDEX {
                let motion = tokens[desc::MOTION_INDEX];
                skill.no_motion = motion.is_empty();
                skill.motion_index = if motion.is_empty() { 0 } else { atoi(motion) as u16 };
            } else {
                skill.motion_index = 0;
            }

            if tokens.len() > desc::TARGET_COUNT_FORMULA {
                skill.target_count_formula = tokens[desc::TARGET_COUNT_FORMULA].to_vec();
            }
            if tokens.len() > desc::MOTION_LOOP_COUNT_FORMULA {
                skill.motion_loop_count_formula = tokens[desc::MOTION_LOOP_COUNT_FORMULA].to_vec();
            }

            for j in 0..desc::AFFECT_COLUMN_COUNT {
                let description = desc::AFFECT_DESCRIPTION_1 + j * desc::AFFECT_STEP_COUNT;
                if column(description).is_empty() {
                    continue;
                }
                let min = column(desc::AFFECT_MIN_1 + j * desc::AFFECT_STEP_COUNT);
                let max = column(desc::AFFECT_MAX_1 + j * desc::AFFECT_STEP_COUNT);
                skill.affects.push(AffectData {
                    description: column(description).to_vec(),
                    min_formula: min.to_vec(),
                    max_formula: max.to_vec(),
                });
            }

            if tokens.len() > desc::MOTION_INDEX_GRADE_NUM {
                let grades = atoi(tokens[desc::MOTION_INDEX_GRADE_NUM]);
                if grades > SKILL_EFFECT_COUNT {
                    return Err(Error::GradeLimit { skill: index, grades });
                }
                let base = i32::from(skill.motion_index);
                for (g, grade) in skill.grades.iter_mut().enumerate() {
                    let g = g as i32;
                    grade.motion_index = if g < grades {
                        (base + g * SKILL_GRADEGAP) as u16
                    } else {
                        base as u16
                    };
                }
            }

            if tokens.len() > desc::LEVEL_LIMIT {
                let limit = tokens[desc::LEVEL_LIMIT];
                // static_cast<WORD> into a BYTE.
                skill.level_limit = if limit.is_empty() { 0 } else { atoi(limit) as u8 };
            }

            if tokens.len() > desc::MAX_LEVEL {
                let max_level = tokens[desc::MAX_LEVEL];
                skill.max_level = if !max_level.is_empty() {
                    atoi(max_level) as u8
                } else if skill.level_limit > 1 {
                    skill.level_limit
                } else {
                    20
                };
            }
        }
        Ok(())
    }

    /// `CPythonSkill::RegisterSkillTable`: lines of exactly 27 columns for
    /// skills SkillDesc.txt registered.
    ///
    /// `cheonma` is `LocaleService_IsCHEONMA()` (the YMIR locale). Elsewhere,
    /// except for skills 34 and 151..=157, the first affect's formulas are
    /// rebuilt from the point formula, with `number`/`atk`/`mwep` renamed to
    /// their min and max variants.
    pub fn register_skill_table(&mut self, data: &[u8], cheonma: bool) {
        for line in text::lines(data) {
            let tokens = text::split_by_tab(line);
            if tokens.len() != table::MAX_NUM {
                continue;
            }
            let vnum = atoi(tokens[table::VNUM]);
            let Some(skill) = self.skills.get_mut(&(vnum as u32)) else {
                continue;
            };

            let set_if_present = |field: &mut Vec<u8>, column: usize| {
                if !tokens[column].is_empty() {
                    *field = tokens[column].to_vec();
                }
            };
            set_if_present(&mut skill.need_sp_formula, table::SP_COST_POLY);
            set_if_present(&mut skill.cool_time_formula, table::COOLDOWN_POLY);
            set_if_present(&mut skill.continuation_sp_formula, table::DURATION_SP_COST_POLY);
            if !tokens[table::TARGET_RANGE].is_empty() {
                skill.target_range = atoi(tokens[table::TARGET_RANGE]) as u32;
            }
            skill.duration = tokens[table::DURATION_POLY].to_vec();

            // Compared against a DWORD, so negative levels count as large.
            if !tokens[table::MAX_LEVEL].is_empty() {
                let max_level = atoi(tokens[table::MAX_LEVEL]);
                if max_level as u32 > 1 {
                    skill.max_level = max_level as u8;
                }
            }
            // C++ tests the current limit, not the new one.
            if !tokens[table::LEVEL_LIMIT].is_empty() {
                let level_limit = atoi(tokens[table::LEVEL_LIMIT]);
                if skill.level_limit > 1 {
                    skill.level_limit = level_limit as u8;
                }
            }

            let use_skill_proto = !cheonma && !matches!(vnum, 34 | 151..=157);
            let Some(affect) = skill.affects.first_mut().filter(|_| use_skill_proto) else {
                continue;
            };
            let poly = tokens[table::POINT_POLY];
            // strstr: the formula as a C string.
            let has = |word: &[u8]| cstr(poly).windows(word.len()).any(|w| w == word);
            if has(b"atk") || has(b"mwep") || has(b"number") {
                let rename = |number: &[u8], atk: &[u8], mwep: &[u8]| {
                    let s = replace_word(poly, b"number", number);
                    let s = replace_word(&s, b"atk", atk);
                    replace_word(&s, b"mwep", mwep)
                };
                affect.min_formula = rename(b"min", b"minatk", b"minmwep");
                affect.max_formula = rename(b"max", b"maxatk", b"maxmwep");
                if vnum == 1 {
                    affect.min_formula.extend_from_slice(b"* 3");
                    affect.max_formula.extend_from_slice(b"* 3");
                }
            } else {
                affect.min_formula = poly.to_vec();
                affect.max_formula.clear();
            }
        }
    }
}

/// `string_replace_word`: every non-overlapping `from`, left to right.
fn replace_word(base: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(base.len());
    let mut cur = 0;
    while cur < base.len() {
        if base[cur..].starts_with(from) {
            out.extend_from_slice(to);
            cur += from.len();
        } else {
            out.push(base[cur]);
            cur += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(columns: &[&str]) -> Vec<u8> {
        let mut s = columns.join("\t").into_bytes();
        s.push(b'\n');
        s
    }

    #[test]
    fn replace() {
        assert_eq!(replace_word(b"atk*2+atk", b"atk", b"minatk"), b"minatk*2+minatk");
        assert_eq!(replace_word(b"aaa", b"aa", b"b"), b"ba");
        assert_eq!(replace_word(b"", b"a", b"b"), b"");
    }

    fn desc_line(vnum: &str, job: &str, extra: &[&str]) -> Vec<u8> {
        let mut columns = vec![
            vnum,
            job,
            "Name",
            "",
            "N3",
            "Desc",
            "c1",
            "",
            "c3",
            "c4",
            "TOGGLE|BAD|USE_HP",
            "SWORD|FAN",
            "icon",
        ];
        columns.extend_from_slice(extra);
        line(&columns)
    }

    #[test]
    fn skill_desc() {
        let mut skills = SkillTables::default();
        let mut data = desc_line(
            "1",
            "WARRIOR",
            &[
                "10", "3", "tc", "lc", "a1", "min1", "", "", "x", "y", "a3", "", "max3", "5", "", "z",
            ],
        );
        data.extend(desc_line("2", "unknown", &[]));
        data.extend(desc_line("0", "WARRIOR", &[]));
        data.extend(desc_line(
            "3",
            "support",
            &["", "2", "", "", "", "", "", "", "", "", "", "", "", "", "30"],
        ));
        skills.register_skill_desc(&data).unwrap();
        let s = &skills.skills[&3];
        assert!(s.no_motion);
        assert_eq!(s.max_level, 30);
        assert_eq!(
            s.grades.iter().map(|g| g.motion_index).collect::<Vec<_>>(),
            [0, 25, 0, 0]
        );

        let s = &skills.skills[&1];
        assert_eq!(s.skill_type, SkillType::Active as u8);
        assert_eq!(s.grades[1].name, b"Name");
        assert_eq!(s.grades[2].name, b"N3");
        assert_eq!(s.conditions, [b"c1".to_vec(), b"c3".to_vec()]);
        assert_eq!(s.skill_attribute, (1 << 1) | (1 << 4));
        assert_eq!(s.need_weapon, 1 | (1 << 5));
        assert_eq!(s.motion_index, 10);
        assert!(!s.no_motion);
        assert_eq!(
            s.grades.iter().map(|g| g.motion_index).collect::<Vec<_>>(),
            [10, 35, 60, 10]
        );
        assert_eq!(s.affects.len(), 2);
        assert_eq!(s.affects[1].max_formula, b"max3");
        assert_eq!((s.level_limit, s.max_level), (5, 5));

        // Unknown job: a default entry with only the index set.
        assert_eq!(skills.skills[&2].skill_type, SkillType::Active as u8);
        assert_eq!(skills.skills[&2].name, b"");
        assert!(!skills.skills.contains_key(&0));

        // More than four grades stops the file.
        let mut data = desc_line("4", "guild", &["1", "5"]);
        data.extend(desc_line("5", "guild", &[]));
        let err = skills.register_skill_desc(&data);
        assert!(matches!(err, Err(Error::GradeLimit { skill: 4, grades: 5 })));
        assert!(skills.skills.contains_key(&4));
        assert!(!skills.skills.contains_key(&5));
    }

    fn table_line(vnum: &str, max_level: &str, poly: &str) -> Vec<u8> {
        let mut columns = vec![""; 27];
        columns[0] = vnum;
        columns[4] = max_level;
        columns[5] = "7";
        columns[7] = poly;
        columns[8] = "sp";
        columns[9] = "dur";
        columns[25] = "300";
        columns[26] = "x";
        line(&columns)
    }

    #[test]
    fn skill_table() {
        let mut skills = SkillTables::default();
        skills
            .register_skill_desc(&desc_line("1", "warrior", &["", "", "", "", "a", "m", "M"]))
            .unwrap();
        skills
            .register_skill_desc(&desc_line("34", "warrior", &["", "", "", "", "a", "m", "M"]))
            .unwrap();
        skills
            .register_skill_desc(&desc_line("5", "warrior", &["", "", "", "", "a", "m", "M"]))
            .unwrap();

        let mut data = table_line("1", "-1", "number(atk, mwep)");
        data.extend(table_line("34", "1", "atk"));
        data.extend(table_line("5", "30", "lv*2"));
        data.extend(table_line("99", "30", "lv"));
        skills.register_skill_table(&data, false);

        let s = &skills.skills[&1];
        assert_eq!(s.affects[0].min_formula, b"min(minatk, minmwep)* 3");
        assert_eq!(s.affects[0].max_formula, b"max(maxatk, maxmwep)* 3");
        assert_eq!((s.max_level, s.level_limit, s.target_range), (255, 0, 300));
        assert_eq!(
            (s.need_sp_formula.as_slice(), s.duration.as_slice()),
            (&b"sp"[..], &b"dur"[..])
        );
        // Skill 34 keeps its SkillDesc formulas; max level 1 is ignored.
        assert_eq!(skills.skills[&34].affects[0].min_formula, b"m");
        assert_eq!(skills.skills[&34].max_level, 20);
        let s = &skills.skills[&5];
        assert_eq!(
            (s.affects[0].min_formula.as_slice(), s.affects[0].max_formula.as_slice()),
            (&b"lv*2"[..], &b""[..])
        );

        // CHEONMA keeps them all.
        let mut skills2 = SkillTables::default();
        skills2
            .register_skill_desc(&desc_line("1", "warrior", &["", "", "", "", "a", "m", "M"]))
            .unwrap();
        skills2.register_skill_table(&table_line("1", "3", "atk"), true);
        assert_eq!(skills2.skills[&1].affects[0].min_formula, b"m");
    }
}
