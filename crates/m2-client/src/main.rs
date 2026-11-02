//! The client: the Bevy app that puts `m2-game` (network, world, drawing) and
//! `m2-ui` (screens) together, with `m2-audio` for sound. There is no Python:
//! the UI is reimplemented in Rust (D17).
//!
//! ```text
//! m2-client [--client <dir>] [--host <addr>] [--auth-port <port>]
//!           [--login <id> <password>] [--enter <slot>] [--demo]
//!           [--screenshot <out.png>] [--vsync]
//! ```
//!
//! `--client` is the folder with `pack/` (default `bin`).

use std::path::PathBuf;

use bevy::prelude::*;
use m2_assets::M2AssetsPlugin;
use m2_audio::{AudioConfig, M2AudioPlugin};
use m2_game::net::{NetPlugin, ServerInfo};
use m2_game::{M2GamePlugin, SceneCamera};
use m2_render::M2RenderPlugin;
use m2_ui::M2UiPlugin;
use m2_ui::login::Autologin;

/// Parsed command line.
#[derive(Debug, PartialEq)]
struct Args {
    client_dir: PathBuf,
    host: Option<String>,
    auth_port: Option<u16>,
    autologin: Autologin,
    vsync: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            client_dir: "bin".into(),
            host: None,
            auth_port: None,
            autologin: Autologin::default(),
            vsync: false,
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut a = Args::default();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--client" => a.client_dir = it.next().ok_or("--client needs a folder")?.into(),
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
            "--enter" => {
                let slot = it.next().ok_or("--enter needs a slot (0-3)")?;
                a.autologin.enter = Some(slot.parse().map_err(|_| format!("bad slot {slot}"))?);
            }
            "--demo" => a.autologin.demo = true,
            "--vsync" => a.vsync = true,
            "--screenshot" => {
                a.autologin.screenshot = Some(PathBuf::from(
                    it.next().ok_or("--screenshot needs <out.png>")?,
                ))
            }
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

    // `CPythonSystem::LoadConfig`: the volumes the sound manager starts with.
    let audio = std::fs::read(args.client_dir.join("metin2.cfg")).map(|cfg| AudioConfig::parse(&cfg)).unwrap_or_default();

    let mut app = App::new();
    app.add_plugins(M2AssetsPlugin::new(args.client_dir))
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "m2rusty".into(),
                present_mode: if args.vsync { bevy::window::PresentMode::AutoVsync } else { bevy::window::PresentMode::AutoNoVsync },
                ..default()
            }),
            ..default()
        }))
        .add_plugins((M2RenderPlugin, M2AudioPlugin::new(audio), M2GamePlugin))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.1)))
        .insert_resource(server)
        .insert_resource(args.autologin)
        .add_plugins((NetPlugin, M2UiPlugin))
        .add_systems(Startup, setup);
    app.run()
}

/// The scene camera (3D, the client's fixed-function colours) and the native
/// Bevy UI camera over it.
///
/// The scene is display-space: the game's fixed-function materials and the
/// terrain shader write the environment's gamma-space colours directly, so
/// the camera disables tonemapping and the exposure is fixed at 1.0
/// (`ev100 = -log2(1.2)`). This is also what the Bevy light mapping in
/// `m2_game::render::lighting` assumes: illuminance π and
/// `Color::srgb` reproduce the authored colour on a white surface. The
/// environment lights themselves are installed per map
/// (`MapScene::spawn_with_fog`) and the neutral fallback by
/// `M2GamePlugin`'s `LightingPlugin`.
fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        // The custom materials write display-space colours.
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        bevy::render::view::Msaa::Off,
        bevy::camera::Exposure {
            ev100: -(1.2f32).log2(),
        },
        SceneCamera,
        // CPU batching is faster here than indirect drawing: the scene has a
        // few hundred draws and the GPU is far from saturated, while Bevy's
        // indirect-draw preparation shows up on the CPU side.
        bevy::render::view::NoIndirectDrawing,
        // The fixed-function materials never read Bevy's clustered lights, so
        // cluster assignment would be wasted per-frame work. `None` needs the
        // CPU clustering path that `M2RenderPlugin` selects (Bevy 0.20's GPU
        // clustering makes a zero-sized texture for it).
        bevy::light::cluster::ClusterConfig::None,
        Transform::from_xyz(0.0, 150.0, -520.0).looking_at(Vec3::new(0.0, 90.0, 0.0), Vec3::Y),
    ));
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            output_mode: bevy::camera::CameraOutputMode::Write {
                blend_state: Some(bevy::render::render_resource::BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
    // No hard-coded sun: the map environment owns the scene lighting.
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
        let a = p(&[
            "--client",
            "c",
            "--host",
            "10.0.0.2",
            "--auth-port",
            "11002",
            "--login",
            "admin",
            "pw",
            "--enter",
            "1",
            "--demo",
            "--screenshot",
            "o.png",
        ])
        .unwrap();
        assert_eq!(a.client_dir, PathBuf::from("c"));
        assert_eq!(a.host.as_deref(), Some("10.0.0.2"));
        assert_eq!(a.auth_port, Some(11002));
        assert_eq!(a.autologin.login, Some(("admin".into(), "pw".into())));
        assert_eq!(a.autologin.enter, Some(1));
        assert!(a.autologin.demo);
        assert_eq!(a.autologin.screenshot, Some(PathBuf::from("o.png")));
        assert!(p(&["--login", "admin"]).is_err());
        assert!(p(&["--auth-port", "x"]).is_err());
        assert!(p(&["--enter", "x"]).is_err());
        assert!(p(&["what"]).is_err());
    }
}
