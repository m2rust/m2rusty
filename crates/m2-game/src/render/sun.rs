//! The sun disc (`CLensFlare::DrawBeforeFlare`, `neoeter/src/render/LensFlare.cpp`).
//!
//! The client draws the environment's main flare texture (`MainFlareFileName`,
//! `MainFlareSize`) as a screen-space square centred where the background
//! light comes from (`Compute`: the camera target minus the light direction
//! times 10⁸, projected), between the sky and the clouds, with alpha blending
//! and no depth test, hidden while the sun is behind the camera.
//!
//! The client only draws it when the `LensFlare` group is enabled as a whole,
//! which none of the stock maps is (their `MainFlare` is on with
//! `sunflare.dds`); the port draws the disc whenever `MainFlare` is on. The
//! ghost flares and the brightening wash stay gated on `LensFlare.enable`
//! and are not ported.
//!
//! The disc is a camera-facing quad placed far beyond the world on the light
//! ray: Bevy's perspective projection has an infinite far plane, so the depth
//! test hides it behind terrain and buildings (the client gets the same from
//! drawing it before the map), and a negative depth bias sorts it before the
//! cloud layer, which covers it as `RenderCloud` does.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use m2_assets::paths;
use m2_formats::map::EnvironmentData;
use m2_render::{Blend, CullMode, DepthState, FixedFunctionMaterial};

use crate::SceneCamera;
use crate::render::lighting::MapLighting;

/// How far along the light ray the disc sits; only its screen size matters.
const SUN_DISTANCE: f32 = 1.0e6;

/// The sun disc: `MainFlareSize` (half the disc's width as a fraction of the
/// viewport width).
#[derive(Component)]
pub struct Sun {
    size: f32,
}

/// Registers the per-frame placement of the [`Sun`].
pub struct SunPlugin;

impl Plugin for SunPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, place_sun.before(TransformSystems::Propagate));
    }
}

/// Spawns the environment's sun disc under `parent`, or nothing when its
/// `MainFlare` is off.
pub fn spawn_sun(world: &mut World, environment: &EnvironmentData, parent: Entity) -> Option<Entity> {
    let flare = &environment.lens_flare;
    if !flare.main_enable || flare.main_texture_file_name.is_empty() {
        return None;
    }
    let texture = world.resource::<AssetServer>().load(paths::asset_source_uri(&flare.main_texture_file_name));
    // `MODULATE(TEXTURE, DIFFUSE)` with white, `SRCALPHA`/`INVSRCALPHA`, no
    // culling, no depth writes, no fog.
    let material = FixedFunctionMaterial::opaque(texture)
        .blend(Blend::ALPHA)
        .cull(CullMode::None)
        .depth(DepthState { write: false, ..Default::default() })
        .depth_bias(-SUN_DISTANCE);
    let material = world.resource_mut::<Assets<FixedFunctionMaterial>>().add(material);
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![[-0.5, -0.5, 0.0], [0.5, -0.5, 0.0], [0.5, 0.5, 0.0], [-0.5, 0.5, 0.0]],
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
    mesh.insert_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]));
    let mesh = world.resource_mut::<Assets<Mesh>>().add(mesh);
    let entity = world
        .spawn((
            Name::new("Sun"),
            Sun { size: flare.main_size },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::default(),
            Visibility::Hidden,
            NoFrustumCulling,
            ChildOf(parent),
        ))
        .id();
    Some(entity)
}

/// Puts the disc on the ray towards the background light, facing the camera,
/// at the client's screen size.
fn place_sun(
    lighting: Option<Res<MapLighting>>,
    cameras: Query<(&Transform, &Projection, &Camera), (With<SceneCamera>, Without<Sun>)>,
    mut suns: Query<(&Sun, &mut Transform, &mut Visibility)>,
) {
    let (Some(lighting), Ok((camera, projection, view))) = (lighting, cameras.single()) else { return };
    let towards_sun = -lighting.background.direction;
    for (sun, mut transform, mut visibility) in &mut suns {
        let visible = towards_sun != Vec3::ZERO && towards_sun.dot(*camera.forward()) > 0.0;
        let shown = if visible { Visibility::Inherited } else { Visibility::Hidden };
        visibility.set_if_neq(shown);
        let (Projection::Perspective(perspective), Some(viewport)) = (projection, view.logical_viewport_size()) else {
            continue;
        };
        if !visible || viewport.y <= 0.0 {
            continue;
        }
        // A square `2 * size * width` pixels wide, at `SUN_DISTANCE`.
        let pixels = 2.0 * sun.size * viewport.x;
        let world_per_pixel = 2.0 * SUN_DISTANCE * (perspective.fov * 0.5).tan() / viewport.y;
        let position = camera.translation + towards_sun.normalize() * SUN_DISTANCE;
        *transform = Transform::from_translation(position)
            .looking_at(camera.translation, camera.up())
            .with_scale(Vec3::splat(pixels * world_per_pixel));
    }
}
