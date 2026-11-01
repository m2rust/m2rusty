//! The map's lighting, from its `.msenv` environment.
//!
//! C++: `Environment_Init`/`Environment_Load` (`client/src/game/MapUtil.cpp`)
//! fill `TEnvironmentData` with two `D3DLIGHT9` directional lights
//! (`DirLights[ENV_DIRLIGHT_BACKGROUND]` and `[ENV_DIRLIGHT_CHARACTER]`) and a
//! `D3DMATERIAL9`; `CMapManager::BeginEnvironment`
//! (`client/src/game/MapManager.cpp`) then sets that material and light 0
//! before drawing the map, and `CPythonBackground::SetCharacterDirLight`
//! (`client/src/ui/PythonBackground.cpp`) swaps in the character light before
//! drawing actors. The terrain pins the same lighting into its vertex colours
//! in `CTerrainPatch::SoftwareTransformPatch_UpdateTerrainLighting`
//! (`client/src/game/TerrainPatch.cpp`).
//!
//! # D3D9 → Bevy
//!
//! Bevy's physically based lights have no fixed-function equivalent, so the
//! port keeps two paths:
//!
//! * The game's own meshes (terrain, Granny models) are drawn with the
//!   client's fixed-function shaders. They take the environment's colours
//!   unchanged, gamma space, exactly like D3D9: [`SceneLight::fixed_function`]
//!   feeds `m2-render`'s [`Lighting`] and [`SceneLight::terrain_colours`] feeds
//!   the terrain shader. No exposure or tonemapping is involved (the scene
//!   camera uses `Tonemapping::None` and exposure 1.0).
//! * A Bevy [`DirectionalLight`]/[`GlobalAmbientLight`] pair mirrors the environment
//!   for any `StandardMaterial` content. With `Tonemapping::None` and exposure
//!   1.0 a white Lambertian surface returns `albedo * light_colour *
//!   illuminance / π`; an illuminance of π (≈3.14 lx) and a colour built with
//!   `Color::srgb(r, g, b)` therefore reproduce the authored gamma-space
//!   colour `(r, g, b)`. The ambient is the same combined colour with
//!   `brightness = 1`.
//!
//! `D3DRS_AMBIENT` is never set by the client (its default is black), so
//! [`SceneLight::global_ambient`] is zero; the fill comes from the light's
//! ambient and the material's emissive, as in D3D9.

use bevy::light::{CascadeShadowConfigBuilder, DirectionalLight, DirectionalLightShadowMap, GlobalAmbientLight};
use bevy::prelude::*;
use m2_assets::coords;
use m2_formats::map::{ENV_DIRLIGHT_BACKGROUND, ENV_DIRLIGHT_CHARACTER, EnvironmentData};
use m2_render::Lighting;

/// The two environment lights and the environment material, in Bevy space and
/// gamma space.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MapLighting {
    /// `DirLights[ENV_DIRLIGHT_BACKGROUND]`: terrain and static objects.
    pub background: SceneLight,
    /// `DirLights[ENV_DIRLIGHT_CHARACTER]`: actors and their equipment.
    pub character: SceneLight,
}

/// One D3D9 directional light plus the global material it is combined with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLight {
    /// Direction the light travels, in Bevy world space (`D3DLIGHT9::Direction`).
    pub direction: Vec3,
    /// `D3DLIGHT9::Diffuse`, gamma space.
    pub diffuse: Vec4,
    /// `D3DLIGHT9::Ambient`, gamma space.
    pub ambient: Vec4,
    /// `D3DMATERIAL9::Diffuse`.
    pub material_diffuse: Vec4,
    /// `D3DMATERIAL9::Ambient`.
    pub material_ambient: Vec4,
    /// `D3DMATERIAL9::Emissive`.
    pub material_emissive: Vec4,
    /// `D3DRS_AMBIENT` (zero in the client).
    pub global_ambient: Vec4,
}

impl Default for MapLighting {
    /// The select/login scene and any map without an environment: a neutral
    /// key light from above and slightly in front of the camera, with enough
    /// ambient to keep the shadow side readable. In Bevy space `-Z` is
    /// towards the select camera, so travelling towards `+Z` front-lights the
    /// characters.
    fn default() -> Self {
        let material = MaterialColours { diffuse: Vec4::ONE, ambient: Vec4::splat(0.9), emissive: Vec4::splat(0.18) };
        let light = Directional { diffuse: Vec4::new(0.65, 0.63, 0.6, 1.0), ambient: Vec4::new(0.25, 0.26, 0.3, 1.0) };
        Self {
            background: SceneLight::new(Vec3::new(0.35, -0.8, 0.55), light, material),
            character: SceneLight::new(Vec3::new(0.35, -0.8, 0.55), light, material),
        }
    }
}

/// The parts of `D3DLIGHT9` the client reads.
#[derive(Clone, Copy, Debug)]
struct Directional {
    diffuse: Vec4,
    ambient: Vec4,
}

/// The parts of `D3DMATERIAL9` the client reads.
#[derive(Clone, Copy, Debug)]
struct MaterialColours {
    diffuse: Vec4,
    ambient: Vec4,
    emissive: Vec4,
}

impl SceneLight {
    fn new(direction: Vec3, light: Directional, material: MaterialColours) -> Self {
        Self {
            direction: direction.normalize_or_zero(),
            diffuse: light.diffuse,
            ambient: light.ambient,
            material_diffuse: material.diffuse,
            material_ambient: material.ambient,
            material_emissive: material.emissive,
            global_ambient: Vec4::ZERO,
        }
    }

    /// The uniforms `FixedFunctionMaterial`'s shader reads. `Lighting`
    /// carries the light colours directly (gamma space), unlike the Bevy
    /// light below.
    pub fn fixed_function(&self) -> Lighting {
        Lighting {
            direction: self.direction,
            light_diffuse: self.diffuse,
            light_ambient: self.ambient,
            global_ambient: self.global_ambient,
            material_ambient: self.material_ambient,
            material_emissive: self.material_emissive,
        }
    }

    /// The terrain shader's `(direction, diffuse, ambient)`: the diffuse and
    /// ambient D3D9 combines with the material before evaluating the vertex.
    /// `diffuse` is `material.diffuse * light.diffuse`; `ambient` is
    /// `material.ambient * (global + light).ambient + material.emissive`.
    /// `CTerrainPatch::SoftwareTransformPatch_UpdateTerrainLighting` also
    /// limits each diffuse channel to `1 - ambient`; the shader does that.
    pub fn terrain_colours(&self) -> (Vec4, Vec4, Vec4) {
        let ambient = (self.material_ambient * (self.global_ambient + self.ambient) + self.material_emissive)
            .clamp(Vec4::ZERO, Vec4::ONE);
        let diffuse = self.material_diffuse * self.diffuse;
        (self.direction.extend(0.0), diffuse, ambient)
    }

    /// The disabled state of a `D3DLIGHT9`: `LightEnable(0, FALSE)` leaves the
    /// material's emissive only, since `D3DRS_AMBIENT` is black.
    fn disabled(mut self) -> Self {
        self.diffuse = Vec4::ZERO;
        self.ambient = Vec4::ZERO;
        self
    }

    /// The Bevy light colour: the authored gamma-space diffuse.
    fn bevy_colour(&self) -> Color {
        Color::srgb(self.diffuse.x.max(0.0), self.diffuse.y.max(0.0), self.diffuse.z.max(0.0))
    }

    /// The ambient the terrain and models see, as a Bevy ambient colour.
    fn bevy_ambient(&self) -> Color {
        let (_, _, ambient) = self.terrain_colours();
        Color::srgb(ambient.x, ambient.y, ambient.z)
    }

    /// Whether this light can light anything at all.
    fn is_enabled(&self) -> bool {
        self.diffuse.x.max(self.diffuse.y).max(self.diffuse.z) > 0.0
    }

    /// The Bevy light for `StandardMaterial` content.
    fn bevy_light(&self, shadows: bool) -> DirectionalLight {
        DirectionalLight {
            color: self.bevy_colour(),
            // See the module docs: under this camera's exposure, π gives a
            // white Lambertian surface the authored display-space colour.
            illuminance: std::f32::consts::PI,
            shadow_maps_enabled: shadows,
            ..default()
        }
    }
}

impl MapLighting {
    /// `Environment_Load`'s result as lighting. The two lights share one
    /// `Direction` in the file (`Environment_Load` copies it to both).
    pub fn from_environment(environment: &EnvironmentData) -> Self {
        let material = MaterialColours {
            diffuse: Vec4::from_array(environment.material.diffuse),
            ambient: Vec4::from_array(environment.material.ambient),
            emissive: Vec4::from_array(environment.material.emissive),
        };
        let light = |index: usize| {
            let light = &environment.lights[index];
            let scene = SceneLight::new(
                coords::vec3(light.direction),
                Directional { diffuse: Vec4::from_array(light.diffuse), ambient: Vec4::from_array(light.ambient) },
                material,
            );
            if light.enable { scene } else { scene.disabled() }
        };
        Self {
            background: light(ENV_DIRLIGHT_BACKGROUND),
            character: light(ENV_DIRLIGHT_CHARACTER),
        }
    }
}

/// The rotation whose forward (`-Z`) is the direction the light travels.
fn direction_rotation(direction: Vec3) -> Quat {
    let direction = direction.try_normalize().unwrap_or(Vec3::NEG_Y);
    Quat::from_rotation_arc(Vec3::NEG_Z, direction)
}

/// Installs `environment`'s lighting (or the neutral fallback) and spawns its
/// Bevy lights under `parent`, so despawning the map's root removes them.
/// Shadows use three 2048² cascades over the near 9 m (the camera's far
/// plane), which is cheap enough per frame and keeps blocky terrain shadow
/// edges out of the near view. The game's own materials are fixed-function
/// and do not sample the shadow maps yet; the client had no dynamic map
/// shadows either, only the blob character shadow.
pub fn install(world: &mut World, environment: Option<&EnvironmentData>, parent: Entity) {
    let lighting = environment.map_or_else(MapLighting::default, MapLighting::from_environment);
    world.insert_resource(lighting);
    world.insert_resource(GlobalAmbientLight {
        color: lighting.background.bevy_ambient(),
        brightness: 1.0,
        ..default()
    });
    // Only the background light casts shadows: the client enables one light
    // at a time (background for the map, character for actors), never both.
    for (name, light, shadows) in [
        ("environment light: background", lighting.background, true),
        ("environment light: character", lighting.character, false),
    ] {
        if !light.is_enabled() {
            continue;
        }
        world.spawn((
            Name::new(name),
            light.bevy_light(shadows),
            CascadeShadowConfigBuilder {
                num_cascades: 3,
                first_cascade_far_bound: 200.0,
                maximum_distance: 900.0,
                ..default()
            }
            .build(),
            Transform::from_rotation(direction_rotation(light.direction)),
            ChildOf(parent),
        ));
    }
}

/// Registers the neutral lighting and the shadow-map budget, so scenes spawned
/// before the first map (select/login) are lit too.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapLighting>()
            .insert_resource(DirectionalLightShadowMap { size: 2048 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment() -> EnvironmentData {
        let mut environment = EnvironmentData::default();
        environment.lights[ENV_DIRLIGHT_BACKGROUND].enable = true;
        environment.lights[ENV_DIRLIGHT_BACKGROUND].direction = [0.35, 0.56, -0.75];
        environment.lights[ENV_DIRLIGHT_BACKGROUND].diffuse = [1.0, 0.97, 0.97, 1.0];
        environment.lights[ENV_DIRLIGHT_BACKGROUND].ambient = [0.0; 4];
        environment.lights[ENV_DIRLIGHT_CHARACTER].enable = true;
        environment.lights[ENV_DIRLIGHT_CHARACTER].direction = [0.35, 0.56, -0.75];
        environment.lights[ENV_DIRLIGHT_CHARACTER].ambient = [0.15, 0.15, 0.15, 1.0];
        environment.material.diffuse = [1.0; 4];
        environment.material.ambient = [0.882, 0.855, 0.784, 1.0];
        environment.material.emissive = [0.322, 0.322, 0.412, 1.0];
        environment
    }

    #[test]
    fn the_direction_is_swapped_into_bevy_space() {
        let lighting = MapLighting::from_environment(&environment());
        // (x, y, z)_metin2 -> (x, z, y)_bevy, then normalised.
        let expected = Vec3::new(0.35, -0.75, 0.56).normalize();
        assert!((lighting.background.direction - expected).length() < 1e-5);
    }

    #[test]
    fn the_character_and_background_lights_are_separate() {
        let lighting = MapLighting::from_environment(&environment());
        assert_eq!(lighting.background.ambient, Vec4::ZERO);
        assert_eq!(lighting.character.ambient, Vec4::new(0.15, 0.15, 0.15, 1.0));
        assert_eq!(lighting.background.diffuse, Vec4::new(1.0, 0.97, 0.97, 1.0));
    }

    #[test]
    fn the_terrain_ambient_is_material_emissive_plus_the_light() {
        let lighting = MapLighting::from_environment(&environment());
        let (direction, diffuse, ambient) = lighting.background.terrain_colours();
        assert_eq!(direction, Vec3::new(0.35, -0.75, 0.56).normalize().extend(0.0));
        // Background ambient is zero, so only the material's emissive remains.
        assert!((ambient - Vec4::new(0.322, 0.322, 0.412, 1.0)).length() < 1e-5);
        assert!((diffuse - Vec4::new(1.0, 0.97, 0.97, 1.0)).length() < 1e-5);
        // The character light adds its ambient times the material ambient.
        let (_, _, ambient) = lighting.character.terrain_colours();
        assert!((ambient.x - (0.15 * 0.882 + 0.322)).abs() < 1e-5);
    }

    #[test]
    fn a_disabled_light_leaves_the_material_emissive() {
        let mut environment = environment();
        environment.lights[ENV_DIRLIGHT_BACKGROUND].enable = false;
        let lighting = MapLighting::from_environment(&environment);
        let (_, diffuse, ambient) = lighting.background.terrain_colours();
        assert_eq!(diffuse, Vec4::ZERO);
        assert_eq!(ambient, Vec4::new(0.322, 0.322, 0.412, 1.0));
    }

    #[test]
    fn the_neutral_fallback_lights_the_front_of_the_select_characters() {
        let lighting = MapLighting::default();
        // Select characters face -Z; a light travelling towards +Z lights them.
        assert!(lighting.character.direction.z > 0.0);
        let (_, _, ambient) = lighting.character.terrain_colours();
        // A readable shade side and some direct light.
        assert!((ambient.truncate().max_element() - 0.45).abs() < 1e-5);
        assert!(lighting.character.diffuse.max_element() > 0.5);
    }

    #[test]
    fn the_bevy_colour_keeps_the_authored_values() {
        let lighting = MapLighting::from_environment(&environment());
        let colour = lighting.background.bevy_colour();
        assert_eq!(colour, Color::srgb(1.0, 0.97, 0.97));
    }
}
