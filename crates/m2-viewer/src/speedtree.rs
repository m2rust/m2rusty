//! SpeedTree `.spt` preview.
//!
//! The tree geometry comes pre-built from [`m2_assets::spt::SptTree`]: branch
//! and frond meshes with static vertex colours, plus one leaf-quad mesh that
//! has to be re-oriented towards the camera every time the camera moves, as
//! the original renderer did per frame.

use bevy::asset::{AssetServer, Assets, LoadState};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::prelude::*;
use bevy_egui::egui;
use m2_assets::paths;
use m2_assets::spt::{SptPart, SptTree};
use m2_render::{CullMode, FixedFunctionMaterial, Fog};

use crate::viewer::{ContentViewer, MainCamera, OpenFile, ViewerEntity};
use m2_render::{MipFilter, SamplerState};

/// Leaf cards are alpha tested against `ALPHAREF 128`. Filtering across mip
/// levels averages the transparent gaps of the atlas into the leaf texels and
/// pulls their alpha below the reference, so a distant tree loses its whole
/// canopy. Sampling level 0 keeps the cutout intact; minification is still
/// linear.
fn leaf_sampler() -> SamplerState {
    SamplerState { mip: MipFilter::None, ..SamplerState::default() }
}

#[derive(Default)]
pub struct SpeedTreeViewer {
    path: String,
    handle: Option<Handle<SptTree>>,
    tree: Option<SptTree>,
    root: Option<Entity>,
    leaf_mesh: Option<Handle<Mesh>>,
    /// Camera position the leaf quads were last oriented towards.
    leaf_camera: Option<Vec3>,
    error: Option<String>,
    fit_camera: bool,
    /// The map environment's fog, when this tree is a map object.
    fog: Option<Fog>,
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
                self.spawn(world, &tree);
                fit_camera(world, tree.bounding_box);
                self.tree = Some(tree);
            } else if self.error.is_none() {
                self.error = load_failed(world.resource::<AssetServer>(), handle.id());
            }
            return;
        }

        if self.fit_camera {
            if let Some(tree) = &self.tree {
                fit_camera(world, tree.bounding_box);
                self.fit_camera = false;
            }
        }

        self.orient_leaves(world);
    }

    fn ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("SpeedTree");
        ui.label(&self.path);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(tree) = &self.tree else {
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
        let _ = world;
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

impl SpeedTreeViewer {
    pub(crate) fn placed(world: &mut World, tree: SptTree, transform: Transform, fog: Option<Fog>) -> Self {
        let mut viewer = Self { fog, ..Self::default() };
        // Each instance needs its own mutable billboard mesh.
        let mut tree = tree;
        if let Some(mesh) = tree.leaf_mesh.as_ref().and_then(|h| world.resource::<Assets<Mesh>>().get(h)).cloned() {
            tree.leaf_mesh = Some(world.resource_mut::<Assets<Mesh>>().add(mesh));
        }
        viewer.spawn(world, &tree);
        world.entity_mut(viewer.root.unwrap()).insert(transform);
        viewer.tree = Some(tree);
        viewer
    }

    fn spawn(&mut self, world: &mut World, tree: &SptTree) {
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
        let root = world.spawn((ViewerEntity, Name::new(self.path.clone()), Transform::IDENTITY, Visibility::Visible)).id();
        self.root = Some(root);

        let composite = tree.textures.composite.as_deref().map(|p| load_texture(world, p));
        let bark = tree.textures.branch.as_deref().map(|p| load_texture(world, p));

        for (name, part, texture) in
            [("branches", &tree.branches, bark.clone()), ("fronds", &tree.fronds, composite.clone())]
        {
            if let Some(part) = part {
                self.spawn_part(world, root, name, part, texture);
            }
        }

        if let Some(leaf_mesh) = &tree.leaf_mesh {
            let base = match composite {
                Some(texture) => FixedFunctionMaterial::opaque(texture).sampler(leaf_sampler()),
                // Vertex colours only, as when a tree ships no atlas.
                None => FixedFunctionMaterial::untextured(),
            };
            let base = if let Some(fog) = self.fog { base.fog(fog) } else { base };
            let material = alpha_tested(base, tree.leaf_alpha_test);
            let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(material);
            world.spawn((
                ViewerEntity,
                Name::new("leaves"),
                Mesh3d(leaf_mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Visible,
                // The quads are rewritten to face the camera every frame, so
                // the AABB computed at spawn says nothing about where they
                // are now; culling them against it makes the canopy vanish.
                NoFrustumCulling,
                ChildOf(root),
            ));
            self.leaf_mesh = Some(leaf_mesh.clone());
            self.leaf_camera = None;
        }

    }

    fn spawn_part(&self, world: &mut World, root: Entity, name: &str, part: &SptPart, texture: Option<Handle<Image>>) {
        let base = match texture {
            Some(texture) => FixedFunctionMaterial::opaque(texture),
            None => FixedFunctionMaterial::untextured().diffuse(Vec4::new(0.42, 0.36, 0.24, 1.0)),
        };
        let base = if let Some(fog) = self.fog { base.fog(fog) } else { base };
        let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(alpha_tested(base, part.alpha_test));
        world.spawn((
            ViewerEntity,
            Name::new(name.to_owned()),
            Mesh3d(part.mesh.clone()),
            MeshMaterial3d(material),
            Transform::IDENTITY,
            Visibility::Visible,
            ChildOf(root),
        ));
    }

    /// Leaves are camera-facing quads, so their positions are rewritten
    /// whenever the camera moved since the last update.
    fn orient_leaves(&mut self, world: &mut World) {
        let (Some(tree), Some(handle)) = (&self.tree, &self.leaf_mesh) else { return };
        let Some(camera) = world.query_filtered::<&GlobalTransform, With<MainCamera>>().single(world).ok().map(|t| t.translation()) else {
            return;
        };
        if self.leaf_camera == Some(camera) {
            return;
        }
        let local_camera = self.root.and_then(|root| world.get::<Transform>(root))
            .map_or(camera, |transform| transform.to_matrix().inverse().transform_point3(camera));
        if let Some(mut mesh) = world.resource_mut::<Assets<Mesh>>().get_mut(handle) {
            tree.update_leaf_billboards(&mut mesh, Vec3::ZERO, local_camera);
            self.leaf_camera = Some(camera);
        }
    }
}

/// SpeedTree materials never cull: branch cards and leaves are two-sided.
///
/// `reference` is `D3DRS_ALPHAREF` as the compat layer reports it, already in
/// 0..=255 (leaves default to 128); 0 means the part has no alpha test.
fn alpha_tested(material: FixedFunctionMaterial, reference: f32) -> FixedFunctionMaterial {
    let material = material.cull(CullMode::None);
    match alpha_reference(reference) {
        Some(reference) => material.alpha_tested(reference),
        None => material,
    }
}

/// `None` when the part has no alpha test.
fn alpha_reference(reference: f32) -> Option<u8> {
    (reference > 0.0).then(|| reference.round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_reference_is_a_d3d_alpharef_not_a_fraction() {
        // The compat layer reports D3DRS_ALPHAREF in 0..=255; leaves default
        // to 128. Scaling it like a 0..1 factor would cut the whole canopy.
        assert_eq!(alpha_reference(128.0), Some(128));
        assert_eq!(alpha_reference(1.0), Some(1));
        assert_eq!(alpha_reference(0.0), None);
        assert_eq!(alpha_reference(300.0), Some(255));
    }

    #[test]
    fn a_leaf_material_without_a_test_is_not_alpha_tested() {
        let plain = alpha_tested(FixedFunctionMaterial::untextured(), 0.0);
        assert!(plain.alpha_test.is_none());
        assert_eq!(plain.cull, CullMode::None);
        let cut = alpha_tested(FixedFunctionMaterial::untextured(), 128.0);
        assert_eq!(cut.alpha_test.map(|t| t.reference), Some(128));
    }
}

fn load_texture(world: &World, path: &str) -> Handle<Image> {
    world.resource::<AssetServer>().load(paths::asset_source_uri(path))
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
