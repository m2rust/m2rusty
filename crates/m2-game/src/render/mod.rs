//! Drawing game content with the client's render states.

pub mod character;
pub mod cloud;
pub mod effect;
pub mod lighting;
pub mod model;
pub mod sky;
pub mod terrain;
pub mod tree;

use bevy::math::Vec4;
use bevy::prelude::*;
use m2_formats::map::EnvironmentData;
use m2_render::{Fog, FogMode};

/// Identical fixed-function materials are shared by every mesh that needs
/// them. Map objects, trees and characters otherwise create a fresh material
/// (and bind group) per instance; that both wastes GPU resources and defeats
/// Bevy's automatic batching of repeated meshes.
#[derive(Resource, Default)]
pub struct MaterialCache {
    entries: Vec<(m2_render::FixedFunctionMaterial, Handle<m2_render::FixedFunctionMaterial>)>,
}

/// Returns a shared handle for `material`, adding it on first use.
pub fn cached_material(world: &mut World, material: m2_render::FixedFunctionMaterial) -> Handle<m2_render::FixedFunctionMaterial> {
    let existing = {
        let cache = world.get_resource_or_init::<MaterialCache>();
        cache.entries.iter().find(|(cached, _)| *cached == material).map(|(_, handle)| handle.clone())
    };
    if let Some(handle) = existing {
        return handle;
    }
    let handle = world.resource_mut::<Assets<m2_render::FixedFunctionMaterial>>().add(material.clone());
    world.resource_mut::<MaterialCache>().entries.push((material, handle.clone()));
    handle
}

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

/// The in-game fog, lighter than the environment's: the client starts the
/// linear fog at 50 m (or at the eye for the desert maps) and reaches full
/// colour at 200 m, which hides most of the scene. The game keeps the near
/// view clear (fog starts at 100 m) and stretches the falloff, so the distance
/// stays hazy instead of the whole map washing out.
pub fn soften_fog(fog: Fog) -> Fog {
    match fog.mode {
        FogMode::Linear { start, end } => {
            let start = start.max(10_000.0);
            Fog::linear(fog.color, start, end.max(start + 40_000.0))
        }
        FogMode::Exp { density } => Fog::exp(fog.color, density * 0.5),
        FogMode::Exp2 { density } => Fog::exp2(fog.color, density * 0.5),
    }
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

    #[test]
    fn the_game_fog_starts_far_away_and_is_stretched() {
        let fog = soften_fog(Fog::linear(Vec4::ONE, 5000.0, 20000.0));
        assert_eq!(fog.mode, FogMode::Linear { start: 10_000.0, end: 50_000.0 });
        // A desert fog that starts at the eye still keeps the near view clear.
        let fog = soften_fog(Fog::linear(Vec4::ONE, 0.0, 30000.0));
        assert_eq!(fog.mode, FogMode::Linear { start: 10_000.0, end: 50_000.0 });
        // Density fog is halved.
        let fog = soften_fog(Fog::exp(Vec4::ONE, 0.00015));
        assert_eq!(fog.mode, FogMode::Exp { density: 0.000075 });
    }
}
