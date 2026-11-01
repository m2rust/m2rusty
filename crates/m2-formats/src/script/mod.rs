//! The grouped text format of `CTextFileLoader`
//! (`neoeter/src/render/TextFileLoader.cpp`), used by `.msm`, `.msa`,
//! `.mse`, `.msf`, `.mss`, `.msenv` and other scripts:
//!
//! ```text
//! ScriptType  MotionData
//! MotionDuration 1.5
//! Group Event00
//! {
//!     StartingTime 0.2
//!     List Positions { 1 2 3
//!         4 5 6 }
//! }
//! ```
//!
//! Lines split on spaces and tabs (`"…"` quotes a token). The first token of
//! a line is lowercased and is `Group <name>`, `List <name>`, `{`, `}` or a
//! key followed by its values. Group names and keys are stored lowercased.
//!
//! C++ walks the tree with a cursor (`SetChildNode`/`SetParentNode`); here
//! [`Script::root`] is a [`Node`] tree and callers hold references instead.
//! The token getters are methods of [`Node`] returning `Option` where C++
//! returns `FALSE`.
//!
//! Also here: [`parse_multiple_text_data`] (`LoadMultipleTextData`, the
//! flat `key values` / `start … end` format of map settings and area files)
//! and the `atoi`/`atof` semantics every caller relies on.

mod multi;
mod number;

pub use multi::{TokenVectorMap, load_multiple_text_data, parse_multiple_text_data};
pub use number::{atof, atof_f32, atoi, to_f32};

use crate::pack::{Error, PackManager};
use crate::text::{self, DEFAULT_DELIMITERS};

/// `CTextFileLoader::SGroupNode::GenNameKey`: plain CRC-32 of the bytes.
/// Keys and child names are found by this hash, not by comparing bytes.
pub fn name_key(name: &[u8]) -> u32 {
    crc32fast::hash(name)
}

/// One `key values…` line or `List` of a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Lowercased key.
    pub key: Vec<u8>,
    /// The tokens after the key (for a list, every token between its braces).
    pub tokens: Vec<Vec<u8>>,
}

/// A group (`SGroupNode`): its keys and child groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// Lowercased group name; `global` for the root.
    pub name: Vec<u8>,
    /// Entries in file order. When two keys have the same [`name_key`] only
    /// the first is kept (`std::map::insert` does not overwrite).
    pub entries: Vec<Entry>,
    /// Child groups in file order.
    pub children: Vec<Node>,
}

impl Node {
    fn new(name: Vec<u8>) -> Self {
        Self {
            name,
            entries: Vec::new(),
            children: Vec::new(),
        }
    }

    /// `SGroupNode::InsertTokenVector`: keeps the first entry per key hash.
    fn insert(&mut self, key: Vec<u8>, tokens: Vec<Vec<u8>>) {
        let hash = name_key(&key);
        if !self.entries.iter().any(|e| name_key(&e.key) == hash) {
            self.entries.push(Entry { key, tokens });
        }
    }

    /// `SetChildNode(const char*)`: the first child whose (lowercased) name
    /// hashes like `name`. `name` is not lowercased, so callers pass
    /// lowercase names, as the C++ callers do.
    pub fn child(&self, name: impl AsRef<[u8]>) -> Option<&Node> {
        let hash = name_key(name.as_ref());
        self.children.iter().find(|c| name_key(&c.name) == hash)
    }

    /// `SetChildNode(head, index)`: the child named `head` followed by the
    /// index as at least two digits (`event00`), cut to 32 bytes like the
    /// C++ `_snprintf` into a 33-byte buffer.
    pub fn child_indexed(&self, head: impl AsRef<[u8]>, index: u32) -> Option<&Node> {
        let mut name = head.as_ref().to_vec();
        name.extend_from_slice(format!("{index:02}").as_bytes());
        name.truncate(32);
        self.child(name)
    }

    /// `IsToken`.
    pub fn has_token(&self, key: impl AsRef<[u8]>) -> bool {
        self.tokens(key).is_some()
    }

    /// `GetTokenVector`: the tokens of `key` (looked up by hash, not
    /// lowercased). May be empty for a `List` with nothing in it.
    pub fn tokens(&self, key: impl AsRef<[u8]>) -> Option<&[Vec<u8>]> {
        let hash = name_key(key.as_ref());
        self.entries
            .iter()
            .find(|e| name_key(&e.key) == hash)
            .map(|e| e.tokens.as_slice())
    }

    /// The first token of `key`; `None` if the key is missing or has none.
    fn first(&self, key: impl AsRef<[u8]>) -> Option<&[u8]> {
        self.tokens(key)?.first().map(Vec::as_slice)
    }

    /// `GetTokenString`: the first token.
    pub fn token_string(&self, key: impl AsRef<[u8]>) -> Option<&[u8]> {
        self.first(key)
    }

    /// `GetTokenInteger`: `atoi` of the first token.
    pub fn token_int(&self, key: impl AsRef<[u8]>) -> Option<i32> {
        self.first(key).map(atoi)
    }

    /// `GetTokenDoubleWord`: `atoi` of the first token, reinterpreted.
    pub fn token_dword(&self, key: impl AsRef<[u8]>) -> Option<u32> {
        self.token_int(key).map(|v| v as u32)
    }

    /// `GetTokenWord`: `atoi` of the first token, truncated.
    pub fn token_word(&self, key: impl AsRef<[u8]>) -> Option<u16> {
        self.token_int(key).map(|v| v as u16)
    }

    /// `GetTokenByte`: `atoi` of the first token, truncated.
    pub fn token_byte(&self, key: impl AsRef<[u8]>) -> Option<u8> {
        self.token_int(key).map(|v| v as u8)
    }

    /// `GetTokenBoolean`: `atoi` of the first token, nonzero is true.
    pub fn token_bool(&self, key: impl AsRef<[u8]>) -> Option<bool> {
        self.token_int(key).map(|v| v != 0)
    }

    /// `GetTokenFloat`: `(float)atof` of the first token.
    pub fn token_float(&self, key: impl AsRef<[u8]>) -> Option<f32> {
        self.first(key).map(atof_f32)
    }

    /// `(float)atof` of every token, when there are exactly `N`. This is
    /// `GetTokenVector2` (N = 2), `GetTokenVector3`/`Position`/`Direction`
    /// (3) and `GetTokenVector4`/`Quaternion`/`Color` (4, in x y z w and
    /// r g b a order).
    pub fn token_floats<const N: usize>(&self, key: impl AsRef<[u8]>) -> Option<[f32; N]> {
        let tokens = self.tokens(key)?;
        if tokens.len() != N {
            return None;
        }
        Some(std::array::from_fn(|i| atof_f32(&tokens[i])))
    }

    /// `GetTokenVector2`.
    pub fn token_vector2(&self, key: impl AsRef<[u8]>) -> Option<[f32; 2]> {
        self.token_floats(key)
    }

    /// `GetTokenVector3`, `GetTokenPosition`, `GetTokenDirection`.
    pub fn token_vector3(&self, key: impl AsRef<[u8]>) -> Option<[f32; 3]> {
        self.token_floats(key)
    }

    /// `GetTokenVector4`, `GetTokenQuaternion` (x y z w), `GetTokenColor`
    /// (r g b a).
    pub fn token_vector4(&self, key: impl AsRef<[u8]>) -> Option<[f32; 4]> {
        self.token_floats(key)
    }
}

/// A loaded script (`CTextFileLoader` after `Load`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// The `global` group.
    pub root: Node,
    /// What `Load` returned for a file that exists: false when braces do not
    /// balance (or a group ended early on a key without a value). The tree
    /// is still what C++ built up to that point, and `CTextFileLoader::Cache`
    /// users (motion, sound scripts) use it regardless.
    pub complete: bool,
}

/// Parses a script like `CTextFileLoader::Load` after reading the file.
pub fn parse(data: &[u8]) -> Script {
    let mut parser = Parser {
        lines: text::lines(data),
        line: 0,
    };
    let mut root = Node::new(b"global".to_vec());
    let complete = parser.load_group(&mut root);
    Script { root, complete }
}

/// `CTextFileLoader::Load` (and `Cache`, which loads the same way): reads
/// `name` through the pack manager and parses it. A missing file is an
/// error, where C++ leaves the loader empty (`IsEmpty`).
pub fn load(manager: &PackManager, name: impl AsRef<[u8]>) -> Result<Script, Error> {
    Ok(parse(&manager.get(name)?))
}

struct Parser<'a> {
    lines: Vec<&'a [u8]>,
    /// `m_dwcurLineIndex`, shared by every nesting level as in C++.
    line: usize,
}

fn first_byte(token: &[u8]) -> u8 {
    // std::string::operator[] at size() is '\0'.
    token.first().copied().unwrap_or(0)
}

impl Parser<'_> {
    /// `CTextFileLoader::LoadGroup`, statement for statement.
    fn load_group(&mut self, node: &mut Node) -> bool {
        let mut depth = 0i32;

        while self.line < self.lines.len() {
            // Error -2 (unterminated quote) is only traced in C++.
            let Ok(tokens) = text::split_line2(self.lines[self.line], DEFAULT_DELIMITERS) else {
                self.line += 1;
                continue;
            };
            let head = tokens[0].to_ascii_lowercase();

            match first_byte(&head) {
                b'{' => {
                    depth += 1;
                    self.line += 1;
                    continue;
                }
                b'}' => {
                    depth -= 1;
                    break;
                }
                _ => {}
            }

            if head == b"group" {
                if tokens.len() == 2 {
                    let mut child = Node::new(tokens[1].to_ascii_lowercase());
                    self.line += 1;
                    let ok = self.load_group(&mut child);
                    // C++ links the child before loading it.
                    node.children.push(child);
                    if !ok {
                        return false;
                    }
                }
            } else if head == b"list" {
                if tokens.len() == 2 {
                    let key = tokens[1].to_ascii_lowercase();
                    let mut values = Vec::new();
                    self.line += 1;
                    while self.line < self.lines.len() {
                        if let Some(sub) = text::split_line(self.lines[self.line], DEFAULT_DELIMITERS) {
                            match first_byte(sub[0]) {
                                b'{' => {}
                                b'}' => break,
                                _ => values.extend(sub.iter().map(|t| t.to_vec())),
                            }
                        }
                        self.line += 1;
                    }
                    node.insert(key, values);
                }
            } else {
                if tokens.len() == 1 {
                    // "must have a value": ends this group without closing it.
                    break;
                }
                node.insert(head, tokens[1..].iter().map(|t| t.to_vec()).collect());
            }
            self.line += 1;
        }

        depth == 0
    }
}
