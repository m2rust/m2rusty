//! The C ABI shared by the C++ shim and the Rust glue generators.

use crate::analyze::snake;
use crate::spec::{Function, Param, Ty};
use std::collections::BTreeSet;

pub struct Ov {
    pub c_name: String,
    pub rust_name: String,
    pub params: Vec<Param>,
    pub arity: usize,
}

pub struct Plan {
    pub ovs: Vec<Ov>,
    pub ret: Ty,
    pub returns_none: bool,
}

pub fn module_ident(module: &str) -> String {
    snake(module)
}

pub fn c_type(t: &Ty) -> &'static str {
    match t {
        Ty::Int => "int32_t",
        Ty::Uint => "uint32_t",
        Ty::Long => "int64_t",
        Ty::Ulong => "uint64_t",
        Ty::Byte => "uint8_t",
        Ty::Word => "uint16_t",
        Ty::Float => "float",
        Ty::Double => "double",
        Ty::Bool => "bool",
        Ty::Str => "const char *",
        Ty::Object => "void *",
        Ty::Handle(_) => "uintptr_t",
        _ => "void",
    }
}

pub fn rust_type(t: &Ty) -> &'static str {
    match t {
        Ty::Int => "i32",
        Ty::Uint => "u32",
        Ty::Long => "i64",
        Ty::Ulong => "u64",
        Ty::Byte => "u8",
        Ty::Word => "u16",
        Ty::Float => "f32",
        Ty::Double => "f64",
        Ty::Bool => "bool",
        Ty::Handle(_) => "usize",
        _ => "()",
    }
}

fn scalar_ret(t: &Ty) -> bool {
    matches!(t, Ty::Int | Ty::Uint | Ty::Long | Ty::Ulong | Ty::Byte | Ty::Word | Ty::Float | Ty::Double | Ty::Bool | Ty::Handle(_))
}

pub fn ret_supported(t: &Ty) -> bool {
    match t {
        Ty::None | Ty::Str | Ty::Bytes => true,
        t if scalar_ret(t) => true,
        Ty::Tuple(v) => v.iter().all(|x| scalar_ret(x) || matches!(x, Ty::Str)),
        Ty::List(e) => scalar_ret(e) || matches!(**e, Ty::Str),
        _ => false,
    }
}

fn param_supported(t: &Ty) -> bool {
    scalar_ret(t) || matches!(t, Ty::Str | Ty::Object)
}

/// Why a function cannot be generated, if so.
pub fn plan(f: &Function) -> Result<Plan, String> {
    if f.status != "classified" {
        return Err(format!("not classified ({})", f.unclassified.join("; ")));
    }
    let ovs_raw = f.parsed_overloads()?;
    let ret = f.return_ty();
    if !ret_supported(&ret) {
        return Err(format!("unsupported return type {ret}"));
    }
    let base = snake(&f.name);
    let multi = ovs_raw.len() > 1;
    let mut ovs = Vec::new();
    for ps in ovs_raw {
        for p in &ps {
            if !param_supported(&p.ty) {
                return Err(format!("unsupported parameter type {}", p.ty));
            }
        }
        let arity = ps.len();
        let suffix = if multi { format!("_n{arity}") } else { String::new() };
        ovs.push(Ov { c_name: format!("{base}{suffix}"), rust_name: rust_ident(&format!("{base}{suffix}")), params: ps, arity });
    }
    Ok(Plan { ovs, ret, returns_none: f.returns_none })
}

pub fn rust_ident(s: &str) -> String {
    const KW: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
        "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro", "override", "priv",
        "typeof", "unsized", "virtual", "yield", "try", "gen",
    ];
    let mut out: String = s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if KW.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

/// Unique C names for every overload of every function in a module.
pub fn unique_names(plans: &mut [(usize, Plan)]) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (_, p) in plans.iter_mut() {
        for ov in p.ovs.iter_mut() {
            let mut n = ov.c_name.clone();
            let mut k = 2;
            while !seen.insert(n.clone()) {
                n = format!("{}_{k}", ov.c_name);
                k += 1;
            }
            ov.c_name = n.clone();
            ov.rust_name = rust_ident(&n);
        }
    }
}

/// Parameter names for Rust: unique, keyword-safe.
pub fn rust_param_names(params: &[Param]) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::new();
    for (i, p) in params.iter().enumerate() {
        let mut n = rust_ident(&p.name);
        if n.starts_with("out") || n == "api" || !seen.insert(n.clone()) {
            n = format!("{n}{i}_");
            seen.insert(n.clone());
        }
        out.push(n);
    }
    out
}

pub fn cfg_to_if(cfg: &str) -> String {
    let parts: Vec<String> = cfg
        .split(" && ")
        .map(|p| {
            let (neg, id) = match p.strip_prefix('!') {
                Some(r) => (true, r),
                None => (false, p),
            };
            if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                format!("{}defined({id})", if neg { "!" } else { "" })
            } else {
                p.to_string()
            }
        })
        .collect();
    parts.join(" && ")
}
