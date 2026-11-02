//! The restart dialog (`uiRestart.RestartDialog`, `uiscript/restartdialog.py`)
//! shown while the main character is dead (`game.py`'s `OnGameOver`):
//! "Restart here" sends `/restart_here`, "Restart in the City" `/restart_town`.
//! It closes once the character is alive again.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use m2_game::ingame::PlayerControl;
use m2_game::net::Net;

use crate::core::{self, Click, Surface, UiSet};
use crate::hud::GameUiRoot;

/// `RestartDialog`'s `ThinBoard`: 200×88 at (50, 50).
const WIDTH: f32 = 200.0;
const HEIGHT: f32 = 88.0;

#[derive(Component)]
struct RestartRoot;
#[derive(Component, Clone, Copy)]
enum RestartButton {
    Here,
    Town,
}

pub struct RestartDialogPlugin;

impl Plugin for RestartDialogPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (press.in_set(UiSet::Actions), refresh.in_set(UiSet::Draw)));
    }
}

/// Spawns the (hidden) dialog with the rest of the in-game UI.
pub fn build(commands: &mut Commands, assets: &AssetServer) {
    let root = commands
        .spawn((
            RestartRoot,
            GameUiRoot,
            Surface,
            Hovered::default(),
            GlobalZIndex(160),
            Node { display: Display::None, ..core::rect(50.0, 50.0, WIDTH, HEIGHT) },
        ))
        .id();
    crate::target::thin_board(commands, assets, root);
    for (button, y, caption) in [(RestartButton::Here, 17.0, "Restart here"), (RestartButton::Town, 47.0, "Restart in the City")] {
        let id = core::button(commands, assets, root, core::rect(10.0, y, 180.0, 25.0), "d:/ymir work/ui/public/XLarge_Button", caption);
        commands.entity(id).insert(button);
    }
}

/// `RestartHere`/`RestartTown`.
fn press(mut clicks: MessageReader<Click>, buttons: Query<&RestartButton>, net: Option<ResMut<Net>>) {
    let Some(mut net) = net else { return };
    for click in clicks.read() {
        let Ok(button) = buttons.get(click.entity) else { continue };
        let command = match button {
            RestartButton::Here => "/restart_here",
            RestartButton::Town => "/restart_town",
        };
        net.game.send_chat(command, 0);
    }
}

/// `OpenRestartDialog` on death, `CloseRestartDialog` on revival.
fn refresh(control: Option<Res<PlayerControl>>, mut root: Query<&mut Node, With<RestartRoot>>) {
    let dead = control.is_some_and(|control| control.dead);
    let Ok(mut root) = root.single_mut() else { return };
    let display = if dead { Display::Flex } else { Display::None };
    if root.display != display {
        root.display = display;
    }
}
