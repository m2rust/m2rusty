//! `load_locale_data` over a folder pack: step order and early returns.

use std::fs;

use m2_formats::pack::{PackManager, load_index};
use m2_formats::proto::{Step, load_locale_data};

fn manager(files: &[(&str, &[u8])]) -> (tempfile::TempDir, PackManager) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("pack/loc/locale/xx");
    fs::create_dir_all(&dir).unwrap();
    for (name, data) in files {
        fs::write(dir.join(name), data).unwrap();
    }
    fs::write(tmp.path().join("pack/Index"), "loc\tFOLDER\n").unwrap();
    let mut mgr = PackManager::new(tmp.path());
    load_index(&mut mgr, "pack", "pack/Index").unwrap();
    (tmp, mgr)
}

fn steps(data: &m2_formats::proto::LocaleData) -> Vec<(Step, bool)> {
    data.steps.iter().map(|(s, r)| (*s, r.is_ok())).collect()
}

#[test]
fn missing_item_proto_stops() {
    let (_tmp, mgr) = manager(&[("item_list.txt", b"1\tx\ticon.tga\n")]);
    let data = load_locale_data(&mgr, "locale/xx", false);
    assert_eq!(steps(&data), [(Step::ItemList, true), (Step::ItemProto, false)]);
    assert!(!data.ok());
    // The item list stays loaded, as in the C++ managers.
    assert_eq!(data.items.items[&1].icon_file_name, b"icon.tga");
}

#[test]
fn bad_item_proto_is_reported() {
    let (_tmp, mgr) = manager(&[("item_proto", b"MIPX\x02\0\0\0")]);
    let data = load_locale_data(&mgr, "locale/xx", true);
    assert_eq!(steps(&data), [(Step::ItemList, false), (Step::ItemProto, false)]);
    assert!(matches!(data.steps[1].1, Err(m2_formats::proto::Error::Truncated)));
}

#[test]
fn file_names() {
    assert_eq!(Step::SkillDesc.file_name(), "SkillDesc.txt");
    assert_eq!(Step::ConvertTable(2).file_name(), "lang2.cvt");
    assert!(Step::MobProto.is_required());
    assert!(!Step::InsultList.is_required());
}
