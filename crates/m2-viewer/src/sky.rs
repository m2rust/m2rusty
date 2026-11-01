//! The map environment's procedural gradient sky (`CSkyBox`,
//! `neoeter/src/render/SkyBox.cpp`), as a Bevy background cubemap.
//!
//! The client builds the sky from `SkyBoxGradientColorVector`: four vertical
//! faces split into `upper + lower` quads, each interpolating a gradient
//! entry's two colours, plus a top face in the first entry's first colour and
//! a bottom face in the last entry's second colour. That is exactly a
//! vertical gradient cubemap, so the port bakes the same piecewise gradient
//! into a small cubemap and installs it as the camera's [`Skybox`]. Bevy draws
//! the skybox as a background, which gives the client's "sky behind
//! everything" result without its render-order problems.
//!
//! Textured sky boxes (`SkyBoxTextureRenderMode`) and the scrolling cloud
//! layer are not ported yet.

use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::Skybox;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureViewDescriptor, TextureViewDimension};
use m2_formats::map::{EnvironmentData, SkyBox};

use crate::viewer::MainCamera;

/// Rows per cubemap face. The gradient is piecewise linear with at most a few
/// entries; 64 rows keep the segment breakpoints within a pixel.
const SKY_SIZE: u32 = 64;

/// Builds the environment's gradient cubemap and installs it on the camera.
/// Returns the image handle, or `None` when the environment asks for no
/// gradient (the client's `Refresh` returns early).
pub fn install_sky(world: &mut World, environment: &EnvironmentData) -> Option<Handle<Image>> {
    let sky = &environment.sky_box;
    let upper = usize::from(sky.gradient_level_upper);
    let lower = usize::from(sky.gradient_level_lower);
    let levels = upper + lower;
    if levels == 0 || sky.gradient.len() != levels {
        clear_sky(world);
        return None;
    }

    let image = cubemap(sky, upper, lower);
    let handle = world.resource_mut::<Assets<Image>>().add(image);
    let mut query = world.query_filtered::<Entity, With<MainCamera>>();
    if let Ok(camera) = query.single(world) {
        world.entity_mut(camera).insert(Skybox {
            image: Some(handle.clone()),
            // The gradient is authored in display space; the skybox pass
            // applies the view's exposure (1.0 here) and no tonemapping.
            brightness: 1.0,
            rotation: Quat::IDENTITY,
        });
    }
    Some(handle)
}

/// Removes the sky from the camera (the next map may have none).
pub fn clear_sky(world: &mut World) {
    let mut query = world.query_filtered::<Entity, With<MainCamera>>();
    if let Ok(camera) = query.single(world) {
        world.entity_mut(camera).remove::<Skybox>();
    }
}

/// The six cube faces in wgpu order: +X, -X, +Y, -Y, +Z, -Z.
fn cubemap(sky: &SkyBox, upper: usize, lower: usize) -> Image {
    let size = SKY_SIZE;
    let mut data = vec![0u8; (size * size * 4 * 6) as usize];
    let top = to_bytes(sky.gradient[0].first);
    let bottom = to_bytes(sky.gradient[upper + lower - 1].second);
    for face in 0..6 {
        for y in 0..size {
            // Row 0 is the top of the face (v = 0). The side faces' vertical
            // gradient maps to game z = 1 .. -1; the caps are solid.
            let v = (y as f32 + 0.5) / size as f32;
            let pixel = match face {
                2 => top,
                3 => bottom,
                _ => to_bytes(sky_color(sky, 1.0 - 2.0 * v, upper, lower)),
            };
            let start = ((face * size * size + y * size) * 4) as usize;
            for x in 0..size as usize {
                let at = start + x * 4;
                data[at..at + 4].copy_from_slice(&pixel);
            }
        }
    }
    let mut image = Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.texture_view_descriptor =
        Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..Default::default() });
    image
}

/// `CSkyBox::SetSkyObjectQuadVertical` + `SetSkyColor`, as a colour at game
/// `z` in `-1..=1`. The upper quads run from `first[k]` (upper edge) to
/// `second[k]` (lower edge), and likewise below the horizon; the jump between
/// `second[k]` and `first[k + 1]` is in the client too.
fn sky_color(sky: &SkyBox, z: f32, upper: usize, lower: usize) -> [f32; 4] {
    let (index, s) = if z >= 0.0 {
        let t = (1.0 - z) * upper as f32;
        let index = (t.floor() as usize).min(upper - 1);
        (index, t - index as f32)
    } else {
        let t = (-z) * lower as f32;
        let index = (t.floor() as usize).min(lower - 1);
        (upper + index, t - index as f32)
    };
    let gradient = sky.gradient[index];
    std::array::from_fn(|i| gradient.first[i] + (gradient.second[i] - gradient.first[i]) * s)
}

fn to_bytes(color: [f32; 4]) -> [u8; 4] {
    color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use m2_formats::map::GradientColor;

    fn environment(upper: u8, lower: u8, gradient: Vec<GradientColor>) -> EnvironmentData {
        EnvironmentData {
            sky_box: SkyBox { gradient_level_upper: upper, gradient_level_lower: lower, gradient, ..SkyBox::default() },
            ..EnvironmentData::default()
        }
    }

    fn gradient(n: usize) -> Vec<GradientColor> {
        (0..n)
            .map(|i| {
                let v = i as f32;
                GradientColor { first: [v, v, v, 1.0], second: [v + 0.5, v + 0.5, v + 0.5, 1.0] }
            })
            .collect()
    }

    #[test]
    fn no_levels_installs_no_sky() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.world_mut().spawn(MainCamera);
        assert!(install_sky(app.world_mut(), &environment(0, 0, vec![])).is_none());
        let world = app.world_mut();
        let camera = world.query_filtered::<Entity, With<MainCamera>>().single(world).unwrap();
        assert!(world.entity(camera).get::<Skybox>().is_none());
    }

    #[test]
    fn a_mismatched_gradient_installs_no_sky() {
        let mut app = App::new();
        app.init_resource::<Assets<Image>>();
        app.world_mut().spawn(MainCamera);
        assert!(install_sky(app.world_mut(), &environment(2, 1, gradient(2))).is_none());
    }

    #[test]
    fn the_cubemap_has_six_solid_or_gradient_faces() {
        let sky =
            SkyBox { gradient_level_upper: 1, gradient_level_lower: 1, gradient: gradient(2), ..SkyBox::default() };
        let image = cubemap(&sky, 1, 1);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 6);
        assert_eq!(image.texture_view_descriptor.as_ref().unwrap().dimension, Some(TextureViewDimension::Cube));
        let data = image.data.as_ref().unwrap();
        let face = (SKY_SIZE * SKY_SIZE * 4) as usize;
        // +Y is solid `gradient[0].first` (0), -Y solid `gradient[1].second`
        // (1.5 clamped to 255).
        assert_eq!(&data[2 * face..2 * face + 4], &[0, 0, 0, 255]);
        assert_eq!(&data[3 * face..3 * face + 4], &[255, 255, 255, 255]);
        // A side face starts just below the top colour and ends at the
        // bottom (row centres, not the exact edges).
        assert_eq!(&data[0..4], &[2, 2, 2, 255]);
        let last = (SKY_SIZE as usize * SKY_SIZE as usize - 1) * 4;
        assert_eq!(&data[last..last + 4], &[255, 255, 255, 255]);
    }

    #[test]
    fn sky_color_follows_the_piecewise_gradient() {
        let sky =
            SkyBox { gradient_level_upper: 2, gradient_level_lower: 1, gradient: gradient(3), ..SkyBox::default() };
        assert_eq!(sky_color(&sky, 1.0, 2, 1), [0.0, 0.0, 0.0, 1.0]);
        // Upper segment 0 runs from first[0] to second[0] = 0.5.
        assert_eq!(sky_color(&sky, 0.75, 2, 1), [0.25, 0.25, 0.25, 1.0]);
        // Upper segment 1 starts at first[1] = 1.0 (the jump).
        assert_eq!(sky_color(&sky, 0.5, 2, 1), [1.0, 1.0, 1.0, 1.0]);
        // Below the horizon, segment 2 runs from first[2] = 2.0 (clamped at
        // render time) to second[2] = 2.5, halfway at z = -0.5.
        assert_eq!(sky_color(&sky, -0.5, 2, 1), [2.25, 2.25, 2.25, 1.0]);
    }
}
