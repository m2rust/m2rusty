//! `m2-modgen` command line. Run from the repository root.
//!
//!   m2-modgen extract [--repo .] [--out modules]
//!   m2-modgen generate [--specs modules] [--out modules/generated] [--prefix ""]
//!   m2-modgen conformance [--repo .] [--specs modules]
//!   m2-modgen report [--repo .] [--specs modules] [--out crates/m2-modgen/MODULES.md]

use m2_modgen::{conformance, extract, gen_cpp, gen_rust, load_specs, report, snapshot, spec_file_name};
use std::path::PathBuf;

fn arg(args: &[String], name: &str, default: &str) -> String {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| default.to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let repo = PathBuf::from(arg(&args, "--repo", "."));
    let specs_dir = PathBuf::from(arg(&args, "--specs", "modules"));
    let result: Result<(), String> = (|| match cmd {
        "extract" => {
            let out = PathBuf::from(arg(&args, "--out", "modules"));
            let snap = snapshot::load(&repo).map_err(|e| e.to_string())?;
            let ex = extract::extract(&repo, &snap);
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            for s in &ex.specs {
                std::fs::write(out.join(spec_file_name(&s.module)), s.to_toml()).map_err(|e| e.to_string())?;
            }
            eprintln!(
                "{} specs; {} dynamic constant sites; {} table entries without body",
                ex.specs.len(),
                ex.dynamic_constants,
                ex.missing_bodies.len()
            );
            Ok(())
        }
        "generate" => {
            let out = PathBuf::from(arg(&args, "--out", "modules/generated"));
            let prefix = arg(&args, "--prefix", "");
            let specs = load_specs(&specs_dir)?;
            gen_cpp::write_all(&specs, &out, &prefix)?;
            gen_rust::write_all(&specs, &out)?;
            Ok(())
        }
        "conformance" => {
            let snap = snapshot::load(&repo).map_err(|e| e.to_string())?;
            let specs = load_specs(&specs_dir)?;
            let d = conformance::compare(&specs, &snap);
            println!("compared {} names", d.compared);
            for (t, v) in [("missing", &d.missing), ("extra", &d.extra), ("kind mismatch", &d.kind_mismatch), ("conditional (info)", &d.conditional_extra)] {
                println!("{t}: {}", v.len());
                for l in v {
                    println!("  {l}");
                }
            }
            if d.is_clean() { Ok(()) } else { Err("conformance diffs".into()) }
        }
        "report" => {
            let out = PathBuf::from(arg(&args, "--out", "crates/m2-modgen/MODULES.md"));
            let snap = snapshot::load(&repo).map_err(|e| e.to_string())?;
            let specs = load_specs(&specs_dir)?;
            let d = conformance::compare(&specs, &snap);
            std::fs::write(out, report::render(&specs, &d)).map_err(|e| e.to_string())
        }
        _ => {
            eprintln!("usage: m2-modgen extract|generate|conformance|report [options]; see crates/m2-modgen/README.md");
            Ok(())
        }
    })();
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
