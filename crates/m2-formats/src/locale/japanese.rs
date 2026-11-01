//! Port of `neoeter/src/platform/locale/Japanese.cpp`: Shift-JIS (CP932)
//! byte classes and the case-insensitive compare `Locale.cpp` exposes.

/// `MACRO_IsLeadByte`.
fn is_lead(c: u16) -> bool {
    (0x81..=0x9f).contains(&c) || (0xe0..=0xfc).contains(&c)
}

/// `ShiftJIS_IsLeadByte`.
pub fn is_lead_byte(b: u8) -> bool {
    is_lead(u16::from(b))
}

/// `ShiftJIS_IsTrailByte`.
pub fn is_trail_byte(b: u8) -> bool {
    (0x40..=0x7e).contains(&b) || (0x80..=0xfc).contains(&b)
}

/// `ShiftJIS_StringCompareCI`: compares up to `length` characters, ASCII
/// case-insensitively, double-byte characters as 16-bit values. Returns -1,
/// 0 or 1. The strings end at their first NUL or at the end of the slice.
///
/// Mirrors the C++ asymmetry: a double-byte character counts against
/// `length` only on the right-hand side.
pub fn string_compare_ci(left: &[u8], right: &[u8], mut length: usize) -> i32 {
    let at = |s: &[u8], i: usize| u16::from(s.get(i).copied().unwrap_or(0));
    let (mut i1, mut i2) = (0, 0);

    while length > 0 {
        length -= 1;
        let mut c1 = at(left, i1);
        i1 += 1;

        // The C++ `goto test` paths break out with c2 directly.
        let c2 = 'test: {
            if is_lead(c1) {
                if length == 0 {
                    c1 = 0;
                    let b = at(right, i2);
                    break 'test if is_lead(b) { 0 } else { b };
                }
                if at(left, i1) == 0 {
                    c1 = 0;
                } else {
                    c1 = (c1 << 8) | at(left, i1);
                    i1 += 1;
                }
            } else {
                c1 = ascii_lower(c1);
            }

            let b = at(right, i2);
            i2 += 1;
            if !is_lead(b) {
                break 'test ascii_lower(b);
            }
            if length == 0 {
                break 'test 0;
            }
            length -= 1;
            if at(right, i2) == 0 {
                0
            } else {
                let c = (b << 8) | at(right, i2);
                i2 += 1;
                c
            }
        };

        if c1 != c2 {
            return if c1 > c2 { 1 } else { -1 };
        }
        if c1 == 0 {
            return 0;
        }
    }
    0
}

/// `tolower` in the C locale.
fn ascii_lower(c: u16) -> u16 {
    if (0x41..=0x5a).contains(&c) { c + 0x20 } else { c }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare() {
        assert_eq!(string_compare_ci(b"Item", b"iTEM", 10), 0);
        assert_eq!(string_compare_ci(b"a", b"b", 1), -1);
        assert_eq!(string_compare_ci(b"b", b"a", 0), 0);
        assert_eq!(string_compare_ci(b"\x82\xa0", b"\x82\xa0", 2), 0);
        // A lead byte as the last counted character compares as 0.
        assert_eq!(string_compare_ci(b"\x82\xa0", b"x", 1), -1);
        assert_eq!(string_compare_ci(b"\x82\xa0", b"\x82\xa1", 1), 0);
    }
}
