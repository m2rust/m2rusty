//! Compiles the generated shims for `dbg`, `time`, `math` and `pack` into Python 2.7
//! extension modules (`gen_dbg`, ...) and runs `tests/real_build/compare.py` in the
//! repository's embedded host, which compares them with the hand-written `dbg`/`time`/`math`.
//!
//! Needs g++/gcc, the nix Python 2.7 and a built `libneoeter_python_abi.so` (from the
//! m2-script build). Skips with a message when something is missing (override the
//! search with M2_PYTHON_ABI_DIR).

use std::path::{Path, PathBuf};
use std::process::Command;

fn find_abi_dir() -> Option<(PathBuf, PathBuf)> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(d) = std::env::var("M2_PYTHON_ABI_DIR") {
        candidates.push(PathBuf::from(d));
    }
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(t) = std::env::var("CARGO_TARGET_DIR") {
        roots.push(PathBuf::from(t));
    }
    let mut p = Some(Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf());
    while let Some(d) = p {
        roots.push(d.join("target"));
        p = d.parent().map(|x| x.to_path_buf());
    }
    for r in roots {
        for profile in ["debug", "release"] {
            let build = r.join(profile).join("build");
            if let Ok(rd) = std::fs::read_dir(&build) {
                for e in rd.flatten() {
                    if e.file_name().to_string_lossy().starts_with("m2-script-") {
                        candidates.push(e.path().join("out/build/src/abi"));
                    }
                }
            }
        }
    }
    for abi in candidates {
        let host = abi.parent()?.join("host/libneoeter_python_host.a");
        if abi.join("libneoeter_python_abi.so").exists() && host.exists() {
            return Some((abi, host));
        }
    }
    None
}

fn run(cmd: &mut Command) -> Result<String, String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(text) } else { Err(format!("{cmd:?} failed:\n{text}")) }
}

#[test]
fn generated_shims_match_hand_written_modules() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let Some((abi_dir, host_a)) = find_abi_dir() else {
        eprintln!("skipped: no built libneoeter_python_abi.so (build crates/m2-script first or set M2_PYTHON_ABI_DIR)");
        return;
    };
    if Command::new("g++").arg("--version").output().is_err() {
        eprintln!("skipped: g++ not found (run through nix-shell)");
        return;
    }
    let py_inc = repo.join("neoeter-python/include/python2.7");
    let Some(py_lib_dir) = std::env::var("M2_PYTHON_LIB_DIR").ok().map(PathBuf::from).or_else(|| {
        // The nix shell puts python2.7 on PATH; its lib dir is next to bin.
        let out = Command::new("python2.7").arg("-c").arg("import sys; print sys.prefix").output().ok()?;
        Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim()).join("lib"))
    }) else {
        eprintln!("skipped: python2.7 not found");
        return;
    };
    if !py_lib_dir.join("libpython2.7.so").exists() {
        eprintln!("skipped: {} has no libpython2.7.so", py_lib_dir.display());
        return;
    }

    let work = tempfile::tempdir().unwrap();
    let w = work.path();
    // Specs -> generated code with the `gen_` module prefix.
    let specs = m2_modgen::load_specs(&repo.join("modules")).expect("modules/*.toml");
    let picked: Vec<_> = specs.into_iter().filter(|s| ["dbg", "time", "math", "pack"].contains(&s.module.as_str())).collect();
    assert_eq!(picked.len(), 4);
    m2_modgen::gen_cpp::write_all(&picked, w, "gen_").unwrap();

    // The host's real argument helpers (Py_BuildException, PyTuple_Get*) from the static host library.
    // (The host library is a unity build, so compile the real PythonUtils.cpp on its own.)
    let _ = &host_a;
    let utils_o = w.join("PythonUtils.o");
    run(Command::new("g++")
        .args(["-std=c++23", "-c", "-fPIC", "-w", "-O1"])
        .arg("-I")
        .arg(repo.join("neoeter-python/include"))
        .arg("-I")
        .arg(&py_inc)
        .arg("-I")
        .arg(repo.join("neoeter/include"))
        .arg(repo.join("neoeter-python/src/host/PythonUtils.cpp"))
        .arg("-o")
        .arg(&utils_o))
    .unwrap();

    let backend = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real_build/backend.cpp");
    for m in ["dbg", "time", "math", "pack"] {
        let so = w.join(format!("gen_{m}.so"));
        run(Command::new("g++")
            .args(["-std=c++20", "-shared", "-fPIC", "-w", "-O1"])
            .arg("-I")
            .arg(w.join("include"))
            .arg("-I")
            .arg(&py_inc)
            .arg(w.join(format!("cpp/{m}_shim.cpp")))
            .arg(&backend)
            .arg(&utils_o)
            .arg("-o")
            .arg(&so)
            .arg(format!("-L{}", py_lib_dir.display()))
            .arg("-lpython2.7")
            .arg("-Wl,--unresolved-symbols=ignore-in-object-files"))
        .unwrap_or_else(|e| panic!("{e}"));
    }

    let harness = w.join("harness");
    run(Command::new("gcc")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real_build/harness.c"))
        .arg("-I")
        .arg(repo.join("neoeter-python/include"))
        .arg(format!("-L{}", abi_dir.display()))
        .arg("-lneoeter_python_abi")
        .arg(format!("-Wl,-rpath,{}", abi_dir.display()))
        .arg("-o")
        .arg(&harness))
    .unwrap();

    let script = format!(
        "GEN_DIR = {:?}\n{}",
        w.to_string_lossy(),
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/real_build/compare.py")).unwrap()
    );
    let script_path = w.join("script.py");
    std::fs::write(&script_path, script).unwrap();
    let out = Command::new(&harness).current_dir(w).arg(&script_path).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    println!("{text}");
    assert!(out.status.success() && text.contains("ALL OK"), "comparison failed:\n{text}");
}
