//! Conformance of `modules/*.toml` with the API snapshots, determinism of the
//! extractor, and that the generated code for ALL modules compiles.

use m2_modgen::{conformance, extract, gen_cpp, gen_rust, load_specs, snapshot};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

#[test]
fn specs_export_the_same_names_as_the_api_snapshots() {
    let repo = repo();
    let snap = snapshot::load(&repo).unwrap();
    let specs = load_specs(&repo.join("modules")).unwrap();
    let d = conformance::compare(&specs, &snap);
    println!("compared {} names; conditional-only: {:?}", d.compared, d.conditional_extra);
    assert!(d.is_clean(), "missing: {:?}\nextra: {:?}\nkind mismatch: {:?}", d.missing, d.extra, d.kind_mismatch);
    // Every snapshot module and every name is present.
    let total: usize = snap.values().map(|m| m.len()).sum();
    assert!(d.compared >= total, "{} compared < {} in the snapshots", d.compared, total);
}

#[test]
fn extraction_is_deterministic_and_matches_committed_specs_in_shape() {
    let repo = repo();
    let snap = snapshot::load(&repo).unwrap();
    let a = extract::extract(&repo, &snap);
    let b = extract::extract(&repo, &snap);
    let ta: Vec<String> = a.specs.iter().map(|s| s.to_toml()).collect();
    let tb: Vec<String> = b.specs.iter().map(|s| s.to_toml()).collect();
    assert_eq!(ta, tb);
    // Same module set as the committed specs.
    let committed = load_specs(&repo.join("modules")).unwrap();
    let names = |v: &[m2_modgen::spec::ModuleSpec]| -> Vec<String> { v.iter().map(|s| s.module.clone()).collect() };
    let mut x = names(&a.specs);
    x.sort();
    let mut y = names(&committed);
    y.sort();
    assert_eq!(x, y);
}

#[test]
fn committed_specs_round_trip_through_toml() {
    let specs = load_specs(&repo().join("modules")).unwrap();
    for s in &specs {
        let text = s.to_toml();
        let back = m2_modgen::spec::ModuleSpec::parse(&text).unwrap();
        assert_eq!(back.to_toml(), text, "{}", s.module);
        for f in &s.functions {
            f.parsed_overloads().unwrap_or_else(|e| panic!("{}.{}: {e}", s.module, f.name));
        }
    }
}

#[test]
fn generated_code_for_all_modules_compiles() {
    let repo = repo();
    let specs = load_specs(&repo.join("modules")).unwrap();
    let work = tempfile::tempdir().unwrap();
    gen_cpp::write_all(&specs, work.path(), "").unwrap();
    gen_rust::write_all(&specs, work.path()).unwrap();

    // Rust: one library, no dependencies.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(rustc)
        .args(["--edition", "2024", "--crate-type", "lib", "--emit", "metadata", "-o"])
        .arg(work.path().join("glue.rmeta"))
        .arg(work.path().join("rust/mod.rs"))
        .output()
        .unwrap();
    assert!(out.status.success(), "rustc:\n{}", String::from_utf8_lossy(&out.stderr));

    // C++: syntax-check every shim against the vendored Python 2.7 headers.
    if Command::new("g++").arg("--version").output().is_err() {
        eprintln!("skipped C++ syntax check: g++ not found (run through nix-shell)");
        return;
    }
    let py = repo.join("neoeter-python/include/python2.7");
    let mut n = 0;
    for e in std::fs::read_dir(work.path().join("cpp")).unwrap().flatten() {
        let out = Command::new("g++")
            .args(["-std=c++20", "-w", "-fsyntax-only", "-I"])
            .arg(work.path().join("include"))
            .arg("-I")
            .arg(&py)
            .arg(e.path())
            .output()
            .unwrap();
        assert!(out.status.success(), "{}:\n{}", e.path().display(), String::from_utf8_lossy(&out.stderr));
        n += 1;
    }
    assert!(n >= 30, "only {n} shims");
}
