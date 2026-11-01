//! The phase machine: a pure port of `CPythonNetworkStream`'s phase handlers
//! (`client/src/ui/PythonNetworkStream*.cpp`) with the Python/UI calls replaced by
//! [`NetEvent`]s.
//!
//! ```text
//!  Offline --PHASE_HANDSHAKE--> HandShake --PHASE_LOGIN--> Login --PHASE_SELECT--> Select
//!                                                                      |
//!                    Game <--PHASE_GAME-- Loading <--PHASE_LOADING-----+
//! ```
//!
//! The server drives every transition with a `HEADER_GC_PHASE` packet; the client only
//! reacts. Each phase accepts a fixed set of headers (the `switch` of the C++ phase
//! function); anything else is `RecvErrorPacket`/`RecvDefaultPacket`: report and
//! `ClearRecvBuffer()` ([`NetEvent::Unhandled`]). The Loading phase falls through to the
//! Game dispatch for every header it does not handle itself.
//!
//! Outgoing bytes (handshake echo, pong, login, client version, ...) are queued in
//! the embedded [`NetStream`]; the driver writes [`NetMachine::take_output`] to the
//! transport.
//!
//! Not ported: the `MAX_RECV_COUNT` pacing of `GamePhase` (a frame-time budget; the
//! driver decides how often to call [`NetMachine::poll`]), the periodic Refresh*Window
//! UI calls, `BuildProcessCRC`/`__DownloadMark` on entering Select (UI/side services;
//! observe [`NetEvent::PhaseChanged`]), and the guild-mark / account-connector side
//! connections.

use crate::stream::{FrameError, NetStream};
use crate::wire::{self, GcPacket, TPacketCGCreateCharacter, TPacketCGLogin, TPacketCGLogin2, Wire, cg, consts, gc};
use crate::wire::{TPacketCGClientVersion, TPacketCGClientVersion2, TPacketCGDestroyCharacter, TPacketCGDirectEnter};
use crate::wire::{TPacketCGEmpire, TPacketCGEnterFrontGame, TPacketCGMove, TPacketCGPong, TPacketCGSelectCharacter};
use crate::wire::{TPacketCGAttack, TPacketGCHandshake, TSimplePlayerInformation};

/// `PHASE_*` values of `EPhase` (Packet.h) that the client switches on.
pub mod phase_id {
    pub const CLOSE: u8 = 0;
    pub const HANDSHAKE: u8 = 1;
    pub const LOGIN: u8 = 2;
    pub const SELECT: u8 = 3;
    pub const LOADING: u8 = 4;
    pub const GAME: u8 = 5;
    pub const DEAD: u8 = 6;
    /// The account server's login phase (`CAccountConnector`, [`crate::auth`]).
    pub const AUTH: u8 = 10;
}

/// `CActorInstance::EType` values the actor handlers compare with.
pub const ACTOR_TYPE_NPC: u8 = 1;
pub const ACTOR_TYPE_PC: u8 = 6;

/// `CHR_EQUIPPART_*` indices into `awPart`.
const PART_ARMOR: usize = 0;
const PART_WEAPON: usize = 1;
const PART_HAIR: usize = 3;

/// `m_strPhase`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Offline,
    HandShake,
    Login,
    Select,
    Loading,
    Game,
}

/// `SNetworkActorData` (NetworkActorManager.h) as filled by the append handlers.
/// Positions are the server's global coordinates (the C++ converts to map-local with
/// `__GlobalPositionToLocalPosition` using the background; that is the game side's job).
#[derive(Clone, Debug, PartialEq)]
pub struct ActorData {
    pub vid: u32,
    pub kind: u8,
    pub race: u16,
    pub move_speed: u8,
    pub attack_speed: u8,
    pub state_flags: u8,
    pub rotation: f32,
    pub x: i32,
    pub y: i32,
    /// `m_kAffectFlags` bits 0..32 and 32..64.
    pub affect_flags: u64,
    /// `None` for monsters: the game looks the name up by race (`CPythonNonPlayer::GetName`).
    pub name: Option<String>,
    pub guild_id: u32,
    pub level: u32,
    pub alignment: i16,
    pub pk_mode: u8,
    pub empire: u8,
    pub armor: u16,
    pub weapon: u16,
    pub hair: u16,
    pub mount_vnum: u32,
}

/// `SNetworkUpdateActorData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorUpdate {
    pub vid: u32,
    pub guild_id: u32,
    pub move_speed: u8,
    pub attack_speed: u8,
    pub state_flags: u8,
    pub affect_flags: u64,
    pub alignment: i16,
    pub pk_mode: u8,
    pub armor: u16,
    pub weapon: u16,
    pub hair: u16,
    pub mount_vnum: u32,
}

/// `SNetworkMoveActorData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorMove {
    pub vid: u32,
    pub func: u8,
    pub arg: u8,
    /// `bRot * 5.0f` degrees.
    pub rotation: f32,
    pub x: i32,
    pub y: i32,
    pub time: u32,
    pub duration: u32,
}

/// What `RecvMainCharacter*` stores and hands to the loading window.
#[derive(Clone, Debug, PartialEq)]
pub struct MainCharacter {
    pub vid: u32,
    pub race: u16,
    pub empire: u8,
    pub skill_group: u8,
    pub name: String,
    pub x: i32,
    pub y: i32,
    /// Field music (`__SetFieldMusicFileName` / `FileInfo`): file and optional volume.
    pub bgm: Option<(String, Option<f32>)>,
}

/// High-level result of processing one packet.
#[derive(Clone, Debug, PartialEq)]
pub enum NetEvent {
    /// A phase packet changed the phase (`SetXxxPhase`).
    PhaseChanged { from: Phase, to: Phase },
    /// `PHASE_CLOSE`: `ClosePhase()` asks the UI to show the login window.
    PhaseClose,
    /// `HEADER_GC_HANDSHAKE` in the HandShake phase; `server_time` is `dwTime + lDelta`
    /// (`ELTimer_SetServerMSec`). The echo is already queued.
    HandshakeReceived { server_time: u32 },
    /// `HEADER_GC_HANDSHAKE_OK` after a time sync: server time at `now_ms`.
    ServerTime { server_ms: u32 },
    /// `RecvPingPacket`; the pong is already queued.
    Ping,
    LoginSuccess { players: Vec<TSimplePlayerInformation>, mark_handle: u32, mark_random_key: u32 },
    /// `szStatus` (e.g. "WRONGPWD", "ALREADY").
    LoginFailure { status: String },
    Empire(u8),
    LoginKey(u32),
    MatrixCard { rows: [u8; 4], cols: [u8; 4] },
    RunupMatrixQuiz(String),
    PasspodRequest,
    PasspodFailure,
    PlayerCreateSuccess { slot: u8, info: TSimplePlayerInformation },
    /// `bAccountCharacterSlot >= PLAYER_PER_ACCOUNT4` (C++: TraceError, ignored).
    PlayerCreateBadSlot { slot: u8 },
    /// `bType` (and 100 for a rename that matched no character).
    PlayerCreateFailure { kind: u8 },
    PlayerDeleteSuccess { slot: u8 },
    PlayerDeleteFailure,
    ChangeName { slot: usize, name: String },
    MainCharacter(MainCharacter),
    ActorAppend(ActorData),
    ActorUpdate(ActorUpdate),
    ActorMove(ActorMove),
    ActorRemove { vid: u32 },
    ActorSync { vid: u32, x: i32, y: i32 },
    ActorOwner { owner_vid: u32, victim_vid: u32 },
    /// `HEADER_GC_PHASE`-independent additional info arrived for a different actor than
    /// the pending `HEADER_GC_CHARACTER_ADD` (C++: TraceError).
    ActorInfoMismatch { vid: u32 },
    Chat { kind: u8, vid: u32, empire: u8, text: String },
    Whisper { kind: u8, from: String, text: String },
    /// `HEADER_GC_WARP`: reconnect to another game server (`lAddr`/`wPort`); direct-enter
    /// mode is set for the selected slot.
    Warp { x: i32, y: i32, addr: u32, port: u16 },
    /// `RecvSafeBoxSetPacket` / `RecvMallItemSetPacket` (read as `TPacketGCItemSet2`).
    SafeboxSet(wire::TPacketGCItemSet2),
    MallSet(wire::TPacketGCItemSet2),
    /// Any other packet the phase accepts, decoded (items, points, quickslots, shop,
    /// party, guild, quests, ...). The game-side handlers consume these.
    Packet(GcPacket),
    /// The phase does not handle this header (`RecvErrorPacket`): the receive buffer
    /// was cleared.
    Unhandled { phase: Phase, header: u8 },
    /// Framing failed (unknown header, bad dynamic size); receive buffer cleared.
    Frame(FrameError),
    /// A frame of a handled header could not be decoded (cannot happen for frames
    /// produced by `NetStream`, whose sizes come from the same map).
    Malformed { header: u8 },
}

/// `ClientVersion` strings of `SendClientVersionPacket` (locale dependent in C++).
#[derive(Clone, Debug)]
pub struct ClientVersion {
    /// `LocaleService_IsEUROPE() && !IsYMIR()` sends `TPacketCGClientVersion2`.
    pub v2: bool,
    pub filename: String,
    pub timestamp: String,
}

impl Default for ClientVersion {
    fn default() -> Self {
        ClientVersion { v2: false, filename: "metin2client".into(), timestamp: "1215955205".into() }
    }
}

/// Construction-time settings.
#[derive(Clone, Debug)]
pub struct NetConfig {
    pub recv_capacity: usize,
    pub send_capacity: usize,
    /// `g_adwEncryptKey`: the client key sent with `CG_LOGIN2`.
    pub client_key: [u32; 4],
    pub version: ClientVersion,
}

impl Default for NetConfig {
    fn default() -> Self {
        NetConfig { recv_capacity: 128 * 1024, send_capacity: 64 * 1024, client_key: [0; 4], version: ClientVersion::default() }
    }
}

const LOGIN_SET: &[u8] = &[
    gc::PHASE,
    gc::LOGIN_SUCCESS3,
    gc::LOGIN_SUCCESS4,
    gc::LOGIN_FAILURE,
    gc::EMPIRE,
    gc::CHINA_MATRIX_CARD,
    gc::RUNUP_MATRIX_QUIZ,
    gc::NEWCIBN_PASSPOD_REQUEST,
    gc::NEWCIBN_PASSPOD_FAILURE,
    gc::LOGIN_KEY,
    gc::PING,
];

const SELECT_SET: &[u8] = &[
    gc::PHASE,
    gc::EMPIRE,
    gc::LOGIN_SUCCESS3,
    gc::LOGIN_SUCCESS4,
    gc::PLAYER_CREATE_SUCCESS,
    gc::PLAYER_CREATE_FAILURE,
    gc::PLAYER_DELETE_WRONG_SOCIAL_ID,
    gc::PLAYER_DELETE_SUCCESS,
    gc::CHANGE_NAME,
    gc::HANDSHAKE,
    gc::HANDSHAKE_OK,
    gc::PLAYER_POINT_CHANGE,
    gc::PING,
];

/// Headers `LoadingPhase` handles before falling through to `GamePhase`.
const LOADING_OWN: &[u8] = &[
    gc::PHASE,
    gc::MAIN_CHARACTER,
    gc::MAIN_CHARACTER2_EMPIRE,
    gc::MAIN_CHARACTER3_BGM,
    gc::MAIN_CHARACTER4_BGM_VOL,
    gc::CHARACTER_UPDATE,
    gc::PLAYER_POINTS,
    gc::PLAYER_POINT_CHANGE,
    gc::ITEM_SET,
    gc::PING,
    gc::QUICKSLOT_ADD,
];

/// The `switch` of `CPythonNetworkStream::GamePhase`.
const GAME_SET: &[u8] = &[
    gc::OBSERVER_ADD,
    gc::OBSERVER_REMOVE,
    gc::OBSERVER_MOVE,
    gc::WARP,
    gc::PHASE,
    gc::PVP,
    gc::DUEL_START,
    gc::CHARACTER_ADD,
    gc::CHAR_ADDITIONAL_INFO,
    gc::CHARACTER_ADD2,
    gc::CHARACTER_UPDATE,
    gc::CHARACTER_UPDATE2,
    gc::CHARACTER_DEL,
    gc::CHAT,
    gc::SYNC_POSITION,
    gc::OWNERSHIP,
    gc::WHISPER,
    gc::CHARACTER_MOVE,
    gc::CHARACTER_POSITION,
    gc::STUN,
    gc::DEAD,
    gc::PLAYER_POINT_CHANGE,
    gc::ITEM_SET,
    gc::ITEM_SET2,
    gc::ITEM_USE,
    gc::ITEM_UPDATE,
    gc::ITEM_GROUND_ADD,
    gc::ITEM_GROUND_DEL,
    gc::ITEM_OWNERSHIP,
    gc::QUICKSLOT_ADD,
    gc::QUICKSLOT_DEL,
    gc::QUICKSLOT_SWAP,
    gc::MOTION,
    gc::SHOP,
    gc::SHOP_SIGN,
    gc::EXCHANGE,
    gc::QUEST_INFO,
    gc::REQUEST_MAKE_GUILD,
    gc::PING,
    gc::SCRIPT,
    gc::QUEST_CONFIRM,
    gc::TARGET,
    gc::DAMAGE_INFO,
    gc::MOUNT,
    gc::CHANGE_SPEED,
    gc::PLAYER_POINTS,
    gc::CREATE_FLY,
    gc::FLY_TARGETING,
    gc::ADD_FLY_TARGETING,
    gc::SKILL_LEVEL,
    gc::SKILL_LEVEL_NEW,
    gc::MESSENGER,
    gc::GUILD,
    gc::PARTY_INVITE,
    gc::PARTY_ADD,
    gc::PARTY_UPDATE,
    gc::PARTY_REMOVE,
    gc::PARTY_LINK,
    gc::PARTY_UNLINK,
    gc::PARTY_PARAMETER,
    gc::SAFEBOX_SET,
    gc::SAFEBOX_DEL,
    gc::SAFEBOX_WRONG_PASSWORD,
    gc::SAFEBOX_SIZE,
    gc::SAFEBOX_MONEY_CHANGE,
    gc::FISHING,
    gc::DUNGEON,
    gc::TIME,
    gc::WALK_MODE,
    gc::CHANGE_SKILL_GROUP,
    gc::REFINE_INFORMATION,
    gc::REFINE_INFORMATION_NEW,
    gc::SEPCIAL_EFFECT,
    gc::NPC_POSITION,
    gc::CHANNEL,
    gc::VIEW_EQUIP,
    gc::LAND_LIST,
    gc::TARGET_CREATE_NEW,
    gc::TARGET_UPDATE,
    gc::TARGET_DELETE,
    gc::AFFECT_ADD,
    gc::AFFECT_REMOVE,
    gc::MALL_OPEN,
    gc::MALL_SET,
    gc::MALL_DEL,
    gc::LOVER_INFO,
    gc::LOVE_POINT_UPDATE,
    gc::DIG_MOTION,
    gc::HANDSHAKE,
    gc::HANDSHAKE_OK,
    gc::SPECIFIC_EFFECT,
    gc::DRAGON_SOUL_REFINE,
];

/// Is `header` handled by `phase`'s dispatch (the C++ `switch` including the Loading to
/// Game fall-through)?
pub fn accepts(phase: Phase, header: u8) -> bool {
    match phase {
        Phase::Offline => header == gc::PHASE,
        Phase::HandShake => matches!(header, gc::PHASE | gc::HANDSHAKE | gc::PING),
        Phase::Login => LOGIN_SET.contains(&header),
        Phase::Select => SELECT_SET.contains(&header),
        Phase::Loading => LOADING_OWN.contains(&header) || GAME_SET.contains(&header),
        Phase::Game => GAME_SET.contains(&header),
    }
}

/// Server headers in the header map that no phase handles (the C++ reports them as
/// unprocessed packets and clears the buffer if they ever arrive).
pub fn unhandled_headers() -> Vec<&'static str> {
    wire::GC_HEADER_MAP
        .iter()
        .filter(|h| ![Phase::Offline, Phase::HandShake, Phase::Login, Phase::Select, Phase::Loading, Phase::Game].iter().any(|p| accepts(*p, h.header)))
        .map(|h| h.name)
        .collect()
}

fn affect_flags(f: &[u32; 2]) -> u64 {
    f[0] as u64 | (f[1] as u64) << 32
}

fn is_invisible_race(race: u16) -> bool {
    matches!(race, 20025 | 20038 | 20039)
}

/// The state of `CPythonNetworkStream` that the packet handlers read and write.
pub struct NetMachine {
    pub stream: NetStream,
    phase: Phase,
    config: NetConfig,
    login_id: String,
    login_password: String,
    login_key: u32,
    empire: u8,
    players: [TSimplePlayerInformation; 4],
    guild_ids: [u32; 4],
    guild_names: [String; 4],
    mark_handle: u32,
    mark_random_key: u32,
    main: Option<MainCharacter>,
    direct_enter: Option<u8>,
    selected_slot: u8,
    pending_actor: Option<ActorData>,
    time_sync: (u32, u32),
    events: Vec<NetEvent>,
}

impl NetMachine {
    pub fn new(config: NetConfig) -> Self {
        NetMachine {
            stream: NetStream::new(config.recv_capacity, config.send_capacity),
            phase: Phase::Offline,
            config,
            login_id: String::new(),
            login_password: String::new(),
            login_key: 0,
            empire: 0,
            players: [TSimplePlayerInformation::default(); 4],
            guild_ids: [0; 4],
            guild_names: Default::default(),
            mark_handle: 0,
            mark_random_key: 0,
            main: None,
            direct_enter: None,
            selected_slot: 0,
            pending_actor: None,
            time_sync: (0, 0),
            events: Vec::new(),
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn players(&self) -> &[TSimplePlayerInformation; 4] {
        &self.players
    }
    pub fn guild_names(&self) -> &[String; 4] {
        &self.guild_names
    }
    pub fn main_character(&self) -> Option<&MainCharacter> {
        self.main.as_ref()
    }
    pub fn empire(&self) -> u8 {
        self.empire
    }
    pub fn login_key(&self) -> u32 {
        self.login_key
    }
    pub fn direct_enter_slot(&self) -> Option<u8> {
        self.direct_enter
    }

    /// `ConnectGameServer`'s state change: enter the game with `slot` after the next
    /// connection (its address is the slot's `lAddr`/`wPort`). The login then keeps the
    /// character list, and the driver sends [`Self::send_select_character`] when the
    /// server reaches the Select phase. `None` for a slot past `PLAYER_PER_ACCOUNT4`.
    pub fn set_direct_enter(&mut self, slot: u8) -> Option<&TSimplePlayerInformation> {
        if usize::from(slot) >= self.players.len() {
            return None;
        }
        self.selected_slot = slot;
        self.direct_enter = Some(slot);
        Some(&self.players[usize::from(slot)])
    }

    /// `SetLoginInfo`.
    pub fn set_login_info(&mut self, id: &str, password: &str) {
        self.login_id = id.to_owned();
        self.login_password = password.to_owned();
    }

    /// `SetLoginKey`: non-zero selects `CG_LOGIN2` (login through the auth server).
    pub fn set_login_key(&mut self, key: u32) {
        self.login_key = key;
    }

    /// `netSetPacketSequenceMode`.
    pub fn set_sequence_mode(&mut self, on: bool) {
        self.stream.set_sequence_mode(on);
    }

    /// The transport closed (`OnRemoteDisconnect`/`Disconnect` then `SetOffLinePhase`).
    pub fn on_disconnect(&mut self) {
        self.stream.clear();
        self.phase = Phase::Offline;
        self.selected_slot = 0;
        self.direct_enter = None;
        self.pending_actor = None;
    }

    /// A new connection was established (the server will start with `PHASE_HANDSHAKE`).
    /// Keeps the phase variable like `CNetworkStream::Connect` (which only `Clear()`s
    /// the buffers), e.g. after a [`NetEvent::Warp`].
    pub fn on_connected(&mut self) {
        self.stream.clear();
    }

    /// Feeds bytes read from the transport; returns how many were taken.
    pub fn feed(&mut self, data: &[u8]) -> usize {
        self.stream.feed(data)
    }

    /// Bytes to write to the transport.
    pub fn take_output(&mut self, max: usize) -> Vec<u8> {
        self.stream.take_output(max)
    }

    /// Dispatches every complete packet and returns the resulting events.
    /// `now_ms` is `ELTimer_GetMSec()`, used only for the server time sync.
    pub fn poll(&mut self, now_ms: u32) -> Vec<NetEvent> {
        loop {
            match self.stream.poll_frame() {
                Ok(Some(frame)) => self.handle(frame.header, &frame.bytes, now_ms),
                Ok(None) => break,
                Err(e) => self.events.push(NetEvent::Frame(e)),
            }
        }
        std::mem::take(&mut self.events)
    }

    fn set_phase(&mut self, to: Phase) {
        let from = self.phase;
        self.phase = to;
        if from != to {
            self.events.push(NetEvent::PhaseChanged { from, to });
        }
        match to {
            Phase::Login => self.enter_login_phase(),
            Phase::Loading => self.direct_enter = None,
            _ => {}
        }
    }

    /// `SetLoginPhase`: sends the login packet right away.
    fn enter_login_phase(&mut self) {
        if self.login_key != 0 {
            let mut p = TPacketCGLogin2::default();
            p.header = cg::LOGIN2;
            p.login_key = self.login_key;
            p.set_name_str(&self.login_id);
            p.adwClientKey = self.config.client_key;
            self.stream.send_packet(&p);
        } else {
            let mut p = TPacketCGLogin::default();
            p.header = cg::LOGIN;
            p.set_name_str(&self.login_id);
            p.set_pwd_str(&self.login_password);
            self.stream.send_packet(&p);
        }
        // "ClearLoginInfo": the password is dropped right after use.
        self.login_password.clear();
        if self.direct_enter.is_none() {
            self.players = [TSimplePlayerInformation::default(); 4];
            self.guild_ids = [0; 4];
            self.guild_names = Default::default();
        }
    }

    fn handle(&mut self, header: u8, bytes: &[u8], now_ms: u32) {
        if !accepts(self.phase, header) {
            // RecvErrorPacket / RecvDefaultPacket: log and drop everything buffered.
            self.stream.clear_recv();
            self.events.push(NetEvent::Unhandled { phase: self.phase, header });
            return;
        }
        if header == gc::SAFEBOX_SET || header == gc::MALL_SET {
            match wire::TPacketGCItemSet2::decode(bytes) {
                Ok(p) => self.events.push(if header == gc::MALL_SET { NetEvent::MallSet(p) } else { NetEvent::SafeboxSet(p) }),
                Err(_) => self.events.push(NetEvent::Malformed { header }),
            }
            return;
        }
        let pkt = match GcPacket::decode(bytes) {
            Ok(p) => p,
            Err(_) => {
                self.events.push(NetEvent::Malformed { header });
                return;
            }
        };
        match pkt {
            GcPacket::Phase(p) => self.on_phase(p.phase),
            GcPacket::Handshake(h) => self.on_handshake(h),
            GcPacket::HandshakeOk(_) => {
                let (server, client) = self.time_sync;
                let delta = now_ms.wrapping_sub(client);
                self.events.push(NetEvent::ServerTime { server_ms: server.wrapping_add(delta) });
            }
            GcPacket::Ping(_) => {
                let pong = TPacketCGPong { bHeader: cg::PONG };
                self.stream.send_packet(&pong);
                self.events.push(NetEvent::Ping);
            }
            GcPacket::LoginSuccess3(p) => self.on_login_success(&p.akSimplePlayerInformation, &p.guild_id, &p.guild_name, p.handle, p.random_key),
            GcPacket::LoginSuccess4(p) => self.on_login_success(&p.akSimplePlayerInformation, &p.guild_id, &p.guild_name, p.handle, p.random_key),
            GcPacket::LoginFailure(p) => self.events.push(NetEvent::LoginFailure { status: p.szStatus_str() }),
            GcPacket::Empire(p) => {
                self.empire = p.bEmpire;
                self.events.push(NetEvent::Empire(p.bEmpire));
            }
            GcPacket::LoginKey(p) => {
                self.login_key = p.dwLoginKey;
                self.events.push(NetEvent::LoginKey(p.dwLoginKey));
            }
            GcPacket::ChinaMatrixCard(p) => {
                // ROW(rows, i) = (rows >> ((4 - i - 1) * 8)) & 0xff
                let b = |v: u32, i: u32| ((v >> ((4 - i - 1) * 8)) & 0xff) as u8;
                self.events.push(NetEvent::MatrixCard {
                    rows: [b(p.dwRows, 0), b(p.dwRows, 1), b(p.dwRows, 2), b(p.dwRows, 3)],
                    cols: [b(p.dwCols, 0), b(p.dwCols, 1), b(p.dwCols, 2), b(p.dwCols, 3)],
                });
            }
            GcPacket::RunupMatrixQuiz(p) => self.events.push(NetEvent::RunupMatrixQuiz(p.szQuiz_str())),
            GcPacket::NewcibnPasspodRequest(_) => self.events.push(NetEvent::PasspodRequest),
            GcPacket::NewcibnPasspodFailure(_) => self.events.push(NetEvent::PasspodFailure),
            GcPacket::PlayerCreateSuccess(p) => {
                if p.bAccountCharacterSlot as usize >= consts::PLAYER_PER_ACCOUNT4 {
                    self.events.push(NetEvent::PlayerCreateBadSlot { slot: p.bAccountCharacterSlot });
                } else {
                    self.players[p.bAccountCharacterSlot as usize] = p.kSimplePlayerInfomation;
                    self.events.push(NetEvent::PlayerCreateSuccess { slot: p.bAccountCharacterSlot, info: p.kSimplePlayerInfomation });
                }
            }
            GcPacket::PlayerCreateFailure(p) => self.events.push(NetEvent::PlayerCreateFailure { kind: p.bType }),
            GcPacket::PlayerDeleteSuccess(_) | GcPacket::PlayerDeleteWrongSocialId(_) => {
                // Both map to TPacketGCBlank in the header map, but the C++ reads the delete success
                // as TPacketGCDestroyCharacterSuccess (2 bytes); see `on_delete`.
                self.on_delete(header, bytes);
            }
            GcPacket::ChangeName(p) => {
                match self.players.iter().position(|pl| pl.dwID == p.pid) {
                    Some(slot) => {
                        self.players[slot].bChangeName = 0;
                        self.players[slot].szName = p.name;
                        self.events.push(NetEvent::ChangeName { slot, name: p.name_str() });
                    }
                    None => self.events.push(NetEvent::PlayerCreateFailure { kind: 100 }),
                }
            }
            GcPacket::MainCharacter(p) => self.on_main(p.dwVID, p.wRaceNum, 0, p.bySkillGroup, p.szName_str(), p.lX, p.lY, None),
            GcPacket::MainCharacter2Empire(p) => self.on_main(p.dwVID, p.wRaceNum, p.byEmpire, p.bySkillGroup, p.szName_str(), p.lX, p.lY, None),
            GcPacket::MainCharacter3Bgm(p) => {
                self.on_main(p.dwVID, p.wRaceNum, p.byEmpire, p.bySkillGroup, p.szUserName_str(), p.lX, p.lY, Some((p.szBGMName_str(), None)))
            }
            GcPacket::MainCharacter4BgmVol(p) => self.on_main(
                p.dwVID,
                p.wRaceNum,
                p.byEmpire,
                p.bySkillGroup,
                p.szUserName_str(),
                p.lX,
                p.lY,
                Some((p.szBGMName_str(), Some(p.fBGMVol))),
            ),
            GcPacket::CharacterAdd(p) => {
                let d = ActorData {
                    vid: p.dwVID,
                    kind: p.bType,
                    race: p.wRaceNum,
                    move_speed: p.bMovingSpeed,
                    attack_speed: p.bAttackSpeed,
                    state_flags: p.bStateFlag,
                    rotation: p.angle,
                    x: p.x,
                    y: p.y,
                    affect_flags: affect_flags(&p.dwAffectFlag),
                    name: None,
                    guild_id: 0,
                    level: 0,
                    alignment: 0,
                    pk_mode: 0,
                    empire: 0,
                    armor: 0,
                    weapon: 0,
                    hair: 0,
                    mount_vnum: 0,
                };
                if d.kind != ACTOR_TYPE_PC && d.kind != ACTOR_TYPE_NPC {
                    self.events.push(NetEvent::ActorAppend(d));
                } else {
                    // Players and NPCs wait for HEADER_GC_CHAR_ADDITIONAL_INFO (s_kNetActorData).
                    self.pending_actor = Some(d);
                }
            }
            GcPacket::CharAdditionalInfo(p) => {
                let Some(mut d) = self.pending_actor.clone() else {
                    self.events.push(NetEvent::ActorInfoMismatch { vid: p.dwVID });
                    return;
                };
                if is_invisible_race(d.race) {
                    return;
                }
                if d.vid == p.dwVID {
                    d.name = Some(p.name_str());
                    d.guild_id = p.dwGuildID;
                    d.level = p.dwLevel;
                    d.alignment = p.sAlignment;
                    d.pk_mode = p.bPKMode;
                    d.empire = p.bEmpire;
                    d.armor = p.awPart[PART_ARMOR];
                    d.weapon = p.awPart[PART_WEAPON];
                    d.hair = p.awPart[PART_HAIR];
                    d.mount_vnum = p.dwMountVnum;
                    self.events.push(NetEvent::ActorAppend(d));
                } else {
                    self.events.push(NetEvent::ActorInfoMismatch { vid: p.dwVID });
                }
            }
            GcPacket::CharacterAdd2(p) => {
                if is_invisible_race(p.wRaceNum) {
                    return;
                }
                self.events.push(NetEvent::ActorAppend(ActorData {
                    vid: p.dwVID,
                    kind: p.bType,
                    race: p.wRaceNum,
                    move_speed: p.bMovingSpeed,
                    attack_speed: p.bAttackSpeed,
                    state_flags: p.bStateFlag,
                    rotation: p.angle,
                    x: p.x,
                    y: p.y,
                    affect_flags: affect_flags(&p.dwAffectFlag),
                    name: Some(p.name_str()),
                    guild_id: p.dwGuild,
                    level: 0,
                    alignment: p.sAlignment,
                    pk_mode: p.bPKMode,
                    empire: p.bEmpire,
                    armor: p.awPart[PART_ARMOR],
                    weapon: p.awPart[PART_WEAPON],
                    hair: p.awPart[PART_HAIR],
                    mount_vnum: p.dwMountVnum,
                }));
            }
            GcPacket::CharacterUpdate(p) => self.events.push(NetEvent::ActorUpdate(ActorUpdate {
                vid: p.dwVID,
                guild_id: p.dwGuildID,
                move_speed: p.bMovingSpeed,
                attack_speed: p.bAttackSpeed,
                state_flags: p.bStateFlag,
                affect_flags: affect_flags(&p.dwAffectFlag),
                alignment: p.sAlignment,
                pk_mode: p.bPKMode,
                armor: p.awPart[PART_ARMOR],
                weapon: p.awPart[PART_WEAPON],
                hair: p.awPart[PART_HAIR],
                mount_vnum: p.dwMountVnum,
            })),
            GcPacket::CharacterUpdate2(p) => self.events.push(NetEvent::ActorUpdate(ActorUpdate {
                vid: p.dwVID,
                guild_id: p.dwGuildID,
                move_speed: p.bMovingSpeed,
                attack_speed: p.bAttackSpeed,
                state_flags: p.bStateFlag,
                affect_flags: affect_flags(&p.dwAffectFlag),
                alignment: p.sAlignment,
                pk_mode: p.bPKMode,
                armor: p.awPart[PART_ARMOR],
                weapon: p.awPart[PART_WEAPON],
                hair: p.awPart[PART_HAIR],
                mount_vnum: p.dwMountVnum,
            })),
            GcPacket::CharacterDel(p) => self.events.push(NetEvent::ActorRemove { vid: p.dwVID }),
            GcPacket::CharacterMove(p) => self.events.push(NetEvent::ActorMove(ActorMove {
                vid: p.dwVID,
                func: p.bFunc,
                arg: p.bArg,
                rotation: p.bRot as f32 * 5.0,
                x: p.lX,
                y: p.lY,
                time: p.dwTime,
                duration: p.dwDuration,
            })),
            GcPacket::Ownership(p) => self.events.push(NetEvent::ActorOwner { owner_vid: p.dwOwnerVID, victim_vid: p.dwVictimVID }),
            GcPacket::SyncPosition { tail, .. } => {
                if let Ok(els) = wire::tails::sync_positions(&tail) {
                    for e in els {
                        self.events.push(NetEvent::ActorSync { vid: e.dwVID, x: e.lX, y: e.lY });
                    }
                }
            }
            GcPacket::Chat { head, tail } => {
                self.events.push(NetEvent::Chat { kind: head.r#type, vid: head.dwVID, empire: head.bEmpire, text: wire::tails::text(&tail) })
            }
            GcPacket::Whisper(p) => {
                let text = wire::tails::text(bytes.get(wire::TPacketGCWhisper::SIZE..).unwrap_or(&[]));
                self.events.push(NetEvent::Whisper { kind: p.bType, from: p.szNameFrom_str(), text });
            }
            GcPacket::Warp(p) => {
                self.direct_enter = Some(self.selected_slot);
                self.events.push(NetEvent::Warp { x: p.lX, y: p.lY, addr: p.lAddr as u32, port: p.wPort });
            }
            other => self.events.push(NetEvent::Packet(other)),
        }
    }

    fn on_delete(&mut self, header: u8, bytes: &[u8]) {
        if header == gc::PLAYER_DELETE_WRONG_SOCIAL_ID {
            self.events.push(NetEvent::PlayerDeleteFailure);
            return;
        }
        // TPacketGCDestroyCharacterSuccess {header, account_index}; the header map's size for this
        // header is sizeof(TPacketGCBlank) = 1, so the index byte (if the server sends it) is
        // the first byte of the *next* frame in C++ as well (see c++_rust_bugs note in the crate docs).
        let slot = bytes.get(1).copied().unwrap_or(0);
        if (slot as usize) < consts::PLAYER_PER_ACCOUNT4 {
            self.players[slot as usize] = TSimplePlayerInformation::default();
            self.guild_ids[slot as usize] = 0;
            self.guild_names[slot as usize].clear();
        }
        self.events.push(NetEvent::PlayerDeleteSuccess { slot });
    }

    fn on_phase(&mut self, id: u8) {
        match id {
            phase_id::CLOSE => self.events.push(NetEvent::PhaseClose),
            phase_id::HANDSHAKE => self.set_phase(Phase::HandShake),
            phase_id::LOGIN => self.set_phase(Phase::Login),
            phase_id::SELECT => self.set_phase(Phase::Select),
            phase_id::LOADING => self.set_phase(Phase::Loading),
            phase_id::GAME => self.set_phase(Phase::Game),
            _ => {} // PHASE_DEAD and the server-only phases: no case in the C++ switch
        }
    }

    fn on_handshake(&mut self, mut h: TPacketGCHandshake) {
        let server = h.dwTime.wrapping_add(h.lDelta as u32);
        if self.phase == Phase::HandShake {
            // HandShakePhase: echo with dwTime += 2 * lDelta, lDelta = 0, header unchanged
            // (0xff is also HEADER_CG_HANDSHAKE), and no SendSequence.
            h.dwTime = server.wrapping_add(h.lDelta as u32);
            h.lDelta = 0;
            self.stream.send(&h.encode());
            self.events.push(NetEvent::HandshakeReceived { server_time: server });
        } else {
            // RecvHandshakePacket: answer with HEADER_CG_TIME_SYNC, remember when for HANDSHAKE_OK.
            self.time_sync = (server, 0);
            h.dwTime = server.wrapping_add(h.lDelta as u32);
            h.lDelta = 0;
            h.header = cg::TIME_SYNC;
            self.stream.send_packet(&h);
            self.events.push(NetEvent::HandshakeReceived { server_time: server });
        }
    }

    fn on_login_success(&mut self, infos: &[TSimplePlayerInformation], guild_ids: &[u32], guild_names: &[[u8; 13]], handle: u32, key: u32) {
        for i in 0..infos.len() {
            self.players[i] = infos[i];
            self.guild_ids[i] = guild_ids[i];
            self.guild_names[i] = wire::cstr_to_string(&guild_names[i]);
        }
        self.mark_handle = handle;
        self.mark_random_key = key;
        self.events.push(NetEvent::LoginSuccess { players: self.players.to_vec(), mark_handle: handle, mark_random_key: key });
    }

    #[allow(clippy::too_many_arguments)]
    fn on_main(&mut self, vid: u32, race: u16, empire: u8, skill_group: u8, name: String, x: i32, y: i32, bgm: Option<(String, Option<f32>)>) {
        let m = MainCharacter { vid, race, empire, skill_group, name, x, y, bgm };
        self.main = Some(m.clone());
        self.events.push(NetEvent::MainCharacter(m));
        self.send_client_version();
    }

    // ---------------------------------------------------------------- senders

    /// `SendClientVersionPacket`.
    pub fn send_client_version(&mut self) -> bool {
        let v = self.config.version.clone();
        if v.v2 {
            let mut p = TPacketCGClientVersion2::default();
            p.header = cg::CLIENT_VERSION2;
            p.set_filename_str(&v.filename);
            p.set_timestamp_str(&v.timestamp);
            self.stream.send_packet(&p)
        } else {
            let mut p = TPacketCGClientVersion::default();
            p.header = cg::CLIENT_VERSION;
            p.set_filename_str(&v.filename);
            p.set_timestamp_str(&v.timestamp);
            self.stream.send_packet(&p)
        }
    }

    /// `SendSelectEmpirePacket`.
    pub fn send_select_empire(&mut self, empire: u8) -> bool {
        self.empire = empire;
        self.stream.send_packet(&TPacketCGEmpire { bHeader: cg::EMPIRE, bEmpire: empire })
    }

    /// `SendSelectCharacterPacket`.
    pub fn send_select_character(&mut self, index: u8) -> bool {
        self.selected_slot = index;
        self.stream.send_packet(&TPacketCGSelectCharacter { header: cg::PLAYER_SELECT, player_index: index })
    }

    /// `SendCreateCharacterPacket`.
    #[allow(clippy::too_many_arguments)]
    pub fn send_create_character(&mut self, index: u8, name: &str, job: u16, shape: u8, con: u8, int: u8, str_: u8, dex: u8) -> bool {
        let mut p = TPacketCGCreateCharacter::default();
        p.header = cg::PLAYER_CREATE;
        p.index = index;
        p.set_name_str(name);
        p.job = job;
        p.shape = shape;
        p.CON = con;
        p.INT = int;
        p.STR = str_;
        p.DEX = dex;
        self.stream.send_packet(&p)
    }

    /// `SendDestroyCharacterPacket` (`strncpy(..., PRIVATE_CODE_LENGTH - 1)`).
    pub fn send_destroy_character(&mut self, index: u8, private_code: &str) -> bool {
        let mut p = TPacketCGDestroyCharacter::default();
        p.header = cg::PLAYER_DESTROY;
        p.index = index;
        p.set_szPrivateCode_str(private_code);
        self.stream.send_packet(&p)
    }

    /// `SendEnterGame`.
    pub fn send_enter_game(&mut self) -> bool {
        self.stream.send_packet(&TPacketCGEnterFrontGame { header: cg::ENTERGAME })
    }

    /// `SendDirectEnterPacket`.
    pub fn send_direct_enter(&mut self, id: &str, password: &str, slot: u8) -> bool {
        let mut p = TPacketCGDirectEnter::default();
        p.bHeader = cg::DIRECT_ENTER;
        p.index = slot;
        p.set_login_str(id);
        p.set_passwd_str(password);
        self.stream.send_packet(&p)
    }

    /// `SendChatPacket` without the client-side `/command` handling: empty and
    /// 512-byte-or-longer texts are dropped (returns true), the text is sent NUL-terminated.
    pub fn send_chat(&mut self, text: &str, kind: u8) -> bool {
        if text.is_empty() || text.len() >= 512 {
            return true;
        }
        let head = wire::TPacketCGChat { header: cg::CHAT, length: (wire::TPacketCGChat::SIZE + text.len() + 1) as u16, r#type: kind };
        let mut body = text.as_bytes().to_vec();
        body.push(0);
        self.stream.send(&head.encode()) && self.stream.send(&body) && self.stream.send_sequence()
    }

    /// Use an item in the player inventory.
    pub fn send_item_use(&mut self, cell: u16) -> bool {
        let packet = wire::TPacketCGItemUse { header: cg::ITEM_USE, pos: wire::TItemPos { window_type: 1, cell } };
        self.stream.send_packet(&packet)
    }

    /// `SendCharacterStatePacket` (position already in global coordinates).
    pub fn send_move(&mut self, func: u8, arg: u8, rot_degrees: f32, x: i32, y: i32, server_ms: u32) -> bool {
        let mut rot = rot_degrees;
        if rot < 0.0 {
            rot += 360.0;
        } else if rot > 360.0 {
            rot %= 360.0;
        }
        self.stream.send_packet(&TPacketCGMove { bHeader: cg::CHARACTER_MOVE, bFunc: func, bArg: arg, bRot: (rot / 5.0) as u8, lX: x, lY: y, dwTime: server_ms })
    }

    /// `SendAttackPacket`: `skill` is 0 for a normal attack, `victim` the target's
    /// VID (0 for a swing at nothing).
    pub fn send_attack(&mut self, skill: u8, victim: u32) -> bool {
        self.stream.send_packet(&TPacketCGAttack {
            header: cg::ATTACK,
            bType: skill,
            dwVictimVID: victim,
            bCRCMagicCubeProcPiece: 0,
            bCRCMagicCubeFilePiece: 0,
        })
    }
}
