//! The game's user interface, in Rust.
//!
//! The original client draws its UI from Python (`root/*.py` drives windows laid
//! out by `uiscript/*.py`). The port does not run Python: each screen is
//! reimplemented here on top of the game state in `m2-game`, and the original
//! layout files are data the UI can read (they are dict literals with simple
//! expressions; see the roadmap in `rust_port.md`).
//!
//! Screens so far:
//! - [`login`]: account, password and channel.
//! - [`select`]: the account's characters in 3D; "Start" enters the game.
//!
//! The screens use egui for now (`bevy_egui`; stopgaps until the `bevy_ui` widget
//! tree, D18). The app adds `EguiPlugin`, the asset and render plugins, and a 3D
//! camera with `m2_game::SceneCamera` for the scenes.

pub mod login;
pub mod select;

use bevy::prelude::*;

/// Every screen of the UI.
pub struct M2UiPlugin;

impl Plugin for M2UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((login::LoginScreenPlugin, select::SelectScreenPlugin));
    }
}
