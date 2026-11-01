//! `CLightData` (`SimpleLightData.cpp`): the `Group Light` element of an
//! effect, a point light.

use super::{Cursor, ElementBase, Error, TimeEventTableFloat, time_event_blend_value, token_time_event_float};

/// `CLightData`.
#[derive(Debug, Clone, PartialEq)]
pub struct LightData {
    pub base: ElementBase,
    pub max_range: f32,
    pub duration: f32,
    /// Range over time, as a factor of `max_range`.
    pub range: TimeEventTableFloat,
    /// r, g, b, a.
    pub ambient: [f32; 4],
    /// r, g, b, a.
    pub diffuse: [f32; 4],
    pub loop_flag: i32,
    pub loop_count: i32,
    pub attenuation0: f32,
    pub attenuation1: f32,
    pub attenuation2: f32,
}

impl Default for LightData {
    /// `CLightData::OnClear`.
    fn default() -> Self {
        Self {
            base: ElementBase::default(),
            max_range: 300.0,
            duration: 1.0,
            range: Vec::new(),
            ambient: [0.5, 0.5, 0.5, 1.0],
            diffuse: [0.0, 0.0, 0.0, 1.0],
            loop_flag: 0,
            loop_count: 0,
            attenuation0: 0.0,
            attenuation1: 0.1,
            attenuation2: 0.0,
        }
    }
}

impl LightData {
    /// `CEffectElementBase::LoadScript` + `CLightData::OnLoadScript` on a
    /// cleared element; what was read before a failure stays.
    pub(crate) fn load(&mut self, cursor: &mut Cursor) -> Result<(), Error> {
        if !self.base.load(cursor)? {
            return Ok(());
        }
        let node = cursor.node();
        self.duration = node.token_float("duration").unwrap_or(1.0);
        self.loop_flag = node.token_int("loopflag").unwrap_or(0);
        self.loop_count = node.token_int("loopcount").unwrap_or(0);

        macro_rules! required {
            ($field:expr, $value:expr) => {
                match $value {
                    Some(v) => $field = v,
                    None => return Ok(()),
                }
            };
        }
        required!(self.ambient, node.token_vector4("ambientcolor"));
        required!(self.diffuse, node.token_vector4("diffusecolor"));
        required!(self.max_range, node.token_float("maxrange"));
        required!(self.attenuation0, node.token_float("attenuation0"));
        required!(self.attenuation1, node.token_float("attenuation1"));
        required!(self.attenuation2, node.token_float("attenuation2"));
        self.range = token_time_event_float(node, b"timeeventrange")?.unwrap_or_default();
        Ok(())
    }

    /// `CLightData::GetRange`: the range at `time`, never negative.
    pub fn range_at(&self, time: f32) -> f32 {
        let factor = if self.range.is_empty() { 1.0 } else { time_event_blend_value(time, &self.range) };
        let range = factor * self.max_range;
        // std::max(range, 0.0f), which keeps a NaN.
        if range < 0.0 { 0.0 } else { range }
    }
}
