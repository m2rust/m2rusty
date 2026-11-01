//! SpeedTree `.spt` preview. Spawning and the camera-facing leaves are
//! [`m2_game::render::tree::Tree`]; this is the browser entry and inspector.

use bevy::asset::{AssetServer, Assets, LoadState};
use bevy::prelude::*;
use bevy_egui::egui;
use m2_assets::paths;
use m2_assets::spt::{SptPart, SptTree};
use m2_game::render::tree::Tree;

use crate::viewer::{ContentViewer, MainCamera, OpenFile, ViewerEntity};

#[derive(Default)]
pub struct SpeedTreeViewer {
    path: String,
    handle: Option<Handle<SptTree>>,
    tree: Option<Tree>,
    error: Option<String>,
    fit_camera: bool,
}

impl ContentViewer for SpeedTreeViewer {
    fn id(&self) -> &'static str {
        "spt tree"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["spt"]
    }

    fn open(&mut self, world: &mut World, file: &OpenFile) {
        *self = Self::default();
        self.path = paths::canonical_game_path(&file.name);
        self.handle = Some(world.resource::<AssetServer>().load(paths::asset_source_uri(&self.path)));
    }

    fn update(&mut self, world: &mut World) {
        if self.tree.is_none() {
            let Some(handle) = &self.handle else { return };
            if let Some(tree) = world.resource::<Assets<SptTree>>().get(handle).cloned() {
                info!(
                    "loaded {}: branches {}, fronds {}, {} leaf quad(s), {} collision(s), bounds {:?}..{:?}, {}",
                    self.path,
                    describe(tree.branches.as_ref(), world),
                    describe(tree.fronds.as_ref(), world),
                    tree.leaves.len(),
                    tree.collisions.len(),
                    tree.bounding_box.0,
                    tree.bounding_box.1,
                    if tree.baked { "baked cache" } else { "generated geometry" },
                );
                fit_camera(world, tree.bounding_box);
                let tree = Tree::spawn(world, tree, Transform::IDENTITY, None, None);
                world.entity_mut(tree.root).insert(ViewerEntity);
                self.tree = Some(tree);
            } else if self.error.is_none() {
                self.error = load_failed(world.resource::<AssetServer>(), handle.id());
            }
            return;
        }

        if self.fit_camera
            && let Some(tree) = &self.tree
        {
            fit_camera(world, tree.tree.bounding_box);
            self.fit_camera = false;
        }

        if let Some(tree) = &mut self.tree {
            tree.orient_leaves(world);
        }
    }

    fn ui(&mut self, _world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("SpeedTree");
        ui.label(&self.path);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(tree) = self.tree.as_ref().map(|t| &t.tree) else {
            if self.error.is_none() {
                ui.spinner();
                ui.label("Loading tree geometry…");
            }
            return;
        };
        ui.label(format!("Size: {:.2} m{}", tree.size, if tree.baked { " · baked cache" } else { " · generated" }));
        let (min, max) = tree.bounding_box;
        ui.label(format!("Bounds: {:?} to {:?}", min, max));
        ui.label(format!(
            "Parts: branches {}, fronds {}, leaves {}",
            present(tree.branches.is_some()),
            present(tree.fronds.is_some()),
            tree.leaves.len()
        ));
        ui.label(format!("Collisions: {}", tree.collisions.len()));
        for (name, path) in [
            ("Bark", &tree.textures.branch),
            ("Leaf atlas", &tree.textures.composite),
            ("Self shadow", &tree.textures.self_shadow),
        ] {
            if let Some(path) = path {
                ui.label(format!("{name}: {}", path.rsplit('/').next().unwrap_or(path)));
            }
        }
        if ui.button("Fit tree (F)").clicked() {
            self.fit_camera = true;
        }
        ui.label("Orbit: drag · pan: right/middle drag · zoom: wheel");
    }
}

/// `"<n> vertex/quads"` for a static part, for the load log.
fn describe(part: Option<&SptPart>, world: &World) -> String {
    let Some(part) = part else { return "none".into() };
    match world.resource::<Assets<Mesh>>().get(&part.mesh).and_then(|m| m.attribute(Mesh::ATTRIBUTE_POSITION)) {
        Some(positions) => format!("{} vertex", positions.len()),
        None => "pending".into(),
    }
}

fn present(yes: bool) -> &'static str {
    if yes {
        "yes"
    } else {
        "no"
    }
}

fn fit_camera(world: &mut World, bounds: (Vec3, Vec3)) {
    let (min, max) = bounds;
    let focus = (min + max) * 0.5;
    let radius = ((max - min).length() * 1.35).max(1.0);
    let mut query = world.query_filtered::<(&mut crate::camera::OrbitCamera, &mut Transform), With<MainCamera>>();
    if let Ok((mut orbit, mut transform)) = query.single_mut(world) {
        orbit.focus = focus;
        orbit.radius = radius;
        *transform = orbit.transform();
    }
}

fn load_failed(server: &AssetServer, id: impl Into<bevy::asset::UntypedAssetId>) -> Option<String> {
    match server.get_load_state(id)? {
        LoadState::Failed(error) => Some(error.to_string()),
        _ => None,
    }
}
