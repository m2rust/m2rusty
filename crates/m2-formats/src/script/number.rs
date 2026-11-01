//! `atoi` and `atof` as glibc implements them in the "C" locale, which is
//! what the reference build runs. Every numeric `GetToken*` accessor, and
//! the callers that convert tokens themselves, go through these two.

/// `isspace` in the "C" locale.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Skips leading white space and an optional sign; returns the rest and
/// whether the sign was `-`.
fn sign(s: &[u8]) -> (&[u8], bool) {
    let s = &s[s.iter().position(|&b| !is_space(b)).unwrap_or(s.len())..];
    match s.first() {
        Some(b'-') => (&s[1..], true),
        Some(b'+') => (&s[1..], false),
        _ => (s, false),
    }
}

/// `atoi`: `(int)strtol(s, NULL, 10)`. Leading white space, an optional
/// sign, then decimal digits up to the first other byte; no digits is 0.
/// `strtol` saturates at the 64-bit `long` range and the cast keeps the low
/// 32 bits, so `"4294967297"` is 1 and anything past `i64::MAX` is -1.
pub fn atoi(s: &[u8]) -> i32 {
    let (digits, negative) = sign(s);
    let mut value: i64 = 0;
    for &b in digits.iter().take_while(|b| b.is_ascii_digit()) {
        let d = i64::from(b - b'0');
        value = if negative {
            value.saturating_mul(10).saturating_sub(d)
        } else {
            value.saturating_mul(10).saturating_add(d)
        };
    }
    value as i32
}

/// `atof`: `strtod(s, NULL)`, correctly rounded like glibc. Accepts leading
/// white space, a sign, then `inf`/`infinity`/`nan` (any case, `nan` with
/// an optional `(chars)` suffix), a hex float (`0x1.8p3`), or a decimal
/// with optional fraction and exponent. The longest valid prefix is used;
/// no number at all is `+0.0`.
///
/// Deviation: glibc puts the number in `nan(123)` into the NaN payload;
/// here every NaN is the default quiet NaN (with the sign applied).
pub fn atof(s: &[u8]) -> f64 {
    let (rest, negative) = sign(s);
    let Some(value) = unsigned_float(rest) else {
        return 0.0;
    };
    if negative { -value } else { value }
}

/// `(float)atof(s)`, which is what `GetTokenFloat` and most callers store.
pub fn atof_f32(s: &[u8]) -> f32 {
    to_f32(atof(s))
}

/// The double-to-float conversion of x86 (`cvtsd2ss`): round to nearest
/// even, and NaNs keep their sign and the top of their payload.
pub fn to_f32(d: f64) -> f32 {
    if d.is_nan() {
        let bits = d.to_bits();
        let sign = ((bits >> 63) as u32) << 31;
        let payload = ((bits >> 29) & 0x007f_ffff) as u32;
        return f32::from_bits(sign | 0x7fc0_0000 | payload);
    }
    d as f32
}

fn starts_with_ignore_case(s: &[u8], prefix: &[u8]) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn unsigned_float(s: &[u8]) -> Option<f64> {
    if starts_with_ignore_case(s, b"inf") {
        return Some(f64::INFINITY);
    }
    if starts_with_ignore_case(s, b"nan") {
        return Some(f64::NAN);
    }
    if s.len() >= 2 && s[0] == b'0' && (s[1] | 0x20) == b'x' {
        if let Some(value) = hex_float(&s[2..]) {
            return Some(value);
        }
        // "0x" without hex digits: just the "0".
        return Some(0.0);
    }
    decimal_float(s)
}

fn digits_len(s: &[u8], pred: impl Fn(&u8) -> bool) -> usize {
    s.iter().take_while(|b| pred(b)).count()
}

/// The length of an exponent suffix (`e-12`, `p+3`) at `s`, or 0 when the
/// marker is not followed by digits and so is not consumed.
fn exponent_len(s: &[u8], marker: u8) -> usize {
    if s.first().map(|b| b | 0x20) != Some(marker) {
        return 0;
    }
    let sign = usize::from(matches!(s.get(1), Some(b'+' | b'-')));
    let digits = digits_len(&s[1 + sign..], u8::is_ascii_digit);
    if digits == 0 { 0 } else { 1 + sign + digits }
}

fn decimal_float(s: &[u8]) -> Option<f64> {
    let int = digits_len(s, u8::is_ascii_digit);
    let mut len = int;
    let mut frac = 0;
    if s.get(len) == Some(&b'.') {
        frac = digits_len(&s[len + 1..], u8::is_ascii_digit);
        len += 1 + frac;
    }
    if int + frac == 0 {
        return None;
    }
    len += exponent_len(&s[len..], b'e');
    // The prefix is ASCII digits, '.', 'e'/'E' and a sign, which Rust's
    // (correctly rounded) parser accepts in this shape.
    let text = std::str::from_utf8(&s[..len]).ok()?;
    text.parse().ok()
}

/// The hex digits after `0x`, rounded to nearest even like glibc.
fn hex_float(s: &[u8]) -> Option<f64> {
    let int = digits_len(s, u8::is_ascii_hexdigit);
    let mut frac = 0;
    if s.get(int) == Some(&b'.') {
        frac = digits_len(&s[int + 1..], u8::is_ascii_hexdigit);
    }
    if int + frac == 0 {
        return None;
    }

    // value = mantissa * 2^exp2, with bits shifted out kept as `sticky`.
    let mut mantissa: u64 = 0;
    let mut exp2: i64 = 0;
    let mut sticky = false;
    let digits = s[..int].iter().map(|&b| (b, false));
    let frac_digits = s.get(int + 1..int + 1 + frac).unwrap_or(&[]).iter().map(|&b| (b, true));
    for (b, is_frac) in digits.chain(frac_digits) {
        let d = u64::from((b as char).to_digit(16).unwrap_or(0));
        if mantissa >> 60 == 0 {
            mantissa = mantissa << 4 | d;
            if is_frac {
                exp2 -= 4;
            }
        } else {
            sticky |= d != 0;
            if !is_frac {
                exp2 += 4;
            }
        }
    }

    let len = int + if s.get(int) == Some(&b'.') { 1 + frac } else { 0 };
    let exp_len = exponent_len(&s[len..], b'p');
    if exp_len > 0 {
        let e = &s[len + 1..len + exp_len];
        let (e, negative) = match e[0] {
            b'-' => (&e[1..], true),
            b'+' => (&e[1..], false),
            _ => (e, false),
        };
        let value = e
            .iter()
            .fold(0i64, |v, &b| v.saturating_mul(10).saturating_add(i64::from(b - b'0')))
            .min(1 << 40);
        exp2 += if negative { -value } else { value };
    }

    if mantissa == 0 {
        return Some(0.0);
    }
    Some(compose(mantissa, exp2, sticky))
}

/// `mantissa * 2^exp2` (plus a nonzero tail if `sticky`) as a double.
fn compose(mantissa: u64, exp2: i64, sticky: bool) -> f64 {
    let top = 63 - i64::from(mantissa.leading_zeros()); // bit index of the leading 1
    let exponent = exp2 + top; // unbiased exponent of the value
    if exponent > 1023 {
        return f64::INFINITY;
    }
    // Number of low bits of `mantissa` to drop: keep 53 significant bits,
    // fewer for subnormals (whose lowest bit is 2^-1074).
    let shift = (top - 52).max(-1074 - exp2);
    if shift <= 0 {
        // Exact. `shift` > -64 here because `top - 52` >= -52.
        let m = mantissa << (-shift);
        return from_parts(m, exp2 + shift);
    }
    if shift > 64 {
        return 0.0; // far below half the smallest subnormal
    }
    let m = if shift == 64 { 0 } else { mantissa >> shift };
    let rest = if shift == 64 { mantissa } else { mantissa & ((1u64 << shift) - 1) };
    let half = 1u64 << (shift - 1);
    let round_up = rest > half || (rest == half && (sticky || m & 1 == 1));
    from_parts(m + u64::from(round_up), exp2 + shift)
}

/// `m * 2^e` where the product is representable (`m` < 2^54).
fn from_parts(mut m: u64, mut e: i64) -> f64 {
    if m == 0 {
        return 0.0;
    }
    if m >> 53 != 0 {
        m >>= 1; // rounding carried to 2^53; the dropped bit is 0
        e += 1;
    }
    let top = 63 - i64::from(m.leading_zeros());
    let exponent = e + top;
    if exponent > 1023 {
        return f64::INFINITY;
    }
    if exponent < -1022 {
        // Subnormal: e == -1074 by construction.
        return f64::from_bits(m);
    }
    let m = m << (52 - top);
    f64::from_bits(((exponent + 1023) as u64) << 52 | (m & ((1 << 52) - 1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atoi_like_glibc() {
        assert_eq!(atoi(b"42"), 42);
        assert_eq!(atoi(b"  -17abc"), -17);
        assert_eq!(atoi(b"\x0b+5"), 5);
        assert_eq!(atoi(b"1.9"), 1);
        assert_eq!(atoi(b"x1"), 0);
        assert_eq!(atoi(b"- 1"), 0);
        assert_eq!(atoi(b""), 0);
        assert_eq!(atoi(b"4294967297"), 1);
        assert_eq!(atoi(b"2147483648"), i32::MIN);
        assert_eq!(atoi(b"99999999999999999999"), -1);
        assert_eq!(atoi(b"-99999999999999999999"), 0);
    }

    #[test]
    fn atof_decimal() {
        assert_eq!(atof(b"1.5"), 1.5);
        assert_eq!(atof(b" -0.25xyz"), -0.25);
        assert_eq!(atof(b".5"), 0.5);
        assert_eq!(atof(b"5."), 5.0);
        assert_eq!(atof(b"1e3"), 1000.0);
        assert_eq!(atof(b"1e"), 1.0);
        assert_eq!(atof(b"1e+"), 1.0);
        assert_eq!(atof(b"2E-1"), 0.2);
        assert_eq!(atof(b"."), 0.0);
        assert_eq!(atof(b"-").to_bits(), 0);
        assert_eq!(atof(b"-0").to_bits(), (-0.0f64).to_bits());
        assert_eq!(atof(b"1e400"), f64::INFINITY);
        assert_eq!(atof(b"1e-400"), 0.0);
        assert_eq!(atof(b"0.1"), 0.1);
    }

    #[test]
    fn atof_special() {
        assert_eq!(atof(b"inf"), f64::INFINITY);
        assert_eq!(atof(b"-Infinity"), f64::NEG_INFINITY);
        assert!(atof(b"NaN").is_nan());
        assert_eq!(atof(b"-nan").to_bits(), 0xfff8_0000_0000_0000);
        assert_eq!(atof_f32(b"-nan").to_bits(), 0xffc0_0000);
        assert_eq!(atof(b"in"), 0.0);
    }

    #[test]
    fn atof_hex() {
        assert_eq!(atof(b"0x10"), 16.0);
        assert_eq!(atof(b"0x1.8p1"), 3.0);
        assert_eq!(atof(b"0x.8"), 0.5);
        assert_eq!(atof(b"-0x1p-2"), -0.25);
        assert_eq!(atof(b"0x"), 0.0);
        assert_eq!(atof(b"0xg"), 0.0);
        assert_eq!(atof(b"0x1p"), 1.0);
        assert_eq!(atof(b"0x1p-1074"), f64::from_bits(1));
        assert_eq!(atof(b"0x1p-1075"), 0.0); // tie, rounds to even
        assert_eq!(atof(b"0x1.0000000000001p-1075"), f64::from_bits(1));
        assert_eq!(atof(b"0x1p1024"), f64::INFINITY);
        // 2^53 + 1 is a tie between 2^53 and 2^53 + 2; even wins.
        assert_eq!(atof(b"0x20000000000001"), 9007199254740992.0);
        assert_eq!(atof(b"0x20000000000003"), 9007199254740996.0);
        // Digits beyond 64 bits only matter through the sticky bit.
        assert_eq!(atof(b"0x200000000000010000001"), atof(b"0x20000000000002") * 2f64.powi(28));
    }

    #[test]
    fn float_conversion() {
        assert_eq!(atof_f32(b"0.1"), 0.1f32);
        assert_eq!(atof_f32(b"1e39"), f32::INFINITY);
    }
}
