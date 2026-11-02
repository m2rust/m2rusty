//! One-shot battle effects and motion sounds: what `CEffectManager::CreateEffect`
//! and `CSoundManager::PlayCharacterSound3D` do for actors in the C++ client.
//!
//! Hit sparks (`EFFECT_HIT`), motion effect events (weapon trails, skill
//! flashes) and the `.mss` sounds of every playing motion are queued here by
//! the in-game scene and drawn/played by small systems, so the scene code does
//! not need the sound manager's system parameter.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use m2_assets::{M2Packs, coords};
use m2_audio::SoundManager;
use m2_formats::effect::EffectData;

use crate::render::effect::{EffectMeshes, EffectRenderer, instance_from_data, load_data, load_instance, load_meshes};

/// `chrmgr.EFFECT_HIT` (`playersettingmodule.py`).
pub const EFFECT_HIT: &str = "d:/ymir work/effect/hit/blow_1/blow_1_low.mse";
/// `chrmgr.EFFECT_CRITICAL` and `EFFECT_PENETRATE` (attached to the victim).
pub const EFFECT_CRITICAL: &str = "d:/ymir work/effect/hit/critical.mse";
pub const EFFECT_PENETRATE: &str = "d:/ymir work/effect/hit/gwantong.mse";
/// `playersettingmodule.RegisterCacheEffect(EFFECT_TARGET)`.
pub const EFFECT_TARGET: &str = "d:/ymir work/effect/etc/click/click_glow_select.mse";
/// `playersettingmodule.RegisterCacheEffect(EFFECT_SELECT)`, shown while the
/// pointer hovers over an actor.
pub const EFFECT_HOVER: &str = "d:/ymir work/effect/etc/click/click_select.mse";

/// The queued effects and sounds of the battle.
#[derive(Resource, Default)]
pub struct CombatFx {
    renderer: EffectRenderer,
    cache: HashMap<String, Option<(Arc<EffectData>, EffectMeshes)>>,
    /// `(Bevy position, file, check_frequency)`.
    sounds: Vec<(Vec3, String, bool)>,
    /// The original client's persistent `EFFECT_TARGET`, following the
    /// currently selected actor.
    target: Option<(u32, Vec3, m2_effect::EffectInstance)>,
    hover: Option<(u32, Vec3, m2_effect::EffectInstance)>,
}

impl CombatFx {
    /// Sets or moves the persistent target glow. Position uses collision
    /// coordinates `(x, -map_y, height)`, like the C++ actor effects.
    pub fn set_target(&mut self, packs: Option<&M2Packs>, target: Option<(u32, Vec3)>) {
        self.set_marker(packs, target, EFFECT_TARGET, false);
    }

    /// Sets the softer hover marker (`EFFECT_SELECT`) while preserving the
    /// selected actor's stronger target effect.
    pub fn set_hover(&mut self, packs: Option<&M2Packs>, target: Option<(u32, Vec3)>) {
        self.set_marker(packs, target, EFFECT_HOVER, true);
    }

    fn set_marker(&mut self, packs: Option<&M2Packs>, target: Option<(u32, Vec3)>, file: &str, hover: bool) {
        let slot = if hover { &mut self.hover } else { &mut self.target };
        let Some((vid, position)) = target else {
            *slot = None;
            return;
        };
        if let Some((current, at, _)) = slot
            && *current == vid
        {
            *at = position;
            return;
        }
        let Some(packs) = packs else {
            *slot = None;
            return;
        };
        let mut matrix = m2_effect::Mat4::rotation_yaw_pitch_roll(0.0, 0.0, 0.0);
        matrix.set_translation(position.to_array());
        let flat = std::array::from_fn(|i| matrix.m[i / 4][i % 4]);
        match load_instance(&packs.packs, file.as_bytes(), flat) {
            Ok(instance) => *slot = Some((vid, position, instance)),
            Err(error) => {
                warn!("target effect {file}: {error}");
                *slot = None;
            }
        }
    }

    /// `CreateEffect(file, position, rotation)`: `position` in Metin2 space,
    /// `rotation` yaw/pitch/roll in degrees (x = yaw).
    pub fn spawn(&mut self, packs: Option<&M2Packs>, file: &str, position: Vec3, rotation: Vec3) {
        let mut m = m2_effect::Mat4::rotation_yaw_pitch_roll(
            rotation.x.to_radians(),
            rotation.y.to_radians(),
            rotation.z.to_radians(),
        );
        m.set_translation(position.to_array());
        self.spawn_matrix(packs, file, m);
    }

    /// Creates `file` with a full world matrix (row-major, Metin2 space).
    pub fn spawn_matrix(&mut self, packs: Option<&M2Packs>, file: &str, matrix: m2_effect::Mat4) {
        self.spawn_textured(packs, file, None, matrix);
    }

    /// `SetEffectTextures` + `CreateEffect`: `file` with every particle
    /// system drawing `texture` instead of its own (the damage digits).
    pub fn spawn_with_texture(&mut self, packs: Option<&M2Packs>, file: &str, texture: &str, position: Vec3, rotation: Vec3) {
        let mut m = m2_effect::Mat4::rotation_yaw_pitch_roll(
            rotation.x.to_radians(),
            rotation.y.to_radians(),
            rotation.z.to_radians(),
        );
        m.set_translation(position.to_array());
        self.spawn_textured(packs, file, Some(texture), m);
    }

    fn spawn_textured(&mut self, packs: Option<&M2Packs>, file: &str, texture: Option<&str>, matrix: m2_effect::Mat4) {
        let path = m2_assets::paths::canonical_asset_path(file);
        let key = match texture {
            Some(texture) => format!("{path}|{}", m2_assets::paths::canonical_asset_path(texture)),
            None => path.clone(),
        };
        let entry = self.cache.entry(key.clone()).or_insert_with(|| {
            let packs = packs?;
            let name = m2_assets::paths::virtual_name(&path);
            match load_data(&packs.packs, &name).and_then(|data| load_meshes(&packs.packs, &data).map(|m| (data, m))) {
                Ok((mut data, meshes)) => {
                    if let Some(texture) = texture {
                        let mut changed = (*data).clone();
                        for particle in &mut changed.particles {
                            particle.particle.textures = vec![m2_assets::paths::virtual_name(texture)];
                        }
                        data = Arc::new(changed);
                    }
                    Some((data, meshes))
                }
                Err(error) => {
                    warn!("battle effect {key}: {error}");
                    None
                }
            }
        });
        let Some((data, meshes)) = entry else { return };
        let flat: [f32; 16] = std::array::from_fn(|i| matrix.m[i / 4][i % 4]);
        self.renderer.instances.push(instance_from_data(data.clone(), meshes, flat));
    }

    /// `PlayCharacterSound3D` at a Bevy position.
    pub fn sound(&mut self, position: Vec3, file: &str, check_frequency: bool) {
        self.sounds.push((position, file.to_owned(), check_frequency));
    }

    /// The fog the effects are drawn with (the map's).
    pub fn set_fog(&mut self, fog: Option<m2_render::Fog>) {
        self.renderer.fog = fog;
    }

    /// Steps and draws the live effects; finished ones are dropped.
    pub fn update(world: &mut World) {
        world.resource_scope(|world, mut fx: Mut<CombatFx>| {
            fx.renderer.instances.retain(|instance| instance.is_alive());
            let mut target = fx.target.take();
            let mut hover = fx.hover.take();
            if let Some((_, position, instance)) = &mut target {
                let mut matrix = m2_effect::Mat4::rotation_yaw_pitch_roll(0.0, 0.0, 0.0);
                matrix.set_translation(position.to_array());
                instance.set_global_matrix(matrix);
                fx.renderer.instances.push(instance.clone());
            }
            if let Some((_, position, instance)) = &mut hover {
                let mut matrix = m2_effect::Mat4::rotation_yaw_pitch_roll(0.0, 0.0, 0.0);
                matrix.set_translation(position.to_array());
                instance.set_global_matrix(matrix);
                fx.renderer.instances.push(instance.clone());
            }
            fx.renderer.update(world);
            if hover.is_some() {
                let instance = fx.renderer.instances.pop();
                if let (Some((vid, position, _)), Some(instance)) = (hover, instance) {
                    fx.hover = Some((vid, position, instance));
                }
            }
            if target.is_some() {
                // The target instance was appended last, so retain its
                // advanced simulation state after the renderer steps it.
                let instance = fx.renderer.instances.pop();
                if let (Some((vid, position, _)), Some(instance)) = (target, instance) {
                    fx.target = Some((vid, position, instance));
                }
            }
        });
    }
}

/// Plays the queued sounds.
pub fn play_sounds(mut fx: ResMut<CombatFx>, mut sound: SoundManager) {
    for (position, file, check) in fx.sounds.drain(..) {
        sound.play_character_3d(position, &file, check);
    }
}

/// A Metin2-space point as a Bevy position.
pub fn bevy_position(p: Vec3) -> Vec3 {
    coords::vec3(p.to_array())
}
