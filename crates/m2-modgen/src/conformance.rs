//! Compares the exported names of the specs with the API snapshots.

use crate::snapshot::Snapshot;
use crate::spec::ModuleSpec;
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Diff {
    /// In the snapshot, not in the specs.
    pub missing: Vec<String>,
    /// In the specs, not in the snapshot (unconditional entries).
    pub extra: Vec<String>,
    /// In the specs only, guarded by an `#if` (build-dependent, informational).
    pub conditional_extra: Vec<String>,
    /// Same name, different kind (callable vs constant, int vs float).
    pub kind_mismatch: Vec<String>,
    pub compared: usize,
}

impl Diff {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty() && self.extra.is_empty() && self.kind_mismatch.is_empty()
    }
}

pub fn compare(specs: &[ModuleSpec], snap: &Snapshot) -> Diff {
    let mut d = Diff::default();
    let spec_mods: BTreeSet<&str> = specs.iter().filter(|s| !s.module.starts_with('(')).map(|s| s.module.as_str()).collect();
    for m in snap.keys() {
        if !spec_mods.contains(m.as_str()) {
            d.missing.push(format!("module {m}"));
        }
    }
    for s in specs.iter().filter(|s| !s.module.starts_with('(')) {
        let Some(items) = snap.get(&s.module) else {
            d.extra.push(format!("module {}", s.module));
            continue;
        };
        let mut seen = BTreeSet::new();
        for f in &s.functions {
            seen.insert(f.name.clone());
            d.compared += 1;
            match items.get(&f.name) {
                None if f.cfg.is_empty() => d.extra.push(format!("{}.{}", s.module, f.name)),
                None => d.conditional_extra.push(format!("{}.{} [{}]", s.module, f.name, f.cfg)),
                Some(it) if it.kind != "callable" => d.kind_mismatch.push(format!("{}.{}: spec function, snapshot {}", s.module, f.name, it.kind)),
                _ => {}
            }
        }
        for c in &s.constants {
            seen.insert(c.name.clone());
            d.compared += 1;
            match items.get(&c.name) {
                None if c.cfg.is_empty() => d.extra.push(format!("{}.{} (constant)", s.module, c.name)),
                None => d.conditional_extra.push(format!("{}.{} [{}]", s.module, c.name, c.cfg)),
                Some(it) if it.kind != c.ty && c.ty != "object" => {
                    d.kind_mismatch.push(format!("{}.{}: spec {}, snapshot {}", s.module, c.name, c.ty, it.kind))
                }
                Some(it) if !c.value.is_empty() && it.value != c.value => {
                    d.kind_mismatch.push(format!("{}.{}: value {} vs {}", s.module, c.name, c.value, it.value))
                }
                _ => {}
            }
        }
        for name in items.keys() {
            if !seen.contains(name) {
                d.missing.push(format!("{}.{}", s.module, name));
            }
        }
    }
    d.missing.sort();
    d.extra.sort();
    d.conditional_extra.sort();
    d.kind_mismatch.sort();
    d
}
