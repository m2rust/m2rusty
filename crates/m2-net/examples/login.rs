//! Headless login against a real server: the account server gives a login key, the
//! game server takes it and answers with the character list. With an `enter` slot it
//! then enters the game the way the client does (reconnect in direct-enter mode,
//! select the character, `ENTERGAME`) and stays connected for 20 s, counting what
//! the server sends.
//!
//! ```text
//! cargo run -p m2-net --example login -- [host] [auth port] [game port] [id] [password] [enter slot]
//! ```
//!
//! Defaults: `127.0.0.1 11000 13000 admin admin`. Sequence mode is on, as the shipped
//! scripts set it (`constInfo.SEQUENCE_PACKET_ENABLE`).

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use m2_net::auth::{AuthEvent, AuthMachine};
use m2_net::phase::{NetConfig, NetEvent, NetMachine, Phase};
use m2_net::transport::pump;
use m2_net::transport::tcp::TcpTransport;

const TIMEOUT: Duration = Duration::from_secs(10);
/// How long to stay in the game with an `enter` slot.
const STAY: Duration = Duration::from_secs(20);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, default: &str| args.get(i).cloned().unwrap_or_else(|| default.to_owned());
    let host = arg(0, "127.0.0.1");
    let auth_port: u16 = arg(1, "11000").parse().expect("auth port");
    let game_port: u16 = arg(2, "13000").parse().expect("game port");
    let (id, password) = (arg(3, "admin"), arg(4, "admin"));
    let enter = args.get(5).map(|s| s.parse::<u8>().expect("enter slot"));
    if let Err(e) = run(&host, auth_port, game_port, &id, &password, enter) {
        eprintln!("login failed: {e}");
        std::process::exit(1);
    }
}

fn addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port).to_socket_addrs().map_err(|e| format!("{host}:{port}: {e}"))?.next().ok_or(format!("{host}: no address"))
}

fn run(host: &str, auth_port: u16, game_port: u16, id: &str, password: &str, enter: Option<u8>) -> Result<(), String> {
    let start = Instant::now();
    let now = || start.elapsed().as_millis() as u32;
    let client_key = m2_net::auth::random_client_key();

    let mut auth = AuthMachine::new(client_key);
    auth.set_login_info(id, password);
    auth.set_sequence_mode(true);
    let mut transport = TcpTransport::connect(addr(host, auth_port)?, Duration::from_secs(3)).map_err(|e| format!("auth server: {e}"))?;
    auth.on_connected();
    println!("connected to the account server {host}:{auth_port}");
    let login_key = 'auth: loop {
        let (events, closed) = pump(&mut auth, &mut transport, now());
        for event in events {
            println!("auth: {event:?}");
            match event {
                AuthEvent::Success { login_key } => break 'auth login_key,
                AuthEvent::Failure { status } => return Err(status),
                _ => {}
            }
        }
        if closed {
            return Err("the account server closed the connection".into());
        }
        if start.elapsed() > TIMEOUT {
            return Err("timeout on the account server".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    drop(transport);

    let mut game = NetMachine::new(NetConfig { client_key, ..NetConfig::default() });
    game.set_login_info(id, "");
    game.set_login_key(login_key);
    game.set_sequence_mode(true);
    let mut transport = TcpTransport::connect(addr(host, game_port)?, Duration::from_secs(3)).map_err(|e| format!("game server: {e}"))?;
    game.on_connected();
    println!("connected to the game server {host}:{game_port}");
    let mut enter_slot = enter;
    let mut in_game_since: Option<Instant> = None;
    let mut counts: std::collections::BTreeMap<String, usize> = Default::default();
    loop {
        let (events, closed) = pump(&mut game, &mut transport, now());
        let mut reconnect = None;
        for event in events {
            if in_game_since.is_some() {
                let kind = format!("{event:?}");
                let kind = kind.split(['(', ' ', '{']).next().unwrap_or("").to_owned();
                let kind = match &event {
                    NetEvent::Packet(p) => format!("Packet({})", format!("{p:?}").split('(').next().unwrap_or("")),
                    NetEvent::Unhandled { header, .. } => format!("Unhandled({header})"),
                    _ => kind,
                };
                *counts.entry(kind).or_default() += 1;
                continue;
            }
            match event {
                NetEvent::LoginSuccess { players, .. } if game.direct_enter_slot().is_none() => {
                    println!("login OK, empire {}", game.empire());
                    for (slot, p) in players.iter().enumerate().filter(|(_, p)| p.dwID != 0) {
                        let addr = std::net::Ipv4Addr::from((p.lAddr as u32).to_le_bytes());
                        println!(
                            "  slot {slot}: {} (race {}, level {}, armor {}, hair {}, at {},{}, server {addr}:{})",
                            m2_net::wire::cstr_to_string(&p.szName),
                            p.byJob,
                            p.byLevel,
                            p.wMainPart,
                            p.wHairPart,
                            p.x,
                            p.y,
                            p.wPort
                        );
                    }
                    let Some(slot) = enter_slot.take() else { return Ok(()) };
                    // `net.DirectEnter`: reconnect to the slot's server in direct-enter mode.
                    let info = game.set_direct_enter(slot).ok_or(format!("no slot {slot}"))?;
                    let host = std::net::Ipv4Addr::from((info.lAddr as u32).to_le_bytes()).to_string();
                    reconnect = Some((host, info.wPort));
                }
                NetEvent::PhaseChanged { to: Phase::Select, .. } if game.direct_enter_slot().is_some() => {
                    let slot = game.direct_enter_slot().unwrap_or(0);
                    println!("selecting slot {slot}");
                    game.send_select_character(slot);
                }
                NetEvent::MainCharacter(main) => {
                    println!("main character {} (vid {}) at {},{}; sending ENTERGAME", main.name, main.vid, main.x, main.y);
                    game.send_enter_game();
                }
                NetEvent::PhaseChanged { to: Phase::Game, .. } => {
                    println!("in game; staying {} s", STAY.as_secs());
                    in_game_since = Some(Instant::now());
                }
                NetEvent::LoginFailure { status } => return Err(status),
                other => println!("game: {other:?}"),
            }
        }
        if let Some((host, port)) = reconnect {
            println!("entering the game: reconnecting to {host}:{port}");
            transport = TcpTransport::connect(addr(&host, port)?, Duration::from_secs(3)).map_err(|e| format!("game server: {e}"))?;
            game.on_connected();
            continue;
        }
        if closed {
            return Err("the game server closed the connection".into());
        }
        if let Some(since) = in_game_since {
            if since.elapsed() > STAY {
                println!("still connected after {} s; packets received in game:", STAY.as_secs());
                for (kind, count) in &counts {
                    println!("  {count:6}  {kind}");
                }
                return Ok(());
            }
        } else if start.elapsed() > TIMEOUT {
            return Err("timeout on the game server".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
