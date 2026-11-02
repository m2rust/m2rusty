//! The in-game HUD: the task bar (HP/SP/stamina gauges and quick slots), the
//! chat log and the lifetime of the game windows.
//!
//! The layout follows `uiscript/taskbar.py`; the artwork is the original
//! `TaskBar.tga` atlas through its `.sub` regions. The inventory itself lives in
//! [`crate::inventory`].

use bevy::input::{ButtonState, keyboard::KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use crate::core::{self, Click, Consumed, FloatingWindow, Surface, Tooltip, UiSet};
use crate::inventory::{self, Slot};
use crate::login::{LoginScreen, Status};
use m2_game::player::{PlayerAction, PlayerState};

/// Marks every top-level entity of the in-game UI, so leaving the game can
/// remove all of it (windows, HUD, tooltips and the drag ghost).
#[derive(Component)]
pub struct GameUiRoot;

/// The chat line: `Enter` opens it, `Enter` sends, `Escape` cancels
/// (`game.py`'s chat input). While typing the game must keep the keyboard.
#[derive(Resource, Default)]
pub struct ChatState {
    pub typing: bool,
    pub input: String,
}

#[derive(Component)]
struct TaskBar;
#[derive(Component)]
struct ChatLog;
#[derive(Component)]
struct ChatInput;
#[derive(Component)]
struct ChatLine;
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Gauge {
    Hp,
    Sp,
    Stamina,
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum TaskAction {
    Inventory,
    Character,
    System,
    Chat,
}
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum SystemAction {
    Exit,
    Close,
}

/// The client's `HPGauge`/`SPGauge`/`STGauge` are 95 px wide.
const GAUGE_WIDTH: f32 = 95.0;

pub struct PlayerHudPlugin;
impl Plugin for PlayerHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatState>()
            .add_systems(
                Update,
                (
                    lifecycle.in_set(UiSet::State),
                    input.in_set(UiSet::Input),
                    (task_actions, system_actions).in_set(UiSet::Actions),
                    refresh.in_set(UiSet::Draw),
                ),
            );
    }
}

fn lifecycle(
    mut commands: Commands,
    assets: Res<AssetServer>,
    screen: Res<LoginScreen>,
    roots: Query<Entity, With<GameUiRoot>>,
    mut chat: ResMut<ChatState>,
    mut inventory: ResMut<inventory::InventoryUi>,
    mut character_tab: ResMut<crate::character::CharacterTab>,
    mut manager: ResMut<crate::core::WindowManager>,
) {
    if !matches!(screen.status, Status::InGame { .. }) {
        for root in &roots {
            commands.entity(root).despawn();
        }
        if !roots.is_empty() {
            *chat = ChatState::default();
            inventory.reset();
            *character_tab = crate::character::CharacterTab::default();
            manager.modal = None;
        }
        return;
    }
    if !roots.is_empty() {
        return;
    }
    build_taskbar(&mut commands, &assets);
    build_chat(&mut commands);
    build_windows(&mut commands, &assets);
    inventory::build(&mut commands, &assets);
    crate::target::build(&mut commands, &assets);
}

/// The character status panel and the system menu behind the task bar's right
/// hand buttons.
fn build_windows(commands: &mut Commands, assets: &AssetServer) {
    crate::character::build(commands, assets);

    let system = core::window(
        commands,
        assets,
        "system",
        "System",
        Vec2::new(130., 120.),
        Vec2::new(20., 80.),
        core::Anchor::BottomRight(Vec2::new(20., 80.)),
        false,
    );
    commands.entity(system).insert(GameUiRoot);
    let exit = core::button(
        commands,
        assets,
        system,
        core::rect(15., 40., 100., 24.),
        "d:/ymir work/ui/public/large_button",
        "Exit",
    );
    commands.entity(exit).insert(SystemAction::Exit);
    let close = core::button(
        commands,
        assets,
        system,
        core::rect(15., 70., 100., 24.),
        "d:/ymir work/ui/public/large_button",
        "Close",
    );
    commands.entity(close).insert(SystemAction::Close);
}

fn build_taskbar(commands: &mut Commands, assets: &AssetServer) {
    let root = commands
        .spawn((
            GameUiRoot,
            TaskBar,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(0.),
                right: Val::Percent(0.),
                bottom: Val::Px(0.),
                height: Val::Px(37.),
                ..default()
            },
            Surface,
            Hovered::default(),
            GlobalZIndex(100),
        ))
        .id();

    // `Base_Board_01`: the tiled bar from the left gauge to the right buttons.
    let base = core::picture(
        commands,
        assets,
        root,
        "d:/ymir work/ui/pattern/TaskBar_Base.tga",
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(263.),
            right: Val::Px(256.),
            top: Val::Px(0.),
            height: Val::Px(37.),
            ..default()
        },
    );
    commands.entity(base).insert(ImageNode {
        image_mode: bevy::ui::widget::NodeImageMode::Tiled { tile_x: true, tile_y: false, stretch_value: 1. },
        ..default()
    });

    // `Gauge_Board` hangs 10 px above the bar.
    core::picture(
        commands,
        assets,
        root,
        "d:/ymir work/ui/game/taskbar/gauge.sub",
        core::rect(0., -10., 158., 47.),
    );
    core::picture(
        commands,
        assets,
        root,
        "d:/ymir work/ui/game/taskbar/exp_gauge.sub",
        core::rect(158., 0., 105., 37.),
    );
    spawn_gauge(commands, assets, root, Gauge::Hp, 4., 11., "d:/ymir work/ui/pattern/hpgauge/01.tga");
    spawn_gauge(commands, assets, root, Gauge::Sp, 14., 11., "d:/ymir work/ui/pattern/spgauge/01.tga");
    spawn_gauge(commands, assets, root, Gauge::Stamina, 28., 6., "d:/ymir work/ui/pattern/stgauge/01.tga");

    // `quickslot_board` is centred, 86 px right of the screen centre.
    let quick = commands
        .spawn((
            GameUiRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.),
                margin: UiRect::left(Val::Px(-86.)),
                top: Val::Px(0.),
                width: Val::Px(281.),
                height: Val::Px(37.),
                ..default()
            },
            GlobalZIndex(101),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(root).add_child(quick);

    let chat = core::button(commands, assets, quick, core::rect(128., 1., 14., 35.), "d:/ymir work/ui/game/taskbar/chat_button", "");
    commands.entity(chat).insert((TaskAction::Chat, Tooltip("Chat".into())));
    for index in 0..8u8 {
        let x = if index < 4 { index as f32 * 32. } else { 142. + (index - 4) as f32 * 32. };
        let slot = inventory::slot(commands, assets, quick, Slot::Quick(index), core::rect(x, 3., 32., 32.), true);
        let key = if index < 4 { format!("{}", index + 1) } else { format!("F{}", index - 3) };
        let label = core::text(commands, slot, key, core::rect(3., 1., 12., 10.));
        commands.entity(label).insert(TextFont { font_size: bevy::text::FontSize::Px(9.), ..default() });
    }
    inventory::page_button(commands, assets, quick, -1, core::rect(270., 12., 9., 5.));
    inventory::page_button(commands, assets, quick, 1, core::rect(270., 25., 9., 5.));
    let page = core::text(commands, quick, "1", core::rect(271., 16., 10., 11.));
    commands.entity(page).insert((inventory::QuickPageLabel, TextFont { font_size: bevy::text::FontSize::Px(9.), ..default() }));

    // The right-hand buttons of `taskbar.py`.
    for (right, prefix, label, action) in [
        (144., "character_button", "Character", TaskAction::Character),
        (110., "inventory_button", "Inventory", TaskAction::Inventory),
        (42., "system_button", "System", TaskAction::System),
    ] {
        let button = core::button(
            commands,
            assets,
            root,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(right),
                top: Val::Px(3.),
                width: Val::Px(32.),
                height: Val::Px(32.),
                ..default()
            },
            &format!("d:/ymir work/ui/game/taskbar/{prefix}"),
            "",
        );
        commands.entity(button).insert((action, Tooltip(label.into())));
    }
}

fn spawn_gauge(
    commands: &mut Commands,
    assets: &AssetServer,
    parent: Entity,
    gauge: Gauge,
    y: f32,
    height: f32,
    path: &str,
) {
    let image: Handle<Image> = assets.load(m2_assets::paths::asset_source_uri(path));
    let entity = commands
        .spawn((
            gauge,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(59.),
                top: Val::Px(y),
                width: Val::Px(GAUGE_WIDTH),
                height: Val::Px(height),
                ..default()
            },
            // The client crops the gauge with `SetRenderingRect`; the visible
            // part of the original `HPGauge`/`SPGauge`/`STGauge` texture is the
            // percentage, so crop its source rect and shrink the node alike.
            ImageNode {
                image,
                rect: Some(Rect::new(0., 0., GAUGE_WIDTH, height)),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(entity);
}

fn build_chat(commands: &mut Commands) {
    // A shadowed log in the lower-left corner, above the task bar.
    let log = commands
        .spawn((
            GameUiRoot,
            ChatLog,
            Text::new(""),
            TextFont { font_size: bevy::text::FontSize::Px(12.), ..default() },
            TextColor(Color::srgb(0.95, 0.92, 0.85)),
            TextShadow::default(),
            TextLayout::justify(Justify::Left),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(10.),
                bottom: Val::Px(42.),
                width: Val::Px(560.),
                height: Val::Px(120.),
                ..default()
            },
            GlobalZIndex(90),
            Pickable::IGNORE,
        ))
        .id();
    let input = commands
        .spawn((
            GameUiRoot,
            ChatInput,
            ChatLine,
            Text::new(""),
            TextFont { font_size: bevy::text::FontSize::Px(12.), ..default() },
            TextColor(Color::WHITE),
            TextShadow::default(),
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                left: Val::Px(10.),
                bottom: Val::Px(42.),
                width: Val::Px(560.),
                height: Val::Px(18.),
                padding: UiRect::horizontal(Val::Px(4.)),
                ..default()
            },
            BackgroundColor(Color::srgba(0., 0., 0., 0.55)),
            GlobalZIndex(91),
            Pickable::IGNORE,
        ))
        .id();
    let _ = (log, input);
}

fn input(
    mut keys: MessageReader<KeyboardInput>,
    screen: Res<LoginScreen>,
    mut chat: ResMut<ChatState>,
    mut consumed: ResMut<Consumed>,
    focus: Res<bevy::input_focus::InputFocus>,
    mut actions: MessageWriter<PlayerAction>,
) {
    if !matches!(screen.status, Status::InGame { .. }) {
        chat.typing = false;
        chat.input.clear();
        return;
    }
    // A text field of another window (the item quantity dialog) owns the keys.
    if focus.get().is_some() {
        return;
    }
    for key in keys.read().filter(|k| k.state == ButtonState::Pressed) {
        if chat.typing {
            consumed.keyboard = true;
            match key.key_code {
                KeyCode::Enter | KeyCode::NumpadEnter => {
                    let text = chat.input.trim().to_owned();
                    if !text.is_empty() {
                        actions.write(PlayerAction::Chat { text, kind: 0 });
                    }
                    chat.input.clear();
                    chat.typing = false;
                }
                KeyCode::Escape => {
                    chat.input.clear();
                    chat.typing = false;
                }
                KeyCode::Backspace => {
                    chat.input.pop();
                }
                _ => {
                    if let Some(text) = &key.text {
                        if !text.chars().any(char::is_control) && chat.input.len() + text.len() < 512 {
                            chat.input.push_str(text);
                        }
                    }
                }
            }
        } else if matches!(key.key_code, KeyCode::Enter | KeyCode::NumpadEnter) {
            chat.typing = true;
            consumed.keyboard = true;
        }
    }
}

fn task_actions(
    mut clicks: MessageReader<Click>,
    buttons: Query<&TaskAction>,
    mut windows: Query<&mut FloatingWindow>,
    mut chat: ResMut<ChatState>,
) {
    for click in clicks.read() {
        if click.button != bevy::picking::pointer::PointerButton::Primary {
            continue;
        }
        let Ok(action) = buttons.get(click.entity) else { continue };
        match action {
            TaskAction::Inventory => toggle(&mut windows, "inventory"),
            TaskAction::Character => toggle(&mut windows, "character"),
            TaskAction::System => toggle(&mut windows, "system"),
            TaskAction::Chat => chat.typing = true,
        }
    }
}

fn toggle(windows: &mut Query<&mut FloatingWindow>, id: &str) {
    for mut window in windows {
        if window.id == id {
            window.open = !window.open;
        }
    }
}

fn system_actions(
    mut clicks: MessageReader<Click>,
    buttons: Query<&SystemAction>,
    mut windows: Query<&mut FloatingWindow>,
    mut exit: MessageWriter<AppExit>,
) {
    for click in clicks.read() {
        let Ok(action) = buttons.get(click.entity) else { continue };
        match action {
            SystemAction::Exit => {
                exit.write(AppExit::Success);
            }
            SystemAction::Close => {
                for mut window in &mut windows {
                    if window.id == "system" {
                        window.open = false;
                    }
                }
            }
        }
    }
}

fn refresh(
    player: Res<PlayerState>,
    chat: Res<ChatState>,
    screen: Res<LoginScreen>,
    mut gauges: Query<(&Gauge, &mut Node, &mut ImageNode), Without<ChatLine>>,
    mut log: Query<&mut Text, (With<ChatLog>, Without<ChatLine>)>,
    mut line: Query<(&mut Text, &mut Node), (With<ChatLine>, Without<ChatLog>, Without<Gauge>)>,
) {
    if !matches!(screen.status, Status::InGame { .. }) {
        return;
    }
    let points = &player.points;
    for (gauge, mut node, mut image) in &mut gauges {
        let (value, max) = match gauge {
            Gauge::Hp => (points[5], points[6]),
            Gauge::Sp => (points[7], points[8]),
            Gauge::Stamina => (points[9], points[10]),
        };
        let ratio = if max > 0 { (value.max(0) as f32 / max as f32).clamp(0., 1.) } else { 0. };
        let width = GAUGE_WIDTH * ratio;
        node.width = Val::Px(width);
        if let Some(rect) = &mut image.rect {
            rect.max.x = width;
        }
    }
    for mut text in &mut log {
        let lines: Vec<String> = player
            .chat
            .iter()
            .rev()
            .take(7)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|line| line.text.clone())
            .collect();
        let value = lines.join("\n");
        if **text != value {
            **text = value;
        }
    }
    for (mut text, mut node) in &mut line {
        node.display = if chat.typing { Display::Flex } else { Display::None };
        if chat.typing {
            let value = format!("> {}_", chat.input);
            if **text != value {
                **text = value;
            }
        }
    }
}
