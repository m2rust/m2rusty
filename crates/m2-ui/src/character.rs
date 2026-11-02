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
#[derive(Component)]
struct StatusText;
#[derive(Component)]
struct StatValues;
#[derive(Component)]
struct CombatText;
#[derive(Component)]
struct QuestText;
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
        app.init_resource::<CharacterTab>().init_resource::<CharacterWindow>()
            .add_systems(Update, (tabs, keys, stat_actions).in_set(UiSet::Actions))
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
    let quest = core::text(commands, quest_page, "No quests.", core::rect(16., 20., 218., 260.));
    commands.entity(quest).insert((QuestText, TextColor(Color::srgb(0.85, 0.82, 0.7))));

    crate::skills::build(commands, assets, root);
    crate::emotes::build(commands, assets, root);
}

fn quests(player: Res<PlayerState>, mut texts: Query<&mut Text, With<QuestText>>) {
    let titles: Vec<_> = player.quests.values().filter(|quest| !quest.title.is_empty()).map(|quest| quest.title.as_str()).collect();
    let value = if titles.is_empty() { "No quests.".to_owned() } else { titles.join("\n") };
    for mut text in &mut texts { if **text != value { **text = value.clone(); } }
}

fn build_status(commands: &mut Commands, assets: &AssetServer, page: Entity) {
    // `Character_Page`: face, level/experience and the two stat blocks.
    core::picture(commands, assets, page, "d:/ymir work/ui/game/windows/box_face.sub", core::rect(7., 7., 50., 50.));
    let face = core::picture(commands, assets, page, "d:/ymir work/ui/game/windows/face_warrior.sub", core::rect(11., 11., 42., 42.));
    commands.entity(face).insert(FaceImage(None));

    let top = core::text(commands, page, "", core::rect(64., 10., 182., 92.));
    commands.entity(top).insert((StatusText, TextLayout::justify(Justify::Left)));

    for (row, (key, caption)) in [("hth", "CON"), ("int", "INT"), ("str", "STR"), ("dex", "DEX")]
        .into_iter()
        .enumerate()
    {
        let y = 138. + row as f32 * 23.;
        let label = core::text(commands, page, caption, core::rect(34., y + 1., 36., 18.));
        commands.entity(label).insert(TextColor(Color::srgb(0.85, 0.78, 0.6)));
        stat_button(commands, assets, page, 16., y, key, false);
        stat_button(commands, assets, page, 202., y, key, true);
    }
    let values = core::text(commands, page, "", core::rect(74., 138., 122., 96.));
    commands.entity(values).insert((StatValues, TextLayout::justify(Justify::Left)));
    let combat = core::text(commands, page, "", core::rect(16., 240., 220., 56.));
    commands.entity(combat).insert((CombatText, TextLayout::justify(Justify::Left)));
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

fn stat_actions(mut clicks: MessageReader<Click>, buttons: Query<&StatAction>, mut actions: MessageWriter<PlayerAction>) {
    for click in clicks.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        let Ok(button) = buttons.get(click.entity) else { continue };
        let text = if button.plus {
            format!("/stat {}", button.key)
        } else {
            format!("/stat- {}", button.key)
        };
        actions.write(PlayerAction::Chat { text, kind: 0 });
    }
}

fn status(
    mut commands: Commands,
    assets: Res<AssetServer>,
    player: Res<PlayerState>,
    mut top: Query<&mut Text, (With<StatusText>, Without<StatValues>, Without<CombatText>)>,
    mut stats: Query<&mut Text, (With<StatValues>, Without<StatusText>, Without<CombatText>)>,
    mut combat: Query<&mut Text, (With<CombatText>, Without<StatusText>, Without<StatValues>)>,
    mut faces: Query<(Entity, &mut FaceImage, &mut ImageNode)>,
) {
    let p = &player.points;
    let top_value = format!(
        "{}    Level {}\nEXP  {} / {}\n\nPoints  {}\nSkill points  {}",
        player.name,
        p[1],
        p[3].max(0),
        p[4].max(0),
        p[26].max(0),
        p[28].max(0),
    );
    let stat_value = format!(
        "{}\n{}\n{}\n{}",
        p[13].max(0),
        p[15].max(0),
        p[12].max(0),
        p[14].max(0),
    );
    let combat_value = format!(
        "HP {} / {}    SP {} / {}\nAttack {}-{}    Defense {}\nMove {}  Attack speed {}\nMagic {}/{}  Evade {}",
        p[5],
        p[6],
        p[7],
        p[8],
        p[29].max(0),
        p[30].max(0),
        p[20].max(0),
        p[19].max(0),
        p[17].max(0),
        p[22].max(0),
        p[23].max(0),
        p[18].max(0),
    );
    for mut text in &mut top {
        if **text != top_value {
            **text = top_value.clone();
        }
    }
    for mut text in &mut stats {
        if **text != stat_value {
            **text = stat_value.clone();
        }
    }
    for mut text in &mut combat {
        if **text != combat_value {
            **text = combat_value.clone();
        }
    }
    let race = (player.race > 0).then_some(player.race);
    for (entity, mut face, mut image) in &mut faces {
        if face.0 == race {
            continue;
        }
        let job = match race.unwrap_or(0) % 4 {
            1 => "assassin",
            2 => "sura",
            3 => "shaman",
            _ => "warrior",
        };
        let art = Art::load(&assets, &format!("d:/ymir work/ui/game/windows/face_{job}.sub"));
        if let Art::Image(handle) = &art {
            image.image = handle.clone();
            image.rect = None;
        }
        commands.entity(entity).insert(art);
        face.0 = race;
    }
}
