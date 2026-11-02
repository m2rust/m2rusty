//! The character window: STATUS/SKILL/EMOTION/QUEST tabs from
//! `uiscript/characterwindow.py` (locale `/ui/windows` artwork).
//!
//! The skill and emotion pages live in [`crate::skills`] and [`crate::emotes`];
//! this module owns the 253x361 board, the bottom tab bar, the status page and
//! the quest page.

use bevy::input_focus::InputFocus;
use bevy::picking::hover::Hovered;
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use m2_game::player::{PlayerAction, PlayerState};

use crate::core::{self, Art, Click, Control, FloatingWindow, Surface, UiSet, WindowTitle};
use crate::hud::{ChatState, GameUiRoot};
use crate::login::{LoginScreen, Status};

/// Which page of the character window is shown (`uicharacter.SetState`).
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq)]
pub enum CharacterTab {
    #[default]
    Status,
    Skill,
    Emotion,
    Quest,
}

/// Marks a page root; the visibility system shows only the active tab's page.
#[derive(Component)]
pub struct PageRoot(pub CharacterTab);

#[derive(Component, Clone, Copy)]
struct TabButton(CharacterTab);
#[derive(Component, Clone, Copy)]
struct TabBackground(CharacterTab);
#[derive(Component)]
struct StatAction {
    key: &'static str,
    plus: bool,
}
#[derive(Component, Clone, Copy)]
enum StatField { Name, Level, Exp, Remaining, Points, Point(usize), Hp, Sp, Attack, Defense, MagicAttack }
#[derive(Component)]
struct QuestRow(usize);
#[derive(Component)]
struct QuestLabel(usize);
#[derive(Component)]
struct QuestIcon(usize);
#[derive(Component)]
struct QuestPage(i32);
#[derive(Component)]
struct QuestPageLabel;
#[derive(Resource, Default)]
struct QuestOffset(usize);
#[derive(Component)]
struct FaceImage(Option<u16>);
#[derive(Resource)]
struct CharacterWindow(Entity);
impl Default for CharacterWindow {
    fn default() -> Self { Self(Entity::PLACEHOLDER) }
}

pub struct CharacterPlugin;
impl Plugin for CharacterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<QuestOffset>().init_resource::<CharacterTab>().init_resource::<CharacterWindow>()
            .add_systems(Update, (tabs, keys, stat_actions, quest_actions).in_set(UiSet::Actions))
            .add_systems(Update, (visibility, status, quests).chain().in_set(UiSet::Draw));
    }
}

pub fn build(commands: &mut Commands, assets: &AssetServer) {
    let root = core::window(
        commands,
        assets,
        "character",
        "Character",
        Vec2::new(253., 361.),
        Vec2::new(24., 0.),
        core::Anchor::LeftCenter(-18.5),
        false,
    );
    commands.entity(root).insert(GameUiRoot);
    commands.insert_resource(CharacterWindow(root));

    // `Tab_01..04`: each 250x31 image is the bar with one tab highlighted.
    for (index, tab) in [
        CharacterTab::Status,
        CharacterTab::Skill,
        CharacterTab::Emotion,
        CharacterTab::Quest,
    ]
    .into_iter()
    .enumerate()
    {
        let image = core::picture(
            commands,
            assets,
            root,
            &format!("locale/en/ui/windows/tab_{}.sub", index + 1),
            core::rect(0., 328., 250., 31.),
        );
        commands.entity(image).insert((TabBackground(tab), ZIndex(1)));
        let hit = commands
            .spawn((
                TabButton(tab),
                Control,
                Surface,
                Hovered::default(),
                core::rect([6., 61., 130., 192.][index], 333., [53., 67., 61., 55.][index], 27.),
                BackgroundColor(Color::NONE),
                ZIndex(2),
            ))
            .id();
        commands.entity(root).add_child(hit);
    }

    let status_page = commands.spawn((PageRoot(CharacterTab::Status), core::rect(0., 24., 250., 304.))).id();
    commands.entity(root).add_child(status_page);
    build_status(commands, assets, status_page);

    let quest_page = commands.spawn((PageRoot(CharacterTab::Quest), core::rect(0., 24., 250., 304.))).id();
    commands.entity(root).add_child(quest_page);
    for row in 0..5 {
        let entry = commands.spawn((QuestRow(row), Control, Surface, Hovered::default(), core::Tooltip(String::new()), core::rect(12., 10. + row as f32 * 51., 226., 48.))).id();
        commands.entity(quest_page).add_child(entry);
        core::picture(commands, assets, entry, "d:/ymir work/ui/game/quest/slot_base.sub", core::rect(0., 5., 32., 32.));
        let icon = core::picture(commands, assets, entry, "season1/icon/scroll_open.tga", core::rect(0., 5., 32., 32.));
        commands.entity(icon).insert(QuestIcon(row));
        let text = core::text(commands, entry, "", core::rect(38., 0., 186., 48.));
        commands.entity(text).insert((QuestLabel(row), TextLayout::no_wrap(), TextFont { font_size: bevy::text::FontSize::Px(11.), ..default() }));
    }
    for (x, delta, label) in [(12., -1, "<"), (202., 1, ">") ] {
        let button = core::button(commands, assets, quest_page, core::rect(x, 271., 36., 22.), "d:/ymir work/ui/public/small_thin_button", label);
        commands.entity(button).insert(QuestPage(delta));
    }
    let label = core::text(commands, quest_page, "", core::rect(50., 274., 150., 18.));
    commands.entity(label).insert(QuestPageLabel);

    crate::skills::build(commands, assets, root);
    crate::emotes::build(commands, assets, root);
}

fn quest_actions(mut clicks: MessageReader<Click>, rows: Query<&QuestRow>, pages: Query<&QuestPage>, mut offset: ResMut<QuestOffset>, player: Res<PlayerState>, mut actions: MessageWriter<PlayerAction>) {
    let entries: Vec<_> = player.quests.iter().filter(|(_, q)| !q.title.is_empty()).collect();
    for click in clicks.read().filter(|c| c.button == PointerButton::Primary) {
        if let Ok(row) = rows.get(click.entity) {
            if let Some((index, _)) = entries.get(offset.0 + row.0) { actions.write(PlayerAction::QuestButton(0x8000_0000 | u32::from(**index))); }
        }
        if let Ok(page) = pages.get(click.entity) {
            offset.0 = offset.0.saturating_add_signed(page.0 as isize * 5).min(entries.len().saturating_sub(1) / 5 * 5);
        }
    }
}

fn quests(mut commands: Commands, assets: Res<AssetServer>, player: Res<PlayerState>, mut offset: ResMut<QuestOffset>, mut rows: Query<(&QuestRow, &mut Node, &mut core::Tooltip)>, mut labels: Query<(&QuestLabel, &mut Text), Without<QuestPageLabel>>, mut page_labels: Query<&mut Text, With<QuestPageLabel>>, icons: Query<(Entity, &QuestIcon)>) {
    let entries: Vec<_> = player.quests.values().filter(|q| !q.title.is_empty()).collect();
    let bound = offset.0.min(entries.len().saturating_sub(1) / 5 * 5);
    if offset.0 != bound { offset.0 = bound; }
    for (row, mut node, mut tip) in &mut rows { core::set_display(&mut node, if entries.get(offset.0 + row.0).is_some() { Display::Flex } else { Display::None }); let title = entries.get(offset.0 + row.0).map_or("", |q| q.title.as_str()); if tip.0 != title {tip.0 = title.to_owned();} }
    for (row, mut text) in &mut labels {
        if let Some(q) = entries.get(offset.0 + row.0) {
            let counter = if q.counter_name.is_empty() { String::new() } else { format!("{}: {}", q.counter_name, q.counter_value) };
            let clock = if q.clock_name.is_empty() { String::new() } else {
                let left = i64::from(q.clock_value) - q.clock_updated.map_or(0, |at| at.elapsed().as_secs() as i64);
                if left <= 0 { "Time over".into() } else { format!("{}: {}m {:02}s", q.clock_name, left / 60, left % 60) }
            };
            let shorten = |s: &str| if s.chars().count() > 28 {format!("{}...",s.chars().take(25).collect::<String>())} else {s.to_owned()};
            core::set_text(&mut text, format!("{}\n{}\n{}", shorten(&q.title), shorten(&counter), shorten(&clock)));
        }
    }
    for mut label in &mut page_labels { core::set_text(&mut label, if entries.is_empty() { "No quests".into() } else { format!("{} / {}", offset.0 / 5 + 1, entries.len().div_ceil(5)) }); }
    if player.is_changed() || offset.is_changed() {
        for (entity, row) in &icons {
            if let Some(q) = entries.get(offset.0 + row.0) {
                let path = if q.icon.is_empty() { "season1/icon/scroll_open.tga".to_owned() } else { format!("d:/ymir work/ui/game/quest/questicon/{}", q.icon) };
                commands.entity(entity).insert(Art::load(&assets, &path));
            }
        }
    }
}

fn field(commands: &mut Commands, assets: &AssetServer, page: Entity, kind: StatField, label: &str, x: f32, y: f32, width: f32, label_width: f32) {
    let caption = core::text(commands, page, label, core::rect(x, y + 2., label_width, 17.));
    commands.entity(caption).insert((TextFont {font_size: bevy::text::FontSize::Px(10.), ..default()}, TextLayout::no_wrap()));
    core::picture(commands, assets, page, "d:/ymir work/ui/public/parameter_slot_01.sub", core::rect(x + label_width, y, width - label_width, 17.));
    let text = core::text(commands, page, "", core::rect(x + label_width, y, width - label_width, 17.));
    commands.entity(text).insert((kind, TextLayout::justify(Justify::Center), TextFont { font_size: bevy::text::FontSize::Px(11.), ..default() }));
}

fn build_status(commands: &mut Commands, assets: &AssetServer, page: Entity) {
    core::picture(commands, assets, page, "d:/ymir work/ui/game/windows/box_face.sub", core::rect(7., 7., 50., 50.));
    let face = core::picture(commands, assets, page, "d:/ymir work/ui/game/windows/face_warrior.sub", core::rect(11., 11., 42., 42.));
    commands.entity(face).insert(FaceImage(None));
    let name = core::text(commands, page, "", core::rect(62., 8., 180., 18.)); commands.entity(name).insert(StatField::Name);
    field(commands, assets, page, StatField::Level, "Level", 62., 31., 180., 48.);
    field(commands, assets, page, StatField::Exp, "EXP", 12., 62., 230., 70.);
    field(commands, assets, page, StatField::Remaining, "Remaining", 12., 82., 230., 70.);
    field(commands, assets, page, StatField::Points, "Stat points", 12., 104., 230., 170.);
    for (row, (key, caption, point)) in [("ht", "CON", 13), ("iq", "INT", 15), ("st", "STR", 12), ("dx", "DEX", 14)].into_iter().enumerate() {
        let y = 130. + row as f32 * 23.;
        field(commands, assets, page, StatField::Point(point), caption, 30., y, 65., 30.);
        stat_button(commands, assets, page, 12., y + 2., key, false);
        stat_button(commands, assets, page, 99., y + 2., key, true);
    }
    for (row, (kind, caption)) in [(StatField::Hp, "HP"), (StatField::Sp, "SP"), (StatField::Attack, "Attack"), (StatField::Defense, "Defense")].into_iter().enumerate() {
        field(commands, assets, page, kind, caption, 116., 130. + row as f32 * 23., 126., 46.);
    }
    for (row, (kind, caption, right, right_caption)) in [
        (StatField::Point(19), "Move", StatField::MagicAttack, "Magic atk"),
        (StatField::Point(17), "Atk spd", StatField::Point(23), "Magic def"),
        (StatField::Point(21), "Cast spd", StatField::Point(18), "Evade"),
    ].into_iter().enumerate() {
        let y = 231. + row as f32 * 23.;
        field(commands, assets, page, kind, caption, 12., y, 100., 55.);
        field(commands, assets, page, right, right_caption, 116., y, 126., 60.);
    }
}

fn stat_button(commands: &mut Commands, assets: &AssetServer, parent: Entity, x: f32, y: f32, key: &'static str, plus: bool) {
    let prefix = if plus { "d:/ymir work/ui/game/windows/btn_plus" } else { "d:/ymir work/ui/game/windows/btn_minus" };
    let art = [
        Art::load(assets, &format!("{prefix}_up.sub")),
        Art::load(assets, &format!("{prefix}_over.sub")),
        Art::load(assets, &format!("{prefix}_down.sub")),
    ];
    let button = core::button_with(commands, parent, core::rect(x, y, 13., 13.), art, "");
    commands.entity(button).insert(StatAction { key, plus });
}

fn tabs(
    mut clicks: MessageReader<Click>,
    buttons: Query<&TabButton>,
    mut windows: Query<&mut FloatingWindow>,
    mut tab: ResMut<CharacterTab>,
) {
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        let Ok(button) = buttons.get(click.entity) else { continue };
        *tab = button.0;
        for mut window in &mut windows {
            if window.id == "character" {
                window.open = true;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    screen: Res<LoginScreen>,
    chat: Res<ChatState>,
    focus: Res<InputFocus>,
    mut tab: ResMut<CharacterTab>,
    mut windows: Query<&mut FloatingWindow>,
    mut consumed: ResMut<crate::core::Consumed>,
) {
    if !matches!(screen.status, Status::InGame { .. }) || chat.typing || focus.get().is_some() {
        return;
    }
    if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight, KeyCode::AltLeft, KeyCode::AltRight]) { return; }
    let target = if keys.just_pressed(KeyCode::KeyC) {
        Some(CharacterTab::Status)
    } else if keys.just_pressed(KeyCode::KeyV) {
        Some(CharacterTab::Skill)
    } else if keys.just_pressed(KeyCode::KeyB) {
        Some(CharacterTab::Emotion)
    } else if keys.just_pressed(KeyCode::KeyN) {
        Some(CharacterTab::Quest)
    } else {
        None
    };
    let Some(target) = target else { return };
    consumed.keyboard = true;
    if *tab == target && windows.iter().any(|window| window.id == "character" && window.open) {
        for mut window in &mut windows {
            if window.id == "character" {
                window.open = false;
            }
        }
    } else {
        *tab = target;
        for mut window in &mut windows {
            if window.id == "character" {
                window.open = true;
            }
        }
    }
}

fn visibility(
    tab: Res<CharacterTab>,
    window: Res<CharacterWindow>,
    mut pages: Query<(&PageRoot, &mut Node)>,
    mut backgrounds: Query<(&TabBackground, &mut Node), Without<PageRoot>>,
    mut titles: Query<(&WindowTitle, &mut Text)>,
) {
    let title = match *tab {
        CharacterTab::Status => "Character",
        CharacterTab::Skill => "Skill",
        CharacterTab::Emotion => "Emotion",
        CharacterTab::Quest => "Quest",
    };
    for (page, mut node) in &mut pages {
        core::set_display(&mut node, if page.0 == *tab { Display::Flex } else { Display::None });
    }
    for (background, mut node) in &mut backgrounds {
        core::set_display(&mut node, if background.0 == *tab { Display::Flex } else { Display::None });
    }
    for (owner, mut text) in &mut titles {
        if owner.0 == window.0 && **text != title {
            **text = title.into();
        }
    }
}

fn stat_actions(mut clicks: MessageReader<Click>, buttons: Query<&StatAction>, player: Res<PlayerState>, mut actions: MessageWriter<PlayerAction>) {
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        let Ok(button) = buttons.get(click.entity) else { continue };
        if player.points[if button.plus {26} else {112}] <= 0 { continue; }
        let text = if button.plus {
            format!("/stat {}", button.key)
        } else {
            format!("/stat- {}", button.key)
        };
        actions.write(PlayerAction::Chat { text, kind: 0 });
    }
}

fn stat_value(field: StatField, player: &PlayerState) -> String {
    let p = &player.points;
    let range = |lo: i32, hi: i32| if lo == hi { lo.to_string() } else { format!("{lo}-{hi}") };
    match field {
        StatField::Name => player.name.clone(), StatField::Level => p[1].to_string(),
        StatField::Exp => (p[3] as u32).to_string(),
        StatField::Remaining => (i64::from(p[4] as u32) - i64::from(p[3] as u32)).to_string(),
        StatField::Points => p[26].to_string(), StatField::Point(index) => p[index].to_string(),
        StatField::Hp => format!("{}/{}", p[5], p[6]), StatField::Sp => format!("{}/{}", p[7], p[8]),
        StatField::Attack => range(p[29] + p[95] + p[91], p[30] + p[95] + p[91]),
        // locale/en keeps constInfo.ADD_DEF_BONUS_ENABLE disabled.
        StatField::Defense => p[20].to_string(),
        StatField::MagicAttack => range(p[22] + p[202], p[22] + p[203]),
    }
}

fn status(mut commands: Commands, assets: Res<AssetServer>, player: Res<PlayerState>, mut fields: Query<(&StatField, &mut Text)>, mut buttons: Query<(&StatAction, &mut Node)>, mut faces: Query<(Entity, &mut FaceImage, &mut ImageNode)>) {
    for (field, mut text) in &mut fields { let value = stat_value(*field, &player); if text.0 != value { text.0 = value; } }
    for (button, mut node) in &mut buttons { core::set_display(&mut node, if player.points[if button.plus {26} else {112}] > 0 { Display::Flex } else { Display::None }); }
    for (entity, mut face, mut image) in &mut faces {
        if face.0 == Some(player.race) { continue; }
        let job = match player.race % 4 { 1 => "assassin", 2 => "sura", 3 => "shaman", _ => "warrior" };
        let art = Art::load(&assets, &format!("d:/ymir work/ui/game/windows/face_{job}.sub"));
        if let Art::Image(handle) = &art { image.image = handle.clone(); image.rect = None; }
        commands.entity(entity).insert(art); face.0 = Some(player.race);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_uses_unsigned_experience_and_total_combat_points() {
        let mut player = PlayerState::default();
        player.points[3] = 3_000_000_000u32 as i32; player.points[4] = 3_500_000_000u32 as i32;
        assert_eq!(stat_value(StatField::Exp, &player), "3000000000");
        assert_eq!(stat_value(StatField::Remaining, &player), "500000000");
        for (index, value) in [(29, 100), (30, 120), (95, 10), (91, 20), (22, 70), (202, 20), (203, 30), (20, 50), (96, 7)] { player.points[index] = value; }
        assert_eq!(stat_value(StatField::Attack, &player), "130-150");
        assert_eq!(stat_value(StatField::MagicAttack, &player), "90-100");
        assert_eq!(stat_value(StatField::Defense, &player), "50");
    }
    #[test]
    fn quest_page_selection_sends_the_high_bit_quest_button_index() {
        let mut app = App::new();
        app.init_resource::<PlayerState>().init_resource::<QuestOffset>().add_message::<Click>().add_message::<PlayerAction>().add_systems(Update, quest_actions);
        for index in 10..17 { app.world_mut().resource_mut::<PlayerState>().quests.insert(index, m2_game::player::Quest {title:format!("Quest {index}"),..default()}); }
        let row = app.world_mut().spawn(QuestRow(1)).id();
        let page = app.world_mut().spawn(QuestPage(1)).id();
        let mut cursor = app.world().resource::<Messages<PlayerAction>>().get_cursor();
        app.world_mut().write_message(Click{entity:page,button:PointerButton::Primary,count:1}); app.update();
        app.world_mut().write_message(Click{entity:row,button:PointerButton::Primary,count:1}); app.update();
        assert!(matches!(cursor.read(app.world().resource::<Messages<PlayerAction>>()).next(), Some(PlayerAction::QuestButton(0x8000_0010))));
    }
    #[test]
    fn stat_buttons_send_the_server_point_names() {
        let mut app = App::new();
        app.init_resource::<PlayerState>().add_message::<Click>().add_message::<PlayerAction>().add_systems(Update, stat_actions);
        app.world_mut().resource_mut::<PlayerState>().points[26] = 3;
        let mut cursor = app.world().resource::<Messages<PlayerAction>>().get_cursor();
        // `statusPlusCommandDict`: `/stat ht|iq|st|dx`, not the key names.
        for key in ["ht", "iq", "st", "dx"] {
            let button = app.world_mut().spawn(StatAction { key, plus: true }).id();
            app.world_mut().write_message(Click{entity:button,button:PointerButton::Primary,count:1});
        }
        let minus = app.world_mut().spawn(StatAction { key: "ht", plus: false }).id();
        app.world_mut().write_message(Click{entity:minus,button:PointerButton::Primary,count:1});
        app.update();
        let sent: Vec<_> = cursor.read(app.world().resource::<Messages<PlayerAction>>()).map(|action| match action { PlayerAction::Chat { text, .. } => text.clone(), _ => String::new() }).collect();
        // No stat reset points: the minus button sends nothing.
        assert_eq!(sent, ["/stat ht", "/stat iq", "/stat st", "/stat dx"]);
    }

}
