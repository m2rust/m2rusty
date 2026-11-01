//! Walks the C++ sources and builds one `ModuleSpec` per `Py_InitModule`.

use crate::analyze::analyze;
use crate::lex::{lex, match_brackets, Kind, Tok};
use crate::snapshot::Snapshot;
use crate::spec::{Callback, Constant, Function, ModuleSpec};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct FuncDef {
    file: usize,
    name_idx: usize,
    open: usize,
    close: usize,
    args_name: String,
}

struct TableEntry {
    name: String,
    cfunc: String,
    flags: String,
    cfg: String,
}

struct File {
    rel: String,
    toks: Vec<Tok>,
    m: Vec<Option<usize>>,
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            walk(&p, out);
        } else if p.extension().is_some_and(|e| e == "cpp") {
            out.push(p);
        }
    }
}

fn load_files(repo: &Path) -> Vec<File> {
    let mut paths = Vec::new();
    for d in ["client/src", "neoeter-python/src", "neoeter/src"] {
        walk(&repo.join(d), &mut paths);
    }
    paths
        .into_iter()
        .filter_map(|p| {
            let text = String::from_utf8_lossy(&std::fs::read(&p).ok()?).into_owned();
            if !text.contains("Py") {
                return None;
            }
            let toks = lex(&text);
            let m = match_brackets(&toks);
            Some(File { rel: p.strip_prefix(repo).unwrap().to_string_lossy().replace('\\', "/"), toks, m })
        })
        .collect()
}

fn find_funcs(fi: usize, f: &File, out: &mut BTreeMap<String, FuncDef>) {
    let t = &f.toks;
    let n = t.len();
    let mut i = 0;
    while i + 8 < n {
        // PyObject * name ( PyObject * [a] , PyObject * [b] ) {
        if t[i].is("PyObject") && t[i + 1].is("*") && t[i + 2].is_ident() && t[i + 3].is("(") {
            let close = f.m[i + 3].unwrap_or(0);
            if close > i && t.get(close + 1).is_some_and(|x| x.is("{")) {
                let inside = &t[i + 4..close];
                let two = inside.iter().filter(|x| x.is("PyObject")).count() == 2;
                if two {
                    let args_name = inside.last().filter(|x| x.is_ident() && !x.is("PyObject")).map(|x| x.text.clone()).unwrap_or_default();
                    let open = close + 1;
                    let body_close = f.m[open].unwrap_or(open);
                    out.entry(t[i + 2].text.clone()).or_insert(FuncDef { file: fi, name_idx: i + 2, open, close: body_close, args_name });
                    i = body_close;
                }
            }
        }
        i += 1;
    }
}

fn find_tables(f: &File) -> BTreeMap<String, Vec<TableEntry>> {
    let t = &f.toks;
    let mut out = BTreeMap::new();
    for i in 0..t.len().saturating_sub(5) {
        if t[i].is("PyMethodDef") && t[i + 1].is_ident() && t[i + 2].is("[") && t[i + 3].is("]") && t[i + 4].is("=") && t[i + 5].is("{") {
            let open = i + 5;
            let close = f.m[open].unwrap_or(open);
            let mut entries = Vec::new();
            let mut j = open + 1;
            while j < close {
                if t[j].is("{") {
                    let c = f.m[j].unwrap_or(j);
                    if t[j + 1].kind == Kind::Str {
                        let mut parts: Vec<&Tok> = Vec::new();
                        let mut k = j + 1;
                        while k < c {
                            if !t[k].is(",") {
                                parts.push(&t[k]);
                            }
                            k += 1;
                        }
                        if parts.len() >= 3 {
                            entries.push(TableEntry {
                                name: parts[0].text.clone(),
                                cfunc: parts[1].text.clone(),
                                flags: parts[2].text.clone(),
                                cfg: t[j].cfg.clone(),
                            });
                        }
                    }
                    j = c;
                }
                j += 1;
            }
            out.insert(t[i + 1].text.clone(), entries);
        }
    }
    out
}

fn module_for_file(rel: &str) -> &'static str {
    let lower = rel.to_lowercase();
    let stem = lower.rsplit('/').next().unwrap_or("");
    const TABLE: &[(&str, &str)] = &[
        ("pythonplayer", "player"),
        ("pythonmessenger", "messenger"),
        ("pythonguild", "guild"),
        ("pythonshop", "shop"),
        ("pythonsafebox", "safebox"),
        ("pythonexchange", "exchange"),
        ("pythonchat", "chat"),
        ("pythonnetworkstream", "net"),
        ("accountconnector", "net"),
        ("pythoncharactermanager", "chrmgr"),
        ("pythoncharacter", "chr"),
        ("pythonitem", "item"),
        ("pythonskill", "skill"),
        ("pythonquest", "quest"),
        ("pythonapplication", "app"),
        ("pythonsystem", "systemSetting"),
        ("pythonbackground", "background"),
        ("pythonminimap", "miniMap"),
        ("pythontexttail", "textTail"),
        ("pythoneffect", "effect"),
        ("pythonime", "ime"),
        ("pythonsound", "snd"),
        ("pythonwindow", "wndMgr"),
        ("pythonslotwindow", "wndMgr"),
        ("pythongridslotwindow", "wndMgr"),
        ("pythongraphicimage", "grpImage"),
        ("pythongraphictext", "grpText"),
        ("pythongraphicthing", "grpThing"),
        ("pythongraphic", "grp"),
        ("pythonnonplayer", "nonplayer"),
        ("pythoneventmanager", "event"),
        ("pythongameeventmanager", "eventMgr"),
        ("serverstatechecker", "ServerStateChecker"),
        ("pythondebug", "dbg"),
        ("pythonlauncher", "(host)"),
        ("pythonutils", "(host)"),
        ("pythonprofiler", "profiler"),
    ];
    for (k, v) in TABLE {
        if stem.starts_with(k) {
            return v;
        }
    }
    "(unattributed)"
}

fn enclosing_fn(f: &File, idx: usize) -> String {
    // Walk back to a `{` at brace depth 0 (namespaces are transparent) preceded by `)`.
    let t = &f.toks;
    let mut best = String::new();
    let mut i = 0;
    while i < idx {
        if t[i].is("{") {
            let c = f.m[i].unwrap_or(i);
            let is_ns = (0..i).rev().take(3).any(|k| t[k].is("namespace"));
            let is_fn = i > 0 && (t[i - 1].is(")") || t[i - 1].is("const") || t[i - 1].is("override"));
            if is_ns {
                i += 1;
                continue;
            }
            if c >= idx && is_fn {
                // name before matching '('
                let mut p = i - 1;
                while p > 0 && !t[p].is(")") {
                    p -= 1;
                }
                if let Some(o) = f.m[p] {
                    if o > 0 {
                        let mut q = o - 1;
                        let mut name = t[q].text.clone();
                        while q >= 2 && t[q - 1].is("::") {
                            name = format!("{}::{}", t[q - 2].text, name);
                            q -= 2;
                        }
                        best = name;
                    }
                }
                break;
            }
            i = c;
        }
        i += 1;
    }
    best
}

pub struct Extraction {
    pub specs: Vec<ModuleSpec>,
    /// Constant-registering calls whose name was not a literal.
    pub dynamic_constants: usize,
    /// Method-table entries whose function body could not be found.
    pub missing_bodies: Vec<String>,
    /// Callback sites that no module claims.
    pub unattributed_callbacks: usize,
}

pub fn extract(repo: &Path, snapshot: &Snapshot) -> Extraction {
    let files = load_files(repo);
    let mut funcs: BTreeMap<String, FuncDef> = BTreeMap::new();
    for (fi, f) in files.iter().enumerate() {
        find_funcs(fi, f, &mut funcs);
    }
    let mut specs: BTreeMap<String, ModuleSpec> = BTreeMap::new();
    let mut dynamic_constants = 0;
    let mut missing = Vec::new();
    for f in files.iter() {
        let tables = find_tables(f);
        let t = &f.toks;
        for i in 0..t.len().saturating_sub(4) {
            if !(t[i].is("Py_InitModule") && t[i + 1].is("(") && t[i + 2].kind == Kind::Str && t[i + 3].is(",")) {
                continue;
            }
            let modname = t[i + 2].text.clone();
            let table_var = t[i + 4].text.clone();
            let modvar = if i >= 2 && t[i - 1].is("=") { t[i - 2].text.clone() } else { String::new() };
            let Some(entries) = tables.get(&table_var) else { continue };
            let spec = specs.entry(modname.clone()).or_insert_with(|| ModuleSpec { module: modname.clone(), ..Default::default() });
            if !spec.sources.contains(&f.rel) {
                spec.sources.push(f.rel.clone());
            }
            for e in entries {
                let mut func = Function {
                    name: e.name.clone(),
                    cfunc: e.cfunc.clone(),
                    flags: if e.flags.contains("NOARGS") { "noargs".into() } else { "varargs".into() },
                    cfg: e.cfg.clone(),
                    ..Default::default()
                };
                match funcs.get(&e.cfunc) {
                    None => {
                        missing.push(format!("{}.{} -> {}", modname, e.name, e.cfunc));
                        func.status = "unclassified".into();
                        func.overloads = vec![vec![]];
                        func.returns = "any".into();
                        func.unclassified = vec!["args: function body not found".into(), "returns: function body not found".into()];
                    }
                    Some(d) => {
                        let df = &files[d.file];
                        func.file = df.rel.clone();
                        func.line = df.toks[d.name_idx].line;
                        let a = analyze(&df.toks[d.open + 1..d.close], &d.args_name, func.flags == "noargs");
                        func.overloads = a.overloads.iter().map(|o| o.iter().map(|p| p.to_string()).collect()).collect();
                        func.returns = a.returns.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "any".into());
                        func.returns_none = a.returns_none && func.returns != "none";
                        func.returns_variants = a.variants;
                        func.errors = a.errors;
                        func.owners = a.owners;
                        func.quirks = a.quirks;
                        func.on_bad_arg = a.on_bad_arg;
                        func.unclassified = a.unclassified;
                        let args_bad = !func.args_ok();
                        let ret_bad = !func.returns_ok();
                        func.status = match (args_bad, ret_bad) {
                            (false, false) => "classified",
                            (true, true) => "unclassified",
                            _ => "partial",
                        }
                        .into();
                    }
                }
                spec.functions.push(func);
            }
            // Constants registered in the same init function.
            let mut depth = 0i32;
            let mut j = i;
            while j < t.len() {
                if t[j].is("{") {
                    depth += 1;
                }
                if t[j].is("}") {
                    depth -= 1;
                    if depth < 0 {
                        break;
                    }
                }
                if t[j].is_ident() && t.get(j + 1).is_some_and(|x| x.is("(")) {
                    let nm = t[j].text.as_str();
                    if matches!(nm, "PyModule_AddIntConstant" | "PyModule_AddStringConstant" | "PyModule_AddObject")
                        && (modvar.is_empty() || t.get(j + 2).is_some_and(|x| x.text == modvar))
                    {
                        let close = f.m[j + 1].unwrap_or(j + 1);
                        if t.get(j + 4).is_some_and(|x| x.kind == Kind::Str) {
                            let cname = t[j + 4].text.clone();
                            let expr_toks = &t[(j + 6).min(close)..close];
                            let expr = crate::lex::join(expr_toks);
                            let (ty, expr) = match nm {
                                "PyModule_AddIntConstant" => ("int", expr),
                                "PyModule_AddStringConstant" => ("str", expr),
                                _ if expr.starts_with("PyFloat_FromDouble") => ("float", expr),
                                _ => ("object", expr),
                            };
                            spec.constants.push(Constant { name: cname, ty: ty.into(), value: String::new(), expr, cfg: t[j].cfg.clone() });
                        } else {
                            dynamic_constants += 1;
                        }
                    }
                }
                j += 1;
            }
        }
    }
    // Snapshot values and constants the sources only register dynamically.
    for (mname, spec) in specs.iter_mut() {
        if let Some(snap) = snapshot.get(mname) {
            for c in spec.constants.iter_mut() {
                if let Some(it) = snap.get(&c.name) {
                    c.value = it.value.clone();
                    if it.kind != "callable" {
                        c.ty = it.kind.clone();
                    }
                }
            }
            let have: std::collections::BTreeSet<String> =
                spec.constants.iter().map(|c| c.name.clone()).chain(spec.functions.iter().map(|f| f.name.clone())).collect();
            for (name, it) in snap {
                if it.kind != "callable" && !have.contains(name) {
                    spec.constants.push(Constant { name: name.clone(), ty: it.kind.clone(), value: it.value.clone(), ..Default::default() });
                }
            }
        }
    }
    // Callbacks.
    let mut unattributed = 0;
    for f in files.iter() {
        let t = &f.toks;
        let module = module_for_file(&f.rel);
        for i in 0..t.len().saturating_sub(2) {
            if t[i].is_ident() && t[i].text.starts_with("PyCallClassMemberFunc") && t[i + 1].is("(") && (i == 0 || !t[i - 1].is("bool")) {
                let close = f.m[i + 1].unwrap_or(i + 1);
                let mut args = Vec::new();
                let mut start = i + 2;
                let mut k = i + 2;
                while k < close {
                    if t[k].kind == Kind::Punct && matches!(t[k].text.as_str(), "(" | "{" | "[") {
                        k = f.m[k].unwrap_or(k);
                    } else if t[k].is(",") {
                        args.push((start, k));
                        start = k + 1;
                    }
                    k += 1;
                }
                args.push((start, close));
                let handler = args
                    .get(1)
                    .map(|(s, e)| if e - s == 1 && t[*s].kind == Kind::Str { t[*s].text.clone() } else { crate::lex::join(&t[*s..*e]) })
                    .unwrap_or_default();
                let cb = Callback { file: f.rel.clone(), line: t[i].line, handler, via: t[i].text.clone(), in_fn: enclosing_fn(f, i) };
                if module == "(unattributed)" || module == "(host)" {
                    unattributed += 1;
                }
                let target = if module.starts_with('(') { None } else { Some(module) };
                if let Some(mn) = target {
                    if let Some(s) = specs.get_mut(mn) {
                        s.callbacks.push(cb);
                        continue;
                    }
                }
                specs.entry("(callbacks)".into()).or_insert_with(|| ModuleSpec { module: "(callbacks)".into(), ..Default::default() }).callbacks.push(cb);
            }
        }
    }
    let mut specs: Vec<ModuleSpec> = specs.into_values().collect();
    for s in specs.iter_mut() {
        s.sources.sort();
        s.functions.sort_by(|a, b| a.name.cmp(&b.name));
        s.constants.sort_by(|a, b| a.name.cmp(&b.name));
        s.constants.dedup_by(|a, b| a.name == b.name);
        s.callbacks.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    }
    missing.sort();
    Extraction { specs, dynamic_constants, missing_bodies: missing, unattributed_callbacks: unattributed }
}
