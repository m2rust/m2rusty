//! Headless login against a real server: the account server gives a login key, the
//! game server takes it and answers with the character list.
//!
//! ```text
//! cargo run -p m2-net --example login -- [host] [auth port] [game port] [id] [password]
//! ```
//!
//! Defaults: `127.0.0.1 11000 13000 admin admin`. Sequence mode is on, as the shipped
//! scripts set it (`constInfo.SEQUENCE_PACKET_ENABLE`).

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use m2_net::auth::{AuthEvent, AuthMachine};
use m2_net::phase::{NetConfig, NetEvent, NetMachine};
use m2_net::transport::pump;
use m2_net::transport::tcp::TcpTransport;

const TIMEOUT: Duration = Duration::from_secs(10);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize, default: &str| args.get(i).cloned().unwrap_or_else(|| default.to_owned());
    let host = arg(0, "127.0.0.1");
    let auth_port: u16 = arg(1, "11000").parse().expect("auth port");
    let game_port: u16 = arg(2, "13000").parse().expect("game port");
    let (id, password) = (arg(3, "admin"), arg(4, "admin"));
    if let Err(e) = run(&host, auth_port, game_port, &id, &password) {
        eprintln!("login failed: {e}");
        std::process::exit(1);
    }
}

fn addr(host: &str, port: u16) -> Result<SocketAddr, String> {
    (host, port).to_socket_addrs().map_err(|e| format!("{host}:{port}: {e}"))?.next().ok_or(format!("{host}: no address"))
}

fn run(host: &str, auth_port: u16, game_port: u16, id: &str, password: &str) -> Result<(), String> {
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
    loop {
        let (events, closed) = pump(&mut game, &mut transport, now());
        for event in events {
            match event {
                NetEvent::LoginSuccess { players, .. } => {
                    println!("login OK, empire {}", game.empire());
                    for (slot, p) in players.iter().enumerate().filter(|(_, p)| p.dwID != 0) {
                        println!("  slot {slot}: {} (job {}, level {})", m2_net::wire::cstr_to_string(&p.szName), p.byJob, p.byLevel);
                    }
                    return Ok(());
                }
                NetEvent::LoginFailure { status } => return Err(status),
                other => println!("game: {other:?}"),
            }
        }
        if closed {
            return Err("the game server closed the connection".into());
        }
        if start.elapsed() > TIMEOUT {
            return Err("timeout on the game server".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
