//! `uiAffectShower.AffectShower`: the row of active effects in the top-left
//! corner. Skill buffs and states come from the main character's affect flags
//! (`SetAffect`/`ResetAffect`, `__AppendAffect`); item and premium effects
//! from `GC_AFFECT_ADD`/`GC_AFFECT_REMOVE` (`BINARY_NEW_AddAffect`, types from
//! 500 up), with their time left in the tooltip.

use std::collections::HashMap;

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use m2_game::combat::SkillCatalog;
use m2_game::player::PlayerState;
use m2_game::social::SocialState;

use crate::core::{self, Art, Tooltip};
use crate::hud::GameUiRoot;
use crate::item_tooltip::LocaleGame;

/// `AffectShower.IMAGE_STEP` and `SetPosition(10, 10)`; icons are scaled 0.7.
const IMAGE_STEP: f32 = 25.0;
const ICON: f32 = 32.0 * 0.7;
/// `MALL_DESC_IDX_START`, `chr.NEW_AFFECT_MALL`.
const MALL_DESC_IDX_START: u32 = 1000;
const NEW_AFFECT_MALL: u32 = 510;
/// `INFINITE_AFFECT_DURATION`: longer durations show no clock.
const INFINITE_AFFECT_DURATION: i32 = 0x1FFF_FFFF;

/// `AFFECT_DATA_DICT`: (`localeInfo` key, icon). Affect flags are below 64,
/// `NEW_AFFECT_*` types 500 and up, mall points `1000 + POINT_*`.
fn affect_data(affect: u32) -> Option<(&'static str, &'static str)> {
    Some(match affect {
        3 => ("SKILL_TOXICDIE", "d:/ymir work/ui/skill/common/affect/poison.sub"),
        4 => ("SKILL_SLOW", "d:/ymir work/ui/skill/common/affect/slow.sub"),
        5 => ("SKILL_STUN", "d:/ymir work/ui/skill/common/affect/stun.sub"),
        12 => ("SKILL_INC_ATKSPD", "d:/ymir work/ui/skill/common/affect/Increase_Attack_Speed.sub"),
        11 => ("SKILL_INC_MOVSPD", "d:/ymir work/ui/skill/common/affect/Increase_Move_Speed.sub"),
        13 => ("SKILL_FISHMIND", "d:/ymir work/ui/skill/common/affect/fishmind.sub"),
        14 => ("SKILL_JEONGWI", "d:/ymir work/ui/skill/warrior/jeongwi_03.sub"),
        15 => ("SKILL_GEOMGYEONG", "d:/ymir work/ui/skill/warrior/geomgyeong_03.sub"),
        16 => ("SKILL_CHEONGEUN", "d:/ymir work/ui/skill/warrior/cheongeun_03.sub"),
        17 => ("SKILL_GYEONGGONG", "d:/ymir work/ui/skill/assassin/gyeonggong_03.sub"),
        18 => ("SKILL_EUNHYEONG", "d:/ymir work/ui/skill/assassin/eunhyeong_03.sub"),
        19 => ("SKILL_GWIGEOM", "d:/ymir work/ui/skill/sura/gwigeom_03.sub"),
        20 => ("SKILL_GONGPO", "d:/ymir work/ui/skill/sura/gongpo_03.sub"),
        21 => ("SKILL_JUMAGAP", "d:/ymir work/ui/skill/sura/jumagap_03.sub"),
        22 => ("SKILL_HOSIN", "d:/ymir work/ui/skill/shaman/hosin_03.sub"),
        23 => ("SKILL_BOHO", "d:/ymir work/ui/skill/shaman/boho_03.sub"),
        24 => ("SKILL_KWAESOK", "d:/ymir work/ui/skill/shaman/kwaesok_03.sub"),
        25 => ("SKILL_HEUKSIN", "d:/ymir work/ui/skill/sura/heuksin_03.sub"),
        26 => ("SKILL_MUYEONG", "d:/ymir work/ui/skill/sura/muyeong_03.sub"),
        28 => ("SKILL_FIRE", "d:/ymir work/ui/skill/sura/hwayeom_03.sub"),
        29 => ("SKILL_GICHEON", "d:/ymir work/ui/skill/shaman/gicheon_03.sub"),
        30 => ("SKILL_JEUNGRYEOK", "d:/ymir work/ui/skill/shaman/jeungryeok_03.sub"),
        32 => ("SKILL_PABEOP", "d:/ymir work/ui/skill/sura/pabeop_03.sub"),
        33 => ("SKILL_CHEONGEUN", "d:/ymir work/ui/skill/warrior/cheongeun_03.sub"),
        38 => ("SKILL_POWERFUL_STRIKE", "d:/ymir work/ui/skill/common/affect/powerfulstrike.sub"),
        500 => ("TOOLTIP_MALL_EXPBONUS_STATIC", "d:/ymir work/ui/skill/common/affect/exp_bonus.sub"),
        501 => ("TOOLTIP_MALL_ITEMBONUS_STATIC", "d:/ymir work/ui/skill/common/affect/item_bonus.sub"),
        502 => ("TOOLTIP_MALL_SAFEBOX", "d:/ymir work/ui/skill/common/affect/safebox.sub"),
        503 => ("TOOLTIP_MALL_AUTOLOOT", "d:/ymir work/ui/skill/common/affect/autoloot.sub"),
        504 => ("TOOLTIP_MALL_FISH_MIND", "d:/ymir work/ui/skill/common/affect/fishmind.sub"),
        505 => ("TOOLTIP_MALL_MARRIAGE_FAST", "d:/ymir work/ui/skill/common/affect/marriage_fast.sub"),
        506 => ("TOOLTIP_MALL_GOLDBONUS_STATIC", "d:/ymir work/ui/skill/common/affect/gold_bonus.sub"),
        511 => ("TOOLTIP_APPLY_NO_DEATH_PENALTY", "d:/ymir work/ui/skill/common/affect/gold_premium.sub"),
        512 => ("TOOLTIP_APPLY_SKILL_BOOK_BONUS", "d:/ymir work/ui/skill/common/affect/gold_premium.sub"),
        513 => ("TOOLTIP_APPLY_SKILL_BOOK_NO_DELAY", "d:/ymir work/ui/skill/common/affect/gold_premium.sub"),
        534 => ("TOOLTIP_AUTO_POTION_REST", "d:/ymir work/ui/pattern/auto_hpgauge/05.dds"),
        535 => ("TOOLTIP_AUTO_POTION_REST", "d:/ymir work/ui/pattern/auto_spgauge/05.dds"),
        1114 => ("TOOLTIP_MALL_ATTBONUS_STATIC", "d:/ymir work/ui/skill/common/affect/att_bonus.sub"),
        1115 => ("TOOLTIP_MALL_DEFBONUS_STATIC", "d:/ymir work/ui/skill/common/affect/def_bonus.sub"),
        1116 => ("TOOLTIP_MALL_EXPBONUS", "d:/ymir work/ui/skill/common/affect/exp_bonus.sub"),
        1117 => ("TOOLTIP_MALL_ITEMBONUS", "d:/ymir work/ui/skill/common/affect/item_bonus.sub"),
        1118 => ("TOOLTIP_MALL_GOLDBONUS", "d:/ymir work/ui/skill/common/affect/gold_bonus.sub"),
        1040 => ("TOOLTIP_APPLY_CRITICAL_PCT", "d:/ymir work/ui/skill/common/affect/critical.sub"),
        1041 => ("TOOLTIP_APPLY_PENETRATE_PCT", "d:/ymir work/ui/skill/common/affect/gold_premium.sub"),
        1119 => ("TOOLTIP_MAX_HP_PCT", "d:/ymir work/ui/skill/common/affect/gold_premium.sub"),
        1120 => ("TOOLTIP_MAX_SP_PCT", "d:/ymir work/ui/skill/common/affect/gold_premium.sub"),
        1125 => ("TOOLTIP_MALL_EXPBONUS_P_STATIC", "d:/ymir work/ui/skill/common/affect/EXP_Bonus_p_on.sub"),
        1126 => ("TOOLTIP_MALL_ITEMBONUS_P_STATIC", "d:/ymir work/ui/skill/common/affect/Item_Bonus_p_on.sub"),
        _ => return None,
    })
}

/// `CPythonPlayer::AffectIndexToSkillIndex` (`m_kMap_dwAffectIndexToSkillIndex`):
/// a skill buff is named after its skill.
fn affect_skill(affect: u32) -> Option<u32> {
    Some(match affect {
        14 => 3,
        15 => 4,
        16 | 33 => 19,
        17 => 49,
        18 => 34,
        19 => 63,
        20 => 64,
        21 => 65,
        22 => 94,
        23 => 95,
        24 => 110,
        25 => 79,
        26 => 78,
        29 => 96,
        30 => 111,
        32 => 66,
        _ => return None,
    })
}

/// `localeInfo.SecondToDHM`.
fn second_to_dhm(locale: &LocaleGame, time: i64) -> String {
    let word = |key: &str| locale.get(key).unwrap_or(key).trim().to_owned();
    if time < 60 {
        return format!("0{}", word("MINUTE"));
    }
    let minute = (time / 60) % 60;
    let hour = (time / 3600) % 24;
    let day = time / 86_400;
    let mut text = String::new();
    if day > 0 {
        text.push_str(&format!("{day}{} ", word("DAY")));
    }
    if hour > 0 {
        text.push_str(&format!("{hour}{} ", word("HOUR")));
    }
    if minute > 0 {
        text.push_str(&format!("{minute}{}", word("MINUTE")));
    }
    text.trim_end().to_owned()
}

/// One icon of the row.
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    key: u32,
    icon: &'static str,
    description: String,
    /// `app.GetGlobalTimeStamp() + duration` in `Time::elapsed` seconds.
    ends: Option<f32>,
}

#[derive(Component)]
struct AffectRow;
#[derive(Component)]
struct AffectIcon(u32);

pub struct AffectShowerPlugin;
impl Plugin for AffectShowerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show.in_set(crate::core::UiSet::Draw));
    }
}

/// The row's container, spawned with the rest of the game UI.
pub fn build(commands: &mut Commands) {
    commands.spawn((
        GameUiRoot,
        AffectRow,
        Node { position_type: PositionType::Absolute, left: Val::Px(10.), top: Val::Px(10.), ..default() },
        Pickable::IGNORE,
    ));
}

#[allow(clippy::too_many_arguments)]
fn show(
    mut commands: Commands,
    assets: Res<AssetServer>,
    time: Res<Time>,
    player: Res<PlayerState>,
    social: Res<SocialState>,
    locale: Res<LocaleGame>,
    skills: Res<SkillCatalog>,
    rows: Query<Entity, With<AffectRow>>,
    mut icons: Query<(Entity, &AffectIcon, &mut Tooltip)>,
    mut started: Local<HashMap<(u32, u8), (f32, i32)>>,
    mut last_update: Local<f32>,
) {
    let Ok(row) = rows.single() else { return };
    let now = time.elapsed_secs();
    // `__AppendAffect` for each set flag.
    let mut entries: Vec<Entry> = (0..64u32)
        .filter(|affect| m2_game::combat::has_affect(player.affect_flags, *affect))
        .filter_map(|affect| {
            let (key, icon) = affect_data(affect)?;
            let name = affect_skill(affect)
                .and_then(|skill| skills.get(skill))
                .map(|data| data.name_utf8(1252).into_owned())
                .or_else(|| locale.get(key).map(str::to_owned))
                .unwrap_or_default();
            Some(Entry { key: affect, icon, description: name, ends: None })
        })
        .collect();
    // `BINARY_NEW_AddAffect`: types of 500 and up, mall points by index.
    started.retain(|key, _| social.affects.contains_key(key));
    for (&(kind, apply_on), element) in &social.affects {
        if kind < 500 {
            continue;
        }
        let affect = if kind == NEW_AFFECT_MALL { MALL_DESC_IDX_START + u32::from(apply_on) } else { kind };
        if entries.iter().any(|entry| entry.key == affect) {
            continue;
        }
        let Some((key, icon)) = affect_data(affect) else { continue };
        let start = started.entry((kind, apply_on)).or_insert((now, element.lDuration));
        if start.1 != element.lDuration {
            *start = (now, element.lDuration);
        }
        let no_clock = matches!(affect, 511 | 512 | 513 | 534 | 535 | 516 | 517);
        let duration = if no_clock { 0 } else { element.lDuration };
        let value = if matches!(apply_on, 117 | 118) { 1 + element.lApplyValue / 100 } else { element.lApplyValue };
        let description = locale.format(key, &[i64::from(value)]).unwrap_or_default();
        let ends = (duration > 0 && duration <= INFINITE_AFFECT_DURATION).then_some(start.0 + duration as f32);
        entries.push(Entry { key: affect, icon, description, ends });
    }

    // `__ArrangeImageList`: rebuild when the set changes.
    let current: Vec<u32> = icons.iter().map(|(_, icon, _)| icon.0).collect();
    let wanted: Vec<u32> = entries.iter().map(|entry| entry.key).collect();
    if current != wanted {
        for (entity, ..) in &icons {
            commands.entity(entity).despawn();
        }
        for (index, entry) in entries.iter().enumerate() {
            let icon = commands
                .spawn((
                    AffectIcon(entry.key),
                    Art::load(&assets, entry.icon),
                    ImageNode::default(),
                    Tooltip(entry.description.clone()),
                    Hovered::default(),
                    core::rect(index as f32 * IMAGE_STEP, 0., ICON, ICON),
                ))
                .id();
            commands.entity(row).add_child(icon);
        }
        *last_update = now;
        return;
    }
    // `OnUpdate`: the time left, twice a second.
    if now - *last_update < 0.5 {
        return;
    }
    *last_update = now;
    for (_, icon, mut tooltip) in &mut icons {
        let Some(entry) = entries.iter().find(|entry| entry.key == icon.0) else { continue };
        let text = match entry.ends {
            Some(ends) => format!(
                "{} ({} : {})",
                entry.description,
                locale.get("LEFT_TIME").unwrap_or("Time Remaining").trim(),
                second_to_dhm(&locale, (ends - now).max(0.0) as i64)
            ),
            None => entry.description.clone(),
        };
        if tooltip.0 != text {
            tooltip.0 = text;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_left_reads_like_second_to_dhm() {
        let locale = LocaleGame::parse(b"DAY\tDays\t\nHOUR\tHours\t\nMINUTE\tMinutes\t\n");
        assert_eq!(second_to_dhm(&locale, 30), "0Minutes");
        assert_eq!(second_to_dhm(&locale, 3 * 3600 + 5 * 60), "3Hours 5Minutes");
        assert_eq!(second_to_dhm(&locale, 86_400 + 60), "1Days 1Minutes");
    }

    #[test]
    fn skill_buffs_are_named_after_their_skill_and_mall_points_offset() {
        assert_eq!(affect_skill(18), Some(34));
        assert_eq!(affect_data(501).map(|(key, _)| key), Some("TOOLTIP_MALL_ITEMBONUS_STATIC"));
        assert_eq!(affect_data(MALL_DESC_IDX_START + 116).map(|(key, _)| key), Some("TOOLTIP_MALL_EXPBONUS"));
        assert!(affect_data(1).is_none());
    }
}
