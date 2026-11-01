//! The connection to the server: [`m2_net`]'s state machines over a
//! non-blocking [`TcpTransport`], pumped once per frame.
//!
//! A [`LoginRequest`] runs the original client's login with
//! `KEEP_ACCOUNT_CONNETION_ENABLE` (`net.ConnectToAccountServer`): connect to the
//! account server, send `CG_LOGIN3`, take the login key from `GC_AUTH_SUCCESS`,
//! disconnect, connect to the chosen channel and log in with `CG_LOGIN2`. Sequence
//! bytes are on, as `constInfo.SEQUENCE_PACKET_ENABLE` sets them. Everything the
//! server says comes out as [`NetMessage`]s; the game connection stays open in
//! [`Net::game`], whose senders the next screens use.
//!
//! Connecting blocks (`std` has no non-blocking connect), so it runs on the
//! `IoTaskPool` with the C++ client's 3 second limit.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task, futures::check_ready};
use m2_net::auth::{AuthEvent, AuthMachine, random_client_key};
use m2_net::phase::{NetConfig, NetEvent, NetMachine, Phase};
use m2_net::transport::pump;
use m2_net::transport::tcp::TcpTransport;

/// `CNetworkStream::Connect`'s limit.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// One channel of [`ServerInfo`].
#[derive(Clone, Debug)]
pub struct Channel {
    pub name: String,
    pub port: u16,
}

/// The server list entry (`serverinfo.py`'s `REGION_DICT`): one account server and
/// its channels on one host.
#[derive(Resource, Clone, Debug)]
pub struct ServerInfo {
    pub name: String,
    pub host: String,
    pub auth_port: u16,
    pub channels: Vec<Channel>,
}

impl Default for ServerInfo {
    /// The shipped `serverinfo.py`: "Metin2" on 127.0.0.1, auth 11000, CH1-CH4 on
    /// 13000-13030.
    fn default() -> Self {
        ServerInfo {
            name: "Metin2".into(),
            host: "127.0.0.1".into(),
            auth_port: 11000,
            channels: (0..4).map(|i| Channel { name: format!("CH{}", i + 1), port: 13000 + 10 * i }).collect(),
        }
    }
}

/// Starts a login (`net.SetLoginInfo` + `net.ConnectToAccountServer`). Ignored while
/// one is in progress or a game connection is open.
#[derive(Message, Clone, Debug)]
pub struct LoginRequest {
    pub id: String,
    pub password: String,
    /// Index into [`ServerInfo::channels`].
    pub channel: usize,
}

/// Enters the game with a character of the account (`net.DirectEnter`, then the
/// loading window's `SendSelectCharacterPacket`): reconnect to the slot's server,
/// log in again with the key, and select the character once the server is in the
/// Select phase. The server then sends the Loading phase and `MainCharacter`; call
/// [`Net::start_game`] when the game is ready to be shown. Ignored unless logged in
/// (game connection in the Select phase).
#[derive(Message, Clone, Copy, Debug)]
pub struct EnterGame {
    pub slot: u8,
}

/// Which of the two connections a [`NetMessage`] is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Server {
    Account,
    Game,
}

/// What the network reports, in order, once per frame.
// `NetEvent` is large (decoded packets inline); m2-net allows the same lint for it.
#[allow(clippy::large_enum_variant)]
#[derive(Message, Clone, Debug)]
pub enum NetMessage {
    /// `OnConnectFailure`.
    ConnectFailed { server: Server, error: String },
    /// The server closed the connection (`OnRemoteDisconnect`).
    Disconnected { server: Server },
    Auth(AuthEvent),
    Game(NetEvent),
}

enum Link {
    Idle,
    Connecting(Server, Task<io::Result<TcpTransport>>),
    Online(Server, TcpTransport),
}

/// Both connections. Only one is open at a time: the account connection closes
/// before the game connection opens, as in the C++ client.
#[derive(Resource)]
pub struct Net {
    /// The game connection's state (`CPythonNetworkStream`); use its senders once
    /// logged in.
    pub game: NetMachine,
    auth: AuthMachine,
    link: Link,
    game_addr: (String, u16),
    start: Instant,
    /// The last handshake's server time and the client time it arrived at
    /// (`ELTimer_SetServerMSec`).
    server_clock: Option<(u32, u32)>,
}

impl Default for Net {
    fn default() -> Self {
        Net {
            game: NetMachine::new(NetConfig::default()),
            auth: AuthMachine::new([0; 4]),
            link: Link::Idle,
            game_addr: (String::new(), 0),
            start: Instant::now(),
            server_clock: None,
        }
    }
}

impl Net {
    /// A connection is being made or is open.
    pub fn busy(&self) -> bool {
        !matches!(self.link, Link::Idle)
    }

    /// `ELTimer_GetServerMSec`: the server time at the last handshake plus the
    /// client time since then.
    pub fn server_ms(&self) -> u32 {
        match self.server_clock {
            Some((server, client)) => server.wrapping_add(self.now_ms().wrapping_sub(client)),
            None => self.now_ms(),
        }
    }

    /// `SendCharacterStatePacket` with global coordinates. `func` is
    /// `CInstanceBase::FUNC_MOVE` (1) or `FUNC_WAIT` (0).
    pub fn send_move(&mut self, func: u8, arg: u8, rotation: f32, x: i32, y: i32) -> bool {
        let server_ms = self.server_ms();
        self.game.send_move(func, arg, rotation, x, y, server_ms)
    }

    /// `SendAttackPacket`: 0 is a normal attack, `victim` 0 swings at nothing.
    pub fn send_attack(&mut self, victim: u32) -> bool {
        self.game.send_attack(0, victim)
    }

    /// `SendEnterGame` (`net.SendEnterGamePacket`, sent by the game window when it
    /// opens): the server answers with the Game phase.
    pub fn start_game(&mut self) -> bool {
        self.game.send_enter_game()
    }

    /// `ConnectGameServer`: drop the connection without leaving the phase and connect
    /// to the server that holds `slot`'s character.
    fn enter_game(&mut self, slot: u8) -> Result<(), String> {
        if !matches!(self.link, Link::Online(Server::Game, _)) || self.game.phase() != Phase::Select {
            return Err("not logged in".into());
        }
        let info = self.game.set_direct_enter(slot).ok_or_else(|| format!("no slot {slot}"))?;
        if info.dwID == 0 {
            return Err(format!("slot {slot} is empty"));
        }
        // `CNetworkStream::Connect(DWORD)`: the lowest byte is the first octet.
        let host = std::net::Ipv4Addr::from((info.lAddr as u32).to_le_bytes()).to_string();
        let port = info.wPort;
        info!("entering the game with slot {slot} on {host}:{port}");
        self.link = Link::Idle;
        self.connect(Server::Game, host, port);
        Ok(())
    }

    /// Closes whatever is open (`net.Disconnect`).
    pub fn disconnect(&mut self) {
        self.link = Link::Idle;
        self.auth.on_disconnect();
        self.game.on_disconnect();
    }

    /// `ELTimer_GetMSec`.
    fn now_ms(&self) -> u32 {
        self.start.elapsed().as_millis() as u32
    }

    fn connect(&mut self, server: Server, host: String, port: u16) {
        let task = IoTaskPool::get().spawn(async move {
            let addr: SocketAddr = (host.as_str(), port)
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{host}: no address")))?;
            TcpTransport::connect(addr, CONNECT_TIMEOUT)
        });
        self.link = Link::Connecting(server, task);
    }
}

pub struct NetPlugin;

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerInfo>()
            .init_resource::<Net>()
            .add_message::<LoginRequest>()
            .add_message::<EnterGame>()
            .add_message::<NetMessage>()
            .add_plugins(crate::player::PlayerStatePlugin)
            // Before the UI reads this frame's messages.
            .add_systems(PreUpdate, (start_login, enter_game, pump_net).chain());
    }
}

fn start_login(mut requests: MessageReader<LoginRequest>, mut net: ResMut<Net>, info: Res<ServerInfo>) {
    for request in requests.read() {
        if net.busy() {
            warn!("login request ignored: a connection is open");
            continue;
        }
        let Some(channel) = info.channels.get(request.channel) else {
            warn!("login request ignored: no channel {}", request.channel);
            continue;
        };
        // `__BuildClientKey_20050304Myevan`: a new key per account connection.
        let key = random_client_key();
        let mut auth = AuthMachine::new(key);
        auth.set_login_info(&request.id, &request.password);
        auth.set_sequence_mode(true);
        // The account connector clears the game stream's password after CG_LOGIN3; the
        // game login uses the key.
        let mut game = NetMachine::new(NetConfig { client_key: key, ..NetConfig::default() });
        game.set_login_info(&request.id, "");
        game.set_sequence_mode(true);
        net.auth = auth;
        net.game = game;
        net.game_addr = (info.host.clone(), channel.port);
        info!("connecting to the account server {}:{} for {} ({})", info.host, info.auth_port, request.id, channel.name);
        net.connect(Server::Account, info.host.clone(), info.auth_port);
    }
}

fn enter_game(mut requests: MessageReader<EnterGame>, mut net: ResMut<Net>) {
    for request in requests.read() {
        if let Err(error) = net.enter_game(request.slot) {
            warn!("enter game ignored: {error}");
        }
    }
}

fn pump_net(mut net: ResMut<Net>, mut out: MessageWriter<NetMessage>) {
    let now = net.now_ms();
    let net = &mut *net;
    match &mut net.link {
        Link::Idle => {}
        Link::Connecting(server, task) => {
            let server = *server;
            let Some(result) = check_ready(task) else { return };
            match result {
                Ok(transport) => {
                    info!("connected to the {server:?} server");
                    match server {
                        Server::Account => net.auth.on_connected(),
                        Server::Game => net.game.on_connected(),
                    }
                    net.link = Link::Online(server, transport);
                }
                Err(error) => {
                    warn!("{server:?} server: {error}");
                    net.link = Link::Idle;
                    out.write(NetMessage::ConnectFailed { server, error: error.to_string() });
                }
            }
        }
        Link::Online(Server::Account, transport) => {
            let (events, closed) = pump(&mut net.auth, transport, now);
            let mut login_key = None;
            for event in events {
                if let AuthEvent::Success { login_key: key } = event {
                    login_key = Some(key);
                }
                out.write(NetMessage::Auth(event));
            }
            if let Some(key) = login_key {
                // `__AuthState_RecvAuthSuccess`: `SetLoginKey`, connect the game stream,
                // then drop the account connection.
                net.game.set_login_key(key);
                let (host, port) = net.game_addr.clone();
                info!("login key received; connecting to the game server {host}:{port}");
                net.auth.on_disconnect();
                net.connect(Server::Game, host, port);
            } else if net.auth.finished() {
                net.auth.on_disconnect();
                net.link = Link::Idle;
            } else if closed {
                net.link = Link::Idle;
                out.write(NetMessage::Disconnected { server: Server::Account });
            }
        }
        Link::Online(Server::Game, transport) => {
            let (events, closed) = pump(&mut net.game, transport, now);
            for event in &events {
                match event {
                    NetEvent::Unhandled { phase, header } => warn!("unhandled packet header {header} in {phase:?}; receive buffer cleared"),
                    NetEvent::Frame(error) => warn!("framing error {error:?}; receive buffer cleared"),
                    NetEvent::Malformed { header } => warn!("malformed packet header {header}"),
                    _ => {}
                }
                // `SetSelectPhase` in direct-enter mode opens the loading window, whose
                // first act is `SendSelectCharacterPacket`.
                if let (NetEvent::PhaseChanged { to: Phase::Select, .. }, Some(slot)) = (event, net.game.direct_enter_slot()) {
                    info!("selecting character slot {slot}");
                    net.game.send_select_character(slot);
                }
                if let NetEvent::HandshakeReceived { server_time } = event {
                    net.server_clock = Some((*server_time, now));
                }
            }
            out.write_batch(events.into_iter().map(NetMessage::Game));
            if closed {
                net.link = Link::Idle;
                out.write(NetMessage::Disconnected { server: Server::Game });
            }
        }
    }
}

/// `localeInfo.LOGIN_FAILURE_*` (English `locale_game.txt`) for a failure status, as
/// `intrologin.py`'s `loginFailureMsgDict` maps them.
pub fn login_failure_text(status: &str) -> String {
    match status {
        "ALREADY" => "Your account is already connected.".into(),
        "WRONGCRD" => "Account name or password incorrect.".into(),
        "FULL" => "You could not connect because too many players are online.".into(),
        "SHUTDOWN" => "The server is not open yet.".into(),
        "REPAIR" => "Items have been restored for this account.".into(),
        "BLOCK" => "Your account is banned.".into(),
        "NOTAVAIL" => "Your account cannot be used.".into(),
        "BLKLOGIN" => "Please choose one of the other servers!".into(),
        "WEBBLK" => "You have been blocked at the website. Please check your EMails.".into(),
        // `LOGIN_FAILURE_UNKNOWN + error`.
        other => format!("You could not register for unknown reasons. {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_server_is_the_shipped_serverinfo() {
        let info = ServerInfo::default();
        assert_eq!((info.host.as_str(), info.auth_port), ("127.0.0.1", 11000));
        let ports: Vec<u16> = info.channels.iter().map(|c| c.port).collect();
        assert_eq!(ports, [13000, 13010, 13020, 13030]);
        assert_eq!(info.channels[0].name, "CH1");
    }

    #[test]
    fn failure_codes_read_like_the_original_ui() {
        assert_eq!(login_failure_text("WRONGCRD"), "Account name or password incorrect.");
        assert!(login_failure_text("XYZ").ends_with("XYZ"));
    }
}
