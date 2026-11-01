//! The client's data tables, as `LoadLocaleData`
//! (`client/src/ui/PythonApplication.cpp`) loads them: item list, item proto
//! and item descriptions (`CItemManager`), mob proto (`CPythonNonPlayer`),
//! skill descriptions and skill table (`CPythonSkill`), the insult list and
//! the empire text conversion tables (`CPythonNetworkStream`).
//!
//! Each loader fills a plain struct the way the C++ manager fills its maps;
//! the managers' gameplay methods are not ported. Binary records are decoded
//! field by field from little-endian bytes, mirroring the packed C++ structs.

mod item;
mod locale;
mod mob;
mod network;
mod skill;

use std::fmt;

pub use item::{
    EMPTY_BOWL_VNUM, ITEM_TABLE_SIZE, ItemApply, ItemData, ItemLimit, ItemTable, ItemTables, item_name_hash,
    parse_item_proto,
};
pub use locale::{LocaleData, Step, load_locale_data, load_locale_files};
pub use mob::{MOB_TABLE_SIZE, MobSkillLevel, MobTable, MobTables, parse_mob_proto};
pub use network::{TextConvertTable, parse_insult_list};
pub use skill::{AffectData, GradeData, SkillData, SkillTables, SkillType};

use crate::{crypt, pack};

#[derive(Debug)]
pub enum Error {
    /// The file is not in the packs.
    Pack(pack::Error),
    /// The file ends before its header or its records do.
    Truncated,
    /// Unknown file type.
    BadFourCC(u32),
    /// `MIPX` item proto of another version than 1.
    BadVersion(u32),
    /// `MIPX` item proto whose record size is not `sizeof(TItemTable)`.
    BadStride(u32),
    /// The `CLZObject` inside did not decode.
    Lz(crypt::Error),
    /// Mob proto whose decompressed size is not a multiple of the record.
    BadSize(u32),
    /// A skill description with more motion grades than
    /// `SKILL_EFFECT_COUNT` (`RegisterSkillDesc` stops there).
    GradeLimit { skill: u32, grades: i32 },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Pack(e) => write!(f, "{e}"),
            Error::Truncated => write!(f, "truncated"),
            Error::BadFourCC(c) => write!(f, "unknown fourcc {c:08x}"),
            Error::BadVersion(v) => write!(f, "unsupported version {v}"),
            Error::BadStride(s) => write!(f, "record size {s}, expected {ITEM_TABLE_SIZE}"),
            Error::Lz(e) => write!(f, "{e}"),
            Error::BadSize(s) => write!(f, "decompressed size {s} is not a multiple of {MOB_TABLE_SIZE}"),
            Error::GradeLimit { skill, grades } => write!(f, "skill {skill}: {grades} grades exceed the limit"),
        }
    }
}

impl std::error::Error for Error {}

impl From<crypt::Error> for Error {
    fn from(e: crypt::Error) -> Self {
        Error::Lz(e)
    }
}

impl From<pack::Error> for Error {
    fn from(e: pack::Error) -> Self {
        Error::Pack(e)
    }
}

/// Little-endian reader over one record. Callers check the length first.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn bytes<const N: usize>(&mut self) -> [u8; N] {
        let out = self.buf[self.pos..self.pos + N].try_into().unwrap();
        self.pos += N;
        out
    }

    fn u8(&mut self) -> u8 {
        self.bytes::<1>()[0]
    }

    fn i8(&mut self) -> i8 {
        self.u8() as i8
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.bytes())
    }

    fn i16(&mut self) -> i16 {
        i16::from_le_bytes(self.bytes())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes())
    }

    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.bytes())
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.bytes())
    }
}

/// Reads the little-endian word at `at`, if the buffer has it.
fn le32(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(at..at + 4)?.try_into().unwrap()))
}

/// `atoi` as glibc implements it on LP64: leading whitespace, an optional
/// sign and digits, accumulated as a `long` that saturates on overflow, then
/// truncated to `int`.
pub fn atoi(s: &[u8]) -> i32 {
    let mut i = 0;
    while i < s.len() && matches!(s[i], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        i += 1;
    }
    let negative = match s.get(i) {
        Some(b'-') => {
            i += 1;
            true
        }
        Some(b'+') => {
            i += 1;
            false
        }
        _ => false,
    };
    // Magnitude, saturating one past i64::MAX so that -2^63 stays exact.
    let mut magnitude: u64 = 0;
    while let Some(&c) = s.get(i).filter(|c| c.is_ascii_digit()) {
        magnitude = magnitude.saturating_mul(10).saturating_add(u64::from(c - b'0'));
        i += 1;
    }
    let value: i64 = if negative {
        if magnitude > i64::MAX as u64 {
            i64::MIN
        } else {
            -(magnitude as i64)
        }
    } else {
        magnitude.min(i64::MAX as u64) as i64
    };
    value as i32
}

/// The bytes before the first NUL, as `std::string(const char *)` sees them.
pub fn cstr(s: &[u8]) -> &[u8] {
    s.iter().position(|&b| b == 0).map_or(s, |n| &s[..n])
}

/// `CMemoryTextFileLoader::SplitLine` (and the identical free `SplitLine` in
/// `PythonSkill.cpp`): tokens separated by runs of any byte in `delims`; a
/// token starting with `"` runs to the next `"`.
///
/// Returns the tokens and whether the line split cleanly. On failure (no
/// token at all, or an unterminated quote) the tokens found so far are still
/// returned, as C++ leaves them in the vector.
pub fn split_line<'a>(line: &'a [u8], delims: &[u8]) -> (Vec<&'a [u8]>, bool) {
    let is_delim = |b: &u8| delims.contains(b);
    let first_not_delim = |from: usize| line.get(from..)?.iter().position(|b| !is_delim(b)).map(|p| from + p);

    let mut tokens = Vec::new();
    let mut base = 0usize;
    loop {
        let Some(mut begin) = first_not_delim(base) else {
            return (tokens, false);
        };
        let end;
        if line[begin] == b'"' {
            begin += 1;
            let Some(close) = line[begin..].iter().position(|&b| b == b'"') else {
                return (tokens, false);
            };
            end = begin + close;
            base = end + 1;
        } else {
            // No delimiter after the token: it runs to the end of the line,
            // and the next search starts at npos.
            end = line[begin..]
                .iter()
                .position(is_delim)
                .map_or(line.len(), |p| begin + p);
            base = if end == line.len() { usize::MAX } else { end };
        }
        tokens.push(&line[begin..end]);

        // "Check for a trailing delimiter" - [levites]
        if first_not_delim(base).is_none() || base >= line.len() {
            return (tokens, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_like_glibc() {
        assert_eq!(atoi(b"123"), 123);
        assert_eq!(atoi(b"  -42abc"), -42);
        assert_eq!(atoi(b"\t+7"), 7);
        assert_eq!(atoi(b""), 0);
        assert_eq!(atoi(b"x1"), 0);
        assert_eq!(atoi(b"- 1"), 0);
        // long arithmetic truncated to int.
        assert_eq!(atoi(b"4294967295"), -1);
        assert_eq!(atoi(b"4294967296"), 0);
        assert_eq!(atoi(b"2147483648"), i32::MIN);
        // strtol saturates at LONG_MAX / LONG_MIN.
        assert_eq!(atoi(b"99999999999999999999"), -1);
        assert_eq!(atoi(b"-99999999999999999999"), 0);
        assert_eq!(atoi(b"-9223372036854775808"), 0);
    }

    #[test]
    fn split_line_tokens() {
        let t = |s: &'static [u8]| split_line(s, b"\t");
        assert_eq!(t(b"1\tA\ticon.tga"), (vec![&b"1"[..], b"A", b"icon.tga"], true));
        // Runs of delimiters are one separator; leading/trailing ones vanish.
        assert_eq!(t(b"\t\t1\t\t2\t"), (vec![&b"1"[..], b"2"], true));
        assert_eq!(t(b""), (vec![], false));
        assert_eq!(t(b"\t\t"), (vec![], false));
        // Quotes keep delimiters; text right after the closing quote starts
        // the next token.
        assert_eq!(t(b"\"a\tb\"c\td"), (vec![&b"a\tb"[..], b"c", b"d"], true));
        assert_eq!(t(b"\"\""), (vec![&b""[..]], true));
        assert_eq!(t(b"x\t\"open"), (vec![&b"x"[..]], false));
        assert_eq!(split_line(b"A|B||C", b"|"), (vec![&b"A"[..], b"B", b"C"], true));
    }

    #[test]
    fn c_strings() {
        assert_eq!(cstr(b"ab\0cd"), b"ab");
        assert_eq!(cstr(b"abc"), b"abc");
    }
}
