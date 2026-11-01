//! Builds `neoeter_python_abi` (the C++ Python host behind a C ABI) with the
//! repository's CMake build and links it.

use std::path::PathBuf;

fn main() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repo = repo.canonicalize().expect("repository root");

    // The library pulls in the engine archives, so engine changes rebuild it.
    for path in ["CMakeLists.txt", "neoeter", "neoeter-python"] {
        println!("cargo:rerun-if-changed={}", repo.join(path).display());
    }

    let dst = cmake::Config::new(&repo)
        .generator("Ninja")
        .profile("RelWithDebInfo")
        .define("M2_BUILD_PYTHON_ABI", "ON")
        .define("M2_UNITY_BUILD", "ON")
        .define("BUILD_TESTS", "OFF")
        .define("BUILD_TESTING", "OFF")
        .build_target("neoeter_python_abi")
        .build();

    // build_target skips the install step; the library stays in the tree.
    let lib_dir = dst.join("build/src/abi");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=neoeter_python_abi");
    // rustc-link-arg reaches only this package's tests; binaries elsewhere
    // add the rpath from DEP_NEOETER_PYTHON_ABI_LIB_DIR.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    println!("cargo:lib_dir={}", lib_dir.display());
}
