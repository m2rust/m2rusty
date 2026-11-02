//! Combat rules ported from the C++ client: what may be attacked, how the
//! client picks a target, mounted attacks, bow shots, skill use and the
//! packets that go with them.
//!
//! The Python game window was only a thin dispatcher here. Its decisions came
//! from the engine:
//!
//! - `CInstanceBase::IsAttackableInstance` (PK mode, duel, guild, party,
//!   empire, alignment and actor type) decides whether a swing may hit.
//! - `CPythonPlayer::NEW_Attack` / `SetTarget` / `__UseSkill` decide *which*
//!   target, whether a shot is allowed (`__CanShot`) and which skill checks
//!   run (`__CheckSkillUsable`).
//! - `CInstanceBase::SHORSE` decides mounted movement, attack and skill
//!   restrictions from the mounted vnum.
//! - `CPythonPlayerEventHandler` translates the motion's hit/fly events into
//!   `CG_ATTACK`, `CG_SHOOT`, `CG_FLY_TARGETING` and `CG_ADD_FLY_TARGETING`.
//!
//! This module keeps those decisions pure so the scene and the UI can share
//! them and so they can be tested without a server or a GPU.

use std::collections::{BTreeMap, HashSet};

use bevy::prelude::*;
use m2_formats::proto::{SkillData, SkillTables};

/// `EPointTypes` (`client/include/Client/UI/Packet.h`).
pub mod point {
    pub const LEVEL: usize = 1;
    pub const HP: usize = 5;
    pub const MAX_HP: usize = 6;
    pub const SP: usize = 7;
    pub const MAX_SP: usize = 8;
    pub const STAMINA: usize = 9;
    pub const ATT_SPEED: usize = 17;
    pub const MOV_SPEED: usize = 19;
    pub const CASTING_SPEED: usize = 21;
    pub const MIN_ATK: usize = 29;
    pub const MAX_ATK: usize = 30;
    pub const BOW_DISTANCE: usize = 34;
    pub const MIN_WEP: usize = 200;
    pub const MAX_WEP: usize = 201;
    pub const MIN_MAGIC_WEP: usize = 202;
    pub const MAX_MAGIC_WEP: usize = 203;
    pub const HIT_RATE: usize = 204;
}

/// `CItemData::WEAPON_*` (`client/include/Client/Game/ItemData.h`).
pub mod weapon {
    pub const SWORD: u8 = 0;
    pub const DAGGER: u8 = 1;
    pub const BOW: u8 = 2;
    pub const TWO_HANDED: u8 = 3;
    pub const BELL: u8 = 4;
    pub const FAN: u8 = 5;
    pub const ARROW: u8 = 6;
    pub const NONE: u8 = 8;
}

/// `CInstanceBase::EType` (`ActorInstance.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActorType {
    Enemy,
    Npc,
    Stone,
    Warp,
    Door,
    Building,
    Pc,
    Poly,
    Horse,
    Goto,
    Object,
}

impl ActorType {
    pub fn from_raw(kind: u8) -> Self {
        match kind {
            0 => Self::Enemy,
            1 => Self::Npc,
            2 => Self::Stone,
            3 => Self::Warp,
            4 => Self::Door,
            5 => Self::Building,
            6 => Self::Pc,
            7 => Self::Poly,
            8 => Self::Horse,
            9 => Self::Goto,
            _ => Self::Object,
        }
    }
}

/// `ADD_CHARACTER_STATE_*` bits of `bStateFlag` (`Packet.h`).
pub const STATE_DEAD: u8 = 1 << 0;
pub const STATE_SPAWN: u8 = 1 << 1;
pub const STATE_KILLER: u8 = 1 << 3;
pub const STATE_PARTY: u8 = 1 << 4;

/// `CInstanceBase::AFFECT_*` ids that change targeting (`InstanceBase.h`).
pub const AFFECT_INVISIBILITY: u32 = 1;
pub const AFFECT_EUNHYEONG: u32 = 18;
pub const AFFECT_REVIVE_INVISIBILITY: u32 = 27;
pub const AFFECT_DASH: u32 = 31;
pub const AFFECT_GEOMGYEONG: u32 = 15;

/// `__Warrior_SetGeomgyeongAffect` extends the weapon sweep by half.
pub fn attack_reach(flags: u64) -> f32 {
    if has_affect(flags, AFFECT_GEOMGYEONG) { 1.5 } else { 1.0 }
}
pub const AFFECT_STUN: u32 = 5;
pub const AFFECT_CHEONGEUN: u32 = 16;
pub const AFFECT_FALLEN_CHEONGEUN: u32 = 33;

/// Whether affect `id` is set in the two-word flag container. This is
/// `CAffectFlagContainer::IsSet`: the id picks a bit in its 32-bit word.
pub fn has_affect(flags: u64, id: u32) -> bool {
    let bit = 1u64 << (id % 64);
    flags & bit != 0
}

/// `ALIGNMENT_TYPE_*` (`InstanceBase.h`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignmentType {
    White,
    Normal,
    Dark,
}

/// What the attacker and the target are, as far as `IsAttackableInstance`
/// needs. Built from the tracked actors and the main character's own state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ActorCombat {
    pub vid: u32,
    pub kind: u8,
    pub race: u16,
    pub empire: u8,
    pub pk_mode: u8,
    pub guild_id: u32,
    pub alignment: i16,
    pub dead: bool,
    pub state_flags: u8,
    pub affect_flags: u64,
    /// `CInstanceBase::IsInSafe` at the actor's feet (`ATTRIBUTE_BANPK`).
    pub in_safe: bool,
    /// Map-global position, as the server reports it.
    pub x: i32,
    pub y: i32,
}

impl ActorCombat {
    pub fn actor_type(&self) -> ActorType {
        ActorType::from_raw(self.kind)
    }
    pub fn is_pc(&self) -> bool {
        self.actor_type() == ActorType::Pc
    }
    pub fn is_npc(&self) -> bool {
        self.actor_type() == ActorType::Npc
    }
    pub fn is_enemy(&self) -> bool {
        self.actor_type() == ActorType::Enemy
    }
    pub fn is_stone(&self) -> bool {
        self.actor_type() == ActorType::Stone
    }
    pub fn is_door(&self) -> bool {
        self.actor_type() == ActorType::Door
    }
    pub fn is_building(&self) -> bool {
        self.actor_type() == ActorType::Building
    }
    pub fn is_poly(&self) -> bool {
        self.actor_type() == ActorType::Poly
    }
    pub fn is_dead(&self) -> bool {
        self.dead || self.state_flags & STATE_DEAD != 0
    }
    pub fn is_killer(&self) -> bool {
        self.state_flags & STATE_KILLER != 0
    }
    pub fn is_party_member(&self) -> bool {
        self.state_flags & STATE_PARTY != 0
    }
    /// `CInstanceBase::GetAlignmentGrade`'s thresholds.
    pub fn alignment_grade(&self) -> u8 {
        match self.alignment {
            12000.. => 0,
            8000.. => 1,
            4000.. => 2,
            1000.. => 3,
            0.. => 4,
            -3999.. => 5,
            -7999.. => 6,
            -11999.. => 7,
            _ => 8,
        }
    }
    /// `CInstanceBase::GetAlignmentType`.
    pub fn alignment_type(&self) -> AlignmentType {
        match self.alignment_grade() {
            0..=3 => AlignmentType::White,
            5..=8 => AlignmentType::Dark,
            _ => AlignmentType::Normal,
        }
    }
    /// `CInstanceBase::IsWoodenDoor`: a door whose vnum is the wooden-door
    /// range. The tracked actor does not carry the vnum, so a door counts as
    /// wooden when its race is one of the shipped wooden-door races.
    pub fn is_wooden_door(&self) -> bool {
        self.is_door() && matches!(self.race, 13000 | 30111..=30119)
    }
    /// `CInstanceBase::CanPickInstance`: hidden players and the dead cannot be
    /// picked. The view-frustum part belongs to the renderer.
    pub fn can_pick(&self) -> bool {
        if self.is_dead() {
            return false;
        }
        if self.is_pc() {
            if has_affect(self.affect_flags, AFFECT_EUNHYEONG) {
                return false;
            }
            if has_affect(self.affect_flags, AFFECT_REVIVE_INVISIBILITY) {
                return false;
            }
            if has_affect(self.affect_flags, AFFECT_INVISIBILITY) {
                return false;
            }
        }
        true
    }
}

/// `PK_MODE_*` (`InstanceBase.h`).
pub const PK_MODE_PEACE: u8 = 0;
pub const PK_MODE_REVENGE: u8 = 1;
pub const PK_MODE_FREE: u8 = 2;
pub const PK_MODE_PROTECT: u8 = 3;
pub const PK_MODE_GUILD: u8 = 4;

/// `DUEL_*` (`InstanceBase.h`).
pub const DUEL_NONE: u8 = 0;
pub const DUEL_CANNOTATTACK: u8 = 1;
pub const DUEL_START: u8 = 2;

/// State the attacker owns outside its actor: party, duel and war lists.
#[derive(Clone, Debug, Default)]
pub struct CombatWorld {
    pub observer: bool,
    pub duel_mode: u8,
    pub party: HashSet<u32>,
    /// `InsertDUELKey` pairs, flattened as `(source, destination)`.
    pub duel_keys: HashSet<(u32, u32)>,
    /// `__FindPVPKey` pairs.
    pub pvp_keys: HashSet<(u32, u32)>,
    /// `__FindGVGKey` pairs (guild ids).
    pub gvg_keys: HashSet<(u32, u32)>,
}

impl CombatWorld {
    pub fn same_party(&self, a: u32, b: u32) -> bool {
        (a == b) || (self.party.contains(&a) && self.party.contains(&b))
    }
    /// `CInstanceBase::__FindDUELKey`.
    pub fn duel_pair(&self, a: u32, b: u32) -> bool {
        self.duel_keys.contains(&(a, b)) || self.duel_keys.contains(&(b, a))
    }
    /// `CInstanceBase::__FindPVPKey`.
    pub fn pvp_pair(&self, a: u32, b: u32) -> bool {
        self.pvp_keys.contains(&(a, b)) || self.pvp_keys.contains(&(b, a))
    }
    /// `CInstanceBase::__FindGVGKey`.
    pub fn gvg_pair(&self, a: u32, b: u32) -> bool {
        a != 0 && b != 0 && (self.gvg_keys.contains(&(a, b)) || self.gvg_keys.contains(&(b, a)))
    }
}

/// `CInstanceBase::IsSameEmpire`. Empire 0 and game masters count as the same
/// empire; the client has no game-master flag, so only the zero case remains.
pub fn is_same_empire(a: &ActorCombat, b: &ActorCombat) -> bool {
    b.empire == 0 || a.empire == b.empire
}

/// `CInstanceBase::IsPVPInstance`.
pub fn is_pvp_instance(world: &CombatWorld, me: &ActorCombat, victim: &ActorCombat) -> bool {
    if world.duel_mode != DUEL_NONE {
        return true;
    }
    world.pvp_pair(me.vid, victim.vid) || world.gvg_pair(me.guild_id, victim.guild_id)
}

/// `CInstanceBase::IsConflictAlignmentInstance`.
pub fn is_conflict_alignment(me: &ActorCombat, victim: &ActorCombat) -> bool {
    if victim.pk_mode == PK_MODE_PROTECT {
        return false;
    }
    match me.alignment_type() {
        AlignmentType::Normal | AlignmentType::White => victim.alignment_type() == AlignmentType::Dark,
        AlignmentType::Dark => me.alignment_type() != victim.alignment_type(),
    }
}

/// `CInstanceBase::IsAttackableInstance` (`InstanceBase.cpp:2142`).
///
/// The client's party test is `IAbstractPlayer::IsSamePartyMember`; the port
/// keeps the party vids in [`CombatWorld::party`]. `IsInSafe` is not part of
/// this predicate in C++ either: `CheckAttacking` rejects safe areas, and
/// [`safe_attack_error`] exposes that as a separate reason.
pub fn is_attackable(world: &CombatWorld, me: &ActorCombat, victim: &ActorCombat) -> bool {
    if world.observer {
        return false;
    }
    if me.vid == victim.vid {
        return false;
    }

    if me.is_stone() {
        if victim.is_pc() {
            return true;
        }
    } else if me.is_pc() {
        if victim.is_stone() {
            return true;
        }
        if victim.is_pc() {
            if world.duel_mode != DUEL_NONE {
                match world.duel_mode {
                    DUEL_CANNOTATTACK => return false,
                    DUEL_START => return world.duel_pair(me.vid, victim.vid),
                    _ => {}
                }
            }
            if me.pk_mode == PK_MODE_GUILD && me.guild_id == victim.guild_id {
                return false;
            }
            if victim.is_killer() && !world.same_party(me.vid, victim.vid) {
                return true;
            }
            if me.pk_mode != PK_MODE_PROTECT {
                if me.pk_mode == PK_MODE_FREE
                    && victim.pk_mode != PK_MODE_PROTECT
                    && !world.same_party(me.vid, victim.vid)
                {
                    return true;
                }
                if me.pk_mode == PK_MODE_GUILD
                    && victim.pk_mode != PK_MODE_PROTECT
                    && !world.same_party(me.vid, victim.vid)
                    && me.guild_id != victim.guild_id
                {
                    return true;
                }
            }
            if is_same_empire(me, victim) {
                if is_pvp_instance(world, me, victim) {
                    return true;
                }
                if me.pk_mode == PK_MODE_REVENGE
                    && !world.same_party(me.vid, victim.vid)
                    && is_conflict_alignment(me, victim)
                {
                    return true;
                }
            } else {
                return true;
            }
        }
        if victim.is_enemy() {
            return true;
        }
        if victim.is_wooden_door() {
            return true;
        }
    } else if me.is_enemy() {
        if victim.is_pc() {
            return true;
        }
        if victim.is_building() {
            return true;
        }
    } else if me.is_poly() {
        if victim.is_pc() {
            return true;
        }
        if victim.is_enemy() {
            return true;
        }
    }
    false
}

/// A skill the player asked to use, from the skill page, the task bar or a
/// keyboard shortcut. `m2-game` turns it into the C++ `__UseSkill` flow and
/// sends `CG_USE_SKILL`; the UI never sends the packet itself.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UseSkillRequest {
    pub vnum: u32,
}

/// Why a normal attack or shot is refused (`OnCannotAttack`/`OnCannotShot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackError {
    Observer,
    InSafe,
    DestInSafe,
    EmptyArrow,
    NeedTarget,
    MountedHandAttack,
    MountLevel,
    NewMountSkill,
    Dead,
    Stunned,
    Frozen,
}

impl AttackError {
    /// The `OnCannotAttack`/`OnCannotShot` reason string of the C++ callbacks.
    pub fn reason(self) -> &'static str {
        match self {
            Self::Observer => "OBSERVER",
            Self::InSafe => "IN_SAFE",
            Self::DestInSafe => "DEST_IN_SAFE",
            Self::EmptyArrow => "EMPTY_ARROW",
            Self::NeedTarget => "NEED_TARGET",
            Self::MountedHandAttack => "HAND_MODE",
            Self::MountLevel => "HORSE_LEVEL",
            Self::NewMountSkill => "NEED_RIDING_SKILL",
            Self::Dead => "DEAD",
            Self::Stunned => "STUNNED",
            Self::Frozen => "FROZEN",
        }
    }
}

/// `CInstanceBase::SHORSE::GetLevel`: the client's table of mount vnums.
///
/// Level 1 cannot attack, level 2 can attack but not use skills and level 3
/// can use skills. Unknown vnums are level 0, which cannot attack.
pub fn mount_level(vnum: u32) -> u32 {
    match vnum {
        20101..=20103 => 1,
        20104..=20106 => 2,
        20107..=20118 | 20120..=20125 => 3,
        20119 | 20219..=20222 => 2,
        20205..=20208 | 20214 | 20217 => 2,
        // The C++ table repeats 20220 in the level-3 block after the level-2
        // block has already returned it; that arm is unreachable there too.
        20209..=20212 | 20215 | 20218 => 3,
        _ => 0,
    }
}

/// `CInstanceBase::SHORSE::IsNewMount`.
pub fn is_new_mount(vnum: u32) -> bool {
    matches!(vnum, 20205..=20212 | 20214 | 20215 | 20217 | 20218 | 20220)
}

/// `CInstanceBase::SHORSE::CanAttack`.
pub fn mount_can_attack(vnum: u32) -> bool {
    vnum == 0 || mount_level(vnum) > 1
}

/// `CInstanceBase::SHORSE::CanUseSkill`.
pub fn mount_can_use_skill(vnum: u32) -> bool {
    vnum == 0 || mount_level(vnum) > 2
}

/// Why the client refuses to start an attack or shot.
#[derive(Clone)]
pub struct AttackContext {
    pub world: CombatWorld,
    pub me: ActorCombat,
    pub mounted: u32,
    /// `CPythonPlayer::GetSkillGrade(109)` / `GetSkillLevel(109)`.
    pub riding_grade: u8,
    pub riding_level: u8,
    pub observer: bool,
    pub in_safe: bool,
    /// `IsHandMode()` (general or horse motion mode).
    pub hand_mode: bool,
    pub arrows: u32,
}

impl AttackContext {
    /// `CPythonPlayer::__CanAttack` (`PythonPlayerInput.cpp:752`).
    pub fn can_attack(&self) -> Result<(), AttackError> {
        if self.observer {
            return Err(AttackError::Observer);
        }
        if self.me.is_dead() {
            return Err(AttackError::Dead);
        }
        if has_affect(self.me.affect_flags, AFFECT_STUN) {
            return Err(AttackError::Stunned);
        }
        if self.mounted != 0
            && is_new_mount(self.mounted)
            && self.riding_grade < 1
            && self.riding_level < 11
        {
            return Err(AttackError::NewMountSkill);
        }
        if !mount_can_attack(self.mounted) {
            return Err(AttackError::MountLevel);
        }
        Ok(())
    }

    /// `CPythonPlayer::__CanShot` (`PythonPlayerInput.cpp:670`).
    pub fn can_shoot(&self, victim_safe: bool) -> Result<(), AttackError> {
        if self.arrows == 0 {
            return Err(AttackError::EmptyArrow);
        }
        if self.in_safe {
            return Err(AttackError::InSafe);
        }
        if victim_safe {
            return Err(AttackError::DestInSafe);
        }
        Ok(())
    }

    /// The `NEW_Attack` mounted hand-mode rule: "attacking bare-handed on a
    /// horse is not supported".
    pub fn mounted_hand_attack(&self) -> Result<(), AttackError> {
        if self.mounted != 0 && self.hand_mode {
            return Err(AttackError::MountedHandAttack);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Skills
// ---------------------------------------------------------------------------

/// `SKILL_ATTRIBUTE_*` (`PythonSkill.h`).
pub mod attribute {
    pub const NEED_TARGET: u32 = 1 << 0;
    pub const TOGGLE: u32 = 1 << 1;
    pub const WEAPON_LIMITATION: u32 = 1 << 2;
    pub const MELEE_ATTACK: u32 = 1 << 3;
    pub const USE_HP: u32 = 1 << 4;
    pub const CAN_CHANGE_DIRECTION: u32 = 1 << 5;
    pub const STANDING_SKILL: u32 = 1 << 6;
    pub const ONLY_FOR_ALLIANCE: u32 = 1 << 7;
    pub const CAN_USE_FOR_ME: u32 = 1 << 8;
    pub const NEED_CORPSE: u32 = 1 << 9;
    pub const FAN_RANGE: u32 = 1 << 10;
    pub const CAN_USE_IF_NOT_ENOUGH: u32 = 1 << 11;
    pub const NEED_EMPTY_BOTTLE: u32 = 1 << 12;
    pub const NEED_POISON_BOTTLE: u32 = 1 << 13;
    pub const ATTACK_SKILL: u32 = 1 << 14;
    pub const TIME_INCREASE_SKILL: u32 = 1 << 15;
    pub const CHARGE_ATTACK: u32 = 1 << 16;
    pub const PASSIVE: u32 = 1 << 17;
    pub const MOVING_SKILL: u32 = 1 << 20;
    pub const HORSE_SKILL: u32 = 1 << 21;
    pub const CIRCLE_RANGE: u32 = 1 << 22;
    pub const SEARCH_TARGET: u32 = 1 << 23;
}

/// `SKILL_NEED_WEAPON_*`.
pub mod need_weapon {
    pub const SWORD: u32 = 1 << 0;
    pub const DAGGER: u32 = 1 << 1;
    pub const BOW: u32 = 1 << 2;
    pub const TWO_HANDED: u32 = 1 << 3;
    pub const BELL: u32 = 1 << 4;
    pub const FAN: u32 = 1 << 5;
    pub const ARROW: u32 = 1 << 6;
    pub const EMPTY_HAND: u32 = 1 << 8;
}

/// `TSkillData` accessors that are inline in C++ (`PythonSkill.cpp`).
pub trait SkillExt {
    fn has(&self, flag: u32) -> bool;
    fn needs_target(&self) -> bool;
    fn is_toggle(&self) -> bool;
    fn can_change_direction(&self) -> bool;
    fn is_fan_range(&self) -> bool;
    fn is_circle_range(&self) -> bool;
    fn is_auto_search_target(&self) -> bool;
    fn needs_corpse(&self) -> bool;
    fn is_standing(&self) -> bool;
    fn only_for_alliance(&self) -> bool;
    fn can_use_for_me(&self) -> bool;
    fn can_use_if_not_enough(&self) -> bool;
    fn needs_empty_bottle(&self) -> bool;
    fn needs_poison_bottle(&self) -> bool;
    fn needs_bow(&self) -> bool;
    fn is_attack_skill(&self) -> bool;
    fn is_horse_skill(&self) -> bool;
    fn is_moving_skill(&self) -> bool;
    fn is_melee(&self) -> bool;
    fn is_charge(&self) -> bool;
    fn can_use_weapon(&self, weapon_type: u8) -> bool;
    /// `GetTargetRange`: melee and charge skills use the fixed 170 range.
    fn target_range(&self) -> u32;
}

impl SkillExt for SkillData {
    fn has(&self, flag: u32) -> bool {
        self.skill_attribute & flag != 0
    }
    fn needs_target(&self) -> bool {
        self.has(attribute::NEED_TARGET)
    }
    fn is_toggle(&self) -> bool {
        self.has(attribute::TOGGLE)
    }
    fn can_change_direction(&self) -> bool {
        self.has(attribute::CAN_CHANGE_DIRECTION)
    }
    fn is_fan_range(&self) -> bool {
        self.has(attribute::FAN_RANGE)
    }
    fn is_circle_range(&self) -> bool {
        self.has(attribute::CIRCLE_RANGE)
    }
    fn is_auto_search_target(&self) -> bool {
        self.has(attribute::SEARCH_TARGET)
    }
    fn needs_corpse(&self) -> bool {
        self.has(attribute::NEED_CORPSE)
    }
    fn is_standing(&self) -> bool {
        self.has(attribute::STANDING_SKILL)
    }
    fn only_for_alliance(&self) -> bool {
        self.has(attribute::ONLY_FOR_ALLIANCE)
    }
    fn can_use_for_me(&self) -> bool {
        self.has(attribute::CAN_USE_FOR_ME)
    }
    fn can_use_if_not_enough(&self) -> bool {
        self.has(attribute::CAN_USE_IF_NOT_ENOUGH)
    }
    fn needs_empty_bottle(&self) -> bool {
        self.has(attribute::NEED_EMPTY_BOTTLE)
    }
    fn needs_poison_bottle(&self) -> bool {
        self.has(attribute::NEED_POISON_BOTTLE)
    }
    fn needs_bow(&self) -> bool {
        self.has(attribute::WEAPON_LIMITATION) && self.need_weapon & need_weapon::BOW != 0
    }
    fn is_attack_skill(&self) -> bool {
        self.has(attribute::ATTACK_SKILL)
    }
    fn is_horse_skill(&self) -> bool {
        self.has(attribute::HORSE_SKILL)
    }
    fn is_moving_skill(&self) -> bool {
        self.has(attribute::MOVING_SKILL)
    }
    fn is_melee(&self) -> bool {
        self.has(attribute::MELEE_ATTACK)
    }
    fn is_charge(&self) -> bool {
        self.has(attribute::CHARGE_ATTACK)
    }
    fn can_use_weapon(&self, weapon_type: u8) -> bool {
        if !self.has(attribute::WEAPON_LIMITATION) {
            return true;
        }
        self.need_weapon & (1 << weapon_type) != 0
    }
    fn target_range(&self) -> u32 {
        if self.is_melee() || self.is_charge() {
            170
        } else {
            self.target_range
        }
    }
}

/// `CPythonPlayer::GetStatus` for the formulas: the point values the skill
/// expressions read.
#[derive(Clone, Copy, Debug, Default)]
pub struct Points {
    pub level: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub sp: i32,
    pub max_sp: i32,
    pub stamina: i32,
    pub att_speed: i32,
    pub move_speed: i32,
    pub casting_speed: i32,
    pub min_atk: i32,
    pub max_atk: i32,
    pub min_wep: i32,
    pub max_wep: i32,
    pub min_magic_wep: i32,
    pub max_magic_wep: i32,
    pub hit_rate: i32,
    pub str_: i32,
    pub dex: i32,
    pub con: i32,
    pub iq: i32,
    pub bow_distance: i32,
}

impl Points {
    /// The skill formulas' view of `CPythonPlayer::m_playerStatus.m_alPoint`.
    pub fn from_points(points: &[i32; 255]) -> Self {
        Self {
            level: points[point::LEVEL],
            hp: points[point::HP],
            max_hp: points[point::MAX_HP],
            sp: points[point::SP],
            max_sp: points[point::MAX_SP],
            stamina: points[point::STAMINA],
            att_speed: points[point::ATT_SPEED],
            move_speed: points[point::MOV_SPEED],
            casting_speed: points[point::CASTING_SPEED],
            min_atk: points[point::MIN_ATK],
            max_atk: points[point::MAX_ATK],
            min_wep: points[point::MIN_WEP],
            max_wep: points[point::MAX_WEP],
            min_magic_wep: points[point::MIN_MAGIC_WEP],
            max_magic_wep: points[point::MAX_MAGIC_WEP],
            hit_rate: points[point::HIT_RATE],
            str_: points[12],
            dex: points[14],
            con: points[13],
            iq: points[15],
            bow_distance: points[point::BOW_DISTANCE],
        }
    }

    /// `CPythonSkill::SSkillData::GetState` for the names used by the shipped
    /// formulas.
    pub fn get(&self, name: &str) -> Option<f32> {
        let value = match name.to_ascii_lowercase().as_str() {
            "lv" | "level" => self.level,
            "hp" => self.hp,
            "maxhp" | "max_hp" => self.max_hp,
            "sp" => self.sp,
            "maxsp" | "max_sp" => self.max_sp,
            "stamina" => self.stamina,
            "attspd" | "att_speed" => self.att_speed,
            "movspd" | "mov_speed" => self.move_speed,
            "casting_speed" => self.casting_speed,
            "atk" | "minatk" | "min_atk" | "attackpower" => self.min_atk,
            "maxatk" | "max_atk" => self.max_atk,
            "wep" | "minwep" | "min_wep" | "minmtk" => self.min_wep,
            "maxwep" | "max_wep" | "maxmtk" => self.max_wep,
            "minmwep" | "min_magic_wep" | "mtk" => self.min_magic_wep,
            "maxmwep" | "max_magic_wep" => self.max_magic_wep,
            "ar" | "hr" | "hit_rate" => self.hit_rate,
            "str" => self.str_,
            "dex" => self.dex,
            "con" => self.con,
            "iq" => self.iq,
            "bow_distance" => self.bow_distance,
            _ => return None,
        };
        Some(value as f32)
    }
}

/// Evaluates the arithmetic subset of `CPoly` the shipped skill tables use:
/// numbers, `k`/`SkillPoint`, the point names, `+ - * / ( )`, unary minus and
/// `%`. An unknown name is 0, as `GetState` leaves the value alone on a miss.
pub fn eval_formula(formula: &str, skill_point: f32, points: &Points) -> f32 {
    let tokens = tokenize(formula);
    let mut parser = Parser { tokens: &tokens, at: 0, skill_point, points };
    parser.expression().unwrap_or(0.0)
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f32),
    Name(String),
    Op(char),
}

fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            out.push(Token::Number(text[start..i].parse().unwrap_or(0.0)));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push(Token::Name(text[start..i].to_owned()));
        } else {
            out.push(Token::Op(c));
            i += 1;
        }
    }
    out
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    skill_point: f32,
    points: &'a Points,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }
    fn eat(&mut self, op: char) -> bool {
        if self.peek() == Some(&Token::Op(op)) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expression(&mut self) -> Option<f32> {
        let mut value = self.term()?;
        loop {
            if self.eat('+') {
                value += self.term()?;
            } else if self.eat('-') {
                value -= self.term()?;
            } else {
                return Some(value);
            }
        }
    }
    fn term(&mut self) -> Option<f32> {
        let mut value = self.factor()?;
        loop {
            if self.eat('*') {
                value *= self.factor()?;
            } else if self.eat('/') {
                let divisor = self.factor()?;
                value = if divisor == 0.0 { 0.0 } else { value / divisor };
            } else if self.eat('%') {
                let divisor = self.factor()?;
                value = if divisor == 0.0 { 0.0 } else { value % divisor };
            } else {
                return Some(value);
            }
        }
    }
    fn factor(&mut self) -> Option<f32> {
        if self.eat('-') {
            return Some(-self.factor()?);
        }
        if self.eat('+') {
            return self.factor();
        }
        match self.peek()?.clone() {
            Token::Number(value) => {
                self.at += 1;
                Some(value)
            }
            Token::Name(name) => {
                self.at += 1;
                if self.eat('(') {
                    let value = self.expression()?;
                    let other = self.eat(',').then(|| self.expression()).flatten();
                    if !self.eat(')') { return None; }
                    return Some(match name.to_ascii_lowercase().as_str() {
                        "floor" => value.floor(),
                        "ceil" => value.ceil(),
                        "abs" => value.abs(),
                        "sqrt" => value.sqrt(),
                        "min" => value.min(other?),
                        "max" => value.max(other?),
                        _ => return None,
                    });
                }
                if name.eq_ignore_ascii_case("k") || name.eq_ignore_ascii_case("skillpoint") {
                    Some(self.skill_point)
                } else {
                    let value = self.points.get(&name).unwrap_or(0.0);
                    // `ProcessFormula`: hit rate is a percentage.
                    Some(if name.eq_ignore_ascii_case("ar") { value / 100.0 } else { value })
                }
            }
            Token::Op('(') => {
                self.at += 1;
                let value = self.expression()?;
                self.eat(')');
                Some(value)
            }
            _ => None,
        }
    }
}

/// International `LocaleService_GetSkillPower` / 100 (the English locale).
pub fn skill_power(level: u8, grade: u8) -> f32 {
    const POWERS: [u8; 41] = [
        0, 5, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40,
        50, 52, 54, 56, 58, 60, 63, 66, 69, 72, 82, 85, 88, 91, 94, 98, 102, 106, 110, 115, 125,
    ];
    let raw = usize::from(level) + match grade { 1 => 19, 2 => 29, 3 => 39, _ => 0 };
    POWERS.get(raw).copied().map_or(0.0, |power| f32::from(power) / 100.0)
}

/// `CPythonSkill::SSkillData::GetNeedSP`: the `SP_COST_POLY` formula.
pub fn skill_need_sp(data: &SkillData, level: f32, points: &Points) -> i32 {
    let formula = String::from_utf8_lossy(&data.need_sp_formula);
    if formula.trim().is_empty() {
        return 0;
    }
    eval_formula(&formula, level, points) as i32
}

/// `CPythonSkill::SSkillData::GetSkillCoolTime`.
pub fn skill_cooldown(data: &SkillData, level: f32, points: &Points, casting_speed: i32) -> f32 {
    let formula = String::from_utf8_lossy(&data.cool_time_formula);
    let base = if formula.trim().is_empty() { 0.0 } else { eval_formula(&formula, level, points) };
    // `__RunCoolTime`: `100 - POINT_CASTING_SPEED`, then `100 + it` when
    // positive and `10000 / (100 - it)` when negative.
    let speed = 100 - casting_speed;
    let factor = match speed {
        s if s > 0 => 100 + s,
        s if s < 0 => 10000 / (100 - s),
        _ => 100,
    };
    base * factor as f32 / 100.0
}

/// `CPythonSkill::SSkillData::GetMotionLoopCount`.
pub fn skill_motion_loop_count(data: &SkillData, level: f32, points: &Points) -> u32 {
    let formula = String::from_utf8_lossy(&data.motion_loop_count_formula);
    if formula.trim().is_empty() {
        0
    } else {
        eval_formula(&formula, level, points).max(0.0) as u32
    }
}

/// `CPythonSkill::SSkillData::GetTargetCount`.
pub fn skill_target_count(data: &SkillData, level: f32, points: &Points) -> u32 {
    let formula = String::from_utf8_lossy(&data.target_count_formula);
    if formula.trim().is_empty() {
        0
    } else {
        eval_formula(&formula, level, points).max(0.0) as u32
    }
}

/// `CPythonSkill::SSkillData::GetSkillMotionIndex`: the grade's registered
/// motion index, `wMotionIndex` when no grade is set.
pub fn skill_motion_index(data: &SkillData, grade: u8) -> u16 {
    data.grades
        .get(usize::from(grade))
        .map(|grade| grade.motion_index)
        .filter(|index| *index != 0)
        .unwrap_or(data.motion_index)
}

/// Why `__CheckSkillUsable` refuses a skill (`OnCannotUseSkill` strings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkillError {
    Dead,
    Stunned,
    Observer,
    MountedNonHorseSkill,
    HorseSkillWithoutMount,
    MountSkillLevel,
    InSafe,
    Passive,
    NeedEmptyBottle,
    NeedPoisonBottle,
    Fishing,
    NotYetLearned,
    WrongWeapon,
    EmptyArrow,
    WaitCooltime,
    NotEnoughHp,
    NotEnoughSp,
    NeedTarget,
    OnlyForCorpse,
    CannotUseSelf,
    OnlyForAlliance,
    CannotAttack,
    CannotAttackEnemyInSafe,
    CannotApproach,
}

impl SkillError {
    /// The `OnCannotUseSkill(vid, type)` type passed to Python.
    pub fn reason(self) -> &'static str {
        match self {
            Self::Dead => "DEAD",
            Self::Stunned => "STUNNED",
            Self::Observer => "OBSERVER",
            Self::MountedNonHorseSkill => "NOT_HORSE_SKILL",
            Self::HorseSkillWithoutMount => "HAVE_TO_RIDE",
            Self::MountSkillLevel => "HORSE_SKILL_LEVEL",
            Self::InSafe => "IN_SAFE",
            Self::Passive => "PASSIVE",
            Self::NeedEmptyBottle => "NEED_EMPTY_BOTTLE",
            Self::NeedPoisonBottle => "NEED_POISON_BOTTLE",
            Self::Fishing => "REMOVE_FISHING_ROD",
            Self::NotYetLearned => "NOT_YET_LEARN",
            Self::WrongWeapon => "NOT_MATCHABLE_WEAPON",
            Self::EmptyArrow => "EMPTY_ARROW",
            Self::WaitCooltime => "WAIT_COOLTIME",
            Self::NotEnoughHp => "NOT_ENOUGH_HP",
            Self::NotEnoughSp => "NOT_ENOUGH_SP",
            Self::NeedTarget => "NEED_TARGET",
            Self::OnlyForCorpse => "ONLY_FOR_CORPSE",
            Self::CannotUseSelf => "CANNOT_USE_SELF",
            Self::OnlyForAlliance => "ONLY_FOR_ALLIANCE",
            Self::CannotAttack => "CANNOT_ATTACK",
            Self::CannotAttackEnemyInSafe => "CANNOT_ATTACK_ENEMY_IN_SAFE_AREA",
            Self::CannotApproach => "CANNOT_APPROACH",
        }
    }
}

/// The state `__CheckSkillUsable` reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct SkillCheck {
    pub level: u8,
    pub grade: u8,
    pub level_limit_enforced: bool,
    pub mounted: u32,
    pub in_safe: bool,
    pub fishing: bool,
    pub weapon_type: u8,
    pub arrows: u32,
    /// `m_sysIsCoolTime`.
    pub cool_time_enforced: bool,
    /// Seconds left on the slot (`fLastUsedTime + fCoolTime - now`).
    pub cool_time_left: f32,
    pub empty_bottles: u32,
    pub poison_bottles: u32,
    pub affected_by_dash: bool,
    pub is_charge_reserved: bool,
}

/// `CPythonPlayer::__CheckSkillUsable` (`PythonPlayerSkill.cpp:169`).
pub fn check_skill_usable(data: &SkillData, check: &SkillCheck, points: &Points) -> Result<(), SkillError> {
    if check.mounted != 0 {
        if !data.is_horse_skill() {
            return Err(SkillError::MountedNonHorseSkill);
        }
        if !mount_can_use_skill(check.mounted) {
            return Err(SkillError::MountSkillLevel);
        }
    }
    if data.is_horse_skill() && check.mounted == 0 {
        return Err(SkillError::HorseSkillWithoutMount);
    }
    if data.is_attack_skill() && check.in_safe {
        return Err(SkillError::InSafe);
    }
    if data.has(attribute::PASSIVE) {
        return Err(SkillError::Passive);
    }
    if data.needs_empty_bottle() && check.empty_bottles == 0 {
        return Err(SkillError::NeedEmptyBottle);
    }
    if data.needs_poison_bottle() && check.poison_bottles == 0 {
        return Err(SkillError::NeedPoisonBottle);
    }
    if check.fishing {
        return Err(SkillError::Fishing);
    }
    if check.level_limit_enforced && check.level == 0 {
        return Err(SkillError::NotYetLearned);
    }
    if !data.can_use_weapon(check.weapon_type) {
        return Err(SkillError::WrongWeapon);
    }
    // Horse skills are exempt from the arrow checks: some do not shoot.
    if !data.is_horse_skill() && data.needs_bow() && check.arrows == 0 {
        return Err(SkillError::EmptyArrow);
    }
    // A charge skill ignores the normal cooldown/life/mana checks while the
    // dash affect is on, as `__CheckSkillUsable` does.
    if check.affected_by_dash {
        if !data.is_charge() && check.cool_time_enforced && check.cool_time_left > 0.0 {
            return Err(SkillError::WaitCooltime);
        }
        return Ok(());
    }
    if check.cool_time_enforced && check.cool_time_left > 0.0 {
        return Err(SkillError::WaitCooltime);
    }
    if data.has(attribute::USE_HP) && skill_need_sp(data, skill_power(check.level, check.grade), points) > points.hp {
        return Err(SkillError::NotEnoughHp);
    }
    // `__CheckShortMana`: the summon skill (vnum 200?) is exempt; the shipped
    // table marks the summon with index 0 and the client tests the explicit
    // constant, so the same value is kept here.
    if data.skill_index != 0 {
        let need = skill_need_sp(data, skill_power(check.level, check.grade), points);
        if !data.is_toggle() && need == 0 {
            return Err(SkillError::NotEnoughSp);
        }
        if data.can_use_if_not_enough() {
            if points.sp <= 0 {
                return Err(SkillError::NotEnoughSp);
            }
        } else if need != -1 && need > points.sp {
            return Err(SkillError::NotEnoughSp);
        }
    }
    Ok(())
}

/// `CPythonPlayer::__GetSkillTargetRange`: the skill range plus the bow
/// distance bonus (metres, 100 units each).
pub fn skill_target_range(data: &SkillData, bow_distance: i32) -> u32 {
    data.target_range() + bow_distance.max(0) as u32 * 100
}

/// `CInstanceBase::NEW_GetFrontInstance`: the nearest attackable actor in a
/// narrow fan in front of the player, used by `SEARCH_TARGET` skills.
pub fn front_target(me: &ActorCombat, world: &CombatWorld, facing: f32, candidates: &[ActorCombat], distance: f32) -> Option<u32> {
    const HALF_FAN_ROT_MIN: f32 = 10.0;
    const HALF_FAN_ROT_MAX: f32 = 50.0;
    const HALF_FAN_ROT_MIN_DISTANCE: f32 = 1000.0;
    let mut best: Option<(f32, u32)> = None;
    for candidate in candidates {
        if candidate.vid == me.vid || !is_attackable(world, me, candidate) {
            continue;
        }
        let Some((dx, dy)) = candidate_position(candidate) else { continue };
        let dx = dx - me.x as f32;
        let dy = dy - me.y as f32;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > distance {
            continue;
        }
        let angle = (dx.atan2(dy).to_degrees() - facing).rem_euclid(360.0);
        let half = angle.min(360.0 - angle);
        let half_fan = (HALF_FAN_ROT_MAX - HALF_FAN_ROT_MIN)
            * (1.0 - (dist.min(HALF_FAN_ROT_MIN_DISTANCE) / HALF_FAN_ROT_MIN_DISTANCE))
            + HALF_FAN_ROT_MIN;
        if half <= half_fan && best.is_none_or(|(best_dist, _)| dist < best_dist) {
            best = Some((dist, candidate.vid));
        }
    }
    best.map(|(_, vid)| vid)
}

/// `CInstanceBase::NEW_GetInstanceVectorInFanRange` / `...CircleRange`.
/// `center` is the target's position, `range` the skill range; the result is
/// nearest first, as the C++ `multimap` iteration is.
pub fn skill_targets(
    me: &ActorCombat,
    world: &CombatWorld,
    candidates: &[ActorCombat],
    center: (f32, f32),
    range: f32,
    fan: bool,
) -> Vec<u32> {
    let mut found: Vec<(f32, u32)> = Vec::new();
    let center_angle = (center.0 - me.x as f32).atan2(center.1 - me.y as f32).to_degrees();
    for candidate in candidates {
        if candidate.vid == me.vid || !is_attackable(world, me, candidate) || candidate.is_dead() {
            continue;
        }
        let Some((dx, dy)) = candidate_position(candidate) else { continue };
        let dx = dx - me.x as f32;
        let dy = dy - me.y as f32;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > range {
            continue;
        }
        if fan {
            const HALF_FAN_ROT_MIN: f32 = 20.0;
            const HALF_FAN_ROT_MAX: f32 = 40.0;
            const HALF_FAN_ROT_MIN_DISTANCE: f32 = 1000.0;
            let rpm = (HALF_FAN_ROT_MAX - HALF_FAN_ROT_MIN) / HALF_FAN_ROT_MIN_DISTANCE;
            let angle = (dx.atan2(dy).to_degrees() - center_angle).rem_euclid(360.0);
            let half = angle.min(360.0 - angle);
            let half_fan =
                (HALF_FAN_ROT_MAX - HALF_FAN_ROT_MIN) - rpm * dist.min(HALF_FAN_ROT_MIN_DISTANCE) + HALF_FAN_ROT_MIN;
            if half > half_fan {
                continue;
            }
        }
        found.push((dist, candidate.vid));
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, vid)| vid).collect()
}

/// The tracked actor list has no position in [`ActorCombat`]; the scene
/// rewrites this function to supply coordinates. Kept as a hook so the pure
/// functions stay usable in tests.
fn candidate_position(candidate: &ActorCombat) -> Option<(f32, f32)> {
    Some((candidate.x as f32, candidate.y as f32))
}

// ---------------------------------------------------------------------------
// Skill levels and catalog (formerly `m2-ui/src/skills.rs`)
// ---------------------------------------------------------------------------

/// `SKILL_INDEX_DICT` with `NEW_678TH_SKILL_ENABLE == localeInfo.IsYMIR()`.
pub const WARRIOR_GROUP1: [u32; 14] = [1, 2, 3, 4, 5, 6, 0, 0, 137, 0, 138, 0, 139, 0];
pub const WARRIOR_GROUP2: [u32; 14] = [16, 17, 18, 19, 20, 21, 0, 0, 137, 0, 138, 0, 139, 0];
pub const ASSASSIN_GROUP1: [u32; 15] = [31, 32, 33, 34, 35, 36, 0, 0, 137, 0, 138, 0, 139, 0, 140];
pub const ASSASSIN_GROUP2: [u32; 15] = [46, 47, 48, 49, 50, 51, 0, 0, 137, 0, 138, 0, 139, 0, 140];
pub const SURA_GROUP1: [u32; 14] = [61, 62, 63, 64, 65, 66, 0, 0, 137, 0, 138, 0, 139, 0];
pub const SURA_GROUP2: [u32; 14] = [76, 77, 78, 79, 80, 81, 0, 0, 137, 0, 138, 0, 139, 0];
pub const SHAMAN_GROUP1: [u32; 14] = [91, 92, 93, 94, 95, 96, 0, 0, 137, 0, 138, 0, 139, 0];
pub const SHAMAN_GROUP2: [u32; 14] = [106, 107, 108, 109, 110, 111, 0, 0, 137, 0, 138, 0, 139, 0];
pub const SUPPORT: [u32; 12] = [122, 123, 121, 124, 125, 129, 0, 0, 130, 131, 141, 142];

/// `CPythonSkill::m_SkillDataMap`.
#[derive(Resource, Default)]
pub struct SkillCatalog(pub SkillTables);

impl SkillCatalog {
    pub fn get(&self, vnum: u32) -> Option<&SkillData> {
        self.0.skills.get(&vnum)
    }
}

/// The player's skill levels and grades, indexed by skill vnum.
#[derive(Resource, Clone)]
pub struct SkillState {
    pub levels: [u8; 255],
    pub grades: [u8; 255],
    /// `TSkillInstance::fCoolTime`/`fLastUsedTime`, indexed by skill vnum.
    pub cooldown: [f32; 255],
    pub last_used: [f32; 255],
    pub active: [bool; 255],
}

impl Default for SkillState {
    fn default() -> Self {
        Self { levels: [0; 255], grades: [0; 255], cooldown: [0.0; 255], last_used: [0.0; 255], active: [false; 255] }
    }
}

impl SkillState {
    /// `LocaleService_GetSkillPower` uses the absolute packet level, including
    /// the mastered grades (20..40), rather than the UI's 1..10 grade level.
    pub fn power(&self, vnum: u32) -> f32 {
        skill_power(self.level(vnum), self.grade(vnum))
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
    /// `CPythonPlayer::GetSkillGrade`.
    pub fn grade(&self, vnum: u32) -> u8 {
        self.grades.get(vnum as usize).copied().unwrap_or(0)
    }
    /// `CPythonPlayer::GetSkillLevel`.
    pub fn level(&self, vnum: u32) -> u8 {
        let Some(&raw) = self.levels.get(vnum as usize) else { return 0 };
        match self.grade(vnum) {
            0 => raw,
            1 => raw.saturating_sub(19),
            2 => raw.saturating_sub(29),
            3 => raw.saturating_sub(39),
            _ => raw,
        }
    }
    /// `SetSkillLevel_`'s reverse, used when a packet stores the absolute
    /// level of a mastered grade.
    pub fn cool_time_left(&self, vnum: u32, now: f32) -> f32 {
        let index = vnum as usize;
        self.last_used.get(index).copied().unwrap_or(0.0) + self.cooldown.get(index).copied().unwrap_or(0.0) - now
    }
    pub fn run_cool_time(&mut self, vnum: u32, cooldown: f32, now: f32) {
        let index = vnum as usize;
        if let Some(slot) = self.cooldown.get_mut(index) {
            *slot = cooldown;
        }
        if let Some(slot) = self.last_used.get_mut(index) {
            *slot = now;
        }
    }
}

/// `playersettingmodule.RegisterSkill`'s `job = chr.RaceToJob(race)`.
pub fn job(race: u16) -> u16 {
    race % 4
}

/// The active skill list of a job and group (`SKILL_INDEX_DICT[job][group]`).
pub fn active_table(race: u16, group: u8) -> &'static [u32] {
    match (job(race), group == 2) {
        (0, false) => &WARRIOR_GROUP1,
        (0, true) => &WARRIOR_GROUP2,
        (1, false) => &ASSASSIN_GROUP1,
        (1, true) => &ASSASSIN_GROUP2,
        (2, false) => &SURA_GROUP1,
        (2, true) => &SURA_GROUP2,
        (3, false) => &SHAMAN_GROUP1,
        _ => &SHAMAN_GROUP2,
    }
}

/// The skill vnum of a source slot index. Active slots are `1..` (unused
/// entries are 0); support slots are `101..112`.
pub fn slot_vnum(race: u16, group: u8, slot: u32, state: &SkillState) -> u32 {
    if slot >= 101 {
        return SUPPORT.get((slot - 101) as usize).copied().unwrap_or(0);
    }
    if slot == 0 {
        return 0;
    }
    if slot == 7 || slot == 8 {
        let (first, last) = if slot == 7 { (112, 115) } else { (116, 119) };
        if let Some(vnum) = (first..=last).rev().find(|vnum| state.levels.get(*vnum as usize).is_some_and(|level| *level != 0)) {
            return vnum;
        }
    }
    active_table(race, group).get((slot - 1) as usize).copied().unwrap_or(0)
}

/// `CPythonPlayer::GetSkillSlotIndex`: the reverse table.
pub fn slot_of_vnum(race: u16, group: u8, vnum: u32) -> Option<u32> {
    (1..=14)
        .find(|slot| active_table(race, group).get((slot - 1) as usize) == Some(&vnum))
        .or_else(|| SUPPORT.iter().position(|entry| *entry == vnum).map(|index| 101 + index as u32))
}

/// The skill a quick slot points at, from `TQuickSlot.Position` (a source
/// slot) and the character's active group.
pub fn quick_slot_vnum(race: u16, group: u8, position: u8, state: &SkillState) -> u32 {
    slot_vnum(race, group.clamp(1, 2), u32::from(position), state)
}

/// The skill vnum lookup used by the skill window and the task bar.
pub fn skill_by_slot(player: &crate::player::PlayerState, state: &SkillState, group: u8, slot: u32) -> Option<(u32, u8)> {
    let vnum = slot_vnum(player.race, group, slot, state);
    (vnum != 0).then(|| (vnum, state.level(vnum)))
}

/// One entry of the combat's skill map: the slot/level the client uses when a
/// skill is cast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillSlot {
    pub vnum: u32,
    pub level: u8,
    pub grade: u8,
}

/// The skills of the active group, as `FindSkillSlotIndexBySkillIndex` sees
/// them.
pub fn skill_slots(race: u16, group: u8, state: &SkillState) -> BTreeMap<u32, SkillSlot> {
    (1..=14)
        .chain(101..=112)
        .filter_map(|slot| {
            let vnum = slot_vnum(race, group, slot, state);
            (vnum != 0).then(|| SkillSlot { vnum, level: state.level(vnum), grade: state.grade(vnum) })
        })
        .map(|slot| (slot.vnum, slot))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(vid: u32, kind: u8, empire: u8) -> ActorCombat {
        ActorCombat { vid, kind, empire, ..Default::default() }
    }

    fn pc(vid: u32, empire: u8) -> ActorCombat {
        actor(vid, ActorType::Pc as u8, empire)
    }

    #[test]
    fn own_actor_is_never_attackable() {
        let world = CombatWorld::default();
        let me = pc(1, 1);
        assert!(!is_attackable(&world, &me, &me));
    }

    #[test]
    fn other_empire_players_are_attackable_and_same_empire_peace_is_not() {
        let world = CombatWorld::default();
        let me = pc(1, 1);
        assert!(is_attackable(&world, &me, &pc(2, 2)));
        assert!(!is_attackable(&world, &me, &pc(3, 1)));
    }

    #[test]
    fn free_pk_attacks_same_empire_unless_party_or_protected() {
        let me = ActorCombat { pk_mode: PK_MODE_FREE, ..pc(1, 1) };
        let victim = ActorCombat { alignment: 1000, ..pc(2, 1) };
        let world = CombatWorld::default();
        assert!(is_attackable(&world, &me, &victim));
        let mut party = CombatWorld::default();
        party.party.extend([1, 2]);
        assert!(!is_attackable(&party, &me, &victim));
    }

    #[test]
    fn protector_mode_only_allows_conflicting_alignment() {
        let me = ActorCombat { pk_mode: PK_MODE_PROTECT, ..pc(1, 1) };
        let victim = ActorCombat { alignment: -5000, ..pc(2, 1) };
        assert!(!is_attackable(&CombatWorld::default(), &me, &victim));
    }

    #[test]
    fn duel_only_hits_the_registered_pairs() {
        let me = ActorCombat { pk_mode: PK_MODE_PROTECT, ..pc(1, 1) };
        let victim = pc(2, 1);
        let mut world = CombatWorld { duel_mode: DUEL_START, ..Default::default() };
        world.duel_keys.insert((1, 2));
        assert!(is_attackable(&world, &me, &victim));
        world.duel_keys.clear();
        assert!(!is_attackable(&world, &me, &victim));
        assert!(!is_attackable(&CombatWorld { duel_mode: DUEL_CANNOTATTACK, ..Default::default() }, &me, &victim));
    }

    #[test]
    fn front_search_picks_the_closest_actor_in_the_fan() {
        let me = ActorCombat { x: 0, y: 0, ..pc(1, 1) };
        let off = ActorCombat { x: 500, y: 500, ..pc(2, 2) };
        let front = ActorCombat { x: 0, y: 300, ..pc(3, 2) };
        let world = CombatWorld { observer: false, ..CombatWorld::default() };
        let found = front_target(&me, &world, 0.0, &[off, front], 2000.0);
        assert_eq!(found, Some(3));
        // Facing south does not see the actor to the north.
        assert_eq!(front_target(&me, &world, 180.0, &[front], 2000.0), None);
    }

    #[test]
    fn circle_and_fan_ranges_are_nearest_first() {
        let me = ActorCombat { x: 0, y: 0, ..pc(1, 1) };
        let near = ActorCombat { x: 100, y: 0, ..pc(2, 2) };
        let far = ActorCombat { x: 300, y: 0, ..pc(3, 2) };
        let world = CombatWorld::default();
        let targets = skill_targets(&me, &world, &[far, near], (100.0, 0.0), 500.0, false);
        assert_eq!(targets, vec![2, 3]);
    }

    #[test]
    fn kingdom_and_building_rules_follow_the_actor_types() {
        let world = CombatWorld::default();
        let enemy = actor(4, ActorType::Enemy as u8, 0);
        let building = actor(5, ActorType::Building as u8, 0);
        assert!(is_attackable(&world, &enemy, &pc(1, 1)));
        assert!(is_attackable(&world, &enemy, &building));
        assert!(is_attackable(&world, &pc(1, 1), &enemy));
        let stone = actor(6, ActorType::Stone as u8, 0);
        assert!(is_attackable(&world, &stone, &pc(1, 1)));
        assert!(is_attackable(&world, &pc(1, 1), &stone));
    }

    #[test]
    fn hidden_and_dead_actors_cannot_be_picked() {
        let mut target = pc(2, 1);
        assert!(target.can_pick());
        target.affect_flags |= 1 << AFFECT_INVISIBILITY;
        assert!(!target.can_pick());
        target.affect_flags = 0;
        target.state_flags |= STATE_DEAD;
        assert!(!target.can_pick());
    }

    #[test]
    fn mount_levels_follow_the_cpp_table() {
        assert_eq!(mount_level(20101), 1);
        assert_eq!(mount_level(20104), 2);
        assert_eq!(mount_level(20107), 3);
        assert_eq!(mount_level(20205), 2);
        assert_eq!(mount_level(20209), 3);
        assert!(!mount_can_attack(20101));
        assert!(mount_can_attack(20104));
        assert!(!mount_can_use_skill(20104));
        assert!(mount_can_use_skill(20109));
        assert!(is_new_mount(20205));
        assert!(!is_new_mount(20104));
    }

    #[test]
    fn new_mount_needs_riding_skill_to_attack() {
        let context = AttackContext {
            world: CombatWorld::default(),
            me: pc(1, 1),
            mounted: 20205,
            riding_grade: 1,
            riding_level: 0,
            observer: false,
            in_safe: false,
            hand_mode: false,
            arrows: 0,
        };
        assert_eq!(context.can_attack(), Ok(()));
        let low = AttackContext { riding_grade: 0, riding_level: 0, ..context };
        assert_eq!(low.can_attack(), Err(AttackError::NewMountSkill));
    }

    #[test]
    fn bare_handed_mounted_attack_is_refused() {
        let context = AttackContext {
            world: CombatWorld::default(),
            me: pc(1, 1),
            mounted: 20107,
            riding_grade: 1,
            riding_level: 20,
            observer: false,
            in_safe: false,
            hand_mode: true,
            arrows: 0,
        };
        assert_eq!(context.mounted_hand_attack(), Err(AttackError::MountedHandAttack));
    }

    #[test]
    fn shot_requires_arrows_and_no_safe_area() {
        let context = AttackContext {
            world: CombatWorld::default(),
            me: pc(1, 1),
            mounted: 0,
            riding_grade: 0,
            riding_level: 0,
            observer: false,
            in_safe: false,
            hand_mode: false,
            arrows: 0,
        };
        assert_eq!(context.can_shoot(false), Err(AttackError::EmptyArrow));
        assert_eq!(AttackContext { arrows: 3, ..context.clone() }.can_shoot(false), Ok(()));
        assert_eq!(AttackContext { arrows: 3, in_safe: true, ..context.clone() }.can_shoot(false), Err(AttackError::InSafe));
        assert_eq!(AttackContext { arrows: 3, ..context.clone() }.can_shoot(true), Err(AttackError::DestInSafe));
    }

    #[test]
    fn formula_evaluator_handles_the_shipped_skill_expressions() {
        let points = Points { min_atk: 100, str_: 20, sp: 500, hp: 400, ..Default::default() };
        assert_eq!(eval_formula("40+100*k", 2.0, &points), 240.0);
        assert_eq!(eval_formula("(1.1*MinATK + (0.1*MinATK + 1.5*STR)*SkillPoint) * 3", 1.0, &points), (110.0 + (10.0 + 30.0)) * 3.0);
        assert_eq!(eval_formula("12", 0.0, &points), 12.0);
        assert_eq!(eval_formula("unknown + 1", 0.0, &points), 1.0);
    }

    #[test]
    fn skill_cooldown_scales_with_casting_speed() {
        let data = SkillData { cool_time_formula: b"10".to_vec(), ..Default::default() };
        let points = Points::default();
        assert_eq!(skill_cooldown(&data, 0.0, &points, 0), 20.0);
        // 100 - 10 = 90 > 0 -> factor 190, so 19 s.
        assert!((skill_cooldown(&data, 0.0, &points, 10) - 19.0).abs() < 1e-5);
        // 100 - 110 = -10 -> 10000 / 110, so 9.09 s.
        assert!((skill_cooldown(&data, 0.0, &points, 110) - 9.0).abs() < 1e-5);
    }

    #[test]
    fn skill_checks_cover_horse_weapon_and_resources() {
        let points = Points { hp: 100, sp: 50, ..Default::default() };
        let mut data = SkillData {
            skill_index: 1,
            skill_attribute: attribute::NEED_TARGET | attribute::ATTACK_SKILL,
            need_weapon: need_weapon::BOW,
            ..Default::default()
        };
        data.skill_attribute |= attribute::WEAPON_LIMITATION;
        data.need_sp_formula = b"10".to_vec();
        let check = SkillCheck { level: 1, weapon_type: weapon::BOW, arrows: 1, ..Default::default() };
        assert_eq!(check_skill_usable(&data, &check, &points), Ok(()));
        assert_eq!(
            check_skill_usable(&data, &SkillCheck { mounted: 20104, ..check }, &points),
            Err(SkillError::MountedNonHorseSkill)
        );
        assert_eq!(
            check_skill_usable(&data, &SkillCheck { arrows: 0, ..check }, &points),
            Err(SkillError::EmptyArrow)
        );
        assert_eq!(
            check_skill_usable(&data, &SkillCheck { weapon_type: weapon::SWORD, ..check }, &points),
            Err(SkillError::WrongWeapon)
        );
        assert_eq!(
            check_skill_usable(&data, &SkillCheck { in_safe: true, ..check }, &points),
            Err(SkillError::InSafe)
        );
        data.need_sp_formula = b"100".to_vec();
        assert_eq!(
            check_skill_usable(&data, &SkillCheck { level: 1, ..check }, &points),
            Err(SkillError::NotEnoughSp)
        );
    }

    #[test]
    fn active_slot_tables_match_playersettingmodule() {
        let mut state = SkillState::default();
        assert_eq!(slot_vnum(0, 1, 1, &state), 1);
        assert_eq!(slot_vnum(4, 1, 1, &state), 1);
        assert_eq!(slot_vnum(0, 1, 14, &state), 0);
        assert_eq!(slot_vnum(0, 2, 1, &state), 16);
        assert_eq!(slot_vnum(5, 1, 1, &state), 31);
        assert_eq!(slot_vnum(6, 1, 1, &state), 61);
        assert_eq!(slot_vnum(7, 1, 1, &state), 91);
        assert_eq!(slot_vnum(0, 1, 101, &state), 122);
        state.levels[114] = 4;
        assert_eq!(slot_vnum(0, 1, 7, &state), 114);
        assert_eq!(slot_of_vnum(0, 1, 3), Some(3));
    }
}
