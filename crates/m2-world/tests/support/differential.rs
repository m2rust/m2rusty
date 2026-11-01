//! Shared plumbing for `tests/differential_*.rs`: locating the client data
//! and the C++ reference dumpers built by `scripts/differential.sh`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn env_path(var: &str) -> PathBuf {
    let value = std::env::var_os(var)
        .unwrap_or_else(|| panic!("{var} is not set; run scripts/differential.sh"));
    // cargo runs tests from the crate directory; relative paths are taken
    // from the workspace root instead.
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(value)
}

/// The client directory (`M2_CLIENT_DIR`, e.g. `bin`).
pub fn client_dir() -> PathBuf {
    env_path("M2_CLIENT_DIR")
}

/// The C++ dumper `m2diff_<module>` (`tools/differential/<module>`).
pub fn cpp_tool(module: &str) -> PathBuf {
    env_path("M2_DIFF_BUILD").join(format!("tools/differential/{module}/m2diff_{module}"))
}

/// Runs `tool args…` and returns its stdout, panicking with stderr on failure.
pub fn run(tool: &Path, args: &[&std::ffi::OsStr]) -> Vec<u8> {
    let out = Command::new(tool).args(args).output().unwrap_or_else(|e| panic!("{}: {e}", tool.display()));
    assert!(
        out.status.success(),
        "{} failed: {}",
        tool.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// Asserts two line-oriented dumps are identical, reporting the first
/// differing line. Returns the number of lines.
pub fn assert_same_lines(cpp: &[u8], rust: &[u8]) -> usize {
    let cpp_lines: Vec<_> = cpp.split(|&b| b == b'\n').collect();
    let rust_lines: Vec<_> = rust.split(|&b| b == b'\n').collect();
    if let Some(i) = cpp_lines.iter().zip(&rust_lines).position(|(a, b)| a != b) {
        panic!(
            "dumps differ at line {}:\n  c++:  {}\n  rust: {}",
            i + 1,
            String::from_utf8_lossy(cpp_lines[i]),
            String::from_utf8_lossy(rust_lines[i])
        );
    }
    assert_eq!(cpp_lines.len(), rust_lines.len(), "dumps differ in length");
    cpp_lines.len().saturating_sub(1)
}
