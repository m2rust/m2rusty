//! A small C++ lexer: enough to find function bodies, calls and string literals.
//! Comments are dropped; preprocessor lines are consumed and tracked so every
//! token carries the `#if` condition that was active when it was lexed.

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Ident,
    Number,
    Str,
    Char,
    Punct,
}

#[derive(Clone, Debug)]
pub struct Tok {
    pub kind: Kind,
    /// Source text; for `Str` the decoded contents (simple escapes only).
    pub text: String,
    pub line: usize,
    /// Active `#if` condition when the token was lexed ("" = unconditional).
    pub cfg: String,
}

impl Tok {
    /// True for a non-string token with exactly this text.
    pub fn is(&self, s: &str) -> bool {
        self.kind != Kind::Str && self.text == s
    }
    pub fn is_ident(&self) -> bool {
        self.kind == Kind::Ident
    }
}

fn negate(c: &str) -> String {
    match c.strip_prefix('!') {
        Some(rest) if !rest.contains(' ') => rest.to_string(),
        _ if c.contains(' ') => format!("!({c})"),
        _ => format!("!{c}"),
    }
}

/// Turns `#ifdef X` / `#ifndef X` / `#if expr` into a compact condition.
fn directive_condition(kw: &str, rest: &str) -> String {
    let rest = rest.trim();
    match kw {
        "ifdef" => rest.split_whitespace().next().unwrap_or("").to_string(),
        "ifndef" => format!("!{}", rest.split_whitespace().next().unwrap_or("")),
        _ => rest.replace("defined(", "").replace("defined (", "").replace(')', "").replace("  ", " ").trim().to_string(),
    }
}

fn current_cfg(stack: &[(String, bool)]) -> String {
    let parts: Vec<String> =
        stack.iter().filter(|(c, _)| !c.is_empty()).map(|(c, neg)| if *neg { negate(c) } else { c.clone() }).collect();
    parts.join(" && ")
}

pub fn lex(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut line = 1;
    let mut out = Vec::new();
    let mut conds: Vec<(String, bool)> = Vec::new();
    let mut at_line_start = true;
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            at_line_start = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '#' && at_line_start {
            // Preprocessor line (with continuations).
            let mut text = String::new();
            while i < b.len() {
                if b[i] == '\\' && i + 1 < b.len() && b[i + 1] == '\n' {
                    line += 1;
                    i += 2;
                    continue;
                }
                if b[i] == '\n' {
                    break;
                }
                if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '/' {
                    while i < b.len() && b[i] != '\n' {
                        i += 1;
                    }
                    break;
                }
                if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                    i += 2;
                    while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                        if b[i] == '\n' {
                            line += 1;
                        }
                        i += 1;
                    }
                    i += 2;
                    continue;
                }
                text.push(b[i]);
                i += 1;
            }
            let t = text.trim_start_matches('#').trim();
            let (kw, rest) = match t.find(|ch: char| ch.is_whitespace()) {
                Some(p) => (&t[..p], &t[p..]),
                None => (t, ""),
            };
            match kw {
                "if" | "ifdef" | "ifndef" => conds.push((directive_condition(kw, rest), false)),
                "elif" => {
                    if let Some(top) = conds.last_mut() {
                        *top = (directive_condition("if", rest), false);
                    }
                }
                "else" => {
                    if let Some(top) = conds.last_mut() {
                        top.1 = !top.1;
                    }
                }
                "endif" => {
                    conds.pop();
                }
                _ => {}
            }
            continue;
        }
        at_line_start = false;
        if c == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == '*' && b[i + 1] == '/') {
                if b[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
            continue;
        }
        let cfg = current_cfg(&conds);
        if c == '"' || (c == 'L' && i + 1 < b.len() && b[i + 1] == '"') {
            if c == 'L' {
                i += 1;
            }
            i += 1;
            let mut s = String::new();
            let start_line = line;
            while i < b.len() && b[i] != '"' {
                if b[i] == '\\' && i + 1 < b.len() {
                    i += 1;
                    match b[i] {
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        '0' => s.push('\0'),
                        other => s.push(other),
                    }
                } else {
                    if b[i] == '\n' {
                        line += 1;
                    }
                    s.push(b[i]);
                }
                i += 1;
            }
            i += 1;
            out.push(Tok { kind: Kind::Str, text: s, line: start_line, cfg });
            continue;
        }
        if c == '\'' {
            let mut s = String::from("'");
            i += 1;
            while i < b.len() && b[i] != '\'' {
                if b[i] == '\\' && i + 1 < b.len() {
                    s.push(b[i]);
                    i += 1;
                }
                s.push(b[i]);
                i += 1;
            }
            i += 1;
            s.push('\'');
            out.push(Tok { kind: Kind::Char, text: s, line, cfg });
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let st = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == '_') {
                i += 1;
            }
            out.push(Tok { kind: Kind::Ident, text: b[st..i].iter().collect(), line, cfg });
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let st = i;
            let hex = c == '0' && i + 1 < b.len() && (b[i + 1] == 'x' || b[i + 1] == 'X');
            while i < b.len()
                && (b[i].is_ascii_alphanumeric()
                    || b[i] == '.'
                    || b[i] == '_'
                    || (!hex && (b[i] == '-' || b[i] == '+') && matches!(b[i - 1], 'e' | 'E')))
            {
                i += 1;
            }
            out.push(Tok { kind: Kind::Number, text: b[st..i].iter().collect(), line, cfg });
            continue;
        }
        let two: String = b[i..(i + 2).min(b.len())].iter().collect();
        if matches!(
            two.as_str(),
            "::" | "->" | "&&" | "||" | "==" | "!=" | "<=" | ">=" | "<<" | ">>" | "++" | "--" | "+=" | "-=" | "*=" | "/="
        ) {
            out.push(Tok { kind: Kind::Punct, text: two, line, cfg });
            i += 2;
        } else {
            out.push(Tok { kind: Kind::Punct, text: c.to_string(), line, cfg });
            i += 1;
        }
    }
    out
}

/// For every bracket, the index of its partner.
pub fn match_brackets(toks: &[Tok]) -> Vec<Option<usize>> {
    let mut m = vec![None; toks.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind != Kind::Punct {
            continue;
        }
        match t.text.as_str() {
            "(" | "{" | "[" => stack.push(i),
            ")" | "}" | "]" => {
                if let Some(o) = stack.pop() {
                    m[o] = Some(i);
                    m[i] = Some(o);
                }
            }
            _ => {}
        }
    }
    m
}

/// Source text of a token range, for messages.
pub fn join(toks: &[Tok]) -> String {
    let mut s = String::new();
    for (i, t) in toks.iter().enumerate() {
        let txt = match t.kind {
            Kind::Str => format!("\"{}\"", t.text),
            _ => t.text.clone(),
        };
        if i > 0 {
            let prev = &toks[i - 1];
            let tight = (prev.kind == Kind::Punct && matches!(prev.text.as_str(), "(" | "::" | "." | "->" | "&" | "!" | "[" | "~"))
                || (t.kind == Kind::Punct && matches!(t.text.as_str(), ")" | "(" | "::" | "." | "->" | "," | ";" | "[" | "]"));
            if !tight {
                s.push(' ');
            }
        }
        s.push_str(&txt);
    }
    s
}
