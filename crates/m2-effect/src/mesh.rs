//! `CEffectMeshInstance` (`EffectMeshInstance.cpp`): an animated effect mesh.

use std::sync::Arc;

use crate::element::{ElementState, FrameController};
use crate::math::{self, Mat4};
use m2_formats::effect::{EffectData, EffectMesh, PtVertex, color_to_dword, time_event_blend_value};

const MESH_BILLBOARD_ALL: u8 = 1;
const MESH_BILLBOARD_Y: u8 = 2;
const MESH_BILLBOARD_MOVE: u8 = 3;

/// One mesh of the `.mde` to draw this frame.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshDraw<'a> {
    /// Index of the mesh inside the `.mde`.
    pub mesh_index: usize,
    /// Texture name of the current texture frame, if there is one.
    pub texture: Option<&'a [u8]>,
    /// World matrix, the effect matrix already applied.
    pub world: Mat4,
    /// Source and destination blend, or `None` when blending is off.
    pub blend: Option<(u8, u8)>,
    /// Colour operation override (always set for scripted meshes).
    pub color_operation: Option<u8>,
    /// Texture factor, `0xAARRGGBB`.
    pub color: u32,
    /// Triangle list of the current mesh frame.
    pub vertices: &'a [PtVertex],
}

#[derive(Debug, Clone)]
struct TextureInstance {
    controller: FrameController,
}

/// `CEffectMeshInstance`.
#[derive(Debug, Clone)]
pub struct EffectMeshInstance {
    effect: Arc<EffectData>,
    index: usize,
    mesh: Option<Arc<EffectMesh>>,
    pub(crate) state: ElementState,
    mesh_controller: FrameController,
    textures: Vec<TextureInstance>,
}

impl EffectMeshInstance {
    /// `SetDataPointer` for `effect.meshes[index]`; `mesh` is the loaded
    /// `.mde` (`None` when the resource is missing: the element then ends at
    /// its first update, as in C++).
    pub fn new(effect: Arc<EffectData>, index: usize, mesh: Option<Arc<EffectMesh>>) -> Self {
        let script = &effect.meshes[index];
        let mut inst = Self {
            state: ElementState::new(script.base.start_time),
            mesh_controller: FrameController::default(),
            textures: Vec::new(),
            mesh: None,
            effect: Arc::clone(&effect),
            index,
        };
        let Some(mesh) = mesh else {
            return inst;
        };
        inst.mesh_controller.clear();
        inst.mesh_controller.set_max_frame(mesh.frame_count as u32);
        inst.mesh_controller.set_frame_time(script.mesh_animation_frame_delay);
        inst.mesh_controller.set_loop(script.mesh_animation_loop != 0);
        inst.mesh_controller.set_loop_count(script.mesh_animation_loop_count);
        inst.mesh_controller.set_start_frame(0);

        inst.textures = vec![TextureInstance { controller: FrameController::default() }; mesh.meshes.len()];
        for (j, tex) in inst.textures.iter_mut().enumerate() {
            let Some(data) = script.mesh_data.get(j) else {
                continue;
            };
            let c = &mut tex.controller;
            c.clear();
            c.set_max_frame(mesh.meshes[j].textures.len() as u32);
            c.set_frame_time(data.texture_animation_frame_delay);
            c.set_loop(data.texture_animation_loop_enable != 0);
            c.set_start_frame(data.texture_animation_start_frame);
        }
        inst.mesh = Some(mesh);
        inst
    }

    pub fn is_started(&self) -> bool {
        self.state.started
    }

    pub fn local_time(&self) -> f32 {
        self.state.local_time
    }

    /// Current mesh animation frame (`GetCurrentFrame`).
    pub fn mesh_frame(&self) -> u8 {
        self.mesh_controller.current_frame()
    }

    /// `(current frame, still active)` of each mesh's texture controller.
    pub fn texture_frames(&self) -> Vec<(u8, bool)> {
        let main = u32::from(self.mesh_controller.current_frame());
        self.textures.iter().map(|t| (t.controller.current_frame(), t.controller.is_active(main))).collect()
    }

    /// `CEffectMeshInstance::isActive`.
    pub fn is_playing(&self) -> bool {
        if !self.state.active || !self.mesh_controller.is_active(0) {
            return false;
        }
        let frame = u32::from(self.mesh_controller.current_frame());
        self.textures.iter().any(|t| t.controller.is_active(frame))
    }

    /// `CEffectElementBaseInstance::Update`.
    pub fn update(&mut self, dt: f32) -> bool {
        match self.state.begin(dt) {
            Some(alive) => alive,
            None => self.on_update(dt),
        }
    }

    fn on_update(&mut self, dt: f32) -> bool {
        if !self.is_playing() {
            return false;
        }
        if self.mesh_controller.is_active(0) {
            self.mesh_controller.update(dt);
        }
        for t in &mut self.textures {
            let frame = u32::from(self.mesh_controller.current_frame());
            if t.controller.is_active(frame) {
                t.controller.update(dt);
            }
        }
        true
    }

    /// `OnRender` without the GPU. `view` is the camera view matrix
    /// (`CScreen::GetViewMatrix`), `render_elapsed` the global timer's
    /// elapsed seconds (used by the moving billboard), `local` the effect's
    /// matrix.
    pub fn render(&self, view: &Mat4, render_elapsed: f32, local: &Mat4) -> Vec<MeshDraw<'_>> {
        let mut out = Vec::new();
        let Some(mesh) = &self.mesh else {
            return out;
        };
        if !self.is_playing() {
            return out;
        }
        let script = &self.effect.meshes[self.index];
        let main = u32::from(self.mesh_controller.current_frame());
        let t = self.state.local_time;

        for (i, element) in mesh.meshes.iter().enumerate() {
            let tex = &self.textures[i].controller;
            if !tex.is_active(main) {
                continue;
            }
            let data = script.mesh_data.get(i);
            let billboard = data.map_or(0, |d| d.billboard_type);

            let mut world = Mat4::IDENTITY;
            match billboard {
                MESH_BILLBOARD_ALL => {
                    let temp = Mat4::rotation_x(90.0);
                    if let Some(inv) = view.inverse() {
                        world = inv;
                    }
                    world = temp.mul(&world);
                }
                MESH_BILLBOARD_Y => {
                    let temp = view.inverse().unwrap_or(Mat4::IDENTITY);
                    world.m[0][0] = temp.m[0][0];
                    world.m[0][1] = temp.m[0][1];
                    world.m[1][0] = temp.m[1][0];
                    world.m[1][1] = temp.m[1][1];
                }
                MESH_BILLBOARD_MOVE => {
                    let position = script.base.position_at(t);
                    let last = script.base.position_at(t - render_elapsed);
                    let mut diff = math::sub(position, last);
                    if math::length_sq(diff) > 0.001f32 {
                        diff = math::normalize(diff);
                        let q = math::safe_rotation_normalized_arc([0.0, -1.0, 0.0], diff);
                        world = Mat4::rotation_quaternion(&q);
                    }
                }
                _ => {}
            }

            let blend = match data {
                Some(d) if d.blending_enable != 0 => Some((d.blending_src_type, d.blending_dest_type)),
                _ => None,
            };

            world.set_translation(script.base.position_at(t));
            world = world.mul(local);

            let color_operation = data.map(|d| d.color_operation_type);
            let color_factor = data.map_or([1.0; 4], |d| d.color_factor);
            let mut alpha = 1.0f32;
            if let Some(d) = data
                && !d.alpha.is_empty()
            {
                alpha = time_event_blend_value(t, &d.alpha);
            }

            let Some(frame) = element.frames.get(usize::from(self.mesh_controller.current_frame())) else {
                continue;
            };
            let texture_frame = tex.current_frame() as usize;
            let texture = element.textures.get(texture_frame).map(|n| n.as_slice());

            let [r, g, b, _] = color_factor;
            let color = color_to_dword([r, g, b, alpha * frame.visibility]);
            out.push(MeshDraw {
                mesh_index: i,
                texture,
                world,
                blend,
                color_operation,
                color,
                vertices: &frame.vertices,
            });
        }
        out
    }
}
