//! The UI's sound calls (`snd.*` in `root/*.py`): the music of each screen
//! (`musicInfo.py`), the login result sounds (`intrologin.py`) and the button
//! click (`ui.Button.CallEvent`).

use bevy::prelude::*;
use bevy::ui_widgets::Button;
use m2_audio::SoundManager;

use crate::core::Click;
use crate::login::{LoginScreen, Status};

/// `musicInfo.loginMusic`.
pub const LOGIN_MUSIC: &str = "BGM/login_window.mp3";
/// `musicInfo.selectMusic`.
pub const SELECT_MUSIC: &str = "BGM/characterselect.mp3";
/// `musicInfo.fieldMusic` (`METIN2THEMA`). The server's per-map music
/// (`net.GetFieldMusicFileName`) and `BGM/lastplay.inf` are not used yet.
pub const FIELD_MUSIC: &str = "BGM/M2BG.mp3";

pub struct UiSoundPlugin;

impl Plugin for UiSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (screen_music, login_sounds, click_sounds));
    }
}

fn music_for(status: &Status) -> &'static str {
    match status {
        Status::Idle | Status::Working(_) | Status::Failed(_) => LOGIN_MUSIC,
        Status::Select { .. } | Status::Entering { .. } => SELECT_MUSIC,
        Status::InGame { .. } => FIELD_MUSIC,
    }
}

/// Each screen fades its music in on open and out on close (`introLogin`,
/// `introSelect`, `game`).
fn screen_music(login: Res<LoginScreen>, mut sounds: SoundManager, mut playing: Local<Option<&'static str>>) {
    let music = music_for(&login.status);
    if *playing == Some(music) {
        return;
    }
    if let Some(previous) = playing.replace(music) {
        sounds.fade_out_music(previous);
    }
    sounds.fade_in_music(music);
}

/// `loginfail.wav` when the login fails, `loginok.wav` when it gets through
/// (Python plays it on the account server's handshake; the port has the
/// character list as its first success).
fn login_sounds(login: Res<LoginScreen>, mut sounds: SoundManager, mut was_working: Local<bool>) {
    if !login.is_changed() {
        return;
    }
    let working = matches!(login.status, Status::Working(_));
    if std::mem::replace(&mut *was_working, working) {
        match login.status {
            Status::Failed(_) => sounds.play_2d("sound/ui/loginfail.wav"),
            Status::Select { .. } => sounds.play_2d("sound/ui/loginok.wav"),
            _ => {}
        }
    }
}

/// `ui.Button.CallEvent`: every button click.
fn click_sounds(mut clicks: MessageReader<Click>, buttons: Query<(), With<Button>>, mut sounds: SoundManager) {
    for click in clicks.read() {
        if buttons.contains(click.entity) {
            sounds.play_2d("sound/ui/click.wav");
        }
    }
}
