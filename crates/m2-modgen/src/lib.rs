//! Spec extractor and generators for the Python bridge (docs/bevy-port-plan.md, phase 5).

pub mod abi;
pub mod analyze;
pub mod conformance;
pub mod extract;
pub mod gen_cpp;
pub mod gen_rust;
pub mod lex;
pub mod report;
pub mod snapshot;
pub mod spec;

use std::path::Path;

/// Reads every `*.toml` in `dir` (sorted by module name). The `(callbacks)`
/// pseudo-module holds callback sites no module owns.
pub fn load_specs(dir: &Path) -> Result<Vec<spec::ModuleSpec>, String> {
    let mut out = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.extension().is_some_and(|e| e == "toml") {
            let text = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
            out.push(spec::ModuleSpec::parse(&text).map_err(|e| format!("{}: {e}", p.display()))?);
        }
    }
    out.sort_by(|a, b| a.module.cmp(&b.module));
    Ok(out)
}

/// File name of a module spec: `player.toml`, `callbacks.toml` for the pseudo-module.
pub fn spec_file_name(module: &str) -> String {
    format!("{}.toml", module.trim_matches(|c| c == '(' || c == ')'))
}
