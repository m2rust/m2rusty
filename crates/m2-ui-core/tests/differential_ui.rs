//! Differential tests: the Rust UI core against the real C++ window manager,
//! window classes, slot windows and text instance (`m2diff_ui`).
//!
//! - hand-written scenarios (`tests/scenarios/*.txt`) and seeded random ones
//!   (`M2_UI_SEEDS`, default 300) are replayed by both sides; hit results,
//!   positions, captures, focus, slot state and every event fired must agree;
//! - a corpus of real locale strings (and tagged variants of them) goes through
//!   the tag parser and the font-independent text layout of both sides.
//!
//! Needs the C++ tool (and, for the corpus, the client's locale packs), so
//! the tests are ignored by default; run them with `scripts/differential.sh`.

mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use support::differential::{assert_same_lines, client_dir, cpp_tool, run};
use support::randgen::{Rng, generate};
use support::text::dump_corpus;
use support::{Scenario, hex};

fn scratch(name: &str, contents: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!("m2ui-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{}-{name}", N.fetch_add(1, Ordering::SeqCst)));
    fs::write(&path, contents).unwrap();
    path
}

/// Line prefixes the dumper itself prints (see `tools/differential/ui/UiDump.cpp`
/// and [`Scenario::run`]). The engine traces to stdout as well — the Python
/// launcher's path, `CResource::Load file not exist …`, `SYSERR: [UI] …`, font
/// loading — and none of that is part of the dump.
const DUMP_PREFIXES: &[&str] = &[
    "w ", " w ", "mgr ", "slotwin ", "s ", "> ", "ev ", "text ", "t ", "textpos ", "pickwin ", "picktop ",
    "mgrpick ", "out ", "q ", "!!",
];

fn is_dump_line(line: &str) -> bool {
    DUMP_PREFIXES.iter().any(|p| line.starts_with(p))
}

fn cpp(mode: &str, input: &str) -> Vec<u8> {
    let path = scratch("in.txt", input);
    let raw = run(&cpp_tool("ui"), &[mode.as_ref(), path.as_os_str()]);
    let _ = fs::remove_file(&path);
    String::from_utf8_lossy(&raw)
        .lines()
        .filter(|l| is_dump_line(l))
        .map(|l| format!("{l}\n"))
        .collect::<String>()
        .into_bytes()
}

fn check_scenario(name: &str, script: &str) -> usize {
    let cpp_out = cpp("scenario", script);
    let rust_out = Scenario::new().run(script);
    let (c, r) = (String::from_utf8_lossy(&cpp_out).into_owned(), rust_out);
    if let Some((i, (a, b))) = c.lines().zip(r.lines()).enumerate().find(|(_, (a, b))| a != b) {
        let from = c.lines().take(i).collect::<Vec<_>>();
        let ctx = from.iter().rev().filter(|l| l.starts_with("> ")).take(1).next().copied().unwrap_or("");
        let path = scratch(&format!("{name}-failing.txt"), script);
        panic!("{name}: dumps differ at line {} (after `{ctx}`)\n  c++:  {a}\n  rust: {b}\n  script: {}", i + 1, path.display());
    }
    assert_eq!(c.lines().count(), r.lines().count(), "{name}: dumps differ in length");
    c.lines().count()
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn hand_written_scenarios_match_cpp() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut total = 0;
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let script = fs::read_to_string(&p).unwrap();
        let n = check_scenario(p.file_name().unwrap().to_str().unwrap(), &script);
        eprintln!("{}: {n} lines identical", p.display());
        total += n;
    }
    assert!(total > 0);
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn random_scenarios_match_cpp() {
    let seeds: u64 = std::env::var("M2_UI_SEEDS").ok().and_then(|s| s.parse().ok()).unwrap_or(300);
    let first: u64 = std::env::var("M2_UI_FIRST_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(1);
    let next = AtomicUsize::new(0);
    let lines = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..std::thread::available_parallelism().map_or(4, |n| n.get().min(8)) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::SeqCst) as u64;
                    if k >= seeds {
                        break;
                    }
                    let seed = first + k;
                    let steps = 80 + (seed % 5) as usize * 60;
                    let n = check_scenario(&format!("seed{seed}"), &generate(seed, steps));
                    lines.fetch_add(n, Ordering::SeqCst);
                }
            });
        }
    });
    eprintln!("{seeds} random scenarios, {} dump lines identical", lines.load(Ordering::SeqCst));
}

/// Code page of each locale pack (`locale_<xx>` in `bin/pack`).
const LOCALES: &[(&str, u32)] = &[
    ("cz", 1250), ("de", 1252), ("dk", 1252), ("en", 1252), ("es", 1252), ("fr", 1252), ("gr", 1253), ("hu", 1250),
    ("it", 1252), ("nl", 1252), ("pl", 1250), ("pt", 1252), ("ro", 1250), ("ru", 1251), ("tr", 1254),
];

/// Real strings from the locale packs, plain and wrapped in tags.
fn build_corpus() -> String {
    let pack = client_dir().join("pack");
    let mut rng = Rng::new(7);
    let mut corpus = String::new();
    let mut count = 0;
    for (loc, cp) in LOCALES {
        for file in ["locale_game.txt", "locale_interface.txt", "itemdesc.txt"] {
            let path = pack.join(format!("locale_{loc}/locale/{loc}/{file}"));
            let Ok(data) = fs::read(&path) else { continue };
            for line in data.split(|b| *b == b'\n') {
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                let value: &[u8] = match line.iter().position(|b| *b == b'\t') {
                    Some(t) => &line[t + 1..],
                    None => line,
                };
                if value.is_empty() || value.len() > 200 || rng.below(4) != 0 {
                    continue;
                }
                let mut variants: Vec<Vec<u8>> = vec![value.to_vec()];
                // Tagged variants, as the server and the Python code build them.
                let mut v = b"|cFFFFD700".to_vec();
                v.extend_from_slice(value);
                v.extend_from_slice(b"|r");
                variants.push(v);
                let mut v = Vec::new();
                let cut = value.len() / 2;
                v.extend_from_slice(&value[..cut]);
                v.extend_from_slice(b"|Hitem:1a2b:0:0:0|h[");
                v.extend_from_slice(&value[cut..]);
                v.extend_from_slice(b"]|h tail||x |cFF00FF00ok|r");
                variants.push(v);
                if rng.below(8) == 0 {
                    let mut v = value.to_vec();
                    v.extend_from_slice(b" @1252 abc @0949 def @9999 end");
                    variants.push(v);
                }
                if rng.below(8) == 0 {
                    let mut v = b"|c12|cZZZZZZZZ".to_vec();
                    v.extend_from_slice(value);
                    v.extend_from_slice(b"|Hx|h|h|");
                    variants.push(v);
                }
                for v in variants {
                    corpus.push_str(&format!("{cp} {}\n", hex(&v)));
                    count += 1;
                }
            }
        }
    }
    eprintln!("corpus: {count} strings");
    corpus
}

#[test]
#[ignore = "differential: run scripts/differential.sh"]
fn text_corpus_matches_cpp() {
    let corpus = build_corpus();
    assert!(!corpus.is_empty());
    let cpp_out = cpp("text", &corpus);
    let rust_out = dump_corpus(&corpus);
    let n = assert_same_lines(&cpp_out, rust_out.as_bytes());
    eprintln!("text corpus: {n} lines identical");
}
