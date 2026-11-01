//! Port of `neoeter/src/io/tests/test_pack_formats.cpp` and `test_eterpack.cpp`.

mod support;

use std::fs;

use m2_formats::pack::{Container, Error, PackManager, Provider, SearchMode, ZipPack, load_index};
use support::{deterministic_bytes, write_raw_name_zip, write_zip};

#[test]
fn zip_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("sample.zip");
    let binary = deterministic_bytes(64 * 1024, 0x1234_5678);
    let with_nul = b"a\0b\0\0c".to_vec();

    write_zip(
        &archive,
        &[
            ("ymir work/ui/Window.DDS", b"DDS \x01\x02\x03\x04"),
            ("locale/en/locale_game.txt", b"hello"),
            ("data/empty.bin", b""),
            ("data/binary.bin", &binary),
            ("data/nul.bin", &with_nul),
            ("data/directory/", b""),
        ],
    );

    let zip = ZipPack::open(&archive).unwrap();
    assert_eq!(zip.file_names().len(), 5, "directory entry skipped");

    assert!(zip.contains(b"d:/ymir work/ui/window.dds"));
    assert!(zip.contains(b"locale/en/locale_game.txt"));
    assert!(!zip.contains(b"data/missing.bin"));

    assert_eq!(zip.read(b"data/binary.bin").unwrap(), binary);
    assert_eq!(zip.read(b"data/empty.bin").unwrap(), b"");
    assert_eq!(zip.read(b"data/nul.bin").unwrap(), with_nul);
    assert!(matches!(zip.read(b"data/missing.bin"), Err(Error::NotFound(_))));
}

#[test]
fn zip_names_follow_libzip_encoding_guess() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("names.zip");
    write_raw_name_zip(
        &archive,
        &[
            // CP949 for 용마, not valid UTF-8: decoded as CP437.
            (b"\xBF\xEB\xB8\xB6.txt", b"cp949"),
            // Valid UTF-8 without the flag: kept as is.
            ("caf\u{e9}.txt".as_bytes(), b"utf8"),
            (b"Plain.TXT", b"ascii"),
        ],
    );

    let zip = ZipPack::open(&archive).unwrap();
    let cp437 = "\u{2510}\u{3b4}\u{2555}\u{2562}.txt".as_bytes();
    assert_eq!(zip.read(cp437).unwrap(), b"cp949");
    assert_eq!(zip.read("caf\u{e9}.txt".as_bytes()).unwrap(), b"utf8");
    assert_eq!(zip.read(b"plain.txt").unwrap(), b"ascii");
}

#[test]
fn manager_overlay_and_search_mode() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::create_dir_all(root.join("folder/ymir work/ui")).unwrap();
    fs::write(root.join("folder/ymir work/ui/Window.dds"), "FOLDER").unwrap();
    write_zip(
        &root.join("overlay.zip"),
        &[("ymir work/ui/Window.dds", b"ZIP"), ("ymir work/ui/OnlyZip.dds", b"Z")],
    );
    // A loose file at the virtual path, for file-first lookups.
    fs::create_dir_all(root.join("d:/ymir work/ui")).unwrap();
    fs::write(root.join("d:/ymir work/ui/Window.dds"), "DISK").unwrap();

    let mut manager = PackManager::new(root);
    manager.register("folder", Container::Folder).unwrap();
    manager.register("overlay", Container::Zip).unwrap();

    manager.set_search_mode(SearchMode::FileFirst);
    assert_eq!(manager.get("d:/ymir work/ui/Window.dds").unwrap(), b"DISK");

    manager.set_search_mode(SearchMode::PackFirst);
    assert_eq!(manager.get("d:/ymir work/ui/Window.dds").unwrap(), b"ZIP");

    assert_eq!(manager.get("D:\\YMIR WORK\\UI\\ONLYZIP.DDS").unwrap(), b"Z");
    assert!(manager.exists_in_pack("d:/YMIR WORK/ui/onlyzip.dds"));
    assert!(!manager.exists("d:/ymir work/ui/missing.dds"));

    // Registering again is a no-op, and does not reorder the overlay.
    manager.register("folder", Container::Folder).unwrap();
    assert_eq!(manager.get_from_pack("d:/ymir work/ui/window.dds").unwrap(), b"ZIP");
}

#[test]
fn case_split_folders_merge_exact_spelling_last() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("pack");
    fs::create_dir_all(pack.join("ETC")).unwrap();
    fs::create_dir_all(pack.join("etc")).unwrap();
    fs::write(pack.join("ETC/shared.txt"), "upper").unwrap();
    fs::write(pack.join("etc/shared.txt"), "lower").unwrap();
    fs::write(pack.join("etc/only_lower.txt"), "x").unwrap();

    let mut manager = PackManager::new(dir.path());
    manager.register("pack/etc", Container::Folder).unwrap();
    assert_eq!(manager.get_from_pack("shared.txt").unwrap(), b"lower");
    assert!(manager.exists_in_pack("only_lower.txt"));

    let mut manager = PackManager::new(dir.path());
    manager.register("pack/ETC", Container::Folder).unwrap();
    assert_eq!(manager.get_from_pack("shared.txt").unwrap(), b"upper");

    // Neither spelling on disk: the lookup is case-insensitive.
    let mut manager = PackManager::new(dir.path());
    manager.register("pack/Etc", Container::Folder).unwrap();
    assert!(manager.exists_in_pack("only_lower.txt"));
}

#[test]
fn malformed_archives_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(root.join("garbage.zip"), "this is definitely not a zip archive").unwrap();
    assert!(ZipPack::open(root.join("garbage.zip")).is_err());

    fs::write(root.join("empty.zip"), "").unwrap();
    assert!(ZipPack::open(root.join("empty.zip")).is_err());

    write_zip(&root.join("source.zip"), &[("a.txt", b"a"), ("b.txt", b"b")]);
    let bytes = fs::read(root.join("source.zip")).unwrap();
    fs::write(root.join("truncated.zip"), &bytes[..bytes.len() / 2]).unwrap();
    assert!(ZipPack::open(root.join("truncated.zip")).is_err());

    let mut manager = PackManager::new(root);
    assert!(manager.register("garbage", Container::Zip).is_err());
    assert!(manager.register("missing", Container::Folder).is_err());
    assert_eq!(manager.file_names().count(), 0);
}

#[test]
fn index_registers_packs_and_texcache() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("pack/Root")).unwrap();
    fs::create_dir_all(root.join("pack/root_texcache")).unwrap();
    fs::write(root.join("pack/Root/a.py"), "a").unwrap();
    fs::write(root.join("pack/root_texcache/a.dds"), "t").unwrap();
    write_zip(&root.join("pack/locale.zip"), &[("locale/x.txt", b"x")]);
    fs::write(root.join("pack/Index"), "root\tFOLDER\nlocale\tZIP\nmissing\tZIP\n").unwrap();

    let mut manager = PackManager::new(root);
    let report = load_index(&mut manager, "pack", "pack/Index").unwrap();
    assert_eq!(report.registered, ["pack/root", "pack/root_texcache", "pack/locale"]);
    assert_eq!(report.failed.len(), 1);
    assert_eq!(report.failed[0].0, "pack/missing");

    assert_eq!(manager.get_from_pack("a.py").unwrap(), b"a");
    assert_eq!(manager.get_from_pack("a.dds").unwrap(), b"t");
    assert_eq!(manager.get_from_pack("locale/x.txt").unwrap(), b"x");

    // An unterminated last line is read (C++ would drop it).
    fs::write(root.join("pack/Index2"), "locale\tZIP").unwrap();
    let mut manager = PackManager::new(root);
    let report = load_index(&mut manager, "pack", "pack/Index2").unwrap();
    assert_eq!(report.registered, ["pack/locale"]);
}

#[test]
fn malformed_index_lines_stop_loading() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("pack/a")).unwrap();

    fs::write(root.join("pack/Index"), "a\tFOLDER\nbroken\n").unwrap();
    let mut manager = PackManager::new(root);
    assert!(matches!(
        load_index(&mut manager, "pack", "pack/Index"),
        Err(Error::IndexLine { line: 1, tokens: 1 })
    ));

    fs::write(root.join("pack/Index"), "a\tFOLDER\n\n\nb\tFOLDER\n").unwrap();
    assert!(matches!(
        load_index(&mut PackManager::new(root), "pack", "pack/Index"),
        Err(Error::IndexLine { line: 1, tokens: 0 })
    ));

    fs::write(root.join("pack/Index"), "a\tTAR\n").unwrap();
    assert!(matches!(
        load_index(&mut PackManager::new(root), "pack", "pack/Index"),
        Err(Error::UnknownContainer(_))
    ));
}
