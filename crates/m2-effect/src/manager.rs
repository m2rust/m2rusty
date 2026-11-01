//! `CEffectManager` (`EffectManager.cpp`) without rendering and resource
//! loading: the registry of effect data and the playing instances.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::instance::{EffectInstance, MeshSource};
use crate::math::{Mat4, Vec3, to_radian};
use crate::rng::GlibcRandom;
use m2_formats::effect::{EffectData, case_crc32, string_path};

/// `CEffectManager`.
#[derive(Default)]
pub struct EffectManager {
    data: BTreeMap<u32, Arc<EffectData>>,
    instances: BTreeMap<u32, EffectInstance>,
    next_index: i32,
}

impl EffectManager {
    pub fn new() -> Self {
        Self { next_index: 1, ..Self::default() }
    }

    /// The id `RegisterEffect` files an effect under: the case-insensitive
    /// CRC of its `StringPath`.
    pub fn effect_id(name: &[u8]) -> u32 {
        case_crc32(&string_path(name))
    }

    /// `RegisterEffect`/`RegisterEffect2` with already loaded data. Returns
    /// the id. With `replace` false an existing registration is kept.
    pub fn register_effect(&mut self, name: &[u8], data: EffectData, replace: bool) -> u32 {
        let id = Self::effect_id(name);
        if replace || !self.data.contains_key(&id) {
            self.data.insert(id, Arc::new(data));
        }
        id
    }

    pub fn effect_data(&self, id: u32) -> Option<&Arc<EffectData>> {
        self.data.get(&id)
    }

    /// `GetEmptyIndex`.
    pub fn empty_index(&mut self) -> u32 {
        if self.next_index > 2_100_000_000 || self.next_index < 1 {
            self.next_index = 1;
        }
        let mut next = self.next_index;
        self.next_index += 1;
        while self.instances.contains_key(&(next as u32)) {
            next += 1;
        }
        next as u32
    }

    /// `CreateEffectInstance`: nothing is created for an unknown id or 0.
    pub fn create_effect_instance(&mut self, index: u32, id: u32, meshes: &dyn MeshSource, now: f32) {
        if id == 0 {
            return;
        }
        let Some(data) = self.data.get(&id) else {
            return;
        };
        let inst = EffectInstance::new(Arc::clone(data), meshes, now);
        self.instances.insert(index, inst);
    }

    /// `CreateEffect`: places the effect with yaw/pitch/roll in degrees
    /// (`c_rv3Rotation` is x = yaw, y = pitch, z = roll). Returns the
    /// instance index, also when the id is unknown (as in C++).
    pub fn create_effect(&mut self, id: u32, position: Vec3, rotation: Vec3, meshes: &dyn MeshSource, now: f32) -> u32 {
        let index = self.empty_index();
        self.create_effect_instance(index, id, meshes, now);
        let mut m = Mat4::rotation_yaw_pitch_roll(to_radian(rotation[0]), to_radian(rotation[1]), to_radian(rotation[2]));
        m.set_translation(position);
        if let Some(inst) = self.instances.get_mut(&index) {
            inst.set_global_matrix(m);
        }
        index
    }

    pub fn instance(&self, index: u32) -> Option<&EffectInstance> {
        self.instances.get(&index)
    }

    pub fn instance_mut(&mut self, index: u32) -> Option<&mut EffectInstance> {
        self.instances.get_mut(&index)
    }

    /// `IsAliveEffect`.
    pub fn is_alive_effect(&self, index: u32) -> bool {
        self.instances.get(&index).is_some_and(|i| i.is_alive())
    }

    /// `DestroyEffectInstance`.
    pub fn destroy_effect_instance(&mut self, index: u32) -> bool {
        self.instances.remove(&index).is_some()
    }

    /// `DeactiveEffectInstance`.
    pub fn deactive_effect_instance(&mut self, index: u32) {
        if let Some(i) = self.instances.get_mut(&index) {
            i.set_deactive();
        }
    }

    /// `DeleteAllInstances`.
    pub fn delete_all_instances(&mut self) {
        self.instances.clear();
    }

    /// `Update`: advances every instance to `now` and drops the dead ones.
    pub fn update(&mut self, now: f32, rng: &mut GlibcRandom) {
        self.instances.retain(|_, inst| {
            inst.update_to(now, rng);
            inst.is_alive()
        });
    }

    /// The instances in `Render` order: grouped by effect data (the C++
    /// sorts by the data pointer; an unstable sort, so only the grouping is
    /// meaningful).
    pub fn render_order(&self) -> Vec<(u32, &EffectInstance)> {
        let mut v: Vec<_> = self.instances.iter().map(|(&k, i)| (k, i)).collect();
        v.sort_by_key(|(_, i)| Arc::as_ptr(i.data()) as usize);
        v
    }

    /// `UpdateSound` for every instance.
    pub fn update_sound(&mut self) {
        for i in self.instances.values_mut() {
            i.update_sound();
        }
    }
}
