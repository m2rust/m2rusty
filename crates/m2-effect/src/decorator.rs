//! `EffectUpdateDecorator.cpp/.h`: the chain of update steps each particle
//! runs every frame (time-event tables, gravity, air resistance, rotation,
//! texture animation).
//!
//! In C++ every particle owns a linked list of heap objects pointing into the
//! particle's own fields. Here a chain is a `Vec<Decorator>` in execution
//! order (the head is the last decorator `BuildDecorator` added); a finished
//! time-event decorator removes itself (`RemoveMe`), which is `Vec::remove`.
//! `CHeaderDecorator` and `CNullDecorator` do nothing and are not kept.

use crate::math::float_to_dword;
use crate::particle::Particle;
use crate::rng::GlibcRandom;
use m2_formats::effect::{ParticleProperty, TimeEvent};

/// The float field of a particle a time-event decorator writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatTarget {
    AirResistance,
    Gravity,
    RotationSpeed,
    ScaleX,
    ScaleY,
}

/// `DWORDCOLOR::operator*=(float)` followed by the copy.
pub fn color_scale(color: u32, f: f32) -> u32 {
    const AG_MASK: u32 = 0xff00ff00;
    const RB_MASK: u32 = 0x00ff00ff;
    let idx = float_to_dword(f * 256.0f32);
    (((color & AG_MASK) >> 8).wrapping_mul(idx) & AG_MASK).wrapping_add(((color & RB_MASK).wrapping_mul(idx) >> 8) & RB_MASK)
}

/// One step of a particle's update chain (`CBaseDecorator` subclasses).
#[derive(Debug, Clone, PartialEq)]
pub enum Decorator {
    /// `CAirResistanceDecorator`: `velocity *= 1 - air_resistance`.
    AirResistance,
    /// `CGravityDecorator`: `velocity.z -= gravity * dt`.
    Gravity,
    /// `CRotationDecorator`: `rotation += rotation_speed * dt`.
    Rotation,
    /// `CTimeEventDecorator<float>`: `cur`/`next` are the iterators.
    TimeFloat { target: FloatTarget, cur: usize, next: usize },
    /// `CTimeEventDecorator<DWORDCOLOR>` writing the particle colour.
    TimeColor { cur: usize, next: usize },
    /// `CTextureAnimationCWDecorator` (`n` is a `DWORD`).
    TextureCw { n: u32, frame_time: f32, last_frame_time: f32 },
    /// `CTextureAnimationCCWDecorator` (`n` is a `BYTE`).
    TextureCcw { n: u8, frame_time: f32, last_frame_time: f32 },
    /// `CTextureAnimationRandomDecorator`.
    TextureRandom { n: u8, frame_time: f32, last_frame_time: f32 },
}

impl Decorator {
    /// `CTimeEventDecorator` constructor for a float table: the iterators
    /// start at the first two events; an empty table clears the target.
    pub(crate) fn time_float(target: FloatTarget, table: &[TimeEvent<f32>], p: &mut Particle) -> Decorator {
        if table.is_empty() {
            p.set_float(target, 0.0);
            Decorator::TimeFloat { target, cur: 0, next: 0 }
        } else {
            Decorator::TimeFloat { target, cur: 0, next: 1 }
        }
    }

    /// `CTimeEventDecorator` constructor for the colour table.
    pub(crate) fn time_color(table: &[TimeEvent<u32>], p: &mut Particle) -> Decorator {
        if table.is_empty() {
            p.color = 0;
            Decorator::TimeColor { cur: 0, next: 0 }
        } else {
            Decorator::TimeColor { cur: 0, next: 1 }
        }
    }
}

/// `CBaseDecorator::Excute`: runs the chain from the head, dropping the
/// decorators that finish.
pub(crate) fn execute(
    chain: &mut Vec<Decorator>,
    p: &mut Particle,
    props: &ParticleProperty,
    life_percentage: f32,
    elapsed: f32,
    rng: &mut GlibcRandom,
) {
    let mut i = 0;
    while i < chain.len() {
        let remove = step(&mut chain[i], p, props, life_percentage, elapsed, rng);
        if remove {
            chain.remove(i);
        } else {
            i += 1;
        }
    }
}

/// Interpolation of `CTimeEventDecorator::__Excute`, generic over the value.
/// Returns the new value and whether the decorator is finished.
fn time_event<T: Copy>(
    table: &[TimeEvent<T>],
    cur: &mut usize,
    next: &mut usize,
    t: f32,
    blend: impl Fn(T, T, f32, f32) -> T,
) -> (Option<T>, bool) {
    if table.is_empty() {
        return (None, true);
    }
    if table[*cur].time > t {
        return (Some(table[*cur].value), false);
    }
    while *next != table.len() && table[*next].time <= t {
        *cur += 1;
        *next += 1;
    }
    if *next == table.len() {
        return (Some(table[*cur].value), true);
    }
    let length = table[*next].time - table[*cur].time;
    let fraction = (t - table[*cur].time) / length;
    (Some(blend(table[*cur].value, table[*next].value, 1.0f32 - fraction, fraction)), false)
}

fn step(
    d: &mut Decorator,
    p: &mut Particle,
    props: &ParticleProperty,
    life_percentage: f32,
    elapsed: f32,
    rng: &mut GlibcRandom,
) -> bool {
    match d {
        Decorator::AirResistance => {
            let f = 1.0f32 - p.air_resistance;
            p.velocity = [p.velocity[0] * f, p.velocity[1] * f, p.velocity[2] * f];
            false
        }
        Decorator::Gravity => {
            p.velocity[2] -= p.gravity * elapsed;
            false
        }
        Decorator::Rotation => {
            p.rotation += p.rotation_speed * elapsed;
            false
        }
        Decorator::TimeFloat { target, cur, next } => {
            let table = match target {
                FloatTarget::AirResistance => &props.air_resistance,
                FloatTarget::Gravity => &props.gravity,
                FloatTarget::RotationSpeed => &props.rotation,
                FloatTarget::ScaleX => &props.scale_x,
                FloatTarget::ScaleY => &props.scale_y,
            };
            let (value, done) = time_event(table, cur, next, life_percentage, |a, b, wa, wb| {
                // *pData = a * (1 - f); *pData += b * f;
                let mut v = a * wa;
                v += b * wb;
                v
            });
            if let Some(v) = value {
                p.set_float(*target, v);
            }
            done
        }
        Decorator::TimeColor { cur, next } => {
            let (value, done) = time_event(&props.color, cur, next, life_percentage, |a, b, wa, wb| {
                // DWORDCOLOR: a * wa, then += b * wb (a plain wrapping add).
                color_scale(a, wa).wrapping_add(color_scale(b, wb))
            });
            if let Some(v) = value {
                p.color = v;
            }
            done
        }
        Decorator::TextureCw { n, frame_time, last_frame_time } => {
            *last_frame_time -= elapsed;
            while *last_frame_time < 0.0 {
                *last_frame_time += *frame_time;
                p.frame_index = p.frame_index.wrapping_add(1);
                if u32::from(p.frame_index) >= *n {
                    p.frame_index = 0;
                }
            }
            false
        }
        Decorator::TextureCcw { n, frame_time, last_frame_time } => {
            *last_frame_time -= elapsed;
            while *last_frame_time < 0.0 {
                *last_frame_time += *frame_time;
                p.frame_index = p.frame_index.wrapping_sub(1);
                if p.frame_index >= *n && *n != 0 {
                    p.frame_index = n.wrapping_sub(1);
                }
            }
            false
        }
        Decorator::TextureRandom { n, frame_time, last_frame_time } => {
            *last_frame_time -= elapsed;
            if *last_frame_time < 0.0 && *n != 0 {
                p.frame_index = rng.random_range(0, i64::from(*n) - 1) as u8;
            }
            while *last_frame_time < 0.0 {
                *last_frame_time += *frame_time;
            }
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_scale_matches_dwordcolor() {
        // 0xff * (256/256) keeps the channel; half halves it.
        assert_eq!(color_scale(0xffffffff, 1.0), 0xffffffff);
        assert_eq!(color_scale(0xff80ff00, 0.5), 0x7f407f00);
        assert_eq!(color_scale(0xffffffff, 0.0), 0);
    }

    #[test]
    fn time_event_interpolates_and_finishes() {
        let table = [TimeEvent { time: 0.0, value: 0.0f32 }, TimeEvent { time: 1.0, value: 10.0 }];
        let (mut cur, mut next) = (0, 1);
        let blend = |a: f32, b: f32, wa: f32, wb: f32| {
            let mut v = a * wa;
            v += b * wb;
            v
        };
        let (v, done) = time_event(&table, &mut cur, &mut next, 0.25, blend);
        assert_eq!((v, done), (Some(2.5), false));
        let (v, done) = time_event(&table, &mut cur, &mut next, 1.5, blend);
        assert_eq!((v, done), (Some(10.0), true));
    }
}
