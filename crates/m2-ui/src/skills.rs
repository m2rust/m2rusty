//! The SKILL page of the character window (`Skill_Page` in
//! `uiscript/characterwindow.py`) and the skill quick-slot overlay.
//!
//! The page shows the active and support skill grids of the character's job
//! and skill group. The visible slots are the ones `RegisterSkill` fills in
//! `root/playersettingmodule.py` (`SKILL_INDEX_DICT`). Levels arrive by skill
//! vnum (`CPythonPlayer::SetSkillLevel_`), so a slot's vnum is looked up in
//! the table every frame and its level in [`SkillState`].
//!
//! Clicking a learned skill sends `TPacketCGUseSkill`
//! (`CPythonNetworkStream::SendUseSkillPacket`, `__UseSkill`); Ctrl+click binds
//! it to the first empty global quick slot (`uicharacter.SelectSkill`'s
//! `RequestAddToEmptyLocalQuickSlot`). The task bar draws the bound skill over
//! `inventory`'s quick slot with a child of its own, never touching the item
//! icon.

use std::collections::HashMap;

use bevy::input_focus::InputFocus;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use bevy::window::PrimaryWindow;

use m2_assets::{M2Packs, paths, subimage::SubImage};
use m2_formats::proto::{SkillData, SkillTables, SkillType};
use m2_game::ingame::LOCALE_PATH;
use m2_game::net::{Net, NetMessage};
use m2_game::player::{PlayerAction, PlayerState};
use m2_net::phase::NetEvent;
use m2_net::wire::{GcPacket, TQuickSlot};

use crate::character::{CharacterTab, PageRoot};
use crate::core::{self, Art, Click, Consumed, Control, Selected, Surface, UiSet};
use crate::hud::{ChatState, GameUiRoot};
use crate::inventory::{InventoryUi, Slot};
use crate::quickslots::{self, QuickSlotUse};

/// `uicharacter.ACTIVE_PAGE_SLOT_COUNT`.
pub const ACTIVE_PAGE_SLOTS: u32 = 8;
/// `uicharacter.SUPPORT_PAGE_SLOT_COUNT`.
pub const SUPPORT_PAGE_SLOTS: u32 = 12;
/// `skill.SKILL_GRADE_STEP_COUNT`: grade widgets are stacked 20 slots apart.
pub const SKILL_GRADE_STEP: u32 = 20;
/// `skill.SKILL_GRADE_COUNT`: grade 0/1/2, plus `3` for a mastered skill.
pub const SKILL_GRADE_COUNT: u8 = 3;
/// `c_iSkillIndex_Riding` (`PythonPlayerModule.cpp`), whose icon is stored
/// under `common/support` even though its type is `SUPPORT`.
pub const RIDING_SKILL: u32 = 130;

/// `POINT_SUB_SKILL` / `POINT_SKILL` / `POINT_HORSE_SKILL` (`Packet.h`).
const POINT_SUPPORT: usize = 27;
const POINT_ACTIVE: usize = 28;
const POINT_HORSE: usize = 113;

// `SKILL_INDEX_DICT` with `NEW_678TH_SKILL_ENABLE == localeInfo.IsYMIR()`.
// The extra YMIR entries are harmless when the server does not know them: an
// unlearned skill has level 0 and its slot is not drawn.
const WARRIOR_GROUP1: [u32; 14] = [1, 2, 3, 4, 5, 6, 0, 0, 137, 0, 138, 0, 139, 0];
const WARRIOR_GROUP2: [u32; 14] = [16, 17, 18, 19, 20, 21, 0, 0, 137, 0, 138, 0, 139, 0];
const ASSASSIN_GROUP1: [u32; 15] = [31, 32, 33, 34, 35, 36, 0, 0, 137, 0, 138, 0, 139, 0, 140];
const ASSASSIN_GROUP2: [u32; 15] = [46, 47, 48, 49, 50, 51, 0, 0, 137, 0, 138, 0, 139, 0, 140];
const SURA_GROUP1: [u32; 14] = [61, 62, 63, 64, 65, 66, 0, 0, 137, 0, 138, 0, 139, 0];
const SURA_GROUP2: [u32; 14] = [76, 77, 78, 79, 80, 81, 0, 0, 137, 0, 138, 0, 139, 0];
const SHAMAN_GROUP1: [u32; 14] = [91, 92, 93, 94, 95, 96, 0, 0, 137, 0, 138, 0, 139, 0];
const SHAMAN_GROUP2: [u32; 14] = [106, 107, 108, 109, 110, 111, 0, 0, 137, 0, 138, 0, 139, 0];
/// The `SUPPORT` row of `SKILL_INDEX_DICT`, slots 101..112.
const SUPPORT: [u32; 12] = [122, 123, 121, 124, 125, 129, 0, 0, 130, 131, 141, 142];

/// `CPythonSkill::m_SkillDataMap`, loaded in the background at startup.
pub use m2_game::combat::{SkillCatalog, SkillState};

#[derive(Resource)]
struct SkillCatalogTask(Task<Result<SkillTables, String>>);

/// The player's skill levels live in `m2_game::combat` (ported from the
/// same packets); the page only reads them.
/// The page's radios: which skill group is shown and whether the Riding page
/// is selected.
#[derive(Resource, Default)]
pub struct SkillUi {
    /// `0` follows [`PlayerState::skill_group`]; `1`/`2` is a group button.
    pub group: u8,
    pub riding: bool,
}

#[derive(Component, Clone, Copy)]
struct SkillCell {
    /// Source slot index (`1..14` active, `101..112` support).
    slot: u32,
    /// Grade widget (0/1/2) of the active grid; always `0` in the support grid.
    grade: u8,
}

#[derive(Component)]
struct SkillIcon {
    slot: u32,
    grade: u8,
    vnum: u32,
    path: Option<String>,
}

#[derive(Component)]
struct SkillCount {
    slot: u32,
    grade: u8,
}

#[derive(Component)]
struct SkillPlus {
    slot: u32,
}

#[derive(Component, Clone, Copy)]
struct GroupButton(u8);

/// A sub-tab: `true` for Riding.
#[derive(Component, Clone, Copy)]
struct SkillTab(bool);

#[derive(Component, Clone, Copy)]
struct SkillPointValue(bool);

#[derive(Component)]
struct SkillBody;

#[derive(Component)]
struct RidingBody;

#[derive(Component)]
struct SkillTip;

#[derive(Component)]
struct SkillTipText;

/// The skill icon drawn over a task bar quick slot (its own child, so the
/// inventory's item `Icon` is untouched).
#[derive(Component)]
struct QuickSkill {
    slot: Entity,
    vnum: u32,
    grade: u8,
    path: Option<String>,
}

pub struct SkillsPlugin;
impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillCatalog>()
            .init_resource::<SkillState>()
            .init_resource::<SkillUi>()
            .add_systems(Startup, load_catalog)
            .add_systems(Update, (poll_catalog, receive_levels).chain().in_set(UiSet::State))
            .add_systems(Update, actions.in_set(UiSet::Actions))
            .add_systems(Update, (refresh, tooltip, quick_overlay).chain().in_set(UiSet::Draw));
    }
}

/// `CPythonSkill::RegisterSkillDesc` + `RegisterSkillTable`, off the main
/// thread (the UI shows an empty page until [`SkillCatalog`] is filled).
fn load_catalog(mut commands: Commands, packs: Option<Res<M2Packs>>) {
    let Some(packs) = packs else { return };
    let packs = packs.packs.clone();
    commands.insert_resource(SkillCatalogTask(IoTaskPool::get().spawn(async move {
        let mut tables = SkillTables::default();
        let desc = packs
            .get(format!("{LOCALE_PATH}/skilldesc.txt"))
            .map_err(|e| format!("skilldesc.txt: {e:?}"))?;
        tables
            .register_skill_desc(&desc)
            .map_err(|e| format!("skilldesc.txt: {e}"))?;
        let table = packs
            .get(format!("{LOCALE_PATH}/skilltable.txt"))
            .map_err(|e| format!("skilltable.txt: {e:?}"))?;
        // The English locale is not the YMIR/CHEONMA one.
        tables.register_skill_table(&table, false);
        Ok(tables)
    })));
}

fn poll_catalog(
    mut commands: Commands,
    task: Option<ResMut<SkillCatalogTask>>,
    mut catalog: ResMut<SkillCatalog>,
) {
    let Some(mut task) = task else { return };
    if let Some(result) = check_ready(&mut task.0) {
        match result {
            Ok(tables) => catalog.0 = tables,
            Err(error) => warn!("{error}"),
        }
        commands.remove_resource::<SkillCatalogTask>();
    }
}

/// `RecvSkillLevel`/`RecvSkillLevelNew` (`PythonNetworkStreamPhaseGame.cpp`):
/// reset with the player and keep the levels indexed by skill vnum.
fn receive_levels(
    mut messages: MessageReader<NetMessage>,
    mut skills: ResMut<SkillState>,
    mut ui: ResMut<SkillUi>,
) {
    for message in messages.read() {
        match message {
            // `NetMessage::Disconnected` resets `PlayerState` as well.
            NetMessage::Disconnected { .. } => {
                skills.reset();
                *ui = SkillUi::default();
            }
            NetMessage::Game(NetEvent::MainCharacter(_)) => {
                skills.reset();
                *ui = SkillUi::default();
            }
            NetMessage::Game(NetEvent::Packet(GcPacket::SkillLevel(packet))) => {
                skills.levels = packet.abSkillLevels;
                skills.grades = [0; 255];
            }
            NetMessage::Game(NetEvent::Packet(GcPacket::SkillLevelNew(packet))) => {
                for (vnum, skill) in packet.skills.iter().enumerate() {
                    skills.levels[vnum] = skill.bLevel;
                    skills.grades[vnum] = skill.bMasterType;
                }
            }
            _ => {}
        }
    }
}

/// `playersettingmodule.RegisterSkill`'s `job = chr.RaceToJob(race)`.
fn job(race: u16) -> u16 {
    race % 4
}

/// The active skill list of a job and group (`SKILL_INDEX_DICT[job][group]`).
fn active_table(race: u16, group: u8) -> &'static [u32] {
    match (job(race), group == 2) {
        (0, false) => &WARRIOR_GROUP1,
        (0, true) => &WARRIOR_GROUP2,
        (1, false) => &ASSASSIN_GROUP1,
        (1, true) => &ASSASSIN_GROUP2,
        (2, false) => &SURA_GROUP1,
        (2, true) => &SURA_GROUP2,
        (3, false) => &SHAMAN_GROUP1,
        _ => &SHAMAN_GROUP2,
    }
}

/// The skill vnum of a source slot index. Active slots are `1..` (unused
/// entries are 0); support slots are `101..112`. `RecvSkillLevelNew` moves the
/// new 6th/7th skills into slots 7 and 8, so those are read from the levels.
pub fn slot_vnum(race: u16, group: u8, slot: u32, state: &SkillState) -> u32 {
    if slot >= 101 {
        return SUPPORT.get((slot - 101) as usize).copied().unwrap_or(0);
    }
    if slot == 0 {
        return 0;
    }
    if slot == 7 || slot == 8 {
        let (first, last) = if slot == 7 { (112, 115) } else { (116, 119) };
        // The C++ loop keeps the last learned vnum.
        if let Some(vnum) = (first..=last)
            .rev()
            .find(|vnum| state.levels.get(*vnum as usize).is_some_and(|level| *level != 0))
        {
            return vnum;
        }
    }
    active_table(race, group)
        .get((slot - 1) as usize)
        .copied()
        .unwrap_or(0)
}

/// `CPythonPlayer::GetSkillSlotIndex`: the reverse table, for tests and the
/// quick-slot binding (which stores the slot index, not the vnum).
pub fn slot_of_vnum(race: u16, group: u8, vnum: u32) -> Option<u32> {
    (1..=14)
        .find(|slot| active_table(race, group).get((slot - 1) as usize) == Some(&vnum))
        .or_else(|| {
            SUPPORT
                .iter()
                .position(|entry| *entry == vnum)
                .map(|index| 101 + index as u32)
        })
}

/// `CPythonSkill::RegisterSkillDesc`'s icon path rules: `g_strImagePath`,
/// `m_PathNameMap`, `__RegisterGradeIconImage` and
/// `__RegisterNormalIconImage`.
pub fn skill_icon_path(data: &SkillData, grade: u8) -> Option<String> {
    let icon = String::from_utf8_lossy(&data.icon);
    if icon.is_empty() {
        return None;
    }
    // `c_iSkillIndex_Riding` is a support skill with three grade icons.
    if data.skill_index == RIDING_SKILL {
        return Some(format!(
            "d:/ymir work/ui/skill/common/support/{icon}_{:02}.sub",
            grade.min(SKILL_GRADE_COUNT - 1) + 1
        ));
    }
    let path = match String::from_utf8_lossy(&data.job).to_ascii_uppercase().as_str() {
        "WARRIOR" => "warrior",
        "ASSASSIN" => "assassin",
        "SURA" => "sura",
        "SHAMAN" => "shaman",
        "PASSIVE" => "passive",
        "SUPPORT" => "common/support",
        "GUILD" => "common/guild",
        "HORSE" => "common/horse",
        _ => return None,
    };
    match data.skill_type {
        ty if ty == SkillType::Active as u8 => Some(format!(
            "d:/ymir work/ui/skill/{path}/{icon}_{:02}.sub",
            grade.min(SKILL_GRADE_COUNT - 1) + 1
        )),
        ty if matches!(
            ty,
            t if t == SkillType::Support as u8
                || t == SkillType::Guild as u8
                || t == SkillType::Horse as u8
        ) =>
        {
            Some(format!("d:/ymir work/ui/skill/{path}/{icon}.sub"))
        }
        _ => None,
    }
}

/// The icon path of a vnum, if the catalog knows the skill.
fn icon_path(catalog: &SkillCatalog, vnum: u32, grade: u8) -> Option<String> {
    skill_icon_path(catalog.get(vnum)?, grade)
}

/// `CPythonPlayer::RequestAddToEmptyLocalQuickSlot`: the first global slot
/// whose `Type` is 0.
fn empty_quick_slot(player: &PlayerState) -> Option<u8> {
    (0..32u8).find(|slot| player.quickslots[usize::from(*slot)].Type == 0)
}

/// The group shown by the page. `SkillUi::group` overrides the character's
/// group while a group button is pressed.
fn effective_group(ui: &SkillUi, player: &PlayerState) -> u8 {
    match ui.group {
        1 | 2 => ui.group,
        _ => player.skill_group.clamp(1, 2),
    }
}

/// The vnum and level of a source slot, if it holds a learned skill.
fn resolve(player: &PlayerState, state: &SkillState, group: u8, slot: u32) -> Option<(u32, u8)> {
    let vnum = slot_vnum(player.race, group, slot, state);
    (vnum != 0).then(|| (vnum, state.level(vnum)))
}

/// The skill page and the task bar hand the vnum to `m2-game`, which runs
/// `CPythonPlayer::__UseSkill` (target choice, checks, motion and packet).
fn send_use_skill(requests: &mut MessageWriter<m2_game::combat::UseSkillRequest>, vnum: u32) {
    requests.write(m2_game::combat::UseSkillRequest { vnum });
}

pub fn build(commands: &mut Commands, assets: &AssetServer, parent: Entity) -> Entity {
    let page = commands
        .spawn((PageRoot(CharacterTab::Skill), core::rect(0., 24., 250., 304.)))
        .id();
    commands.entity(parent).add_child(page);

    // Sub-tabs "Skill"/"Riding" under the window's title bar.
    for (index, (riding, caption)) in [(false, "Skill"), (true, "Riding")].into_iter().enumerate() {
        let button = core::button(
            commands,
            assets,
            page,
            core::rect(16. + index as f32 * 58., 6., 54., 16.),
            "d:/ymir work/ui/game/windows/tab_button_middle",
            caption,
        );
        commands.entity(button).insert(SkillTab(riding));
    }

    let body = commands
        .spawn((SkillBody, core::rect(0., 0., 250., 304.), Pickable::IGNORE))
        .id();
    commands.entity(page).add_child(body);

    // `Skill_Active_Title_Bar`: group radios, the "Active" caption and the
    // active skill point value.
    bar(commands, assets, body, 12., 24., 223.);
    header_text(commands, body, "Active", 17., 26.);
    for (x, group) in [(66., 1u8), (110., 2u8)] {
        let button = core::button(
            commands,
            assets,
            body,
            core::rect(x, 24., 42., 18.),
            "d:/ymir work/ui/game/windows/skill_tab_button",
            &format!("Group{group}"),
        );
        commands.entity(button).insert(GroupButton(group));
    }
    core::picture(
        commands,
        assets,
        body,
        "locale/en/ui/windows/label_uppt.sub",
        core::rect(158., 27., 52., 14.),
    );
    let active_points = core::text(commands, body, "", core::rect(184., 28., 22., 13.));
    commands
        .entity(active_points)
        .insert((SkillPointValue(false), TextLayout::justify(Justify::Right)));

    // `Skill_Board` and `Skill_Active_Slot`: 6 columns x 4 rows, the three
    // columns of each half being the three grade icons of one skill.
    core::picture(
        commands,
        assets,
        body,
        "d:/ymir work/ui/game/windows/skill_board.sub",
        core::rect(13., 44., 223., 223.),
    );
    for row in 0..4u32 {
        for half in 0..2u32 {
            let slot = row * 2 + half + 1;
            for grade in 0..SKILL_GRADE_COUNT {
                let column = half * 3 + u32::from(grade);
                let cell = SkillCell { slot, grade };
                skill_cell(
                    commands,
                    assets,
                    body,
                    cell,
                    17. + column as f32 * 37.,
                    48. + row as f32 * 36.,
                );
            }
        }
    }

    // `Skill_ETC_Title_Bar` and `Skill_ETC_Slot` (support/guild/horse).
    bar(commands, assets, body, 12., 206., 223.);
    header_text(commands, body, "Support", 17., 208.);
    core::picture(
        commands,
        assets,
        body,
        "locale/en/ui/windows/label_uppt.sub",
        core::rect(158., 209., 52., 14.),
    );
    let support_points = core::text(commands, body, "", core::rect(184., 210., 22., 13.));
    commands
        .entity(support_points)
        .insert((SkillPointValue(true), TextLayout::justify(Justify::Right)));
    for index in 0..SUPPORT_PAGE_SLOTS {
        skill_cell(
            commands,
            assets,
            body,
            SkillCell { slot: 101 + index, grade: 0 },
            18. + (index % 6) as f32 * 37.,
            227. + (index / 6) as f32 * 36.,
        );
    }

    // The original page has a horse-skill group; the port draws an empty
    // Riding page (the two group buttons are for the active skill groups).
    let riding = commands
        .spawn((
            RidingBody,
            Pickable::IGNORE,
            Node { display: Display::None, ..core::rect(0., 0., 250., 304.) },
        ))
        .id();
    commands.entity(page).add_child(riding);
    bar(commands, assets, riding, 12., 24., 223.);
    header_text(commands, riding, "Riding", 17., 26.);
    let hint = core::text(
        commands,
        riding,
        "Riding skills are not available.",
        core::rect(16., 48., 218., 30.),
    );
    commands.entity(hint).insert(TextColor(Color::srgb(0.85, 0.82, 0.7)));

    // The skill tooltip. It lives outside the page so the character window
    // cannot clip it.
    let tip = commands
        .spawn((
            GameUiRoot,
            SkillTip,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Px(0.),
                top: Val::Px(0.),
                width: Val::Px(250.),
                padding: UiRect::all(Val::Px(8.)),
                border: UiRect::all(Val::Px(1.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.9)),
            BorderColor::all(Color::srgb(0.7, 0.65, 0.5)),
            GlobalZIndex(100_001),
            Pickable::IGNORE,
        ))
        .id();
    let label = core::text(commands, tip, "", Node { width: Val::Percent(100.), ..default() });
    commands.entity(label).insert(SkillTipText);

    page
}

/// `ui.HorizontalBar`: a 32-px left cap, a tiled centre and a 32-px right cap.
fn bar(commands: &mut Commands, assets: &AssetServer, parent: Entity, x: f32, y: f32, width: f32) {
    core::picture(
        commands,
        assets,
        parent,
        "d:/ymir work/ui/pattern/horizontalbar_left.tga",
        core::rect(x, y, 32., 17.),
    );
    let center = core::picture(
        commands,
        assets,
        parent,
        "d:/ymir work/ui/pattern/horizontalbar_center.tga",
        core::rect(x + 32., y, (width - 64.).max(0.), 17.),
    );
    commands.entity(center).insert(ImageNode {
        image_mode: bevy::ui::widget::NodeImageMode::Tiled {
            tile_x: true,
            tile_y: false,
            stretch_value: 1.,
        },
        ..default()
    });
    core::picture(
        commands,
        assets,
        parent,
        "d:/ymir work/ui/pattern/horizontalbar_right.tga",
        core::rect(x + width - 32., y, 32., 17.),
    );
}

fn header_text(commands: &mut Commands, parent: Entity, caption: &str, x: f32, y: f32) {
    let label = core::text(commands, parent, caption, core::rect(x, y, 120., 14.));
    commands.entity(label).insert(TextColor(Color::srgb(0.85, 0.78, 0.6)));
}

/// A 32-px skill cell: `Slot_Base.sub`, the grade icon, the level count and
/// the `btn_plus` level-up button of `uicharacter.OnPressedSlotButton`.
fn skill_cell(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    cell: SkillCell,
    x: f32,
    y: f32,
) -> Entity {
    let entity = commands
        .spawn((
            cell,
            Control,
            Surface,
            Hovered::default(),
            core::rect(x, y, 32., 32.),
            BackgroundColor(Color::NONE),
            ZIndex(1),
        ))
        .id();
    commands.entity(parent).add_child(entity);
    core::picture(
        commands,
        assets,
        entity,
        "d:/ymir work/ui/public/slot_base.sub",
        core::rect(0., 0., 32., 32.),
    );
    let icon = commands
        .spawn((
            SkillIcon { slot: cell.slot, grade: cell.grade, vnum: 0, path: None },
            Node { display: Display::None, ..core::rect(0., 0., 32., 32.) },
            ImageNode::default(),
            ZIndex(2),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(entity).add_child(icon);

    let count = core::text(commands, entity, "", core::rect(0., 17., 30., 15.));
    commands.entity(count).insert((
        SkillCount { slot: cell.slot, grade: cell.grade },
        ZIndex(3),
        TextLayout::justify(Justify::Right),
    ));

    let art = [
        Art::load(assets, "d:/ymir work/ui/game/windows/btn_plus_up.sub"),
        Art::load(assets, "d:/ymir work/ui/game/windows/btn_plus_over.sub"),
        Art::load(assets, "d:/ymir work/ui/game/windows/btn_plus_down.sub"),
    ];
    let plus = core::button_with(commands, entity, core::rect(17., 0., 14., 14.), art, "");
    commands.entity(plus).insert((SkillPlus { slot: cell.slot }, ZIndex(4)));
    entity
}

#[allow(clippy::too_many_arguments)]
fn actions(
    mut clicks: MessageReader<Click>,
    cells: Query<&SkillCell>,
    pluses: Query<&SkillPlus>,
    groups: Query<&GroupButton>,
    tabs: Query<&SkillTab>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    chat: Res<ChatState>,
    mut ui: ResMut<SkillUi>,
    player: Res<PlayerState>,
    state: Res<SkillState>,
    mut net: ResMut<Net>,
    mut quick_uses: MessageReader<QuickSlotUse>,
    mut requests: MessageWriter<PlayerAction>,
    mut use_skill: MessageWriter<m2_game::combat::UseSkillRequest>,
    mut consumed: ResMut<Consumed>,
) {
    let typing = focus.get().is_some() || chat.typing;
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        if let Ok(tab) = tabs.get(click.entity) {
            ui.riding = tab.0;
            consumed.pointer = true;
            continue;
        }
        if let Ok(group) = groups.get(click.entity) {
            ui.group = group.0;
            consumed.pointer = true;
            continue;
        }
        if let Ok(plus) = pluses.get(click.entity) {
            // `uicharacter.OnPressedSlotButton`: `net.SendChatPacket`.
            let group = effective_group(&ui, &player);
            if let Some((vnum, _)) = resolve(&player, &state, group, plus.slot)
                && !typing
            {
                requests.write(PlayerAction::Chat { text: format!("/skillup {vnum}"), kind: 0 });
            }
            consumed.pointer = true;
            continue;
        }
        let Ok(cell) = cells.get(click.entity) else { continue };
        if typing || ui.riding {
            continue;
        }
        let group = effective_group(&ui, &player);
        let Some((vnum, level)) = resolve(&player, &state, group, cell.slot) else { continue };
        if level == 0 {
            continue;
        }
        if ctrl {
            // `uicharacter.SelectSkill` on Ctrl: bind the source slot index.
            if let Some(slot) = empty_quick_slot(&player) {
                quickslots::bind(&mut net, slot, quickslots::SKILL, cell.slot as u8);
            }
        } else {
            send_use_skill(&mut use_skill, vnum);
        }
        consumed.pointer = true;
    }

    // A task bar skill slot (`inventory::Slot::Quick` with `Type == 2`).
    for used in quick_uses.read() {
        let QuickSlotUse::Skill(position) = *used else { continue };
        if typing || ui.riding {
            continue;
        }
        // The task bar reflects the character's active group, not the group
        // radio the page happens to show (`RegisterSkill` is server-driven).
        let group = player.skill_group.clamp(1, 2);
        if let Some((vnum, level)) = resolve(&player, &state, group, position)
            && level != 0
        {
            send_use_skill(&mut use_skill, vnum);
        }
    }
}

/// The vnum and grade of the skill bound to a visible task bar slot, from
/// `TQuickSlot` (`Type == SLOT_TYPE_SKILL`) and its `Position` (source slot).
fn quick_skill(
    player: &PlayerState,
    state: &SkillState,
    quick_page: u8,
    local: u8,
) -> Option<(u32, u8)> {
    let index = usize::from(quick_page) * 8 + usize::from(local);
    let quick: &TQuickSlot = player.quickslots.get(index)?;
    if quick.Type != quickslots::SKILL {
        return None;
    }
    let group = player.skill_group.clamp(1, 2);
    let vnum = slot_vnum(player.race, group, u32::from(quick.Position), state);
    let level = state.level(vnum);
    (vnum != 0 && level != 0).then(|| (vnum, state.grade(vnum).min(SKILL_GRADE_COUNT - 1)))
}

fn asset_exists(packs: &Option<Res<M2Packs>>, path: &str) -> bool {
    packs.as_ref().is_none_or(|packs| packs.packs.exists(paths::virtual_name(path)))
}

/// `Art::load`, resolved once up front so a changed cell does not wait a frame
/// for `core::resolve_art` (as `inventory::refresh` does for item icons).
fn load_art(assets: &AssetServer, subs: &Assets<SubImage>, path: &str) -> (Art, ImageNode) {
    let art = Art::load(assets, path);
    let image = match &art {
        Art::Image(handle) => ImageNode { image: handle.clone(), ..default() },
        Art::Sub(handle) => subs.get(handle).map_or_else(ImageNode::default, |sub| ImageNode {
            image: sub.image.clone(),
            rect: Some(sub.rect),
            ..default()
        }),
    };
    (art, image)
}

/// The grade whose widget of the active grid shows the level count: grade 3
/// (mastered) uses the last grade widget, as `__RefreshSkillPage` does.
fn count_grade(grade: u8) -> u8 {
    grade.min(SKILL_GRADE_COUNT - 1)
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    mut commands: Commands,
    assets: Res<AssetServer>,
    packs: Option<Res<M2Packs>>,
    catalog: Res<SkillCatalog>,
    state: Res<SkillState>,
    player: Res<PlayerState>,
    ui: Res<SkillUi>,
    mut cache: Local<HashMap<(u32, u8), Option<String>>>,
    mut icons: Query<
        (Entity, &mut SkillIcon, &mut Node),
        (Without<SkillBody>, Without<RidingBody>, Without<SkillCount>, Without<SkillPlus>),
    >,
    mut counts: Query<
        (&SkillCount, &mut Text, &mut Node),
        (
            Without<SkillIcon>,
            Without<SkillBody>,
            Without<RidingBody>,
            Without<SkillPlus>,
            Without<SkillPointValue>,
        ),
    >,
    mut pluses: Query<
        (&SkillPlus, &mut Node),
        (Without<SkillBody>, Without<RidingBody>, Without<SkillIcon>, Without<SkillCount>),
    >,
    mut points: Query<(&SkillPointValue, &mut Text), (Without<SkillCount>,)>,
    mut tabs: Query<(&SkillTab, &mut Selected), Without<GroupButton>>,
    mut groups: Query<(&GroupButton, &mut Selected), Without<SkillTab>>,
    mut skill_body: Query<
        &mut Node,
        (With<SkillBody>, Without<RidingBody>, Without<SkillIcon>, Without<SkillCount>, Without<SkillPlus>),
    >,
    mut riding_body: Query<
        &mut Node,
        (With<RidingBody>, Without<SkillBody>, Without<SkillIcon>, Without<SkillCount>, Without<SkillPlus>),
    >,
) {
    let group = effective_group(&ui, &player);
    // The catalog finishes loading after the page may already have drawn.
    if catalog.is_changed() {
        cache.clear();
    }

    for (entity, mut icon, mut node) in &mut icons {
        let vnum = slot_vnum(player.race, group, icon.slot, &state);
        let level = state.level(vnum);
        // Support icons are not grade-stacked; Riding uses its grade art.
        let art_grade = if icon.slot >= 101 {
            state.grade(vnum).min(SKILL_GRADE_COUNT - 1)
        } else {
            icon.grade
        };
        let path = if vnum != 0 && level != 0 {
            let key = (vnum, art_grade);
            if let Some(cached) = cache.get(&key) {
                cached.clone()
            } else {
                let computed = icon_path(&catalog, vnum, art_grade)
                    .filter(|path| asset_exists(&packs, path));
                cache.insert(key, computed.clone());
                computed
            }
        } else {
            None
        };
        if icon.vnum != vnum || icon.path != path {
            icon.vnum = vnum;
            icon.path = path.clone();
            match &path {
                Some(path) => {
                    // `core::resolve_art` fills the `ImageNode` this frame or
                    // the next; a level-up may wait one frame on the `.sub`.
                    commands.entity(entity).insert(Art::load(&assets, path));
                }
                None => {
                    commands.entity(entity).remove::<Art>();
                }
            }
        }
        node.display = if path.is_some() { Display::Flex } else { Display::None };
    }

    for (count, mut text, mut node) in &mut counts {
        let vnum = slot_vnum(player.race, group, count.slot, &state);
        let level = state.level(vnum);
        let shown = level != 0
            && (count.slot >= 101 || count.grade == count_grade(state.grade(vnum)));
        let value = if shown { level.to_string() } else { String::new() };
        if **text != value {
            **text = value;
        }
        // Keep the original bottom-right position; the empty text draws nothing.
        node.display = if shown { Display::Flex } else { Display::None };
    }

    for (plus, mut node) in &mut pluses {
        let vnum = slot_vnum(player.race, group, plus.slot, &state);
        let level = state.level(vnum);
        let point = if plus.slot >= 101 {
            player.points[POINT_SUPPORT].max(0)
        } else if ui.riding {
            player.points[POINT_HORSE].max(0)
        } else {
            player.points[POINT_ACTIVE].max(0)
        };
        let max_level = catalog.get(vnum).map_or(20, |data| data.max_level);
        // `CanShowPlusButton`: points left, first grade, below max level.
        node.display = if point > 0 && vnum != 0 && level != 0 && state.grade(vnum) == 0 && level < max_level
        {
            Display::Flex
        } else {
            Display::None
        };
    }

    for (point, mut text) in &mut points {
        let value = if point.0 {
            player.points[POINT_SUPPORT].max(0)
        } else if ui.riding {
            player.points[POINT_HORSE].max(0)
        } else {
            player.points[POINT_ACTIVE].max(0)
        };
        let value = value.to_string();
        if **text != value {
            **text = value;
        }
    }

    for (tab, mut selected) in &mut tabs {
        selected.0 = tab.0 == ui.riding;
    }
    for (button, mut selected) in &mut groups {
        selected.0 = !ui.riding && button.0 == group;
    }

    for mut node in &mut skill_body {
        node.display = if ui.riding { Display::None } else { Display::Flex };
    }
    for mut node in &mut riding_body {
        node.display = if ui.riding { Display::Flex } else { Display::None };
    }
}

#[allow(clippy::too_many_arguments)]
fn tooltip(
    primary: Query<&Window, With<PrimaryWindow>>,
    scale: Res<UiScale>,
    time: Res<Time>,
    catalog: Res<SkillCatalog>,
    state: Res<SkillState>,
    player: Res<PlayerState>,
    ui: Res<SkillUi>,
    cells: Query<(Entity, &SkillCell, &Hovered)>,
    mut tip: Query<&mut Node, With<SkillTip>>,
    mut text: Query<&mut Text, With<SkillTipText>>,
    mut last: Local<Option<(Entity, f32)>>,
) {
    let Ok(window) = primary.single() else { return };
    let cursor = window.cursor_position().map(|point| point / scale.0);
    let viewport = Vec2::new(window.width(), window.height()) / scale.0;
    let hovered = cells.iter().find(|(_, _, hovered)| hovered.0);
    let current = hovered.map(|(entity, _, _)| entity);
    if current != last.as_ref().map(|(entity, _)| *entity) {
        *last = current.map(|entity| (entity, time.elapsed_secs()));
    }
    let show = ui.riding == false
        && cursor.is_some()
        && last.is_some_and(|(_, at)| time.elapsed_secs() - at > 0.3)
        && hovered.is_some();
    for mut node in &mut tip {
        node.display = if show { Display::Flex } else { Display::None };
        if let Some(cursor) = cursor {
            node.left = Val::Px((cursor.x + 16.).min(viewport.x - 250.).max(0.));
            node.top = Val::Px((cursor.y + 18.).max(0.));
        }
    }
    let (true, Some((_, cell, _)), Some(_)) = (show, hovered, cursor) else { return };
    let group = effective_group(&ui, &player);
    let Some((vnum, level)) = resolve(&player, &state, group, cell.slot) else { return };
    if level == 0 {
        return;
    }
    let grade = state.grade(vnum).min(SKILL_GRADE_COUNT - 1);
    let details = match catalog.get(vnum) {
        Some(data) => {
            let name = if grade == 0 {
                data.name_utf8(1252).into_owned()
            } else {
                data.grades[grade as usize].name_utf8(1252).into_owned()
            };
            let mut value = format!("{name}   Level {level} / {}", data.max_level);
            if !data.description.is_empty() {
                value.push('\n');
                value.push_str(&data.description_utf8(1252));
            }
            value
        }
        None => format!("Skill {vnum}   Level {level}"),
    };
    for mut label in &mut text {
        if **label != details {
            **label = details.clone();
        }
    }
}

/// Draw each bound skill over the visible task bar quick slots. The overlay is
/// a separate child component so `inventory`'s item `Icon` keeps working.
fn quick_overlay(
    mut commands: Commands,
    assets: Res<AssetServer>,
    subs: Res<Assets<SubImage>>,
    packs: Option<Res<M2Packs>>,
    catalog: Res<SkillCatalog>,
    state: Res<SkillState>,
    player: Res<PlayerState>,
    ui: Res<InventoryUi>,
    slots: Query<(Entity, &Slot)>,
    mut overlays: Query<(Entity, &mut QuickSkill, &mut Node, &mut ImageNode)>,
) {
    let existing: HashMap<Entity, Entity> = overlays
        .iter()
        .map(|(entity, marker, _, _)| (marker.slot, entity))
        .collect();

    for (slot_entity, slot) in &slots {
        let Slot::Quick(local) = *slot else { continue };
        let bound = quick_skill(&player, &state, ui.quick_page, local);
        let Some(overlay) = existing.get(&slot_entity).copied() else {
            let Some((vnum, grade)) = bound else { continue };
            let Some(path) = icon_path(&catalog, vnum, grade)
                .filter(|path| asset_exists(&packs, path))
            else {
                continue;
            };
            let (art, image) = load_art(&assets, &subs, &path);
            let helper = commands
                .spawn((
                    QuickSkill { slot: slot_entity, vnum, grade, path: Some(path.clone()) },
                    Node { display: Display::Flex, ..core::rect(0., 0., 32., 32.) },
                    image,
                    art,
                    ZIndex(3),
                    Pickable::IGNORE,
                ))
                .id();
            commands.entity(slot_entity).add_child(helper);
            continue;
        };
        let Ok((entity, mut marker, mut node, mut image)) = overlays.get_mut(overlay) else { continue };
        let path = match bound {
            Some((vnum, grade)) => {
                icon_path(&catalog, vnum, grade).filter(|path| asset_exists(&packs, path))
                    .map(|path| (vnum, grade, path))
            }
            None => None,
        };
        match path {
            Some((vnum, grade, path)) if marker.path.as_deref() != Some(&path) => {
                marker.vnum = vnum;
                marker.grade = grade;
                marker.path = Some(path.clone());
                let (art, loaded) = load_art(&assets, &subs, &path);
                *image = loaded;
                commands.entity(entity).insert(art);
                node.display = Display::Flex;
            }
            Some((vnum, grade, _)) => {
                marker.vnum = vnum;
                marker.grade = grade;
                node.display = Display::Flex;
            }
            None => {
                marker.vnum = 0;
                marker.path = None;
                node.display = Display::None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc_line(vnum: u32, job: &str, icon: &str) -> String {
        // Columns 0..=12: VNUM, JOB, name1, name2, name3, description,
        // condition1..3, attribute, weapon, icon.
        format!("{vnum}\t{job}\tName\t\t\tDesc\t\t\t\t\t\t\t{icon}\n")
    }

    fn catalog() -> SkillCatalog {
        let mut tables = SkillTables::default();
        let data = [
            desc_line(1, "WARRIOR", "samyeon"),
            desc_line(122, "SUPPORT", "combo"),
            desc_line(130, "SUPPORT", "riding"),
        ];
        tables.register_skill_desc(data.concat().as_bytes()).unwrap();
        SkillCatalog(tables)
    }

    #[test]
    fn skill_index_dict_maps_job_and_group() {
        let state = SkillState::default();
        // Warrior group 1/2 (`RaceToJob` is race % 4).
        assert_eq!(slot_vnum(0, 1, 1, &state), 1);
        assert_eq!(slot_vnum(0, 1, 6, &state), 6);
        assert_eq!(slot_vnum(0, 2, 1, &state), 16);
        assert_eq!(slot_vnum(4, 1, 1, &state), 1);
        // Assassin group 2 has the extra 15th entry.
        assert_eq!(slot_vnum(1, 2, 1, &state), 46);
        assert_eq!(slot_vnum(1, 2, 15, &state), 140);
        assert_eq!(slot_vnum(5, 1, 1, &state), 31);
        // Sura and shaman.
        assert_eq!(slot_vnum(2, 1, 1, &state), 61);
        assert_eq!(slot_vnum(3, 2, 1, &state), 106);
        // Support slots 101..112 (101 = first entry).
        assert_eq!(slot_vnum(0, 1, 101, &state), 122);
        assert_eq!(slot_vnum(0, 1, 109, &state), RIDING_SKILL);
        assert_eq!(slot_vnum(0, 1, 112, &state), 142);
        assert_eq!(slot_vnum(0, 1, 0, &state), 0);
        assert_eq!(slot_vnum(0, 1, 20, &state), 0);
        // Reverse lookup, as `CPythonPlayer::GetSkillSlotIndex`.
        assert_eq!(slot_of_vnum(0, 1, 1), Some(1));
        assert_eq!(slot_of_vnum(0, 1, 122), Some(101));
        assert_eq!(slot_of_vnum(0, 1, 999), None);
    }

    #[test]
    fn the_new_skills_use_slots_seven_and_eight() {
        let mut state = SkillState::default();
        state.levels[114] = 1;
        assert_eq!(slot_vnum(0, 1, 7, &state), 114);
        // The C++ loop keeps the last learned vnum.
        state.levels[115] = 1;
        assert_eq!(slot_vnum(0, 1, 7, &state), 115);
        state.levels[118] = 1;
        assert_eq!(slot_vnum(0, 1, 8, &state), 118);
        state.levels[114] = 0;
        state.levels[115] = 0;
        // No learned new skill: the table entry (zero here) is used.
        assert_eq!(slot_vnum(0, 1, 7, &state), 0);
    }

    #[test]
    fn icon_paths_follow_register_skill_desc() {
        let catalog = catalog();
        // Active: grade-indexed under the job folder.
        assert_eq!(
            skill_icon_path(catalog.get(1).unwrap(), 0).as_deref(),
            Some("d:/ymir work/ui/skill/warrior/samyeon_01.sub")
        );
        assert_eq!(
            skill_icon_path(catalog.get(1).unwrap(), 1).as_deref(),
            Some("d:/ymir work/ui/skill/warrior/samyeon_02.sub")
        );
        assert_eq!(
            skill_icon_path(catalog.get(1).unwrap(), 3).as_deref(),
            Some("d:/ymir work/ui/skill/warrior/samyeon_03.sub")
        );
        // Support: one icon, no grade suffix.
        assert_eq!(
            skill_icon_path(catalog.get(122).unwrap(), 2).as_deref(),
            Some("d:/ymir work/ui/skill/common/support/combo.sub")
        );
        // Riding is support artwork with three grade icons.
        assert_eq!(
            skill_icon_path(catalog.get(130).unwrap(), 2).as_deref(),
            Some("d:/ymir work/ui/skill/common/support/riding_03.sub")
        );
    }

    #[test]
    fn skill_levels_drop_the_mastery_offset() {
        let mut state = SkillState::default();
        state.levels[31] = 25;
        state.grades[31] = 1;
        assert_eq!(state.level(31), 6);
        state.grades[31] = 2;
        state.levels[31] = 32;
        assert_eq!(state.level(31), 3);
        assert_eq!(state.level(255), 0);
    }

    #[test]
    fn quick_slots_resolve_skills_by_source_slot() {
        let mut player = PlayerState::default();
        let mut state = SkillState::default();
        player.race = 0;
        player.skill_group = 1;
        state.levels[1] = 5;
        player.quickslots[0] = TQuickSlot { Type: quickslots::SKILL, Position: 1 };
        assert_eq!(quick_skill(&player, &state, 0, 0), Some((1, 0)));
        // A second page, and a support slot.
        state.levels[122] = 3;
        player.quickslots[8] = TQuickSlot { Type: quickslots::SKILL, Position: 101 };
        assert_eq!(quick_skill(&player, &state, 1, 0), Some((122, 0)));
        // Not a skill, or a slot whose skill is not learned.
        player.quickslots[8] = TQuickSlot { Type: quickslots::ITEM, Position: 101 };
        assert_eq!(quick_skill(&player, &state, 1, 0), None);
        player.quickslots[8] = TQuickSlot { Type: quickslots::SKILL, Position: 102 };
        assert_eq!(quick_skill(&player, &state, 1, 0), None);
    }

    #[test]
    fn the_first_empty_quick_slot_is_used_for_ctrl_click() {
        let mut player = PlayerState::default();
        assert_eq!(empty_quick_slot(&player), Some(0));
        player.quickslots[0] = TQuickSlot { Type: quickslots::SKILL, Position: 1 };
        assert_eq!(empty_quick_slot(&player), Some(1));
    }
}
