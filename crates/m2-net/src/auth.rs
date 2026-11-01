//! The account (auth) server connection: a port of `CAccountConnector`
//! (`client/src/ui/AccountConnector.cpp`), sans-IO like [`crate::phase`].
//!
//! ```text
//!  connect --> Handshake --PHASE_AUTH: send CG_LOGIN3--> Auth --GC_AUTH_SUCCESS--> done
//! ```
//!
//! The account server answers `CG_LOGIN3` (id, password, client key) with a login
//! key; the client then disconnects and logs in to the game server with `CG_LOGIN2`
//! carrying that key ([`crate::phase::NetMachine::set_login_key`]). The driver does
//! both steps when it sees [`AuthEvent::Success`].
//!
//! Differences from C++ (`c++_rust_bugs.md` N36, N37): the C++ `__AnalyzePacket` chain
//! leaves a header it does not know in the buffer forever (the connection stalls);
//! this port frames every packet with the header map, drops an unhandled one and
//! reports [`AuthEvent::Unhandled`]. The OpenID login (`USE_OPENID`, `CG_LOGIN5`) is not
//! ported; the client is not built with it.

use crate::phase::phase_id;
use crate::stream::{FrameError, NetStream};
use crate::wire::{GcPacket, TPacketCGLogin3, TPacketCGPong, TPacketGCHandshake, Wire, cg};

/// A fresh `g_adwEncryptKey`. `__BuildClientKey_20050304Myevan` fills it with
/// `random()`; the server only stores it, so any random value does. Seeded by std's
/// per-process random hasher keys.
pub fn random_client_key() -> [u32; 4] {
    use std::hash::BuildHasher;
    let state = std::collections::hash_map::RandomState::new();
    std::array::from_fn(|i| state.hash_one(i) as u32)
}

/// `CAccountConnector::m_eState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthState {
    Offline,
    Handshake,
    Auth,
}

/// What the account connection reports to the UI (the C++ `m_poHandler` calls).
#[derive(Clone, Debug, PartialEq)]
pub enum AuthEvent {
    /// `HEADER_GC_HANDSHAKE`; `server_time` is `dwTime + lDelta` (`ELTimer_SetServerMSec`).
    /// The echo is already queued.
    Handshake { server_time: u32 },
    /// `PHASE_AUTH`: `CG_LOGIN3` is queued and the password forgotten.
    LoginSent,
    /// `GC_AUTH_SUCCESS` with `bResult != 0`: connect to the game server and log in with
    /// this key. The account connection is finished ([`AuthMachine::finished`]).
    Success { login_key: u32 },
    /// `OnLoginFailure`: `GC_LOGIN_FAILURE`'s `szStatus` (`"WRONGPWD"`, `"NOID"`, ...),
    /// or `"BESAMEKEY"` for a `GC_AUTH_SUCCESS` with `bResult == 0` (which also
    /// finishes the connection).
    Failure { status: String },
    /// `OnMatrixCard`.
    MatrixCard { rows: [u8; 4], cols: [u8; 4] },
    /// `BINARY_OnRunupMatrixQuiz`.
    RunupMatrixQuiz(String),
    /// `BINARY_OnNEWCIBNPasspodRequest`.
    PasspodRequest,
    /// A header this state does not handle (dropped; C++ stalls on it).
    Unhandled { state: AuthState, header: u8 },
    /// Framing failed; the receive buffer was cleared.
    Frame(FrameError),
}

/// `CAccountConnector`'s state and buffers.
pub struct AuthMachine {
    pub stream: NetStream,
    state: AuthState,
    id: String,
    password: String,
    client_key: [u32; 4],
    finished: bool,
    events: Vec<AuthEvent>,
}

impl AuthMachine {
    /// `client_key` is `g_adwEncryptKey` (sent in `CG_LOGIN3`). The C++ uses a
    /// 128 KiB receive and a 2 KiB send buffer.
    pub fn new(client_key: [u32; 4]) -> Self {
        AuthMachine {
            stream: NetStream::new(128 * 1024, 2048),
            state: AuthState::Offline,
            id: String::new(),
            password: String::new(),
            client_key,
            finished: false,
            events: Vec::new(),
        }
    }

    pub fn state(&self) -> AuthState {
        self.state
    }

    /// The server answered (success or `BESAMEKEY`): close the connection.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// `SetLoginInfo`.
    pub fn set_login_info(&mut self, id: &str, password: &str) {
        self.id = id.to_owned();
        self.password = password.to_owned();
    }

    /// `SetPacketSequenceMode` (the Python `net.SetPacketSequenceMode` sets it on both
    /// connections).
    pub fn set_sequence_mode(&mut self, on: bool) {
        self.stream.set_sequence_mode(on);
    }

    /// `OnConnectSuccess`: the server will send its handshake.
    pub fn on_connected(&mut self) {
        self.stream.clear();
        self.state = AuthState::Handshake;
        self.finished = false;
    }

    /// `OnRemoteDisconnect` / `Disconnect`.
    pub fn on_disconnect(&mut self) {
        self.stream.clear();
        self.state = AuthState::Offline;
    }

    /// Dispatches every complete packet and returns the resulting events.
    pub fn poll(&mut self) -> Vec<AuthEvent> {
        while !self.finished {
            match self.stream.poll_frame() {
                Ok(Some(frame)) => self.handle(frame.header, &frame.bytes),
                Ok(None) => break,
                Err(e) => self.events.push(AuthEvent::Frame(e)),
            }
        }
        std::mem::take(&mut self.events)
    }

    fn handle(&mut self, header: u8, bytes: &[u8]) {
        let pkt = match GcPacket::decode(bytes) {
            Ok(pkt) => pkt,
            Err(_) => {
                self.events.push(AuthEvent::Unhandled { state: self.state, header });
                return;
            }
        };
        match (self.state, pkt) {
            (AuthState::Handshake | AuthState::Auth, GcPacket::Phase(p)) => self.on_phase(p.phase),
            (AuthState::Handshake | AuthState::Auth, GcPacket::Handshake(h)) => self.on_handshake(h),
            (AuthState::Handshake | AuthState::Auth, GcPacket::Ping(_)) => {
                self.stream.send_packet(&TPacketCGPong { bHeader: cg::PONG });
            }
            (AuthState::Auth, GcPacket::AuthSuccess(p)) => {
                self.events.push(if p.bResult != 0 {
                    AuthEvent::Success { login_key: p.dwLoginKey }
                } else {
                    AuthEvent::Failure { status: "BESAMEKEY".into() }
                });
                // `Disconnect(); __OfflineState_Set();` either way.
                self.finished = true;
            }
            // `__AuthState_RecvAuthFailure` keeps the connection: its `__OfflineState_Set`
            // is commented out.
            (AuthState::Auth, GcPacket::LoginFailure(p)) => self.events.push(AuthEvent::Failure { status: p.szStatus_str() }),
            (AuthState::Auth, GcPacket::ChinaMatrixCard(p)) => {
                // ROW(rows, i) = (rows >> ((4 - i - 1) * 8)) & 0xff
                let b = |v: u32, i: u32| ((v >> ((4 - i - 1) * 8)) & 0xff) as u8;
                self.events.push(AuthEvent::MatrixCard {
                    rows: [b(p.dwRows, 0), b(p.dwRows, 1), b(p.dwRows, 2), b(p.dwRows, 3)],
                    cols: [b(p.dwCols, 0), b(p.dwCols, 1), b(p.dwCols, 2), b(p.dwCols, 3)],
                });
            }
            (AuthState::Auth, GcPacket::RunupMatrixQuiz(p)) => self.events.push(AuthEvent::RunupMatrixQuiz(p.szQuiz_str())),
            (AuthState::Auth, GcPacket::NewcibnPasspodRequest(_)) => self.events.push(AuthEvent::PasspodRequest),
            // C++ bug N36: the C++ never consumes this header and stalls.
            (state, _) => self.events.push(AuthEvent::Unhandled { state, header }),
        }
    }

    /// `__AuthState_RecvPhase`.
    fn on_phase(&mut self, phase: u8) {
        match phase {
            phase_id::HANDSHAKE => self.state = AuthState::Handshake,
            phase_id::AUTH => {
                let mut p = TPacketCGLogin3::default();
                p.header = cg::LOGIN3;
                p.set_name_str(&self.id);
                p.set_pwd_str(&self.password);
                p.adwClientKey = self.client_key;
                // "ClearLoginInfo": the password is dropped right after use.
                self.password.clear();
                self.stream.send_packet(&p);
                self.state = AuthState::Auth;
                self.events.push(AuthEvent::LoginSent);
            }
            _ => {}
        }
    }

    /// `__AuthState_RecvHandshake`: echo with `dwTime += 2 * lDelta`, `lDelta = 0`, no
    /// sequence byte.
    fn on_handshake(&mut self, mut h: TPacketGCHandshake) {
        let server = h.dwTime.wrapping_add(h.lDelta as u32);
        h.dwTime = server.wrapping_add(h.lDelta as u32);
        h.lDelta = 0;
        self.stream.send(&h.encode());
        self.events.push(AuthEvent::Handshake { server_time: server });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::SEQUENCE_TABLE;
    use crate::wire::{TPacketGCAuthSuccess, TPacketGCLoginFailure, TPacketGCPhase, gc};

    fn phase(id: u8) -> Vec<u8> {
        TPacketGCPhase { header: gc::PHASE, phase: id }.encode()
    }

    fn handshake(time: u32, delta: i32) -> Vec<u8> {
        TPacketGCHandshake { header: gc::HANDSHAKE, dwHandshake: 7, dwTime: time, lDelta: delta }.encode()
    }

    fn connected(sequence: bool) -> AuthMachine {
        let mut m = AuthMachine::new([1, 2, 3, 4]);
        m.set_login_info("admin", "secret");
        m.set_sequence_mode(sequence);
        m.on_connected();
        m
    }

    #[test]
    fn handshake_then_auth_sends_login3_with_one_sequence_byte() {
        let mut m = connected(true);
        m.stream.feed(&[phase(phase_id::HANDSHAKE), handshake(1000, 5)].concat());
        assert_eq!(m.poll(), vec![AuthEvent::Handshake { server_time: 1005 }]);
        // The echo carries no sequence byte.
        let echo = m.stream.take_output(usize::MAX);
        assert_eq!(echo, TPacketGCHandshake { header: gc::HANDSHAKE, dwHandshake: 7, dwTime: 1010, lDelta: 0 }.encode());

        m.stream.feed(&phase(phase_id::AUTH));
        assert_eq!(m.poll(), vec![AuthEvent::LoginSent]);
        assert_eq!(m.state(), AuthState::Auth);
        let out = m.stream.take_output(usize::MAX);
        assert_eq!(out.len(), TPacketCGLogin3::SIZE + 1);
        let login = TPacketCGLogin3::decode(&out[..TPacketCGLogin3::SIZE]).unwrap();
        assert_eq!(login.header, cg::LOGIN3);
        assert_eq!(login.name_str(), "admin");
        assert_eq!(login.pwd_str(), "secret");
        assert_eq!(login.adwClientKey, [1, 2, 3, 4]);
        assert_eq!(out[TPacketCGLogin3::SIZE], SEQUENCE_TABLE[0]);
        assert!(m.password.is_empty());
    }

    #[test]
    fn auth_success_finishes_with_the_key() {
        let mut m = connected(false);
        m.stream.feed(&phase(phase_id::AUTH));
        m.poll();
        let ok = TPacketGCAuthSuccess { bHeader: gc::AUTH_SUCCESS, dwLoginKey: 0xdead_beef, bResult: 1 }.encode();
        // Anything after the answer is not read: the connection is closed.
        m.stream.feed(&[ok, phase(phase_id::HANDSHAKE)].concat());
        assert_eq!(m.poll(), vec![AuthEvent::Success { login_key: 0xdead_beef }]);
        assert!(m.finished());
    }

    #[test]
    fn a_zero_result_is_besamekey_and_a_failure_keeps_the_connection() {
        let mut m = connected(false);
        m.stream.feed(&phase(phase_id::AUTH));
        m.poll();
        let mut fail = TPacketGCLoginFailure::default();
        fail.header = gc::LOGIN_FAILURE;
        fail.set_szStatus_str("WRONGPWD");
        m.stream.feed(&fail.encode());
        assert_eq!(m.poll(), vec![AuthEvent::Failure { status: "WRONGPWD".into() }]);
        assert!(!m.finished());
        m.stream.feed(&TPacketGCAuthSuccess { bHeader: gc::AUTH_SUCCESS, dwLoginKey: 9, bResult: 0 }.encode());
        assert_eq!(m.poll(), vec![AuthEvent::Failure { status: "BESAMEKEY".into() }]);
        assert!(m.finished());
    }

    #[test]
    fn ping_is_answered_and_unknown_packets_are_reported() {
        let mut m = connected(false);
        m.stream.feed(&[gc::PING]);
        assert!(m.poll().is_empty());
        assert_eq!(m.stream.take_output(usize::MAX), vec![cg::PONG]);
        // AUTH_SUCCESS before PHASE_AUTH is not handled in the handshake state.
        m.stream.feed(&TPacketGCAuthSuccess { bHeader: gc::AUTH_SUCCESS, dwLoginKey: 1, bResult: 1 }.encode());
        assert_eq!(m.poll(), vec![AuthEvent::Unhandled { state: AuthState::Handshake, header: gc::AUTH_SUCCESS }]);
    }
}
