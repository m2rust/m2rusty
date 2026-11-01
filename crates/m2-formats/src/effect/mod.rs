//! Effect data: port of the loaders of `neoeter/src/effect` and of
//! `client/src/game/FlyingData.cpp`.
//!
//! - [`load_effect`]: `.mse` effect scripts (`CEffectData::LoadScript`), made
//!   of [`ParticleSystemData`], [`EffectMeshScript`] and [`LightData`]
//!   elements plus the sound script of the effect.
//! - [`load_effect_mesh`]: `.mde` binary effect meshes (`CEffectMesh`).
//! - [`load_flying_data`]: `.msf` flying objects (`CFlyingData`).
//!
//! The structs hold what the C++ objects hold after loading. Textures are
//! the file names C++ passes to `CResourceManager::GetResourcePointer`;
//! creating the images is the caller's business. Runtime state (instances,
//! decorators, rendering) is not here.
//!
//! The C++ loaders walk a [`script::Script`] with a cursor that moves even
//! when a lookup fails (`CGotoChild` always calls `SetParentNode`), and
//! later lookups depend on where it ends up. [`Cursor`] reproduces that.

mod data;
mod element;
mod flying;
mod light;
mod mesh;
mod particle;
mod sound;

pub use data::{EffectData, load_effect, parse_effect};
pub use element::{EffectPosition, ElementBase, MOVING_TYPE_BEZIER_CURVE, MOVING_TYPE_DIRECT};
pub use flying::{FlyingAttachData, FlyingData, load_flying_data, parse_flying_data};
pub use light::LightData;
pub use mesh::{
    EffectMesh, EffectMeshElement, EffectMeshFrame, EffectMeshScript, MeshData, MeshError, PtVertex, load_effect_mesh,
    parse_effect_mesh,
};
pub use particle::{EmitterProperty, ParticleProperty, ParticleSystemData};
pub use sound::{SoundInstance, load_sound_information};

use crate::pack;
use crate::script::{Node, atof_f32};

/// Why an effect or flying script did not load.
#[derive(Debug)]
pub enum Error {
    /// The file is not registered (`CEterPackManager::Get` failed).
    Pack(pack::Error),
    /// `CTextFileLoader::Load` returned false (unbalanced braces).
    Unbalanced,
    /// C++ throws: `std::vector::at` past the end (`std::out_of_range`, an
    /// odd time-event list or a truncated `TimeEventPosition`) or `resize`
    /// to a negative count (`std::length_error`). Uncaught in the client.
    Throws,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pack(e) => write!(f, "{e}"),
            Self::Unbalanced => f.write_str("unbalanced braces"),
            Self::Throws => f.write_str("the C++ loader throws on this file"),
        }
    }
}

impl std::error::Error for Error {}

impl From<pack::Error> for Error {
    fn from(e: pack::Error) -> Self {
        Self::Pack(e)
    }
}

/// `CTimeEvent<T>`: a value at a time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimeEvent<T> {
    pub time: f32,
    pub value: T,
}

/// `TTimeEventTableFloat`.
pub type TimeEventTableFloat = Vec<TimeEvent<f32>>;

/// `TTimeEventTableColor`: `DWORDCOLOR` values (`0xAARRGGBB`).
pub type TimeEventTableColor = Vec<TimeEvent<u32>>;

/// The `CTextFileLoader` node cursor (`m_pcurNode`) as a path from the root.
pub(crate) struct Cursor<'a> {
    path: Vec<&'a Node>,
}

impl<'a> Cursor<'a> {
    /// `SetTop`.
    pub(crate) fn new(root: &'a Node) -> Self {
        Self { path: vec![root] }
    }

    /// The current node.
    pub(crate) fn node(&self) -> &'a Node {
        self.path[self.path.len() - 1]
    }

    /// `GetChildNodeCount`.
    pub(crate) fn child_count(&self) -> usize {
        self.node().children.len()
    }

    /// `SetChildNode(const char *)`; stays put when there is no such child.
    pub(crate) fn enter(&mut self, name: &[u8]) -> bool {
        match self.node().child(name) {
            Some(child) => {
                self.path.push(child);
                true
            }
            None => false,
        }
    }

    /// `SetChildNode(DWORD)`.
    pub(crate) fn enter_index(&mut self, index: usize) -> bool {
        match self.node().children.get(index) {
            Some(child) => {
                self.path.push(child);
                true
            }
            None => false,
        }
    }

    /// `SetParentNode`; fails (and stays) at the root.
    pub(crate) fn leave(&mut self) -> bool {
        if self.path.len() > 1 {
            self.path.pop();
            true
        } else {
            false
        }
    }

    /// `GetCurrentNodeName`: `None` at the root.
    pub(crate) fn name(&self) -> Option<&'a [u8]> {
        (self.path.len() > 1).then(|| self.node().name.as_slice())
    }

    /// `CGotoChild` scope: enter `name` (or not), run `f`, then leave
    /// regardless, as the C++ destructor does.
    pub(crate) fn goto_child<R>(&mut self, name: &[u8], f: impl FnOnce(&mut Self) -> R) -> R {
        self.enter(name);
        let r = f(self);
        self.leave();
        r
    }

    /// `CGotoChild` by index.
    pub(crate) fn goto_child_index<R>(&mut self, index: usize, f: impl FnOnce(&mut Self) -> R) -> R {
        self.enter_index(index);
        let r = f(self);
        self.leave();
        r
    }
}

/// `GetTokenTimeEventFloat` (`Type.cpp`): `time value` pairs.
///
/// `Ok(None)` when the key is missing (C++ returns `FALSE` and leaves the
/// table alone); an odd number of tokens makes C++ throw `out_of_range`.
#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn token_time_event_float(node: &Node, key: &[u8]) -> Result<Option<TimeEventTableFloat>, Error> {
    let Some(tokens) = node.tokens(key) else {
        return Ok(None);
    };
    if tokens.len() % 2 != 0 {
        return Err(Error::Throws);
    }
    Ok(Some(
        tokens.chunks_exact(2).map(|pair| TimeEvent { time: atof_f32(&pair[0]), value: atof_f32(&pair[1]) }).collect(),
    ))
}

/// libstdc++ `std::equal_range` over `table` by time (`operator<` on
/// floats), so unsorted tables search the same elements as C++.
fn equal_range<T>(table: &[TimeEvent<T>], value: f32) -> (usize, usize) {
    let lower_bound = |mut first: usize, last: usize| {
        let mut len = last - first;
        while len > 0 {
            let half = len >> 1;
            let middle = first + half;
            if table[middle].time < value {
                first = middle + 1;
                len = len - half - 1;
            } else {
                len = half;
            }
        }
        first
    };
    let upper_bound = |mut first: usize, last: usize| {
        let mut len = last - first;
        while len > 0 {
            let half = len >> 1;
            let middle = first + half;
            if value < table[middle].time {
                len = half;
            } else {
                first = middle + 1;
                len = len - half - 1;
            }
        }
        first
    };

    let mut first = 0;
    let mut len = table.len();
    while len > 0 {
        let half = len >> 1;
        let middle = first + half;
        if table[middle].time < value {
            first = middle + 1;
            len = len - half - 1;
        } else if value < table[middle].time {
            len = half;
        } else {
            let left = lower_bound(first, middle);
            let right = upper_bound(middle + 1, first + len);
            return (left, right);
        }
    }
    (first, first)
}

/// `GetTimeEventBlendValue<float>` (`Type.h`): the value at `time`,
/// interpolated linearly between the events around it.
pub fn time_event_blend_value(time: f32, table: &[TimeEvent<f32>]) -> f32 {
    match table {
        [] => 0.0,
        [only] => only.value,
        [front, .., back] => {
            if time < front.time {
                return front.value;
            }
            if time > back.time {
                return back.value;
            }
            let (first, second) = equal_range(table, time);
            if first != second {
                return table[first].value;
            }
            let (a, b) = (&table[first - 1], &table[second]);
            let head = (b.time - time) / (b.time - a.time);
            (a.value - b.value) * head + b.value
        }
    }
}

/// `static_cast<DWORD>(float)` as gcc compiles it on x86-64: a 64-bit
/// truncating conversion (`cvttss2si`, which gives `i64::MIN` for NaN and
/// out-of-range values) whose low 32 bits are kept.
pub(crate) fn float_to_dword(f: f32) -> u32 {
    const TWO_POW_63: f32 = 9_223_372_036_854_775_808.0;
    if f.is_nan() || f.abs() >= TWO_POW_63 {
        0 // i64::MIN (or exactly -2^63), whose low half is 0
    } else {
        f as i64 as u32
    }
}

/// `D3DXCOLOR::operator DWORD` (`DirectXMathCompat.h`): `0xAARRGGBB`.
pub fn color_to_dword([r, g, b, a]: [f32; 4]) -> u32 {
    let channel = |c: f32| {
        if c >= 1.0 {
            0xff
        } else if c <= 0.0 {
            0
        } else {
            float_to_dword(c * 255.0 + 0.5)
        }
    };
    (channel(a) << 24) | (channel(r) << 16) | (channel(g) << 8) | channel(b)
}

/// `StringPath` (`Utils.cpp`): `\` to `/`, ASCII lowercase.
pub fn string_path(name: &[u8]) -> Vec<u8> {
    name.iter().map(|&b| if b == b'\\' { b'/' } else { b.to_ascii_lowercase() }).collect()
}

/// `GetOnlyPathName`: everything up to and including the last `/` or `\`.
pub fn only_path_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b'/' || b == b'\\') {
        Some(i) => &name[..=i],
        None => &[],
    }
}

/// `IsGlobalFileName`: whether the name has a `:` (a drive).
pub fn is_global_file_name(name: &[u8]) -> bool {
    name.contains(&b':')
}

/// The name a script resolves a relative file reference to: unchanged if
/// global, else prefixed by the script's directory.
fn resolve(script_name: &[u8], name: &[u8]) -> Vec<u8> {
    if is_global_file_name(name) { name.to_vec() } else { [only_path_name(script_name), name].concat() }
}

/// `GetCaseCRC32` (`CRC32.cpp`): CRC-32 of the ASCII-lowercased bytes. The
/// effect manager keys effects by it.
pub fn case_crc32(name: &[u8]) -> u32 {
    crc32fast::hash(&name.to_ascii_lowercase())
}
