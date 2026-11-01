use m2_net::wire::*;

#[test]
fn little_endian_layout_of_a_known_packet() {
    let p = TPacketGCMove { bHeader: gc::CHARACTER_MOVE, bFunc: 1, bArg: 2, bRot: 3, dwVID: 0x0403_0201, lX: -2, lY: 5, dwTime: 7, dwDuration: 8 };
    let b = p.encode();
    assert_eq!(b.len(), 24);
    assert_eq!(&b[..8], &[3, 1, 2, 3, 1, 2, 3, 4]);
    assert_eq!(&b[8..12], &(-2i32).to_le_bytes());
    assert_eq!(TPacketGCMove::decode(&b).unwrap(), p);
}

#[test]
fn truncated_input_is_an_error_not_a_panic() {
    let b = TPacketGCMove::default().encode();
    for n in 0..b.len() {
        assert_eq!(TPacketGCMove::decode(&b[..n]), Err(DecodeError::Truncated));
    }
    assert_eq!(GcPacket::decode(&[]), Err(DecodeError::Truncated));
    assert_eq!(GcPacket::decode(&[0xee]), Err(DecodeError::UnknownHeader(0xee)));
}

#[test]
fn every_layout_round_trips_patterned_bytes() {
    // decode(any bytes) then encode gives the same bytes back, for every struct reachable from the map.
    for h in GC_HEADER_MAP {
        let mut f: Vec<u8> = (0..h.size.max(3) + 5).map(|i| (i * 31 + h.header as usize) as u8).collect();
        f[0] = h.header;
        if h.dynamic {
            let l = f.len() as u16;
            f[1..3].copy_from_slice(&l.to_le_bytes());
        }
        let p = GcPacket::decode(&f).unwrap();
        let again = p.encode();
        assert_eq!(GcPacket::decode(&again).unwrap(), p, "{}", h.name);
        if h.name == "ITEM_SET2" {
            continue; // the C++ bool field normalises 0x24 to 1
        }
        if h.dynamic {
            assert_eq!(again, f, "{}", h.name);
        } else {
            assert_eq!(again[..], f[..h.size], "{}", h.name);
        }
    }
}

#[test]
fn strings_are_utf8_views_of_fixed_fields() {
    let mut p = TPacketCGLogin::default();
    p.set_name_str("héllo");
    assert_eq!(p.name_str(), "héllo");
    // truncation never splits a character and always leaves a NUL
    p.set_name_str(&"é".repeat(40));
    assert_eq!(p.name.len(), 31);
    assert_eq!(p.name[30], 0);
    assert_eq!(p.name_str().chars().count(), 15);
    // invalid bytes are replaced, not an error
    p.name[0] = 0xff;
    assert!(p.name_str().starts_with('\u{fffd}'));
    // an unterminated field uses the whole array
    p.name = [b'a'; 31];
    assert_eq!(p.name_str().len(), 31);
}

#[test]
fn tails() {
    assert_eq!(tails::text(b"abc\0def"), "abc");
    assert_eq!(tails::duel_vids(&[1, 0, 0, 0, 2, 0, 0, 0, 9]), vec![1, 2]);
    assert!(tails::sync_positions(&[0; 11]).unwrap().is_empty());
}
