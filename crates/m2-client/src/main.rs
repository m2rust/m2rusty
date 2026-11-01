//! The client: the Bevy app that puts `m2-game` (network, world) and `m2-ui`
//! (screens) together. There is no Python: the UI is reimplemented in Rust.
//!
//! ```text
//! m2-client [--host <addr>] [--auth-port <port>] [--login <id> <password>] [--screenshot <out.png>]
//! ```

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use m2_game::net::{NetPlugin, ServerInfo};
use m2_ui::M2UiPlugin;
use m2_ui::login::Autologin;

/// Parsed command line.
#[derive(Debug, Default, PartialEq)]
struct Args {
    host: Option<String>,
    auth_port: Option<u16>,
    autologin: Autologin,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--host" => a.host = Some(it.next().ok_or("--host needs an address")?),
            "--auth-port" => {
                let port = it.next().ok_or("--auth-port needs a port")?;
                a.auth_port = Some(port.parse().map_err(|_| format!("bad port {port}"))?);
            }
            "--login" => {
                let id = it.next().ok_or("--login needs <id> <password>")?;
                let password = it.next().ok_or("--login needs <id> <password>")?;
                a.autologin.login = Some((id, password));
            }
            "--screenshot" => a.autologin.screenshot = Some(PathBuf::from(it.next().ok_or("--screenshot needs <out.png>")?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(a)
}

fn main() -> AppExit {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("m2-client: {err}");
            return AppExit::error();
        }
    };
    let mut server = ServerInfo::default();
    if let Some(host) = args.host {
        server.host = host;
    }
    if let Some(port) = args.auth_port {
        server.auth_port = port;
    }
    if let Some(out) = &args.autologin.screenshot {
        // The capture is detected by the file appearing.
        let _ = std::fs::remove_file(out);
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "m2rusty".into(), ..default() }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.1)))
        .insert_resource(server)
        .insert_resource(args.autologin)
        .add_plugins((NetPlugin, M2UiPlugin))
        .add_systems(Startup, setup)
        .run()
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: &[&str]) -> Result<Args, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn args() {
        assert_eq!(p(&[]).unwrap(), Args::default());
        let a = p(&["--host", "10.0.0.2", "--auth-port", "11002", "--login", "admin", "pw", "--screenshot", "o.png"]).unwrap();
        assert_eq!(a.host.as_deref(), Some("10.0.0.2"));
        assert_eq!(a.auth_port, Some(11002));
        assert_eq!(a.autologin.login, Some(("admin".into(), "pw".into())));
        assert_eq!(a.autologin.screenshot, Some(PathBuf::from("o.png")));
        assert!(p(&["--login", "admin"]).is_err());
        assert!(p(&["--auth-port", "x"]).is_err());
        assert!(p(&["what"]).is_err());
    }
}
