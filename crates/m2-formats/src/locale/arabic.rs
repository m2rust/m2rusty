//! Port of `neoeter/src/platform/locale/Arabic.cpp`: contextual shaping of
//! Arabic letters into presentation forms, used by the text renderer
//! (`GrpTextInstance.cpp`). Characters are `wchar_t` code units (`u32`).
//!
//! `Arabic_ConvEnglishModeSymbol` is declared in `Arabic.h` but defined and
//! used nowhere, so it is not ported.

const BASE: u32 = 0x0621;
const LAST: u32 = 0x064a;

// ARABIC_FORM_TYPE columns.
const ISOLATED: usize = 1;
const INITIAL: usize = 2;
const MEDIAL: usize = 3;
const FINAL: usize = 4;

/// `Arabic_GetMap` tables, indexed by `code - 0x0621`: debug, isolated,
/// initial, medial, final. U+063B..U+063F have no entry (all zero).
#[rustfmt::skip]
const MAP: [[u16; 5]; 42] = [
    [0x0621, 0xfe80, 0, 0, 0],                // HAMZA
    [0x0622, 0xfe81, 0, 0, 0xfe82],           // ALEF_MADDA
    [0x0623, 0xfe83, 0, 0, 0xfe84],           // ALEF_HAMZA_ABOVE
    [0x0624, 0xfe85, 0, 0, 0xfe86],           // WAW_HAMZA
    [0x0625, 0xfe87, 0, 0, 0xfe88],           // ALEF_HAMZA_BELOW
    [0x0626, 0xfe89, 0xfe8b, 0xfe8c, 0xfe8a], // YEH_HAMZA
    [0x0627, 0xfe8d, 0, 0, 0xfe8e],           // ALEF
    [0x0628, 0xfe8f, 0xfe91, 0xfe92, 0xfe90], // BEH
    [0x0629, 0xfe93, 0, 0, 0xfe94],           // TEH_MARBUTA
    [0x062a, 0xfe95, 0xfe97, 0xfe98, 0xfe96], // TEH
    [0x062b, 0xfe99, 0xfe9b, 0xfe9c, 0xfe9a], // THEH
    [0x062c, 0xfe9d, 0xfe9f, 0xfea0, 0xfe9e], // JEEM
    [0x062d, 0xfea1, 0xfea3, 0xfea4, 0xfea2], // HAH
    [0x062e, 0xfea5, 0xfea7, 0xfea8, 0xfea6], // KHAH
    [0x062f, 0xfea9, 0, 0, 0xfeaa],           // DAL
    [0x0630, 0xfeab, 0, 0, 0xfeac],           // THAL
    [0x0631, 0xfead, 0, 0, 0xfeae],           // REH
    [0x0632, 0xfeaf, 0, 0, 0xfeb0],           // ZAIN
    [0x0633, 0xfeb1, 0xfeb3, 0xfeb4, 0xfeb2], // SEEN
    [0x0634, 0xfeb5, 0xfeb7, 0xfeb8, 0xfeb6], // SHEEN
    [0x0635, 0xfeb9, 0xfebb, 0xfebc, 0xfeba], // SAD
    [0x0636, 0xfebd, 0xfebf, 0xfec0, 0xfebe], // DAD
    [0x0637, 0xfec1, 0xfec3, 0xfec4, 0xfec2], // TAH
    [0x0638, 0xfec5, 0xfec7, 0xfec8, 0xfec6], // ZAH
    [0x0639, 0xfec9, 0xfecb, 0xfecc, 0xfeca], // AIN
    [0x063a, 0xfecd, 0xfecf, 0xfed0, 0xfece], // GHAIN
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0],
    [0x0640, 0x0640, 0, 0, 0],                // TATWEEL
    [0x0641, 0xfed1, 0xfed3, 0xfed4, 0xfed2], // FEH
    [0x0642, 0xfed5, 0xfed7, 0xfed8, 0xfed6], // QAF
    [0x0643, 0xfed9, 0xfedb, 0xfedc, 0xfeda], // KAF
    [0x0644, 0xfedd, 0xfedf, 0xfee0, 0xfede], // LAM
    [0x0645, 0xfee1, 0xfee3, 0xfee4, 0xfee2], // MEEM
    [0x0646, 0xfee5, 0xfee7, 0xfee8, 0xfee6], // NOON
    [0x0647, 0xfee9, 0xfeeb, 0xfeec, 0xfeea], // HEH
    [0x0648, 0xfeed, 0, 0, 0xfeee],           // WAW
    [0x0649, 0xfeef, 0, 0, 0xfef0],           // ALEF_MAKSURA
    [0x064a, 0xfef1, 0xfef3, 0xfef4, 0xfef2], // YEH
];

/// `Arabic_IsInSpace`.
pub fn is_in_space(code: u32) -> bool {
    code == u32::from(b' ') || code == u32::from(b'\t')
}

/// `Arabic_IsInSymbol`: ASCII punctuation and the space.
pub fn is_in_symbol(code: u32) -> bool {
    matches!(code, 0x20..=0x2f | 0x3a..=0x40 | 0x5b..=0x60 | 0x7b..=0x7e)
}

/// `Arabic_IsInPresentation`: the presentation form blocks, and 0x08.
pub fn is_in_presentation(code: u32) -> bool {
    matches!(code, 0xfb50..=0xfdff | 0xfe70..=0xfeff | 0x08)
}

/// `Arabic_HasPresentation`: whether the last non-space character in
/// `codes[1..=last]` is a presentation form (`codes[0]` is never looked at).
pub fn has_presentation(codes: &[u32], mut last: i32) -> bool {
    while last > 0 {
        let code = codes[last as usize];
        if !is_in_space(code) {
            return is_in_presentation(code);
        }
        last -= 1;
    }
    false
}

/// `Arabic_ConvSymbol`: mirrors brackets for right-to-left drawing.
pub fn conv_symbol(c: u32) -> u32 {
    match char::from_u32(c) {
        Some('(') => ')'.into(),
        Some(')') => '('.into(),
        Some('<') => '>'.into(),
        Some('>') => '<'.into(),
        Some('{') => '}'.into(),
        Some('}') => '{'.into(),
        Some('[') => ']'.into(),
        Some(']') => '['.into(),
        _ => c,
    }
}

/// `Arabic_GetMap`.
fn get_map(code: u32, pos: usize) -> u32 {
    if is_in_map(code) { u32::from(MAP[(code - BASE) as usize][pos]) } else { 0 }
}

/// `Arabic_GetComposition`: the LAM-ALEF ligatures.
fn get_composition(cur: u32, next: u32, pos: usize) -> u32 {
    let forms: [u32; 5] = match (cur, next) {
        (0x0644, 0x0622) => [0x0622, 0xfef5, 0, 0, 0xfef6],
        (0x0644, 0x0623) => [0x0623, 0xfef7, 0, 0, 0xfef8],
        (0x0644, 0x0625) => [0x0625, 0xfef9, 0, 0, 0xfefa],
        (0x0644, 0x0627) => [0x0627, 0xfefb, 0, 0, 0xfefc],
        _ => return 0,
    };
    forms[pos]
}

/// `Arabic_IsInMap`.
fn is_in_map(code: u32) -> bool {
    (BASE..=LAST).contains(&code)
}

/// `Arabic_IsInComposing`: harakat and Quranic marks.
fn is_in_composing(code: u32) -> bool {
    matches!(code, 0x064b..=0x0655 | 0x0670 | 0x06d6..=0x06dc | 0x06df..=0x06e4 | 0x06e7 | 0x06e8 | 0x06ea..=0x06ed)
}

/// `Arabic_MakeShape`: replaces each Arabic letter by its isolated, initial,
/// medial or final form, and LAM + ALEF by a ligature. Never longer than
/// `src`.
///
/// Mirrors the C++ quirks: after a ligature only the next index is skipped,
/// even when marks stood between LAM and ALEF; letters without forms
/// (U+063B..U+063F) become 0.
pub fn make_shape(src: &[u32]) -> Vec<u32> {
    let mut dst = vec![0; src.len()];
    let last_index = src.len().wrapping_sub(1);
    let mut d = 0;
    let mut s = 0;

    while s < src.len() {
        let cur = src[s];
        if is_in_map(cur) {
            let mut prev = 0;
            let mut i = s;
            while i > 0 {
                i -= 1;
                prev = src[i];
                if !is_in_composing(prev) {
                    break;
                }
            }
            if s == 0 || !is_in_map(prev) || (get_map(prev, INITIAL) == 0 && get_map(prev, MEDIAL) == 0) {
                prev = 0;
            }

            let mut next = 0;
            let mut i = s;
            while i < last_index {
                i += 1;
                next = src[i];
                if !is_in_composing(next) {
                    break;
                }
            }
            if !is_in_map(next) || (get_map(next, MEDIAL) == 0 && get_map(next, FINAL) == 0 && next != 0x0640) {
                next = 0;
            }

            let medial = get_map(cur, MEDIAL);
            let final_ = get_map(cur, FINAL);
            let initial = get_map(cur, INITIAL);
            dst[d] = if cur == 0x0644 && matches!(next, 0x0622 | 0x0623 | 0x0625 | 0x0627) {
                s += 1;
                get_composition(cur, next, if prev != 0 { FINAL } else { ISOLATED })
            } else if prev != 0 && next != 0 && medial != 0 {
                medial
            } else if prev != 0 && final_ != 0 {
                final_
            } else if next != 0 && initial != 0 {
                initial
            } else {
                get_map(cur, ISOLATED)
            };
        } else {
            dst[d] = cur;
        }
        d += 1;
        s += 1;
    }
    dst.truncate(d);
    dst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        // BEH alone, then BEH BEH BEH: initial, medial, final.
        assert_eq!(make_shape(&[0x0628]), [0xfe8f]);
        assert_eq!(make_shape(&[0x0628, 0x0628, 0x0628]), [0xfe91, 0xfe92, 0xfe90]);
        // LAM ALEF ligature, isolated and after a joining letter.
        assert_eq!(make_shape(&[0x0644, 0x0627]), [0xfefb]);
        assert_eq!(make_shape(&[0x0628, 0x0644, 0x0627]), [0xfe91, 0xfefc]);
        // No forms: zero.
        assert_eq!(make_shape(&[0x063b]), [0]);
        assert!(make_shape(&[]).is_empty());
    }

    #[test]
    fn presentation() {
        assert!(has_presentation(&[0, 0xfe8f, 0x20], 2));
        assert!(!has_presentation(&[0xfe8f], 0));
        assert!(!has_presentation(&[], -1));
        assert_eq!(conv_symbol(u32::from(b'(')), u32::from(b')'));
    }
}
