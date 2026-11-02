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
//! - [`frontend`]: the native login and character-select windows.
//! - [`hud`]: the in-game task bar, status and chat.
//! - [`inventory`]: the in-game inventory and equipment window.
//! - [`character`]: the tabbed character window (status, skills, emotions,
//!   quests), with [`skills`] and [`emotes`] for the two large pages.
//! - [`quickslots`]: the shared quick-slot actions.
//! - [`sound`]: screen music, login sounds and button clicks (`m2-audio`).
//! - [`target`]: the target board (name and HP of the selected actor).
//!
//! Everything is drawn with `bevy_ui` from the original `.sub`/`.tga` artwork
//! (D18); `core` has the shared widgets (boards, title bars, buttons, tooltips
//! and floating windows).

pub mod character;
pub mod core;
pub mod emotes;
pub mod frontend;
pub mod hud;
pub mod inventory;
pub mod login;
pub mod quickslots;
pub mod quest_dialog;
pub mod select;
pub mod skills;
pub mod sound;
pub mod shop;
pub mod storage;
pub mod target;

use bevy::prelude::*;

/// Every screen of the UI.
pub struct M2UiPlugin;

impl Plugin for M2UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<quickslots::QuickSlotUse>().add_plugins((
            core::UiCorePlugin,
            login::LoginScreenPlugin,
            select::SelectScreenPlugin,
            frontend::FrontendPlugin,
            hud::PlayerHudPlugin,
            inventory::InventoryPlugin,
            character::CharacterPlugin,
            skills::SkillsPlugin,
            emotes::EmotesPlugin,
            sound::UiSoundPlugin,
            shop::ShopPlugin,
            storage::StoragePlugin,
            target::TargetBoardPlugin,
            quest_dialog::QuestDialogPlugin,
        ));
    }
}
