//! Shows one golden scene in a window: the Rust render on the left, the C++
//! golden on the right (both 3x, nearest filtering).
//!
//! ```text
//! cargo run -p m2-render --example golden_view -- alpha_test
//! cargo run -p m2-render --example golden_view -- alpha_test --diff /tmp/alpha_test.diff.png
//! ```
//!
//! With `--diff PATH` the example renders for a few frames, writes the diff
//! image (red = per-pixel worst channel delta) next to `PATH.actual.png`,
//! prints the statistics and exits.
//!
//! The 3D scene is drawn into an `Rgba8Unorm` image in gamma space. A
//! `Camera2d` with `CompositingSpace::Srgb` presents it, so the window shows
//! the stored values unchanged (Bevy otherwise treats window output as linear).

#[path = "../tests/golden/scenes.rs"]
mod scenes;

use std::sync::{Arc, Mutex};

use bevy::camera::CompositingSpace;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use m2_render::M2RenderPlugin;
use scenes::*;

#[derive(Resource)]
struct Args {
    scene: &'static SceneDef,
    diff: Option<std::path::PathBuf>,
}

#[derive(Resource)]
struct TargetImage(Handle<Image>);

#[derive(Resource, Default)]
struct Captured(Arc<Mutex<Option<Rgba>>>);

fn main() {
    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "alpha_test".into());
    let mut diff = None;
    while let Some(arg) = args.next() {
        if arg == "--diff" {
            diff = args.next().map(Into::into);
        }
    }
    let Some(scene) = SCENES.iter().find(|s| s.name == name) else {
        let names: Vec<_> = SCENES.iter().map(|s| s.name).collect();
        eprintln!("unknown scene {name}; scenes: {names:?}");
        std::process::exit(2);
    };
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("golden_view: {name}"),
                resolution: (WIDTH * 3 * 2, HEIGHT * 3).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(M2RenderPlugin)
        .insert_resource(Args { scene, diff })
        .init_resource::<Captured>()
        .add_systems(Startup, setup)
        .add_systems(Update, finish)
        .run();
}

fn setup(world: &mut World) {
    let scene = world.resource::<Args>().scene;
    let mut target = new_target();
    target.sampler = ImageSampler::nearest();
    let target = world.resource_mut::<Assets<Image>>().add(target);
    (scene.build)(&mut Ctx { world, target: target.clone() });

    let golden = read_png(&golden_dir().join(format!("{}.png", scene.name)));
    let mut golden_image = Image::new(
        bevy::render::render_resource::Extent3d { width: golden.width, height: golden.height, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        golden.data,
        bevy::render::render_resource::TextureFormat::Rgba8Unorm,
        bevy::asset::RenderAssetUsages::default(),
    );
    golden_image.sampler = ImageSampler::nearest();
    let golden = world.resource_mut::<Assets<Image>>().add(golden_image);

    let half = (WIDTH * 3) as f32 / 2.0;
    world.spawn((Camera2d, CompositingSpace::Srgb, Camera { order: 10, ..default() }));
    world.spawn((Sprite::from_image(target.clone()), Transform::from_xyz(-half, 0.0, 0.0).with_scale(Vec3::splat(3.0))));
    world.spawn((Sprite::from_image(golden), Transform::from_xyz(half, 0.0, 0.0).with_scale(Vec3::splat(3.0))));
    world.insert_resource(TargetImage(target));
}

fn finish(mut commands: Commands, args: Res<Args>, target: Res<TargetImage>, captured: Res<Captured>, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = &args.diff else { return };
    *frame += 1;
    if *frame == 10 {
        let slot = captured.0.clone();
        commands.spawn(Screenshot::image(target.0.clone())).observe(move |c: On<ScreenshotCaptured>| {
            *slot.lock().unwrap() = Some(Rgba { width: c.image.width(), height: c.image.height(), data: c.image.data.clone().unwrap() });
        });
    }
    let Some(actual) = captured.0.lock().unwrap().take() else { return };
    let golden = read_png(&golden_dir().join(format!("{}.png", args.scene.name)));
    let (stats, diff) = compare(&golden, &actual);
    write_png(path, &diff);
    write_png(&path.with_extension("actual.png"), &actual);
    println!("{}: {stats:?} -> {}", args.scene.name, path.display());
    exit.write(AppExit::Success);
}
