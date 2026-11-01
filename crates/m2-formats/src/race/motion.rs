//! `CRaceMotionData::LoadMotionData` (`client/src/game/RaceMotionData.cpp`)
//! and the motion events of `RaceMotionDataEvent.h`.

use super::data::{AttackData, CollisionData, MotionAttackData};
use super::sound::{SoundInstance, data_to_instance, parse_sound_information};
use super::{Cursor, case_crc32, float_to_dword, no_extension, vector3_of};
use crate::pack::PackManager;
use crate::script::{self, Script};

/// `g_fGameFPS` (`GameType.cpp`), used for event frames.
pub const GAME_FPS: f32 = 60.0;

/// `CRaceMotionData::EMotionEventType`.
pub const MOTION_EVENT_TYPE_NONE: i32 = 0;
/// `MOTION_EVENT_TYPE_EFFECT`.
pub const MOTION_EVENT_TYPE_EFFECT: i32 = 1;
/// `MOTION_EVENT_TYPE_SCREEN_WAVING`.
pub const MOTION_EVENT_TYPE_SCREEN_WAVING: i32 = 2;
/// `MOTION_EVENT_TYPE_SCREEN_FLASHING` (not loadable: C++ rejects it).
pub const MOTION_EVENT_TYPE_SCREEN_FLASHING: i32 = 3;
/// `MOTION_EVENT_TYPE_SPECIAL_ATTACKING`.
pub const MOTION_EVENT_TYPE_SPECIAL_ATTACKING: i32 = 4;
/// `MOTION_EVENT_TYPE_SOUND`.
pub const MOTION_EVENT_TYPE_SOUND: i32 = 5;
/// `MOTION_EVENT_TYPE_FLY`.
pub const MOTION_EVENT_TYPE_FLY: i32 = 6;
/// `MOTION_EVENT_TYPE_CHARACTER_SHOW`.
pub const MOTION_EVENT_TYPE_CHARACTER_SHOW: i32 = 7;
/// `MOTION_EVENT_TYPE_CHARACTER_HIDE`.
pub const MOTION_EVENT_TYPE_CHARACTER_HIDE: i32 = 8;
/// `MOTION_EVENT_TYPE_WARP`.
pub const MOTION_EVENT_TYPE_WARP: i32 = 9;
/// `MOTION_EVENT_TYPE_EFFECT_TO_TARGET`.
pub const MOTION_EVENT_TYPE_EFFECT_TO_TARGET: i32 = 10;

/// `NMotionEvent::SMotionEventDataEffect`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectEvent {
    /// `isAttaching` (`BOOL`, the `atoi` value).
    pub is_attaching: i32,
    /// `isFollowing`.
    pub is_following: i32,
    /// `isIndependent`.
    pub is_independent: i32,
    /// `strAttachingBoneName`.
    pub attaching_bone_name: Vec<u8>,
    /// `v3EffectPosition`.
    pub effect_position: [f32; 3],
    /// `dwEffectIndex`: `GetCaseCRC32` of the file name.
    pub effect_index: u32,
    /// `strEffectFileName`.
    pub effect_file_name: Vec<u8>,
}

impl EffectEvent {
    /// `SMotionEventDataEffect::Load`. C++ also registers the effect.
    fn load(&mut self, cur: &Cursor) -> bool {
        if !cur.int("independentflag", &mut self.is_independent) {
            self.is_independent = 0;
        }
        if !cur.int("attachingenable", &mut self.is_attaching)
            || !cur.string("attachingbonename", &mut self.attaching_bone_name)
            || !cur.string("effectfilename", &mut self.effect_file_name)
            || !cur.vector3("effectposition", &mut self.effect_position)
        {
            return false;
        }
        if !cur.int("followingenable", &mut self.is_following) {
            self.is_following = 0;
        }
        self.effect_index = case_crc32(&self.effect_file_name);
        true
    }
}

/// `NMotionEvent::SMotionEventDataEffectToTarget`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectToTargetEvent {
    /// `dwEffectIndex`: `GetCaseCRC32` of the file name.
    pub effect_index: u32,
    /// `strEffectFileName`.
    pub effect_file_name: Vec<u8>,
    /// `v3EffectPosition`.
    pub effect_position: [f32; 3],
    /// `isFollowing`.
    pub is_following: i32,
    /// `isFishingEffect`.
    pub is_fishing_effect: i32,
}

impl EffectToTargetEvent {
    /// `SMotionEventDataEffectToTarget::Load`.
    fn load(&mut self, cur: &Cursor) -> bool {
        if !cur.string("effectfilename", &mut self.effect_file_name)
            || !cur.vector3("effectposition", &mut self.effect_position)
        {
            return false;
        }
        if !cur.int("followingenable", &mut self.is_following) {
            self.is_following = 0;
        }
        if !cur.int("fishingeffectflag", &mut self.is_fishing_effect) {
            self.is_fishing_effect = 0;
        }
        self.effect_index = case_crc32(&self.effect_file_name);
        true
    }
}

/// `NMotionEvent::SMotionEventDataFly`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlyEvent {
    /// `isAttaching`.
    pub is_attaching: i32,
    /// `strAttachingBoneName`.
    pub attaching_bone_name: Vec<u8>,
    /// `v3FlyPosition`.
    pub fly_position: [f32; 3],
    /// `dwFlyIndex`: `GetCaseCRC32` of the file name.
    pub fly_index: u32,
    /// `strFlyFileName`.
    pub fly_file_name: Vec<u8>,
}

impl FlyEvent {
    /// `SMotionEventDataFly::Load`. C++ also registers the flying data.
    fn load(&mut self, cur: &Cursor) -> bool {
        if !cur.int("attachingenable", &mut self.is_attaching)
            || !cur.string("attachingbonename", &mut self.attaching_bone_name)
            || !cur.string("flyfilename", &mut self.fly_file_name)
            || !cur.vector3("flyposition", &mut self.fly_position)
        {
            return false;
        }
        self.fly_index = case_crc32(&self.fly_file_name);
        true
    }
}

/// `NMotionEvent::SMotionEventDataScreenWaving` (its `DuringTime` is the
/// event's [`MotionEvent::duration_time`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScreenWavingEvent {
    /// `iPower`.
    pub power: i32,
    /// `iAffectingRange`.
    pub affecting_range: i32,
}

/// `NMotionEvent::SMotionEventDataAttack` (its `DuringTime` is the event's
/// [`MotionEvent::duration_time`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AttackingEvent {
    /// `CollisionData`.
    pub collision: CollisionData,
    /// `AttackData`.
    pub attack: AttackData,
    /// `isEnableHitProcess`: `TRUE` (1) when the key is missing.
    pub is_enable_hit_process: i32,
}

/// `NMotionEvent::SMotionEventDataSound`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoundEvent {
    /// `strSoundFileName`.
    pub sound_file_name: Vec<u8>,
}

/// The type-specific part of a motion event: which `SMotionEventData`
/// subclass C++ allocated for `MotionEventType`.
#[derive(Debug, Clone, PartialEq)]
pub enum MotionEventData {
    /// Type 1.
    Effect(EffectEvent),
    /// Type 2.
    ScreenWaving(ScreenWavingEvent),
    /// Type 4.
    SpecialAttacking(AttackingEvent),
    /// Type 5.
    Sound(SoundEvent),
    /// Type 6.
    Fly(FlyEvent),
    /// Type 7.
    CharacterShow,
    /// Type 8.
    CharacterHide,
    /// Type 9.
    Warp,
    /// Type 10.
    EffectToTarget(EffectToTargetEvent),
}

/// `NMotionEvent::SMotionEventData` with its subclass.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionEvent {
    /// `iType` (`MOTION_EVENT_TYPE_*`).
    pub event_type: i32,
    /// `dwFrame`: `StartingTime` in [`GAME_FPS`] frames, truncated.
    pub frame: u32,
    /// `fStartingTime`.
    pub starting_time: f32,
    /// `fDurationTime` (`DuringTime` of waving and attacking events).
    pub duration_time: f32,
    /// The subclass fields.
    pub data: MotionEventData,
}

/// `CRaceMotionData::SComboInputData`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ComboInputData {
    /// `fInputStartTime` (`PreInputTime`).
    pub input_start_time: f32,
    /// `fNextComboTime` (`DirectInputTime`).
    pub next_combo_time: f32,
    /// `fInputEndTime` (`InputLimitTime`).
    pub input_end_time: f32,
}

/// What `CRaceMotionData` holds after a successful `LoadMotionData`.
/// Fields that only make sense with their flag (`combo_input`,
/// `motion_attack_data`, `accumulation_position`) are defaults without it.
/// The name/type/lock set by `CRaceMotionData::SetName` at registration are
/// not part of loading.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RaceMotionData {
    /// `m_strMotionFileName`: the `.gr2` animation.
    pub motion_file_name: Vec<u8>,
    /// `m_strSoundScriptDataFileName` (see [`sound_script_file_name`]).
    pub sound_script_file_name: Vec<u8>,
    /// `m_fMotionDuration`.
    pub motion_duration: f32,
    /// `m_isAccumulationMotion`.
    pub is_accumulation_motion: bool,
    /// `m_accumulationPosition`.
    pub accumulation_position: [f32; 3],
    /// `m_isComboMotion`.
    pub is_combo_motion: bool,
    /// `m_ComboInputData`.
    pub combo_input: ComboInputData,
    /// `m_isLoopMotion`.
    pub is_loop_motion: bool,
    /// `m_iLoopCount`: -1 (forever) when `LoopData` has no count.
    pub loop_count: i32,
    /// `m_bCancelEnableSkill`: a `BOOL` holding `atoi` of the token.
    pub cancel_enable_skill: i32,
    /// `m_fLoopStartTime`.
    pub loop_start_time: f32,
    /// `m_fLoopEndTime`.
    pub loop_end_time: f32,
    /// `m_isAttackingMotion`.
    pub is_attacking_motion: bool,
    /// `m_MotionAttackData`.
    pub motion_attack_data: MotionAttackData,
    /// `m_hasSplashEvent`: some event is special attacking.
    pub has_splash_event: bool,
    /// `m_MotionEventDataVector`.
    pub events: Vec<MotionEvent>,
    /// `m_SoundInstanceVector`, from the sound script.
    pub sound_instances: Vec<SoundInstance>,
}

/// The sound script `LoadMotionData` reads for the motion `file_name`:
/// `sound/` followed by the name without extension plus `.mss`, from its
/// 14th byte on (past `d:/ymir work/`). `None` when that name is 13 bytes
/// or shorter.
pub fn sound_script_file_name(file_name: &[u8]) -> Option<Vec<u8>> {
    let name = [no_extension(file_name), b".mss"].concat();
    (name.len() > 13).then(|| [&b"sound/"[..], &name[13..]].concat())
}

impl RaceMotionData {
    /// `CRaceMotionData::LoadMotionData`: the motion script `file_name` and
    /// its sound script. `None` where C++ returns `false` (its caller then
    /// deletes the motion).
    pub fn load(manager: &PackManager, file_name: impl AsRef<[u8]>) -> Option<Self> {
        let file_name = file_name.as_ref();
        let mut motion = Self::from_script(&script::load(manager, file_name).ok()?)?;

        if let Some(sound) = sound_script_file_name(file_name) {
            // `LoadSoundScriptData`; its result is ignored.
            if let Ok(script) = script::load(manager, &sound)
                && let Some(data) = parse_sound_information(&script, None)
            {
                data_to_instance(&data, &mut motion.sound_instances);
            }
            motion.sound_script_file_name = sound;
        }
        Some(motion)
    }

    /// `CRaceMotionData::LoadMotionData` on a parsed motion script, without
    /// the sound script. Like `CTextFileLoader::Cache`, it uses the tree even
    /// when the braces do not balance.
    pub fn from_script(script: &Script) -> Option<Self> {
        let frame_time = 1.0f32 / GAME_FPS;
        let mut m = Self::default();
        let mut cur = Cursor::new(&script.root);

        if !cur.string("motionfilename", &mut m.motion_file_name) || !cur.float("motionduration", &mut m.motion_duration)
        {
            return None;
        }

        if let Some(tokens) = cur.tokens("accumulation") {
            if tokens.len() != 3 {
                return None;
            }
            m.accumulation_position = vector3_of(tokens);
            m.is_accumulation_motion = true;
        }

        let mut node_name: &[u8] = &[];
        let mut i = 0;
        while i < cur.child_count() {
            // `CGotoChild`: SetChildNode(i) now, SetParentNode at the end of
            // the iteration (also on `continue`).
            cur.set_child_at(i);
            if let Some(name) = cur.current_name() {
                node_name = name;
            }

            match node_name {
                b"comboinputdata" => {
                    m.is_combo_motion = true;
                    let c = &mut m.combo_input;
                    if !cur.float("preinputtime", &mut c.input_start_time)
                        || !cur.float("directinputtime", &mut c.next_combo_time)
                        || !cur.float("inputlimittime", &mut c.input_end_time)
                    {
                        return None;
                    }
                }
                b"attackingdata" => {
                    m.is_attacking_motion = true;
                    if !m.motion_attack_data.load(&mut cur) {
                        return None;
                    }
                }
                b"loopdata" => {
                    m.is_loop_motion = true;
                    if !cur.int("motionloopcount", &mut m.loop_count) {
                        m.loop_count = -1;
                    }
                    if !cur.int("loopcancelenable", &mut m.cancel_enable_skill) {
                        m.cancel_enable_skill = 0;
                    }
                    if !cur.float("loopstarttime", &mut m.loop_start_time)
                        || !cur.float("loopendtime", &mut m.loop_end_time)
                    {
                        return None;
                    }
                }
                b"motioneventdata" => {
                    let mut count = 0u32;
                    if cur.dword("motioneventdatacount", &mut count) {
                        m.events.clear();
                        for j in 0..count {
                            let event = load_event(&mut cur, j, frame_time, &mut m.has_splash_event)?;
                            m.events.push(event);
                        }
                    }
                }
                _ => {}
            }

            cur.set_parent();
            i += 1;
        }

        Some(m)
    }
}

/// One iteration of the event loop of `LoadMotionData`: `EventNN`.
///
/// The event's own `Load` result is ignored, as in C++: fields it did not
/// reach keep their defaults, and a failed collision load can leave the
/// cursor in a `SphereDataNN` group, where `StartingTime` is then looked up.
fn load_event(cur: &mut Cursor, j: u32, frame_time: f32, has_splash: &mut bool) -> Option<MotionEvent> {
    if !cur.set_child_indexed(b"event", j) {
        return None;
    }
    let mut event_type = 0;
    if !cur.int("motioneventtype", &mut event_type) {
        return None;
    }

    let mut duration_time = 0.0;
    let data = match event_type {
        MOTION_EVENT_TYPE_FLY => {
            let mut e = FlyEvent::default();
            e.load(cur);
            MotionEventData::Fly(e)
        }
        MOTION_EVENT_TYPE_EFFECT => {
            let mut e = EffectEvent::default();
            e.load(cur);
            MotionEventData::Effect(e)
        }
        MOTION_EVENT_TYPE_SCREEN_WAVING => {
            // `SMotionEventDataScreenWaving::Load`.
            let mut e = ScreenWavingEvent::default();
            if cur.float("duringtime", &mut duration_time)
                && cur.int("power", &mut e.power)
                && !cur.int("affectingrange", &mut e.affecting_range)
            {
                e.affecting_range = 0;
            }
            MotionEventData::ScreenWaving(e)
        }
        MOTION_EVENT_TYPE_SPECIAL_ATTACKING => {
            *has_splash = true;
            // `SMotionEventDataAttack::Load`.
            let mut e = AttackingEvent::default();
            if cur.float("duringtime", &mut duration_time) {
                if !cur.int("enablehitprocess", &mut e.is_enable_hit_process) {
                    e.is_enable_hit_process = 1;
                }
                if e.attack.load(cur) {
                    e.collision.load(cur);
                }
            }
            MotionEventData::SpecialAttacking(e)
        }
        MOTION_EVENT_TYPE_SOUND => {
            let mut e = SoundEvent::default();
            cur.string("soundfilename", &mut e.sound_file_name);
            MotionEventData::Sound(e)
        }
        MOTION_EVENT_TYPE_CHARACTER_SHOW => MotionEventData::CharacterShow,
        MOTION_EVENT_TYPE_CHARACTER_HIDE => MotionEventData::CharacterHide,
        MOTION_EVENT_TYPE_WARP => MotionEventData::Warp,
        MOTION_EVENT_TYPE_EFFECT_TO_TARGET => {
            let mut e = EffectToTargetEvent::default();
            e.load(cur);
            MotionEventData::EffectToTarget(e)
        }
        // "Strange Event Type", including screen flashing.
        _ => return None,
    };

    let mut starting_time = 0.0;
    if !cur.float("startingtime", &mut starting_time) {
        return None;
    }
    cur.set_parent();

    Some(MotionEvent {
        event_type,
        frame: float_to_dword(starting_time / frame_time),
        starting_time,
        duration_time,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::parse;

    fn motion(text: &[u8]) -> Option<RaceMotionData> {
        RaceMotionData::from_script(&parse(text))
    }

    #[test]
    fn sound_script_names() {
        assert_eq!(
            sound_script_file_name(b"d:/ymir work/pc/warrior/general/wait.msa").as_deref(),
            Some(&b"sound/pc/warrior/general/wait.mss"[..])
        );
        assert_eq!(sound_script_file_name(b"a.msa"), None);
        assert_eq!(sound_script_file_name(b"123456789.msa"), None);
        assert_eq!(sound_script_file_name(b"1234567890.msa").as_deref(), Some(&b"sound/s"[..]));
    }

    #[test]
    fn loads_groups_and_events() {
        let m = motion(
            b"MotionFileName \"a.gr2\"\nMotionDuration 1.5\nAccumulation 0 -10 0\n\
              Group LoopData\n{\nLoopStartTime 0.1\nLoopEndTime 0.9\nLoopCancelEnable 3\n}\n\
              Group MotionEventData\n{\nMotionEventDataCount 3\n\
              Group Event00\n{\nMotionEventType 5\nStartingTime 0.5\nSoundFileName \"s.wav\"\n}\n\
              Group Event01\n{\nMotionEventType 2\nStartingTime 1\nDuringTime 0.25\nPower 3\n}\n\
              Group Event02\n{\nMotionEventType 1\nStartingTime 0.3\nAttachingEnable 1\nAttachingBoneName \"\"\n\
              EffectFileName \"E.mse\"\nEffectPosition 1 2\n}\n}\n",
        )
        .unwrap();
        assert!(m.is_accumulation_motion);
        assert_eq!(m.accumulation_position, [0.0, -10.0, 0.0]);
        assert!(m.is_loop_motion);
        assert_eq!((m.loop_count, m.cancel_enable_skill), (-1, 3));
        assert_eq!(m.events.len(), 3);
        assert_eq!(m.events[0].frame, 29, "0.5 / (1.0f / 60) is just below 30");
        assert_eq!(m.events[1].duration_time, 0.25);
        assert_eq!(m.events[1].data, MotionEventData::ScreenWaving(ScreenWavingEvent { power: 3, affecting_range: 0 }));
        // Effect load stops at the two-number position; the result is ignored.
        let MotionEventData::Effect(e) = &m.events[2].data else { panic!() };
        assert_eq!(e.effect_file_name, b"E.mse");
        assert_eq!(e.effect_index, 0);
        assert_eq!(m.events[2].frame, 18);
    }

    #[test]
    fn failures() {
        assert!(motion(b"MotionFileName a\n").is_none());
        assert!(motion(b"MotionFileName a\nMotionDuration 1\nAccumulation 1 2\n").is_none());
        assert!(motion(b"MotionFileName a\nMotionDuration 1\nGroup ComboInputData\n{\nPreInputTime 1\n}\n").is_none());
        let event = |t: &str| {
            format!("MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{{\nMotionEventDataCount 1\nGroup Event00\n{{\nMotionEventType {t}\nStartingTime 0\n}}\n}}\n")
        };
        assert!(motion(event("3").as_bytes()).is_none());
        assert!(motion(event("0").as_bytes()).is_none());
        assert!(motion(event("7").as_bytes()).is_some());
        // A missing event count skips the group; unbalanced braces still load.
        assert!(motion(b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\n}\nGroup X\n{\n").is_some());
    }

    #[test]
    fn failed_collision_strands_the_cursor() {
        // The sphere group lacks a position: the cursor stays in
        // SphereData00, StartingTime is found there, and SetParentNode only
        // gets back to Event00, so Event01 is not found.
        let text = b"MotionFileName a\nMotionDuration 1\nGroup MotionEventData\n{\nMotionEventDataCount 2\n\
              Group Event00\n{\nMotionEventType 4\nStartingTime 0\nDuringTime 1\nHittingType 0\nStiffenTime 0\n\
              InvisibleTime 0\nExternalForce 0\nCollisionType 2\nSphereDataCount 1\nGroup SphereData00\n{\nRadius 5\nStartingTime 2\n}\n}\n\
              Group Event01\n{\nMotionEventType 7\nStartingTime 0\n}\n}\n";
        assert!(motion(text).is_none());
        let one = String::from_utf8_lossy(text).replace("MotionEventDataCount 2", "MotionEventDataCount 1");
        let m = motion(one.as_bytes()).unwrap();
        assert_eq!(m.events[0].starting_time, 2.0);
        assert!(m.has_splash_event);
    }
}
