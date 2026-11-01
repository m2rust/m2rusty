//! The content-viewer plugin interface.
//!
//! The shell owns the pack browser, the inspector, the camera and the scene.
//! A [`ContentViewer`] is registered for file extensions; when the user opens
//! a file the shell routes it by extension, calls [`ContentViewer::open`],
//! then [`ContentViewer::update`] every frame and [`ContentViewer::ui`] inside
//! the inspector panel, and [`ContentViewer::close`] when another file is
//! opened. Entities a viewer spawns must carry [`ViewerEntity`]; the shell
//! despawns them on close.
//!
//! Register from a Bevy plugin:
//! ```ignore
//! app.add_viewer(MyViewer::default());   // via the ViewerAppExt trait
//! ```

use std::sync::Arc;

use bevy::prelude::*;
use bevy_egui::egui;
use m2_formats::pack::PackManager;

use crate::index::{FileKind, extension};

/// The file being viewed.
#[derive(Clone)]
pub struct OpenFile {
    /// Virtual name (lowercase, `/`-separated).
    pub name: String,
    pub display: String,
    pub ext: String,
    pub kind: FileKind,
    pub bytes: Arc<Vec<u8>>,
    /// For viewers that load companions (textures of a model, ...).
    pub manager: Arc<PackManager>,
}

/// Tag for entities owned by the active viewer; despawned when it closes.
#[derive(Component)]
pub struct ViewerEntity;

/// Marker of the shell's single 3D camera.
#[derive(Component)]
pub struct MainCamera;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CameraMode {
    /// Orbit/pan/zoom with grid and axes.
    #[default]
    Orbit,
    /// The viewer moves the camera itself (e.g. flat texture view); grid and axes hidden.
    Custom,
}

/// Pointer state for viewers, already filtered: `scene_hovered` is false while
/// egui owns the pointer. Cursor deltas and scroll are in logical pixels / lines.
#[derive(Resource, Default, Clone, Copy)]
pub struct ViewerInput {
    pub scene_hovered: bool,
    pub cursor_delta: Vec2,
    pub scroll: f32,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

pub trait ContentViewer: Send + Sync + 'static {
    /// Stable id, e.g. `"texture"`.
    fn id(&self) -> &'static str;
    /// Lowercase extensions handled, without the dot.
    fn extensions(&self) -> &'static [&'static str];
    fn camera_mode(&self) -> CameraMode {
        CameraMode::Orbit
    }
    /// A file routed to this viewer was opened. Spawn entities with [`ViewerEntity`].
    fn open(&mut self, world: &mut World, file: &OpenFile);
    /// Every frame while open.
    fn update(&mut self, _world: &mut World) {}
    /// Viewer-specific controls and info, drawn in the inspector panel.
    fn ui(&mut self, _world: &mut World, _ui: &mut egui::Ui) {}
    /// Called before the entities are despawned and another file opens.
    fn close(&mut self, _world: &mut World) {}
}

#[derive(Resource, Default)]
pub struct ViewerRegistry {
    pub viewers: Vec<Box<dyn ContentViewer>>,
    /// Index into `viewers` of the open viewer.
    pub active: Option<usize>,
}

impl ViewerRegistry {
    pub fn register(&mut self, v: Box<dyn ContentViewer>) {
        self.viewers.push(v);
    }
    /// Later registrations win, so an integrator can override a stand-in viewer.
    pub fn route(&self, name: &str) -> Option<usize> {
        let ext = extension(name).to_ascii_lowercase();
        route_extension(self.viewers.iter().map(|v| v.extensions()), &ext)
    }
}

/// Pure routing: index of the last extension list containing `ext`.
pub fn route_extension<'a>(lists: impl Iterator<Item = &'a [&'static str]>, ext: &str) -> Option<usize> {
    let mut found = None;
    for (i, l) in lists.enumerate() {
        if l.contains(&ext) {
            found = Some(i);
        }
    }
    found
}

pub trait ViewerAppExt {
    fn add_viewer(&mut self, v: impl ContentViewer) -> &mut Self;
}

impl ViewerAppExt for App {
    fn add_viewer(&mut self, v: impl ContentViewer) -> &mut Self {
        self.init_resource::<ViewerRegistry>();
        self.world_mut().resource_mut::<ViewerRegistry>().register(Box::new(v));
        self
    }
}

/// The open file, if any.
#[derive(Resource, Default)]
pub struct CurrentFile(pub Option<OpenFile>);

/// Set by the browser (or CLI) to open a file; consumed by [`drive_viewers`].
#[derive(Resource, Default)]
pub struct PendingOpen(pub Option<OpenFile>);

pub fn drive_viewers(world: &mut World) {
    world.resource_scope(|world, mut reg: Mut<ViewerRegistry>| {
        if let Some(file) = world.resource_mut::<PendingOpen>().0.take() {
            if let Some(a) = reg.active.take() {
                reg.viewers[a].close(world);
            }
            let stale: Vec<Entity> = world.query_filtered::<Entity, With<ViewerEntity>>().iter(world).collect();
            for e in stale {
                world.despawn(e);
            }
            reg.active = reg.route(&file.name);
            if let Some(a) = reg.active {
                reg.viewers[a].open(world, &file);
            }
            world.resource_mut::<CurrentFile>().0 = Some(file);
        }
        if let Some(a) = reg.active {
            reg.viewers[a].update(world);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing() {
        let lists: Vec<&[&str]> = vec![&["dds", "tga"], &["gr2"], &["tga"]];
        assert_eq!(route_extension(lists.iter().copied(), "dds"), Some(0));
        assert_eq!(route_extension(lists.iter().copied(), "tga"), Some(2));
        assert_eq!(route_extension(lists.iter().copied(), "spt"), None);
    }
}
