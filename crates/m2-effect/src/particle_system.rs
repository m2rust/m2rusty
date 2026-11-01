//! `CParticleSystemInstance` (`ParticleSystemInstance.cpp`) and
//! `CParticleSystemData::BuildDecorator`: the emitter and its live
//! particles.

use std::sync::Arc;

use crate::decorator::{Decorator, FloatTarget};
use crate::element::ElementState;
use crate::math::{self, Mat4, Vec3, float_to_int};
use crate::particle::{Camera, Particle, billboard};
use crate::rng::GlibcRandom;
use m2_formats::effect::{EffectData, ParticleSystemData, time_event_blend_value};

const SHAPE_POINT: u8 = 0;
const SHAPE_ELLIPSE: u8 = 1;
const SHAPE_SQUARE: u8 = 2;
const SHAPE_SPHERE: u8 = 3;
const ADVANCED_OUTER: u8 = 1;
const ADVANCED_INNER: u8 = 2;
const TEX_ANI_CW: u8 = 1;
const TEX_ANI_CCW: u8 = 2;
const TEX_ANI_RANDOM_FRAME: u8 = 3;
const TEX_ANI_RANDOM_DIRECTION: u8 = 4;
const ROTATION_TIME_EVENT: u8 = 1;
const ROTATION_CW: u8 = 2;
const ROTATION_CCW: u8 = 3;
const ROTATION_RANDOM_DIRECTION: u8 = 4;

/// One billboard to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleQuad {
    /// Corners in triangle-strip order (see [`crate::particle::QUAD_UV`]).
    pub positions: [Vec3; 4],
    /// `0xAARRGGBB` texture factor.
    pub color: u32,
    /// Index into [`ParticleSystemDraw::textures`].
    pub texture: usize,
    /// World position of the particle centre and its radius, for culling.
    pub center: Vec3,
    pub radius: f32,
}

/// What `CParticleSystemInstance::OnRender` would draw.
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleSystemDraw<'a> {
    pub src_blend: u8,
    pub dest_blend: u8,
    pub color_operation: u8,
    /// Texture names, one per animation frame.
    pub textures: &'a [Vec<u8>],
    /// In draw order: texture frame by texture frame, 1 to 3 quads per
    /// particle.
    pub quads: Vec<ParticleQuad>,
}

/// `CParticleSystemInstance`.
#[derive(Debug, Clone)]
pub struct ParticleSystemInstance {
    effect: Arc<EffectData>,
    index: usize,
    pub(crate) state: ElementState,
    emission_residue: f32,
    emission_count: u32,
    loop_count: i32,
    /// `m_ParticleInstanceListVector`: one list per texture frame, then a
    /// pending list per frame for particles that changed frame.
    lists: Vec<Vec<Particle>>,
}

impl ParticleSystemInstance {
    /// `SetDataPointer` for `effect.particles[index]`.
    pub fn new(effect: Arc<EffectData>, index: usize) -> Self {
        let data = &effect.particles[index];
        let frames = data.particle.textures.len();
        Self {
            state: ElementState::new(data.base.start_time),
            emission_residue: 0.0,
            emission_count: 0,
            loop_count: data.emitter.loop_count,
            lists: vec![Vec::new(); frames * 2 + 2],
            effect,
            index,
        }
    }

    pub fn data(&self) -> &ParticleSystemData {
        &self.effect.particles[self.index]
    }

    /// `GetEmissionCount`: live particles.
    pub fn emission_count(&self) -> u32 {
        self.emission_count
    }

    pub fn local_time(&self) -> f32 {
        self.state.local_time
    }

    pub fn is_started(&self) -> bool {
        self.state.started
    }

    pub fn is_active(&self) -> bool {
        self.state.active
    }

    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    pub fn emission_residue(&self) -> f32 {
        self.emission_residue
    }

    /// The live particles of texture frame `frame`, in C++ list order.
    pub fn particles(&self, frame: usize) -> &[Particle] {
        self.lists.get(frame).map_or(&[], |l| l.as_slice())
    }

    /// Number of per-frame lists that are updated and drawn.
    pub fn frame_count(&self) -> usize {
        self.data().particle.textures.len()
    }

    /// `CEffectElementBaseInstance::Update`: false once the system is done.
    pub fn update(&mut self, dt: f32, local: &Mat4, rng: &mut GlibcRandom) -> bool {
        match self.state.begin(dt) {
            Some(alive) => alive,
            None => self.on_update(dt, local, rng),
        }
    }

    fn on_update(&mut self, dt: f32, local: &Mat4, rng: &mut GlibcRandom) -> bool {
        let effect = Arc::clone(&self.effect);
        let data = &effect.particles[self.index];
        let emitter = &data.emitter;
        let props = &data.particle;
        let mut make_particles = true;

        if self.state.local_time >= emitter.cycle_length {
            // `isCycleLoop() && --m_iLoopCount != 0`
            let again = emitter.cycle_loop != 0 && {
                self.loop_count = self.loop_count.wrapping_sub(1);
                self.loop_count != 0
            };
            if again {
                self.loop_count = self.loop_count.max(0);
                self.state.local_time -= emitter.cycle_length;
            } else {
                make_particles = false;
                self.loop_count = 1;
                if self.emission_count == 0 {
                    return false;
                }
            }
        }

        let frame_count = props.textures.len();
        let angular_velocity = time_event_blend_value(self.state.local_time, &emitter.emitting_angular_velocity);
        let mut z_axis = [0.0, 0.0, 1.0];
        if angular_velocity != 0.0 && props.attach == 0 {
            z_axis = local.transform_normal([0.0, 0.0, 1.0]);
        }

        for frame in 0..frame_count {
            let list = std::mem::take(&mut self.lists[frame]);
            let mut stay = Vec::with_capacity(list.len());
            for mut p in list {
                if !p.update(props, z_axis, dt, angular_velocity, rng) {
                    self.emission_count = self.emission_count.wrapping_sub(1);
                } else if usize::from(p.frame_index) != frame {
                    let to = frame_count + usize::from(p.frame_index);
                    if to >= self.lists.len() {
                        self.lists.resize(to + 1, Vec::new());
                    }
                    self.lists[to].push(p);
                } else {
                    stay.push(p);
                }
            }
            self.lists[frame] = stay;
        }

        if self.state.active && make_particles {
            self.create_particles(data, dt, local, rng);
        }

        for frame in 0..frame_count {
            let pending = std::mem::take(&mut self.lists[frame + frame_count]);
            self.lists[frame].extend(pending);
        }
        true
    }

    /// `CParticleSystemInstance::CreateParticles`.
    fn create_particles(&mut self, data: &ParticleSystemData, dt: f32, local: &Mat4, rng: &mut GlibcRandom) {
        let emitter = &data.emitter;
        let props = &data.particle;
        let t = self.state.local_time;

        let emission = time_event_blend_value(t, &emitter.emission_count_per_second);
        let creating_value = (emission * (dt / 1.0f32)) + self.emission_residue;
        let mut creating = float_to_int(creating_value);
        self.emission_residue = creating_value - creating as f32;

        let current = self.emission_count as i32;
        let max_count = emitter.max_emission_count as i32;
        let next = current.wrapping_add(creating);
        creating = creating.wrapping_sub(0.max(next.wrapping_sub(max_count)));

        let mut life_time = 0.0;
        let mut emitting_size = 0.0;
        let mut time_position = [0.0f32; 3];
        let mut dir = [0.0f32; 3];
        let mut velocity = 0.0;
        let mut half_size = [0.0f32; 2];
        let mut lie_rotation = 0.0f32;
        if creating != 0 {
            life_time = time_event_blend_value(t, &emitter.life_time);
            if life_time == 0.0 {
                return;
            }
            emitting_size = time_event_blend_value(t, &emitter.emitting_size_events);
            time_position = data.base.position_at(t);
            dir = [
                time_event_blend_value(t, &emitter.emitting_direction_x),
                time_event_blend_value(t, &emitter.emitting_direction_y),
                time_event_blend_value(t, &emitter.emitting_direction_z),
            ];
            velocity = time_event_blend_value(t, &emitter.emitting_velocity);
            half_size = [time_event_blend_value(t, &emitter.size_x), time_event_blend_value(t, &emitter.size_y)];

            if props.billboard_type == billboard::LIE {
                let fsx = local.m[2][1];
                let fcx = (1.0f32 - (fsx * fsx)).sqrt();
                if fcx >= 0.00001f32 {
                    lie_rotation = math::to_degree((-local.m[0][1]).atan2(local.m[1][1]));
                }
            }
        }

        let mut first: Option<usize> = None; // index into `created`
        let mut created: Vec<Particle> = Vec::new();

        for _ in 0..creating.max(0) {
            let mut p = Particle { last_life_time: life_time, life_time, ..Particle::default() };

            // Position
            match emitter.emitter_shape {
                SHAPE_POINT => p.position = [0.0; 3],
                SHAPE_ELLIPSE => {
                    p.position[0] = rng.frandom(-500.0, 500.0);
                    p.position[1] = rng.frandom(-500.0, 500.0);
                    p.position[2] = 0.0;
                    p.position = math::normalize(p.position);
                    if emitter.emit_from_edge != 0 {
                        p.position = math::scale(p.position, emitter.emitting_radius + emitting_size);
                    } else {
                        p.position = math::scale(p.position, rng.frandom(0.0, emitter.emitting_radius) + emitting_size);
                    }
                }
                SHAPE_SQUARE => {
                    let s = emitter.emitting_size;
                    p.position[0] = rng.frandom(-s[0] / 2.0, s[0] / 2.0) + emitting_size;
                    p.position[1] = rng.frandom(-s[1] / 2.0, s[1] / 2.0) + emitting_size;
                    p.position[2] = rng.frandom(-s[2] / 2.0, s[2] / 2.0) + emitting_size;
                }
                SHAPE_SPHERE => {
                    p.position[0] = rng.frandom(-500.0, 500.0);
                    p.position[1] = rng.frandom(-500.0, 500.0);
                    p.position[2] = rng.frandom(-500.0, 500.0);
                    p.position = math::normalize(p.position);
                    if emitter.emit_from_edge != 0 {
                        p.position = math::scale(p.position, emitter.emitting_radius + emitting_size);
                    } else {
                        p.position = math::scale(p.position, rng.frandom(0.0, emitter.emitting_radius) + emitting_size);
                    }
                }
                _ => {}
            }

            let mut v3_time_position = time_position;
            p.position = math::add(p.position, v3_time_position);
            if props.attach == 0 {
                p.position = local.transform_coord(p.position);
                v3_time_position = local.transform_coord(v3_time_position);
            }
            p.start_position = v3_time_position;

            // Direction & velocity
            p.velocity = [0.0; 3];
            if emitter.emitter_advanced_type == ADVANCED_INNER {
                p.velocity = math::normalize(math::sub(p.position, v3_time_position));
                p.velocity = math::scale(p.velocity, -100.0);
            } else if emitter.emitter_advanced_type == ADVANCED_OUTER {
                if emitter.emitter_shape == SHAPE_POINT {
                    p.velocity[0] = rng.frandom(-100.0, 100.0);
                    p.velocity[1] = rng.frandom(-100.0, 100.0);
                    p.velocity[2] = rng.frandom(-100.0, 100.0);
                } else {
                    p.velocity = math::normalize(math::sub(p.position, v3_time_position));
                    p.velocity = math::scale(p.velocity, 100.0);
                }
            }

            let mut v3_velocity = dir;
            if props.attach == 0 {
                v3_velocity = local.transform_normal(v3_velocity);
            }
            p.velocity = math::add(p.velocity, v3_velocity);
            for k in 0..3 {
                let d = emitter.emitting_direction[k];
                if d > 0.0 {
                    p.velocity[k] += rng.frandom(-d / 2.0, d / 2.0) * 1000.0;
                }
            }
            p.velocity = math::scale(p.velocity, velocity);

            p.half_size = half_size;

            // Rotation (the first assignment is overwritten, as in C++)
            p.rotation = rng.frandom(
                f32::from(props.rotation_random_starting_begin),
                f32::from(props.rotation_random_starting_end),
            );
            if props.billboard_type == billboard::LIE {
                p.rotation += lie_rotation;
            }

            // Texture animation
            p.frame_index = 0;
            p.texture_animation_type = props.tex_ani_type;
            let frames = props.textures.len();
            if frames > 1 {
                if props.tex_ani_type == TEX_ANI_RANDOM_DIRECTION {
                    if rng.random() & 1 != 0 {
                        p.frame_index = 0;
                        p.texture_animation_type = TEX_ANI_CW;
                    } else {
                        p.frame_index = (frames - 1) as u8;
                        p.texture_animation_type = TEX_ANI_CCW;
                    }
                }
                if props.tex_ani_random_start_frame != 0 {
                    p.frame_index = rng.random_range(0, frames as i64 - 1) as u8;
                }
            }

            // Simple update
            p.last_position = math::sub(p.position, math::scale(p.velocity, dt));
            p.scale = [
                props.scale_x.first().map_or(0.0, |e| e.value),
                props.scale_y.first().map_or(0.0, |e| e.value),
            ];
            p.color = props.color.first().map_or(0, |e| e.value);

            match first {
                None => {
                    build_decorator(&mut p, props, rng);
                    first = Some(created.len());
                }
                Some(f) => {
                    // `Clone`: the same chain, fresh; three values are copied.
                    let src = &created[f];
                    p.decorators = src.decorators.clone();
                    p.air_resistance = src.air_resistance;
                    p.gravity = src.gravity;
                    p.rotation_speed = src.rotation_speed;
                }
            }
            created.push(p);
        }

        for p in created {
            let i = usize::from(p.frame_index);
            if i >= self.lists.len() {
                self.lists.resize(i + 1, Vec::new());
            }
            self.lists[i].push(p);
            self.emission_count = self.emission_count.wrapping_add(1);
        }
    }

    /// `OnRender` without the GPU: the quads to draw. Culling is left to the
    /// renderer (C++ stops at the first particle outside the frustum).
    pub fn render(&self, camera: &Camera, local: &Mat4) -> ParticleSystemDraw<'_> {
        let props = &self.data().particle;
        let attach = props.attach != 0;
        let mut quads = Vec::new();
        for frame in 0..props.textures.len() {
            for p in self.particles(frame) {
                let local_opt = Some(local);
                let mut push = |positions: [Vec3; 4]| {
                    let center = if attach {
                        [p.position[0] + local.m[3][0], p.position[1] + local.m[3][1], p.position[2] + local.m[3][2]]
                    } else {
                        p.position
                    };
                    quads.push(ParticleQuad {
                        positions,
                        color: p.color,
                        texture: frame,
                        center,
                        radius: p.radius_approximation(),
                    });
                };
                match props.billboard_type {
                    t if t < billboard::TWO_FACE => {
                        push(p.quad(props, camera, if attach { local_opt } else { None }, None));
                    }
                    billboard::TWO_FACE => {
                        let m = if attach { local_opt } else { None };
                        push(p.quad(props, camera, m, Some(math::to_radian(-30.0))));
                        push(p.quad(props, camera, m, Some(math::to_radian(30.0))));
                    }
                    billboard::THREE_FACE => {
                        let m = if attach { local_opt } else { None };
                        push(p.quad(props, camera, m, None));
                        push(p.quad(props, camera, m, Some(math::to_radian(-60.0))));
                        push(p.quad(props, camera, m, Some(math::to_radian(60.0))));
                    }
                    _ => {}
                }
            }
        }
        ParticleSystemDraw {
            src_blend: props.src_blend_type,
            dest_blend: props.dest_blend_type,
            color_operation: props.color_operation_type,
            textures: &props.textures,
            quads,
        }
    }
}

/// `CParticleSystemData::BuildDecorator`: the chain in execution order.
fn build_decorator(p: &mut Particle, props: &m2_formats::effect::ParticleProperty, rng: &mut GlibcRandom) {
    // Decorators in the order C++ adds them; each goes to the chain front.
    let mut added: Vec<Decorator> = Vec::new();

    if props.air_resistance.len() > 1 {
        added.push(Decorator::AirResistance);
        added.push(Decorator::time_float(FloatTarget::AirResistance, &props.air_resistance, p));
    } else if props.air_resistance.len() == 1 {
        p.air_resistance = props.air_resistance[0].value;
        added.push(Decorator::AirResistance);
    }

    if props.gravity.len() > 1 {
        added.push(Decorator::Gravity);
        added.push(Decorator::time_float(FloatTarget::Gravity, &props.gravity, p));
    } else if props.gravity.len() == 1 {
        p.gravity = props.gravity[0].value;
        added.push(Decorator::Gravity);
    }

    added.push(Decorator::time_color(&props.color, p));
    added.push(Decorator::time_float(FloatTarget::ScaleX, &props.scale_x, p));
    added.push(Decorator::time_float(FloatTarget::ScaleY, &props.scale_y, p));

    if props.textures.len() > 1 && f64::from(props.tex_ani_delay) > 1e-6 {
        let (n, delay) = (props.textures.len(), props.tex_ani_delay);
        match p.texture_animation_type {
            TEX_ANI_CW => added.push(Decorator::TextureCw { n: n as u32, frame_time: delay, last_frame_time: delay }),
            TEX_ANI_CCW => added.push(Decorator::TextureCcw { n: n as u8, frame_time: delay, last_frame_time: delay }),
            TEX_ANI_RANDOM_FRAME => {
                added.push(Decorator::TextureRandom { n: n as u8, frame_time: delay, last_frame_time: delay })
            }
            _ => {}
        }
    }

    let mut rotation_type = props.rotation_type;
    if props.rotation_speed == 0.0 && rotation_type != ROTATION_TIME_EVENT {
        rotation_type = 0;
    } else if rotation_type == ROTATION_RANDOM_DIRECTION {
        rotation_type = if rng.random() & 1 != 0 { ROTATION_CW } else { ROTATION_CCW };
    }
    match rotation_type {
        ROTATION_TIME_EVENT => {
            added.push(Decorator::Rotation);
            added.push(Decorator::time_float(FloatTarget::RotationSpeed, &props.rotation, p));
        }
        ROTATION_CW => {
            p.rotation_speed = props.rotation_speed;
            added.push(Decorator::Rotation);
        }
        ROTATION_CCW => {
            p.rotation_speed = -props.rotation_speed;
            added.push(Decorator::Rotation);
        }
        _ => {}
    }

    added.reverse();
    p.decorators = added;
}
