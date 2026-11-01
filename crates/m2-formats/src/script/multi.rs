//! `LoadMultipleTextData` (`neoeter/src/render/Util.cpp`): the flat format of
//! map `setting.txt`, area and texture-set files.
//!
//! ```text
//! ScriptType MapSetting
//! CellScale 200
//! Start TextureSet
//!     textureset/a.txt
//! End
//! ```

use std::collections::BTreeMap;

use crate::pack::{Error, PackManager};
use crate::text::{self, DEFAULT_DELIMITERS};

/// `CTokenVectorMap`: keys in `std::map` (byte) order.
pub type TokenVectorMap = BTreeMap<Vec<u8>, Vec<Vec<u8>>>;

/// Parses like `LoadMultipleTextData` after reading the file.
///
/// Each line's first token is lowercased and is the key of the rest of the
/// line, or `start <name>`: every token up to a line starting with `end`
/// becomes the value of `<name>` (lowercased), with the first token of each
/// of those lines lowercased too. The first occurrence of a key wins.
///
/// Deviation: for a `start` line without a name C++ reads past the end of
/// the token vector (undefined behaviour); here the name is empty.
pub fn parse_multiple_text_data(data: &[u8]) -> TokenVectorMap {
    let lines = text::lines(data);
    let mut map = TokenVectorMap::new();

    let mut i = 0;
    while i < lines.len() {
        let Some(tokens) = text::split_line(lines[i], DEFAULT_DELIMITERS) else {
            i += 1;
            continue;
        };
        let head = tokens[0].to_ascii_lowercase();

        if head == b"start" {
            let key = tokens.get(1).map_or_else(Vec::new, |t| t.to_ascii_lowercase());
            let mut values = Vec::new();
            i += 1;
            while i < lines.len() {
                if let Some(sub) = text::split_line(lines[i], DEFAULT_DELIMITERS) {
                    let first = sub[0].to_ascii_lowercase();
                    if first == b"end" {
                        break;
                    }
                    values.push(first);
                    values.extend(sub[1..].iter().map(|t| t.to_vec()));
                }
                i += 1;
            }
            map.entry(key).or_insert(values);
        } else {
            map.entry(head).or_insert_with(|| tokens[1..].iter().map(|t| t.to_vec()).collect());
        }
        i += 1;
    }
    map
}

/// `LoadMultipleTextData`: reads `name` through the pack manager and parses
/// it. A missing file is the only failure.
pub fn load_multiple_text_data(manager: &PackManager, name: impl AsRef<[u8]>) -> Result<TokenVectorMap, Error> {
    Ok(parse_multiple_text_data(&manager.get(name)?))
}
