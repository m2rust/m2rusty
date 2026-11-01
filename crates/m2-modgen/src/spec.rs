//! The module spec: one TOML file per Python module (see README.md).

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

/// Value types of the spec. `int` is the C++ `int`; the sized variants record
/// which `PyTuple_Get*` helper (and so which narrowing) the C++ used.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ty {
    Int,
    Uint,
    Long,
    Ulong,
    Byte,
    Word,
    Float,
    Double,
    Bool,
    Str,
    Bytes,
    Object,
    Any,
    None,
    /// A pointer passed to Python as an integer (`Py_BuildValue("l", (intptr_t)p)`).
    /// The optional kind is the C++ type read back by `PyTuple_Get<Kind>`.
    Handle(Option<String>),
    Tuple(Vec<Ty>),
    List(Box<Ty>),
    Dict,
}

impl Ty {
    pub fn parse(s: &str) -> Result<Ty, String> {
        let s = s.trim();
        if let Some(inner) = s.strip_prefix("tuple[").and_then(|r| r.strip_suffix(']')) {
            if inner.trim().is_empty() {
                return Ok(Ty::Tuple(vec![]));
            }
            let mut v = Vec::new();
            for part in split_top(inner) {
                v.push(Ty::parse(&part)?);
            }
            return Ok(Ty::Tuple(v));
        }
        if let Some(inner) = s.strip_prefix("list[").and_then(|r| r.strip_suffix(']')) {
            return Ok(Ty::List(Box::new(Ty::parse(inner)?)));
        }
        if let Some(kind) = s.strip_prefix("handle:") {
            return Ok(Ty::Handle(Some(kind.to_string())));
        }
        Ok(match s {
            "int" => Ty::Int,
            "uint" => Ty::Uint,
            "long" => Ty::Long,
            "ulong" => Ty::Ulong,
            "byte" => Ty::Byte,
            "word" => Ty::Word,
            "float" => Ty::Float,
            "double" => Ty::Double,
            "bool" => Ty::Bool,
            "str" => Ty::Str,
            "bytes" => Ty::Bytes,
            "object" => Ty::Object,
            "any" => Ty::Any,
            "none" => Ty::None,
            "handle" => Ty::Handle(None),
            "dict" => Ty::Dict,
            other => return Err(format!("unknown type `{other}`")),
        })
    }

    pub fn is_scalar_number(&self) -> bool {
        matches!(self, Ty::Int | Ty::Uint | Ty::Long | Ty::Ulong | Ty::Byte | Ty::Word | Ty::Float | Ty::Double | Ty::Bool)
    }
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Int => write!(f, "int"),
            Ty::Uint => write!(f, "uint"),
            Ty::Long => write!(f, "long"),
            Ty::Ulong => write!(f, "ulong"),
            Ty::Byte => write!(f, "byte"),
            Ty::Word => write!(f, "word"),
            Ty::Float => write!(f, "float"),
            Ty::Double => write!(f, "double"),
            Ty::Bool => write!(f, "bool"),
            Ty::Str => write!(f, "str"),
            Ty::Bytes => write!(f, "bytes"),
            Ty::Object => write!(f, "object"),
            Ty::Any => write!(f, "any"),
            Ty::None => write!(f, "none"),
            Ty::Handle(None) => write!(f, "handle"),
            Ty::Handle(Some(k)) => write!(f, "handle:{k}"),
            Ty::Dict => write!(f, "dict"),
            Ty::Tuple(v) => {
                write!(f, "tuple[")?;
                for (i, t) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, "]")
            }
            Ty::List(t) => write!(f, "list[{t}]"),
        }
    }
}

fn split_top(s: &str) -> Vec<String> {
    let mut depth = 0;
    let mut cur = String::new();
    let mut out = Vec::new();
    for c in s.chars() {
        match c {
            '[' => {
                depth += 1;
                cur.push(c)
            }
            ']' => {
                depth -= 1;
                cur.push(c)
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// One positional argument: `name: type` (required) or `name: type = default`
/// / `name: type?` (optional; the `= default` text is the C++ initialiser).
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
    pub optional: bool,
    pub default: Option<String>,
}

impl Param {
    pub fn parse(s: &str) -> Result<Param, String> {
        let (name, rest) = s.split_once(':').ok_or_else(|| format!("bad param `{s}`"))?;
        let rest = rest.trim();
        let (ty_s, default) = match rest.split_once('=') {
            Some((t, d)) => (t.trim(), Some(d.trim().to_string())),
            None => (rest, None),
        };
        let (ty_s, q) = match ty_s.strip_suffix('?') {
            Some(t) => (t, true),
            None => (ty_s, false),
        };
        Ok(Param { name: name.trim().to_string(), ty: Ty::parse(ty_s)?, optional: q || default.is_some(), default })
    }
}

impl std::fmt::Display for Param {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.ty)?;
        if let Some(d) = &self.default {
            write!(f, " = {d}")
        } else if self.optional {
            write!(f, "?")
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub cfunc: String,
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub line: usize,
    /// "varargs" (METH_VARARGS) or "noargs" (METH_NOARGS).
    #[serde(default)]
    pub flags: String,
    /// `#if` condition guarding the table entry, if any.
    #[serde(default)]
    pub cfg: String,
    /// "classified", "partial" (args or returns unclear) or "unclassified".
    #[serde(default)]
    pub status: String,
    /// What the C++ does when an argument is missing or of the wrong type:
    /// "none" (`Py_BuildException()`: returns None, error cleared),
    /// "bad_argument" (`Py_BadArgument()`: TypeError), "parse_tuple"
    /// (`PyArg_ParseTuple` failed: its own TypeError), "mixed" or "".
    #[serde(default)]
    pub on_bad_arg: String,
    /// One list of `name: type` strings per accepted argument-count form.
    #[serde(default)]
    pub overloads: Vec<Vec<String>>,
    #[serde(default)]
    pub returns: String,
    /// True when the C++ also returns None on a failure path of a value function.
    #[serde(default)]
    pub returns_none: bool,
    #[serde(default)]
    pub returns_variants: Vec<String>,
    /// Explicit Python errors the body raises: "Kind: message".
    #[serde(default)]
    pub errors: Vec<String>,
    /// C++ singletons/managers the body calls.
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub quirks: Vec<String>,
    /// Why the function is not fully classified ("args: ..." / "returns: ...").
    #[serde(default)]
    pub unclassified: Vec<String>,
}

impl Function {
    pub fn parsed_overloads(&self) -> Result<Vec<Vec<Param>>, String> {
        self.overloads.iter().map(|o| o.iter().map(|p| Param::parse(p)).collect()).collect()
    }
    pub fn return_ty(&self) -> Ty {
        Ty::parse(&self.returns).unwrap_or(Ty::Any)
    }
    pub fn args_ok(&self) -> bool {
        !self.unclassified.iter().any(|r| r.starts_with("args:"))
    }
    pub fn returns_ok(&self) -> bool {
        !self.unclassified.iter().any(|r| r.starts_with("returns:"))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Constant {
    pub name: String,
    /// "int", "float" or "str".
    #[serde(rename = "type", default)]
    pub ty: String,
    /// The value as text (taken from the API snapshot when available).
    #[serde(default)]
    pub value: String,
    /// The C++ expression passed to `PyModule_AddIntConstant`, if found.
    #[serde(default)]
    pub expr: String,
    #[serde(default)]
    pub cfg: String,
}

/// A place where C++ calls back into Python (`PyCallClassMemberFunc`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Callback {
    pub file: String,
    pub line: usize,
    /// Python method name when it is a string literal, else the expression.
    pub handler: String,
    /// "PyCallClassMemberFunc", "PyCallClassMemberFunc_ByPyString", ...
    #[serde(default)]
    pub via: String,
    /// The C++ function containing the call.
    #[serde(default, rename = "in")]
    pub in_fn: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ModuleSpec {
    pub module: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default, rename = "function")]
    pub functions: Vec<Function>,
    #[serde(default, rename = "constant")]
    pub constants: Vec<Constant>,
    #[serde(default, rename = "callback")]
    pub callbacks: Vec<Callback>,
}

fn q(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

fn qlist(v: &[String]) -> String {
    let items: Vec<String> = v.iter().map(|s| q(s)).collect();
    format!("[{}]", items.join(", "))
}

impl ModuleSpec {
    pub fn parse(text: &str) -> Result<ModuleSpec, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Deterministic TOML: fixed field order, sorted entries.
    pub fn to_toml(&self) -> String {
        let mut o = String::new();
        let _ = writeln!(o, "# Generated by `m2-modgen extract`; see crates/m2-modgen/README.md. Do not edit by hand.");
        let _ = writeln!(o, "module = {}", q(&self.module));
        let _ = writeln!(o, "sources = {}", qlist(&self.sources));
        for f in &self.functions {
            let _ = writeln!(o, "\n[[function]]");
            let _ = writeln!(o, "name = {}", q(&f.name));
            let _ = writeln!(o, "cfunc = {}", q(&f.cfunc));
            let _ = writeln!(o, "file = {}", q(&f.file));
            let _ = writeln!(o, "line = {}", f.line);
            let _ = writeln!(o, "flags = {}", q(&f.flags));
            if !f.cfg.is_empty() {
                let _ = writeln!(o, "cfg = {}", q(&f.cfg));
            }
            let _ = writeln!(o, "status = {}", q(&f.status));
            if !f.on_bad_arg.is_empty() {
                let _ = writeln!(o, "on_bad_arg = {}", q(&f.on_bad_arg));
            }
            if f.overloads.len() == 1 {
                let _ = writeln!(o, "overloads = [{}]", qlist(&f.overloads[0]));
            } else {
                let _ = writeln!(o, "overloads = [");
                for ov in &f.overloads {
                    let _ = writeln!(o, "  {},", qlist(ov));
                }
                let _ = writeln!(o, "]");
            }
            let _ = writeln!(o, "returns = {}", q(&f.returns));
            if f.returns_none {
                let _ = writeln!(o, "returns_none = true");
            }
            if !f.returns_variants.is_empty() {
                let _ = writeln!(o, "returns_variants = {}", qlist(&f.returns_variants));
            }
            if !f.errors.is_empty() {
                let _ = writeln!(o, "errors = {}", qlist(&f.errors));
            }
            if !f.owners.is_empty() {
                let _ = writeln!(o, "owners = {}", qlist(&f.owners));
            }
            if !f.quirks.is_empty() {
                let _ = writeln!(o, "quirks = {}", qlist(&f.quirks));
            }
            if !f.unclassified.is_empty() {
                let _ = writeln!(o, "unclassified = {}", qlist(&f.unclassified));
            }
        }
        for c in &self.constants {
            let _ = writeln!(o, "\n[[constant]]");
            let _ = writeln!(o, "name = {}", q(&c.name));
            let _ = writeln!(o, "type = {}", q(&c.ty));
            let _ = writeln!(o, "value = {}", q(&c.value));
            if !c.expr.is_empty() {
                let _ = writeln!(o, "expr = {}", q(&c.expr));
            }
            if !c.cfg.is_empty() {
                let _ = writeln!(o, "cfg = {}", q(&c.cfg));
            }
        }
        for c in &self.callbacks {
            let _ = writeln!(o, "\n[[callback]]");
            let _ = writeln!(o, "file = {}", q(&c.file));
            let _ = writeln!(o, "line = {}", c.line);
            let _ = writeln!(o, "handler = {}", q(&c.handler));
            let _ = writeln!(o, "via = {}", q(&c.via));
            let _ = writeln!(o, "in = {}", q(&c.in_fn));
        }
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_round_trip() {
        for s in ["int", "tuple[int, str]", "list[tuple[int, int]]", "handle:window", "tuple[]", "bytes", "dict"] {
            assert_eq!(Ty::parse(s).unwrap().to_string(), s);
        }
        assert!(Ty::parse("nope").is_err());
    }

    #[test]
    fn param_round_trip() {
        for s in ["x: int", "life: int = CActorInstance::EFFECT_LIFE_INFINITE", "name: str?", "w: handle:window"] {
            assert_eq!(Param::parse(s).unwrap().to_string(), s);
        }
        assert!(Param::parse("x: int = 5").unwrap().optional);
    }

    #[test]
    fn spec_round_trip() {
        let m = ModuleSpec {
            module: "m".into(),
            sources: vec!["a.cpp".into()],
            functions: vec![Function {
                name: "F".into(),
                cfunc: "mF".into(),
                overloads: vec![vec!["x: int".into()], vec![]],
                returns: "tuple[int, int]".into(),
                status: "classified".into(),
                ..Default::default()
            }],
            constants: vec![Constant { name: "K".into(), ty: "int".into(), value: "3".into(), ..Default::default() }],
            callbacks: vec![],
        };
        let t = m.to_toml();
        let back = ModuleSpec::parse(&t).unwrap();
        assert_eq!(back.to_toml(), t);
        assert_eq!(back.functions[0].overloads.len(), 2);
    }
}
