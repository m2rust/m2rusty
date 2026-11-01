//! The Linux `MultiByteToWideChar` / `WideCharToMultiByte` of
//! `StringCodec.cpp`: a loop around glibc iconv that skips one input unit
//! after every conversion error.
//!
//! The code page tables are glibc's (2.44), dumped by
//! `m2diff_locale tables` into `tables/cp<N>.bin`; UTF-8 is computed.
//! `encoding_rs` is not used: its WHATWG tables differ from glibc's (CP1252
//! 0x81, GBK, Big5, the CP1255/CP1258 combining logic, ...).
//!
//! Table file format (little endian):
//! - `"M2CP"`, `u32` version 1
//! - 256 × `u32`, per byte: the character (bit 31 set when glibc holds it
//!   back to combine it with the next one), [`INVALID`] or [`LEAD`]
//! - `u32` n, n × `u16`: for lead byte `l` and trail `t`, entry
//!   `(l - 0x80) * 256 + t`: the character, 0 when invalid, [`PAIR_SKIP2`]
//! - `u32` n, n × (`u32` held, `u32` next, `u32` composed, `u32` keep held)
//! - `u32` n, n × (`u32` code point, `u8` length, 3 bytes): encoder entries
//!   that differ from the inverse of the decoder (first single byte, else
//!   first pair, in byte order); length 0 means unencodable

use std::collections::HashMap;
use std::sync::OnceLock;

const INVALID: u32 = 0xffff_ffff;
const LEAD: u32 = 0xffff_fffe;
const HELD: u32 = 0x8000_0000;
/// glibc's UHC rejects A2 E8 after consuming both bytes (the only such pair).
const PAIR_SKIP2: u16 = 0xffff;

/// One glibc code page.
struct Table {
    single: [u32; 256],
    pairs: Vec<u16>,
    /// (held character, next character) -> (composed, composed stays held)
    compositions: HashMap<(u32, u32), (u32, bool)>,
    encoder: HashMap<u32, Vec<u8>>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> &[u8] {
        let out = &self.data[self.pos..self.pos + n];
        self.pos += n;
        out
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes(4).try_into().unwrap())
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.bytes(2).try_into().unwrap())
    }
}

impl Table {
    fn parse(data: &[u8]) -> Self {
        let mut r = Reader { data, pos: 0 };
        assert_eq!(r.bytes(4), b"M2CP");
        assert_eq!(r.u32(), 1);
        let single: [u32; 256] = std::array::from_fn(|_| r.u32());
        let pair_count = r.u32() as usize;
        let pairs: Vec<u16> = (0..pair_count).map(|_| r.u16()).collect();
        let compositions = (0..r.u32())
            .map(|_| {
                let (held, next, composed, keep) = (r.u32(), r.u32(), r.u32(), r.u32());
                ((held, next), (composed, keep != 0))
            })
            .collect();

        let mut encoder = HashMap::new();
        for (b, &entry) in single.iter().enumerate() {
            if entry != INVALID && entry != LEAD {
                encoder.entry(entry & !HELD).or_insert_with(|| vec![b as u8]);
            }
        }
        for (i, &c) in pairs.iter().enumerate() {
            if c != 0 && c != PAIR_SKIP2 {
                encoder.entry(u32::from(c)).or_insert_with(|| vec![(0x80 + i / 256) as u8, (i % 256) as u8]);
            }
        }
        for _ in 0..r.u32() {
            let c = r.u32();
            let len = r.bytes(1)[0] as usize;
            let bytes = r.bytes(3)[..len].to_vec();
            if bytes.is_empty() {
                encoder.remove(&c);
            } else {
                encoder.insert(c, bytes);
            }
        }
        assert_eq!(r.pos, data.len(), "trailing table data");
        Table { single, pairs, compositions, encoder }
    }
}

/// The table for a code page, `None` for UTF-8 (`GetIconvEncoding`: CP_ACP,
/// CP_UTF8 and every unknown code page).
fn table(code_page: u32) -> Option<&'static Table> {
    static TABLES: [OnceLock<Table>; 14] = [const { OnceLock::new() }; 14];
    let (slot, data): (usize, &[u8]) = match code_page {
        949 => (0, include_bytes!("tables/cp949.bin")),
        932 => (1, include_bytes!("tables/cp932.bin")),
        936 => (2, include_bytes!("tables/cp936.bin")),
        950 => (3, include_bytes!("tables/cp950.bin")),
        1250 => (4, include_bytes!("tables/cp1250.bin")),
        1251 => (5, include_bytes!("tables/cp1251.bin")),
        1252 => (6, include_bytes!("tables/cp1252.bin")),
        1253 => (7, include_bytes!("tables/cp1253.bin")),
        1254 => (8, include_bytes!("tables/cp1254.bin")),
        1255 => (9, include_bytes!("tables/cp1255.bin")),
        1256 => (10, include_bytes!("tables/cp1256.bin")),
        1257 => (11, include_bytes!("tables/cp1257.bin")),
        1258 => (12, include_bytes!("tables/cp1258.bin")),
        874 => (13, include_bytes!("tables/cp874.bin")),
        _ => return None,
    };
    Some(TABLES[slot].get_or_init(|| Table::parse(data)))
}

/// Decodes at most `cap` characters (`usize::MAX` for the size query).
pub(super) fn decode(code_page: u32, input: &[u8], cap: usize) -> Vec<u32> {
    match table(code_page) {
        Some(t) => decode_table(t, input, cap),
        None => decode_utf8(input, cap),
    }
}

/// Encodes into at most `cap` bytes (`usize::MAX` for the size query).
pub(super) fn encode(code_page: u32, input: &[u32], cap: usize) -> Vec<u8> {
    let table = table(code_page);
    let mut out = Vec::new();
    for &c in input {
        if out.len() >= cap {
            break;
        }
        let mut utf8 = [0; 6];
        let bytes: &[u8] = match table {
            Some(t) => match t.encoder.get(&c) {
                Some(bytes) => bytes,
                // glibc drops the tag characters U+E0000..U+E007F
                // (UNICODE_TAG_HANDLER); anything else is an error, and the
                // C++ loop skips the character.
                None => continue,
            },
            None => match encode_utf8(c, &mut utf8) {
                Some(n) => &utf8[..n],
                None => continue,
            },
        };
        if out.len() + bytes.len() > cap {
            break; // E2BIG
        }
        out.extend_from_slice(bytes);
    }
    out
}

fn decode_table(t: &Table, input: &[u8], cap: usize) -> Vec<u32> {
    let mut out = Vec::new();
    let mut held: Option<u32> = None;
    let mut pos = 0;

    // glibc checks for room before every character, even one it only holds.
    while pos < input.len() && out.len() < cap {
        let lead = input[pos];
        let entry = t.single[usize::from(lead)];
        match entry {
            // EILSEQ: the C++ loop skips the byte; a held character stays.
            INVALID => pos += 1,
            LEAD => {
                let Some(&trail) = input.get(pos + 1) else {
                    pos += 1; // EINVAL at the end, skipped too
                    continue;
                };
                match t.pairs[(usize::from(lead) - 0x80) * 256 + usize::from(trail)] {
                    0 => pos += 1,
                    // Both bytes consumed, then the C++ loop skips another.
                    // At the end of the input that runs past the buffer
                    // (undefined behaviour); stop instead.
                    PAIR_SKIP2 => pos += 3,
                    c => {
                        out.push(u32::from(c));
                        pos += 2;
                    }
                }
            }
            _ => {
                let c = entry & !HELD;
                let hold = entry & HELD != 0;
                if let Some(last) = held.take() {
                    if let Some(&(composed, keep)) = t.compositions.get(&(last, c)) {
                        if keep {
                            held = Some(composed);
                        } else {
                            out.push(composed);
                        }
                        pos += 1;
                        continue;
                    }
                    out.push(last);
                    if !hold && out.len() == cap {
                        break; // no room for this one: E2BIG
                    }
                }
                if hold {
                    held = Some(c);
                } else {
                    out.push(c);
                }
                pos += 1;
            }
        }
    }
    // A character still held at the end is lost: the C++ code never flushes
    // the iconv state.
    out
}

/// One UTF-8 character as glibc reads it: up to six bytes (31 bits), no
/// overlong forms, no surrogates.
fn utf8_char(input: &[u8]) -> Option<(u32, usize)> {
    let b0 = input[0];
    let (len, init, min) = match b0 {
        0x00..=0x7f => return Some((u32::from(b0), 1)),
        0xc2..=0xdf => (2, b0 & 0x1f, 0x80),
        0xe0..=0xef => (3, b0 & 0x0f, 0x800),
        0xf0..=0xf7 => (4, b0 & 0x07, 0x1_0000),
        0xf8..=0xfb => (5, b0 & 0x03, 0x20_0000),
        0xfc..=0xfd => (6, b0 & 0x01, 0x400_0000),
        _ => return None,
    };
    let tail = input.get(1..len)?;
    let mut c = u32::from(init);
    for &b in tail {
        if b & 0xc0 != 0x80 {
            return None;
        }
        c = (c << 6) | u32::from(b & 0x3f);
    }
    (c >= min && !(0xd800..0xe000).contains(&c)).then_some((c, len))
}

fn decode_utf8(input: &[u8], cap: usize) -> Vec<u32> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < input.len() && out.len() < cap {
        match utf8_char(&input[pos..]) {
            Some((c, len)) => {
                out.push(c);
                pos += len;
            }
            None => pos += 1, // EILSEQ or EINVAL: skipped
        }
    }
    out
}

/// glibc's UTF-8 encoder: up to 0x7FFFFFFF in up to six bytes, no surrogates.
fn encode_utf8(c: u32, buf: &mut [u8; 6]) -> Option<usize> {
    let len = match c {
        0..0x80 => {
            buf[0] = c as u8;
            return Some(1);
        }
        0xd800..0xe000 => return None,
        0x80..0x800 => 2,
        0x800..0x1_0000 => 3,
        0x1_0000..0x20_0000 => 4,
        0x20_0000..0x400_0000 => 5,
        0x400_0000..0x8000_0000 => 6,
        _ => return None,
    };
    let mut rest = c;
    for b in buf[1..len].iter_mut().rev() {
        *b = 0x80 | (rest & 0x3f) as u8;
        rest >>= 6;
    }
    buf[0] = ((0xff00u32 >> len) as u8) | rest as u8;
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_forms() {
        assert_eq!(decode_utf8(b"\xc0\x80\xc2\x80", 9), [0x80]);
        assert_eq!(decode_utf8(b"\xed\xa0\x80A", 9), [0x41]);
        assert_eq!(decode_utf8(b"\xfd\xbf\xbf\xbf\xbf\xbf", 9), [0x7fff_ffff]);
        let mut buf = [0; 6];
        let n = encode_utf8(0x7fff_ffff, &mut buf).unwrap();
        assert_eq!(&buf[..n], b"\xfd\xbf\xbf\xbf\xbf\xbf");
        let n = encode_utf8(0xac00, &mut buf).unwrap();
        assert_eq!(&buf[..n], "\u{ac00}".as_bytes());
        assert_eq!(encode_utf8(0xdc00, &mut buf), None);
    }

    #[test]
    fn uhc_a2e8_swallows_the_next_byte() {
        assert_eq!(decode(949, b"\xa2\xe8AB", 9), [0x42]);
        // At the end the C++ code reads past the input; stop instead.
        assert_eq!(decode(949, b"A\xa2\xe8", 9), [0x41]);
    }

    #[test]
    fn held_characters() {
        // CP1258 holds letters back for a tone mark and never flushes.
        assert_eq!(decode(1258, b"A", 9), Vec::<u32>::new());
        assert_eq!(decode(1258, b"A\xec ", 9), [0xc1, 0x20]);
        // An undefined byte keeps the held letter.
        assert_eq!(decode(1255, b"\xe0\x81 ", 9), [0x5d0, 0x20]);
    }
}
