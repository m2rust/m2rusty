//! The game's user interface, in Rust.
//!
//! The original client draws its UI from Python (`root/*.py` drives windows laid
//! out by `uiscript/*.py`). The port does not run Python: each screen is
//! reimplemented here on top of the game state in `m2-game`, and the original
//! layout files are data the UI can read (they are dict literals with simple
//! expressions; see the roadmap in `rust_port.md`).
//!
//! Screens so far:
//! - [`login`]: account, password and channel; the account's characters once the
//!   server accepts the login.
//!
//! The screens use egui for now (`bevy_egui`); the app adds `EguiPlugin`.

pub mod login;

use bevy::prelude::*;

/// Every screen of the UI.
pub struct M2UiPlugin;

impl Plugin for M2UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(login::LoginScreenPlugin);
    }
}
