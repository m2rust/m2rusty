//! `CParticleInstance` (`ParticleInstance.cpp`): one particle, its physics
//! step and its billboard quad.

use crate::decorator::{Decorator, FloatTarget, execute};
use crate::math::{self, Mat4, Quat, Vec3};
use crate::rng::GlibcRandom;
use m2_formats::effect::ParticleProperty;

/// `BILLBOARD_TYPE_*` (`Type.h`).
pub mod billboard {
    pub const NONE: u8 = 0;
    pub const ALL: u8 = 1;
    pub const Y: u8 = 2;
    pub const LIE: u8 = 3;
    pub const TWO_FACE: u8 = 4;
    pub const THREE_FACE: u8 = 5;
}

/// What a billboard needs from the camera (`CCamera::GetUp/GetCross/GetView`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub up: Vec3,
    pub cross: Vec3,
    pub view: Vec3,
}

/// The texture coordinates of the four quad corners (`__Initialize`).
pub const QUAD_UV: [[f32; 2]; 4] = [[0.0, 1.0], [0.0, 0.0], [1.0, 1.0], [1.0, 0.0]];

/// `CParticleInstance`.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle {
    pub start_position: Vec3,
    pub position: Vec3,
    pub last_position: Vec3,
    pub velocity: Vec3,
    pub half_size: [f32; 2],
    pub scale: [f32; 2],
    /// Degrees.
    pub rotation: f32,
    /// `m_dcColor`, `0xAARRGGBB`.
    pub color: u32,
    pub texture_animation_type: u8,
    /// Which texture (animation frame) the particle shows.
    pub frame_index: u8,
    pub life_time: f32,
    pub last_life_time: f32,
    pub(crate) air_resistance: f32,
    pub(crate) rotation_speed: f32,
    pub(crate) gravity: f32,
    pub(crate) decorators: Vec<Decorator>,
}

impl Default for Particle {
    /// `CParticleInstance::__Initialize`.
    fn default() -> Self {
        Self {
            start_position: [0.0; 3],
            position: [0.0; 3],
            last_position: [0.0; 3],
            velocity: [0.0; 3],
            half_size: [0.0; 2],
            scale: [1.0, 1.0],
            rotation: 0.0,
            color: 0xffffffff,
            texture_animation_type: 0,
            frame_index: 0,
            life_time: 0.0,
            last_life_time: 0.0,
            air_resistance: 0.0,
            rotation_speed: 0.0,
            gravity: 0.0,
            decorators: Vec::new(),
        }
    }
}

impl Particle {
    pub(crate) fn set_float(&mut self, target: FloatTarget, v: f32) {
        match target {
            FloatTarget::AirResistance => self.air_resistance = v,
            FloatTarget::Gravity => self.gravity = v,
            FloatTarget::RotationSpeed => self.rotation_speed = v,
            FloatTarget::ScaleX => self.scale[0] = v,
            FloatTarget::ScaleY => self.scale[1] = v,
        }
    }

    /// `GetRadiusApproximation`.
    pub fn radius_approximation(&self) -> f32 {
        (self.half_size[1] * self.scale[1]) + (self.half_size[0] * self.scale[0])
    }

    /// `CParticleInstance::Update`: false once the particle has died.
    pub(crate) fn update(
        &mut self,
        props: &ParticleProperty,
        z_axis: Vec3,
        elapsed: f32,
        mut angle: f32,
        rng: &mut GlibcRandom,
    ) -> bool {
        self.last_life_time -= elapsed;
        if self.last_life_time < 0.0 {
            return false;
        }
        let life_percentage = (self.life_time - self.last_life_time) / self.life_time;

        let mut chain = std::mem::take(&mut self.decorators);
        execute(&mut chain, self, props, life_percentage, elapsed, rng);
        self.decorators = chain;

        self.last_position = self.position;
        self.position = math::add(self.position, math::scale(self.velocity, elapsed));

        if angle != 0.0 {
            if props.attach != 0 {
                angle = math::to_radian(angle);
                let (cos, sin) = (angle.cos(), angle.sin());
                let rx = self.position[0] - self.start_position[0];
                let ry = self.position[1] - self.start_position[1];
                self.position[0] = (cos * rx) + (sin * ry) + self.start_position[0];
                self.position[1] = (-sin * rx) + (cos * ry) + self.start_position[1];
            } else {
                let q = Quat::rotation_axis(z_axis, math::to_radian(angle));
                let qc = q.conjugate();
                let mut qr = Quat::new(
                    self.position[0] - self.start_position[0],
                    self.position[1] - self.start_position[1],
                    self.position[2] - self.start_position[2],
                    0.0,
                );
                qr = q.mul(&qr);
                qr = qr.mul(&qc);
                self.position = [qr.x, qr.y, qr.z];
                self.position = math::add(self.position, self.start_position);
            }
        }
        true
    }

    /// `CParticleInstance::Transform`: the four corners of the billboard
    /// (triangle strip order). `local` is the effect matrix (used by
    /// stretched and attached particles). With `z_rotation` it is the
    /// two-argument overload the 2-face and 3-face renderers use.
    pub fn quad(
        &self,
        props: &ParticleProperty,
        camera: &Camera,
        local: Option<&Mat4>,
        z_rotation: Option<f32>,
    ) -> [Vec3; 4] {
        let mut up: Vec3;
        let mut cross: Vec3;

        if props.stretch == 0 {
            match props.billboard_type {
                billboard::LIE => {
                    let c = math::to_radian(self.rotation).cos();
                    let s = math::to_radian(self.rotation).sin();
                    up = [c, -s, 0.0];
                    cross = [s, c, 0.0];
                }
                billboard::TWO_FACE | billboard::THREE_FACE | billboard::Y => {
                    up = [0.0, 0.0, 1.0];
                    let view = camera.view;
                    if (up[0] * view[1]) - (up[1] * view[0]) < 0.0 {
                        up = math::scale(up, -1.0);
                    }
                    cross = math::normalize(math::cross(up, [view[0], view[1], 0.0]));
                    if self.rotation != 0.0 {
                        let c = -math::to_radian(self.rotation).sin();
                        let s = math::to_radian(self.rotation).cos();
                        let temp = math::sub(math::scale(up, c), math::scale(cross, s));
                        cross = math::add(math::scale(cross, c), math::scale(up, s));
                        up = temp;
                    }
                }
                _ => {
                    if self.rotation == 0.0 {
                        up = math::neg(camera.cross);
                        cross = camera.up;
                    } else if z_rotation.is_none() {
                        let view = camera.view;
                        let q = Quat::rotation_axis(view, math::to_radian(self.rotation));
                        let qc = q.conjugate();
                        let neg_cross = math::neg(camera.cross);
                        let qr = Quat::new(neg_cross[0], neg_cross[1], neg_cross[2], 0.0);
                        let qr = qc.mul(&qr);
                        let qr = qr.mul(&q);
                        up = [qr.x, qr.y, qr.z];
                        let qr = Quat::new(camera.up[0], camera.up[1], camera.up[2], 0.0);
                        let qr = qc.mul(&qr);
                        let qr = qr.mul(&q);
                        cross = [qr.x, qr.y, qr.z];
                    } else {
                        let rot = Mat4::rotation_axis(camera.view, math::to_radian(self.rotation));
                        up = rot.transform_coord(math::neg(camera.cross));
                        cross = rot.transform_coord(camera.up);
                    }
                }
            }
        } else {
            up = math::sub(self.position, self.last_position);
            if let Some(m) = local {
                up = m.transform_normal(up);
            }
            let len = math::length(up);
            if len == 0.0 {
                up = [0.0, 0.0, 1.0];
            } else {
                up = math::scale(up, (1.0f32 + (1.0f32 + len).ln()) / len);
            }
            cross = math::normalize(math::cross(up, camera.view));
        }

        if let Some(z) = z_rotation
            && z != 0.0
        {
            let (cos, sin) = (z.cos(), z.sin());
            let (x, y) = (up[0], up[1]);
            up[0] = (x * cos) - (y * sin);
            up[1] = (y * cos) + (x * sin);
            let (x, y) = (cross[0], cross[1]);
            cross[0] = (x * cos) - (y * sin);
            cross[1] = (y * cos) + (x * sin);
        }

        cross = math::scale(cross, -(self.half_size[0] * self.scale[0]));
        up = math::scale(up, self.half_size[1] * self.scale[1]);

        let base = match local {
            Some(m) if props.attach != 0 => m.transform_coord(self.position),
            _ => self.position,
        };
        [
            math::add(math::sub(base, up), cross),
            math::sub(math::sub(base, up), cross),
            math::add(math::add(base, up), cross),
            math::sub(math::add(base, up), cross),
        ]
    }
}
