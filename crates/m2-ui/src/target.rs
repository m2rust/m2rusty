//! The target board (`uiTarget.TargetBoard`): the name of the selected actor
//! at the top of the screen and, for monsters and stones, the HP the server
//! reports (`GC_TARGET`, `game.py`'s `SetHPTargetBoard`).
//!
//! Layout, as `uitarget.py` builds it: a `ThinBoard` 250×40 with the name
//! centred; once an HP arrives it is `200 + 7·len(name)` wide, the name moves
//! to `(23, 13)` and a red 130-pixel `Gauge` sits 175 pixels from the right
//! edge at `y = 17`; the close button is 30 pixels from the right at
//! `y = 13`. The board is centred 10 pixels below the top of the screen.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use m2_game::ingame::{ClearTarget, TargetBoard};

use crate::core::{self, Click, Surface, UiSet};
use crate::hud::GameUiRoot;

const HEIGHT: f32 = 40.0;
/// `ThinBoard.CORNER_WIDTH`.
const CORNER: f32 = 16.0;
/// `MakeGauge(130, "red")`.
const GAUGE_WIDTH: f32 = 130.0;
/// `Gauge.SLOT_WIDTH`, `SLOT_HEIGHT`, `GAUGE_TEMPORARY_PLACE`.
const SLOT_WIDTH: f32 = 16.0;
const SLOT_HEIGHT: f32 = 7.0;
const GAUGE_PLACE: f32 = 12.0;

#[derive(Component)]
struct TargetRoot;
#[derive(Component)]
struct TargetName;
#[derive(Component)]
struct TargetGauge;
#[derive(Component)]
struct TargetGaugeBar;
#[derive(Component)]
struct TargetClose;

pub struct TargetBoardPlugin;

impl Plugin for TargetBoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (close.in_set(UiSet::Actions), refresh.in_set(UiSet::Draw)));
    }
}

/// A `ThinBoard`'s corners, edges and translucent base, stretched with the
/// node.
pub(crate) fn thin_board(commands: &mut Commands, assets: &AssetServer, parent: Entity) {
    let tiled = |x: bool, y: bool| ImageNode {
        image_mode: NodeImageMode::Tiled { tile_x: x, tile_y: y, stretch_value: 1.0 },
        ..default()
    };
    let base = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(CORNER),
                right: Val::Px(CORNER),
                top: Val::Px(CORNER),
                bottom: Val::Px(CORNER),
                ..default()
            },
            // `BOARD_COLOR`: black at 0.51.
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.51)),
            Pickable::IGNORE,
        ))
        .id();
    commands.entity(parent).add_child(base);
    let dir = "d:/ymir work/ui/pattern/";
    let edge = |left: Option<f32>, right: Option<f32>, top: Option<f32>, bottom: Option<f32>, w: Option<f32>, h: Option<f32>| Node {
        position_type: PositionType::Absolute,
        left: left.map_or(Val::Auto, Val::Px),
        right: right.map_or(Val::Auto, Val::Px),
        top: top.map_or(Val::Auto, Val::Px),
        bottom: bottom.map_or(Val::Auto, Val::Px),
        width: w.map_or(Val::Auto, Val::Px),
        height: h.map_or(Val::Auto, Val::Px),
        ..default()
    };
    let c = Some(CORNER);
    let zero = Some(0.0);
    for (name, node, image) in [
        ("ThinBoard_Line_Left", edge(zero, None, c, c, c, None), tiled(false, true)),
        ("ThinBoard_Line_Right", edge(None, zero, c, c, c, None), tiled(false, true)),
        ("ThinBoard_Line_Top", edge(c, c, zero, None, None, c), tiled(true, false)),
        ("ThinBoard_Line_Bottom", edge(c, c, None, zero, None, c), tiled(true, false)),
        ("ThinBoard_Corner_LeftTop", edge(zero, None, zero, None, c, c), ImageNode::default()),
        ("ThinBoard_Corner_RightTop", edge(None, zero, zero, None, c, c), ImageNode::default()),
        ("ThinBoard_Corner_LeftBottom", edge(zero, None, None, zero, c, c), ImageNode::default()),
        ("ThinBoard_Corner_RightBottom", edge(None, zero, None, zero, c, c), ImageNode::default()),
    ] {
        let id = core::picture(commands, assets, parent, &format!("{dir}{name}.tga"), node);
        commands.entity(id).insert(image);
    }
}

/// Spawns the (hidden) board with the rest of the in-game UI.
pub fn build(commands: &mut Commands, assets: &AssetServer) {
    let root = commands
        .spawn((
            TargetRoot,
            GameUiRoot,
            Surface,
            Hovered::default(),
            GlobalZIndex(150),
            Node { display: Display::None, ..core::rect(0.0, 10.0, 250.0, HEIGHT) },
        ))
        .id();
    thin_board(commands, assets, root);
    let name = core::text(commands, root, "", core::rect(0.0, 13.0, 250.0, 14.0));
    commands.entity(name).insert(TargetName);

    let gauge = commands
        .spawn((TargetGauge, Pickable::IGNORE, Node { display: Display::None, ..core::rect(0.0, 17.0, GAUGE_WIDTH, SLOT_HEIGHT) }))
        .id();
    commands.entity(root).add_child(gauge);
    let dir = "d:/ymir work/ui/pattern/";
    core::picture(commands, assets, gauge, &format!("{dir}gauge_slot_left.tga"), core::rect(0.0, 0.0, SLOT_WIDTH, SLOT_HEIGHT));
    core::picture(
        commands,
        assets,
        gauge,
        &format!("{dir}gauge_slot_right.tga"),
        core::rect(GAUGE_WIDTH - SLOT_WIDTH, 0.0, SLOT_WIDTH, SLOT_HEIGHT),
    );
    let centre = core::picture(
        commands,
        assets,
        gauge,
        &format!("{dir}gauge_slot_center.tga"),
        core::rect(SLOT_WIDTH, 0.0, GAUGE_WIDTH - 2.0 * SLOT_WIDTH, SLOT_HEIGHT),
    );
    commands.entity(centre).insert(ImageNode {
        image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: false, stretch_value: 1.0 },
        ..default()
    });
    let bar = core::picture(commands, assets, gauge, &format!("{dir}gauge_red.tga"), core::rect(GAUGE_PLACE, 0.0, 0.0, SLOT_HEIGHT));
    commands.entity(bar).insert((
        TargetGaugeBar,
        ImageNode { image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: false, stretch_value: 1.0 }, ..default() },
    ));

    let close = core::button(commands, assets, root, core::rect(250.0 - 30.0, 13.0, 16.0, 16.0), "d:/ymir work/ui/public/close_button", "");
    commands.entity(close).insert(TargetClose);
}

/// The close button: `OnPressedCloseButton` -> `player.ClearTarget()`.
fn close(mut clicks: MessageReader<Click>, buttons: Query<(), With<TargetClose>>, mut clear: MessageWriter<ClearTarget>) {
    if clicks.read().any(|click| buttons.contains(click.entity)) {
        clear.write(ClearTarget);
    }
}

/// Mirrors [`TargetBoard`] into the widgets: `ResetTargetBoard`, `SetHP` and
/// `UpdatePosition`.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn refresh(
    board: Res<TargetBoard>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Res<UiScale>,
    mut root: Query<&mut Node, (With<TargetRoot>, Without<TargetName>, Without<TargetGauge>, Without<TargetGaugeBar>, Without<TargetClose>)>,
    mut name: Query<(&mut Text, &mut Node, &mut TextLayout), (With<TargetName>, Without<TargetGauge>, Without<TargetGaugeBar>, Without<TargetClose>)>,
    mut gauge: Query<&mut Node, (With<TargetGauge>, Without<TargetGaugeBar>, Without<TargetClose>)>,
    mut bar: Query<&mut Node, (With<TargetGaugeBar>, Without<TargetClose>)>,
    mut close: Query<&mut Node, With<TargetClose>>,
) {
    let Ok(mut root) = root.single_mut() else { return };
    if !board.visible {
        if root.display != Display::None {
            root.display = Display::None;
        }
        return;
    }
    if !board.is_changed() && !scale.is_changed() {
        return;
    }
    let width = match board.hp_percent {
        // `SetHP`: `200 + 7 * nameLength`.
        Some(_) => 200.0 + 7.0 * board.name.chars().count() as f32,
        None => 250.0,
    };
    let viewport = windows.single().map_or(800.0, |window| window.width() / scale.0);
    root.display = Display::Flex;
    root.width = Val::Px(width);
    root.left = Val::Px((viewport / 2.0 - width / 2.0).floor());
    if let Ok((mut text, mut node, mut layout)) = name.single_mut() {
        if text.0 != board.name {
            text.0.clone_from(&board.name);
        }
        if board.hp_percent.is_some() {
            node.left = Val::Px(23.0);
            node.width = Val::Px(width - 23.0);
            layout.justify = Justify::Left;
        } else {
            node.left = Val::Px(0.0);
            node.width = Val::Px(width);
            layout.justify = Justify::Center;
        }
    }
    if let Ok(mut gauge) = gauge.single_mut() {
        gauge.display = if board.hp_percent.is_some() { Display::Flex } else { Display::None };
        // `SetWindowHorizontalAlignRight` + `SetPosition(175, 17)`.
        gauge.left = Val::Px(width - 175.0);
    }
    if let Ok(mut bar) = bar.single_mut() {
        // `SetPercentage`: the bar shows `(width - 24) * percentage` pixels.
        let percentage = f32::from(board.hp_percent.unwrap_or(0).min(100)) / 100.0;
        bar.width = Val::Px(((GAUGE_WIDTH - GAUGE_PLACE * 2.0) * percentage).floor());
    }
    if let Ok(mut close) = close.single_mut() {
        close.left = Val::Px(width - 30.0);
    }
}
