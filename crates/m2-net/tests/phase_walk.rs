#![allow(clippy::field_reassign_with_default)]
//! A scripted in-process fake server walks the whole phase machine:
//! handshake, login, character select, loading and game.

use m2_net::phase::*;
use m2_net::stream::SEQUENCE_TABLE;
use m2_net::transport::mem::{self, MemTransport};
use m2_net::transport::{Transport, pump};
use m2_net::wire::*;

fn gc_phase(p: u8) -> Vec<u8> {
    TPacketGCPhase { header: gc::PHASE, phase: p }.encode()
}

#[test]
fn receive_burst_larger_than_buffer_preserves_every_packet() {
    let (mut client_transport, mut server) = mem::pair();
    let mut client = NetMachine::new(NetConfig { recv_capacity: 64, ..Default::default() });
    server.write(&gc_phase(phase_id::HANDSHAKE)).unwrap();
    assert!(!pump(&mut client, &mut client_transport, 0).1);
    let packets = vec![gc::PING; 512];
    server.write(&packets).unwrap();
    let (events, closed) = pump(&mut client, &mut client_transport, 1);
    assert!(!closed);
    assert_eq!(events.iter().filter(|event| matches!(event, NetEvent::Ping)).count(), packets.len());
}

#[test]
fn quest_info_uses_flag_fields_when_server_leaves_header_size_at_six() {
    let mut stream = m2_net::stream::NetStream::new(1024, 1024);
    let mut packet = vec![gc::QUEST_INFO, 6, 0, 106, 0, 0b0000_0011, 1];
    let mut title = [0u8; 31];
    title[..5].copy_from_slice(b"Title");
    packet.extend(title);
    packet.push(gc::PING);
    assert_eq!(stream.feed(&packet[..12]), 12);
    assert!(stream.poll_frame().unwrap().is_none());
    assert_eq!(stream.feed(&packet[12..]), packet.len() - 12);
    let quest = stream.poll_frame().unwrap().unwrap();
    assert_eq!(quest.bytes, packet[..packet.len() - 1]);
    assert_eq!(stream.poll_frame().unwrap().unwrap().header, gc::PING);
}

struct Rig {
    client: NetMachine,
    ct: MemTransport,
    server: MemTransport,
    now: u32,
}

impl Rig {
    fn new() -> Rig {
        let (mut ct, server) = mem::pair();
        ct.max_read = 3; // force partial reads everywhere
        Rig { client: NetMachine::new(NetConfig::default()), ct, server, now: 0 }
    }
    fn send(&mut self, bytes: &[u8]) -> Vec<NetEvent> {
        self.server.write(bytes).unwrap();
        let mut events = Vec::new();
        for _ in 0..bytes.len() {
            self.now += 1;
            events.extend(pump(&mut self.client, &mut self.ct, self.now).0);
        }
        events
    }
    /// Everything the client wrote since the last call.
    fn client_output(&mut self) -> Vec<u8> {
        pump(&mut self.client, &mut self.ct, self.now);
        let mut out = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = self.server.read(&mut buf).unwrap();
            if n == 0 {
                return out;
            }
            out.extend_from_slice(&buf[..n]);
        }
    }
}

fn player(id: u32, name: &str) -> TSimplePlayerInformation {
    let mut p = TSimplePlayerInformation::default();
    p.dwID = id;
    p.set_szName_str(name);
    p.byLevel = 12;
    p.lAddr = 0x0100_007f;
    p.wPort = 13000;
    p
}

#[test]
fn walks_handshake_login_select_loading_game() {
    let mut r = Rig::new();
    assert_eq!(r.client.phase(), Phase::Offline);
    r.client.set_login_info("user", "secret");
    r.client.set_sequence_mode(true);

    // ---- handshake
    let ev = r.send(&gc_phase(phase_id::HANDSHAKE));
    assert_eq!(ev, vec![NetEvent::PhaseChanged { from: Phase::Offline, to: Phase::HandShake }]);
    let hs = TPacketGCHandshake { header: gc::HANDSHAKE, dwHandshake: 7, dwTime: 1000, lDelta: 50 };
    let ev = r.send(&hs.encode());
    assert_eq!(ev, vec![NetEvent::HandshakeReceived { server_time: 1050 }]);
    // echo: same header (0xff), dwTime += 2 * delta, delta 0, and no sequence byte
    let echo = r.client_output();
    assert_eq!(echo, TPacketGCHandshake { header: 0xff, dwHandshake: 7, dwTime: 1100, lDelta: 0 }.encode());
    let ev = r.send(&TPacketGCPing { header: gc::PING }.encode());
    assert_eq!(ev, vec![NetEvent::Ping]);
    let mut pong = TPacketCGPong { bHeader: cg::PONG }.encode();
    pong.push(SEQUENCE_TABLE[0]);
    assert_eq!(r.client_output(), pong);

    // ---- login: the client sends its credentials when the phase starts
    let ev = r.send(&gc_phase(phase_id::LOGIN));
    assert_eq!(ev, vec![NetEvent::PhaseChanged { from: Phase::HandShake, to: Phase::Login }]);
    let out = r.client_output();
    let login = TPacketCGLogin::decode(&out).unwrap();
    assert_eq!((login.header, login.name_str(), login.pwd_str()), (cg::LOGIN, "user".into(), "secret".into()));
    assert_eq!(out.len(), TPacketCGLogin::SIZE + 1);
    assert_eq!(out[TPacketCGLogin::SIZE], SEQUENCE_TABLE[1]);

    let mut fail = TPacketGCLoginFailure::default();
    fail.header = gc::LOGIN_FAILURE;
    fail.set_szStatus_str("WRONGPWD");
    assert_eq!(r.send(&fail.encode()), vec![NetEvent::LoginFailure { status: "WRONGPWD".into() }]);
    assert_eq!(r.send(&TPacketGCLoginKey { bHeader: gc::LOGIN_KEY, dwLoginKey: 0xabcd }.encode()), vec![NetEvent::LoginKey(0xabcd)]);
    assert_eq!(r.send(&TPacketGCEmpire { bHeader: gc::EMPIRE, bEmpire: 2 }.encode()), vec![NetEvent::Empire(2)]);

    // A header the Login phase does not handle: reported, receive buffer dropped.
    let mut bytes = TPacketGCCreateFailure { header: gc::PLAYER_CREATE_FAILURE, bType: 1 }.encode();
    bytes.extend(TPacketGCPing { header: gc::PING }.encode()); // dropped with it
    let ev = r.send(&bytes);
    assert_eq!(ev, vec![NetEvent::Unhandled { phase: Phase::Login, header: gc::PLAYER_CREATE_FAILURE }]);

    // ---- select
    let mut ok = TPacketGCLoginSuccess4::default();
    ok.header = gc::LOGIN_SUCCESS4;
    ok.akSimplePlayerInformation[1] = player(77, "Hero");
    ok.guild_id[1] = 5;
    ok.guild_name[1][..4].copy_from_slice(b"Pals");
    ok.handle = 11;
    ok.random_key = 22;
    let ev = r.send(&ok.encode());
    assert!(matches!(&ev[..], [NetEvent::LoginSuccess { players, mark_handle: 11, mark_random_key: 22 }] if players[1].dwID == 77));
    assert_eq!(r.client.guild_names()[1], "Pals");
    r.send(&gc_phase(phase_id::SELECT));
    assert_eq!(r.client.phase(), Phase::Select);

    // a state-changing reply: create success fills the slot
    let ev = r.send(&TPacketGCPlayerCreateSuccess { header: gc::PLAYER_CREATE_SUCCESS, bAccountCharacterSlot: 2, kSimplePlayerInfomation: player(88, "New") }.encode());
    assert!(matches!(&ev[..], [NetEvent::PlayerCreateSuccess { slot: 2, .. }]));
    assert_eq!(r.client.players()[2].szName_str_for_test(), "New");
    let ev = r.send(&TPacketGCPlayerCreateSuccess { header: gc::PLAYER_CREATE_SUCCESS, bAccountCharacterSlot: 9, kSimplePlayerInfomation: player(1, "x") }.encode());
    assert_eq!(ev, vec![NetEvent::PlayerCreateBadSlot { slot: 9 }]);
    // delete success is read as header + account_index (2 bytes) though the map says 1
    let ev = r.send(&[gc::PLAYER_DELETE_SUCCESS, 2]);
    assert_eq!(ev, vec![NetEvent::PlayerDeleteSuccess { slot: 2 }]);
    assert_eq!(r.client.players()[2].dwID, 0);
    // rename by pid
    let mut cn = TPacketGCChangeName::default();
    cn.header = gc::CHANGE_NAME;
    cn.pid = 77;
    cn.set_name_str("Hero2");
    assert_eq!(r.send(&cn.encode()), vec![NetEvent::ChangeName { slot: 1, name: "Hero2".into() }]);
    cn.pid = 4242;
    assert_eq!(r.send(&cn.encode()), vec![NetEvent::PlayerCreateFailure { kind: 100 }]);

    assert!(r.client.send_select_character(1));
    let out = r.client_output();
    assert_eq!(&out[..2], &[cg::PLAYER_SELECT, 1]);

    // ---- loading
    r.send(&gc_phase(phase_id::LOADING));
    assert_eq!(r.client.phase(), Phase::Loading);
    let mut mc = TPacketGCMainCharacter4_BGM_VOL::default();
    mc.header = gc::MAIN_CHARACTER4_BGM_VOL;
    mc.dwVID = 9001;
    mc.wRaceNum = 3;
    mc.set_szUserName_str("Hero2");
    mc.set_szBGMName_str("bgm/field.mp3");
    mc.fBGMVol = 0.5;
    mc.lX = 469300;
    mc.lY = 964200;
    mc.byEmpire = 2;
    mc.bySkillGroup = 1;
    let ev = r.send(&mc.encode());
    assert_eq!(
        ev,
        vec![NetEvent::MainCharacter(MainCharacter {
            vid: 9001,
            race: 3,
            empire: 2,
            skill_group: 1,
            name: "Hero2".into(),
            x: 469300,
            y: 964200,
            bgm: Some(("bgm/field.mp3".into(), Some(0.5)))
        })]
    );
    // loading sends CG_CLIENT_VERSION
    let out = r.client_output();
    assert_eq!(out[0], cg::CLIENT_VERSION);
    assert_eq!(out.len(), TPacketCGClientVersion::SIZE + 1);

    let mut pts = TPacketGCPoints::default();
    pts.header = gc::PLAYER_POINTS;
    pts.points[1] = 15;
    let ev = r.send(&pts.encode());
    assert!(matches!(&ev[..], [NetEvent::Packet(GcPacket::PlayerPoints(p))] if p.points[1] == 15));
    let mut item = TPacketGCItemSet::default();
    item.header = gc::ITEM_SET;
    item.vnum = 19;
    item.Cell.cell = 4;
    let ev = r.send(&item.encode());
    assert!(matches!(&ev[..], [NetEvent::Packet(GcPacket::ItemSet(p))] if p.vnum == 19 && p.Cell.cell == 4));
    // Loading falls through to the game dispatch: shop sign is a game-only header.
    let mut sign = TPacketGCShopSign::default();
    sign.bHeader = gc::SHOP_SIGN;
    sign.dwVID = 5;
    assert_eq!(r.send(&sign.encode()).len(), 1);

    // ---- game
    r.send(&gc_phase(phase_id::GAME));
    assert_eq!(r.client.phase(), Phase::Game);

    // a monster is appended at once; a player waits for the additional info
    let mut add = TPacketGCCharacterAdd::default();
    add.header = gc::CHARACTER_ADD;
    add.dwVID = 100;
    add.bType = 0;
    add.wRaceNum = 101;
    add.x = 10;
    add.y = 20;
    add.dwAffectFlag = [1, 2];
    let ev = r.send(&add.encode());
    assert!(matches!(&ev[..], [NetEvent::ActorAppend(a)] if a.vid == 100 && a.name.is_none() && a.affect_flags == (2u64 << 32 | 1)));
    add.dwVID = 200;
    add.bType = ACTOR_TYPE_PC;
    add.wRaceNum = 2;
    assert!(r.send(&add.encode()).is_empty());
    let mut info = TPacketGCCharacterAdditionalInfo::default();
    info.header = gc::CHAR_ADDITIONAL_INFO;
    info.dwVID = 200;
    info.set_name_str("Bob");
    info.awPart = [11, 22, 33, 44];
    info.dwLevel = 7;
    let ev = r.send(&info.encode());
    match &ev[..] {
        [NetEvent::ActorAppend(a)] => {
            assert_eq!((a.vid, a.name.as_deref(), a.armor, a.weapon, a.hair, a.level), (200, Some("Bob"), 11, 22, 44, 7));
        }
        other => panic!("{other:?}"),
    }
    info.dwVID = 999;
    assert_eq!(r.send(&info.encode()), vec![NetEvent::ActorInfoMismatch { vid: 999 }]);

    let mv = TPacketGCMove { bHeader: gc::CHARACTER_MOVE, bFunc: 1, bArg: 2, bRot: 18, dwVID: 200, lX: 5, lY: 6, dwTime: 7, dwDuration: 8 };
    assert_eq!(
        r.send(&mv.encode()),
        vec![NetEvent::ActorMove(ActorMove { vid: 200, func: 1, arg: 2, rotation: 90.0, x: 5, y: 6, time: 7, duration: 8 })]
    );
    assert_eq!(r.send(&TPacketGCCharacterDelete { header: gc::CHARACTER_DEL, dwVID: 200 }.encode()), vec![NetEvent::ActorRemove { vid: 200 }]);

    // dynamic: chat, sync position
    let text = b"hello world\0";
    let head = TPacketGCChat { header: gc::CHAT, size: (TPacketGCChat::SIZE + text.len()) as u16, r#type: 0, dwVID: 100, bEmpire: 1 };
    let mut chat = head.encode();
    chat.extend_from_slice(text);
    assert_eq!(r.send(&chat), vec![NetEvent::Chat { kind: 0, vid: 100, empire: 1, text: "hello world".into() }]);

    let mut sync = TPacketGCSyncPosition { bHeader: gc::SYNC_POSITION, wSize: (TPacketGCSyncPosition::SIZE + 2 * TPacketGCSyncPositionElement::SIZE) as u16 }.encode();
    sync.extend(TPacketGCSyncPositionElement { dwVID: 1, lX: 10, lY: 11 }.encode());
    sync.extend(TPacketGCSyncPositionElement { dwVID: 2, lX: 20, lY: 21 }.encode());
    assert_eq!(r.send(&sync), vec![NetEvent::ActorSync { vid: 1, x: 10, y: 11 }, NetEvent::ActorSync { vid: 2, x: 20, y: 21 }]);

    // whisper is mapped static but carries a text tail
    let mut w = TPacketGCWhisper::default();
    w.bHeader = gc::WHISPER;
    w.set_szNameFrom_str("Alice");
    w.wSize = (TPacketGCWhisper::SIZE + 3) as u16;
    let mut wb = w.encode();
    wb.extend_from_slice(b"hi\0");
    assert_eq!(r.send(&wb), vec![NetEvent::Whisper { kind: 0, from: "Alice".into(), text: "hi".into() }]);

    // game-phase handshake: TIME_SYNC reply, then HANDSHAKE_OK turns it into a server time
    let _ = r.client_output();
    let ev = r.send(&TPacketGCHandshake { header: gc::HANDSHAKE, dwHandshake: 1, dwTime: 5000, lDelta: 10 }.encode());
    assert_eq!(ev, vec![NetEvent::HandshakeReceived { server_time: 5010 }]);
    let out = r.client_output();
    assert_eq!(out[0], cg::TIME_SYNC);
    assert!(matches!(&r.send(&[gc::HANDSHAKE_OK])[..], [NetEvent::ServerTime { .. }]));

    // main character packets are loading-only
    let ev = r.send(&mc.encode());
    assert_eq!(ev, vec![NetEvent::Unhandled { phase: Phase::Game, header: gc::MAIN_CHARACTER4_BGM_VOL }]);
    // warp: reconnect request, direct-enter mode
    // A level change is a point packet, even when its payload contains byte 65
    // (the warp header); it must never be mistaken for a transfer.
    let level = TPacketGCPointChange { header: gc::PLAYER_POINT_CHANGE, dwVID: 100, Type: 1, amount: 65, value: 2 };
    assert_eq!(r.send(&level.encode()), vec![NetEvent::Packet(GcPacket::PlayerPointChange(level))]);
    let ev = r.send(&TPacketGCWarp { bHeader: gc::WARP, lX: 1, lY: 2, lAddr: 0x0100_007f, wPort: 13001 }.encode());
    assert_eq!(ev, vec![NetEvent::Warp { x: 1, y: 2, addr: 0x0100_007f, port: 13001 }]);
    assert_eq!(r.client.direct_enter_slot(), Some(1));

    // chat out: header, length, type, NUL-terminated text, sequence byte
    assert!(r.client.send_chat("hi", 0));
    let out = r.client_output();
    assert_eq!(out.len(), TPacketCGChat::SIZE + 3 + 1);
    assert_eq!(u16::from_le_bytes([out[1], out[2]]) as usize, TPacketCGChat::SIZE + 3);

    // Match the original client's buffer recovery after an unknown header.
    let ev = r.send(&[35, 0, 0]);
    assert_eq!(ev, vec![NetEvent::Frame(m2_net::stream::FrameError::UnknownHeader(35))]);
    assert_eq!(r.send(&[gc::PING]), vec![NetEvent::Ping]);
    r.server.close();
    assert!(pump(&mut r.client, &mut r.ct, 0).1);
    assert_eq!(r.client.phase(), Phase::Offline);
}

#[allow(non_snake_case)]
trait NameForTest {
    fn szName_str_for_test(&self) -> String;
}
impl NameForTest for TSimplePlayerInformation {
    fn szName_str_for_test(&self) -> String {
        cstr_to_string(&self.szName)
    }
}

#[test]
fn login2_when_a_login_key_is_set() {
    let mut r = Rig::new();
    r.client = NetMachine::new(NetConfig { client_key: [1, 2, 3, 4], ..NetConfig::default() });
    r.client.set_login_info("acc", "ignored");
    r.client.set_login_key(0x1234);
    r.send(&gc_phase(phase_id::HANDSHAKE));
    r.send(&gc_phase(phase_id::LOGIN));
    let p = TPacketCGLogin2::decode(&r.client_output()).unwrap();
    assert_eq!((p.header, p.login_key, p.adwClientKey, p.name_str()), (cg::LOGIN2, 0x1234, [1, 2, 3, 4], "acc".into()));
}

#[test]
fn every_server_header_is_accounted_for() {
    // Headers in the map that no phase handles; the integrator reads this list.
    let list = unhandled_headers();
    eprintln!("unhandled by every phase ({}): {}", list.len(), list.join(", "));
    for h in m2_net::wire::GC_HEADER_MAP {
        let any = [Phase::Offline, Phase::HandShake, Phase::Login, Phase::Select, Phase::Loading, Phase::Game].iter().any(|p| accepts(*p, h.header));
        assert_eq!(!any, list.contains(&h.name));
    }
}

#[cfg(feature = "tcp")]
#[test]
fn works_over_a_real_tcp_socket() {
    use m2_net::transport::tcp::TcpTransport;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.write_all(&gc_phase(phase_id::HANDSHAKE)).unwrap();
        s.write_all(&TPacketGCHandshake { header: gc::HANDSHAKE, dwHandshake: 1, dwTime: 10, lDelta: 1 }.encode()).unwrap();
        let mut echo = vec![0u8; TPacketGCHandshake::SIZE];
        s.read_exact(&mut echo).unwrap();
        echo
    });
    let mut t = TcpTransport::connect(addr, Duration::from_secs(3)).unwrap();
    let mut m = NetMachine::new(NetConfig::default());
    let mut seen = Vec::new();
    let t0 = Instant::now();
    while !seen.iter().any(|e| matches!(e, NetEvent::HandshakeReceived { .. })) {
        seen.extend(pump(&mut m, &mut t, 0).0);
        assert!(t0.elapsed() < Duration::from_secs(5), "timeout: {seen:?}");
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..50 {
        pump(&mut m, &mut t, 0);
        std::thread::sleep(Duration::from_millis(1));
    }
    let echo = server.join().unwrap();
    assert_eq!(TPacketGCHandshake::decode(&echo).unwrap().dwTime, 12);
}
