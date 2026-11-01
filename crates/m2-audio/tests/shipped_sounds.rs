//! Every `.wav`/`.mp3` under `bin/` decodes (skipped without the client data).

use std::path::Path;

use m2_audio::M2Sound;

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("wav") || e.eq_ignore_ascii_case("mp3")) {
            out.push(path);
        }
    }
}

#[test]
fn shipped_sounds_decode() {
    let bin = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bin");
    let mut files = Vec::new();
    walk(&bin, &mut files);
    if files.is_empty() {
        eprintln!("no client data under {}; skipped", bin.display());
        return;
    }
    let failed: Vec<_> = files
        .iter()
        .filter(|path| {
            let hint = if path.extension().unwrap().eq_ignore_ascii_case("mp3") { "mp3" } else { "wav" };
            M2Sound::new(std::fs::read(path).unwrap(), hint).is_err()
        })
        .collect();
    eprintln!("{} sounds, {} failed", files.len(), failed.len());
    assert!(failed.is_empty(), "{failed:#?}");
}
