//! Phase 2 asset viewer shell (docs/bevy-port-plan.md). See `README`-style
//! notes in [`viewer`] for the content-viewer plugin interface.

pub mod camera;
pub mod index;
pub mod map;
pub mod model;
pub mod speedtree;
pub mod texture;
pub mod tree;
pub mod ui;
pub mod viewer;

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use m2_assets::M2AssetsPlugin;
use m2_render::M2RenderPlugin;

/// Parsed command line: `m2-viewer [client dir] [--index <file>]
/// [--motion <vpath>] [--map <name>] [--screenshot <vpath> <out.png>]
/// [--screenshot-only <out.png>]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub client_dir: PathBuf,
    pub index: String,
    /// Motion played when the viewed model opens; `.msa` or motion-only `.gr2`.
    pub motion: Option<String>,
    /// Outdoor map to open on startup, e.g. `metin2_map_a1`.
    pub map: Option<String>,
    pub screenshot: Option<(String, PathBuf)>,
    /// Capture a screenshot without a client file to open (used with `--map`,
    /// which has no browser entry of its own).
    pub screenshot_only: Option<PathBuf>,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        client_dir: "bin".into(),
        index: "pack/Index.dev".into(),
        motion: None,
        map: None,
        screenshot: None,
        screenshot_only: None,
    };
    let mut it = args.into_iter();
    while let Some(s) = it.next() {
        match s.as_str() {
            "--screenshot" => {
                let v = it.next().ok_or("--screenshot needs <virtual path> <out.png>")?;
                let o = it.next().ok_or("--screenshot needs <virtual path> <out.png>")?;
                a.screenshot = Some((v.to_lowercase(), o.into()));
            }
            "--screenshot-only" => {
                a.screenshot_only = Some(it.next().ok_or("--screenshot-only needs <out.png>")?.into());
            }
            "--motion" => a.motion = Some(it.next().ok_or("--motion needs a virtual path")?.to_lowercase()),
            "--map" => {
                let name = it.next().ok_or("--map needs a map name (e.g. metin2_map_a1)")?;
                a.map = Some(m2_assets::paths::canonical_game_path(&name));
            }
            "--index" => a.index = it.next().ok_or("--index needs a file")?,
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            _ => a.client_dir = s.into(),
        }
    }
    Ok(a)
}

/// The viewer app with texture and Granny model/motion viewers registered.
pub fn build_app(args: &Args) -> App {
    use viewer::ViewerAppExt;
    let mut app = App::new();
    app.add_plugins(M2AssetsPlugin::new(args.client_dir.clone()).with_index(&args.index))
    .add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window { title: "Metin2 asset viewer".into(), ..default() }),
        ..default()
    }))
    .add_plugins(EguiPlugin::default())
    .add_plugins(M2RenderPlugin)
    .insert_resource(ClearColor(Color::srgb(0.12, 0.12, 0.14)))
    .insert_resource(ui::Shell::new(
        args.client_dir.clone(),
        args.index.clone(),
        // With a map the screenshot path is only the output file: opening it
        // would replace the map viewer in the same frame.
        args.screenshot.as_ref().filter(|_| args.map.is_none()).map(|s| s.0.clone()),
    ))
    .add_plugins((ui::ShellPlugin, camera::CameraPlugin, model::ModelViewerPlugin, map::MapViewerPlugin))
    .add_viewer(texture::TextureViewer::default())
    .add_viewer(speedtree::SpeedTreeViewer::default())
    .add_viewer(model::Gr2Viewer::default().with_motion(args.motion.clone()))
    .add_viewer(map::MapViewer::default());
    // `--map` has no browser entry, so its screenshot may omit the virtual
    // path; the map load itself queues the shell's `PendingOpen`.
    if let Some(map) = &args.map {
        app.world_mut().resource_mut::<map::MapPreload>().map = Some(map.clone());
    }
    let out = args.screenshot_only.clone().or_else(|| args.screenshot.as_ref().map(|(_, out)| out.clone()));
    if let Some(out) = out {
        app.insert_resource(ui::ScreenshotJob::new(out)).add_systems(Update, ui::screenshot_system);
    }
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: &[&str]) -> Result<Args, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn args() {
        assert_eq!(p(&[]).unwrap().client_dir, PathBuf::from("bin"));
        let a = p(&["x", "--screenshot", "D:/A.dds", "o.png"]).unwrap();
        assert_eq!(a.client_dir, PathBuf::from("x"));
        assert_eq!(a.screenshot.unwrap().0, "d:/a.dds");
        assert!(p(&["--screenshot", "a"]).is_err());
        assert!(p(&["--nope"]).is_err());
        let a = p(&["--motion", "D:/Ymir Work/PC/A/Wait.msa"]).unwrap();
        assert_eq!(a.motion.as_deref(), Some("d:/ymir work/pc/a/wait.msa"));
        assert!(p(&["--motion"]).is_err());
        let a = p(&["--map", "Metin2_Map_A1"]).unwrap();
        assert_eq!(a.map.as_deref(), Some("metin2_map_a1"));
        assert!(p(&["--map"]).is_err());
        let a = p(&["--screenshot-only", "o.png"]).unwrap();
        assert_eq!(a.screenshot_only, Some(PathBuf::from("o.png")));
        assert!(p(&["--screenshot-only"]).is_err());
    }
}
