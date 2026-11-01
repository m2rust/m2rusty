//! Reader for the API snapshot files (`module<TAB>name`, then `  name<TAB>kind[<TAB>value]`).

use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// "callable", "int", "float", "str" ...
    pub kind: String,
    pub value: String,
}

pub type Snapshot = BTreeMap<String, BTreeMap<String, Item>>;

pub fn parse_into(text: &str, out: &mut Snapshot) {
    let mut cur: Option<String> = None;
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        if let Some(m) = line.strip_prefix("module\t") {
            cur = Some(m.trim().to_string());
            out.entry(m.trim().to_string()).or_default();
        } else if let Some(c) = &cur {
            let mut it = line.trim_start().split('\t');
            let name = it.next().unwrap_or("").to_string();
            let kind = it.next().unwrap_or("").to_string();
            let value = it.next().unwrap_or("").to_string();
            out.get_mut(c).unwrap().insert(name, Item { kind, value });
        }
    }
}

pub fn load(repo: &Path) -> std::io::Result<Snapshot> {
    let files = [
        "client/src/ui/tests/data/python_api_client.txt",
        "neoeter-python/src/bindings/tests/data/python_api_eterpythonlib.txt",
        "neoeter-python/src/host/tests/data/python_api_scriptlib.txt",
    ];
    let mut s = Snapshot::new();
    for f in files {
        parse_into(&std::fs::read_to_string(repo.join(f))?, &mut s);
    }
    Ok(s)
}
