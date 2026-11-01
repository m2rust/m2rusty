//! Virtual-folder tree, filter and the flattened visible rows the
//! virtualised list draws. Pure functions, no Bevy.

use std::collections::{BTreeMap, HashSet};

use crate::index::{Entry, FileKind};

#[derive(Debug, Default)]
pub struct Dir {
    pub name: String,
    pub dirs: BTreeMap<String, usize>,
    /// Indices into the entry list.
    pub files: Vec<u32>,
}

#[derive(Debug)]
pub struct Tree {
    /// `dirs[0]` is the root.
    pub dirs: Vec<Dir>,
}

impl Tree {
    pub fn build(entries: &[Entry]) -> Tree {
        let mut dirs = vec![Dir::default()];
        for (i, e) in entries.iter().enumerate() {
            let mut cur = 0usize;
            let mut parts = e.display().split('/').peekable();
            while let Some(part) = parts.next() {
                if parts.peek().is_none() {
                    dirs[cur].files.push(i as u32);
                    break;
                }
                cur = match dirs[cur].dirs.get(part) {
                    Some(&d) => d,
                    None => {
                        let id = dirs.len();
                        dirs.push(Dir { name: part.to_string(), ..Dir::default() });
                        dirs[cur].dirs.insert(part.to_string(), id);
                        id
                    }
                };
            }
        }
        Tree { dirs }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    pub query: String,
    pub kinds: [bool; FileKind::ALL.len()],
}

impl Default for Filter {
    fn default() -> Self {
        Filter { query: String::new(), kinds: [true; FileKind::ALL.len()] }
    }
}

impl Filter {
    /// Every whitespace-separated term of the query must occur in the display path.
    pub fn matches(&self, e: &Entry) -> bool {
        if !self.kinds[e.kind.index()] {
            return false;
        }
        self.query.split_whitespace().all(|t| e.display().contains(&t.to_lowercase()))
    }
    pub fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKey {
    Dir(usize),
    Group(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Dir { dir: usize, depth: u16, expanded: bool, count: u32 },
    Group { kind: FileKind, expanded: bool, count: u32 },
    File { entry: u32, depth: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Folders,
    ByType,
}

pub fn visible_rows(
    entries: &[Entry],
    tree: &Tree,
    filter: &Filter,
    expanded: &HashSet<NodeKey>,
    mode: ViewMode,
) -> Vec<Row> {
    let mut out = Vec::new();
    match mode {
        ViewMode::Folders => {
            walk(entries, tree, filter, expanded, 0, 0, &mut out);
        }
        ViewMode::ByType => {
            for kind in FileKind::ALL {
                let files: Vec<u32> =
                    (0..entries.len() as u32).filter(|&i| entries[i as usize].kind == kind && filter.matches(&entries[i as usize])).collect();
                if files.is_empty() {
                    continue;
                }
                let open = filter.searching() || expanded.contains(&NodeKey::Group(kind.index()));
                out.push(Row::Group { kind, expanded: open, count: files.len() as u32 });
                if open {
                    out.extend(files.into_iter().map(|entry| Row::File { entry, depth: 1 }));
                }
            }
        }
    }
    out
}

/// Appends the rows of `dir`'s children; returns the number of matching files below it.
fn walk(entries: &[Entry], tree: &Tree, filter: &Filter, expanded: &HashSet<NodeKey>, dir: usize, depth: u16, out: &mut Vec<Row>) -> u32 {
    let d = &tree.dirs[dir];
    let mut total = 0;
    for &child in d.dirs.values() {
        let at = out.len();
        let open = filter.searching() || expanded.contains(&NodeKey::Dir(child));
        out.push(Row::Dir { dir: child, depth, expanded: open, count: 0 });
        // Rows of a collapsed folder are computed to count, then dropped.
        let count = walk(entries, tree, filter, expanded, child, depth + 1, out);
        if count == 0 {
            out.truncate(at);
            continue;
        }
        if !open {
            out.truncate(at + 1);
        }
        out[at] = Row::Dir { dir: child, depth, expanded: open, count };
        total += count;
    }
    for &f in &d.files {
        if filter.matches(&entries[f as usize]) {
            total += 1;
            out.push(Row::File { entry: f, depth });
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<Entry> {
        let mut v: Vec<Entry> = ["d:/ymir work/a/x.gr2", "d:/ymir work/a/y.dds", "d:/ymir work/b/z.txt", "locale/q.py", "root.msm"]
            .iter()
            .map(|s| Entry::new(s.to_string()))
            .collect();
        v.sort_by(|a, b| a.display().cmp(b.display()));
        v
    }

    #[test]
    fn collapsed_and_expanded() {
        let e = entries();
        let t = Tree::build(&e);
        let f = Filter::default();
        let rows = visible_rows(&e, &t, &f, &HashSet::new(), ViewMode::Folders);
        // locale, ymir work (collapsed) + root.msm
        assert_eq!(rows.len(), 3);
        assert!(matches!(rows[1], Row::Dir { count: 3, expanded: false, .. }));
        let ymir = t.dirs[0].dirs["ymir work"];
        let ex: HashSet<_> = [NodeKey::Dir(ymir)].into();
        let rows = visible_rows(&e, &t, &f, &ex, ViewMode::Folders);
        assert_eq!(rows.len(), 5); // + a, b collapsed
    }

    #[test]
    fn search_expands_and_hides() {
        let e = entries();
        let t = Tree::build(&e);
        let f = Filter { query: "Y.dds".into(), ..Filter::default() };
        let rows = visible_rows(&e, &t, &f, &HashSet::new(), ViewMode::Folders);
        assert_eq!(rows.len(), 3); // ymir work, a, file
        assert!(matches!(rows[2], Row::File { depth: 2, .. }));
    }

    #[test]
    fn kind_filter_and_groups() {
        let e = entries();
        let t = Tree::build(&e);
        let mut f = Filter::default();
        f.kinds[FileKind::Text.index()] = false;
        let rows = visible_rows(&e, &t, &f, &HashSet::new(), ViewMode::ByType);
        assert!(rows.iter().all(|r| !matches!(r, Row::Group { kind: FileKind::Text, .. })));
        assert_eq!(rows.len(), 3); // gr2, texture, race groups collapsed
    }

    #[test]
    fn big_tree_is_fast_enough() {
        let e: Vec<Entry> = (0..60_000).map(|i| Entry::new(format!("d:/ymir work/d{}/s{}/f{i}.dds", i % 50, i % 7))).collect();
        let t = Tree::build(&e);
        let rows = visible_rows(&e, &t, &Filter::default(), &HashSet::new(), ViewMode::Folders);
        assert_eq!(rows.len(), 1);
    }
}
