//! Drawing game content with the client's render states.

pub mod cloud;
pub mod effect;
pub mod model;
pub mod sky;
pub mod terrain;
pub mod tree;

use bevy::math::Vec4;
use m2_formats::map::EnvironmentData;
use m2_render::Fog;

/// `CMapManager::SetEnvironmentData`'s fog state as the fixed-function fog of
/// `m2-render`. Density fog is exponential at the client's fixed 0.00015.
pub fn environment_fog(environment: &EnvironmentData) -> Option<Fog> {
    let fog = environment.fog;
    if !fog.enable {
        return None;
    }
    let [r, g, b, a] = fog.color;
    let color = Vec4::new(r, g, b, a);
    Some(if fog.density { Fog::exp(color, 0.00015) } else { Fog::linear(color, fog.near_distance, fog.far_distance) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_fog_becomes_fixed_function_fog() {
        use m2_render::FogMode;
        let mut environment = EnvironmentData::default();
        assert!(environment_fog(&environment).is_none());
        environment.fog.enable = true;
        environment.fog.near_distance = 10.0;
        environment.fog.far_distance = 20.0;
        assert_eq!(environment_fog(&environment).unwrap().mode, FogMode::Linear { start: 10.0, end: 20.0 });
        environment.fog.density = true;
        assert_eq!(environment_fog(&environment).unwrap().mode, FogMode::Exp { density: 0.00015 });
    }
}
