//! Effect `.mse` preview.
//!
//! The script is loaded with `m2-formats::effect`, meshes with the same
//! loader, and the simulation runs in `m2-effect`. Drawing goes through the
//! shared [`m2_game::render::effect::EffectRenderer`], so the viewer and the map
//! effects use the same fixed-function state translation.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_egui::egui;
use m2_assets::coords;
use m2_formats::effect::{EffectData, TimeEventTableFloat};
use m2_game::render::effect::{EffectMeshes, EffectRenderer, instance_from_data, load_data, load_meshes};

use crate::viewer::{ContentViewer, MainCamera, OpenFile, ViewerEntity};

/// Column-major identity in the layout `EffectInstance::set_global_matrix` takes.
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// How many frames of live particles the automatic camera fit watches before
/// it stops growing. One second at 60 fps; the longest cycle in the shipped
/// effects is well under that.
const FIT_FRAMES: u32 = 60;

/// Accumulated extent of the effect, in game coordinates.
#[derive(Clone, Copy)]
struct Extent {
    min: [f32; 3],
    max: [f32; 3],
    any: bool,
}

impl Default for Extent {
    fn default() -> Self {
        Self { min: [f32::INFINITY; 3], max: [f32::NEG_INFINITY; 3], any: false }
    }
}

impl Extent {
    fn add(&mut self, point: [f32; 3]) {
        for ((min, max), p) in self.min.iter_mut().zip(self.max.iter_mut()).zip(point) {
            *min = min.min(p);
            *max = max.max(p);
        }
        self.any = true;
    }

    fn add_radius(&mut self, point: [f32; 3], radius: f32) {
        self.add([point[0] - radius, point[1] - radius, point[2] - radius]);
        self.add([point[0] + radius, point[1] + radius, point[2] + radius]);
    }

    fn center_and_radius(&self) -> ([f32; 3], f32) {
        let center = std::array::from_fn(|k| (self.min[k] + self.max[k]) * 0.5);
        let diagonal = ((self.max[0] - self.min[0]).powi(2)
            + (self.max[1] - self.min[1]).powi(2)
            + (self.max[2] - self.min[2]).powi(2))
        .sqrt();
        (center, (diagonal * 0.5 * 1.2).max(1.0))
    }
}

#[derive(Default)]
pub struct EffectViewer {
    path: String,
    data: Option<Arc<EffectData>>,
    meshes: EffectMeshes,
    renderer: EffectRenderer,
    error: Option<String>,
    /// Restart automatically when every element has finished.
    looping: bool,
    /// Extent watched so far while fitting; see [`FIT_FRAMES`].
    fit: Extent,
    fit_frames: u32,
    fit_done: bool,
}

impl EffectViewer {
    fn restart(&mut self) {
        let meshes = &self.meshes;
        for instance in &mut self.renderer.instances {
            instance.restart(&|name: &[u8]| meshes.get(name).cloned(), 0.0);
        }
    }

    fn begin_fit(&mut self) {
        self.fit = Extent::default();
        // The effect is placed at its origin, so the origin is always in view.
        self.fit.add([0.0; 3]);
        self.fit_frames = 0;
        self.fit_done = false;
    }

    /// Grows the fit extent from the live particles and applies it. Returns
    /// whether the fit is complete.
    fn update_fit(&mut self, world: &mut World) -> bool {
        if let Some(instance) = self.renderer.instances.first() {
            for system in instance.particle_systems() {
                for frame in 0..system.frame_count() {
                    for particle in system.particles(frame) {
                        // Billboards can be rotated, so use the larger half size.
                        let radius = particle.half_size[0].abs().max(particle.half_size[1].abs());
                        self.fit.add_radius(particle.position, radius);
                    }
                }
            }
        }
        self.fit_frames += 1;
        if self.fit.any {
            let (center, radius) = self.fit.center_and_radius();
            apply_camera(world, center, radius);
            self.fit_done = self.fit_frames >= FIT_FRAMES;
        } else if self.fit_frames >= 5 {
            // Effects without particles (mesh/light only) fall back to the
            // declared trajectories.
            if let Some(data) = &self.data {
                let (center, radius) = declared_extent(data);
                apply_camera(world, center, radius);
            }
            self.fit_done = true;
        }
        self.fit_done
    }
}

impl ContentViewer for EffectViewer {
    fn id(&self) -> &'static str {
        "effect"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["mse"]
    }

    fn open(&mut self, world: &mut World, file: &OpenFile) {
        *self = Self::default();
        self.path = file.name.clone();
        // A preview that plays once is empty most of the time; loop by default.
        self.looping = true;
        let data = match load_data(&file.manager, file.name.as_bytes()) {
            Ok(data) => data,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let meshes = match load_meshes(&file.manager, &data) {
            Ok(meshes) => meshes,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let instance = instance_from_data(Arc::clone(&data), &meshes, IDENTITY);
        self.renderer.instances = vec![instance];
        self.renderer.root = Some(world.spawn((ViewerEntity, Name::new("effect"), Transform::IDENTITY, Visibility::Visible)).id());
        self.data = Some(data);
        self.meshes = meshes;
        self.begin_fit();
    }

    fn update(&mut self, world: &mut World) {
        self.renderer.update(world);
        if !self.fit_done {
            self.update_fit(world);
        }
        if self.looping && self.renderer.instances.first().is_some_and(|i| !i.is_alive()) {
            self.restart();
        }
    }

    fn ui(&mut self, world: &mut World, ui: &mut egui::Ui) {
        ui.separator();
        ui.heading("Effect");
        ui.label(&self.path);
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, error);
            return;
        }
        let Some(data) = &self.data else {
            ui.spinner();
            ui.label("Loading effect…");
            return;
        };
        ui.label(format!(
            "Elements: {} particle, {} mesh, {} light, {} sound",
            data.particles.len(),
            data.meshes.len(),
            data.lights.len(),
            data.sounds.len()
        ));
        if data.bounding_sphere_radius > 0.0 {
            ui.label(format!("Bounding sphere radius: {:.2}", data.bounding_sphere_radius));
        }
        if let Some(instance) = self.renderer.instances.first() {
            let live: u32 = instance.particle_systems().iter().map(|s| s.emission_count()).sum();
            ui.label(format!(
                "Running: {} · {live} live particles, {} meshes, {} lights",
                if instance.is_alive() { "yes" } else { "finished" },
                instance.meshes().len(),
                instance.lights().len()
            ));
        }
        ui.checkbox(&mut self.looping, "Loop");
        if ui.button("Restart (R)").clicked() {
            self.restart();
        }
        if ui.button("Fit effect (F)").clicked() {
            self.begin_fit();
        }
        let _ = world;
        ui.label("Orbit: drag · pan: right/middle drag · zoom: wheel");
    }
}

/// The extent the script declares: element trajectories plus the largest
/// emitter size. Used for effects whose particles never appear.
fn declared_extent(data: &EffectData) -> ([f32; 3], f32) {
    let mut extent = Extent::default();
    extent.add([0.0; 3]);
    let max_of = |table: &TimeEventTableFloat| table.iter().map(|e| e.value).fold(0.0_f32, f32::max);
    for system in &data.particles {
        for p in &system.base.positions {
            extent.add(p.position);
        }
        let size_x = max_of(&system.emitter.size_x);
        let size_y = max_of(&system.emitter.size_y);
        let scale = max_of(&system.particle.scale_x).max(max_of(&system.particle.scale_y));
        let radius = (size_x.max(size_y) * scale.max(1.0)) * 0.5;
        for p in &system.base.positions {
            extent.add_radius(p.position, radius);
        }
    }
    for mesh in &data.meshes {
        for p in &mesh.base.positions {
            extent.add(p.position);
        }
    }
    for light in &data.lights {
        for p in &light.base.positions {
            extent.add_radius(p.position, light.max_range);
        }
    }
    if extent.any {
        extent.center_and_radius()
    } else {
        (data.bounding_sphere_position, data.bounding_sphere_radius.max(1.0))
    }
}

/// Points the orbit camera at `center` (game coordinates) from `radius` away
/// and lets the far plane follow the effect's size.
fn apply_camera(world: &mut World, center: [f32; 3], radius: f32) {
    set_far_plane(world, (radius * 8.0).max(1000.0));
    let mut query = world.query_filtered::<(&mut crate::camera::OrbitCamera, &mut Transform), With<MainCamera>>();
    if let Ok((mut orbit, mut transform)) = query.single_mut(world) {
        orbit.focus = coords::vec3(center);
        orbit.radius = radius;
        *transform = orbit.transform();
    }
}

fn set_far_plane(world: &mut World, far: f32) {
    let mut query = world.query_filtered::<&mut Projection, With<MainCamera>>();
    if let Ok(mut projection) = query.single_mut(world)
        && let Projection::Perspective(perspective) = &mut *projection
    {
        perspective.near = 0.5;
        perspective.far = far;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_formats::effect::{EffectPosition, ParticleSystemData};

    fn effect_with(positions: &[[f32; 3]], size: f32) -> EffectData {
        let mut system = ParticleSystemData::default();
        system.base.positions = positions
            .iter()
            .enumerate()
            .map(|(i, &position)| EffectPosition { time: i as f32, position, ..Default::default() })
            .collect();
        system.emitter.size_x = vec![m2_formats::effect::TimeEvent { time: 0.0, value: size }];
        system.emitter.size_y = vec![m2_formats::effect::TimeEvent { time: 0.0, value: size }];
        EffectData { particles: vec![system], ..EffectData::default() }
    }

    #[test]
    fn declared_extent_covers_trajectory_and_emitter_size() {
        // A single position still leaves the origin visible, and the emitter
        // size widens the sphere.
        let (center, radius) = declared_extent(&effect_with(&[[0.0, 0.0, 0.0]], 100.0));
        assert_eq!(center, [0.0, 0.0, 0.0]);
        // The size makes a 100-wide box, so the diagonal is 100√3.
        assert!((radius - 103.923_05).abs() < 1e-2, "{radius}");

        // A trajectory to a far point is centred on its middle.
        let (center, radius) = declared_extent(&effect_with(&[[0.0, 0.0, 0.0], [100.0, 0.0, 0.0]], 0.0));
        assert_eq!(center, [50.0, 0.0, 0.0]);
        assert!((radius - 60.0).abs() < 1e-3, "{radius}");
    }

    #[test]
    #[ignore = "requires the bundled client packs under bin/"]
    fn bundled_effect_loads_particles_and_meshes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
        let mut packs = m2_formats::pack::PackManager::new(root);
        m2_formats::pack::load_index(&mut packs, "pack", "pack/Index.dev").unwrap();

        let data = load_data(&packs, b"d:/ymir work/effect/affect/damagevalue/critical.mse").unwrap();
        assert_eq!(data.particles.len(), 1);
        assert!(data.bounding_sphere_radius == 0.0);
        let meshes = load_meshes(&packs, &data).unwrap();
        let instance = instance_from_data(data, &meshes, IDENTITY);
        assert!(instance.is_alive());
        assert_eq!(instance.particle_systems().len(), 1);

        // An effect with `.mde` meshes loads them through the same path.
        let data = load_data(&packs, b"d:/ymir work/effect/etc/ox/success.mse").unwrap();
        let meshes = load_meshes(&packs, &data).unwrap();
        assert_eq!(meshes.len(), data.meshes.len());
    }
}
