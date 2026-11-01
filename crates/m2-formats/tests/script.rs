//! `m2_formats::script` on small inputs: the parsed tree and its accessors.

use m2_formats::script::{parse, parse_multiple_text_data};

const MOTION: &[u8] = b"ScriptType MotionData\r\n\
MotionFileName \"d:/ymir work/a b.gr2\"\r\n\
MotionDuration 1.5\r\n\
Accumulation 0 -10.5 0\r\n\
Group Event00\r\n{\r\n\tMotionEventType 3\r\n\tColor 1 0.5 0 1\r\n\
\tList Times\r\n\t{\r\n\t\t0.1 1\r\n\t\t0.2 2\r\n\t}\r\n}\r\n\
Group Event01\r\n{\r\n\tMotionEventType 300\r\n}\r\n";

#[test]
fn accessors() {
    let script = parse(MOTION);
    assert!(script.complete);
    let root = &script.root;
    assert_eq!(root.name, b"global");
    assert_eq!(root.token_string("scripttype"), Some(&b"MotionData"[..]));
    assert_eq!(root.token_string("motionfilename"), Some(&b"d:/ymir work/a b.gr2"[..]));
    assert_eq!(root.token_float("motionduration"), Some(1.5));
    assert_eq!(root.token_vector3("accumulation"), Some([0.0, -10.5, 0.0]));
    assert_eq!(root.token_vector4("accumulation"), None);
    // Lookups are not lowercased, like C++.
    assert!(!root.has_token("ScriptType"));

    assert_eq!(root.children.len(), 2);
    let event = root.child_indexed("event", 0).unwrap();
    assert_eq!(event.name, b"event00");
    assert_eq!(event.token_int("motioneventtype"), Some(3));
    assert_eq!(event.token_vector4("color"), Some([1.0, 0.5, 0.0, 1.0]));
    assert_eq!(event.tokens("times").unwrap().len(), 4);
    let second = root.child("event01").unwrap();
    assert_eq!(second.token_byte("motioneventtype"), Some(44)); // 300 as BYTE
    assert_eq!(second.token_word("motioneventtype"), Some(300));
    assert_eq!(second.token_bool("motioneventtype"), Some(true));
    assert!(root.child("Event01").is_none());
}

#[test]
fn first_key_wins_and_failures_keep_the_tree() {
    let script = parse(b"k 1\nK 2\nGroup G\n{\nx 1\n");
    assert!(!script.complete);
    assert_eq!(script.root.token_int("k"), Some(1));
    assert_eq!(script.root.child("g").unwrap().token_int("x"), Some(1));

    // A key without a value ends the group: the rest of the file is ignored.
    let script = parse(b"a 1\nlonely\nb 2\n");
    assert!(script.complete);
    assert!(script.root.has_token("a"));
    assert!(!script.root.has_token("b"));

    assert!(!parse(b"}\n").complete);
}

#[test]
fn multiple_text_data() {
    let map = parse_multiple_text_data(b"CellScale 200\nStart TextureSet\n\tA.TXT x\nEnd\ncellscale 300\n");
    assert_eq!(map[&b"cellscale"[..]], [b"200".to_vec()]);
    assert_eq!(map[&b"textureset"[..]], [b"a.txt".to_vec(), b"x".to_vec()]);
}
