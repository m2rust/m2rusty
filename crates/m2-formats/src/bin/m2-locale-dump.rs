//! Rust counterpart of `m2diff_locale` (`tools/differential/locale/LocaleDump.cpp`).
//!
//!   m2-locale-dump codec <code page> | vietnamese | arabic | japanese |
//!                  corpus <code page> <file>...
//!
//! Prints the same lines as the C++ tool (its `tables` command has no
//! counterpart: it generates `src/locale/tables`).

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use m2_formats::locale::{
    arabic, japanese, multi_byte_to_wide_char, wide_char_to_multi_byte, ymir_multi_byte_to_wide_char,
    ymir_wide_char_to_multi_byte,
};

const CODE_PAGES: [u32; 15] = [65001, 949, 932, 936, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 874];

type Out = BufWriter<std::io::StdoutLock<'static>>;
type DecodeFn = fn(u32, &[u8], &mut [u32]) -> usize;
type EncodeFn = fn(u32, &[u32], &mut [u8]) -> usize;

fn wide(w: &[u32]) -> String {
    if w.is_empty() {
        return "-".into();
    }
    w.iter().map(|c| format!("{c:x}")).collect::<Vec<_>>().join(",")
}

fn bytes(b: &[u8]) -> String {
    if b.is_empty() {
        return "-".into();
    }
    b.iter().map(|c| format!("{c:02x}")).collect()
}

fn dump_decode(out: &mut Out, tag: char, decode: DecodeFn, cp: u32, input: &[u8]) {
    let mut buf = [0; 64];
    let written = decode(cp, input, &mut buf);
    let count = decode(cp, input, &mut []);
    let mut one = [0; 1];
    let written1 = decode(cp, input, &mut one);
    writeln!(out, "{tag} {} {} n={count} c1={}", bytes(input), wide(&buf[..written]), wide(&one[..written1])).unwrap();
}

fn dump_encode(out: &mut Out, tag: char, encode: EncodeFn, cp: u32, input: &[u32]) {
    let mut buf = [0; 64];
    let written = encode(cp, input, &mut buf);
    let count = encode(cp, input, &mut []);
    let mut one = [0; 1];
    let written1 = encode(cp, input, &mut one);
    writeln!(out, "{tag} {} {} n={count} c1={}", wide(input), bytes(&buf[..written]), bytes(&one[..written1])).unwrap();
}

fn encode_sample() -> Vec<u32> {
    let mut cps: Vec<u32> = (0..0x1_0000).collect();
    cps.extend((0x1_0000..0x11_0000).step_by(0x101));
    cps.extend(0xd_ff00..0xe_0100);
    cps.extend([
        0x10_ffff,
        0x11_0000,
        0x1f_ffff,
        0x20_0000,
        0x3ff_ffff,
        0x400_0000,
        0x7fff_ffff,
        0x8000_0000,
        0xffff_ff41,
        0xffff_ffff,
    ]);
    cps
}

fn codec(out: &mut Out, cp: u32) {
    let d = multi_byte_to_wide_char as DecodeFn;
    for b in 0..=255u8 {
        dump_decode(out, 'D', d, cp, &[b]);
        dump_decode(out, 'D', d, cp, &[b, 0x20]);
    }
    for l in 0..=255u8 {
        for t in 0..=255u8 {
            // See LocaleDump.cpp: C++ reads past the input here.
            if !(cp == 949 && l == 0xa2 && t == 0xe8) {
                dump_decode(out, 'D', d, cp, &[l, t]);
            }
            dump_decode(out, 'D', d, cp, &[l, t, 0x20]);
        }
    }
    dump_decode(out, 'D', d, cp, &[0xa2, 0xe8, 0x41, 0x42]);

    let mut marks: Vec<u8> = match cp {
        1255 => (0xc0..=0xd2).collect(),
        1258 => vec![0xcc, 0xd2, 0xde, 0xec, 0xf2, 0xca],
        _ => Vec::new(),
    };
    if !marks.is_empty() {
        marks.extend([0x20, 0x41, 0x81]);
        for x in 0..=255u8 {
            for &y in &marks {
                for &z in &marks {
                    dump_decode(out, 'D', d, cp, &[x, y, z]);
                    dump_decode(out, 'D', d, cp, &[x, y, z, 0x20]);
                }
            }
        }
    }

    if cp == 65001 {
        let tails = [0x00, 0x41, 0x7f, 0x80, 0x8f, 0x90, 0x9f, 0xa0, 0xbf, 0xc0, 0xff];
        for a in 0xc0..=0xffu8 {
            for b in 0x80..0xc0u8 {
                for c in tails {
                    dump_decode(out, 'D', d, cp, &[a, b, c]);
                }
            }
        }
        for a in 0xf0..=0xffu8 {
            for b in 0x80..0xc0u8 {
                for c in [0x41, 0x80, 0xbf] {
                    for e in [0x41, 0x80, 0xbf] {
                        dump_decode(out, 'D', d, cp, &[a, b, c, e]);
                        dump_decode(out, 'D', d, cp, &[a, b, c, e, 0x80, 0x80]);
                        dump_decode(out, 'D', d, cp, &[a, b, c, e, 0xbf, 0x41]);
                    }
                }
            }
        }
    }

    let e = wide_char_to_multi_byte as EncodeFn;
    for c in encode_sample() {
        dump_encode(out, 'E', e, cp, &[c]);
    }
    let sequences: [&[u32]; 7] = [
        &[0x41, 0xe0001, 0x42],
        &[0x41, 0xd800, 0x42],
        &[0x41, 0xffff_ffff, 0x42],
        &[0xac00, 0x41],
        &[0x41, 0x11_0000, 0x42],
        &[0xfb2c, 0x41, 0x1ea0],
        &[0x30a2, 0x4e00, 0x41],
    ];
    for s in sequences {
        dump_encode(out, 'E', e, cp, s);
    }
}

fn vietnamese(out: &mut Out) {
    let d = ymir_multi_byte_to_wide_char as DecodeFn;
    for b in 0..=255u8 {
        dump_decode(out, 'V', d, 1258, &[b]);
    }
    for x in 0..=255u8 {
        for y in 0..=255u8 {
            dump_decode(out, 'V', d, 1258, &[x, y]);
        }
    }
    for x in 0..=255u8 {
        for t in [0xcc, 0xd2, 0xde, 0xec, 0xf2] {
            for y in [0xcc, 0xd2, 0xde, 0xec, 0xf2, 0x41, 0x20, 0xc3] {
                dump_decode(out, 'V', d, 1258, &[x, t, y]);
            }
        }
    }
    let e = ymir_wide_char_to_multi_byte as EncodeFn;
    for c in encode_sample() {
        dump_encode(out, 'W', e, 1258, &[c]);
    }
    dump_encode(out, 'W', e, 1258, &[0x1ea0, 0x41, 0x4e00, 0x1ec7]);
}

/// `NextRandom` of the C++ tool.
struct Random(u32);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }
}

fn dump_shape(out: &mut Out, src: &[u32]) {
    let dst = arabic::make_shape(src);
    let h: String = (-1..dst.len() as i32).map(|i| if arabic::has_presentation(&dst, i) { '1' } else { '0' }).collect();
    writeln!(out, "S {} {} h={h}", wide(src), wide(&dst)).unwrap();
}

fn arabic_dump(out: &mut Out) {
    for c in 0..0x1_0000u32 {
        let flag = |b: bool| u8::from(b);
        writeln!(
            out,
            "P {c:x} {}{}{} {:x}",
            flag(arabic::is_in_space(c)),
            flag(arabic::is_in_symbol(c)),
            flag(arabic::is_in_presentation(c)),
            arabic::conv_symbol(c)
        )
        .unwrap();
    }

    let mut alphabet: Vec<u32> = (0x0621..=0x064a).collect();
    alphabet.extend([0x064b, 0x0651, 0x0670, 0x06d6, 0x06ed, 0x06dd, 0x20, 0x41, 0x28, 0xfe8d, 0x08]);
    dump_shape(out, &[]);
    for &a in &alphabet {
        dump_shape(out, &[a]);
        for &b in &alphabet {
            dump_shape(out, &[a, b]);
            for &c in &alphabet {
                dump_shape(out, &[a, b, c]);
            }
        }
    }
    let mut rng = Random(0x2545_f491);
    for _ in 0..20000 {
        let len = 4 + rng.next() as usize % 21;
        let src: Vec<u32> = (0..len).map(|_| alphabet[rng.next() as usize % alphabet.len()]).collect();
        dump_shape(out, &src);
    }
}

fn dump_compare(out: &mut Out, a: &[u8], b: &[u8], length: usize) {
    writeln!(out, "C {} {} {length} {}", bytes(a), bytes(b), japanese::string_compare_ci(a, b, length)).unwrap();
}

fn japanese_dump(out: &mut Out) {
    for b in 0..=255u8 {
        writeln!(out, "L {b:02x} {}{}", u8::from(japanese::is_lead_byte(b)), u8::from(japanese::is_trail_byte(b)))
            .unwrap();
    }
    let alphabet = [b'a', b'A', b'z', b'Z', b'[', 0x81, 0x40, 0x9f, 0xe0, 0xfc, 0x80, 0x7e];
    let mut strings: Vec<Vec<u8>> = vec![Vec::new()];
    for &a in &alphabet {
        strings.push(vec![a]);
        for &b in &alphabet {
            strings.push(vec![a, b]);
        }
    }
    for a in &strings {
        for b in &strings {
            for length in 0..4 {
                dump_compare(out, a, b, length);
            }
        }
    }
    let mut rng = Random(0x2545_f491);
    for _ in 0..20000 {
        let la = rng.next() as usize % 9;
        let lb = rng.next() as usize % 9;
        let a: Vec<u8> = (0..la).map(|_| alphabet[rng.next() as usize % alphabet.len()]).collect();
        let b: Vec<u8> = (0..lb).map(|_| alphabet[rng.next() as usize % alphabet.len()]).collect();
        let length = rng.next() as usize % 11;
        dump_compare(out, &a, &b, length);
    }
}

fn fnv1a64(data: impl IntoIterator<Item = u8>) -> u64 {
    data.into_iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

fn hash_wide(w: &[u32]) -> u64 {
    fnv1a64(w.iter().flat_map(|c| c.to_le_bytes()))
}

fn hash_file(out: &mut Out, path: &str, data: &[u8], cp: u32, decode: DecodeFn, encode: EncodeFn, label: &str) {
    let count = decode(cp, data, &mut []);
    let mut w = vec![0; data.len()];
    let written = decode(cp, data, &mut w);
    let w = &w[..written];
    let bytes_count = encode(cp, w, &mut []);
    let mut back = vec![0; written * 6 + 1];
    let bytes_written = encode(cp, w, &mut back);
    writeln!(
        out,
        "H {path} {label} {count} {written} {:016x} {bytes_count} {bytes_written} {:016x}",
        hash_wide(w),
        fnv1a64(back[..bytes_written].iter().copied())
    )
    .unwrap();
}

fn corpus(out: &mut Out, cp: u32, files: &[String]) -> Result<(), String> {
    for path in files {
        let data = std::fs::read(path).map_err(|e| format!("cannot open {path}: {e}"))?;
        let mut start = 0;
        let mut line_no = 0;
        while start < data.len() {
            let end = data[start..].iter().position(|&b| b == b'\n').map_or(data.len(), |p| start + p);
            let line = &data[start..end];
            let len = line.len();

            let mut w = vec![0; len];
            let written = ymir_multi_byte_to_wide_char(cp, line, &mut w);
            let count = ymir_multi_byte_to_wide_char(cp, line, &mut []);
            let half = ymir_multi_byte_to_wide_char(cp, line, &mut vec![0; len / 2]);
            let w = &w[..written];

            let mut back = vec![0; written * 6 + 1];
            let n = ymir_wide_char_to_multi_byte(cp, w, &mut back);
            let bytes_count = ymir_wide_char_to_multi_byte(cp, w, &mut []);
            let back = &back[..n];
            let encoded = if back == line { "=".into() } else { bytes(back) };
            writeln!(out, "L {path} {line_no} {} n={count} t={half} {encoded} n={bytes_count}", wide(w)).unwrap();

            start = end + 1;
            line_no += 1;
        }

        for cp in CODE_PAGES {
            hash_file(out, path, &data, cp, multi_byte_to_wide_char, wide_char_to_multi_byte, &cp.to_string());
        }
        hash_file(out, path, &data, 1258, ymir_multi_byte_to_wide_char, ymir_wide_char_to_multi_byte, "ymir1258");
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut out = BufWriter::new(std::io::stdout().lock());
    let cp = |i: usize| args.get(i).and_then(|s| s.parse::<u32>().ok());
    let result = match (args.get(1).map(String::as_str), args.len()) {
        (Some("codec"), 3) if cp(2).is_some() => {
            codec(&mut out, cp(2).unwrap());
            Ok(())
        }
        (Some("vietnamese"), 2) => {
            vietnamese(&mut out);
            Ok(())
        }
        (Some("arabic"), 2) => {
            arabic_dump(&mut out);
            Ok(())
        }
        (Some("japanese"), 2) => {
            japanese_dump(&mut out);
            Ok(())
        }
        (Some("corpus"), n) if n >= 3 && cp(2).is_some() => corpus(&mut out, cp(2).unwrap(), &args[3..]),
        _ => {
            eprintln!(
                "usage: {} codec <code page> | vietnamese | arabic | japanese | corpus <code page> <file>...",
                args[0]
            );
            return ExitCode::from(2);
        }
    };
    out.flush().unwrap();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
