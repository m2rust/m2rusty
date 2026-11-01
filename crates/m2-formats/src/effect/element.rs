//! `CEffectElementBase` (`EffectElementBase.cpp`): what every effect element
//! (particle system, mesh, light) shares.

use super::{Cursor, Error};
use crate::script::atof_f32;

/// `MOVING_TYPE_DIRECT`: straight line to the next position.
pub const MOVING_TYPE_DIRECT: i32 = 0;
/// `MOVING_TYPE_BEZIER_CURVE`: quadratic curve through the control point.
pub const MOVING_TYPE_BEZIER_CURVE: i32 = 1;

/// `TEffectPosition`: one entry of `TimeEventPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EffectPosition {
    pub time: f32,
    pub position: [f32; 3],
    /// [`MOVING_TYPE_DIRECT`] or [`MOVING_TYPE_BEZIER_CURVE`]: how to move
    /// from this position to the next.
    pub moving_type: i32,
    /// Relative to `position`; zero for direct moves.
    pub control_point: [f32; 3],
}

/// `CEffectElementBase`'s data.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementBase {
    /// `m_fStartTime`.
    pub start_time: f32,
    /// `m_TimeEventTablePosition`.
    pub positions: Vec<EffectPosition>,
}

impl ElementBase {
    /// `CEffectElementBase::LoadScript` up to `OnLoadScript`: returns
    /// whether to go on (C++ returns `FALSE` before calling `OnLoadScript`
    /// on an unknown moving type, keeping the positions read so far).
    pub(crate) fn load(&mut self, cursor: &Cursor) -> Result<bool, Error> {
        let node = cursor.node();
        self.start_time = node.token_float("starttime").unwrap_or(0.0);

        let Some(tokens) = node.tokens("timeeventposition") else {
            return Ok(true);
        };
        self.positions.clear();
        // std::vector::at: reading past the end throws.
        let at = |i: usize| tokens.get(i).map(|t| t.as_slice()).ok_or(Error::Throws);
        let vector = |i: usize| -> Result<[f32; 3], Error> {
            Ok([atof_f32(at(i)?), atof_f32(at(i + 1)?), atof_f32(at(i + 2)?)])
        };
        let mut i = 0;
        while i < tokens.len() {
            let time = atof_f32(at(i)?);
            i += 1;
            let kind = at(i)?;
            let position = if kind == b"MOVING_TYPE_BEZIER_CURVE" {
                let p = EffectPosition {
                    time,
                    position: vector(i + 1)?,
                    moving_type: MOVING_TYPE_BEZIER_CURVE,
                    control_point: vector(i + 4)?,
                };
                i += 7;
                p
            } else if kind == b"MOVING_TYPE_DIRECT" {
                let p = EffectPosition {
                    time,
                    position: vector(i + 1)?,
                    moving_type: MOVING_TYPE_DIRECT,
                    control_point: [0.0; 3],
                };
                i += 4;
                p
            } else {
                return Ok(false);
            };
            self.positions.push(position);
        }
        Ok(true)
    }

    /// `CEffectElementBase::GetPosition`: where the element is at `time`.
    pub fn position_at(&self, time: f32) -> [f32; 3] {
        let table = &self.positions;
        match table.as_slice() {
            [] => return [0.0; 3],
            [only] => return only.position,
            [front, .., back] => {
                if front.time > time {
                    return front.position;
                }
                if back.time < time {
                    return back.position;
                }
            }
        }
        // std::lower_bound
        let (mut first, mut len) = (0, table.len());
        while len > 0 {
            let half = len >> 1;
            if table[first + half].time < time {
                first += half + 1;
                len -= half + 1;
            } else {
                len = half;
            }
        }
        if first == 0 {
            return table[0].position;
        }
        let (prev, next) = (&table[first - 1], &table[first]);
        match prev.moving_type {
            MOVING_TYPE_DIRECT => {
                let head = (next.time - time).abs() / (next.time - prev.time).abs();
                let tail = 1.0 - (next.time - time).abs() / (next.time - prev.time).abs();
                std::array::from_fn(|k| prev.position[k] * head + next.position[k] * tail)
            }
            MOVING_TYPE_BEZIER_CURVE => {
                let t = (time - prev.time) / (next.time - prev.time);
                std::array::from_fn(|k| {
                    prev.position[k] * (1.0 - t) * (1.0 - t)
                        + (prev.position[k] + prev.control_point[k]) * (1.0 - t) * t * 2.0
                        + next.position[k] * t * t
                })
            }
            // C++ leaves the output untouched; callers pass zero.
            _ => [0.0; 3],
        }
    }
}
