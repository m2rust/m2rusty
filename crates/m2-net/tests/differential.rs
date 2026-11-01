#![allow(clippy::type_complexity, clippy::same_item_push)]
//! Differential tests against the real C++ (`tools/differential/net`, target
//! `m2diff_net`). They skip with a message when the dumper is not built; build it with
//! `scripts/differential.sh` or
//! `cmake -S . -B build/differential -DM2_BUILD_DIFF_TOOLS=ON && cmake --build build/differential --target m2diff_net`.
//! `M2_DIFF_BUILD` points at the build directory (default `build/differential` in the
//! workspace root).

use m2_net::stream::{FrameError, NetStream, SEQUENCE_TABLE, header_info};
use m2_net::wire::{self, GC_HEADER_MAP, Wire};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

fn dumper() -> Option<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let build = std::env::var_os("M2_DIFF_BUILD").map(PathBuf::from).unwrap_or_else(|| root.join("build/differential"));
    let p = build.join("tools/differential/net/m2diff_net");
    if p.exists() {
        Some(p)
    } else {
        eprintln!("skipping: {} not built", p.display());
        None
    }
}

fn run(args: &[&str]) -> Vec<String> {
    let out = Command::new(dumper().unwrap()).args(args).output().expect("run m2diff_net");
    assert!(out.status.success(), "m2diff_net {args:?} failed");
    String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect()
}

#[test]
fn struct_layouts_match_cpp() {
    if dumper().is_none() {
        return;
    }
    let mut cpp: BTreeMap<String, (usize, Vec<(String, usize, usize)>)> = BTreeMap::new();
    for l in run(&["layout"]) {
        let p: Vec<&str> = l.split(' ').collect();
        let name = p.get(1).map(|n| n.trim_start_matches("TPacketGCShopStartEx::").to_owned());
        match p[0] {
            "S" => {
                cpp.insert(name.unwrap(), (p[2].parse().unwrap(), vec![]));
            }
            "F" => cpp.get_mut(&name.unwrap()).unwrap().1.push((p[2].to_owned(), p[3].parse().unwrap(), p[4].parse().unwrap())),
            _ => {}
        }
    }
    let rust = wire::layouts();
    assert_eq!(rust.len(), cpp.len(), "struct count");
    let mut fields = 0;
    for (name, size, offs) in &rust {
        let (csize, cfields) = cpp.get(*name).unwrap_or_else(|| panic!("{name} missing in C++"));
        assert_eq!(size, csize, "sizeof({name})");
        assert_eq!(offs.len(), cfields.len(), "field count of {name}");
        for ((rn, ro), (cn, co, _)) in offs.iter().zip(cfields) {
            assert_eq!((*rn, *ro), (cn.as_str(), *co), "field of {name}");
            fields += 1;
        }
    }
    eprintln!("{} structs, {} field offsets identical", rust.len(), fields);
}

#[test]
fn header_constants_match_cpp() {
    if dumper().is_none() {
        return;
    }
    let mut n = 0;
    for l in run(&["headers"]) {
        let p: Vec<&str> = l.split(' ').collect();
        if p[0] != "H" {
            continue;
        }
        let table = if p[1] == "CG" { wire::cg::ALL } else { wire::gc::ALL };
        let v: u8 = p[3].parse().unwrap();
        assert!(table.contains(&(p[2], v)), "{} {} = {v}", p[1], p[2]);
        n += 1;
    }
    assert_eq!(n, wire::cg::ALL.len() + wire::gc::ALL.len());
    eprintln!("{n} header constants identical");
}

#[test]
fn header_map_matches_cpp() {
    if dumper().is_none() {
        return;
    }
    let mut cpp = BTreeMap::new();
    for l in run(&["map"]) {
        let p: Vec<&str> = l.split(' ').collect();
        if p[0] == "M" {
            cpp.insert(p[1].parse::<u8>().unwrap(), (p[2].parse::<usize>().unwrap(), p[3] == "1"));
        }
    }
    // The C++ map is keyed by header, so a header registered twice keeps the last entry.
    let mut rust = BTreeMap::new();
    for h in GC_HEADER_MAP {
        rust.insert(h.header, (h.size, h.dynamic));
    }
    assert_eq!(rust, cpp);
    eprintln!("{} header map entries identical", cpp.len());
}

#[test]
fn sequence_bytes_match_cpp() {
    if dumper().is_none() {
        return;
    }
    // More than one full lap of the table, to check the wrap-around too.
    let n = 32768 + 100;
    let out = run(&["sequence", &n.to_string()]);
    let hex = out.last().unwrap();
    let bytes: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
    assert_eq!(bytes.len(), n);
    let mut s = NetStream::new(1 << 20, 1 << 20);
    s.set_sequence_mode(true);
    let mut mine = Vec::new();
    for _ in 0..n {
        assert!(s.send_sequence());
        mine.extend(s.take_output(usize::MAX));
    }
    assert_eq!(mine, bytes);
    assert_eq!(&mine[..4], &SEQUENCE_TABLE[..4]);
    assert_eq!(mine[32768], mine[0]);
    eprintln!("{n} sequence bytes identical");
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A scripted stream: valid packets of random headers with random zero padding, optional
/// unknown header at the end, cut into random chunks.
fn script(seed: u64, with_unknown: bool) -> Vec<Vec<u8>> {
    let mut rng = Rng(seed);
    let mut stream = Vec::new();
    for _ in 0..60 {
        for _ in 0..rng.below(3) {
            stream.push(0);
        }
        // The four headers whose handlers read more than the map announces are framed differently on purpose.
        let h = loop {
            let h = &GC_HEADER_MAP[rng.below(GC_HEADER_MAP.len())];
            if !matches!(h.header, wire::gc::WHISPER | wire::gc::PLAYER_DELETE_SUCCESS | wire::gc::SAFEBOX_SET | wire::gc::MALL_SET) {
                break h;
            }
        };
        let mut p = vec![0u8; if h.dynamic { 3 + rng.below(40) } else { h.size }];
        for b in p.iter_mut() {
            *b = rng.next() as u8;
        }
        p[0] = h.header;
        if h.dynamic {
            let l = p.len() as u16;
            p[1..3].copy_from_slice(&l.to_le_bytes());
        }
        stream.extend(p);
    }
    if with_unknown {
        stream.extend([0xee, 1, 2, 3]); // 0xee is not a registered header
        assert!(header_info(0xee).is_none());
    }
    let mut chunks = Vec::new();
    let mut i = 0;
    while i < stream.len() {
        let lim = if rng.below(3) == 0 { 4 } else { 90 };
        let n = (1 + rng.below(lim)).min(stream.len() - i);
        chunks.push(stream[i..i + n].to_vec());
        i += n;
    }
    chunks
}

fn rust_frames(chunks: &[Vec<u8>]) -> Vec<String> {
    let mut s = NetStream::new(1 << 20, 1 << 10);
    let mut out = Vec::new();
    for c in chunks {
        assert_eq!(s.feed(c), c.len());
        loop {
            match s.poll_frame() {
                Ok(Some(f)) => out.push(format!("P {} {} {}", f.header, f.bytes.len(), hex(&f.bytes))),
                Ok(None) => break,
                Err(FrameError::UnknownHeader(h)) => {
                    out.push(format!("U {h}"));
                    break;
                }
                Err(e) => panic!("{e:?}"),
            }
        }
        out.push(format!("R {}", s.recv_len()));
    }
    out
}

#[test]
fn framing_matches_real_cnetworkstream() {
    if dumper().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("m2net-frame-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut total = 0;
    for seed in 1..=12u64 {
        let chunks = script(seed * 7919, seed % 3 == 0);
        let path = dir.join(format!("s{seed}.txt"));
        let text: String = chunks.iter().map(|c| format!("chunk {}\n", hex(c))).collect();
        std::fs::write(&path, text).unwrap();
        let cpp: Vec<String> = run(&["frame", path.to_str().unwrap()]).into_iter().filter(|l| matches!(l.as_bytes()[0], b'P' | b'U' | b'R') && l.as_bytes()[1] == b' ').collect();
        let rust = rust_frames(&chunks);
        assert_eq!(rust, cpp, "seed {seed}");
        total += rust.iter().filter(|l| l.starts_with('P')).count();
    }
    std::fs::remove_dir_all(&dir).ok();
    eprintln!("12 scripted streams, {total} packets identical (partial reads, dynamic sizes, zero padding, unknown header)");
}

#[test]
fn every_header_map_struct_decodes() {
    // Not a C++ comparison: GcPacket::decode accepts a frame of exactly the mapped size for each header.
    for h in GC_HEADER_MAP {
        let mut f = vec![0u8; if h.dynamic { h.size.max(3) } else { h.size }];
        f[0] = h.header;
        if h.dynamic {
            let l = f.len() as u16;
            f[1..3].copy_from_slice(&l.to_le_bytes());
        }
        let p = wire::GcPacket::decode(&f).unwrap_or_else(|e| panic!("{}: {e}", h.name));
        assert_eq!(p.header(), h.header);
        assert_eq!(p.encode(), f, "{}", h.name);
    }
    let _ = <wire::TPacketGCMove as Wire>::SIZE;
}
