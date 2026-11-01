//! Rust counterpart of `m2diff_pack` (`tools/differential/pack/PackDump.cpp`).
//!
//!   m2-pack-dump <client dir> [index file, default pack/Index]
//!
//! Prints `name<TAB>size<TAB>fnv1a64` for every registered file, sorted by
//! name, reading archive-first. Unreadable files print `-` for both.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use m2_formats::pack::{PackManager, SearchMode, load_index};

fn fnv1a64(data: &[u8]) -> u64 {
    data.iter()
        .fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let Some(client_dir) = args.get(1) else {
        eprintln!("usage: {} <client dir> [index file]", args[0]);
        return ExitCode::from(2);
    };
    let index = args.get(2).map_or("pack/Index", String::as_str);

    let mut manager = PackManager::new(client_dir);
    manager.set_search_mode(SearchMode::PackFirst);
    let report = match load_index(&mut manager, "pack", index) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("{index}: {e}");
            return ExitCode::FAILURE;
        }
    };
    for (pack, e) in &report.failed {
        eprintln!("cannot register {pack}: {e}");
    }

    let mut names: Vec<&[u8]> = manager.file_names().collect();
    names.sort_unstable();

    let mut out = BufWriter::new(std::io::stdout().lock());
    for name in names {
        let result = out.write_all(name).and_then(|()| match manager.get(name) {
            Ok(data) => writeln!(out, "\t{}\t{:016x}", data.len(), fnv1a64(&data)),
            Err(_) => writeln!(out, "\t-\t-"),
        });
        if result.is_err() {
            // Closed pipe (e.g. `| head`).
            return ExitCode::SUCCESS;
        }
    }
    ExitCode::SUCCESS
}
