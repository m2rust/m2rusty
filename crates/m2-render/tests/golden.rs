//! Golden conformance: renders the C++ renderer's 12 golden scenes headless
//! and compares them with the committed PNGs. Skips when no GPU adapter exists.

#[path = "golden/scenes.rs"]
mod scenes;

use scenes::*;

#[test]
fn golden_scenes() {
    let Some(mut rig) = Rig::try_new() else {
        eprintln!("SKIP golden_scenes: no GPU adapter available");
        return;
    };
    let out = output_dir();
    let mut failures = Vec::new();
    for scene in SCENES {
        let actual = rig.render(&scene.build);
        let golden = read_png(&golden_dir().join(format!("{}.png", scene.name)));
        let (stats, diff) = compare(&golden, &actual);
        eprintln!(
            "[golden] {:<18} max_delta={:>3} differing={:>4} ({:.3}%) off_edge={}",
            scene.name,
            stats.max_delta,
            stats.differing,
            stats.fraction() * 100.0,
            stats.off_edge
        );
        if std::env::var_os("M2_GOLDEN_DUMP").is_some() {
            write_png(&out.join(format!("{}.actual.png", scene.name)), &actual);
            write_png(&out.join(format!("{}.diff.png", scene.name)), &diff);
        }
        let ok = stats.max_delta <= scene.tolerance.max_delta
            && stats.fraction() <= scene.tolerance.max_fraction
            && stats.off_edge == 0;
        if !ok {
            let actual_path = out.join(format!("{}.actual.png", scene.name));
            let diff_path = out.join(format!("{}.diff.png", scene.name));
            write_png(&actual_path, &actual);
            write_png(&diff_path, &diff);
            eprintln!("[golden] FAIL {}: actual {} diff {}", scene.name, actual_path.display(), diff_path.display());
            failures.push(scene.name);
        }
    }
    assert!(failures.is_empty(), "golden scenes failed: {failures:?} (images in {})", out.display());
}
