//! `AtlasInfo.txt`: the maps the client knows and where they sit in the
//! world (`CMapManager::__LoadMapInfoVector`, `client/src/game/MapManager.cpp`;
//! the client reads `<locale path>/AtlasInfo.txt`, else `AtlasInfo.txt`).
//!
//! ```text
//! metin2_map_a1  409600 896000 4 5      (name, base x, base y, terrains x, y)
//! ```

use crate::text;

use super::TERRAIN_WORLD_SIZE;

/// `CMapManager::TMapInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtlasEntry {
    pub name: Vec<u8>,
    pub base_x: u32,
    pub base_y: u32,
    /// Size in terrains.
    pub size_x: u32,
    pub size_y: u32,
    /// `base + size * TERRAIN_XSIZE` (25600), in `DWORD` arithmetic.
    pub end_x: u32,
    pub end_y: u32,
}

fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn skip_space(s: &[u8]) -> &[u8] {
    &s[s.iter().position(|&b| !is_space(b)).unwrap_or(s.len())..]
}

/// `%d`: white space, an optional sign and at least one digit; the value is
/// `strtol`'s, truncated to `int` (as glibc stores it).
fn scan_int(s: &[u8]) -> Option<(i32, &[u8])> {
    let s = skip_space(s);
    let sign = usize::from(matches!(s.first(), Some(b'+' | b'-')));
    let digits = s[sign..].iter().take_while(|b| b.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let end = sign + digits;
    Some((crate::script::atoi(&s[..end]), &s[end..]))
}

/// `sscanf(line, "%s %d %d %d %d", name, &x, &y, &w, &h)` into variables
/// that keep their previous values when a conversion fails.
fn scan_line(line: &[u8], name: &mut Vec<u8>, values: &mut [i32; 4]) {
    let s = skip_space(line);
    if s.is_empty() {
        return;
    }
    let len = s.iter().position(|&b| is_space(b)).unwrap_or(s.len());
    *name = s[..len].to_vec();
    let mut rest = &s[len..];
    for value in values.iter_mut() {
        let Some((v, r)) = scan_int(rest) else {
            return;
        };
        *value = v;
        rest = r;
    }
}

/// `CMapManager::__LoadMapInfoVector` after reading the file.
///
/// The `sscanf` targets are not reset per line, so a line that does not
/// scan (an empty one, such as the one after the final newline) repeats the
/// previous entry, and a short line keeps the previous line's trailing
/// numbers. Mirrored. Lines are cut at a NUL byte (`c_str()`).
///
/// Deviation: before the first scanned name C++ reads an uninitialised
/// buffer; here it is empty, so such lines are skipped.
pub fn parse_atlas_info(data: &[u8]) -> Vec<AtlasEntry> {
    let mut name = Vec::new();
    let mut values = [0i32; 4];
    let mut entries = Vec::new();
    for line in text::lines(data) {
        let line = &line[..line.iter().position(|&b| b == 0).unwrap_or(line.len())];
        scan_line(line, &mut name, &mut values);
        if name.is_empty() {
            continue;
        }
        let [x, y, w, h] = values.map(|v| v as u32);
        entries.push(AtlasEntry {
            name: name.clone(),
            base_x: x,
            base_y: y,
            size_x: w,
            size_y: h,
            end_x: x.wrapping_add(w.wrapping_mul(TERRAIN_WORLD_SIZE)),
            end_y: y.wrapping_add(h.wrapping_mul(TERRAIN_WORLD_SIZE)),
        });
    }
    entries
}
