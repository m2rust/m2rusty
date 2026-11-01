//! Differential tests: the Rust table loaders (`crypt`, `proto`) against the
//! real C++ managers, on every locale pack and the root pack of the client,
//! plus synthetic packs for the cases real data does not reach. Both sides
//! print every field of every loaded record (`m2diff_proto` /
//! `m2-proto-dump`), which must be byte-identical.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use m2_formats::crypt::{self, ITEM_PROTO_KEY, MOB_PROTO_KEY};
use m2_formats::proto::{ITEM_TABLE_SIZE, MOB_TABLE_SIZE};
use support::deterministic_bytes;
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

const RUST_PROTO_DUMP: &str = env!("CARGO_BIN_EXE_m2-proto-dump");

/// The 7 files `LoadLocaleData` reads, in order.
struct Files<'a> {
    cvt_prefix: &'a str,
    names: [String; 7],
}

impl<'a> Files<'a> {
    fn locale(path: &'a str) -> Self {
        let names = [
            "item_list.txt",
            "item_proto",
            "itemdesc.txt",
            "mob_proto",
            "SkillDesc.txt",
            "SkillTable.txt",
            "insult.txt",
        ]
        .map(|f| format!("{path}/{f}"));
        Self {
            cvt_prefix: path,
            names,
        }
    }
}

/// Runs both dumpers in `tables` mode; returns the number of lines.
fn assert_same_tables(client: &Path, index: &str, ymir: bool, files: &Files) -> usize {
    let mut args: Vec<&OsStr> = vec!["tables".as_ref(), client.as_os_str(), index.as_ref()];
    args.push(if ymir { "1" } else { "0" }.as_ref());
    args.push(files.cvt_prefix.as_ref());
    args.extend(files.names.iter().map(OsStr::new));
    let cpp = run(&cpp_tool("proto"), &args);
    let rust = run(Path::new(RUST_PROTO_DUMP), &args);
    assert_same_lines(&cpp, &rust)
}

fn steps_ok(client: &Path, index: &str, ymir: bool, files: &Files) -> Vec<String> {
    let mut args: Vec<&OsStr> = vec!["tables".as_ref(), client.as_os_str(), index.as_ref()];
    args.push(if ymir { "1" } else { "0" }.as_ref());
    args.push(files.cvt_prefix.as_ref());
    args.extend(files.names.iter().map(OsStr::new));
    String::from_utf8_lossy(&run(Path::new(RUST_PROTO_DUMP), &args))
        .lines()
        .filter(|l| l.starts_with("step "))
        .map(str::to_owned)
        .collect()
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn every_locale_matches_cpp() {
    let client = client_dir();
    let mut locales: Vec<String> = fs::read_dir(client.join("pack"))
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().to_str()?.strip_prefix("locale_").map(str::to_owned))
        .collect();
    locales.sort();
    assert!(locales.len() >= 15, "locale packs missing: {locales:?}");

    for locale in &locales {
        let path = format!("locale/{locale}");
        let files = Files::locale(&path);
        let steps = steps_ok(&client, "pack/Index.dev", false, &files);
        assert_eq!(steps.len(), 7, "{locale}: {steps:?}");
        assert!(steps.iter().all(|s| s.ends_with(" ok")), "{locale}: {steps:?}");
        for ymir in [false, true] {
            let lines = assert_same_tables(&client, "pack/Index.dev", ymir, &files);
            eprintln!("{locale} (ymir {ymir}): {lines} lines identical");
        }
    }
}

/// The root pack's old-format tables: 20110915_ymir_item_proto has a
/// 152-byte stride and 936mob_proto 235-byte records, so both loaders
/// reject them (and LoadLocaleData stops), as does RegisterSkillDesc for
/// 936skilldesc.txt; 936skilltable.txt loads.
/// Each is run behind loaders that succeed so that it is reached.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn root_pack_matches_cpp() {
    let client = client_dir();
    let with = |i: usize, name: &str| {
        let mut files = Files::locale("locale/de");
        files.names[i] = name.to_owned();
        files
    };

    for (files, last) in [
        (
            with(1, "20110915_ymir_item_proto"),
            "step 20110915_ymir_item_proto fail",
        ),
        (with(3, "936mob_proto"), "step 936mob_proto fail"),
        // Another column layout: column 14 (grade count) holds 31..47, so
        // RegisterSkillDesc stops at the first line.
        (with(4, "936skilldesc.txt"), "step 936skilldesc.txt fail"),
        (with(5, "936skilltable.txt"), "step locale/de/insult.txt ok"),
    ] {
        let steps = steps_ok(&client, "pack/Index.dev", false, &files);
        assert_eq!(steps.last().map(String::as_str), Some(last), "{steps:?}");
        for ymir in [false, true] {
            assert_same_tables(&client, "pack/Index.dev", ymir, &files);
        }
    }

    // The CLZObjects inside the rejected files still decode.
    let tmp = tempfile::tempdir().unwrap();
    for (file, offset, key) in [
        ("20110915_ymir_item_proto", 20, ITEM_PROTO_KEY),
        ("936mob_proto", 12, MOB_PROTO_KEY),
    ] {
        let bytes = fs::read(client.join("pack/root").join(file)).unwrap();
        let blob = tmp.path().join(file);
        fs::write(&blob, &bytes[offset..]).unwrap();
        let out = assert_same_lz(&blob, Some(&key));
        assert!(out.starts_with("lz ok"), "{file}: {out}");
    }
}

fn key_args(key: Option<&[u32; 4]>) -> Vec<String> {
    key.map_or_else(Vec::new, |k| k.iter().map(u32::to_string).collect())
}

/// `lz` mode on both sides; returns the (identical) output line.
fn assert_same_lz(blob: &Path, key: Option<&[u32; 4]>) -> String {
    let keys = key_args(key);
    let mut args: Vec<&OsStr> = vec!["lz".as_ref(), blob.as_os_str()];
    args.extend(keys.iter().map(OsStr::new));
    let cpp = run(&cpp_tool("proto"), &args);
    let rust = run(Path::new(RUST_PROTO_DUMP), &args);
    assert_same_lines(&cpp, &rust);
    String::from_utf8(rust).unwrap()
}

/// Compresses (and encrypts) `data` with the real CLZO.
fn cpp_lzpack(dir: &Path, data: &[u8], key: Option<&[u32; 4]>) -> Vec<u8> {
    let input = dir.join("lzpack.in");
    let output = dir.join("lzpack.out");
    fs::write(&input, data).unwrap();
    let keys = key_args(key);
    let mut args: Vec<&OsStr> = vec!["lzpack".as_ref(), input.as_os_str(), output.as_os_str()];
    args.extend(keys.iter().map(OsStr::new));
    run(&cpp_tool("proto"), &args);
    fs::read(output).unwrap()
}

/// liblzo's lzo1x_999 output for inputs that exercise every instruction
/// (literal runs, near/far matches, long runs), decoded by both sides.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn lz_objects_match_cpp() {
    let tmp = tempfile::tempdir().unwrap();
    let mut inputs: Vec<Vec<u8>> = vec![vec![], vec![7], b"abc".to_vec(), vec![0; 100_000]];
    for (len, seed) in [(17usize, 1u32), (300, 2), (5000, 3), (70_000, 4), (300_000, 5)] {
        // Random bytes, and text-like data with repeats at many distances.
        inputs.push(deterministic_bytes(len, seed));
        let words = deterministic_bytes(len / 8 + 1, seed);
        inputs.push(
            (0..len)
                .map(|i| b"lorem ipsum dolor sit amet "[(i + usize::from(words[i / 8]) * (i / 64)) % 27])
                .collect(),
        );
    }
    // Real tables, raw.
    let client = client_dir();
    for file in ["locale_de/locale/de/item_list.txt", "locale_ru/locale/ru/itemdesc.txt"] {
        inputs.push(fs::read(client.join("pack").join(file)).unwrap());
    }

    for (i, data) in inputs.iter().enumerate() {
        for key in [None, Some(&ITEM_PROTO_KEY), Some(&MOB_PROTO_KEY)] {
            let blob = cpp_lzpack(tmp.path(), data, key);
            assert_eq!(crypt::lz_decompress(&blob, key).unwrap(), *data, "input {i}");
            let path = tmp.path().join("blob");
            fs::write(&path, &blob).unwrap();
            assert!(assert_same_lz(&path, key).starts_with("lz ok"));
        }
    }
    eprintln!("{} inputs, 3 keys each, identical", inputs.len());
}

/// An item record: deterministic bytes with the given vnum, range and name.
fn item_record(vnum: u32, range: u32, name: &[u8], seed: u32) -> Vec<u8> {
    let mut r = deterministic_bytes(ITEM_TABLE_SIZE, seed);
    r[0..4].copy_from_slice(&vnum.to_le_bytes());
    r[4..8].copy_from_slice(&range.to_le_bytes());
    r[8..33].fill(0);
    r[8..8 + name.len()].copy_from_slice(name);
    // End szLocaleName, so that hashing an unterminated szName stays inside
    // the names (C++ would go on into the numeric fields).
    r[57] = 0;
    r
}

fn mob_record(vnum: u32, seed: u32) -> Vec<u8> {
    let mut r = deterministic_bytes(MOB_TABLE_SIZE, seed);
    r[0..4].copy_from_slice(&vnum.to_le_bytes());
    r
}

fn words(list: &[u32]) -> Vec<u8> {
    list.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// Writes a folder pack `pack/synth` with the files of `locale/<name>`.
fn write_locale(root: &Path, name: &str, files: &[(&str, Vec<u8>)]) {
    let dir = root.join("pack/synth/locale").join(name);
    fs::create_dir_all(&dir).unwrap();
    for (file, data) in files {
        fs::write(dir.join(file), data).unwrap();
    }
}

fn skill_desc_line(columns: &[&str]) -> String {
    let mut c: Vec<&str> = columns.to_vec();
    // Columns 2..=12 must exist: C++ reads them unchecked. A trailing empty
    // column is not a token, so the icon column (12) must not be last and
    // empty.
    while c.len() < 13 {
        c.push("");
    }
    if c.len() == 13 && c[12].is_empty() {
        c[12] = "icon";
    }
    let mut s = c.join("\t");
    s.push('\n');
    s
}

fn skill_table_line(vnum: &str, max: &str, limit: &str, poly: &str) -> String {
    let mut c = vec![""; 27];
    c[0] = vnum;
    c[1] = "name";
    c[4] = max;
    c[5] = limit;
    c[7] = poly;
    c[8] = "40+lv";
    c[9] = "60";
    c[10] = "2*lv";
    c[11] = "";
    c[25] = "250";
    c[26] = "flag";
    let mut s = c.join("\t");
    s.push('\n');
    s
}

/// Edge cases real data does not reach: quoting, short and duplicate lines,
/// icon fallbacks, duplicate records, unknown skill jobs and attributes,
/// skill-proto formula rewriting, conversion tables.
#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_tables_match_cpp() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("client");
    fs::create_dir_all(root.join("pack/synth/icon/item")).unwrap();
    fs::write(root.join("pack/Index.dev"), "synth\tFOLDER\n").unwrap();
    for icon in ["00100", "00110", "27995"] {
        fs::write(root.join(format!("pack/synth/icon/item/{icon}.tga")), b"tga").unwrap();
    }

    let item_list = b"100\tWEAPON\ticon/item/listed.tga\td:/ymir work/item/weapon/00100.gr2\n\
        101\tETC\t\"icon/item/quoted name.tga\"\n\
        102\tETC\n\
        \t\t\n\
        103\tETC\ticon\tmodel\textra\n\
        100\tWEAPON\ticon/item/again.tga\n\
        104 \tETC\t\"unterminated\n\
        105\tETC\ticon/\xb0\nhidden.tga\n\
        -5\tETC\ticon/neg.tga\n"
        .to_vec();
    let mut records = Vec::new();
    for (i, (vnum, range, name)) in [
        (100u32, 0u32, &b"sword"[..]),
        (113, 0, b"sword"),
        (117, 5, b"other"),
        (125, 0, b"nothing"),
        (5000, 0, &[b'n'; 25][..]),
        (113, 9, b"dup"),
        (101, 0, b"\xb0\xa1name"),
    ]
    .into_iter()
    .enumerate()
    {
        records.extend(item_record(vnum, range, name, i as u32 + 10));
    }
    let blob = cpp_lzpack(tmp.path(), &records, Some(&ITEM_PROTO_KEY));
    let mut item_proto_mipt = b"MIPT".to_vec();
    item_proto_mipt.extend(words(&[7, blob.len() as u32]));
    item_proto_mipt.extend(&blob);
    // MIPX claiming fewer records than it holds.
    let mut item_proto_mipx = b"MIPX".to_vec();
    item_proto_mipx.extend(words(&[1, ITEM_TABLE_SIZE as u32, 5, blob.len() as u32]));
    item_proto_mipx.extend(&blob);

    let item_desc = b"100\tSword\t\"A sword\"\t\"summary\"\n\
        113\tx\t\"half\n\
        117\n\
        999\tnone\tdesc\tsumm\n\
        \n\
        125\tn\t\"\"\t\"\n"
        .to_vec();

    let mobs: Vec<u8> = [(1u32, 1u32), (2, 2), (1, 3), (300, 4)]
        .into_iter()
        .flat_map(|(vnum, seed)| mob_record(vnum, seed))
        .collect();
    let blob = cpp_lzpack(tmp.path(), &mobs, Some(&MOB_PROTO_KEY));
    let mut mob_proto = b"MMPT".to_vec();
    mob_proto.extend(words(&[4, blob.len() as u32]));
    mob_proto.extend(&blob);

    let mut skill_desc = String::new();
    skill_desc += &skill_desc_line(&[
        "1",
        "WARRIOR",
        "Slash",
        "",
        "Slash III",
        "desc",
        "c1",
        "",
        "c3",
        "c4",
        "ATTACK_SKILL|MELEE_ATTACK|NOPE",
        "SWORD|TWO_HANDED",
        "slash",
        "5",
        "4",
        "1",
        "2",
        "dmg",
        "atk*2",
        "",
        "",
        "",
        "",
        "x",
        "y",
        "z",
        "10",
        "",
        "end",
    ]);
    skill_desc += &skill_desc_line(&[
        "2",
        "Assassin",
        "Stab",
        "Stab2",
        "",
        "",
        "",
        "",
        "",
        "",
        "\"NEED_TARGET|TOGGLE\"|USE_HP",
        "DAGGER|\"BOW",
        "stab",
        "",
        "2",
        "",
        "",
        "a",
        "",
        "max",
        "b",
        "",
        "",
    ]);
    skill_desc += &skill_desc_line(&["3", "dragon", "Unknown"]);
    skill_desc += &skill_desc_line(&["0", "WARRIOR", "zero"]);
    // A vnum alone: skipped before any other column is read.
    skill_desc += "4\n";
    skill_desc += &skill_desc_line(&[
        "34", "SUPPORT", "Support", "", "", "", "", "", "", "", "", "", "s", "70000", "0", "", "", "aff", "min", "max",
        "", "", "", "", "", "", "", "3",
    ]);
    skill_desc += &skill_desc_line(&[
        "1",
        "WARRIOR",
        "Slash again",
        "",
        "",
        "",
        "",
        "",
        "",
        "",
        "TOGGLE",
        "FAN",
        "slash",
        "",
        "1",
        "",
        "",
        "second",
        "m",
        "M",
    ]);
    skill_desc += &skill_desc_line(&["130", "horse", "Ride", "", "", "", "", "", "", "", "", "", "ride", "1"]);
    skill_desc += &skill_desc_line(&[
        "151", "guild", "Guild", "", "", "", "", "", "", "", "", "", "g", "", "", "", "", "aff", "x", "y", "", "", "",
        "", "", "", "30", "",
    ]);
    let mut skill_table = String::new();
    skill_table += &skill_table_line("1", "-1", "7", "number(atk, mwep)*1.5");
    skill_table += &skill_table_line("2", "30", "5", "lv*3");
    skill_table += &skill_table_line("34", "1", "", "atk");
    skill_table += &skill_table_line("151", "40", "1", "number(1,2)");
    skill_table += &skill_table_line("99", "40", "1", "atk");
    skill_table += "130\tshort\tline\n";
    skill_table += &skill_table_line("130", "", "", "");

    let insult = b"bad\n\nword\r\n\xb0\xa1\xb0\xa2\nlast".to_vec();
    let mut cvt: Vec<u8> = (b'a'..=b'z').rev().chain(b'A'..=b'Z').collect();
    cvt.extend(deterministic_bytes(2350 * 2 + 10, 99));

    let common = |item_proto: Vec<u8>, skill_desc: String| {
        vec![
            ("item_list.txt", item_list.clone()),
            ("item_proto", item_proto),
            ("itemdesc.txt", item_desc.clone()),
            ("mob_proto", mob_proto.clone()),
            ("skilldesc.txt", skill_desc.into_bytes()),
            ("skilltable.txt", skill_table.clone().into_bytes()),
            ("insult.txt", insult.clone()),
        ]
    };
    let mut xx = common(item_proto_mipt, skill_desc.clone());
    xx.push(("lang1.cvt", cvt.clone()));
    xx.push(("lang3.cvt", cvt[..4751].to_vec()));
    write_locale(&root, "xx", &xx);
    // A grade count above 4 stops RegisterSkillDesc, and LoadLocaleData.
    let stop = skill_desc
        + &skill_desc_line(&[
            "7", "sura", "Too many", "", "", "", "", "", "", "", "", "", "t", "1", "5",
        ]);
    write_locale(&root, "yy", &common(item_proto_mipx, stop));
    // Missing and corrupt files.
    let mut bad_proto = b"MIPX".to_vec();
    bad_proto.extend(words(&[2, 156, 0, 0]));
    write_locale(
        &root,
        "zz",
        &[("item_proto", bad_proto), ("mob_proto", b"MMPT".to_vec())],
    );

    for (locale, steps) in [("xx", 10), ("yy", 5), ("zz", 2)] {
        let path = format!("locale/{locale}");
        let files = Files::locale(&path);
        for ymir in [false, true] {
            let lines = assert_same_tables(&root, "pack/Index.dev", ymir, &files);
            eprintln!("synthetic {locale} (ymir {ymir}): {lines} lines identical");
        }
        assert_eq!(steps_ok(&root, "pack/Index.dev", true, &files).len(), steps, "{locale}");
    }
    // An item_proto that is not encrypted: CLZObject ignores the key.
    let plain = cpp_lzpack(tmp.path(), &records, None);
    let mut proto = b"MIPT".to_vec();
    proto.extend(words(&[7, plain.len() as u32]));
    proto.extend(&plain);
    write_locale(&root, "ww", &[("item_proto", proto)]);
    let files = Files::locale("locale/ww");
    assert_same_tables(&root, "pack/Index.dev", false, &files);
}
