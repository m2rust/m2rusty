//! `CLightInstance` (`SimpleLightInstance.cpp`): a dynamic point light.

use std::sync::Arc;

use crate::element::ElementState;
use crate::math::{Mat4, Vec3};
use m2_formats::effect::EffectData;

/// What `CLightManager` holds for the light (`D3DLIGHT9`, point light).
#[derive(Debug, Clone, PartialEq)]
pub struct LightState {
    /// World position.
    pub position: Vec3,
    pub range: f32,
    /// r, g, b, a.
    pub ambient: [f32; 4],
    pub diffuse: [f32; 4],
    pub attenuation: [f32; 3],
}

/// `CLightInstance`.
#[derive(Debug, Clone)]
pub struct LightInstance {
    effect: Arc<EffectData>,
    index: usize,
    pub(crate) state: ElementState,
    loop_count: u32,
    light: Option<LightState>,
}

impl LightInstance {
    /// `SetDataPointer` for `effect.lights[index]`: registers the light with
    /// the values of `CLightData::InitializeLight`.
    pub fn new(effect: Arc<EffectData>, index: usize) -> Self {
        let data = &effect.lights[index];
        let light = LightState {
            position: data.base.position_at(0.0),
            range: data.range_at(0.0),
            ambient: data.ambient,
            diffuse: data.diffuse,
            attenuation: [data.attenuation0, data.attenuation1, data.attenuation2],
        };
        Self {
            state: ElementState::new(data.base.start_time),
            loop_count: data.loop_count as u32,
            light: Some(light),
            effect,
            index,
        }
    }

    /// The light as the light manager holds it, `None` once destroyed.
    pub fn light(&self) -> Option<&LightState> {
        self.light.as_ref()
    }

    pub fn is_started(&self) -> bool {
        self.state.started
    }

    pub fn local_time(&self) -> f32 {
        self.state.local_time
    }

    /// `Destroy`: deletes the light and resets the element (it will start
    /// over on the next update, without a light; a C++ quirk kept as is).
    fn destroy(&mut self) {
        self.light = None;
        self.state.reset();
    }

    /// `CEffectElementBaseInstance::Update`.
    pub fn update(&mut self, dt: f32, local: &Mat4) -> bool {
        match self.state.begin(dt) {
            Some(alive) => alive,
            None => self.on_update(local),
        }
    }

    fn on_update(&mut self, local: &Mat4) -> bool {
        let effect = Arc::clone(&self.effect);
        let data = &effect.lights[self.index];
        if !self.state.active {
            self.destroy();
            return false;
        }
        if self.state.local_time >= data.duration {
            // `isLoop() && --m_iLoopCount != 0` on a DWORD.
            let again = data.loop_flag != 0 && {
                self.loop_count = self.loop_count.wrapping_sub(1);
                self.loop_count != 0
            };
            if again {
                self.state.local_time -= data.duration;
            } else {
                self.destroy();
                self.loop_count = 1;
                return false;
            }
        }
        if let Some(light) = &mut self.light {
            light.ambient = data.ambient;
            light.diffuse = data.diffuse;
            light.range = data.range_at(self.state.local_time);
            light.position = local.transform_coord(data.base.position_at(self.state.local_time));
        }
        true
    }
}
