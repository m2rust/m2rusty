//! Orbit/pan/zoom camera, ground grid, axis gizmo, and the filtered pointer
//! input handed to viewers. Scene convention is Bevy's (Y up, right-handed).

use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, EguiGlobalSettings, PrimaryEguiContext};

use crate::ui::ScreenshotJob;
use crate::viewer::{CameraMode, MainCamera, ViewerInput, ViewerRegistry};

#[derive(Component, Clone, Copy)]
pub struct OrbitCamera {
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { focus: Vec3::ZERO, yaw: 0.6, pitch: 0.5, radius: 8.0 }
    }
}

impl OrbitCamera {
    pub fn transform(&self) -> Transform {
        let rot = Quat::from_euler(EulerRot::YXZ, self.yaw, -self.pitch, 0.0);
        Transform::from_translation(self.focus + rot * Vec3::Z * self.radius).looking_at(self.focus, Vec3::Y)
    }
}

#[derive(Resource, Default)]
pub struct ActiveCameraMode(pub CameraMode);

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        // Egui must measure the full window, not the scene viewport that its
        // own panels shrink. Sharing a camera creates a feedback loop that
        // collapses both the scene and UI to a one-pixel strip.
        app.world_mut().resource_mut::<EguiGlobalSettings>().auto_create_primary_context = false;
        app.init_resource::<ViewerInput>()
            .init_resource::<ActiveCameraMode>()
            .add_systems(Startup, setup)
            .add_systems(Update, (update_mode, gather_input, orbit_control, draw_helpers).chain());
    }
}

fn setup(mut commands: Commands) {
    let orbit = OrbitCamera::default();
    commands.spawn((
        Camera3d::default(),
        // Unlit texture previews must show the file's colours untouched.
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        bevy::core_pipeline::tonemapping::DebandDither::Disabled,
        // The viewer's custom materials write display-space colours; this
        // makes the skybox pass use the same scale (exposure 1.0).
        bevy::camera::Exposure { ev100: -(1.2f32).log2() },
        MainCamera,
        orbit,
        orbit.transform(),
    ));
    commands.spawn((
        Camera2d,
        PrimaryEguiContext,
        bevy::camera::visibility::RenderLayers::none(),
        Camera {
            order: 1,
            output_mode: bevy::camera::CameraOutputMode::Write {
                blend_state: Some(bevy::render::render_resource::BlendState::ALPHA_BLENDING),
                clear_color: ClearColorConfig::None,
            },
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
    ));
    commands.spawn((DirectionalLight::default(), Transform::from_xyz(4.0, 8.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy_egui::EguiContext;

    #[test]
    fn panel_layout_cannot_shrink_the_egui_camera() {
        let mut app = App::new();
        app.init_resource::<EguiGlobalSettings>().add_plugins(CameraPlugin);
        assert!(!app.world().resource::<EguiGlobalSettings>().auto_create_primary_context);
        let world = app.world_mut();
        world.run_system_once(setup).unwrap();

        // The scene can be resized by the panels without changing the camera
        // from which egui measures and renders the next frame's full window.
        let scene = world.query_filtered::<Entity, With<MainCamera>>().single(world).unwrap();
        assert!(!world.entity(scene).contains::<EguiContext>());
        world.get_mut::<Camera>(scene).unwrap().viewport = Some(bevy::camera::Viewport {
            physical_position: UVec2::new(380, 0),
            physical_size: UVec2::new(560, 720),
            ..default()
        });
        let (ui_camera, layers) = world
            .query_filtered::<(&Camera, &bevy::camera::visibility::RenderLayers), With<PrimaryEguiContext>>()
            .single(world)
            .unwrap();
        assert!(ui_camera.viewport.is_none());
        assert_eq!(ui_camera.order, 1);
        assert_eq!(*layers, bevy::camera::visibility::RenderLayers::none());
    }
}

fn update_mode(reg: Res<ViewerRegistry>, mut mode: ResMut<ActiveCameraMode>) {
    mode.0 = reg.active.map(|a| reg.viewers[a].camera_mode()).unwrap_or_default();
}

// Bevy systems read their state as parameters; grouping these into a custom
// `SystemParam` would only move the same list one level down.
#[allow(clippy::too_many_arguments)]
fn gather_input(
    mut input: ResMut<ViewerInput>,
    mut contexts: EguiContexts,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    job: Option<Res<ScreenshotJob>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<&Camera, With<MainCamera>>,
    mut last: Local<Option<Vec2>>,
) {
    // A screenshot must show the framed scene, not whatever the physical
    // mouse did to the orbit camera while the app waited for its frame.
    if job.is_some() {
        *input = ViewerInput::default();
        *last = window.cursor_position();
        return;
    }
    let egui_wants = contexts.ctx_mut().map(|c| c.egui_wants_pointer_input() || c.is_pointer_over_egui()).unwrap_or(false);
    let cursor = window.cursor_position();
    let in_view = match (cursor, camera.logical_viewport_rect()) {
        (Some(c), Some(r)) => r.contains(c),
        (Some(_), None) => true,
        _ => false,
    };
    input.cursor_delta = match (cursor, *last) {
        (Some(c), Some(l)) => c - l,
        _ => Vec2::ZERO,
    };
    *last = cursor;
    input.scene_hovered = in_view && !egui_wants;
    input.scroll = if input.scene_hovered {
        match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        }
    } else {
        0.0
    };
    input.left = buttons.pressed(MouseButton::Left);
    input.right = buttons.pressed(MouseButton::Right);
    input.middle = buttons.pressed(MouseButton::Middle);
}

pub(crate) fn orbit_control(
    input: Res<ViewerInput>,
    mode: Res<ActiveCameraMode>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut contexts: EguiContexts,
    mut cam: Single<(&mut OrbitCamera, &mut Transform), With<MainCamera>>,
) {
    if mode.0 != CameraMode::Orbit {
        return;
    }
    let (orbit, tf) = &mut *cam;
    let kb = !contexts.ctx_mut().map(|c| c.egui_wants_keyboard_input()).unwrap_or(false);
    let dt = time.delta_secs();
    if input.scene_hovered {
        let d = input.cursor_delta;
        if input.left {
            orbit.yaw -= d.x * 0.005;
            orbit.pitch = (orbit.pitch + d.y * 0.005).clamp(-1.5, 1.5);
        }
        if input.right || input.middle {
            let s = orbit.radius * 0.0015;
            let right = tf.right();
            let up = tf.up();
            orbit.focus += (-right * d.x + up * d.y) * s;
        }
        orbit.radius = (orbit.radius * 0.9f32.powf(input.scroll)).clamp(0.05, 300_000.0);
    }
    if kb {
        let k = |c| keys.pressed(c);
        let turn = 1.8 * dt;
        if k(KeyCode::ArrowLeft) {
            orbit.yaw += turn;
        }
        if k(KeyCode::ArrowRight) {
            orbit.yaw -= turn;
        }
        if k(KeyCode::ArrowUp) {
            orbit.pitch = (orbit.pitch + turn).clamp(-1.5, 1.5);
        }
        if k(KeyCode::ArrowDown) {
            orbit.pitch = (orbit.pitch - turn).clamp(-1.5, 1.5);
        }
        let fwd = Vec3::new(-tf.forward().x, 0.0, -tf.forward().z).normalize_or_zero() * -1.0;
        let right = tf.right().as_vec3();
        let pan = orbit.radius * dt;
        if k(KeyCode::KeyW) {
            orbit.focus += fwd * pan;
        }
        if k(KeyCode::KeyS) {
            orbit.focus -= fwd * pan;
        }
        if k(KeyCode::KeyD) {
            orbit.focus += right * pan;
        }
        if k(KeyCode::KeyA) {
            orbit.focus -= right * pan;
        }
        if k(KeyCode::KeyQ) {
            orbit.focus.y -= pan;
        }
        if k(KeyCode::KeyE) {
            orbit.focus.y += pan;
        }
        if k(KeyCode::Equal) || k(KeyCode::NumpadAdd) || k(KeyCode::PageUp) {
            orbit.radius = (orbit.radius * (1.0 - 1.5 * dt)).max(0.05);
        }
        if k(KeyCode::Minus) || k(KeyCode::NumpadSubtract) || k(KeyCode::PageDown) {
            orbit.radius = (orbit.radius * (1.0 + 1.5 * dt)).min(300_000.0);
        }
        if keys.just_pressed(KeyCode::KeyR) {
            **orbit = OrbitCamera::default();
        }
    }
    **tf = orbit.transform();
}

fn draw_helpers(mut gizmos: Gizmos, mode: Res<ActiveCameraMode>) {
    if mode.0 != CameraMode::Orbit {
        return;
    }
    let n = 10;
    let grid = Color::srgba(0.5, 0.5, 0.5, 0.35);
    for i in -n..=n {
        let f = i as f32;
        if i != 0 {
            gizmos.line(Vec3::new(f, 0.0, -n as f32), Vec3::new(f, 0.0, n as f32), grid);
            gizmos.line(Vec3::new(-n as f32, 0.0, f), Vec3::new(n as f32, 0.0, f), grid);
        }
    }
    // X red, Y green (up), Z blue.
    gizmos.arrow(Vec3::ZERO, Vec3::X * 2.0, Color::srgb(0.9, 0.2, 0.2));
    gizmos.arrow(Vec3::ZERO, Vec3::Y * 2.0, Color::srgb(0.2, 0.9, 0.2));
    gizmos.arrow(Vec3::ZERO, Vec3::Z * 2.0, Color::srgb(0.3, 0.4, 1.0));
    gizmos.line(Vec3::new(-n as f32, 0.0, 0.0), Vec3::ZERO, Color::srgba(0.9, 0.2, 0.2, 0.5));
    gizmos.line(Vec3::new(0.0, 0.0, -n as f32), Vec3::ZERO, Color::srgba(0.3, 0.4, 1.0, 0.5));
}
