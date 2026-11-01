//! Granny model and motion preview (`.gr2` + `.msa`).

use std::collections::HashMap;

use bevy::animation::graph::{AnimationGraph, AnimationGraphHandle, AnimationNodeIndex};
use bevy::animation::{AnimationClip, AnimationPlayer, AnimatedBy};
use bevy::asset::{AssetServer, Assets, LoadState};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::ecs::hierarchy::ChildOf;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy_egui::egui;
use m2_assets::gr2::{self, Gr2Model};
use m2_assets::paths;
use m2_assets::race::RaceMotionAsset;
use m2_assets::M2Packs;
use m2_render::{Blend, CullMode, FixedFunctionMaterial, TextureArg, TextureOp, TextureStage};

use crate::viewer::{ContentViewer, MainCamera, OpenFile, ViewerEntity};

#[derive(Component)]
struct SkeletonLink(Option<Entity>);

#[derive(Resource, Default)]
struct SkeletonOverlay(bool);

#[derive(Default)]
pub struct Gr2Viewer {
    path: String,
    /// Motion requested on the command line; survives the `open` reset so the
    /// skinning walkthrough can be driven without a click.
    initial_motion: Option<String>,
    model_handle: Option<Handle<Gr2Model>>,
    model: Option<Gr2Model>,
    root: Option<Entity>,
    bones: Vec<Entity>,
    motions: Vec<String>,
    selected_motion: Option<String>,
    motion_handle: Option<Handle<RaceMotionAsset>>,
    motion: Option<RaceMotionAsset>,
    clip_handle: Option<Handle<AnimationClip>>,
    animation_node: Option<AnimationNodeIndex>,
    graph_handle: Option<Handle<AnimationGraph>>,
    error: Option<String>,
    fit_camera: bool,
}

pub struct ModelViewerPlugin;

impl Plugin for ModelViewerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkeletonOverlay>().add_systems(Update, draw_skeleton);
    }
}

pub(crate) fn spawn_model(world: &mut World, model: &Gr2Model) -> (Entity, Vec<Entity>) {
    let root = world
        .spawn((ViewerEntity, Name::new(model.name.clone()), Transform::IDENTITY, Visibility::Visible, AnimationPlayer::default()))
        .id();

    let bones = model
        .skeleton
        .as_ref()
        .map(|skeleton| {
            skeleton
                .bones
                .iter()
                .map(|bone| {
                    world
                        .spawn((
                            ViewerEntity,
                            Name::new(bone.name.clone()),
                            bone.local,
                            Visibility::Visible,
                            gr2::bone_target_id(&bone.name),
                            AnimatedBy(root),
                        ))
                        .id()
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(skeleton) = &model.skeleton {
        for (index, bone) in skeleton.bones.iter().enumerate() {
            let parent = bone.parent.map_or(root, |parent| bones[parent]);
            world.entity_mut(bones[index]).insert((ChildOf(parent), SkeletonLink(bone.parent.map(|p| bones[p]))));
        }
    }

    let bone_by_name: HashMap<String, Entity> = model
        .skeleton
        .iter()
        .flat_map(|skeleton| skeleton.bones.iter().zip(&bones))
        .map(|(bone, &entity)| (bone.name.clone(), entity))
        .collect();
    let lowercase: HashMap<String, Entity> = bone_by_name.iter().map(|(name, entity)| (name.to_ascii_lowercase(), *entity)).collect();
    let joints: Vec<Entity> = model
        .skeleton
        .iter()
        .flat_map(|skeleton| {
            skeleton
                .bones
                .iter()
                .map(|bone| bone_by_name.get(&bone.name).or_else(|| lowercase.get(&bone.name.to_ascii_lowercase())).copied().unwrap_or(root))
        })
        .collect();

    for mesh in &model.meshes {
        for part in &mesh.parts {
            let asset_server = world.resource::<AssetServer>();
            let diffuse = part.material.diffuse.as_deref().map(|path| asset_server.load::<Image>(paths::asset_source_uri(path)));
            let opacity = part.material.opacity.as_deref().map(|path| asset_server.load::<Image>(paths::asset_source_uri(path)));
            let mut material = diffuse
                .map(FixedFunctionMaterial::opaque)
                .unwrap_or_else(|| FixedFunctionMaterial::untextured().diffuse(Vec4::new(0.82, 0.72, 0.58, 1.0)));
            if let Some(opacity) = opacity {
                let alpha_stage = TextureStage {
                    color_op: TextureOp::SelectArg1,
                    color_arg1: TextureArg::CURRENT,
                    color_arg2: TextureArg::CURRENT,
                    alpha_op: TextureOp::Modulate,
                    alpha_arg1: TextureArg::TEXTURE,
                    alpha_arg2: TextureArg::CURRENT,
                };
                material = material.stage1(opacity, alpha_stage, Default::default()).blend(Blend::ALPHA);
            }
            if mesh.two_sided {
                material = material.cull(CullMode::None);
            }
            let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(material);
            let mut components = world.spawn((
                ViewerEntity,
                Name::new(mesh.name.clone()),
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                Visibility::Visible,
                ChildOf(root),
            ));
            if let (Some(bindposes), true) = (&model.inverse_bindposes, !joints.is_empty()) {
                components.insert((SkinnedMesh { inverse_bindposes: bindposes.clone(), joints: joints.clone() }, NoFrustumCulling));
            }
        }
    }
    (root, bones)
}

fn fit_camera(world: &mut World, model: &Gr2Model) {
    let (min, max) = model.bounds;
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

impl Gr2Viewer {
    /// Starts the given motion (`.msa` or a motion-only `.gr2`) as soon as the
    /// model opens.
    pub fn with_motion(mut self, motion: Option<String>) -> Self {
        self.initial_motion = motion.map(|m| paths::canonical_game_path(&m));
        self
    }

    /// Requests a motion; the clip and animation graph follow when the assets
    /// have loaded. Also used by the inspector's motion picker.
    fn select_motion(&mut self, world: &mut World, path: &str) {
        self.motion_handle = Some(world.resource::<AssetServer>().load(paths::asset_source_uri(path)));
        self.motion = None;
        self.clip_handle = None;
        self.animation_node = None;
        self.error = None;
        if let Some(root) = self.root {
            if let Some(mut player) = world.entity_mut(root).get_mut::<AnimationPlayer>() {
                player.stop_all();
            }
        }
        if let Some(graph) = self.graph_handle.take() {
            world.resource_mut::<Assets<AnimationGraph>>().remove(graph.id());
        }
        self.selected_motion = Some(path.to_owned());
    }
}

impl ContentViewer for Gr2Viewer {
    fn id(&self) -> &'static str {
        "gr2 model"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["gr2"]
    }

    fn open(&mut self, world: &mut World, file: &OpenFile) {
        let initial_motion = self.initial_motion.take();
        *self = Self { initial_motion, ..Self::default() };
        self.path = paths::canonical_game_path(&file.name);
        self.model_handle = Some(world.resource::<AssetServer>().load(paths::asset_source_uri(&self.path)));
        if let Some(packs) = world.get_resource::<M2Packs>() {
            self.motions = packs.motion_files(paths::parent(&self.path)).iter().map(|path| paths::canonical_game_path(path)).collect();
        }
        if let Some(motion) = self.initial_motion.clone() {
            self.select_motion(world, &motion);
        }
    }

    fn update(&mut self, world: &mut World) {
        if self.model.is_none() {
            if let Some(handle) = &self.model_handle {
                if let Some(model) = world.resource::<Assets<Gr2Model>>().get(handle).cloned() {
                    let parts = model.meshes.iter().flat_map(|m| &m.parts).collect::<Vec<_>>();
                    let ready_meshes = parts.iter().filter(|part| world.resource::<Assets<Mesh>>().get(&part.mesh).is_some()).count();
                    info!("loaded {}: {} mesh group(s), {}/{} mesh parts ready, {} bone(s), bounds {:?}..{:?}",
                        self.path, model.meshes.len(), ready_meshes, parts.len(), model.skeleton.as_ref().map_or(0, |s| s.bones.len()), model.bounds.0, model.bounds.1);
                    let (root, bones) = spawn_model(world, &model);
                    fit_camera(world, &model);
                    self.root = Some(root);
                    self.bones = bones;
                    self.model = Some(model);
                    self.fit_camera = false;
                } else if self.error.is_none() {
                    self.error = load_failed(world.resource::<AssetServer>(), handle.id());
                }
            }
        }

        if self.fit_camera {
            if let Some(model) = &self.model {
                fit_camera(world, model);
                self.fit_camera = false;
            }
        }

        if let Some(handle) = &self.motion_handle {
            if self.motion.is_none() {
                if let Some(motion) = world.resource::<Assets<RaceMotionAsset>>().get(handle).cloned() {
                    self.clip_handle = Some(world.resource::<AssetServer>().load(paths::asset_source_uri(&motion.motion_file)));
                    self.motion = Some(motion);
                } else if self.error.is_none() {
                    self.error = load_failed(world.resource::<AssetServer>(), handle.id());
                }
            }
        }

        if self.animation_node.is_none() {
            if let Some(handle) = &self.clip_handle {
                if world.resource::<Assets<AnimationClip>>().get(handle).is_some() {
                    let (graph, node) = AnimationGraph::from_clip(handle.clone());
                    let graph = world.resource_mut::<Assets<AnimationGraph>>().add(graph);
                    if let Some(root) = self.root {
                        world.entity_mut(root).insert(AnimationGraphHandle(graph.clone()));
                        if let Some(mut player) = world.entity_mut(root).get_mut::<AnimationPlayer>() {
                            player.play(node).repeat();
                        }
                        self.graph_handle = Some(graph);
                        self.animation_node = Some(node);
                    }
                } else if self.error.is_none() {
                    self.error = load_failed(world.resource::<AssetServer>(), handle.id());
                }
            }
        }
    }

    fn ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("3D model");
        ui.label(&self.path);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
        }
        let Some(model) = &self.model else {
            if self.error.is_none() {
                ui.spinner();
                ui.label("Loading model and mesh assets…");
            }
            return;
        };
        ui.label(format!("{} mesh group(s), {} material part(s)", model.meshes.len(), model.meshes.iter().map(|m| m.parts.len()).sum::<usize>()));
        ui.label(format!("{} bones", model.skeleton.as_ref().map_or(0, |s| s.bones.len())));
        ui.label(format!("Bounds: {:?} to {:?}", model.bounds.0, model.bounds.1));
        if ui.button("Fit model (F)").clicked() {
            self.fit_camera = true;
        }
        let mut overlay = world.resource::<SkeletonOverlay>().0;
        if ui.checkbox(&mut overlay, "Show skeleton").changed() {
            world.resource_mut::<SkeletonOverlay>().0 = overlay;
        }
        if overlay {
            egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                if let Some(skeleton) = &model.skeleton {
                    for bone in &skeleton.bones {
                        ui.label(format!("{}  parent: {}", bone.name, bone.parent.map_or("root".into(), |p| skeleton.bones[p].name.clone())));
                    }
                }
            });
        }

        ui.separator();
        ui.heading("Motion");
        if self.motions.is_empty() {
            ui.label("No .msa files found beside this model.");
        } else {
            let mut chosen = self.selected_motion.clone();
            egui::ComboBox::from_label("Motion script")
                .selected_text(chosen.as_deref().map_or("Choose a motion…", |p| p.rsplit('/').next().unwrap_or(p)))
                .show_ui(ui, |ui| {
                    for motion in &self.motions {
                        ui.selectable_value(&mut chosen, Some(motion.clone()), motion.rsplit('/').next().unwrap_or(motion));
                    }
                });
            if chosen != self.selected_motion {
                match chosen.clone() {
                    Some(path) => self.select_motion(world, &path),
                    None => self.selected_motion = None,
                }
            }
            if let Some(motion) = &self.motion {
                ui.label(format!("Animation: {}", motion.motion_file));
                ui.label(format!("Duration: {:.2}s · {} event(s)", motion.duration, motion.events.len()));
                if let (Some(root), Some(node)) = (self.root, self.animation_node) {
                    let mut entity = world.entity_mut(root);
                    if let Some(mut player) = entity.get_mut::<AnimationPlayer>() {
                        ui.horizontal(|ui| {
                            if player.all_paused() {
                                if ui.button("Play").clicked() { player.resume_all(); }
                            } else if ui.button("Pause").clicked() {
                                player.pause_all();
                            }
                            if ui.button("Restart").clicked() {
                                player.start(node).repeat();
                            }
                        });
                        if let Some(active) = player.animation(node) {
                            let mut seek = active.seek_time();
                            let end = motion.duration.max(0.01);
                            if ui.add(egui::Slider::new(&mut seek, 0.0..=end).text("time (s)")).changed() {
                                if let Some(active) = player.animation_mut(node) { active.set_seek_time(seek); }
                            }
                        }
                    }
                }
            }
        }
        ui.label("Orbit: drag · pan: right/middle drag · zoom: wheel");
    }

    fn close(&mut self, world: &mut World) {
        if let Some(graph) = self.graph_handle.take() {
            world.resource_mut::<Assets<AnimationGraph>>().remove(graph.id());
        }
    }
}

fn draw_skeleton(mut gizmos: Gizmos, overlay: Res<SkeletonOverlay>, bones: Query<(&GlobalTransform, &SkeletonLink)>, parents: Query<&GlobalTransform>) {
    if !overlay.0 {
        return;
    }
    for (bone, link) in &bones {
        if let Some(parent) = link.0.and_then(|entity| parents.get(entity).ok()) {
            gizmos.line(parent.translation(), bone.translation(), Color::srgb(0.25, 0.9, 0.7));
        }
    }
}
