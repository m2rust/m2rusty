//! `CEffectInstance` (`EffectInstance.cpp`): one playing effect.

use std::sync::Arc;

use crate::light::LightInstance;
use crate::math::{Mat4, Vec3};
use crate::mesh::{EffectMeshInstance, MeshDraw};
use crate::particle::Camera;
use crate::particle_system::{ParticleSystemDraw, ParticleSystemInstance};
use crate::rng::GlibcRandom;
use m2_formats::effect::{EffectData, EffectMesh, SoundInstance};

/// Looks up the loaded `.mde` an effect mesh element names
/// (`CResourceManager::GetResourcePointer`). `None` is a missing resource.
pub trait MeshSource {
    fn effect_mesh(&self, name: &[u8]) -> Option<Arc<EffectMesh>>;
}

impl<F: Fn(&[u8]) -> Option<Arc<EffectMesh>>> MeshSource for F {
    fn effect_mesh(&self, name: &[u8]) -> Option<Arc<EffectMesh>> {
        self(name)
    }
}

/// No meshes are available: every mesh element ends immediately.
pub struct NoMeshes;

impl MeshSource for NoMeshes {
    fn effect_mesh(&self, _name: &[u8]) -> Option<Arc<EffectMesh>> {
        None
    }
}

/// Everything to draw for one effect, for one camera.
#[derive(Debug, Clone)]
pub struct EffectDraw<'a> {
    pub particle_systems: Vec<ParticleSystemDraw<'a>>,
    pub meshes: Vec<MeshDraw<'a>>,
}

/// `CEffectInstance`.
#[derive(Debug, Clone)]
pub struct EffectInstance {
    data: Arc<EffectData>,
    alive: bool,
    frame: u32,
    global: Mat4,
    particles: Vec<ParticleSystemInstance>,
    meshes: Vec<EffectMeshInstance>,
    lights: Vec<LightInstance>,
    last_time: f32,
}

impl EffectInstance {
    /// `SetEffectDataPointer`. `now` is the global timer's current second
    /// (`CTimer::GetCurrentSecond`), the base of [`Self::update_to`].
    pub fn new(data: Arc<EffectData>, meshes: &dyn MeshSource, now: f32) -> Self {
        let mut inst = Self {
            data: Arc::clone(&data),
            alive: false,
            frame: 0,
            global: Mat4::IDENTITY,
            particles: Vec::new(),
            meshes: Vec::new(),
            lights: Vec::new(),
            last_time: 0.0,
        };
        inst.set_data(data, meshes, now);
        inst
    }

    fn set_data(&mut self, data: Arc<EffectData>, meshes: &dyn MeshSource, now: f32) {
        self.alive = true;
        self.last_time = now;
        self.particles = (0..data.particles.len()).map(|i| ParticleSystemInstance::new(Arc::clone(&data), i)).collect();
        self.meshes = (0..data.meshes.len())
            .map(|i| {
                let mesh = meshes.effect_mesh(&data.meshes[i].mesh_file_name);
                EffectMeshInstance::new(Arc::clone(&data), i, mesh)
            })
            .collect();
        self.lights = (0..data.lights.len()).map(|i| LightInstance::new(Arc::clone(&data), i)).collect();
        self.data = data;
    }

    /// `Clear()` followed by `SetEffectDataPointer`: plays the effect again
    /// from the start. As in C++, `Clear` resets the global matrix and the
    /// sound frame counter.
    pub fn restart(&mut self, meshes: &dyn MeshSource, now: f32) {
        self.frame = 0;
        self.global = Mat4::IDENTITY;
        let data = Arc::clone(&self.data);
        self.set_data(data, meshes, now);
    }

    pub fn data(&self) -> &Arc<EffectData> {
        &self.data
    }

    /// `SetGlobalMatrix`: the effect's place in the world.
    pub fn set_global_matrix(&mut self, m: Mat4) {
        self.global = m;
    }

    pub fn global_matrix(&self) -> &Mat4 {
        &self.global
    }

    /// `isAlive`: false once every element is done.
    pub fn is_alive(&self) -> bool {
        self.alive
    }

    /// `SetActive`: elements may emit again.
    pub fn set_active(&mut self) {
        for p in &mut self.particles {
            p.state.active = true;
        }
        for m in &mut self.meshes {
            m.state.active = true;
        }
        for l in &mut self.lights {
            l.state.active = true;
        }
    }

    /// `SetDeactive`: stop emitting; live particles fade out, meshes and
    /// lights end at their next update.
    pub fn set_deactive(&mut self) {
        for p in &mut self.particles {
            p.state.active = false;
        }
        for m in &mut self.meshes {
            m.state.active = false;
        }
        for l in &mut self.lights {
            l.state.active = false;
        }
    }

    /// `OnUpdate` with an explicit step (the `WORLD_EDITOR` variant).
    pub fn update(&mut self, dt: f32, rng: &mut GlibcRandom) {
        let mut alive = false;
        let global = self.global;
        for p in &mut self.particles {
            alive |= p.update(dt, &global, rng);
        }
        for m in &mut self.meshes {
            alive |= m.update(dt);
        }
        for l in &mut self.lights {
            alive |= l.update(dt, &global);
        }
        self.alive = alive;
    }

    /// `OnUpdate` as the client runs it: the step is the change of the global
    /// timer since the last update.
    pub fn update_to(&mut self, now: f32, rng: &mut GlibcRandom) {
        let dt = now - self.last_time;
        self.update(dt, rng);
        self.last_time = now;
    }

    /// `UpdateSound`: the sounds scheduled for this frame (60 per second) and
    /// where to play them; advances the frame counter.
    pub fn update_sound(&mut self) -> Vec<(Vec3, &SoundInstance)> {
        let at = self.global.translation();
        let frame = self.frame;
        self.frame += 1;
        self.data.sounds.iter().filter(|s| s.frame == frame).map(|s| (at, s)).collect()
    }

    /// `GetBoundingSphere`: centre and radius, radius 0 when the effect has
    /// none.
    pub fn bounding_sphere(&self) -> (Vec3, f32) {
        let t = self.global.translation();
        let p = self.data.bounding_sphere_position;
        ([t[0] + p[0], t[1] + p[1], t[2] + p[2]], self.data.bounding_sphere_radius)
    }

    /// Particle and mesh texture frames, including ones not emitted yet.
    pub fn texture_names(&self) -> impl Iterator<Item = &[u8]> {
        self.data.particles.iter().flat_map(|system| &system.particle.textures).map(Vec::as_slice)
            .chain(self.meshes.iter().flat_map(EffectMeshInstance::texture_names))
    }

    pub fn particle_systems(&self) -> &[ParticleSystemInstance] {
        &self.particles
    }

    pub fn meshes(&self) -> &[EffectMeshInstance] {
        &self.meshes
    }

    pub fn lights(&self) -> &[LightInstance] {
        &self.lights
    }

    /// What `OnRender` draws: particle quads and mesh draws. `view` is the
    /// camera view matrix, `render_elapsed` the timer's elapsed seconds.
    pub fn render(&self, camera: &Camera, view: &Mat4, render_elapsed: f32) -> EffectDraw<'_> {
        EffectDraw {
            particle_systems: self.particles.iter().map(|p| p.render(camera, &self.global)).collect(),
            meshes: self.meshes.iter().flat_map(|m| m.render(view, render_elapsed, &self.global)).collect(),
        }
    }
}
