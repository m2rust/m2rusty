//! The `pack/Index` file: one `name<TAB>container` line per pack, registered
//! in order (`PackInitialize` in `client/src/ui/UserInterface.cpp`).

use std::fs;

use super::{Container, Error, PackManager};
use crate::text;

/// What `load_index` registered. C++ ignores registration failures; they are
/// collected here instead.
#[derive(Debug, Default)]
pub struct IndexReport {
    /// Pack names as registered (`<pack folder>/<name>`), `_texcache` included.
    pub registered: Vec<String>,
    pub failed: Vec<(String, Error)>,
}

/// Registers every pack listed in `index_file` (relative to the manager's
/// root). Each pack is `<pack_folder>/<name>`, followed by
/// `<name>_texcache` when that exists.
///
/// Errors on the first malformed line, like C++, keeping the packs before it.
/// One deliberate difference: C++ always skips the last line, which is only
/// correct when the file ends with a newline; here an unterminated last line
/// is read too.
pub fn load_index(manager: &mut PackManager, pack_folder: &str, index_file: &str) -> Result<IndexReport, Error> {
    let bytes = fs::read(manager.root().join(index_file))?;
    let mut lines = text::lines(&bytes);
    if lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    let mut report = IndexReport::default();
    for (number, line) in lines.into_iter().enumerate() {
        let tokens = text::split_by_tab(line);
        let [name, container] = tokens[..] else {
            return Err(Error::IndexLine {
                line: number,
                tokens: tokens.len(),
            });
        };
        let container = Container::parse(container)?;
        let name = format!("{pack_folder}/{}", String::from_utf8_lossy(name));

        let texcache = format!("{name}_texcache");
        for (pack, required) in [(name, true), (texcache, false)] {
            if !required && !manager.pack_exists(&pack, container) {
                continue;
            }
            match manager.register(&pack, container) {
                Ok(()) => report.registered.push(pack),
                Err(e) => report.failed.push((pack, e)),
            }
        }
    }
    Ok(report)
}
