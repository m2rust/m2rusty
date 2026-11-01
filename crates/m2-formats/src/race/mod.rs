//! Race and motion data, as `client/src/game` loads them:
//!
//! - [`RaceData`]: `.msm` race scripts (`CRaceData::LoadRaceData`,
//!   `RaceDataFile.cpp`): model, tree and attribute file names, smoke
//!   effects, shape (armor) and hair models with their skin replacements,
//!   and attaching data (collision spheres, effects, objects).
//! - [`RaceMotionData`]: `.msa` motion scripts
//!   (`CRaceMotionData::LoadMotionData`, `RaceMotionData.cpp`): duration,
//!   accumulation, combo input times, attacking data, loop data and motion
//!   events, plus the `.mss` sound script loaded alongside.
//! - [`sound`]: `.mss` sound scripts (`NSound::LoadSoundInformationPiece`,
//!   `neoeter/src/audio/Type.cpp`).
//! - The shared `NRaceData` records (`GameType.cpp`): attack, hit, collision
//!   and attaching data.
//!
//! The C++ loaders walk a [`script::Script`] with a cursor
//! (`SetChildNode`/`SetParentNode`) and some of them leave it in a child
//! node on an early `continue` or a failed sub-load; the ports keep that
//! cursor (`Cursor`) so the same files load the same way.
//!
//! What is not here: motion registration (`RegisterMotionMode`,
//! `RegisterMotionData`, motion lists — `CRaceManager`), and the effect and
//! flying-data loading the C++ starts as a side effect (`RegisterEffect`,
//! `RegisterFlyingData`); only the CRCs it computes are kept.

mod data;
mod motion;
mod race_data;
pub mod sound;

pub use data::{
    ATTACHING_DATA_TYPE_COLLISION_DATA, ATTACHING_DATA_TYPE_EFFECT, ATTACHING_DATA_TYPE_NONE,
    ATTACHING_DATA_TYPE_OBJECT, AttachingData, AttachingEffectData, AttachingObjectData, AttachingPayload, AttackData,
    COLLISION_TYPE_ATTACKING, COLLISION_TYPE_BODY, COLLISION_TYPE_DEFENDING, COLLISION_TYPE_NONE,
    COLLISION_TYPE_SPLASH, CollisionData, HitData, HitPosition, MotionAttackData, SphereData,
};
pub use motion::{
    AttackingEvent, ComboInputData, EffectEvent, EffectToTargetEvent, FlyEvent, GAME_FPS, MOTION_EVENT_TYPE_CHARACTER_HIDE,
    MOTION_EVENT_TYPE_CHARACTER_SHOW, MOTION_EVENT_TYPE_EFFECT, MOTION_EVENT_TYPE_EFFECT_TO_TARGET,
    MOTION_EVENT_TYPE_FLY, MOTION_EVENT_TYPE_NONE, MOTION_EVENT_TYPE_SCREEN_FLASHING, MOTION_EVENT_TYPE_SCREEN_WAVING,
    MOTION_EVENT_TYPE_SOUND, MOTION_EVENT_TYPE_SPECIAL_ATTACKING, MOTION_EVENT_TYPE_WARP, MotionEvent,
    MotionEventData, RaceMotionData, ScreenWavingEvent, SoundEvent, sound_script_file_name,
};
pub use race_data::{Hair, RaceData, SMOKE_NUM, Shape, Skin};

use crate::script::{Node, atof_f32};

/// `GetCaseCRC32`: CRC-32 with ASCII `A`–`Z` folded to lowercase.
pub fn case_crc32(name: &[u8]) -> u32 {
    crc32fast::hash(&name.to_ascii_lowercase())
}

/// `StringPath` (`Utils.cpp`, and `CFileNameHelper::StringPath`): `\` to
/// `/` and ASCII lowercase; other bytes unchanged.
pub fn string_path(name: &[u8]) -> Vec<u8> {
    crate::pack::normalize(name)
}

/// `CFileNameHelper::ChangeDosPath`: `/` to `\`.
pub fn change_dos_path(name: &[u8]) -> Vec<u8> {
    name.iter().map(|&b| if b == b'/' { b'\\' } else { b }).collect()
}

/// `CFileNameHelper::NoExtension`: everything before the last `.`.
pub fn no_extension(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b'.') {
        Some(dot) => &name[..dot],
        None => name,
    }
}

/// A `std::string` passed on as `c_str()`: cut at the first NUL.
fn c_str(s: &[u8]) -> &[u8] {
    match s.iter().position(|&b| b == 0) {
        Some(nul) => &s[..nul],
        None => s,
    }
}

/// A `float` stored into a `DWORD` as gcc does on x86-64: truncated to a
/// 64-bit integer (`cvttss2si`) and cut to its low 32 bits, so negative
/// values wrap and NaN or anything outside the `i64` range is 0.
pub(crate) fn float_to_dword(f: f32) -> u32 {
    const LIMIT: f32 = 9_223_372_036_854_775_808.0; // 2^63
    if f.is_nan() || !(-LIMIT..LIMIT).contains(&f) {
        return 0;
    }
    (f as i64) as u32
}

/// The `CTextFileLoader` cursor: `m_pcurNode` with its parent chain.
pub(crate) struct Cursor<'a> {
    stack: Vec<&'a Node>,
}

impl<'a> Cursor<'a> {
    /// At the `global` node (`SetTop`).
    pub(crate) fn new(root: &'a Node) -> Self {
        Self { stack: vec![root] }
    }

    fn node(&self) -> &'a Node {
        self.stack[self.stack.len() - 1]
    }

    /// `GetChildNodeCount`.
    pub(crate) fn child_count(&self) -> usize {
        self.node().children.len()
    }

    fn enter(&mut self, child: Option<&'a Node>) -> bool {
        match child {
            Some(child) => {
                self.stack.push(child);
                true
            }
            None => false,
        }
    }

    /// `SetChildNode(const char *)`; the cursor stays put on failure.
    pub(crate) fn set_child(&mut self, name: &[u8]) -> bool {
        let child = self.node().child(name);
        self.enter(child)
    }

    /// `SetChildNode(head, index)`: `head` followed by `%02u`.
    pub(crate) fn set_child_indexed(&mut self, head: &[u8], index: u32) -> bool {
        let child = self.node().child_indexed(head, index);
        self.enter(child)
    }

    /// `SetChildNode(DWORD)`: the child at `index`, whatever its name.
    pub(crate) fn set_child_at(&mut self, index: usize) -> bool {
        let child = self.node().children.get(index);
        self.enter(child)
    }

    /// `SetParentNode`; fails (and stays) at the top.
    pub(crate) fn set_parent(&mut self) -> bool {
        if self.stack.len() > 1 {
            self.stack.pop();
            true
        } else {
            false
        }
    }

    /// `GetCurrentNodeName`: `None` at the top.
    pub(crate) fn current_name(&self) -> Option<&'a [u8]> {
        (self.stack.len() > 1).then(|| self.node().name.as_slice())
    }

    /// `GetTokenVector`.
    pub(crate) fn tokens(&self, key: &str) -> Option<&'a [Vec<u8>]> {
        self.node().tokens(key)
    }

    // The getters below write `out` only on success, as C++ does, and
    // return whether they did.

    /// `GetTokenString`.
    pub(crate) fn string(&self, key: &str, out: &mut Vec<u8>) -> bool {
        set(out, self.node().token_string(key).map(<[u8]>::to_vec))
    }

    /// `GetTokenInteger` (also `GetTokenBoolean`, whose `BOOL` keeps the
    /// whole `atoi` value).
    pub(crate) fn int(&self, key: &str, out: &mut i32) -> bool {
        set(out, self.node().token_int(key))
    }

    /// `GetTokenDoubleWord`.
    pub(crate) fn dword(&self, key: &str, out: &mut u32) -> bool {
        set(out, self.node().token_dword(key))
    }

    /// `GetTokenFloat`.
    pub(crate) fn float(&self, key: &str, out: &mut f32) -> bool {
        set(out, self.node().token_float(key))
    }

    /// `GetTokenPosition` / `GetTokenVector3`: exactly three tokens.
    pub(crate) fn vector3(&self, key: &str, out: &mut [f32; 3]) -> bool {
        set(out, self.node().token_vector3(key))
    }
}

fn set<T>(out: &mut T, value: Option<T>) -> bool {
    match value {
        Some(value) => {
            *out = value;
            true
        }
        None => false,
    }
}

/// `D3DXVECTOR3(atof(a), atof(b), atof(c))`.
pub(crate) fn vector3_of(tokens: &[Vec<u8>]) -> [f32; 3] {
    [atof_f32(&tokens[0]), atof_f32(&tokens[1]), atof_f32(&tokens[2])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_to_dword_wraps_like_x86() {
        assert_eq!(float_to_dword(12.9), 12);
        assert_eq!(float_to_dword(-0.5), 0);
        assert_eq!(float_to_dword(-1.5), u32::MAX);
        assert_eq!(float_to_dword(5_000_000_000.0), 705_032_704);
        assert_eq!(float_to_dword(f32::NAN), 0);
        assert_eq!(float_to_dword(f32::INFINITY), 0);
        assert_eq!(float_to_dword(1e19), 0);
    }

    #[test]
    fn crcs_and_paths() {
        assert_eq!(case_crc32(b"D:\\Ymir Work\\A.mse"), crc32fast::hash(b"d:\\ymir work\\a.mse"));
        assert_eq!(string_path(b"D:\\Ymir Work\\\xC7A"), b"d:/ymir work/\xC7a");
        assert_eq!(change_dos_path(b"d:/a/b"), b"d:\\a\\b");
        assert_eq!(no_extension(b"a.b/c.msa"), b"a.b/c");
        assert_eq!(no_extension(b"abc"), b"abc");
        assert_eq!(c_str(b"ab\0c"), b"ab");
    }

    #[test]
    fn cursor_moves_like_the_loader() {
        let script = crate::script::parse(b"Group A\n{\n Group B\n {\n  k 1\n }\n}\nGroup C00\n{\n}\n");
        let mut cur = Cursor::new(&script.root);
        assert_eq!(cur.current_name(), None);
        assert!(!cur.set_parent());
        assert!(cur.set_child_indexed(b"c", 0));
        assert_eq!(cur.current_name(), Some(&b"c00"[..]));
        assert!(!cur.set_child(b"a"), "children of c00 only");
        assert!(cur.set_parent());
        assert!(cur.set_child_at(0));
        assert!(cur.set_child(b"b"));
        let mut k = 0;
        assert!(cur.int("k", &mut k));
        assert_eq!(k, 1);
        assert!(!cur.int("missing", &mut k));
        assert_eq!(k, 1);
        assert!(!cur.set_child_at(5));
        assert_eq!(cur.current_name(), Some(&b"b"[..]));
    }
}
