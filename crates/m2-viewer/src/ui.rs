//! The shell UI (egui): pack browser, inspector, HUD; plus the background
//! pack load and the `--screenshot` job.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, futures::check_ready};
use bevy_egui::{EguiContext, EguiPrimaryContextPass, PrimaryEguiContext, egui};

use crate::index::{FileKind, LoadedPack, load_pack};
use crate::tree::{Filter, NodeKey, Row, Tree, ViewMode, visible_rows};
use crate::viewer::{CurrentFile, MainCamera, OpenFile, PendingOpen, ViewerRegistry, drive_viewers};

type LoadResult = Result<(LoadedPack, Tree), String>;

pub struct Browser {
    pub pack: Arc<LoadedPack>,
    pub tree: Tree,
}

#[derive(Resource)]
pub struct Shell {
    pub client_dir: PathBuf,
    pub index: String,
    task: Option<Task<LoadResult>>,
    pub error: Option<String>,
    pub browser: Option<Browser>,
    pub filter: Filter,
    pub mode: ViewMode,
    pub expanded: HashSet<NodeKey>,
    rows: Vec<Row>,
    rows_dirty: bool,
    pub selected: Option<u32>,
    pub open_error: Option<String>,
    inspect: Option<Inspect>,
    /// `--screenshot`: virtual path to open as soon as the index is loaded.
    pub cli_open: Option<String>,
}

struct Inspect {
    name: String,
    size: usize,
    hex: String,
    text: Option<String>,
}

impl Shell {
    pub fn new(client_dir: PathBuf, index: String, cli_open: Option<String>) -> Self {
        Shell {
            client_dir,
            index,
            task: None,
            error: None,
            browser: None,
            filter: Filter::default(),
            mode: ViewMode::Folders,
            expanded: HashSet::new(),
            rows: Vec::new(),
            rows_dirty: true,
            selected: None,
            open_error: None,
            inspect: None,
            cli_open,
        }
    }
}

pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentFile>()
            .init_resource::<PendingOpen>()
            .init_resource::<ViewerRegistry>()
            .add_systems(Startup, start_load)
            .add_systems(Update, (poll_load, drive_viewers).chain())
            .add_systems(EguiPrimaryContextPass, ui_system);
    }
}

fn start_load(mut shell: ResMut<Shell>) {
    let dir = shell.client_dir.clone();
    let index = shell.index.clone();
    shell.task = Some(AsyncComputeTaskPool::get().spawn(async move {
        let pack = load_pack(&dir, &index)?;
        let tree = Tree::build(&pack.entries);
        Ok((pack, tree))
    }));
}

fn poll_load(mut shell: ResMut<Shell>) {
    let Some(task) = &mut shell.task else { return };
    let Some(result) = check_ready(task) else { return };
    shell.task = None;
    match result {
        Ok((pack, tree)) => {
            info!("pack index: {} files from {} packs ({} failed)", pack.entries.len(), pack.registered, pack.failed.len());
            for f in &pack.failed {
                warn!("pack failed: {f}");
            }
            shell.browser = Some(Browser { pack: Arc::new(pack), tree });
            shell.rows_dirty = true;
        }
        Err(e) => {
            error!("{e}");
            shell.error = Some(e);
        }
    }
}

fn hex_dump(bytes: &[u8], n: usize) -> String {
    let mut s = String::new();
    for (i, chunk) in bytes[..bytes.len().min(n)].chunks(16).enumerate() {
        s += &format!("{:06x}  ", i * 16);
        for j in 0..16 {
            match chunk.get(j) {
                Some(b) => s += &format!("{b:02x} "),
                None => s += "   ",
            }
        }
        s.push(' ');
        s.extend(chunk.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }));
        s.push('\n');
    }
    s
}

/// Text preview rule: a text-kind file, or any non-image/model file without NULs at its start.
fn text_preview(kind: FileKind, bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(1024)];
    let texty = kind == FileKind::Text || (matches!(kind, FileKind::Other) && !head.contains(&0));
    // D14: UTF-8 shown as is; legacy bytes via lossy decoding for now.
    texty.then(|| String::from_utf8_lossy(&bytes[..bytes.len().min(32 * 1024)]).into_owned())
}

fn open_entry(world: &mut World, shell: &mut Shell, idx: u32) {
    let Some(b) = &shell.browser else { return };
    let entry = &b.pack.entries[idx as usize];
    shell.selected = Some(idx);
    match b.pack.manager.get(&entry.name) {
        Ok(bytes) => {
            shell.open_error = None;
            shell.inspect = Some(Inspect {
                name: entry.display().to_string(),
                size: bytes.len(),
                hex: hex_dump(&bytes, 256),
                text: text_preview(entry.kind, &bytes),
            });
            world.resource_mut::<PendingOpen>().0 = Some(OpenFile {
                name: entry.name.clone(),
                display: entry.display().to_string(),
                ext: entry.ext().to_string(),
                kind: entry.kind,
                bytes: Arc::new(bytes),
                manager: b.pack.manager.clone(),
            });
        }
        Err(e) => shell.open_error = Some(format!("{}: {e}", entry.name)),
    }
}

fn ui_system(world: &mut World) {
    let Ok(ctx) = world.query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>().single_mut(world).map(|mut c| c.get_mut().clone()) else {
        return;
    };
    world.resource_scope(|world, mut shell: Mut<Shell>| {
        world.resource_scope(|world, mut reg: Mut<ViewerRegistry>| {
            let mut want_open = None;

            // CLI: open the requested file once the index is ready.
            if shell.browser.is_some() && shell.cli_open.is_some() {
                let want = shell.cli_open.take().unwrap();
                let hit = shell.browser.as_ref().unwrap().pack.entries.iter().position(|e| e.name == want || e.display() == want);
                match hit {
                    Some(i) => want_open = Some(i as u32),
                    None => {
                        error!("--screenshot: no such virtual file: {want}");
                        world.write_message(AppExit::error());
                    }
                }
            }

            let mut root = egui::Ui::new(
                ctx.clone(),
                "viewport".into(),
                egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.content_rect()),
            );

            egui::Panel::left("browser").resizable(true).default_size(380.0).show(&mut root, |ui| {
                browser_ui(ui, &mut shell, &mut want_open);
            });

            egui::Panel::right("inspector").resizable(true).default_size(340.0).show(&mut root, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    ui.heading("Inspector");
                    if let Some(e) = &shell.open_error {
                        ui.colored_label(egui::Color32::LIGHT_RED, e);
                    }
                    match &shell.inspect {
                        None => {
                            ui.label("Select a file in the pack browser.");
                        }
                        Some(i) => {
                            ui.label(format!("Name: {}", i.name));
                            if let Some(cur) = &world.resource::<CurrentFile>().0 {
                                ui.label(format!("Type: {} ({})", cur.kind.label(), cur.ext));
                            }
                            ui.label(format!("Size: {} bytes", i.size));
                            ui.label(match reg.active {
                                Some(a) => format!("Viewer: {}", reg.viewers[a].id()),
                                None => "Viewer: none registered for this type".to_string(),
                            });
                            ui.separator();
                            ui.label("First bytes");
                            ui.add(egui::Label::new(egui::RichText::new(&i.hex).monospace().size(11.0)).extend());
                            if let Some(t) = &i.text {
                                ui.separator();
                                ui.label("Text preview (UTF-8, lossy for legacy bytes)");
                                ui.add(egui::Label::new(egui::RichText::new(t).monospace().size(11.0)).extend());
                            }
                        }
                    }
                    if let Some(a) = reg.active {
                        reg.viewers[a].ui(world, ui);
                    }
                });
            });

            let scene = root.available_rect_before_wrap();
            // The 3D camera renders into what the panels leave free.
            let ppp = ctx.pixels_per_point();
            if let Ok(mut cam) = world.query_filtered::<&mut Camera, With<MainCamera>>().single_mut(world) {
                cam.viewport = Some(bevy::camera::Viewport {
                    physical_position: UVec2::new((scene.min.x * ppp) as u32, (scene.min.y * ppp) as u32),
                    physical_size: UVec2::new((scene.width() * ppp).max(1.0) as u32, (scene.height() * ppp).max(1.0) as u32),
                    ..default()
                });
            }
            egui::Area::new("hud".into()).fixed_pos(scene.min + egui::vec2(8.0, 8.0)).interactable(false).show(&ctx, |ui| {
                let t = "Scene: Bevy convention, Y up (client data is Z up; loaders convert later)\n\
                         Orbit: LMB drag / arrows   Pan: RMB or MMB drag / WASD QE   Zoom: wheel / +-   Reset: R\n\
                         Axes: X red, Y green (up), Z blue";
                ui.label(egui::RichText::new(t).small().color(egui::Color32::from_gray(190)));
            });

            if let Some(i) = want_open {
                open_entry(world, &mut shell, i);
            }
        });
    });
}

fn browser_ui(ui: &mut egui::Ui, shell: &mut Shell, want_open: &mut Option<u32>) {
    ui.heading("Pack browser");
    if let Some(e) = &shell.error {
        ui.colored_label(egui::Color32::LIGHT_RED, e);
        return;
    }
    let Some(browser) = shell.browser.as_ref().map(|b| (b.pack.clone(),)) else {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(format!("Loading {} ...", shell.index));
        });
        return;
    };
    let pack = browser.0;
    let before = (shell.filter.clone(), shell.mode);
    ui.horizontal(|ui| {
        ui.label("Search");
        ui.add(egui::TextEdit::singleline(&mut shell.filter.query).hint_text("substring, terms ANDed").desired_width(f32::INFINITY));
    });
    ui.horizontal(|ui| {
        ui.selectable_value(&mut shell.mode, ViewMode::Folders, "Folders");
        ui.selectable_value(&mut shell.mode, ViewMode::ByType, "By type");
    });
    ui.horizontal_wrapped(|ui| {
        for k in FileKind::ALL {
            ui.checkbox(&mut shell.filter.kinds[k.index()], k.label());
        }
    });
    if (shell.filter.clone(), shell.mode) != before {
        shell.rows_dirty = true;
    }
    if shell.rows_dirty {
        shell.rows = visible_rows(&pack.entries, &shell.browser.as_ref().unwrap().tree, &shell.filter, &shell.expanded, shell.mode);
        shell.rows_dirty = false;
    }
    ui.label(format!("{} files in {} packs, {} rows shown", pack.entries.len(), pack.registered, shell.rows.len()));
    ui.separator();

    let row_h = ui.spacing().interact_size.y;
    let mut toggle = None;
    let mut open = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row_h, shell.rows.len(), |ui, range| {
        for r in &shell.rows[range] {
            match *r {
                Row::Dir { dir, depth, expanded, count } => {
                    let name = &shell.browser.as_ref().unwrap().tree.dirs[dir].name;
                    ui.horizontal(|ui| {
                        ui.add_space(depth as f32 * 14.0);
                        let mark = if expanded { "v" } else { ">" };
                        if ui.selectable_label(false, format!("{mark} {name}/  ({count})")).clicked() {
                            toggle = Some(NodeKey::Dir(dir));
                        }
                    });
                }
                Row::Group { kind, expanded, count } => {
                    ui.horizontal(|ui| {
                        let mark = if expanded { "v" } else { ">" };
                        if ui.selectable_label(false, format!("{mark} [{}]  ({count})", kind.label())).clicked() {
                            toggle = Some(NodeKey::Group(kind.index()));
                        }
                    });
                }
                Row::File { entry, depth } => {
                    let e = &pack.entries[entry as usize];
                    let label = if shell.mode == ViewMode::ByType { e.display() } else { e.display().rsplit('/').next().unwrap_or("") };
                    ui.horizontal(|ui| {
                        ui.add_space(depth as f32 * 14.0);
                        if ui.selectable_label(shell.selected == Some(entry), label).on_hover_text(e.display()).clicked() {
                            open = Some(entry);
                        }
                    });
                }
            }
        }
    });
    if let Some(k) = toggle {
        if !shell.expanded.remove(&k) {
            shell.expanded.insert(k);
        }
        shell.rows_dirty = true;
    }
    if open.is_some() {
        *want_open = open;
    }
}

/// `--screenshot <virtual path> <out.png>` job.
#[derive(Resource)]
pub struct ScreenshotJob {
    pub out: PathBuf,
    frames: u32,
    requested: bool,
}

impl ScreenshotJob {
    pub fn new(out: PathBuf) -> Self {
        ScreenshotJob { out, frames: 0, requested: false }
    }
}

pub fn screenshot_system(mut commands: Commands, mut job: ResMut<ScreenshotJob>, cur: Res<CurrentFile>, mut exit: MessageWriter<AppExit>) {
    if job.requested {
        job.frames += 1;
        if job.out.exists() && job.frames > 5 {
            info!("screenshot written to {}", job.out.display());
            exit.write(AppExit::Success);
        } else if job.frames > 600 {
            error!("screenshot timed out");
            exit.write(AppExit::error());
        }
        return;
    }
    if cur.0.is_some() {
        job.frames += 1;
        // Give the pack reader, GR2 loader, and first egui/GPU frame time to
        // settle before capturing the window. This keeps the CLI useful for
        // checking real assets instead of saving the startup frame.
        if job.frames >= 120 {
            job.requested = true;
            job.frames = 0;
            let out = job.out.clone();
            commands.spawn(bevy::render::view::screenshot::Screenshot::primary_window()).observe(bevy::render::view::screenshot::save_to_disk(out));
        }
    }
}
