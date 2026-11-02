//! NPC quest scripts (`HEADER_GC_SCRIPT`), following the C++
//! `CPythonEventManager` parser and Python `uiquest.QuestDialog` controls.

use bevy::picking::pointer::PointerButton;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Button;
use m2_game::net::NetMessage;
use m2_game::player::PlayerAction;
use m2_net::{phase::NetEvent, wire::GcPacket};
use crate::core::Control;
use crate::core::{self, Click, Consumed, Surface, UiSet, WindowManager};
use crate::hud::GameUiRoot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EndAction {
    Continue,
    Done,
}

#[derive(Clone, Debug)]
struct Dialog {
    text: String,
    answers: Vec<String>,
    end: EndAction,
}

#[derive(Resource, Default)]
struct DialogState {
    serial: u64,
    page: usize,
    dialog: Option<Dialog>,
}

#[derive(Component)]
struct DialogRoot(u64, usize);
#[derive(Component)]
struct Answer(usize);
#[derive(Component)]
struct Continue;
#[derive(Component)]
struct Finish;
#[derive(Component)]
struct Cancel;
#[derive(Component)]
struct PageBy(isize);
#[derive(Component)]
struct ChoiceSurface;

pub struct QuestDialogPlugin;
impl Plugin for QuestDialogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DialogState>().add_systems(
            Update,
            (
                receive_scripts.in_set(UiSet::State),
                actions.in_set(UiSet::Actions),
                (draw, choice_hover).in_set(UiSet::Draw),
            ),
        );
    }
}

fn receive_scripts(mut messages: MessageReader<NetMessage>, mut state: ResMut<DialogState>) {
    for message in messages.read() {
        match message {
            NetMessage::Game(NetEvent::Packet(GcPacket::Script { head, tail })) => {
                state.serial = state.serial.wrapping_add(1);
                state.page = 0;
                state.dialog = Some(parse_dialog(head.skin, tail));
            }
            NetMessage::Disconnected { .. } => {
                state.serial = state.serial.wrapping_add(1);
                state.dialog = None;
            }
            _ => {}
        }
    }
}

fn parse_dialog(_skin: u8, bytes: &[u8]) -> Dialog {
    // The server uses the active locale's legacy code page, as the C++
    // `CPythonEventManager` does before passing strings to Python.
    let source = m2_formats::utf8::decode(1252, bytes).into_owned();
    let mut text = String::new();
    let mut answers = Vec::new();
    let mut end = EndAction::Continue;
    let mut rest = source.as_str();
    while let Some(open) = rest.find('[') {
        text.push_str(&rest[..open]);
        let command_start = open + 1;
        let Some(close) = rest[command_start..].find(']') else {
            text.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let block = rest[command_start..command_start + close].trim();
        let mut parts = block.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or_default().trim_start_matches('/').to_ascii_uppercase();
        let args = parts.next().unwrap_or_default();
        match command.as_str() {
            "ENTER" => text.push('\n'),
            "CLEAR" | "CLEAR_TEXT" => text.clear(),
            "QUESTION" => answers = script_values(args),
            "NEXT" => {
                end = EndAction::Continue;
                rest = "";
                break;
            }
            "DONE" => {
                end = EndAction::Done;
                rest = "";
                break;
            }
            _ => {}
        }
        rest = &rest[command_start + close + 1..];
    }
    text.push_str(rest);
    let text = text
        .lines()
        .filter(|line| !answers.iter().any(|answer| answer == line.trim()))
        .collect::<Vec<_>>()
        .join("\n");
    Dialog { text: text.trim().to_owned(), answers, end }
}

/// Extracts the values from the event parser's `name;value|...` argument list.
fn script_values(args: &str) -> Vec<String> {
    args.split('|')
        .filter_map(|arg| arg.split_once(';').map(|(_, value)| value.trim().to_owned()))
        .filter(|value| !value.is_empty())
        .take(255)
        .collect()
}

fn actions(
    mut commands: MessageReader<Click>,
    answers: Query<&Answer>,
    continues: Query<(), With<Continue>>,
    finishes: Query<(), With<Finish>>,
    cancels: Query<(), With<Cancel>>,
    pages: Query<&PageBy>,
    keys: Res<ButtonInput<KeyCode>>,
    mut consumed: ResMut<Consumed>,
    mut state: ResMut<DialogState>,
    mut player_actions: MessageWriter<PlayerAction>,
) {
    for click in commands.read() {
        if click.button != PointerButton::Primary {
            continue;
        }
        if let Ok(answer) = answers.get(click.entity) {
            player_actions.write(PlayerAction::QuestAnswer(answer.0 as u8));
            state.dialog = None;
            consumed.pointer = true;
        } else if continues.contains(click.entity) {
            player_actions.write(PlayerAction::QuestAnswer(254));
            state.dialog = None;
            consumed.pointer = true;
        } else if cancels.contains(click.entity) {
            player_actions.write(PlayerAction::QuestAnswer(254));
            state.dialog = None;
            consumed.pointer = true;
        } else if finishes.contains(click.entity) {
            state.dialog = None;
            consumed.pointer = true;
        } else if let Ok(page) = pages.get(click.entity) {
            let max_page = state.dialog.as_ref().map_or(0, |dialog| dialog.answers.len().saturating_sub(1) / 8);
            state.page = (state.page as isize + page.0).clamp(0, max_page as isize) as usize;
            consumed.pointer = true;
        }
    }
    if state.dialog.is_some() && keys.just_pressed(KeyCode::Escape) && !consumed.keyboard {
        state.dialog = None;
        consumed.keyboard = true;
    } else if state.dialog.as_ref().is_some_and(|dialog| dialog.answers.is_empty())
        && keys.just_pressed(KeyCode::Enter)
        && !consumed.keyboard
    {
        if state.dialog.as_ref().is_some_and(|dialog| dialog.end == EndAction::Continue) {
            player_actions.write(PlayerAction::QuestAnswer(254));
        }
        state.dialog = None;
        consumed.keyboard = true;
    }
}

fn draw(
    mut commands: Commands,
    assets: Res<AssetServer>,
    state: Res<DialogState>,
    roots: Query<(Entity, &DialogRoot)>,
    mut windows: ResMut<WindowManager>,
) {
    let wanted = state.dialog.as_ref().map(|_| (state.serial, state.page));
    for (entity, root) in &roots {
        if Some((root.0, root.1)) != wanted {
            commands.entity(entity).try_despawn();
            if windows.modal == Some(entity) {
                windows.modal = None;
            }
        }
    }
    let Some(dialog) = state.dialog.as_ref() else { return };
    if roots.iter().any(|(_, root)| root.0 == state.serial && root.1 == state.page) {
        return;
    }

    let start = state.page * 8;
    let end = (start + 8).min(dialog.answers.len());
    let visible_answers = end.saturating_sub(start);
    let height = if visible_answers == 0 {
        245.
    } else if dialog.answers.len() > 8 {
        218. + visible_answers as f32 * 34.
    } else {
        188. + visible_answers as f32 * 34.
    };
    let root = commands
        .spawn((
            GameUiRoot,
            DialogRoot(state.serial, state.page),
            Surface,
            GlobalZIndex(300),
            Node { position_type: PositionType::Absolute, left: Val::Px(150.), top: Val::Px(90.), width: Val::Px(500.), height: Val::Px(height), ..default() },
        ))
        .id();
    windows.modal = Some(root);
    crate::target::thin_board(&mut commands, &assets, root);
    let title = core::text(&mut commands, root, "Quest", core::rect(25., 17., 450., 24.));
    commands.entity(title).insert((TextFont { font_size: bevy::text::FontSize::Px(15.), ..default() }, TextColor(Color::srgb(0.96, 0.83, 0.48))));
    let body = core::text(&mut commands, root, dialog.text.clone(), Node {
        position_type: PositionType::Absolute,
        left: Val::Px(25.), top: Val::Px(48.), width: Val::Px(450.), height: Val::Px(if visible_answers > 0 { 112. } else { 130. }),
        overflow: Overflow::clip_y(), ..default()
    });
    commands.entity(body).insert((TextLayout::justify(Justify::Left), TextFont { font_size: bevy::text::FontSize::Px(14.), ..default() }, TextColor(Color::srgb(0.92, 0.89, 0.82))));

    if !dialog.answers.is_empty() {
        for (index, answer) in dialog.answers.iter().enumerate().take(end).skip(start) {
            let button = choice_button(&mut commands, root, core::rect(28., 166. + (index - start) as f32 * 34., 444., 28.), answer);
            commands.entity(button).insert(Answer(index));
        }
        if start > 0 {
            let button = choice_button(&mut commands, root, core::rect(28., height - 40., 120., 26.), "‹ Previous");
            commands.entity(button).insert(PageBy(-1));
        }
        if end < dialog.answers.len() {
            let button = choice_button(&mut commands, root, core::rect(352., height - 40., 120., 26.), "More ›");
            commands.entity(button).insert(PageBy(1));
        }
    } else {
        let (caption, is_continue) = match dialog.end {
            EndAction::Continue => ("Continue", true),
            EndAction::Done => ("OK", false),
        };
        let button = choice_button(&mut commands, root, core::rect(315., height - 43., 155., 30.), caption);
        if is_continue {
            commands.entity(button).insert(Continue);
        } else {
            commands.entity(button).insert(Finish);
        }
        let cancel = choice_button(&mut commands, root, core::rect(30., height - 43., 155., 30.), "Cancel");
        commands.entity(cancel).insert(Cancel);
    }
}

fn choice_button(commands: &mut Commands, parent: Entity, node: Node, caption: &str) -> Entity {
    let button = commands
        .spawn((
            Button,
            Control,
            Surface,
            ChoiceSurface,
            Hovered::default(),
            node,
            BackgroundColor(Color::srgba(0.12, 0.10, 0.08, 0.94)),
            BorderColor::all(Color::srgba(0.67, 0.53, 0.32, 0.9)),
        ))
        .id();
    commands.entity(parent).add_child(button);
    let label = core::text(commands, button, caption, Node {
        width: Val::Percent(100.), height: Val::Percent(100.), display: Display::Flex,
        justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default()
    });
    commands.entity(label).insert((TextFont { font_size: bevy::text::FontSize::Px(13.), ..default() }, TextColor(Color::srgb(0.95, 0.90, 0.78))));
    button
}

fn choice_hover(mut buttons: Query<(&Hovered, &mut BackgroundColor), With<ChoiceSurface>>) {
    for (hovered, mut background) in &mut buttons {
        background.0 = if hovered.0 {
            Color::srgba(0.31, 0.21, 0.10, 0.98)
        } else {
            Color::srgba(0.12, 0.10, 0.08, 0.94)
        };
    }
}
