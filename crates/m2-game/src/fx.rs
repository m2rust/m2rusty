//! One-shot battle effects and motion sounds: what `CEffectManager::CreateEffect`
//! and `CSoundManager::PlayCharacterSound3D` do for actors in the C++ client.
//!
//! Hit sparks (`EFFECT_HIT`), motion effect events (weapon trails, skill
//! flashes) and the `.mss` sounds of every playing motion are queued here by
//! the in-game scene and drawn/played by small systems, so the scene code does
//! not need the sound manager's system parameter.

use std::collections::{HashMap, HashSet};
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
/// `player.RegisterCacheEffect(EFFECT_PICK)`: where a ground click sends the
/// main character (`__ShowPickedEffect`).
pub const EFFECT_PICK: &str = "d:/ymir work/effect/etc/click/click.mse";

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
    /// `__AttachEffect(EFFECT_AFFECT + n)` loops, keyed by `(vid, affect)`.
    attached: Vec<((u32, u32), m2_effect::EffectInstance)>,
    /// Finite motion effects attached to a bone or actor until they expire.
    following: Vec<(Entity, Vec3, m2_effect::EffectInstance)>,
    /// Weapon auras and quest arrows keep full bone/world transforms.
    oriented: Vec<((u32, u32), m2_effect::EffectInstance)>,
    /// Ground items' `dropitem.mse` glows.
    ground: Vec<((u32, u32), m2_effect::EffectInstance)>,
    /// `CFlyingManager`'s live flying objects (`RecvCreateFlyPacket`).
    flies: Vec<Fly>,
    /// Effect and `.msf` files that failed to load, not retried every frame.
    failed: HashSet<String>,
}

impl CombatFx {
    /// Remove effects tied to the previous map while retaining loaded assets.
    pub fn clear_scene(&mut self) {
        self.renderer.instances.clear();
        self.target = None;
        self.hover = None;
        self.attached.clear();
        self.ground.clear();
        self.oriented.clear();
        self.following.clear();
        self.flies.clear();
        self.sounds.clear();
    }

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
        if let Some((current, at, instance)) = slot
            && *current == vid && instance.is_alive()
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

    /// `AttachEffectByID`: a finite motion effect keeps following its bone.
    pub fn spawn_following(&mut self, packs: Option<&M2Packs>, file: &str, entity: Entity, offset: Vec3) {
        let (Some(packs), false) = (packs, self.failed.contains(file)) else { return };
        match load_instance(&packs.packs, file.as_bytes(), Mat4::IDENTITY.to_cols_array()) {
            Ok(instance) => self.following.push((entity, offset, instance)),
            Err(error) => {
                warn!("motion effect {file}: {error}");
                self.failed.insert(file.to_owned());
            }
        }
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

    /// Steps and draws the live effects; finished ones are dropped. The
    /// markers, affect loops and flying objects' attached effects are kept:
    /// they are appended after the one-shots and taken back once stepped.
    pub fn update(world: &mut World) {
        world.resource_scope(|world, mut fx: Mut<CombatFx>| {
            let fx = &mut *fx;
            fx.renderer.instances.retain(|instance| instance.is_alive());
            let start = fx.renderer.instances.len();
            let place = |instance: &mut m2_effect::EffectInstance, position: Vec3| {
                let mut matrix = m2_effect::Mat4::rotation_yaw_pitch_roll(0.0, 0.0, 0.0);
                matrix.set_translation(position.to_array());
                instance.set_global_matrix(matrix);
            };
            let mut kept: Vec<&mut m2_effect::EffectInstance> = Vec::new();
            if let Some((_, position, instance)) = &mut fx.target {
                place(instance, *position);
                kept.push(instance);
            }
            if let Some((_, position, instance)) = &mut fx.hover {
                place(instance, *position);
                kept.push(instance);
            }
            kept.extend(fx.attached.iter_mut().map(|(_, instance)| instance));
            kept.extend(fx.ground.iter_mut().map(|(_, instance)| instance));
            kept.extend(fx.oriented.iter_mut().map(|(_, instance)| instance));
            fx.following.retain_mut(|(entity, offset, instance)| {
                if !instance.is_alive() { return false; }
                let Some(transform) = world.get::<GlobalTransform>(*entity) else { return false };
                let matrix = transform.to_matrix() * Mat4::from_translation(*offset);
                let swap = Mat4::from_mat3(coords::SWAP_YZ);
                instance.set_global_matrix(m2_effect::Mat4::from_array((swap * matrix * swap).to_cols_array()));
                true
            });
            kept.extend(fx.following.iter_mut().map(|(_, _, instance)| instance));
            kept.extend(fx.flies.iter_mut().flat_map(|fly| fly.attach.iter_mut().map(|(_, instance)| instance)));
            fx.renderer.instances.extend(kept.iter().map(|instance| (**instance).clone()));
            fx.renderer.update(world);
            for (slot, stepped) in kept.into_iter().zip(fx.renderer.instances.drain(start..)) {
                *slot = stepped;
            }
        });
    }

    /// `__SetAffect` for every actor: `wanted` lists the `(vid, affect)` loops
    /// that should be on and where (Metin2 space). Others are detached.
    pub fn sync_attached(&mut self, packs: Option<&M2Packs>, wanted: &[((u32, u32), &'static str, Vec3)]) {
        sync_effects(&mut self.attached, &mut self.failed, packs, wanted);
    }

    /// Loops attached with an entire bone transform, plus quest target arrows.
    pub fn sync_oriented(&mut self, packs: Option<&M2Packs>, wanted: &[((u32, u32), &'static str, Mat4)]) {
        self.oriented.retain(|(key, instance)| instance.is_alive() && wanted.iter().any(|(wanted, ..)| wanted == key));
        for (key, file, matrix) in wanted {
            // Bevy and the effect simulator use opposite matrix conventions
            // and swapped up axes. The swap is its own inverse.
            let swap = Mat4::from_mat3(coords::SWAP_YZ);
            let flat = (swap * *matrix * swap).to_cols_array();
            if let Some((_, instance)) = self.oriented.iter_mut().find(|(current, _)| current == key) {
                instance.set_global_matrix(m2_effect::Mat4::from_array(flat));
                continue;
            }
            let (Some(packs), false) = (packs, self.failed.contains(*file)) else { continue };
            match load_instance(&packs.packs, file.as_bytes(), flat) {
                Ok(instance) => self.oriented.push((*key, instance)),
                Err(error) => {
                    warn!("attached effect {file}: {error}");
                    self.failed.insert((*file).to_owned());
                }
            }
        }
    }

    /// The `dropitem.mse` glow of every ground item, keyed by `(item vid, 0)`.
    pub fn sync_ground(&mut self, packs: Option<&M2Packs>, wanted: &[((u32, u32), &'static str, Vec3)]) {
        sync_effects(&mut self.ground, &mut self.failed, packs, wanted);
    }

    /// `CFlyingManager::CreateIndexedFly`: `file` (an `.msf`) flies from
    /// `start` (Metin2 space) to the actor `target`.
    pub fn fly(&mut self, packs: Option<&M2Packs>, file: &str, start: Vec3, target: u32, target_at: Vec3) {
        let (Some(packs), false) = (packs, self.failed.contains(file)) else { return };
        let (data, ok) = m2_formats::effect::load_flying_data(&packs.packs, m2_assets::paths::virtual_name(file));
        if !ok {
            warn!("flying data {file} did not load");
            self.failed.insert(file.to_owned());
            return;
        }
        let seed = (start.x.to_bits() ^ start.y.to_bits().rotate_left(7)) ^ target;
        let mut fly = Fly::new(Arc::new(data), start, target, target_at, seed);
        for (index, attach) in fly.data.attach.iter().enumerate() {
            // `FLY_ATTACH_EFFECT`; objects are not implemented in C++ either.
            if attach.kind != 1 {
                continue;
            }
            let name = String::from_utf8_lossy(&attach.file_name).into_owned();
            if self.failed.contains(&name) {
                continue;
            }
            match load_instance(&packs.packs, &attach.file_name, fly.attach_matrix(index, 0.0)) {
                Ok(instance) => fly.attach.push((index, instance)),
                Err(error) => {
                    warn!("flying effect {name}: {error}");
                    self.failed.insert(name);
                }
            }
        }
        self.flies.push(fly);
    }

    /// The actors the flying objects head for.
    pub fn fly_targets(&self) -> Vec<u32> {
        self.flies.iter().map(|fly| fly.target).collect()
    }

    /// `CFlyingInstance::Update` for every flying object, homing on its
    /// target's current position (`None` once the target is gone). A hit
    /// spawns the data's bomb effect.
    pub fn update_flies(&mut self, packs: Option<&M2Packs>, dt: f32, target_position: impl Fn(u32) -> Option<Vec3>) -> Vec<u32> {
        let mut bombs = Vec::new();
        let mut hits = Vec::new();
        self.flies.retain_mut(|fly| {
            let target = target_position(fly.target);
            if let Some(target) = target {
                fly.target_at = target;
            }
            match fly.update(dt) {
                FlyState::Flying => true,
                FlyState::Hit => {
                    if fly.target != 0 && target.is_some() { hits.push(fly.target); }
                    if !fly.data.bomb_effect_name.is_empty() {
                        bombs.push((String::from_utf8_lossy(&fly.data.bomb_effect_name).into_owned(), fly.position));
                    }
                    false
                }
                FlyState::OutOfRange => false,
            }
        });
        for (file, position) in bombs {
            self.spawn(packs, &file, position, Vec3::ZERO);
        }
        hits
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

/// `item.SetUseSoundFileName(USESOUND_POTION)` (`playersettingmodule.py`).
pub const USESOUND_POTION: &str = "sound/ui/eat_potion.wav";

/// `RecvSpecialEffect`: the effect `playersettingmodule.py` registers for a
/// `SPECIAL_EFFECT` type, and whether it is a potion (`bPlayPotionSound`).
/// Types with no registered effect (block, dodge, success, fail) give `None`.
pub fn special_effect(kind: u8) -> Option<(&'static str, bool)> {
    Some(match kind {
        1 => ("d:/ymir work/effect/etc/recuperation/drugup_red.mse", true),
        2 => ("d:/ymir work/effect/etc/recuperation/drugup_blue.mse", true),
        3 => ("d:/ymir work/effect/etc/recuperation/drugup_green.mse", true),
        4 => ("d:/ymir work/effect/etc/recuperation/drugup_purple.mse", true),
        5 => (EFFECT_CRITICAL, false),
        6 => (EFFECT_PENETRATE, false),
        9 => ("d:/ymir work/effect/etc/firecracker/newyear_firecracker.mse", false),
        10 => ("d:/ymir work/effect/etc/firecracker/paing_i.mse", false),
        14 => ("season1/effect/paymessage_warning.mse", false),
        15 => ("season1/effect/paymessage_decide.mse", false),
        16 => ("d:/ymir work/effect/hit/percent_damage1.mse", false),
        17 => ("d:/ymir work/effect/hit/percent_damage2.mse", false),
        18 => ("d:/ymir work/effect/hit/percent_damage3.mse", false),
        19 => ("d:/ymir work/effect/etc/recuperation/autodrugup_red.mse", false),
        20 => ("d:/ymir work/effect/etc/recuperation/autodrugup_blue.mse", false),
        21 => ("d:/ymir work/effect/etc/buff/buff_item1.mse", false),
        22 => ("d:/ymir work/effect/etc/buff/buff_item2.mse", false),
        23 => ("d:/ymir work/effect/etc/buff/buff_item3.mse", false),
        24 => ("d:/ymir work/effect/etc/buff/buff_item4.mse", false),
        _ => return None,
    })
}

/// `chrmgr.RegisterEffect(chrmgr.EFFECT_AFFECT + n, bone, file)`
/// (`playersettingmodule.py`): the looping effect `__SetAffect` attaches
/// while affect `n` is on, and the bone it follows (`""` is the actor root).
/// Invisibility (1) and the warrior's weapon aura (15) are handled separately.
pub fn affect_effect(affect: u32) -> Option<(&'static str, &'static str)> {
    Some(match affect {
        0 => ("Bip01", "locale/en/effect/gm.mse"),
        3 => ("Bip01", "d:/ymir work/effect/hit/blow_poison/poison_loop.mse"),
        4 => ("", "d:/ymir work/effect/affect/slow.mse"),
        5 => ("Bip01 Head", "d:/ymir work/effect/etc/stun/stun_loop.mse"),
        6 => ("", "d:/ymir work/effect/etc/ready/ready.mse"),
        16 => ("", "d:/ymir work/pc/warrior/effect/gyeokgongjang_loop.mse"),
        17 => ("", "d:/ymir work/pc/assassin/effect/gyeonggong_loop.mse"),
        19 => ("Bip01 R Finger2", "d:/ymir work/pc/sura/effect/gwigeom_loop.mse"),
        20 => ("", "d:/ymir work/pc/sura/effect/fear_loop.mse"),
        21 => ("", "d:/ymir work/pc/sura/effect/jumagap_loop.mse"),
        22 => ("", "d:/ymir work/pc/shaman/effect/3hosin_loop.mse"),
        23 => ("", "d:/ymir work/pc/shaman/effect/boho_loop.mse"),
        24 => ("", "d:/ymir work/pc/shaman/effect/10kwaesok_loop.mse"),
        25 => ("", "d:/ymir work/pc/sura/effect/heuksin_loop.mse"),
        26 => ("", "d:/ymir work/pc/sura/effect/muyeong_loop.mse"),
        28 => ("Bip01", "d:/ymir work/effect/hit/blow_flame/flame_loop.mse"),
        29 => ("Bip01 R Hand", "d:/ymir work/pc/shaman/effect/6gicheon_hand.mse"),
        30 => ("Bip01 L Hand", "d:/ymir work/pc/shaman/effect/jeungryeok_hand.mse"),
        32 => ("Bip01 Head", "d:/ymir work/pc/sura/effect/pabeop_loop.mse"),
        33 => ("", "d:/ymir work/pc/warrior/effect/gyeokgongjang_loop.mse"),
        35 => ("", "d:/ymir work/effect/etc/guild_war_flag/flag_red.mse"),
        36 => ("", "d:/ymir work/effect/etc/guild_war_flag/flag_blue.mse"),
        37 => ("", "d:/ymir work/effect/etc/guild_war_flag/flag_yellow.mse"),
        _ => return None,
    })
}

/// `__Warrior_SetGeomgyeongAffect`: the right-hand aura for the weapon mode.
pub fn sword_aura(two_handed: bool) -> &'static str {
    if two_handed { "d:/ymir work/pc/warrior/effect/geom_spear_loop.mse" }
    else { "d:/ymir work/pc/warrior/effect/geom_sword_loop.mse" }
}

pub const EFFECT_QUEST_TARGET: &str = "d:/ymir work/effect/etc/direction/direction_land.mse";

/// `AFFECT_GYEONGGONG`/`AFFECT_KWAESOK` are attached only while running.
pub fn affect_effect_needs_walking(affect: u32) -> bool {
    matches!(affect, 17 | 24)
}

/// `CInstanceBase::__GetRefinedEffect`: effect key, attachment and script.
/// The attachment is `right`, `left`, or a body bone name.
pub fn refined_effects(vnum: u32, item: &m2_formats::proto::ItemData) -> Vec<(u32, &'static str, &'static str)> {
    const SWORD: [&str; 3] = ["d:/ymir work/pc/common/effect/sword/sword_7.mse", "d:/ymir work/pc/common/effect/sword/sword_8.mse", "d:/ymir work/pc/common/effect/sword/sword_9.mse"];
    const SMALL: [&str; 3] = ["d:/ymir work/pc/common/effect/sword/sword_7_s.mse", "d:/ymir work/pc/common/effect/sword/sword_8_s.mse", "d:/ymir work/pc/common/effect/sword/sword_9_s.mse"];
    const BOW: [&str; 3] = ["d:/ymir work/pc/common/effect/sword/sword_7_b.mse", "d:/ymir work/pc/common/effect/sword/sword_8_b.mse", "d:/ymir work/pc/common/effect/sword/sword_9_b.mse"];
    const FAN: [&str; 3] = ["d:/ymir work/pc/common/effect/sword/sword_7_f.mse", "d:/ymir work/pc/common/effect/sword/sword_8_f.mse", "d:/ymir work/pc/common/effect/sword/sword_9_f.mse"];
    const ARMOR: [&str; 3] = ["d:/ymir work/pc/common/effect/armor/armor_7.mse", "d:/ymir work/pc/common/effect/armor/armor_8.mse", "d:/ymir work/pc/common/effect/armor/armor_9.mse"];
    let mut effects = Vec::new();
    if item.table.item_type == 2 && item.table.sub_type == 0 && (12010..=12049).contains(&vnum) {
        effects.push((190, "Bip01", "d:/ymir work/pc/common/effect/armor/armor-4-2-1.mse"));
        effects.push((191, "Bip01", "d:/ymir work/pc/common/effect/armor/armor-4-2-2.mse"));
    }
    let refine = (vnum % 10 + u32::from(item.table.gain_socket_pct)).saturating_sub(3);
    if !(7..=9).contains(&refine) { return effects; }
    let index = (refine - 7) as usize;
    match (item.table.item_type, item.table.sub_type) {
        (1, subtype) => {
            let files = match subtype {
                1 | 4 | 6 => &SMALL,
                2 => &BOW,
                5 => &FAN,
                _ => &SWORD,
            };
            effects.push((100 + refine * 10 + u32::from(subtype), "right", files[index]));
            if subtype == 1 {
                effects.push((200 + refine * 10, "left", SMALL[index]));
            }
        }
        (2, 0) => effects.push((300 + refine, "Bip01", ARMOR[index])),
        _ => {}
    }
    effects
}

/// Where a [`Fly`] stands after a step.
enum FlyState {
    Flying,
    /// `OnExplodingAtTarget`: it reached `m_fBombRange` of the target.
    Hit,
    /// `OnExplodingOutOfRange`: `m_fRange` used up, no bomb.
    OutOfRange,
}

/// `CFlyingInstance` towards an actor (`CFlyTarget::TYPE_OBJECT`), in Metin2
/// space. Quaternions follow glam's order: `a * b` applies `b` first (D3DX's
/// `Multiply(b, a)`). Tails (`CFlyTrace`) are not drawn.
struct Fly {
    data: Arc<m2_formats::effect::FlyingData>,
    target: u32,
    target_at: Vec3,
    position: Vec3,
    rotation: Quat,
    attach_rotation: Quat,
    velocity: Vec3,
    accel: Vec3,
    remain_range: f32,
    elapsed: f32,
    attach: Vec<(usize, m2_effect::EffectInstance)>,
}

impl Fly {
    /// `__SetDataPointer` + `__SetTargetDirection`.
    fn new(data: Arc<m2_formats::effect::FlyingData>, start: Vec3, target: u32, target_at: Vec3, seed: u32) -> Self {
        let mut rotation = yaw_pitch_roll((data.roll_angle - 90.0).to_radians(), 0.0, data.cone_angle.to_radians());
        if data.spreading {
            let mut rng = m2_effect::rng::GlibcRandom::new(seed);
            let third = std::f32::consts::PI / 3.0;
            let tilt = (rng.frandom(-third, third) + rng.frandom(-third, third)) / 2.0;
            let q2 = Quat::from_axis_angle(Vec3::Z, tilt);
            let q1 = Quat::from_axis_angle(Vec3::NEG_Y, rng.frandom(0.0, std::f32::consts::TAU));
            rotation *= q1 * q2;
        }
        let local_velocity = Vec3::new(0.0, -data.initial_velocity, 0.0);
        let mut target_at = target_at;
        if data.maintain_parallel {
            target_at.z += 50.0;
        }
        let direction = (target_at - start).normalize_or(Vec3::NEG_Y);
        rotation = Quat::from_rotation_arc(Vec3::NEG_Y, direction) * rotation;
        let accel = rotation * Vec3::from_array(data.acceleration);
        Fly {
            target,
            target_at,
            position: start,
            velocity: rotation * local_velocity,
            accel,
            rotation,
            attach_rotation: Quat::IDENTITY,
            remain_range: data.range,
            elapsed: 0.0,
            attach: Vec::new(),
            data,
        }
    }

    /// `AdjustDirectionForHoming`: turn the velocity towards the target, by at
    /// most `m_fHomingMaxAngle` per step.
    fn home(&mut self) {
        let towards = (self.target_at - self.position).normalize_or_zero();
        let heading = self.velocity.normalize_or_zero();
        if towards == Vec3::ZERO || heading == Vec3::ZERO || (heading - towards).length_squared() < 0.001 {
            return;
        }
        let mut q = Quat::from_rotation_arc(heading, towards);
        if self.data.homing_max_angle > 180.0 {
            self.velocity = q * self.velocity;
            self.accel = q * self.accel;
            self.rotation *= q;
            return;
        }
        let (s, c) = self.data.homing_max_angle.to_radians().sin_cos();
        if q.w <= -1.0 + 0.0001 {
            q = Quat::from_xyzw(0.0, 0.0, s, c);
        } else if q.w <= c && q.w <= 1.0 - 0.0001 {
            let factor = s / (1.0 - q.w * q.w).sqrt();
            q = Quat::from_xyzw(q.x * factor, q.y * factor, q.z * factor, c);
        }
        self.velocity = q * self.velocity;
        self.accel = q * self.accel;
        self.rotation = q * self.rotation;
    }

    /// `CFlyingInstance::Update` for an object target.
    fn update(&mut self, dt: f32) -> FlyState {
        self.elapsed += dt;
        if self.data.homing && self.elapsed > self.data.homing_start_time {
            self.home();
        }
        let last = self.position;
        self.velocity += self.accel * dt;
        self.velocity.z += self.data.gravity * dt;
        let movement = self.velocity * dt;
        self.remain_range -= movement.length();
        self.position += movement;
        self.update_attach(dt);
        if self.remain_range < 0.0 {
            return FlyState::OutOfRange;
        }
        if segment_point_distance_squared(last, self.position, self.target_at) < self.data.bomb_range * self.data.bomb_range {
            return FlyState::Hit;
        }
        FlyState::Flying
    }

    /// `UpdateAttachInstance`: spin the attachments by `m_v3AngVel` and place
    /// each on its `FLY_ATTACH_TYPE_*` path.
    fn update_attach(&mut self, dt: f32) {
        let angular = self.data.angular_velocity;
        let spin = yaw_pitch_roll(angular[1].to_radians() * dt, angular[0].to_radians() * dt, angular[2].to_radians() * dt);
        self.attach_rotation = spin * self.attach_rotation;
        for i in 0..self.attach.len() {
            let index = self.attach[i].0;
            let matrix = m2_effect::Mat4::from_array(self.attach_matrix(index, self.elapsed));
            self.attach[i].1.set_global_matrix(matrix);
        }
    }

    /// The world matrix (row-major, D3D) of attachment `index` at `elapsed`.
    fn attach_matrix(&self, index: usize, elapsed: f32) -> [f32; 16] {
        let Some(attach) = self.data.attach.get(index) else { return Mat4::IDENTITY.to_cols_array() };
        let q = self.rotation * self.attach_rotation;
        let (sin_roll, cos_roll) = attach.roll.to_radians().sin_cos();
        let offset = |length: f32| q * Vec3::new(-sin_roll * length, 0.0, -cos_roll * length);
        let (rotation, position) = match attach.fly_type {
            // `FLY_ATTACH_TYPE_LINE`
            1 => (self.rotation, self.position),
            // `FLY_ATTACH_TYPE_MULTI_LINE`
            2 => (q, self.position + offset(attach.distance)),
            // `FLY_ATTACH_TYPE_SINE`
            3 => {
                let angle = elapsed * std::f32::consts::TAU / attach.period;
                (q, self.position + offset(attach.amplitude * angle.sin()))
            }
            // `FLY_ATTACH_TYPE_EXP`
            4 => {
                let t = elapsed / attach.period;
                (q, self.position + offset(attach.amplitude * (-t).exp() * t))
            }
            _ => (self.rotation, self.position),
        };
        // A D3D row-vector matrix is the transpose of glam's, so its rows are
        // glam's columns.
        Mat4::from_rotation_translation(rotation, position).to_cols_array()
    }
}

/// `D3DXQuaternionRotationYawPitchRoll`: roll (Z), then pitch (X), then yaw (Y).
fn yaw_pitch_roll(yaw: f32, pitch: f32, roll: f32) -> Quat {
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll)
}

/// `square_distance_between_linesegment_and_point`.
fn segment_point_distance_squared(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let ab = b - a;
    let length = ab.length_squared();
    let t = if length > 0.0 { ((p - a).dot(ab) / length).clamp(0.0, 1.0) } else { 0.0 };
    (a + ab * t - p).length_squared()
}

/// `effect.RegisterIndexedFlyData(n, INDEX_FLY_TYPE_NORMAL, file)`
/// (`playersettingmodule.py`): the `.msf` a `GC_CREATE_FLY` of type `n` sends
/// from one actor to another. The firework types fly to a point, not an actor,
/// and are not handled.
pub fn indexed_fly(kind: u8) -> Option<&'static str> {
    Some(match kind {
        1 => "d:/ymir work/effect/etc/gathering/ga_piece_yellow_small2.msf",
        2 => "d:/ymir work/effect/etc/gathering/ga_piece_red_small.msf",
        3 => "d:/ymir work/effect/etc/gathering/ga_piece_red_big.msf",
        4 => "d:/ymir work/effect/etc/gathering/ga_piece_blue_warrior_small.msf",
        5 => "d:/ymir work/effect/etc/gathering/ga_piece_blue_small.msf",
        6 => "d:/ymir work/effect/etc/gathering/ga_piece_blue_big.msf",
        14 => "d:/ymir work/pc/shaman/effect/pokroe.msf",
        15 => "d:/ymir work/effect/etc/gathering/ga_piece_red_smallest.msf",
        _ => return None,
    })
}

/// `CActorInstance::OnGetFlyTargetPosition`: roughly the centre of the body.
pub const FLY_TARGET_HEIGHT: f32 = 100.0;

/// Keeps one looping effect per wanted key at its position, loading new ones
/// and dropping those no longer wanted.
fn sync_effects(
    list: &mut Vec<((u32, u32), m2_effect::EffectInstance)>,
    failed: &mut HashSet<String>,
    packs: Option<&M2Packs>,
    wanted: &[((u32, u32), &'static str, Vec3)],
) {
    list.retain(|(key, instance)| instance.is_alive() && wanted.iter().any(|(k, ..)| k == key));
    for (key, file, position) in wanted {
        let mut matrix = m2_effect::Mat4::rotation_yaw_pitch_roll(0.0, 0.0, 0.0);
        matrix.set_translation(position.to_array());
        if let Some((_, instance)) = list.iter_mut().find(|(k, _)| k == key) {
            instance.set_global_matrix(matrix);
            continue;
        }
        let (Some(packs), false) = (packs, failed.contains(*file)) else { continue };
        let flat = std::array::from_fn(|i| matrix.m[i / 4][i % 4]);
        match load_instance(&packs.packs, file.as_bytes(), flat) {
            Ok(instance) => list.push((*key, instance)),
            Err(error) => {
                warn!("attached effect {file}: {error}");
                failed.insert((*file).to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_enemy_arrow_loads_its_visible_attachment_and_flies() {
        let dir = std::env::var_os("M2_CLIENT_DIR").map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"));
        if !dir.join("pack/Index.dev").exists() {
            eprintln!("SKIP: client data not found (set M2_CLIENT_DIR)");
            return;
        }
        let mut manager = m2_formats::pack::PackManager::new(&dir);
        m2_formats::pack::load_index(&mut manager, "pack", "pack/Index.dev").unwrap();
        let packs = Arc::new(manager);
        let packs = M2Packs { reader: m2_assets::source::PackAssetReader::new(packs.clone()), packs };
        let mut fx = CombatFx::default();
        fx.fly(Some(&packs), "d:/ymir work/pc/assassin/effect/arrow_01_fast.msf", Vec3::ZERO, 7, Vec3::new(0.0, -1000.0, 100.0));
        assert_eq!(fx.flies.len(), 1);
        assert!(!fx.flies[0].attach.is_empty(), "arrow must have a visible effect attachment");
        let start = fx.flies[0].position;
        fx.update_flies(Some(&packs), 0.05, |_| None);
        assert_eq!(fx.flies.len(), 1);
        assert!(fx.flies[0].position.distance(start) > 0.0);
    }

    #[test]
    fn gm_affect_uses_the_original_locale_effect() {
        let dir = std::env::var_os("M2_CLIENT_DIR").map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bin"));
        if !dir.join("pack/Index.dev").exists() {
            eprintln!("SKIP: client data not found (set M2_CLIENT_DIR)");
            return;
        }
        let mut manager = m2_formats::pack::PackManager::new(&dir);
        m2_formats::pack::load_index(&mut manager, "pack", "pack/Index.dev").unwrap();
        let packs = Arc::new(manager);
        let packs = M2Packs { reader: m2_assets::source::PackAssetReader::new(packs.clone()), packs };
        let (bone, file) = affect_effect(0).unwrap();
        assert_eq!(bone, "Bip01");
        let mut fx = CombatFx::default();
        fx.sync_attached(Some(&packs), &[((7, 0), file, Vec3::ZERO)]);
        assert_eq!(fx.attached.len(), 1);
        fx.sync_attached(Some(&packs), &[]);
        assert!(fx.attached.is_empty());
    }

    #[test]
    fn refined_equipment_effects_follow_the_client_threshold_and_weapon_type() {
        let mut item = m2_formats::proto::ItemData::default();
        item.table.item_type = 1;
        item.table.gain_socket_pct = 3;
        assert!(refined_effects(16, &item).is_empty());
        assert_eq!(refined_effects(19, &item)[0].2, "d:/ymir work/pc/common/effect/sword/sword_9.mse");
        item.table.sub_type = 1;
        let effects = refined_effects(19, &item);
        assert_eq!(effects.len(), 2);
        assert_eq!(effects[0].1, "right");
        assert_eq!(effects[1].1, "left");
        item.table.item_type = 2;
        item.table.sub_type = 0;
        let effects = refined_effects(12019, &item);
        assert_eq!(effects.len(), 3);
        assert_eq!(effects[2].2, "d:/ymir work/pc/common/effect/armor/armor_9.mse");
    }
}
