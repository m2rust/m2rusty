//! Skill tooltips: `uiToolTip.SkillToolTip.SetSkillNew`/`SetSkillOnlyName`
//! for the skill page and the task bar. The lines come from `skilldesc.txt`
//! (name, level limit, description, conditions, affect formulas, duration,
//! cool time and SP cost), evaluated at the skill's current and next power,
//! with the wording of `locale_game.txt`.

use m2_formats::proto::SkillData;
use m2_game::combat::{self, Points, SkillExt, attribute};

use crate::item_tooltip::{Line, LocaleGame, color};

/// `CAN_LEVEL_UP_COLOR` (`0xff8EC292`).
const CAN_LEVEL_UP: bevy::color::Color =
    bevy::color::Color::srgb(0x8E as f32 / 255.0, 0xC2 as f32 / 255.0, 0x92 as f32 / 255.0);

/// `player.SKILL_INDEX_TONGSOL` (leadership) and `SKILL_INDEX_RIDING`.
const SKILL_LEADERSHIP: u32 = 121;
const SKILL_RIDING: u32 = 130;
/// `skill.SKILL_GRADE_COUNT`: the perfect master grade.
const GRADE_PERFECT: u8 = 3;

/// What the hovered slot shows (`uicharacter.OverInItem`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillTip {
    /// `SetSkillNew`: everything, for the grade the skill is at.
    Full,
    /// `SetSkillOnlyName`: a grade cell the skill has not reached yet.
    OnlyName,
}

pub struct SkillInfo<'a> {
    pub data: &'a SkillData,
    pub vnum: u32,
    /// The grade the hovered cell stands for.
    pub grade: u8,
    /// `player.GetSkillGrade` / `GetSkillLevel` (the level within the grade).
    pub real_grade: u8,
    pub level: u8,
    pub points: &'a [i32; 255],
}

fn text(locale: &LocaleGame, key: &str, fallback: &str) -> String {
    locale.get(key).unwrap_or(fallback).trim_end().to_owned()
}

fn format(locale: &LocaleGame, key: &str, fallback: &str, args: &[i64]) -> String {
    locale
        .format(key, args)
        .unwrap_or_else(|| crate::item_tooltip::printf(fallback, args, &[]))
        .trim_end()
        .to_owned()
}

fn decode(bytes: &[u8]) -> String {
    m2_formats::utf8::decode(1252, bytes).into_owned()
}

/// `skill.GetSkillName(index, grade)`: the grade's own name when it has one.
fn skill_name(data: &SkillData, grade: u8) -> String {
    let grade_name = data.grades.get(usize::from(grade)).map(|g| decode(&g.name)).filter(|_| grade > 0);
    grade_name.filter(|name| !name.is_empty()).unwrap_or_else(|| decode(&data.name))
}

/// `CPythonSkill::SSkillData::ProcessFormula` on one of the skill's formulas.
fn eval(formula: &[u8], power: f32, points: &Points) -> f32 {
    let formula = String::from_utf8_lossy(formula);
    if formula.trim().is_empty() { 0.0 } else { combat::eval_formula(&formula, power, points) }
}

/// The `_snprintf(description, min, max)` of `GetAffectDescription`: the
/// float conversions (`%.0f`, `%.1f`, `%f`, `%d`, `%%`) of the shipped
/// descriptions, fed from the two values in order.
pub fn format_affect(description: &str, min: f32, max: f32) -> String {
    let (min, max) = (min.abs(), max.abs());
    let (min, max) = if description.contains("%.0f") { (min.floor(), max.floor()) } else { (min, max) };
    let mut values = [min, max].into_iter();
    let mut out = String::new();
    let mut chars = description.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut precision = None;
        let mut spec = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_ascii_digit() || matches!(next, '.' | '-' | '+') {
                spec.push(next);
                chars.next();
            } else {
                break;
            }
        }
        if let Some((_, digits)) = spec.split_once('.') {
            precision = digits.parse::<usize>().ok().or(Some(0));
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('f') => {
                let value = values.next().unwrap_or(0.0);
                out.push_str(&format!("{value:.*}", precision.unwrap_or(6)));
            }
            Some('d' | 'i') => out.push_str(&(values.next().unwrap_or(0.0) as i64).to_string()),
            Some(other) => {
                out.push('%');
                out.push_str(&spec);
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

/// `__SetSkillTitle`, then `AppendDefaultData`'s level limit, description and
/// `AppendSkillConditionData`.
fn default_data(lines: &mut Vec<Line>, skill: &SkillInfo, locale: &LocaleGame, limit: bool) {
    title(lines, skill.data, skill.grade, locale);
    let level_limit = i64::from(skill.data.level_limit);
    if limit && level_limit > 0 {
        let player_level = i64::from(skill.points[1]);
        let tint = if player_level < level_limit { color::NEGATIVE } else { color::NORMAL };
        lines.push((format(locale, "TOOLTIP_ITEM_LIMIT_LEVEL", "From Level: %d", &[level_limit]), tint));
    }
    description(lines, skill.data);
}

fn title(lines: &mut Vec<Line>, data: &SkillData, grade: u8, locale: &LocaleGame) {
    lines.push((skill_name(data, grade), color::TITLE));
    // `__AppendSkillGradeName`.
    let key = match grade {
        1 => Some(("SKILL_GRADE_NAME_MASTER", "%s Master")),
        2 => Some(("SKILL_GRADE_NAME_GRAND_MASTER", "%s Grand Master")),
        3 => Some(("SKILL_GRADE_NAME_PERFECT_MASTER", "%s Perfect Master")),
        _ => None,
    };
    if let Some((key, fallback)) = key {
        let base = skill_name(data, 0);
        let value = locale.get(key).unwrap_or(fallback).trim_end().replacen("%s", &base, 1);
        lines.push((value, CAN_LEVEL_UP));
    }
}

fn description(lines: &mut Vec<Line>, data: &SkillData) {
    let description = decode(&data.description);
    if !description.trim().is_empty() {
        lines.push((description.trim().to_owned(), color::NORMAL));
    }
}

fn conditions(lines: &mut Vec<Line>, data: &SkillData) {
    for condition in &data.conditions {
        lines.push((decode(condition), color::CONDITION));
    }
}

/// `HasSkillLevelDescription`. The Python passes the level, not the power,
/// to the cool time and SP formulas here.
fn has_level_description(data: &SkillData, level: u8, points: &Points) -> bool {
    let level = f32::from(level);
    !data.affects.is_empty() || eval(&data.cool_time_formula, level, points) as u32 > 0 || eval(&data.need_sp_formula, level, points) as i32 > 0
}

/// `AppendSkillLevelDescriptionNew` (no `AffectDataNew` is ever loaded, so
/// the `skilldesc` affect descriptions are used).
fn level_description(lines: &mut Vec<Line>, data: &SkillData, power: f32, points: &Points, locale: &LocaleGame, tint: bevy::color::Color) {
    for affect in &data.affects {
        let min = eval(&affect.min_formula, power, points);
        let max = eval(&affect.max_formula, power, points);
        lines.push((format_affect(&decode(&affect.description), min, max), tint));
    }
    let duration = eval(&data.duration, power, points) as u32;
    if duration > 0 {
        lines.push((format(locale, "TOOLTIP_SKILL_DURATION", "Duration: %d Sek.", &[i64::from(duration)]), tint));
    }
    let cool_time = eval(&data.cool_time_formula, power, points) as u32;
    if cool_time > 0 {
        lines.push((format!("{}{cool_time}", locale.get("TOOLTIP_SKILL_COOL_TIME").unwrap_or("Recharge: ")), tint));
    }
    let need = eval(&data.need_sp_formula, power, points) as i32;
    if need != 0 {
        let continuation = i64::from(eval(&data.continuation_sp_formula, power, points) as u32);
        let (cost, per_second) = if data.has(attribute::USE_HP) {
            (format(locale, "TOOLTIP_NEED_HP", "Needed HP: %d", &[i64::from(need)]), ("TOOLTIP_NEED_HP_PER_SEC", "HP / Sec.: %d"))
        } else if need == -1 {
            (text(locale, "TOOLTIP_NEED_ALL_SP", "Needed SP: Max."), ("TOOLTIP_NEED_SP_PER_SEC", "SP / Sec.: %d"))
        } else {
            (format(locale, "TOOLTIP_NEED_SP", "Needed SP: %d", &[i64::from(need)]), ("TOOLTIP_NEED_SP_PER_SEC", "SP / Sec.: %d"))
        };
        lines.push((cost, tint));
        if continuation > 0 {
            lines.push((format(locale, per_second.0, per_second.1, &[continuation]), tint));
        }
    }
}

/// `AppendSkillDataNew`: the current level's effect, then the next level's.
fn skill_data(lines: &mut Vec<Line>, skill: &SkillInfo, points: &Points, locale: &LocaleGame) {
    // `skillMaxLevelEndDict`, by the real grade.
    let end = match skill.real_grade { 1 | 2 => 10, _ => 20 };
    let level = skill.level;
    // `fcurEfficientPercentage` / `fnextEfficientPercentage`.
    let current = combat::skill_power(level, skill.real_grade);
    let next = if level == 0 { 0.05 } else { combat::skill_power(level.saturating_add(1), skill.real_grade) };
    if level > 0 && has_level_description(skill.data, level, points) {
        if skill.grade != GRADE_PERFECT {
            let label = if level == end {
                format(locale, "TOOLTIP_SKILL_LEVEL_MASTER", "Current Level: %d (Master)", &[i64::from(level)])
            } else {
                format(locale, "TOOLTIP_SKILL_LEVEL", "Current Level: %d", &[i64::from(level)])
            };
            lines.push((label, color::NORMAL));
        }
        level_description(lines, skill.data, current, points, locale, color::NORMAL);
    }
    if skill.grade != GRADE_PERFECT && level < end && has_level_description(skill.data, level + 1, points) {
        let label = if matches!(skill.vnum, 141 | 142) {
            format(locale, "TOOLTIP_NEXT_SKILL_LEVEL_3", "Next Level: %d", &[i64::from(level) + 1])
        } else {
            format(locale, "TOOLTIP_NEXT_SKILL_LEVEL_1", "Next Level: %d (Max. %d)", &[i64::from(level) + 1, i64::from(end)])
        };
        lines.push((label, color::DISABLE));
        level_description(lines, skill.data, next, points, locale, color::DISABLE);
    }
}

/// The level shown by the support pages: grade levels continue at 20/30/40.
fn absolute_level(grade: u8, level: u8) -> i64 {
    let level = i64::from(level);
    match grade { 1 => level + 19, 2 => level + 29, 3 => 40, _ => level }
}

/// `chop`: `round(n - 0.5, 1)`.
fn chop(value: f32) -> i64 {
    (((value - 0.5) * 10.0).round() / 10.0).round() as i64
}

/// `AppendPartySkillData`. C++ bug N38: the Python reads `player.GetSkillLevel(skillIndex)`
/// with the skill index as a slot index; the port uses the skill's level.
fn party_data(lines: &mut Vec<Line>, skill: &SkillInfo, locale: &LocaleGame) {
    let level = absolute_level(skill.grade, skill.level);
    if level <= 0 {
        return;
    }
    let k = f32::from(skill.level) / 100.0;
    lines.push((format(locale, "TOOLTIP_PARTY_SKILL_LEVEL", "Current Level: %d", &[level]), color::NORMAL));
    for (from, key, fallback, value) in [
        (10, "PARTY_SKILL_ATTACKER", "Base Attack Value Attacker +%.0f", 10.0 + 60.0 * k),
        (20, "PARTY_SKILL_BERSERKER", "Attack Speed Berserker +%.0f", 1.0 + 5.0 * k),
        (20, "PARTY_SKILL_TANKER", "Max. HP blade fighter +%.0f", 50.0 + 1450.0 * k),
        (25, "PARTY_SKILL_BUFFER", "Skill Duration Blocker +%.0f", 5.0 + 45.0 * k),
        (35, "PARTY_SKILL_SKILL_MASTER", "Max. SP Skill Master +%.0f", 25.0 + 600.0 * k),
        (40, "PARTY_SKILL_DEFENDER", "Defence Defender +%.0f", 5.0 + 30.0 * k),
    ] {
        if level >= from {
            lines.push((format(locale, key, fallback, &[chop(value)]), color::NORMAL));
        }
    }
}

/// The lines of the skill tooltip, title first.
pub fn skill_lines(skill: &SkillInfo, tip: SkillTip, locale: &LocaleGame) -> Vec<Line> {
    let points = Points::from_points(skill.points);
    let mut lines = Vec::new();
    match (tip, skill.vnum) {
        (SkillTip::OnlyName, _) => {
            default_data(&mut lines, skill, locale, true);
            conditions(&mut lines, skill.data);
        }
        (SkillTip::Full, SKILL_LEADERSHIP) => {
            default_data(&mut lines, skill, locale, true);
            party_data(&mut lines, skill, locale);
        }
        (SkillTip::Full, SKILL_RIDING) => {
            // `AppendSupportSkillDefaultData(..., 30)`.
            title(&mut lines, skill.data, skill.grade, locale);
            description(&mut lines, skill.data);
            let level = absolute_level(skill.grade, skill.level);
            lines.push((format(locale, "TOOLTIP_SKILL_LEVEL_WITH_MAX", "Current Level: %d (Max. %d)", &[level, 30]), color::NORMAL));
        }
        (SkillTip::Full, _) => {
            default_data(&mut lines, skill, locale, true);
            conditions(&mut lines, skill.data);
            skill_data(&mut lines, skill, &points, locale);
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_formats::proto::AffectData;

    fn locale() -> LocaleGame {
        LocaleGame::parse(
            b"TOOLTIP_SKILL_LEVEL\tCurrent Level: %d\t\nTOOLTIP_NEXT_SKILL_LEVEL_1\tNext Level: %d (Max. %d)\t\n\
TOOLTIP_SKILL_COOL_TIME\tRecharge: \nTOOLTIP_NEED_SP\tNeeded SP: %d\t\nTOOLTIP_SKILL_DURATION\tDuration: %d Sek.\t\n\
SKILL_GRADE_NAME_MASTER\t%s Master\t\nTOOLTIP_ITEM_LIMIT_LEVEL\tFrom Level: %d\t\n",
        )
    }

    fn skill() -> SkillData {
        SkillData {
            name: b"Three-Way Cut".to_vec(),
            description: b"Strikes three times.".to_vec(),
            conditions: vec![b"Requires a sword".to_vec()],
            affects: vec![AffectData { description: b"Damage %.0f - %.0f".to_vec(), min_formula: b"100*k".to_vec(), max_formula: b"-200*k".to_vec() }],
            cool_time_formula: b"10".to_vec(),
            need_sp_formula: b"20+40*k".to_vec(),
            duration: b"5+10*k".to_vec(),
            level_limit: 5,
            ..SkillData::default()
        }
    }

    #[test]
    fn affect_descriptions_use_c_float_conversions_on_absolute_values() {
        assert_eq!(format_affect("Damage %.0f - %.0f", 12.7, -30.2), "Damage 12 - 30");
        assert_eq!(format_affect("Speed +%.1f%%", 3.25, 0.0), "Speed +3.2%");
        assert_eq!(format_affect("%d hits", 3.9, 0.0), "3 hits");
    }

    #[test]
    fn a_learned_skill_lists_current_and_next_level_effects() {
        let data = skill();
        let mut points = [0; 255];
        points[1] = 3;
        let info = SkillInfo { data: &data, vnum: 1, grade: 0, real_grade: 0, level: 1, points: &points };
        let lines: Vec<String> = skill_lines(&info, SkillTip::Full, &locale()).into_iter().map(|(text, _)| text).collect();
        // Power 0.05 now, 0.06 at the next level.
        assert_eq!(
            lines,
            [
                "Three-Way Cut", "From Level: 5", "Strikes three times.", "Requires a sword",
                "Current Level: 1", "Damage 5 - 10", "Duration: 5 Sek.", "Recharge: 10", "Needed SP: 22",
                "Next Level: 2 (Max. 20)", "Damage 6 - 12", "Duration: 5 Sek.", "Recharge: 10", "Needed SP: 22",
            ]
        );
        assert_eq!(skill_lines(&info, SkillTip::Full, &locale())[1].1, color::NEGATIVE);
    }

    #[test]
    fn an_unreached_grade_shows_only_its_name_and_conditions() {
        let data = skill();
        let points = [0; 255];
        let info = SkillInfo { data: &data, vnum: 1, grade: 1, real_grade: 0, level: 5, points: &points };
        let lines: Vec<String> = skill_lines(&info, SkillTip::OnlyName, &locale()).into_iter().map(|(text, _)| text).collect();
        assert_eq!(lines, ["Three-Way Cut", "Three-Way Cut Master", "From Level: 5", "Strikes three times.", "Requires a sword"]);
    }
}
