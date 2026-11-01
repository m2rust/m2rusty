//! Golden scenes: Rust re-creations of the 12 scenes in
//! `neoeter/src/render/tests/render_golden.cpp`, plus the offscreen rig that
//! renders them and the comparison against the committed PNGs.
//!
//! Included by `tests/golden.rs` and `examples/golden_view.rs` through `#[path]`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::asset::RenderAssetUsages;
use bevy::camera::{Camera, RenderTarget, Viewport};
use bevy::log::LogPlugin;
use bevy::math::{Quat, Vec3};
use bevy::mesh::{Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages};
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy::render::RenderPlugin;
use bevy::window::{ExitCondition, WindowPlugin};
use bevy::winit::WinitPlugin;
use m2_render::*;

pub const WIDTH: u32 = 160;
pub const HEIGHT: u32 = 120;
/// `kBackground` in render_golden.cpp (0xAARRGGBB).
pub const BACKGROUND: u32 = 0xFF101820;

pub fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../neoeter/src/render/tests/golden/render")
}

// ---------------------------------------------------------------------------
// Scene context
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct SceneEntity;

/// One vertex of render_golden.cpp: position, `D3DCOLOR`, UV.
#[derive(Clone, Copy)]
pub struct V {
    pub p: [f32; 3],
    pub color: u32,
    pub uv: [f32; 2],
}

pub const fn argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

pub struct Ctx<'w> {
    pub world: &'w mut World,
    pub target: Handle<Image>,
}

impl Ctx<'_> {
    pub fn spawn(&mut self, bundle: impl Bundle) -> Entity {
        self.world.spawn((SceneEntity, bundle)).id()
    }

    pub fn mesh(&mut self, vertices: &[V]) -> Handle<Mesh> {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices.iter().map(|v| v.p).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vertices.iter().map(|v| d3d_color(v.color).to_array()).collect::<Vec<_>>());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vertices.iter().map(|v| v.uv).collect::<Vec<_>>());
        self.world.resource_mut::<Assets<Mesh>>().add(mesh)
    }

    pub fn material(&mut self, material: FixedFunctionMaterial) -> Handle<FixedFunctionMaterial> {
        self.world.resource_mut::<Assets<FixedFunctionMaterial>>().add(material)
    }

    /// An `Rgba8Unorm` (non-sRGB) texture from `D3DCOLOR` pixels.
    pub fn texture(&mut self, size: u32, pixels: &[u32]) -> Handle<Image> {
        let data: Vec<u8> = pixels
            .iter()
            .flat_map(|&c| {
                let [b, g, r, a] = c.to_le_bytes();
                [r, g, b, a]
            })
            .collect();
        let image = Image::new(
            Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8Unorm,
            RenderAssetUsages::default(),
        );
        self.world.resource_mut::<Assets<Image>>().add(image)
    }

    pub fn draw(&mut self, vertices: &[V], material: FixedFunctionMaterial, transform: Transform) -> Entity {
        let mesh = self.mesh(vertices);
        let material = self.material(material);
        self.spawn((Mesh3d(mesh), MeshMaterial3d(material), transform))
    }

    pub fn camera(&mut self, order: isize, projection: FixedFunctionProjection, transform: Transform, viewport: Option<Viewport>, clear: bool) {
        let clear_color = if clear { clear_color(d3d_color(BACKGROUND)) } else { bevy::camera::ClearColorConfig::None };
        let target = RenderTarget::Image(self.target.clone().into());
        self.spawn((
            gamma_space_camera(),
            Camera { order, viewport, clear_color, ..default() },
            target,
            projection.into_projection(),
            transform,
        ));
    }

    /// `SetOrtho2D(160, 120, 1)` camera.
    pub fn ortho_camera(&mut self) {
        self.camera(0, FixedFunctionProjection::orthographic_2d(WIDTH as f32, HEIGHT as f32), Transform::IDENTITY, None, true);
    }

    /// `SetEyeCamera(eye, 0, 0, 0, up y)` + `SetPerspective(60, 160/120, 0.5, 100)`.
    pub fn perspective_camera(&mut self, eye: Vec3) {
        let projection = FixedFunctionProjection::perspective(60f32.to_radians(), WIDTH as f32 / HEIGHT as f32, 0.5, 100.0);
        self.camera(0, projection, Transform::from_translation(eye).looking_at(Vec3::ZERO, Vec3::Y), None, true);
    }
}

// ---------------------------------------------------------------------------
// Scenes (render_golden.cpp)
// ---------------------------------------------------------------------------

fn quad(left: f32, top: f32, right: f32, bottom: f32, z: f32, color: u32, uv: [f32; 4]) -> Vec<V> {
    let [u0, v0, u1, v1] = uv;
    let v = |x, y, u, v| V { p: [x, y, z], color, uv: [u, v] };
    vec![
        v(left, top, u0, v0),
        v(right, top, u1, v0),
        v(right, bottom, u1, v1),
        v(left, top, u0, v0),
        v(right, bottom, u1, v1),
        v(left, bottom, u0, v1),
    ]
}

fn flat(x: f32, y: f32, z: f32, color: u32) -> V {
    V { p: [x, y, z], color, uv: [0.0, 0.0] }
}

fn checker(size: u32, cell: u32, even: u32, odd: u32) -> Vec<u32> {
    (0..size * size)
        .map(|i| {
            let (x, y) = (i % size, i / size);
            if ((x / cell) + (y / cell)) & 1 == 1 { odd } else { even }
        })
        .collect()
}

const WHITE: u32 = 0xFFFFFFFF;
const NO_DEPTH: DepthState = DepthState::DISABLED;

/// `ResetState` leaves culling off, depth off, nearest filtering, stage 0 modulate.
fn base() -> FixedFunctionMaterial {
    FixedFunctionMaterial::untextured().cull(CullMode::None).depth(NO_DEPTH).sampler(SamplerState::NEAREST)
}

fn textured(c: &mut Ctx, size: u32, pixels: &[u32]) -> FixedFunctionMaterial {
    let tex = c.texture(size, pixels);
    FixedFunctionMaterial { textures: [Some(tex), None], ..base() }
}

fn scene_clear(c: &mut Ctx) {
    // Nothing to draw: pins the clear colour and the empty target.
    c.ortho_camera();
}

fn scene_ortho_triangle(c: &mut Ctx) {
    c.ortho_camera();
    let v = [
        flat(80.0, 10.0, -0.5, argb(255, 220, 60, 60)),
        flat(150.0, 110.0, -0.5, argb(255, 60, 220, 90)),
        flat(10.0, 110.0, -0.5, argb(255, 70, 90, 230)),
    ];
    c.draw(&v, base(), Transform::IDENTITY);
}

fn scene_ortho_blend(c: &mut Ctx) {
    c.ortho_camera();
    let mut v = quad(25.0, 25.0, 115.0, 95.0, -0.5, argb(255, 40, 80, 220), [0.0, 0.0, 1.0, 1.0]);
    v.extend(quad(70.0, 45.0, 140.0, 110.0, -0.5, argb(140, 230, 60, 50), [0.0, 0.0, 1.0, 1.0]));
    c.draw(&v, base().blend(Blend::ALPHA), Transform::IDENTITY);
}

fn scene_checker_nearest(c: &mut Ctx) {
    c.ortho_camera();
    let m = textured(c, 8, &checker(8, 2, argb(255, 235, 235, 235), argb(255, 40, 40, 40)));
    c.draw(&quad(20.0, 20.0, 140.0, 100.0, -0.5, WHITE, [0.0, 0.0, 1.0, 1.0]), m, Transform::IDENTITY);
}

fn scene_texture_modulate(c: &mut Ctx) {
    c.ortho_camera();
    let m = textured(c, 8, &checker(8, 1, argb(255, 240, 120, 40), argb(255, 40, 120, 240)));
    c.draw(&quad(20.0, 20.0, 140.0, 100.0, -0.5, argb(255, 128, 255, 128), [0.0, 0.0, 1.0, 1.0]), m, Transform::IDENTITY);
}

fn scene_texture_wrap(c: &mut Ctx) {
    c.ortho_camera();
    let m = textured(c, 8, &checker(8, 2, argb(255, 250, 250, 250), argb(255, 30, 90, 200)));
    c.draw(&quad(10.0, 15.0, 150.0, 105.0, -0.5, WHITE, [0.0, 0.0, 4.0, 3.0]), m, Transform::IDENTITY);
}

fn scene_depth_test(c: &mut Ctx) {
    c.perspective_camera(Vec3::new(0.0, 0.0, 3.0));
    let depth = DepthState { test: true, write: true, func: CompareFunc::Less };
    // Near quad first, far quad second: the far one must not overwrite it.
    let mut v = quad(-1.5, -1.5, 1.5, 1.5, -2.0, argb(255, 60, 210, 100), [0.0; 4]);
    v.extend(quad(-4.0, -4.0, 4.0, 4.0, -5.0, argb(255, 220, 60, 50), [0.0; 4]));
    c.draw(&v, base().depth(depth), Transform::IDENTITY);
}

fn scene_perspective_cube(c: &mut Ctx) {
    c.perspective_camera(Vec3::new(0.0, 0.0, 3.2));
    let s = 0.7;
    let corners = [
        [-s, -s, -s], [s, -s, -s], [s, s, -s], [-s, s, -s], [-s, -s, s], [s, -s, s], [s, s, s], [-s, s, s],
    ];
    let faces = [[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4], [3, 7, 6, 2], [0, 4, 7, 3], [1, 2, 6, 5]];
    let colors = [
        argb(255, 210, 70, 70), argb(255, 70, 200, 90), argb(255, 70, 90, 210),
        argb(255, 230, 200, 60), argb(255, 190, 70, 210), argb(255, 70, 200, 210),
    ];
    let mut v = Vec::new();
    for (face, color) in faces.iter().zip(colors) {
        for i in [0, 1, 2, 0, 2, 3] {
            v.push(flat(corners[face[i]][0], corners[face[i]][1], corners[face[i]][2], color));
        }
    }
    // D3DXMatrixRotationYawPitchRoll(yaw 32, pitch 21, roll 0): row-vector Rz*Rx*Ry == Ry*Rx*Rz for columns.
    let rotation = Quat::from_euler(EulerRot::YXZ, 32f32.to_radians(), 21f32.to_radians(), 0.0);
    let depth = DepthState { test: true, write: true, func: CompareFunc::LessEqual };
    c.draw(&v, base().depth(depth), Transform::from_rotation(rotation));
}

fn scene_cull_backface(c: &mut Ctx) {
    c.ortho_camera();
    let (w, h) = (WIDTH as f32, HEIGHT as f32);
    let culled = argb(255, 220, 50, 220);
    let visible = argb(255, 60, 220, 230);
    // Clockwise on screen (culled by D3DCULL_CW), then counter-clockwise (kept).
    let v = [
        flat(0.0, 0.0, -0.5, culled), flat(w, 0.0, -0.5, culled), flat(0.0, h, -0.5, culled),
        flat(40.0, 90.0, -0.5, visible), flat(120.0, 90.0, -0.5, visible), flat(80.0, 30.0, -0.5, visible),
    ];
    c.draw(&v, base().cull(CullMode::Cw), Transform::IDENTITY);
}

fn scene_fog_linear(c: &mut Ctx) {
    c.perspective_camera(Vec3::new(0.0, 0.0, 3.0));
    let color = argb(255, 30, 50, 160);
    let mut v = quad(-4.0, -3.0, 4.0, 3.0, -3.0, color, [0.0; 4]);
    v.extend(quad(-9.0, -9.0, 9.0, 9.0, -14.0, color, [0.0; 4]));
    let fog = Fog::linear(d3d_color(argb(255, 235, 240, 255)), 3.0, 12.0);
    let depth = DepthState { test: true, write: true, func: CompareFunc::LessEqual };
    c.draw(&v, base().depth(depth).fog(fog), Transform::IDENTITY);
}

fn scene_viewport(c: &mut Ctx) {
    // Clear the whole target, then draw the full ortho quad into a sub-rectangle.
    c.camera(0, FixedFunctionProjection::orthographic_2d(WIDTH as f32, HEIGHT as f32), Transform::IDENTITY, None, true);
    // The clearing camera must not draw the quad: put it on an empty render layer.
    let clearer = {
        let world = &mut *c.world;
        world.query_filtered::<Entity, With<Camera>>().iter(world).next().unwrap()
    };
    c.world.entity_mut(clearer).insert(bevy::camera::visibility::RenderLayers::layer(1));
    let viewport = Viewport { physical_position: UVec2::new(40, 30), physical_size: UVec2::new(80, 60), ..default() };
    c.camera(1, FixedFunctionProjection::orthographic_2d(WIDTH as f32, HEIGHT as f32), Transform::IDENTITY, Some(viewport), false);
    c.draw(&quad(0.0, 0.0, WIDTH as f32, HEIGHT as f32, -0.5, argb(255, 220, 140, 60), [0.0; 4]), base(), Transform::IDENTITY);
}

fn scene_alpha_test(c: &mut Ctx) {
    c.ortho_camera();
    let pixels: Vec<u32> = (0..64)
        .map(|i| {
            let (x, y) = (i % 8, i / 8);
            if ((x / 4) + (y / 4)) & 1 == 0 { argb(255, 250, 190, 40) } else { argb(0, 30, 60, 200) }
        })
        .collect();
    let m = textured(c, 8, &pixels).alpha_test(AlphaTest::greater(128));
    c.draw(&quad(20.0, 20.0, 140.0, 100.0, -0.5, WHITE, [0.0, 0.0, 1.0, 1.0]), m, Transform::IDENTITY);
}

/// Acceptance for one scene. The C++ rule is: a pixel matches when every
/// channel is within 2, and the scene passes at 99.5% similarity. Each scene
/// pins what was measured (see README), which is tighter.
#[derive(Clone, Copy, Debug)]
pub struct Tolerance {
    /// Largest per-channel delta allowed anywhere.
    pub max_delta: u8,
    /// Largest fraction of pixels whose worst channel differs by more than 2.
    pub max_fraction: f64,
}

pub struct SceneDef {
    pub name: &'static str,
    pub build: fn(&mut Ctx),
    pub tolerance: Tolerance,
}

const fn tol(max_delta: u8, max_fraction: f64) -> Tolerance {
    Tolerance { max_delta, max_fraction }
}

pub const SCENES: &[SceneDef] = &[
    SceneDef { name: "clear", build: scene_clear, tolerance: tol(0, 0.0) },
    SceneDef { name: "ortho_triangle", build: scene_ortho_triangle, tolerance: tol(255, 1.0) },
    SceneDef { name: "ortho_blend", build: scene_ortho_blend, tolerance: tol(255, 1.0) },
    SceneDef { name: "checker_nearest", build: scene_checker_nearest, tolerance: tol(255, 1.0) },
    SceneDef { name: "texture_modulate", build: scene_texture_modulate, tolerance: tol(255, 1.0) },
    SceneDef { name: "texture_wrap", build: scene_texture_wrap, tolerance: tol(255, 1.0) },
    SceneDef { name: "depth_test", build: scene_depth_test, tolerance: tol(255, 1.0) },
    SceneDef { name: "perspective_cube", build: scene_perspective_cube, tolerance: tol(255, 1.0) },
    SceneDef { name: "cull_backface", build: scene_cull_backface, tolerance: tol(255, 1.0) },
    SceneDef { name: "fog_linear", build: scene_fog_linear, tolerance: tol(255, 1.0) },
    SceneDef { name: "viewport", build: scene_viewport, tolerance: tol(255, 1.0) },
    SceneDef { name: "alpha_test", build: scene_alpha_test, tolerance: tol(255, 1.0) },
];

// ---------------------------------------------------------------------------
// Offscreen rig
// ---------------------------------------------------------------------------

/// Pixels as RGBA8, top row first (both the PNGs and wgpu readback are
/// top-down: the C++ capture flips GL's bottom-up `glReadPixels` before
/// writing the PNG, so no flip is needed here).
#[derive(Clone)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub struct Rig {
    app: App,
}

pub fn new_target() -> Image {
    let mut image = Image::new_fill(
        Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC | TextureUsages::TEXTURE_BINDING;
    image
}

impl Rig {
    /// Starts a headless renderer, or `None` when there is no GPU adapter.
    pub fn try_new() -> Option<Rig> {
        let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut app = App::new();
            app.add_plugins(
                DefaultPlugins
                    .set(WindowPlugin { primary_window: None, exit_condition: ExitCondition::DontExit, ..default() })
                    .set(RenderPlugin { synchronous_pipeline_compilation: true, ..default() })
                    .disable::<WinitPlugin>()
                    .disable::<LogPlugin>()
                    .disable::<PipelinedRenderingPlugin>(),
            );
            app.add_plugins(M2RenderPlugin);
            app.finish();
            app.cleanup();
            app
        }));
        match built {
            Ok(app) => Some(Rig { app }),
            Err(_) => None,
        }
    }

    /// Builds a scene, renders it to a 160x120 `Rgba8Unorm` image and reads it back.
    pub fn render(&mut self, build: &dyn Fn(&mut Ctx)) -> Rgba {
        let target = self.app.world_mut().resource_mut::<Assets<Image>>().add(new_target());
        build(&mut Ctx { world: self.app.world_mut(), target: target.clone() });
        for _ in 0..3 {
            self.app.update();
        }
        let slot: Arc<Mutex<Option<Image>>> = Arc::default();
        let sink = slot.clone();
        self.app.world_mut().spawn(Screenshot::image(target)).observe(move |captured: On<ScreenshotCaptured>| {
            *sink.lock().unwrap() = Some(captured.image.clone());
        });
        for _ in 0..120 {
            self.app.update();
            if slot.lock().unwrap().is_some() {
                break;
            }
        }
        let image = slot.lock().unwrap().take().expect("screenshot was not captured");
        let world = self.app.world_mut();
        let entities: Vec<Entity> = world.query_filtered::<Entity, With<SceneEntity>>().iter(world).collect();
        for entity in entities {
            world.despawn(entity);
        }
        self.app.update();
        assert_eq!(image.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        Rgba { width: image.width(), height: image.height(), data: image.data.expect("captured image has data") }
    }
}

// ---------------------------------------------------------------------------
// PNG + comparison
// ---------------------------------------------------------------------------

pub fn read_png(path: &Path) -> Rgba {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut reader = png::Decoder::new(std::io::BufReader::new(file)).read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    let data = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => panic!("unsupported PNG colour type {other:?}"),
    };
    Rgba { width: info.width, height: info.height, data }
}

pub fn write_png(path: &Path, image: &Rgba) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&image.data).unwrap();
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub max_delta: u8,
    /// Pixels whose worst channel differs by more than 2 (the C++ rule).
    pub differing: usize,
    pub total: usize,
    /// Differing pixels with no strong edge (> 16 levels) within one pixel in the golden.
    pub off_edge: usize,
}

impl Stats {
    pub fn fraction(&self) -> f64 {
        self.differing as f64 / self.total as f64
    }
}

pub fn compare(golden: &Rgba, actual: &Rgba) -> (Stats, Rgba) {
    assert_eq!((golden.width, golden.height), (actual.width, actual.height), "size mismatch");
    let (w, h) = (golden.width as i32, golden.height as i32);
    let px = |img: &Rgba, x: i32, y: i32| -> [u8; 4] {
        let i = ((y * w + x) * 4) as usize;
        [img.data[i], img.data[i + 1], img.data[i + 2], img.data[i + 3]]
    };
    let mut stats = Stats { total: (w * h) as usize, ..Default::default() };
    let mut diff = Rgba { width: golden.width, height: golden.height, data: vec![0; golden.data.len()] };
    for y in 0..h {
        for x in 0..w {
            let (g, a) = (px(golden, x, y), px(actual, x, y));
            let delta = (0..4).map(|c| g[c].abs_diff(a[c])).max().unwrap();
            stats.max_delta = stats.max_delta.max(delta);
            let i = ((y * w + x) * 4) as usize;
            diff.data[i..i + 4].copy_from_slice(&[delta.saturating_mul(8).max(if delta > 0 { 64 } else { 0 }), 0, 0, 255]);
            if delta > 2 {
                stats.differing += 1;
                let edge = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| {
                        let (nx, ny) = (x + dx, y + dy);
                        nx >= 0 && ny >= 0 && nx < w && ny < h && {
                            let n = px(golden, nx, ny);
                            (0..4).any(|c| n[c].abs_diff(g[c]) > 16)
                        }
                    })
                });
                if !edge {
                    stats.off_edge += 1;
                }
            }
        }
    }
    (stats, diff)
}

/// Failure images go to `$M2_GOLDEN_OUT`, else `<temp dir>/m2_render_golden`.
pub fn output_dir() -> PathBuf {
    std::env::var_os("M2_GOLDEN_OUT").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("m2_render_golden"))
}
