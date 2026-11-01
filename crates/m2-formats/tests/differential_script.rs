//! Differential tests: the Rust `CTextFileLoader` / `LoadMultipleTextData`
//! ports against the C++ ones (`tools/differential/script`). Both sides load
//! every registered file with the given extensions and print the whole tree,
//! including `atoi`/`atof` of every token.
//!
//! Needs the client and the C++ tool, so the tests are ignored by default;
//! run them with `scripts/differential.sh`.

mod support;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

use m2_formats::pack::{PackManager, SearchMode, load_index};
use m2_formats::script::{self, Node, atof, atoi, name_key, to_f32};
use support::differential::{assert_same_lines, client_dir, cpp_tool, run};

/// Extensions of files loaded through `CTextFileLoader` (`.msm` race and
/// item data, `.msa` motions, `.mse` effects, `.msf` flying data, `.mss`
/// sounds, `.msenv` environments) and other text the client ships.
const TREE_EXTS: &str = "msm,msa,mse,msf,mss,msenv,txt,prt,pra,prb,prd,pre,sub,msk,scc";
/// Extensions read with `LoadMultipleTextData` (map/area `.txt`) or text
/// in that shape.
const MULTI_EXTS: &str = "txt,msenv,prt,prb,prd,pre,pra";

fn extension(name: &[u8]) -> Vec<u8> {
    match name.iter().rposition(|&b| b == b'.') {
        Some(dot) => name[dot + 1..].to_ascii_lowercase(),
        None => Vec::new(),
    }
}

fn put_token(out: &mut Vec<u8>, token: &[u8], numbers: bool) {
    out.extend_from_slice(b"T\t");
    out.extend_from_slice(token);
    if numbers {
        let d = atof(token);
        write!(out, "\t{}\t{:016x}\t{:08x}", atoi(token), d.to_bits(), to_f32(d).to_bits()).unwrap();
    }
    out.push(b'\n');
}

fn dump_node(out: &mut Vec<u8>, node: &Node, depth: usize) {
    write!(out, "G\t{depth}\t").unwrap();
    out.extend_from_slice(&node.name);
    out.push(b'\n');
    // C++ keeps the keys in a std::map by hash.
    let mut entries: Vec<_> = node.entries.iter().collect();
    entries.sort_by_key(|e| name_key(&e.key));
    for entry in entries {
        writeln!(out, "K\t{depth}\t{:08x}\t{}", name_key(&entry.key), entry.tokens.len()).unwrap();
        for token in &entry.tokens {
            put_token(out, token, true);
        }
    }
    for child in &node.children {
        dump_node(out, child, depth + 1);
    }
}

/// Whether C++ `CMemoryTextFileLoader::Bind` reads past the end of `data`
/// (a lead byte >= 0x80 as the last byte). What it reads there is undefined,
/// so such files are skipped on both sides; `text::lines` clamps instead.
fn bind_overreads(data: &[u8]) -> bool {
    let mut pos = 0;
    while pos < data.len() {
        pos += 1;
        if data[pos - 1] >= 0x80 {
            if pos == data.len() {
                return true;
            }
            pos += 1;
        }
    }
    false
}

type Counts = BTreeMap<String, (usize, usize, usize)>;

/// Same output as `m2diff_script`. Returns it and, per extension, the
/// number of files, how many loaded completely, and how many were skipped.
fn rust_dump(client: &Path, index: &str, mode: &str, exts: &str) -> (Vec<u8>, Counts) {
    let mut manager = PackManager::new(client);
    manager.set_search_mode(SearchMode::PackFirst);
    load_index(&mut manager, "pack", index).unwrap();

    let exts: Vec<&[u8]> = exts.split(',').map(str::as_bytes).collect();
    let mut names: Vec<&[u8]> = manager.file_names().filter(|n| exts.contains(&extension(n).as_slice())).collect();
    names.sort_unstable();

    let mut out = Vec::new();
    let mut counts = BTreeMap::new();
    for name in names {
        let data = manager.get(name).unwrap();
        out.extend_from_slice(b"F\t");
        out.extend_from_slice(name);
        let count: &mut (usize, usize, usize) =
            counts.entry(String::from_utf8_lossy(&extension(name)).into_owned()).or_default();
        count.0 += 1;
        if bind_overreads(&data) {
            out.extend_from_slice(b"\tskip\n");
            count.2 += 1;
        } else if mode == "tree" {
            let parsed = script::parse(&data);
            writeln!(out, "\t{}", u8::from(parsed.complete)).unwrap();
            count.1 += usize::from(parsed.complete);
            dump_node(&mut out, &parsed.root, 0);
        } else {
            out.extend_from_slice(b"\t1\n");
            count.1 += 1;
            for (key, tokens) in script::parse_multiple_text_data(&data) {
                out.extend_from_slice(b"K\t");
                out.extend_from_slice(&key);
                writeln!(out, "\t{}", tokens.len()).unwrap();
                for token in &tokens {
                    put_token(&mut out, token, false);
                }
            }
        }
    }
    (out, counts)
}

fn assert_same_dump(client: &Path, index: &str, mode: &str, exts: &str) -> Counts {
    let args = [client.as_os_str(), index.as_ref(), mode.as_ref(), exts.as_ref()];
    let cpp = run(&cpp_tool("script"), &args);
    let (rust, counts) = rust_dump(client, index, mode, exts);
    let lines = assert_same_lines(&cpp, &rust);
    eprintln!("{mode}: {lines} lines identical");
    for (ext, (files, complete, skipped)) in &counts {
        eprintln!("  .{ext}: {files} files, {complete} complete, {skipped} skipped");
    }
    counts
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_scripts_match_cpp() {
    let counts = assert_same_dump(&client_dir(), "pack/Index.dev", "tree", TREE_EXTS);
    assert!(counts.get("msa").is_some_and(|c| c.0 > 0), "no .msa files found");
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn real_multiple_text_data_matches_cpp() {
    let counts = assert_same_dump(&client_dir(), "pack/Index.dev", "multi", MULTI_EXTS);
    assert!(counts.get("txt").is_some_and(|c| c.0 > 0), "no .txt files found");
}

/// Inputs the real data may not have: failure paths, odd tokens, numbers.
const SYNTHETIC: &[(&str, &[u8])] = &[
    ("basic.msa", b"ScriptType MotionData\r\nMotionDuration 1.5\r\nGroup Event00\r\n{\r\n\tStartingTime 0.2\r\n\tList Pos\r\n\t{\r\n\t\t1 2 3\r\n\t\t4\t5 6\r\n\t}\r\n}\r\n"),
    ("quotes.mse", b"Key \"a b\" \"\" c\"d\" \"x\"y\nBad \"open\nAfter 1\n\"\" empty-key\n"),
    ("dupes.msm", b"key 1\nKEY 2\nkey 3\nGroup A\n{\n}\nGroup a\n{\n x 1\n}\n"),
    ("unbalanced_close.mse", b"a 1\n}\nb 2\n"),
    ("unbalanced_open.mse", b"a 1\nGroup G\n{\nb 2\n"),
    ("missing_value_top.msa", b"a 1\nlonely\nb 2\n"),
    ("missing_value_child.msa", b"Group G\n{\nx 1\nlonely\ny 2\n}\nz 3\n"),
    ("missing_value_braceless.msa", b"Group G\nx 1\nlonely\ny 2\n}\nz 3\n"),
    ("group_variants.msa", b"Group\nGroup A B\ngroup C {\n}\nGROUP D\n{ ignored tokens\nk v\n}\nList\nList L1 L2\n"),
    ("lists.mse", b"List L\n{ 1 2\n 3 \"4 5\" \"bad\n }\nList Unclosed\n1 2\n"),
    ("nested.msf", b"Group A\n{\n Group B\n {\n  Group C\n  {\n   k 1\n  }\n }\n m 2\n}\nGroup A\n{\n n 3\n}\n"),
    ("braces_inline.msa", b"{abc\nk 1\n}xyz\nk2 2\n"),
    ("numbers.txt", b"n 0 -0 +1 42 -17abc 1.9 x1 2147483648 4294967297 99999999999999999999 -99999999999999999999\nf 1.5 .5 5. 1e3 1e 1e+ 2E-1 . - 0.1 1e400 1e-400 3.4028235e38 3.4028236e38 1e-45 7e-46\nh 0x10 0x1.8p1 0x.8 -0x1p-2 0x 0xg 0x1p 0x1p-1074 0x1p-1075 0x1.0000000000001p-1075 0x1p1024 0x20000000000001 0x20000000000003 0x200000000000010000001 0X1P+3\ns inf -Infinity NaN -nan nanx infx in 1.7976931348623157e308 2.2250738585072011e-308 123456789012345678901234567890\n"),
    ("multi.txt", b"ScriptType MapSetting\nCellScale 200\nStart TextureSet\n\tTextureSet/A.txt\nEnd\nstart Other\nFirst Second\nEND\nonly\ncellscale 300\nStart Unclosed\nx y\n"),
    ("cp949.mss", b"Key \xb0\x0a\xb0 value\n\xbf\xeb\xb8\xb6 1\nGroup \xC7\xD1\n{\n}\n"),
    ("empty.msa", b""),
    ("tail.msm", b"k v"),
    ("vtab.msa", b"k \x0b12 \x0c1.5\n"),
];

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn synthetic_scripts_match_cpp() {
    let fixture = tempfile::tempdir().unwrap();
    let pack = fixture.path().join("pack/synth");
    fs::create_dir_all(&pack).unwrap();
    for (name, data) in SYNTHETIC {
        fs::write(pack.join(name), data).unwrap();
    }
    fs::write(fixture.path().join("pack/Index"), "synth\tFOLDER\n").unwrap();

    let exts = "msa,mse,msm,msf,mss,txt";
    let counts = assert_same_dump(fixture.path(), "pack/Index", "tree", exts);
    assert_eq!(counts.values().map(|c| c.0).sum::<usize>(), SYNTHETIC.len());
    assert_same_dump(fixture.path(), "pack/Index", "multi", exts);
}
