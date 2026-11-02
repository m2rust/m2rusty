//! Item tooltips: `uiToolTip.ItemToolTip.AddItemData` for the items the
//! inventory, equipment and task bar show. The lines are built from
//! `item_proto` (limits, attack/defence values, applies, wear flags), the
//! item's own bonuses (`TPlayerItemAttribute`) and metin sockets, with the
//! wording of `locale_game.txt` (`localeInfo.TOOLTIP_*`).

use std::collections::HashMap;

use bevy::prelude::*;
use m2_assets::M2Packs;
use m2_formats::proto::ItemData;
use m2_game::ingame::LOCALE_PATH;
use m2_game::player::{ItemCatalog, ItemStack};

/// `localeInfo`'s `locale_game.txt` strings. `SA` entries format their one
/// argument; `SNA` entries ignore it (`LoadLocaleFile`'s `funcDict`).
#[derive(Resource, Default)]
pub struct LocaleGame {
    strings: HashMap<String, (String, bool)>,
}

impl LocaleGame {
    /// `localeInfo.LoadLocaleFile`, from the English locale (code page 1252).
    pub fn parse(data: &[u8]) -> Self {
        let text = m2_formats::utf8::decode(1252, data);
        let mut strings = HashMap::new();
        for line in text.lines() {
            let mut fields = line.split('\t');
            let (Some(key), Some(value)) = (fields.next(), fields.next()) else { continue };
            let ignores_argument = fields.next().is_some_and(|kind| kind.trim() == "SNA");
            strings.insert(key.to_owned(), (value.to_owned(), ignores_argument));
        }
        Self { strings }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.strings.get(key).map(|(value, _)| value.as_str())
    }

    /// A string formatted with integer arguments (`text % args`).
    pub fn format(&self, key: &str, args: &[i64]) -> Option<String> {
        let (value, ignores) = self.strings.get(key)?;
        Some(if *ignores { value.clone() } else { printf(value, args, &[]) })
    }

    fn format_str(&self, key: &str, args: &[&str]) -> Option<String> {
        self.strings.get(key).map(|(value, _)| printf(value, &[], args))
    }
}

/// The `%d`, `%s` and `%%` of Python's `%` operator, applied in order.
pub(crate) fn printf(format: &str, ints: &[i64], strs: &[&str]) -> String {
    let mut out = String::new();
    let (mut ints, mut strs) = (ints.iter(), strs.iter());
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // Skip flags and width (`%3d`, `%-d`, `%.1f`).
        let mut spec = String::new();
        while let Some(&next) = chars.peek() {
            if next.is_ascii_digit() || matches!(next, '.' | '-' | '+') {
                spec.push(next);
                chars.next();
            } else {
                break;
            }
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('d' | 'i' | 'u') => out.push_str(&ints.next().map_or_else(String::new, i64::to_string)),
            Some('f') => out.push_str(&ints.next().map_or_else(String::new, i64::to_string)),
            Some('s') => out.push_str(strs.next().copied().unwrap_or("")),
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

/// `uiToolTip.ItemToolTip.AFFECT_DICT`: `item.APPLY_*` (the `EApplyTypes`
/// order of `ItemData.h`) to its `localeInfo` key.
fn apply_key(apply: u8) -> Option<&'static str> {
    Some(match apply {
        1 => "TOOLTIP_MAX_HP",
        2 => "TOOLTIP_MAX_SP",
        3 => "TOOLTIP_CON",
        4 => "TOOLTIP_INT",
        5 => "TOOLTIP_STR",
        6 => "TOOLTIP_DEX",
        7 => "TOOLTIP_ATT_SPEED",
        8 => "TOOLTIP_MOV_SPEED",
        9 => "TOOLTIP_CAST_SPEED",
        10 => "TOOLTIP_HP_REGEN",
        11 => "TOOLTIP_SP_REGEN",
        12 => "TOOLTIP_APPLY_POISON_PCT",
        13 => "TOOLTIP_APPLY_STUN_PCT",
        14 => "TOOLTIP_APPLY_SLOW_PCT",
        15 => "TOOLTIP_APPLY_CRITICAL_PCT",
        16 => "TOOLTIP_APPLY_PENETRATE_PCT",
        17 => "TOOLTIP_APPLY_ATTBONUS_HUMAN",
        18 => "TOOLTIP_APPLY_ATTBONUS_ANIMAL",
        19 => "TOOLTIP_APPLY_ATTBONUS_ORC",
        20 => "TOOLTIP_APPLY_ATTBONUS_MILGYO",
        21 => "TOOLTIP_APPLY_ATTBONUS_UNDEAD",
        22 => "TOOLTIP_APPLY_ATTBONUS_DEVIL",
        23 => "TOOLTIP_APPLY_STEAL_HP",
        24 => "TOOLTIP_APPLY_STEAL_SP",
        25 => "TOOLTIP_APPLY_MANA_BURN_PCT",
        26 => "TOOLTIP_APPLY_DAMAGE_SP_RECOVER",
        27 => "TOOLTIP_APPLY_BLOCK",
        28 => "TOOLTIP_APPLY_DODGE",
        29 => "TOOLTIP_APPLY_RESIST_SWORD",
        30 => "TOOLTIP_APPLY_RESIST_TWOHAND",
        31 => "TOOLTIP_APPLY_RESIST_DAGGER",
        32 => "TOOLTIP_APPLY_RESIST_BELL",
        33 => "TOOLTIP_APPLY_RESIST_FAN",
        34 => "TOOLTIP_RESIST_BOW",
        35 => "TOOLTIP_RESIST_FIRE",
        36 => "TOOLTIP_RESIST_ELEC",
        37 => "TOOLTIP_RESIST_MAGIC",
        38 => "TOOLTIP_APPLY_RESIST_WIND",
        39 => "TOOLTIP_APPLY_REFLECT_MELEE",
        40 => "TOOLTIP_APPLY_REFLECT_CURSE",
        41 => "TOOLTIP_APPLY_POISON_REDUCE",
        42 => "TOOLTIP_APPLY_KILL_SP_RECOVER",
        43 => "TOOLTIP_APPLY_EXP_DOUBLE_BONUS",
        44 => "TOOLTIP_APPLY_GOLD_DOUBLE_BONUS",
        45 => "TOOLTIP_APPLY_ITEM_DROP_BONUS",
        46 => "TOOLTIP_APPLY_POTION_BONUS",
        47 => "TOOLTIP_APPLY_KILL_HP_RECOVER",
        48 => "TOOLTIP_APPLY_IMMUNE_STUN",
        49 => "TOOLTIP_APPLY_IMMUNE_SLOW",
        50 => "TOOLTIP_APPLY_IMMUNE_FALL",
        52 => "TOOLTIP_BOW_DISTANCE",
        53 => "TOOLTIP_ATT_GRADE",
        54 => "TOOLTIP_DEF_GRADE",
        55 => "TOOLTIP_MAGIC_ATT_GRADE",
        56 => "TOOLTIP_MAGIC_DEF_GRADE",
        58 => "TOOLTIP_MAX_STAMINA",
        59 => "TOOLTIP_APPLY_ATTBONUS_WARRIOR",
        60 => "TOOLTIP_APPLY_ATTBONUS_ASSASSIN",
        61 => "TOOLTIP_APPLY_ATTBONUS_SURA",
        62 => "TOOLTIP_APPLY_ATTBONUS_SHAMAN",
        63 => "TOOLTIP_APPLY_ATTBONUS_MONSTER",
        64 => "TOOLTIP_MALL_ATTBONUS",
        65 => "TOOLTIP_MALL_DEFBONUS",
        66 => "TOOLTIP_MALL_EXPBONUS",
        67 => "TOOLTIP_MALL_ITEMBONUS",
        68 => "TOOLTIP_MALL_GOLDBONUS",
        69 => "TOOLTIP_APPLY_MAX_HP_PCT",
        70 => "TOOLTIP_APPLY_MAX_SP_PCT",
        71 => "TOOLTIP_SKILL_DAMAGE_BONUS",
        72 => "TOOLTIP_NORMAL_HIT_DAMAGE_BONUS",
        73 => "TOOLTIP_SKILL_DEFEND_BONUS",
        74 => "TOOLTIP_NORMAL_HIT_DEFEND_BONUS",
        76 => "TOOLTIP_MALL_EXPBONUS_P_STATIC",
        77 => "TOOLTIP_MALL_ITEMBONUS_P_STATIC",
        78 => "TOOLTIP_APPLY_RESIST_WARRIOR",
        79 => "TOOLTIP_APPLY_RESIST_ASSASSIN",
        80 => "TOOLTIP_APPLY_RESIST_SURA",
        81 => "TOOLTIP_APPLY_RESIST_SHAMAN",
        82 => "TOOLTIP_ENERGY",
        84 => "TOOLTIP_COSTUME_ATTR_BONUS",
        85 => "TOOLTIP_MAGIC_ATTBONUS_PER",
        86 => "TOOLTIP_MELEE_MAGIC_ATTBONUS_PER",
        87 => "TOOLTIP_RESIST_ICE",
        88 => "TOOLTIP_RESIST_EARTH",
        89 => "TOOLTIP_RESIST_DARK",
        90 => "TOOLTIP_ANTI_CRITICAL_PCT",
        91 => "TOOLTIP_ANTI_PENETRATE_PCT",
        _ => return None,
    })
}

/// `ItemToolTip` colours (`grp.GenerateColor` / packed ARGB).
pub mod color {
    use bevy::color::Color;
    pub const TITLE: Color = Color::srgb(0.9490, 0.9058, 0.7568);
    pub const SPECIAL_TITLE: Color = Color::srgb(1.0, 0.7843, 0.0);
    pub const NORMAL: Color = Color::srgb(0.7607, 0.7607, 0.7607);
    pub const DISABLE: Color = Color::srgb(0.9, 0.4745, 0.4627);
    pub const NEGATIVE: Color = DISABLE;
    pub const POSITIVE: Color = Color::srgb(0.5411, 0.7254, 0.5568);
    pub const SPECIAL_POSITIVE: Color = Color::srgb(0.6911, 0.8754, 0.7068);
    pub const SPECIAL_POSITIVE2: Color = Color::srgb(0.8824, 0.9804, 0.8824);
    /// `0xffBEB47D`.
    pub const CONDITION: Color = Color::srgb(0xBE as f32 / 255.0, 0xB4 as f32 / 255.0, 0x7D as f32 / 255.0);
}

/// `ITEM_TYPE_*` (`ItemData.h`).
const ITEM_TYPE_WEAPON: u8 = 1;
const ITEM_TYPE_ARMOR: u8 = 2;
const ITEM_TYPE_METIN: u8 = 10;
const ITEM_TYPE_COSTUME: u8 = 28;
const ITEM_TYPE_RING: u8 = 33;
const ITEM_TYPE_BELT: u8 = 34;
/// `WEAPON_FAN`, `ARMOR_WRIST`/`ARMOR_NECK`/`ARMOR_EAR`.
const WEAPON_FAN: u8 = 5;
/// `LIMIT_LEVEL`.
const LIMIT_LEVEL: u8 = 1;
/// `ITEM_ANTIFLAG_*`.
const ANTIFLAG_FEMALE: u32 = 1 << 0;
const ANTIFLAG_MALE: u32 = 1 << 1;
const ANTIFLAG_JOBS: [(u32, &str); 4] = [
    (1 << 2, "TOOLTIP_WARRIOR"),
    (1 << 3, "TOOLTIP_ASSASSIN"),
    (1 << 4, "TOOLTIP_SURA"),
    (1 << 5, "TOOLTIP_SHAMAN"),
];
/// `player.METIN_SOCKET_TYPE_*` and `item.METIN_GOLD`.
const SOCKET_SILVER: i32 = 1;
const SOCKET_GOLD: i32 = 2;

/// One line of the tooltip; `None` text is `AppendSpace(5)`.
pub type Line = (String, Color);

fn change_color(value: i64) -> Color {
    // `GetChangeTextLineColor`.
    match value.cmp(&0) {
        std::cmp::Ordering::Greater => color::POSITIVE,
        std::cmp::Ordering::Equal => color::NORMAL,
        std::cmp::Ordering::Less => color::NEGATIVE,
    }
}

/// `__GetAffectString`.
fn affect_string(locale: &LocaleGame, apply: u8, value: i64) -> Option<String> {
    if apply == 0 || value == 0 {
        return None;
    }
    match apply_key(apply) {
        Some(key) => locale.format(key, &[value]),
        None => Some(format!("UNKNOWN_TYPE[{apply}] {value}")),
    }
}

/// `ItemToolTip.AddItemData` for an item the player holds. `level` is the
/// player's level (`GetLimitTextLineColor`).
pub fn item_lines(data: &ItemData, stack: &ItemStack, catalog: &ItemCatalog, locale: &LocaleGame, level: u32) -> Vec<Line> {
    let table = &data.table;
    let mut lines: Vec<Line> = Vec::new();
    let has_attributes = stack.attributes.iter().any(|attribute| attribute.bType != 0);
    // `__SetItemTitle`: bonus items get the special (gold) title.
    let name = table.locale_name_utf8(1252).into_owned();
    lines.push((name, if has_attributes { color::SPECIAL_TITLE } else { color::TITLE }));

    let description = data.description_utf8(1252);
    if !description.is_empty() {
        lines.push((description.into_owned(), color::NORMAL));
    }
    let summary = data.summary_utf8(1252);
    if !summary.is_empty() {
        lines.push((summary.into_owned(), color::CONDITION));
    }

    let limits = |lines: &mut Vec<Line>| {
        // `__AppendLimitInformation` (only the level limit is shown).
        for limit in &table.limits {
            if limit.value > 0 && limit.limit_type == LIMIT_LEVEL
                && let Some(text) = locale.format("TOOLTIP_ITEM_LIMIT_LEVEL", &[i64::from(limit.value)])
            {
                let shade = if (level as i64) < i64::from(limit.value) { color::DISABLE } else { color::NORMAL };
                lines.push((text, shade));
            }
        }
    };
    let affects = |lines: &mut Vec<Line>| {
        // `__AppendAffectInformation`: the proto applies.
        for apply in &table.applies {
            if let Some(text) = affect_string(locale, apply.apply_type, i64::from(apply.value)) {
                lines.push((text, change_color(i64::from(apply.value))));
            }
        }
    };
    let attributes = |lines: &mut Vec<Line>| {
        // `__AppendAttributeInformation`: the rolled bonuses; slots 5 and 6
        // are the rare ones (`SPECIAL_POSITIVE_COLOR2`).
        for (index, attribute) in stack.attributes.iter().enumerate() {
            let value = i64::from(attribute.sValue);
            if let Some(text) = affect_string(locale, attribute.bType, value) {
                let shade = if value < 0 {
                    color::NEGATIVE
                } else if index >= 5 {
                    color::SPECIAL_POSITIVE2
                } else {
                    color::SPECIAL_POSITIVE
                };
                lines.push((text, shade));
            }
        }
    };
    let wearable = |lines: &mut Vec<Line>| {
        // `AppendWearableInformation`.
        if let Some(text) = locale.get("TOOLTIP_ITEM_WEARABLE_JOB") {
            lines.push((text.to_owned(), color::NORMAL));
        }
        let jobs: Vec<&str> = ANTIFLAG_JOBS
            .iter()
            .filter(|(flag, _)| table.anti_flags & flag == 0)
            .filter_map(|(_, key)| locale.get(key))
            .collect();
        lines.push((jobs.join(" "), color::NORMAL));
        if table.anti_flags & ANTIFLAG_MALE != 0
            && let Some(text) = locale.get("FOR_FEMALE")
        {
            lines.push((text.to_owned(), color::NORMAL));
        }
        if table.anti_flags & ANTIFLAG_FEMALE != 0
            && let Some(text) = locale.get("FOR_MALE")
        {
            lines.push((text.to_owned(), color::NORMAL));
        }
    };
    let sockets = |lines: &mut Vec<Line>| {
        // `__AppendMetinSlotInfo`: a socket is 0 (none), 1/2 (an empty
        // silver/gold slot) or the vnum of the stone in it.
        for &socket in &stack.sockets {
            match socket {
                0 => {}
                SOCKET_SILVER | SOCKET_GOLD => {
                    if let Some(text) = locale.get("TOOLTIP_SOCKET_EMPTY") {
                        lines.push((format!("[{text}]"), color::NORMAL));
                    }
                }
                stone => {
                    let Some(stone) = catalog.get(stone as u32) else { continue };
                    lines.push((stone.table.locale_name_utf8(1252).into_owned(), color::NORMAL));
                    let apply = &stone.table.applies[0];
                    if let Some(text) = affect_string(locale, apply.apply_type, i64::from(apply.value)) {
                        lines.push((format!("  {text}"), color::POSITIVE));
                    }
                }
            }
        }
    };
    let values = table.values.map(i64::from);

    match table.item_type {
        ITEM_TYPE_WEAPON => {
            limits(&mut lines);
            let attack = |lines: &mut Vec<Line>| {
                // `__AppendAttackPowerInfo`.
                let (min, max, add) = (values[3], values[4], values[5]);
                let text = if max > min {
                    locale.format("TOOLTIP_ITEM_ATT_POWER", &[min + add, max + add])
                } else {
                    locale.format("TOOLTIP_ITEM_ATT_POWER_ONE_ARG", &[min + add])
                };
                lines.extend(text.map(|text| (text, color::POSITIVE)));
            };
            let magic = |lines: &mut Vec<Line>| {
                // `__AppendMagicAttackInfo`.
                let (min, max, add) = (values[1], values[2], values[5]);
                if min > 0 || max > 0 {
                    let text = if max > min {
                        locale.format("TOOLTIP_ITEM_MAGIC_ATT_POWER", &[min + add, max + add])
                    } else {
                        locale.format("TOOLTIP_ITEM_MAGIC_ATT_POWER_ONE_ARG", &[min + add])
                    };
                    lines.extend(text.map(|text| (text, color::POSITIVE)));
                }
            };
            // Fans show their magic attack first.
            if table.sub_type == WEAPON_FAN {
                magic(&mut lines);
                attack(&mut lines);
            } else {
                attack(&mut lines);
                magic(&mut lines);
            }
            affects(&mut lines);
            attributes(&mut lines);
            wearable(&mut lines);
            sockets(&mut lines);
        }
        ITEM_TYPE_ARMOR => {
            limits(&mut lines);
            // The defence shown is value[1] + 2 * value[5] (the refine bonus).
            let (grade, bonus) = (values[1], values[5] * 2);
            if grade > 0
                && let Some(text) = locale.format("TOOLTIP_ITEM_DEF_GRADE", &[grade + bonus])
            {
                lines.push((text, change_color(grade)));
            }
            // `__AppendMagicDefenceInfo`.
            if values[0] > 0
                && let Some(text) = locale.format("TOOLTIP_ITEM_MAGIC_DEF_POWER", &[values[0]])
            {
                lines.push((text, change_color(values[0])));
            }
            affects(&mut lines);
            attributes(&mut lines);
            wearable(&mut lines);
            sockets(&mut lines);
        }
        ITEM_TYPE_RING | ITEM_TYPE_BELT | ITEM_TYPE_COSTUME => {
            limits(&mut lines);
            affects(&mut lines);
            attributes(&mut lines);
            if table.item_type == ITEM_TYPE_COSTUME {
                wearable(&mut lines);
            }
        }
        ITEM_TYPE_METIN => {
            // `AppendMetinInformation`: the stone's own bonus.
            let apply = &table.applies[0];
            if let Some(text) = affect_string(locale, apply.apply_type, i64::from(apply.value)) {
                lines.push((text, change_color(i64::from(apply.value))));
            }
        }
        _ => {
            limits(&mut lines);
            affects(&mut lines);
            attributes(&mut lines);
        }
    }

    // `AppendSellingPrice`.
    if let Some(text) = locale.format_str("TOOLTIP_SELLPRICE", &[&money(i64::from(table.isell_item_price))])
        && table.isell_item_price > 0
    {
        lines.push((text, color::NORMAL));
    }
    lines.retain(|(text, _)| !text.trim().is_empty());
    lines
}

/// `localeInfo.NumberToMoneyString`: thousands separated by `.`, then "Yang".
fn money(value: i64) -> String {
    let digits = value.abs().to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(digit);
    }
    format!("{}{grouped} Yang", if value < 0 { "-" } else { "" })
}

pub struct ItemTooltipPlugin;
impl Plugin for ItemTooltipPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LocaleGame>().add_systems(Startup, load_locale);
    }
}

fn load_locale(mut locale: ResMut<LocaleGame>, packs: Option<Res<M2Packs>>) {
    let Some(packs) = packs else { return };
    match packs.packs.get(format!("{LOCALE_PATH}/locale_game.txt")) {
        Ok(data) => *locale = LocaleGame::parse(&data),
        Err(error) => warn!("locale_game.txt: {error:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printf_follows_python_percent() {
        assert_eq!(printf("Attack Value %d - %d", &[10, 15], &[]), "Attack Value 10 - 15");
        assert_eq!(printf("Max. HP +%d%%", &[12], &[]), "Max. HP +12%");
        assert_eq!(printf("Price: %s", &[], &["1.000 Yang"]), "Price: 1.000 Yang");
    }

    #[test]
    fn sa_entries_format_and_sna_entries_do_not() {
        let locale = LocaleGame::parse(b"TOOLTIP_MAX_HP\tMax. HP +%d\tSA\nTOOLTIP_APPLY_IMMUNE_STUN\tImmune to stun\tSNA\nTOOLTIP_ITEM_WEARABLE_JOB\t[ Wearable ]\n");
        assert_eq!(affect_string(&locale, 1, 500).as_deref(), Some("Max. HP +500"));
        assert_eq!(affect_string(&locale, 48, 1).as_deref(), Some("Immune to stun"));
        assert_eq!(affect_string(&locale, 1, 0), None);
        assert_eq!(locale.get("TOOLTIP_ITEM_WEARABLE_JOB"), Some("[ Wearable ]"));
    }

    #[test]
    fn money_groups_thousands() {
        assert_eq!(money(0), "0 Yang");
        assert_eq!(money(1_234_567), "1.234.567 Yang");
    }
}
