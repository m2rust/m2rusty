//! Body analysis: argument reads, return kinds, errors, owners.
//!
//! The scanner is deliberately pragmatic: it recognises the helper idioms the
//! hand-written modules use (`PyTuple_Get*`, `PyArg_ParseTuple`,
//! `switch (PyTuple_Size(args))`, `Py_BuildValue` formats ...) and records a
//! reason for everything else, so nothing is silently guessed.

use crate::lex::{join, match_brackets, Kind, Tok};
use crate::spec::{Param, Ty};
use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Analysis {
    pub overloads: Vec<Vec<Param>>,
    pub returns: Option<Ty>,
    pub returns_none: bool,
    pub variants: Vec<String>,
    pub errors: Vec<String>,
    pub owners: Vec<String>,
    pub quirks: Vec<String>,
    pub unclassified: Vec<String>,
    pub on_bad_arg: String,
}

#[derive(Clone, Debug, PartialEq)]
enum Ctx {
    None,
    Case(usize, usize), // (switch id, arity)
    IfSize,
}

#[derive(Clone, Debug)]
struct Get {
    default: Option<String>,
    pos: Option<usize>,
    ty: Ty,
    name: String,
    required: bool,
    fail: String, // "none" | "bad_argument" | "other" | ""
    ctx: Ctx,
}

fn helper_type(name: &str) -> Option<Ty> {
    Some(match name {
        "PyTuple_GetInteger" => Ty::Int,
        "PyTuple_GetByte" => Ty::Byte,
        "PyTuple_GetUnsignedInteger" => Ty::Uint,
        "PyTuple_GetLong" => Ty::Long,
        "PyTuple_GetUnsignedLong" => Ty::Ulong,
        "PyTuple_GetFloat" => Ty::Float,
        "PyTuple_GetDouble" => Ty::Double,
        "PyTuple_GetString" => Ty::Str,
        "PyTuple_GetBoolean" => Ty::Bool,
        "PyTuple_GetObject" => Ty::Object,
        "PyTuple_GetWindow" => Ty::Handle(Some("window".into())),
        "PyTuple_GetTextInstance" => Ty::Handle(Some("text_instance".into())),
        "PyTuple_GetThingInstance" => Ty::Handle(Some("thing_instance".into())),
        "PyTuple_GetTextBar" => Ty::Handle(Some("text_bar".into())),
        "PyTuple_GetImageInstance" => Ty::Handle(Some("image_instance".into())),
        "PyTuple_GetExpandedImageInstance" => Ty::Handle(Some("expanded_image_instance".into())),
        _ => return None,
    })
}

/// Splits the tokens of a call's argument list at top-level commas.
fn split_args(toks: &[Tok], open: usize, close: usize, m: &[Option<usize>]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = open + 1;
    let mut i = open + 1;
    while i < close {
        let t = &toks[i];
        if t.kind == Kind::Punct && matches!(t.text.as_str(), "(" | "{" | "[") {
            i = m[i].unwrap_or(i);
        } else if t.is(",") {
            out.push((start, i));
            start = i + 1;
        }
        i += 1;
    }
    if start < close || !out.is_empty() {
        out.push((start, close));
    }
    out
}

pub fn snake(s: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let prev_lower = i > 0 && (chars[i - 1].is_ascii_lowercase() || chars[i - 1].is_ascii_digit());
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            if i > 0 && (prev_lower || (next_lower && chars[i - 1].is_ascii_uppercase())) && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '.' || c == '-' || c == '>' {
            if !out.ends_with('_') {
                out.push('_');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `iSlotIndex` -> `slot_index`, `szFileName` -> `file_name`, `c_szName` -> `name`.
pub fn param_name(lvalue: &str) -> String {
    let base = lvalue.rsplit(['.', '>']).next().unwrap_or(lvalue);
    let mut s = base;
    for p in ["c_", "m_"] {
        if let Some(r) = s.strip_prefix(p) {
            s = r;
        }
    }
    const PREFIXES: [&str; 22] = [
        "psz", "sz", "pch", "pc", "pk", "po", "pp", "str", "dw", "ul", "by", "rk", "i", "f", "b", "n", "l", "w", "p", "u", "s", "d",
    ];
    for p in PREFIXES {
        if let Some(rest) = s.strip_prefix(p) {
            if rest.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                s = rest;
                break;
            }
        }
    }
    let mut n = snake(s);
    if n.is_empty() {
        n = "arg".into();
    }
    n
}

struct Declared {
    ty: String,
    init: Option<String>,
}

/// Finds `<type> var [= init];` before `before`.
fn find_decl(toks: &[Tok], var: &str, before: usize) -> Option<Declared> {
    for i in (1..before.min(toks.len())).rev() {
        if toks[i].is_ident() && toks[i].text == var {
            let nxt = toks.get(i + 1);
            let is_decl_follow = nxt.is_some_and(|n| n.is("=") || n.is(";") || n.is(",") || n.is("{") || n.is("("));
            if !is_decl_follow {
                continue;
            }
            // Walk back over the type tokens.
            let mut j = i;
            let mut ty: Vec<&str> = Vec::new();
            while j > 0 {
                let p = &toks[j - 1];
                if p.is_ident() || p.is("*") || p.is("&") || p.is("::") {
                    ty.push(p.text.as_str());
                    j -= 1;
                } else {
                    break;
                }
            }
            if ty.is_empty() {
                continue;
            }
            ty.reverse();
            if ty.iter().all(|t| matches!(*t, "return" | "else")) || ty.last().is_some_and(|t| matches!(*t, "return" | "else" | "case")) {
                continue;
            }
            let init = if nxt.is_some_and(|n| n.is("=")) {
                let mut k = i + 2;
                let mut buf = Vec::new();
                while k < toks.len() && !toks[k].is(";") && !toks[k].is(",") {
                    buf.push(toks[k].clone());
                    k += 1;
                }
                Some(join(&buf))
            } else {
                None
            };
            return Some(Declared { ty: ty.join(" "), init });
        }
    }
    None
}

fn refine_int(helper: &Ty, decl: &str) -> Ty {
    if *helper != Ty::Int {
        return helper.clone();
    }
    let d = decl.replace('*', "").replace("const", "").replace("::", "_");
    let d = d.trim();
    match d {
        "BYTE" | "unsigned char" | "uint8_t" | "char" | "int8_t" => Ty::Byte,
        "WORD" | "unsigned short" | "uint16_t" | "short" | "int16_t" => Ty::Word,
        _ => Ty::Int,
    }
}

fn lvalue_text(toks: &[Tok]) -> String {
    let mut t: &[Tok] = toks;
    // Drop a leading `&`.
    if t.first().is_some_and(|x| x.is("&")) {
        t = &t[1..];
    }
    join(t)
}

fn classify_fail(toks: &[Tok], from: usize, to: usize) -> String {
    // First `return` in range.
    for i in from..to {
        if toks[i].is("return") {
            let name = toks.get(i + 1).map(|t| t.text.as_str()).unwrap_or("");
            return match name {
                "Py_BuildException" => "none".into(),
                "Py_BadArgument" => "bad_argument".into(),
                "nullptr" | "NULL" => "null".into(),
                _ => "other".into(),
            };
        }
    }
    String::new()
}

fn body_extent(toks: &[Tok], after_cond: usize, m: &[Option<usize>]) -> (usize, usize) {
    // Statement or block following `if (...)` / `else`.
    if after_cond < toks.len() && toks[after_cond].is("{") {
        (after_cond, m[after_cond].unwrap_or(toks.len() - 1))
    } else {
        let mut i = after_cond;
        while i < toks.len() && !toks[i].is(";") {
            if toks[i].kind == Kind::Punct && matches!(toks[i].text.as_str(), "(" | "{" | "[") {
                i = m[i].unwrap_or(i);
            }
            i += 1;
        }
        (after_cond, i.min(toks.len() - 1))
    }
}

struct Switch {
    id: usize,
    cases: Vec<(usize, usize, usize)>, // (arity, seg_start, seg_end)
    has_default: bool,
    body: (usize, usize),
    fallthrough: Vec<usize>,
}

pub fn analyze(toks: &[Tok], args_name: &str, noargs: bool) -> Analysis {
    let mut a = Analysis::default();
    let m = match_brackets(toks);
    let n = toks.len();

    // ---- Size contexts -------------------------------------------------
    let mut switches: Vec<Switch> = Vec::new();
    let mut if_size: Vec<(usize, usize)> = Vec::new();
    let mut size_calls_ok: BTreeSet<usize> = BTreeSet::new();
    for i in 0..n {
        if toks[i].is_ident() && toks[i].text == "PyTuple_Size" && toks.get(i + 1).is_some_and(|t| t.is("(")) {
            let close = m[i + 1].unwrap_or(i + 1);
            // switch ( PyTuple_Size(..) ) {
            if i >= 2 && toks[i - 1].is("(") && toks[i - 2].is("switch") {
                let sw_close = m[i - 1].unwrap_or(i);
                if toks.get(sw_close + 1).is_some_and(|t| t.is("{")) {
                    let ob = sw_close + 1;
                    let cb = m[ob].unwrap_or(n - 1);
                    let mut labels: Vec<(Option<usize>, usize)> = Vec::new();
                    let mut j = ob + 1;
                    while j < cb {
                        if toks[j].kind == Kind::Punct && matches!(toks[j].text.as_str(), "(" | "{" | "[") {
                            j = m[j].unwrap_or(j) + 1;
                            continue;
                        }
                        if toks[j].is("case") {
                            let num = toks.get(j + 1).and_then(|t| t.text.parse::<usize>().ok());
                            labels.push((num, j));
                            j += 1;
                            continue;
                        }
                        if toks[j].is("default") && toks.get(j + 1).is_some_and(|t| t.is(":")) {
                            labels.push((None, j));
                        }
                        j += 1;
                    }
                    let mut sw = Switch { id: switches.len(), cases: vec![], has_default: false, body: (ob, cb), fallthrough: vec![] };
                    for (k, (num, start)) in labels.iter().enumerate() {
                        let end = labels.get(k + 1).map(|l| l.1).unwrap_or(cb);
                        match num {
                            Some(v) => {
                                sw.cases.push((*v, *start, end));
                                if !segment_terminates(toks, *start, end, &m) {
                                    sw.fallthrough.push(*v);
                                }
                            }
                            None => sw.has_default = true,
                        }
                    }
                    switches.push(sw);
                    size_calls_ok.insert(i);
                    continue;
                }
            }
            // if ( PyTuple_Size(..) OP N ) body
            let mut k = i;
            let mut depth_ok = false;
            while k > 0 {
                k -= 1;
                if toks[k].is("(") && m[k].is_some_and(|c| c > i) {
                    if k > 0 && toks[k - 1].is("if") {
                        let c = m[k].unwrap();
                        let (s, e) = body_extent(toks, c + 1, &m);
                        if_size.push((s, e));
                        depth_ok = true;
                    }
                    break;
                }
                if toks[k].is(";") || toks[k].is("{") || toks[k].is("}") {
                    break;
                }
            }
            let _ = close;
            if depth_ok {
                size_calls_ok.insert(i);
            }
        }
    }
    let ctx_at = |idx: usize| -> Ctx {
        for sw in &switches {
            if idx > sw.body.0 && idx < sw.body.1 {
                for (arity, s, e) in &sw.cases {
                    if idx >= *s && idx < *e {
                        return Ctx::Case(sw.id, *arity);
                    }
                }
            }
        }
        for (s, e) in &if_size {
            if idx >= *s && idx <= *e {
                return Ctx::IfSize;
            }
        }
        Ctx::None
    };

    // ---- `if` statements: for guard analysis ---------------------------
    struct IfStmt {
        cond: (usize, usize),
        body: (usize, usize),
    }
    let mut ifs: Vec<IfStmt> = Vec::new();
    for i in 0..n {
        if toks[i].is("if") && toks.get(i + 1).is_some_and(|t| t.is("(")) {
            let c = m[i + 1].unwrap_or(i + 1);
            let (s, e) = body_extent(toks, c + 1, &m);
            ifs.push(IfStmt { cond: (i + 1, c), body: (s, e) });
        }
    }

    // ---- Argument reads ------------------------------------------------
    let mut gets: Vec<Get> = Vec::new();
    let mut handled_args_refs: BTreeSet<usize> = BTreeSet::new();
    let mut parse_tuple = false;
    let mut parse_named = true;
    for i in 0..n {
        if !toks[i].is_ident() || !toks.get(i + 1).is_some_and(|t| t.is("(")) {
            continue;
        }
        let name = toks[i].text.as_str();
        let close = m[i + 1].unwrap_or(i + 1);
        if let Some(helper) = helper_type(name) {
            let parts = split_args(toks, i + 1, close, &m);
            if parts.len() != 3 {
                a.unclassified.push(format!("args: unusual call to {name}"));
                continue;
            }
            // arg 0 must be the args tuple.
            handled_args_refs.insert(parts[0].0);
            let pos_toks = &toks[parts[1].0..parts[1].1];
            let pos = if pos_toks.len() == 1 && pos_toks[0].kind == Kind::Number { pos_toks[0].text.parse::<usize>().ok() } else { None };
            let lv = lvalue_text(&toks[parts[2].0..parts[2].1]);
            let last_ident = lv.rsplit(['.', '>']).next().unwrap_or(&lv).to_string();
            let plain_var = lv.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            let decl = if plain_var { find_decl(toks, &lv, i) } else { None };
            let mut ty = helper.clone();
            if let Some(d) = &decl {
                ty = refine_int(&helper, &d.ty);
            }
            // Guard analysis.
            let mut required = false;
            let mut fail = String::new();
            let negated = i >= 1 && toks[i - 1].is("!");
            let mut in_cond = false;
            // innermost if whose condition contains i
            let mut best: Option<&IfStmt> = None;
            for st in &ifs {
                if i > st.cond.0 && i < st.cond.1 && best.is_none_or(|b| st.cond.0 > b.cond.0) {
                    best = Some(st);
                }
            }
            if let Some(st) = best {
                in_cond = true;
                let has_return = (st.body.0..=st.body.1.min(n - 1)).any(|k| toks[k].is("return"));
                if negated && has_return {
                    required = true;
                    fail = classify_fail(toks, st.body.0, st.body.1.min(n - 1));
                }
            }
            if !in_cond {
                a.quirks.push(format!("argument {} read outside an `if` condition", pos.map(|p| p.to_string()).unwrap_or("?".into())));
            }
            gets.push(Get {
                default: decl.as_ref().and_then(|d| d.init.clone()).filter(|d| d != "nullptr" && !d.is_empty()),
                pos,
                ty,
                name: param_name(&last_ident),
                required,
                fail,
                ctx: ctx_at(i),
            });
            if pos.is_none() {
                a.unclassified.push(format!("args: non-literal index in {name}"));
            }
        } else if name == "PyArg_ParseTuple" {
            parse_tuple = true;
            let parts = split_args(toks, i + 1, close, &m);
            handled_args_refs.insert(parts[0].0);
            if parts.len() < 2 || toks[parts[1].0].kind != Kind::Str {
                a.unclassified.push("args: PyArg_ParseTuple with a non-literal format".into());
                continue;
            }
            let fmt = toks[parts[1].0].text.clone();
            parse_named &= fmt.contains(':');
            let fmt = fmt.split(':').next().unwrap_or("").split(';').next().unwrap_or("").to_string();
            let mut optional = false;
            let mut k = 0;
            let chars: Vec<char> = fmt.chars().collect();
            let mut pi = 2;
            let mut params = Vec::new();
            while k < chars.len() {
                let c = chars[k];
                k += 1;
                if c == '|' {
                    optional = true;
                    continue;
                }
                let mut ty = match c {
                    'i' | 'h' => Ty::Int,
                    'l' | 'L' => Ty::Long,
                    'k' | 'K' => Ty::Ulong,
                    'I' => Ty::Uint,
                    'b' | 'B' => Ty::Byte,
                    'd' => Ty::Double,
                    'f' => Ty::Float,
                    's' | 'z' => Ty::Str,
                    'O' => Ty::Object,
                    _ => {
                        a.unclassified.push(format!("args: PyArg_ParseTuple format `{c}`"));
                        Ty::Any
                    }
                };
                if k < chars.len() && chars[k] == '#' {
                    k += 1;
                    ty = Ty::Bytes;
                    pi += 1;
                }
                if k < chars.len() && chars[k] == '!' {
                    k += 1;
                    pi += 1;
                    ty = Ty::Object;
                }
                let nm = parts
                    .get(pi)
                    .map(|p| param_name(lvalue_text(&toks[p.0..p.1]).as_str()))
                    .unwrap_or_else(|| format!("arg{}", params.len()));
                pi += 1;
                params.push(Param { name: nm, ty, optional, default: None });
            }
            a.overloads.push(params);
        } else if name == "PyTuple_GetItem" || name == "PyTuple_GET_ITEM" {
            let parts = split_args(toks, i + 1, close, &m);
            if parts.first().is_some_and(|p| toks[p.0].text == args_name && p.1 == p.0 + 1) {
                a.unclassified.push("args: raw PyTuple_GetItem on the argument tuple".into());
                handled_args_refs.insert(parts[0].0);
            }
        }
    }
    if toks.iter().any(|t| t.is_ident() && matches!(t.text.as_str(), "PyDict_Next" | "PyDict_GetItemString" | "PyDict_Check")) {
        a.unclassified.push("args: reads a dict argument by hand".into());
    }
    if toks.iter().any(|t| t.is_ident() && matches!(t.text.as_str(), "PyList_GetItem" | "PyList_Size" | "PyList_GET_ITEM" | "PySequence_GetItem")) {
        a.unclassified.push("args: reads a list/sequence argument by hand".into());
    }
    // Any other use of the args tuple.
    for (i, t) in toks.iter().enumerate() {
        if t.is_ident() && t.text == args_name && !args_name.is_empty() && !handled_args_refs.contains(&i) {
            // `PyTuple_Size(args)` handled sites
            if i >= 2 && toks[i - 1].is("(") && toks[i - 2].is_ident() && toks[i - 2].text == "PyTuple_Size" && size_calls_ok.contains(&(i - 2)) {
                continue;
            }
            a.unclassified.push(format!("args: `{}` used directly ({})", args_name, join(&toks[i.saturating_sub(2)..(i + 2).min(n)])));
            break;
        }
    }
    for (i, t) in toks.iter().enumerate() {
        if t.is_ident() && t.text == "PyTuple_Size" && !size_calls_ok.contains(&i) {
            // arguments of the form PyTuple_Size(otherTuple) are not about args
            if toks.get(i + 2).is_some_and(|x| x.text == args_name) {
                a.unclassified.push("args: PyTuple_Size(args) outside a switch/if".into());
                break;
            }
        }
    }
    for sw in &switches {
        for v in &sw.fallthrough {
            a.quirks.push(format!("`case {v}` falls through to the next label (no break/return)"));
        }
    }

    // ---- Overloads -----------------------------------------------------
    if parse_tuple {
        if !gets.is_empty() {
            a.unclassified.push("args: mixes PyArg_ParseTuple and PyTuple_Get*".into());
        }
        a.on_bad_arg = if parse_named { "parse_tuple" } else { "parse_tuple_anon" }.into();
    } else if noargs {
        a.overloads.push(vec![]);
    } else {
        a.overloads = build_overloads(&gets, &switches, &mut a.unclassified, &mut a.quirks);
        // on_bad_arg
        let mut kinds: BTreeSet<String> = BTreeSet::new();
        for g in &gets {
            // An argument guarded by a `PyTuple_Size` check is optional; its
            // failure return is not the shim's bad-argument path.
            if g.required && g.ctx == Ctx::None && !g.fail.is_empty() {
                kinds.insert(g.fail.clone());
            }
        }
        a.on_bad_arg = match kinds.len() {
            0 => String::new(),
            1 => kinds.into_iter().next().unwrap(),
            _ => "mixed".into(),
        };
    }
    if a.overloads.is_empty() {
        a.overloads.push(vec![]);
    }

    // ---- Returns -------------------------------------------------------
    analyze_returns(toks, &m, &mut a);

    // ---- Owners --------------------------------------------------------
    let mut owners = BTreeSet::new();
    for i in 0..n {
        if toks[i].is_ident() && matches!(toks[i].text.as_str(), "Instance" | "GetInstance") && i >= 2 && toks[i - 1].is("::") && toks.get(i + 1).is_some_and(|t| t.is("(")) {
            let mut j = i - 2;
            let mut name = toks[j].text.clone();
            while j >= 2 && toks[j - 1].is("::") && toks[j - 2].is_ident() {
                name = format!("{}::{}", toks[j - 2].text, name);
                j -= 2;
            }
            owners.insert(name);
        }
        if toks[i].is_ident() && toks[i].text.starts_with("g_") && !toks[i - usize::from(i > 0)].is("::") {
            owners.insert(toks[i].text.clone());
        }
    }
    a.owners = owners.into_iter().collect();

    // ---- Errors raised directly ---------------------------------------
    for i in 0..n {
        if toks[i].is("PyErr_SetString") && toks.get(i + 1).is_some_and(|t| t.is("(")) {
            let close = m[i + 1].unwrap_or(i + 1);
            let p = split_args(toks, i + 1, close, &m);
            if p.len() == 2 {
                let kind = toks[p[0].0].text.trim_start_matches("PyExc_").to_string();
                let msg = if toks[p[1].0].kind == Kind::Str { toks[p[1].0].text.clone() } else { "<expr>".into() };
                a.errors.push(format!("{kind}: {msg}"));
            }
        }
    }
    a.errors.sort();
    a.errors.dedup();
    a.quirks.sort();
    a.quirks.dedup();
    a.unclassified.sort();
    a.unclassified.dedup();
    a
}

fn segment_terminates(toks: &[Tok], start: usize, end: usize, m: &[Option<usize>]) -> bool {
    // Skip the `case N :` label.
    let mut i = start;
    while i < end && !toks[i].is(":") {
        i += 1;
    }
    i += 1;
    // A single wrapping block `{ ... }`.
    let (mut lo, mut hi) = (i, end);
    if lo < hi && toks[lo].is("{") && m[lo] == Some(hi - 1) {
        lo += 1;
        hi -= 1;
    }
    let mut k = lo;
    while k < hi {
        let t = &toks[k];
        if t.kind == Kind::Punct && matches!(t.text.as_str(), "(" | "{" | "[") {
            k = m[k].unwrap_or(k) + 1;
            continue;
        }
        let stmt_start = k == 0 || toks[k - 1].is(";") || toks[k - 1].is(":") || toks[k - 1].is("{") || toks[k - 1].is("}");
        if stmt_start && t.is_ident() && matches!(t.text.as_str(), "break" | "return" | "goto" | "continue") {
            return true;
        }
        k += 1;
    }
    false
}

fn merge_get(by_pos: &mut Vec<(usize, Get)>, g: &Get, unclassified: &mut Vec<String>) {
    let Some(pos) = g.pos else { return };
    if let Some((_, existing)) = by_pos.iter_mut().find(|(p, _)| *p == pos) {
        if existing.ty != g.ty {
            // int vs sized int is only a narrowing difference; anything else is a real conflict.
            let both_int = existing.ty.is_scalar_number() && g.ty.is_scalar_number() && existing.ty != Ty::Float && g.ty != Ty::Float;
            if !both_int {
                unclassified.push(format!("args: position {pos} read as {} and {}", existing.ty, g.ty));
            }
        }
        if !g.required || g.ctx != Ctx::None {
            // keep first; optional if any read is optional
        }
        return;
    }
    by_pos.push((pos, g.clone()));
}

fn build_overloads(gets: &[Get], switches: &[Switch], unclassified: &mut Vec<String>, quirks: &mut Vec<String>) -> Vec<Vec<Param>> {
    let to_params = |mut v: Vec<(usize, Get)>, unclassified: &mut Vec<String>| -> Vec<Param> {
        v.sort_by_key(|(p, _)| *p);
        for (i, (p, _)) in v.iter().enumerate() {
            if *p != i {
                unclassified.push(format!("args: argument positions are not contiguous (missing {i})"));
                break;
            }
        }
        // Unique names.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut out = Vec::new();
        for (p, g) in v {
            let mut name = g.name.clone();
            if !seen.insert(name.clone()) {
                name = format!("{}{}", name, p);
                seen.insert(name.clone());
            }
            out.push(Param { name, ty: g.ty.clone(), optional: !(g.required && g.ctx == Ctx::None || matches!(g.ctx, Ctx::Case(..)) && g.required), default: None });
        }
        out
    };
    if let Some(sw) = switches.first() {
        if switches.len() > 1 {
            unclassified.push("args: more than one switch on PyTuple_Size".into());
        }
        let mut arities: Vec<usize> = sw.cases.iter().map(|c| c.0).collect();
        arities.sort();
        arities.dedup();
        let mut out = Vec::new();
        for ar in arities {
            let mut by_pos: Vec<(usize, Get)> = Vec::new();
            let mut seen_pos = BTreeSet::new();
            for g in gets {
                if g.ctx == Ctx::Case(sw.id, ar) {
                    if let Some(p) = g.pos {
                        if !seen_pos.insert(p) {
                            unclassified.push(format!("args: `case {ar}` reads position {p} twice"));
                            quirks.push(format!("C++ bug: `case {ar}` reads argument {p} twice"));
                        }
                    }
                }
                match g.ctx {
                    Ctx::None | Ctx::IfSize => merge_get(&mut by_pos, g, unclassified),
                    Ctx::Case(id, a2) if id == sw.id && a2 == ar => merge_get(&mut by_pos, g, unclassified),
                    _ => {}
                }
            }
            let mut ps = to_params(by_pos, unclassified);
            if ps.len() != ar && !ps.iter().any(|p| p.optional) {
                quirks.push(format!("`case {ar}` reads {} arguments", ps.len()));
            }
            // Within an arity-specific form every argument is required.
            for p in &mut ps {
                p.optional = false;
            }
            out.push(ps);
        }
        return out;
    }
    let mut by_pos: Vec<(usize, Get)> = Vec::new();
    for g in gets {
        merge_get(&mut by_pos, g, unclassified);
    }
    // A position is required only if some read of it is an unconditional guard.
    let mut ps = to_params(by_pos, unclassified);
    for p in &mut ps {
        let _ = &p;
    }
    // Recompute requiredness across all reads of a position.
    for (i, p) in ps.iter_mut().enumerate() {
        let req = gets.iter().any(|g| g.pos == Some(i) && g.required && g.ctx == Ctx::None);
        p.optional = !req;
        if p.optional {
            p.default = gets.iter().find(|g| g.pos == Some(i)).and_then(|g| g.default.clone());
        }
    }
    vec![ps]
}

fn fmt_to_ty(fmt: &str) -> Option<Ty> {
    // Returns the type produced by a Py_BuildValue format string.
    let f: String = fmt.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
    let (items, tuple) = parse_fmt_items(&f)?;
    if f == "()" {
        return Some(Ty::Tuple(vec![]));
    }
    if items.len() == 1 && !tuple {
        return Some(items.into_iter().next().unwrap());
    }
    Some(Ty::Tuple(items))
}

fn parse_fmt_items(f: &str) -> Option<(Vec<Ty>, bool)> {
    let chars: Vec<char> = f.chars().collect();
    let (mut inner, tuple) = if chars.first() == Some(&'(') && chars.last() == Some(&')') && matching_paren(&chars) == Some(chars.len() - 1) {
        (&chars[1..chars.len() - 1], true)
    } else {
        (&chars[..], false)
    };
    let mut items = Vec::new();
    while !inner.is_empty() {
        let c = inner[0];
        inner = &inner[1..];
        let t = match c {
            'i' | 'h' => Ty::Int,
            'I' => Ty::Uint,
            'b' | 'B' => Ty::Byte,
            'l' | 'L' => Ty::Long,
            'k' | 'K' => Ty::Ulong,
            'f' => Ty::Float,
            'd' => Ty::Double,
            's' | 'z' => {
                if inner.first() == Some(&'#') {
                    inner = &inner[1..];
                    Ty::Bytes
                } else {
                    Ty::Str
                }
            }
            'O' | 'N' => Ty::Object,
            '(' => {
                let end = matching_paren(inner.iter().cloned().collect::<Vec<_>>().as_slice().iter().fold(vec!['('], |mut v, c| { v.push(*c); v }).as_slice())?;
                let sub: String = std::iter::once('(').chain(inner[..end].iter().cloned()).collect();
                inner = &inner[end..];
                fmt_to_ty(&sub)?
            }
            _ => return None,
        };
        items.push(t);
    }
    Some((items, tuple))
}

fn matching_paren(chars: &[char]) -> Option<usize> {
    let mut d = 0i32;
    for (i, c) in chars.iter().enumerate() {
        match c {
            '(' => d += 1,
            ')' => {
                d -= 1;
                if d == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn analyze_returns(toks: &[Tok], m: &[Option<usize>], a: &mut Analysis) {
    let n = toks.len();
    let mut value_types: Vec<Ty> = Vec::new();
    let mut none_returns = 0;
    let mut error_returns = 0;
    let mut unresolved: Vec<String> = Vec::new();
    for i in 0..n {
        let is_macro_none = toks[i].is_ident() && toks[i].text == "Py_RETURN_NONE";
        if is_macro_none {
            none_returns += 1;
            continue;
        }
        if !toks[i].is("return") {
            continue;
        }
        // Statement extent.
        let mut j = i + 1;
        while j < n && !toks[j].is(";") {
            if toks[j].kind == Kind::Punct && matches!(toks[j].text.as_str(), "(" | "{" | "[") {
                j = m[j].unwrap_or(j);
            }
            j += 1;
        }
        let e = &toks[i + 1..j.min(n)];
        if e.is_empty() {
            continue; // `return;` in a void helper lambda
        }
        let head = e[0].text.as_str();
        let is_call = e.len() >= 3 && e[0].is_ident() && e[1].is("(") && m[i + 2] == Some(i + 1 + e.len() - 1);
        match head {
            "nullptr" | "NULL" if e.len() == 1 => {
                error_returns += 1;
            }
            "Py_BuildNone" if is_call => none_returns += 1,
            "Py_BuildException" | "Py_BadArgument" if is_call => {
                error_returns += 1;
                if head == "Py_BuildException" && e.len() > 3 && e[2].kind == Kind::Str {
                    a.errors.push(format!("RuntimeError: {}", e[2].text));
                } else if head == "Py_BadArgument" {
                    a.errors.push("TypeError: bad argument".into());
                }
            }
            "Py_BuildEmptyTuple" if is_call => value_types.push(Ty::Tuple(vec![])),
            "Py_BuildValue" if is_call => {
                let parts = split_args(toks, i + 2, i + 1 + e.len() - 1, m);
                if parts.is_empty() || toks[parts[0].0].kind != Kind::Str {
                    unresolved.push(format!("Py_BuildValue with a non-literal format: {}", trunc(&join(e))));
                } else {
                    let fmt = toks[parts[0].0].text.as_str();
                    if fmt == "None" || fmt.is_empty() {
                        none_returns += 1;
                    } else if let Some(mut ty) = fmt_to_ty(fmt) {
                        // `(intptr_t)ptr` with "l" is a pointer handed to Python as an integer.
                        let has_ptr = e.iter().any(|t| t.is("intptr_t") || t.is("uintptr_t"));
                        if has_ptr && ty == Ty::Long {
                            ty = Ty::Handle(None);
                        }
                        value_types.push(ty);
                    } else {
                        unresolved.push(format!("Py_BuildValue format \"{fmt}\""));
                    }
                }
            }
            "PyFloat_FromDouble" if is_call => value_types.push(Ty::Double),
            "PyInt_FromLong" | "PyLong_FromLong" if is_call => value_types.push(Ty::Long),
            "PyBool_FromLong" if is_call => value_types.push(Ty::Bool),
            "PyString_FromString" | "PyString_FromStringAndSize" if is_call => value_types.push(Ty::Str),
            _ if e.len() == 1 && e[0].is_ident() => {
                // A local PyObject*: look for its construction.
                let var = e[0].text.as_str();
                match local_kind(toks, var, i) {
                    Some(t) => value_types.push(t),
                    None => unresolved.push(format!("returns local `{var}`")),
                }
            }
            _ => unresolved.push(trunc(&join(e))),
        }
    }
    // Direct returns hidden behind helper macros are not seen; fine.
    if !unresolved.is_empty() {
        unresolved.sort();
        unresolved.dedup();
        for u in unresolved {
            a.unclassified.push(format!("returns: {u}"));
        }
    }
    let mut distinct: Vec<Ty> = Vec::new();
    for t in value_types {
        if !distinct.contains(&t) {
            distinct.push(t);
        }
    }
    distinct.sort();
    a.variants = if distinct.len() > 1 { distinct.iter().map(|t| t.to_string()).collect() } else { vec![] };
    match distinct.len() {
        0 => {
            if none_returns > 0 || error_returns > 0 {
                a.returns = Some(Ty::None);
            } else {
                a.unclassified.push("returns: no recognisable return statement".into());
            }
        }
        1 => {
            a.returns = Some(distinct.pop().unwrap());
            a.returns_none = none_returns > 0;
        }
        _ => {
            a.returns = Some(Ty::Any);
            a.returns_none = none_returns > 0;
            a.unclassified.push(format!("returns: several value types ({})", a.variants.join(" | ")));
        }
    }
}

fn trunc(s: &str) -> String {
    if s.chars().count() > 70 {
        let t: String = s.chars().take(67).collect();
        format!("{t}...")
    } else {
        s.to_string()
    }
}

/// `PyObject *var = PyList_New(0)` and friends: the Python type of a local.
fn local_kind(toks: &[Tok], var: &str, before: usize) -> Option<Ty> {
    for i in (0..before).rev() {
        if toks[i].is_ident() && toks[i].text == var && toks.get(i + 1).is_some_and(|t| t.is("=")) {
            let ctor = toks.get(i + 2)?;
            return match ctor.text.as_str() {
                "PyList_New" => {
                    // Element type from the first PyList_Append(var, X).
                    let elem = list_elem(toks, var).unwrap_or(Ty::Any);
                    Some(Ty::List(Box::new(elem)))
                }
                "PyTuple_New" => Some(Ty::Tuple(vec![Ty::Any])),
                "PyDict_New" => Some(Ty::Dict),
                "Py_BuildValue" => None,
                _ => None,
            };
        }
    }
    None
}

fn list_elem(toks: &[Tok], var: &str) -> Option<Ty> {
    for i in 0..toks.len().saturating_sub(4) {
        if toks[i].is("PyList_Append") && toks[i + 1].is("(") && toks[i + 2].text == var && toks[i + 3].is(",") {
            let item = &toks[i + 4].text;
            // item variable assigned from PyString_FromString etc.
            for j in 0..i {
                if toks[j].is_ident() && toks[j].text == *item && toks.get(j + 1).is_some_and(|t| t.is("=")) {
                    return match toks.get(j + 2).map(|t| t.text.as_str()) {
                        Some("PyString_FromString") | Some("PyString_FromStringAndSize") => Some(Ty::Str),
                        Some("PyInt_FromLong") | Some("PyLong_FromLong") => Some(Ty::Long),
                        Some("PyFloat_FromDouble") => Some(Ty::Double),
                        _ => None,
                    };
                }
            }
            if item.starts_with("PyString_From") {
                return Some(Ty::Str);
            }
            if item.starts_with("PyInt_From") {
                return Some(Ty::Long);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex::lex;

    fn run(body: &str) -> Analysis {
        analyze(&lex(body), "poArgs", false)
    }

    #[test]
    fn required_and_optional_arguments() {
        let a = run(r#"
            int iSlot = 0;
            if (!PyTuple_GetInteger(poArgs, 0, &iSlot)) return Py_BadArgument();
            int iLife = 5;
            if (PyTuple_Size(poArgs) == 2) { if (!PyTuple_GetInteger(poArgs, 1, &iLife)) return Py_BuildException(); }
            return Py_BuildValue("(is)", 1, "x");
        "#);
        assert_eq!(a.overloads.len(), 1);
        let p = &a.overloads[0];
        assert_eq!(p[0].to_string(), "slot: int");
        assert_eq!(p[1].to_string(), "life: int = 5");
        assert_eq!(a.on_bad_arg, "bad_argument");
        assert_eq!(a.returns.unwrap().to_string(), "tuple[int, str]");
        assert!(a.unclassified.is_empty(), "{:?}", a.unclassified);
    }

    #[test]
    fn switch_overloads_and_fallthrough() {
        let a = run(r#"
            TItemPos c;
            switch (PyTuple_Size(poArgs)) {
            case 1: if (!PyTuple_GetInteger(poArgs, 0, &c.cell)) return Py_BuildException(); break;
            case 2: if (!PyTuple_GetByte(poArgs, 0, &c.window_type)) return Py_BuildException();
                    if (!PyTuple_GetInteger(poArgs, 1, &c.cell)) return Py_BuildException();
            default: return Py_BuildException();
            }
            return Py_BuildNone();
        "#);
        assert_eq!(a.overloads.len(), 2);
        assert_eq!(a.overloads[1].len(), 2);
        assert_eq!(a.overloads[1][0].ty, Ty::Byte);
        assert!(a.quirks.iter().any(|q| q.contains("case 2")), "{:?}", a.quirks);
    }

    #[test]
    fn unrecognised_constructs_are_reported() {
        let a = run("PyObject *p = PyTuple_GetItem(poArgs, 0); return foo(p);");
        assert!(a.unclassified.iter().any(|r| r.starts_with("args:")));
        assert!(a.unclassified.iter().any(|r| r.starts_with("returns:")));
    }

    #[test]
    fn parse_tuple_formats_and_errors() {
        let a = run(r#"double x; PyObject *b = nullptr;
            if (!PyArg_ParseTuple(poArgs, "d|O:log", &x, &b)) return nullptr;
            if (x <= 0) { PyErr_SetString(PyExc_ValueError, "math domain error"); return nullptr; }
            return PyFloat_FromDouble(x);"#);
        assert_eq!(a.overloads[0][0].to_string(), "x: double");
        assert!(a.overloads[0][1].optional);
        assert_eq!(a.errors, vec!["ValueError: math domain error"]);
        assert_eq!(a.returns, Some(Ty::Double));
    }

    #[test]
    fn names() {
        assert_eq!(param_name("szFileName"), "file_name");
        assert_eq!(param_name("iSlotIndex"), "slot_index");
        assert_eq!(snake("GetItemIndex"), "get_item_index");
        assert_eq!(snake("SendGuildMarkUploadPacket"), "send_guild_mark_upload_packet");
        assert_eq!(snake("wndMgr"), "wnd_mgr");
    }

    #[test]
    fn build_value_formats() {
        for (f, want) in
            [("i", "int"), ("(ii)", "tuple[int, int]"), ("ii", "tuple[int, int]"), ("s#", "bytes"), ("()", "tuple[]"), ("(i)", "tuple[int]")]
        {
            assert_eq!(fmt_to_ty(f).unwrap().to_string(), want, "{f}");
        }
    }
}
