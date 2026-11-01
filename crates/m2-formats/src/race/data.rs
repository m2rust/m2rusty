//! `NRaceData` (`client/src/game/GameType.cpp`): the attack, hit, collision
//! and attaching records shared by race and motion scripts.

use super::{Cursor, vector3_of};
use crate::script::atof_f32;

/// `NRaceData::ECollisionType`.
pub const COLLISION_TYPE_NONE: i32 = 0;
/// `COLLISION_TYPE_BODY`.
pub const COLLISION_TYPE_BODY: i32 = 1;
/// `COLLISION_TYPE_ATTACKING`.
pub const COLLISION_TYPE_ATTACKING: i32 = 2;
/// `COLLISION_TYPE_DEFENDING`.
pub const COLLISION_TYPE_DEFENDING: i32 = 3;
/// `COLLISION_TYPE_SPLASH`.
pub const COLLISION_TYPE_SPLASH: i32 = 4;

/// `NRaceData::EAttachingDataType`.
pub const ATTACHING_DATA_TYPE_NONE: u32 = 0;
/// `ATTACHING_DATA_TYPE_COLLISION_DATA`.
pub const ATTACHING_DATA_TYPE_COLLISION_DATA: u32 = 1;
/// `ATTACHING_DATA_TYPE_EFFECT`.
pub const ATTACHING_DATA_TYPE_EFFECT: u32 = 2;
/// `ATTACHING_DATA_TYPE_OBJECT`.
pub const ATTACHING_DATA_TYPE_OBJECT: u32 = 3;

/// `NRaceData::SAttackData`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AttackData {
    /// `iAttackType` (`EAttackType`: 0 splash, 1 snipe).
    pub attack_type: i32,
    /// `iHittingType` (`EHitType`).
    pub hitting_type: i32,
    /// `fInvisibleTime`.
    pub invisible_time: f32,
    /// `fExternalForce`.
    pub external_force: f32,
    /// `fStiffenTime`.
    pub stiffen_time: f32,
    /// `iHitLimitCount`.
    pub hit_limit_count: i32,
}

impl AttackData {
    /// `NRaceData::LoadAttackData`. Fields read before a failure keep their
    /// new values.
    pub(crate) fn load(&mut self, cur: &Cursor) -> bool {
        if !cur.int("attacktype", &mut self.attack_type) {
            self.attack_type = 0; // ATTACK_TYPE_SPLASH
        }
        if !cur.int("hittingtype", &mut self.hitting_type)
            || !cur.float("stiffentime", &mut self.stiffen_time)
            || !cur.float("invisibletime", &mut self.invisible_time)
            || !cur.float("externalforce", &mut self.external_force)
        {
            return false;
        }
        if !cur.int("hitlimitcount", &mut self.hit_limit_count) {
            self.hit_limit_count = 0;
        }
        true
    }
}

/// One `HitPosition` entry (`THitTimePosition`, a `CDynamicSphereInstance`
/// of which only the two positions are set).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct HitPosition {
    /// `v3LastPosition`.
    pub last_position: [f32; 3],
    /// `v3Position`.
    pub position: [f32; 3],
}

/// `NRaceData::SHitData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HitData {
    /// `fAttackStartTime`.
    pub attack_start_time: f32,
    /// `fAttackEndTime`.
    pub attack_end_time: f32,
    /// `fWeaponLength`.
    pub weapon_length: f32,
    /// `strBoneName`.
    pub bone_name: Vec<u8>,
    /// `mapHitPosition` (`std::map<float, …>`): ascending times, one entry
    /// per time, the last one in the file winning.
    pub hit_positions: Vec<(f32, HitPosition)>,
}

impl HitData {
    /// `SHitData::Load`.
    ///
    /// Deviations: `HitPosition` is read seven numbers at a time and C++
    /// runs past the end of the list when its length is not a multiple of
    /// seven (undefined); here a trailing incomplete group is ignored. And
    /// `std::map` with NaN keys has no defined order; here a NaN time is
    /// never equal to an existing one and goes last.
    pub(crate) fn load(&mut self, cur: &Cursor) -> bool {
        if !cur.float("attackingstarttime", &mut self.attack_start_time)
            || !cur.float("attackingendtime", &mut self.attack_end_time)
        {
            return false;
        }
        if !cur.string("attackingbone", &mut self.bone_name) {
            self.bone_name.clear();
        }
        if !cur.float("weaponlength", &mut self.weapon_length) {
            self.weapon_length = 0.0;
        }

        self.hit_positions.clear();
        if let Some(tokens) = cur.tokens("hitposition") {
            for chunk in tokens.as_chunks::<7>().0 {
                let time = atof_f32(&chunk[0]);
                let hit = HitPosition {
                    last_position: vector3_of(&chunk[1..4]),
                    position: vector3_of(&chunk[4..7]),
                };
                insert_by_time(&mut self.hit_positions, time, hit);
            }
        }
        true
    }
}

/// `map[time] = hit` on a `std::map<float, …>`: an equal key (`-0.0` equals
/// `0.0`) keeps its key and takes the new value.
fn insert_by_time(map: &mut Vec<(f32, HitPosition)>, time: f32, hit: HitPosition) {
    if let Some(entry) = map.iter_mut().find(|(k, _)| *k == time) {
        entry.1 = hit;
        return;
    }
    let at = map.iter().position(|(k, _)| *k > time).unwrap_or(map.len());
    map.insert(at, (time, hit));
}

/// `NRaceData::SMotionAttackData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionAttackData {
    /// The `SAttackData` base.
    pub attack: AttackData,
    /// `iMotionType` (`EMotionType`).
    pub motion_type: i32,
    /// `HitDataContainer`.
    pub hit_data: Vec<HitData>,
}

impl MotionAttackData {
    /// `NRaceData::LoadMotionAttackData`. With `HitDataCount` the hit data
    /// are the first that many child groups, whatever their names; without
    /// it one hit is read from the group itself.
    pub(crate) fn load(&mut self, cur: &mut Cursor) -> bool {
        if !self.attack.load(cur) {
            return false;
        }
        // `MotionType` replaced `AttackingType`; old files still use it.
        if !cur.int("motiontype", &mut self.motion_type) && !cur.int("attackingtype", &mut self.motion_type) {
            return false;
        }

        self.hit_data.clear();
        let mut count = 0u32;
        if !cur.dword("hitdatacount", &mut count) {
            self.hit_data.push(HitData::default());
            return self.hit_data.last_mut().unwrap().load(cur);
        }
        for i in 0..count {
            if !cur.set_child_at(i as usize) {
                return false;
            }
            self.hit_data.push(HitData::default());
            if !self.hit_data.last_mut().unwrap().load(cur) {
                return false;
            }
            cur.set_parent();
        }
        true
    }
}

/// `TSphereData` of a `CSphereCollisionInstance`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SphereData {
    /// `fRadius`.
    pub radius: f32,
    /// `v3Position`.
    pub position: [f32; 3],
}

/// `NRaceData::SCollisionData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollisionData {
    /// `iCollisionType` (`COLLISION_TYPE_*`).
    pub collision_type: i32,
    /// `SphereDataVector`.
    pub spheres: Vec<SphereData>,
}

impl CollisionData {
    /// `NRaceData::LoadCollisionData`. The sphere vector is sized from
    /// `SphereDataCount` first, so a failure leaves default spheres after the
    /// loaded ones, and the cursor in the failing `SphereDataNN` group.
    pub(crate) fn load(&mut self, cur: &mut Cursor) -> bool {
        if !cur.int("collisiontype", &mut self.collision_type) {
            return false;
        }
        let mut count = 0u32;
        if !cur.dword("spheredatacount", &mut count) {
            return false;
        }

        self.spheres.clear();
        self.spheres.resize(count as usize, SphereData::default());
        for i in 0..count {
            let sphere = &mut self.spheres[i as usize];
            if !cur.set_child_indexed(b"spheredata", i)
                || !cur.float("radius", &mut sphere.radius)
                || !cur.vector3("position", &mut sphere.position)
            {
                return false;
            }
            cur.set_parent();
        }
        true
    }
}

/// `NRaceData::SAttachingEffectData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttachingEffectData {
    /// `strFileName`.
    pub file_name: Vec<u8>,
    /// `v3Position`.
    pub position: [f32; 3],
    /// `v3Rotation`.
    pub rotation: [f32; 3],
}

impl AttachingEffectData {
    /// `NRaceData::LoadEffectData`. C++ also registers the effect, ignoring
    /// whether that works.
    fn load(&mut self, cur: &Cursor) -> bool {
        if !cur.string("effectscriptname", &mut self.file_name) {
            return false;
        }
        if !cur.vector3("effectposition", &mut self.position) {
            self.position = [0.0; 3];
        }
        if !cur.vector3("effectrotation", &mut self.rotation) {
            self.rotation = [0.0; 3];
        }
        true
    }
}

/// `NRaceData::SAttachingObjectData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttachingObjectData {
    /// `strFileName`.
    pub file_name: Vec<u8>,
}

/// What `SAttachingData` points to: C++ allocates the record matching
/// `dwType`, if any.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum AttachingPayload {
    /// No record (`dwType` is not 1–3, or loading stopped before it).
    #[default]
    None,
    /// `pCollisionData`.
    Collision(CollisionData),
    /// `pEffectData`.
    Effect(AttachingEffectData),
    /// `pObjectData`.
    Object(AttachingObjectData),
}

/// `NRaceData::SAttachingData`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttachingData {
    /// `dwType` (`ATTACHING_DATA_TYPE_*`).
    pub data_type: u32,
    /// `isAttaching`: a `BOOL` holding `atoi` of the token.
    pub is_attaching: i32,
    /// `dwAttachingModelIndex`.
    pub attaching_model_index: u32,
    /// `strAttachingBoneName`.
    pub attaching_bone_name: Vec<u8>,
    /// The record for `data_type`.
    pub payload: AttachingPayload,
}

/// `NRaceData::LoadAttachingData`: `AttachingDataCount` entries from the
/// `AttachingDataNN` child groups. The vector is replaced and sized first,
/// so on failure it holds the entries loaded so far (the last one possibly
/// partial) followed by defaults, and the cursor may be left in a child.
pub(crate) fn load_attaching_data(cur: &mut Cursor, out: &mut Vec<AttachingData>) -> bool {
    let mut count = 0u32;
    if !cur.dword("attachingdatacount", &mut count) {
        return false;
    }

    out.clear();
    out.resize(count as usize, AttachingData::default());
    for i in 0..count {
        let data = &mut out[i as usize];
        if !cur.set_child_indexed(b"attachingdata", i)
            || !cur.dword("attachingdatatype", &mut data.data_type)
            || !cur.int("isattaching", &mut data.is_attaching)
            || !cur.dword("attachingmodelindex", &mut data.attaching_model_index)
            || !cur.string("attachingbonename", &mut data.attaching_bone_name)
        {
            return false;
        }

        let ok = match data.data_type {
            ATTACHING_DATA_TYPE_COLLISION_DATA => {
                let mut collision = CollisionData::default();
                let ok = collision.load(cur);
                data.payload = AttachingPayload::Collision(collision);
                ok
            }
            ATTACHING_DATA_TYPE_EFFECT => {
                let mut effect = AttachingEffectData::default();
                let ok = effect.load(cur);
                data.payload = AttachingPayload::Effect(effect);
                ok
            }
            ATTACHING_DATA_TYPE_OBJECT => {
                // `LoadObjectData`.
                let mut object = AttachingObjectData::default();
                let ok = cur.string("objectscriptname", &mut object.file_name);
                data.payload = AttachingPayload::Object(object);
                ok
            }
            _ => true,
        };
        if !ok {
            return false;
        }
        cur.set_parent();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::parse;

    #[test]
    fn hit_positions_are_a_float_map() {
        let script = parse(
            b"AttackingStartTime 0.1\nAttackingEndTime 0.2\nList HitPosition\n{\n\
              0.5 1 2 3 4 5 6\n0.25 0 0 0 0 0 0\n0.5 9 9 9 9 9 9\n-0 1 1 1 1 1 1\n0 2 2 2 2 2 2\n7 7\n}\n",
        );
        let mut hit = HitData::default();
        assert!(hit.load(&Cursor::new(&script.root)));
        let times: Vec<u32> = hit.hit_positions.iter().map(|(t, _)| t.to_bits()).collect();
        assert_eq!(times, [(-0.0f32).to_bits(), 0.25f32.to_bits(), 0.5f32.to_bits()]);
        assert_eq!(hit.hit_positions[0].1.position, [2.0; 3]);
        assert_eq!(hit.hit_positions[2].1.last_position, [9.0; 3]);
        assert_eq!(hit.weapon_length, 0.0);
        assert!(hit.bone_name.is_empty());
    }

    #[test]
    fn attaching_data_failure_keeps_partial_vector() {
        let script = parse(
            b"AttachingDataCount 3\nGroup AttachingData00\n{\nAttachingDataType 1\nisAttaching 2\n\
              AttachingModelIndex 0\nAttachingBoneName \"b\"\nCollisionType 1\nSphereDataCount 2\n\
              Group SphereData00\n{\nRadius 5\nPosition 1 2 3\n}\n}\n",
        );
        let mut cur = Cursor::new(&script.root);
        let mut out = Vec::new();
        assert!(!load_attaching_data(&mut cur, &mut out));
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].is_attaching, 2);
        let AttachingPayload::Collision(c) = &out[0].payload else { panic!() };
        assert_eq!(c.spheres, [SphereData { radius: 5.0, position: [1.0, 2.0, 3.0] }, SphereData::default()]);
        assert_eq!(out[1], AttachingData::default());
        // Left in AttachingData00 (SphereData01 was not found).
        assert_eq!(cur.current_name(), Some(&b"attachingdata00"[..]));
    }

    #[test]
    fn hit_data_count_uses_child_order() {
        let script = parse(
            b"HittingType 1\nStiffenTime 0\nInvisibleTime 0\nExternalForce 0\nAttackingType 2\nHitDataCount 2\n\
              Group Zzz\n{\nAttackingStartTime 1\nAttackingEndTime 2\n}\nGroup Aaa\n{\nAttackingStartTime 3\nAttackingEndTime 4\n}\n",
        );
        let mut data = MotionAttackData::default();
        assert!(data.load(&mut Cursor::new(&script.root)));
        assert_eq!(data.motion_type, 2);
        assert_eq!(data.hit_data.len(), 2);
        assert_eq!(data.hit_data[1].attack_start_time, 3.0);
    }
}
