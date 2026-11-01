//! Native login screen (egui): account, password and channel, and the progress
//! of the login. The Rust counterpart of `intrologin.py`; once the server
//! accepts the account, [`crate::select`] takes over.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use m2_game::net::{LoginRequest, Net, NetMessage, Server, ServerInfo, login_failure_text};
use m2_net::auth::AuthEvent;
use m2_net::phase::NetEvent;
use m2_net::wire::cstr_to_string;

/// One character slot of the account (`TSimplePlayerInformation`).
#[derive(Clone, Debug)]
pub struct CharacterSlot {
    pub slot: u8,
    pub name: String,
    pub race: u8,
    pub level: u8,
    /// `wMainPart`: the armor (an item vnum, or a shape for 0 and 1).
    pub armor: u16,
    /// `wHairPart`: the hair index of the race.
    pub hair: u16,
    pub play_minutes: u32,
}

/// Where the player is, from the login form to the game.
#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Idle,
    Working(String),
    Failed(String),
    /// Logged in: the account's characters (the select screen).
    Select { empire: u8, characters: Vec<CharacterSlot> },
    /// "Start" was pressed for `slot`; `step` says what the client waits for.
    Entering { slot: u8, step: String },
    /// The server switched to the Game phase.
    InGame { name: String, x: i32, y: i32 },
}

/// The form and what the login reached.
#[derive(Resource, Default)]
pub struct LoginScreen {
    pub id: String,
    pub password: String,
    pub channel: usize,
    pub status: Status,
}

/// Unattended runs: `--login <id> <password>` submits once at startup, `--enter
/// <slot>` starts the game with that slot once the select screen is ready, and
/// `--screenshot <png>` saves the window when the run is done (select screen ready
/// without `--enter`, in game, or failed), then exits.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct Autologin {
    pub login: Option<(String, String)>,
    pub enter: Option<u8>,
    pub screenshot: Option<PathBuf>,
}

/// Set by a screen when an unattended run has reached what it waited for.
#[derive(Resource, Default)]
pub struct RunDone(pub bool);

pub struct LoginScreenPlugin;

impl Plugin for LoginScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoginScreen>()
            .init_resource::<Autologin>()
            .init_resource::<RunDone>()
            .add_systems(Startup, autologin)
            .add_systems(Update, (track_login, screenshot_when_done))
            .add_systems(EguiPrimaryContextPass, login_ui);
    }
}

fn autologin(auto: Res<Autologin>, mut screen: ResMut<LoginScreen>, mut requests: MessageWriter<LoginRequest>) {
    if let Some((id, password)) = &auto.login {
        screen.id = id.clone();
        screen.status = Status::Working("Connecting to the account server…".into());
        requests.write(LoginRequest { id: id.clone(), password: password.clone(), channel: screen.channel });
    }
}

/// Follows the login (and the connection's failures) through the network messages.
fn track_login(mut messages: MessageReader<NetMessage>, mut screen: ResMut<LoginScreen>, mut net: ResMut<Net>, info: Res<ServerInfo>) {
    for message in messages.read() {
        match message {
            NetMessage::ConnectFailed { server, error } => {
                screen.status = Status::Failed(format!("Could not connect to the {} server: {error}", server_name(*server)));
            }
            NetMessage::Disconnected { server } => {
                screen.status = Status::Failed(format!("The {} server closed the connection.", server_name(*server)));
            }
            NetMessage::Auth(AuthEvent::Handshake { .. }) => screen.status = Status::Working("Checking the account…".into()),
            NetMessage::Auth(AuthEvent::Success { .. }) => {
                let channel = info.channels.get(screen.channel).map_or("the game server", |c| c.name.as_str());
                screen.status = Status::Working(format!("Connecting to {channel}…"));
            }
            NetMessage::Auth(AuthEvent::Failure { status }) | NetMessage::Game(NetEvent::LoginFailure { status }) => {
                // `__DisconnectAndInputPassword`.
                net.disconnect();
                screen.password.clear();
                screen.status = Status::Failed(login_failure_text(status));
            }
            NetMessage::Auth(AuthEvent::MatrixCard { .. } | AuthEvent::RunupMatrixQuiz(_) | AuthEvent::PasspodRequest) => {
                net.disconnect();
                screen.status = Status::Failed("This server asks for a matrix card or passpod, which the port does not support.".into());
            }
            // The login while entering the game (direct-enter mode) refreshes nothing.
            NetMessage::Game(NetEvent::LoginSuccess { players, .. }) if net.game.direct_enter_slot().is_none() => {
                let characters = players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.dwID != 0)
                    .map(|(slot, p)| CharacterSlot {
                        slot: slot as u8,
                        name: cstr_to_string(&p.szName),
                        race: p.byJob,
                        level: p.byLevel,
                        armor: p.wMainPart,
                        hair: p.wHairPart,
                        play_minutes: p.dwPlayMinutes,
                    })
                    .collect();
                info!("logged in as {}", screen.id);
                screen.password.clear();
                screen.status = Status::Select { empire: net.game.empire(), characters };
            }
            NetMessage::Game(NetEvent::Empire(empire)) => {
                if let Status::Select { empire: shown, .. } = &mut screen.status {
                    *shown = *empire;
                }
            }
            _ => {}
        }
    }
}

fn server_name(server: Server) -> &'static str {
    match server {
        Server::Account => "account",
        Server::Game => "game",
    }
}

fn login_ui(
    mut contexts: EguiContexts,
    mut screen: ResMut<LoginScreen>,
    net: Res<Net>,
    info: Res<ServerInfo>,
    mut requests: MessageWriter<LoginRequest>,
) -> Result {
    if !matches!(screen.status, Status::Idle | Status::Working(_) | Status::Failed(_)) {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    egui::Window::new(format!("{} — login", info.name))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let screen = &mut *screen;
            let working = matches!(screen.status, Status::Working(_)) || net.busy();
            ui.add_enabled_ui(!working, |ui| {
                egui::Grid::new("form").num_columns(2).show(ui, |ui| {
                    ui.label("ID");
                    ui.text_edit_singleline(&mut screen.id);
                    ui.end_row();
                    ui.label("Password");
                    let password = ui.add(egui::TextEdit::singleline(&mut screen.password).password(true));
                    ui.end_row();
                    ui.label("Channel");
                    let selected = info.channels.get(screen.channel).map_or("?", |c| c.name.as_str());
                    egui::ComboBox::from_id_salt("channel").selected_text(selected).show_ui(ui, |ui| {
                        for (index, channel) in info.channels.iter().enumerate() {
                            ui.selectable_value(&mut screen.channel, index, format!("{} ({})", channel.name, channel.port));
                        }
                    });
                    ui.end_row();
                    let submit = ui.button("Log in").clicked()
                        || (password.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                    if submit && !screen.id.is_empty() {
                        screen.status = Status::Working("Connecting to the account server…".into());
                        requests.write(LoginRequest {
                            id: screen.id.clone(),
                            password: screen.password.clone(),
                            channel: screen.channel,
                        });
                    }
                    ui.end_row();
                });
            });
            ui.label(format!("Account server {}:{}", info.host, info.auth_port));
            match &screen.status {
                Status::Working(text) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(text);
                    });
                }
                Status::Failed(text) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, text);
                }
                _ => {}
            }
        });
    Ok(())
}

/// With `--screenshot`: capture once the run is done (or failed), then exit.
fn screenshot_when_done(
    mut commands: Commands,
    auto: Res<Autologin>,
    screen: Res<LoginScreen>,
    done: Res<RunDone>,
    mut frames: Local<u32>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(out) = &auto.screenshot else { return };
    if !done.0 && !matches!(screen.status, Status::Failed(_)) {
        *frames = 0;
        return;
    }
    *frames += 1;
    // A few frames for egui to lay the result out, then the capture, then the write.
    if *frames == 10 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(out.clone()));
    } else if *frames > 10 && out.exists() {
        info!("screenshot written to {}", out.display());
        exit.write(AppExit::Success);
    } else if *frames > 600 {
        error!("screenshot timed out");
        exit.write(AppExit::error());
    }
}

/// `MAIN_RACE_*`: 0 warrior (m), 1 assassin (f), 2 sura (m), 3 shaman (f), then the
/// other gender of each in the same order.
pub fn race_name(race: u8) -> &'static str {
    match race {
        0 => "Warrior (m)",
        1 => "Ninja (f)",
        2 => "Sura (m)",
        3 => "Shaman (f)",
        4 => "Warrior (f)",
        5 => "Ninja (m)",
        6 => "Sura (f)",
        7 => "Shaman (m)",
        _ => "Unknown race",
    }
}

pub fn empire_name(empire: u8) -> &'static str {
    match empire {
        1 => "Shinsoo",
        2 => "Chunjo",
        3 => "Jinno",
        _ => "no empire yet",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn races_follow_main_race_order() {
        assert_eq!(race_name(0), "Warrior (m)");
        assert_eq!(race_name(5), "Ninja (m)");
        assert_eq!(race_name(7), "Shaman (m)");
        assert_eq!(empire_name(2), "Chunjo");
    }
}
