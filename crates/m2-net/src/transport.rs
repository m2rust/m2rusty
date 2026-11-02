//! The byte pipe under the state machine. Everything above is sans-IO; a platform
//! provides a [`Transport`]: `std::net` ([`tcp`], feature `tcp`), a WebSocket on wasm
//! later, or the in-memory pair used by the tests.

use crate::auth::{AuthEvent, AuthMachine};
use crate::phase::{NetEvent, NetMachine};
use crate::stream::NetStream;
use std::io;

/// Connection state, as `CNetworkStream::IsOnline()` plus the connecting interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnState {
    Connecting,
    Online,
    Closed,
}

/// A non-blocking byte pipe. Methods never block.
pub trait Transport {
    fn state(&mut self) -> ConnState;
    /// Reads available bytes. `Ok(0)` means nothing is available right now
    /// (`WSAEWOULDBLOCK`); an error means the connection is gone (`recv` returned 0 or
    /// failed: `OnRemoteDisconnect`).
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    /// Writes as many bytes as the pipe takes; `Ok(0)` means it is full for now.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;
    fn close(&mut self);
}

/// A state machine over a [`NetStream`] that [`pump`] can drive: the game stream
/// ([`NetMachine`]) or the account connection ([`AuthMachine`]).
pub trait Machine {
    type Event;
    fn stream(&mut self) -> &mut NetStream;
    /// Dispatches every complete packet.
    fn poll(&mut self, now_ms: u32) -> Vec<Self::Event>;
    fn on_disconnect(&mut self);
}

impl Machine for NetMachine {
    type Event = NetEvent;
    fn stream(&mut self) -> &mut NetStream {
        &mut self.stream
    }
    fn poll(&mut self, now_ms: u32) -> Vec<NetEvent> {
        NetMachine::poll(self, now_ms)
    }
    fn on_disconnect(&mut self) {
        NetMachine::on_disconnect(self)
    }
}

impl Machine for AuthMachine {
    type Event = AuthEvent;
    fn stream(&mut self) -> &mut NetStream {
        &mut self.stream
    }
    fn poll(&mut self, _now_ms: u32) -> Vec<AuthEvent> {
        AuthMachine::poll(self)
    }
    fn on_disconnect(&mut self) {
        AuthMachine::on_disconnect(self)
    }
}

/// `CNetworkStream::Process` for one machine: reads into the machine, dispatches its
/// packets and writes its queued output. Returns the events and whether the transport
/// closed (the machine is then `Offline`, like `OnRemoteDisconnect` + `Clear`).
pub fn pump<M: Machine, T: Transport>(machine: &mut M, transport: &mut T, now_ms: u32) -> (Vec<M::Event>, bool) {
    let mut closed = false;
    let mut events = Vec::new();
    if transport.state() == ConnState::Closed {
        closed = true;
    } else if transport.state() == ConnState::Online {
        let mut buf = [0u8; 8192];
        loop {
            match transport.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let mut off = 0;
                    while off < n {
                        let taken = machine.stream().feed(&buf[off..n]);
                        if taken == 0 {
                            // Dispatch complete frames before feeding the rest of
                            // this socket read. Dropping that remainder would put
                            // the next read in the middle of a packet.
                            let before = machine.stream().recv_len();
                            let batch = machine.poll(now_ms);
                            events.extend(batch);
                            if machine.stream().recv_len() >= before {
                                // A single incomplete packet cannot fit at all.
                                closed = true;
                                break;
                            }
                            continue;
                        }
                        off += taken;
                    }
                    if off < n {
                        break;
                    }
                }
                Err(_) => {
                    closed = true;
                    break;
                }
            }
        }
    }
    let batch = machine.poll(now_ms);
    events.extend(batch);
    if !closed && transport.state() == ConnState::Online {
        loop {
            let out = machine.stream().take_output(8192);
            if out.is_empty() {
                break;
            }
            match transport.write(&out) {
                Ok(n) if n == out.len() => {}
                Ok(n) => {
                    // Partial write: push the remainder back to the front is not possible, so
                    // keep it simple and write the rest in the next call.
                    machine.stream().requeue_front(&out[n..]);
                    break;
                }
                Err(_) => {
                    closed = true;
                    break;
                }
            }
        }
    }
    if closed {
        transport.close();
        machine.on_disconnect();
    }
    (events, closed)
}

/// In-memory transport pair for tests.
pub mod mem {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;

    #[derive(Default)]
    struct Pipe {
        a_to_b: VecDeque<u8>,
        b_to_a: VecDeque<u8>,
        closed: bool,
    }

    /// One end of the pair; `max_read` / `max_write` model short reads and writes.
    pub struct MemTransport {
        pipe: Rc<RefCell<Pipe>>,
        is_a: bool,
        pub max_read: usize,
        pub max_write: usize,
    }

    /// Returns `(client, server)`.
    pub fn pair() -> (MemTransport, MemTransport) {
        let pipe = Rc::new(RefCell::new(Pipe::default()));
        (
            MemTransport { pipe: pipe.clone(), is_a: true, max_read: usize::MAX, max_write: usize::MAX },
            MemTransport { pipe, is_a: false, max_read: usize::MAX, max_write: usize::MAX },
        )
    }

    impl Transport for MemTransport {
        fn state(&mut self) -> ConnState {
            if self.pipe.borrow().closed { ConnState::Closed } else { ConnState::Online }
        }
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let mut p = self.pipe.borrow_mut();
            let q = if self.is_a { &mut p.b_to_a } else { &mut p.a_to_b };
            let n = buf.len().min(q.len()).min(self.max_read);
            for b in buf.iter_mut().take(n) {
                *b = q.pop_front().unwrap();
            }
            Ok(n)
        }
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let mut p = self.pipe.borrow_mut();
            let n = buf.len().min(self.max_write);
            let q = if self.is_a { &mut p.a_to_b } else { &mut p.b_to_a };
            q.extend(&buf[..n]);
            Ok(n)
        }
        fn close(&mut self) {
            self.pipe.borrow_mut().closed = true;
        }
    }
}

/// `std::net` adapter (feature `tcp`): the only module that touches sockets.
#[cfg(feature = "tcp")]
pub mod tcp {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    /// A connected, non-blocking `TcpStream`.
    pub struct TcpTransport {
        stream: Option<TcpStream>,
    }

    impl TcpTransport {
        /// Connects (this one call may block up to `timeout`; `std` has no non-blocking
        /// connect; the C++ polls with a 3 second limit), then switches to non-blocking.
        pub fn connect(addr: SocketAddr, timeout: Duration) -> io::Result<Self> {
            let s = TcpStream::connect_timeout(&addr, timeout)?;
            s.set_nonblocking(true)?;
            s.set_nodelay(true).ok();
            Ok(TcpTransport { stream: Some(s) })
        }
    }

    impl Transport for TcpTransport {
        fn state(&mut self) -> ConnState {
            if self.stream.is_some() { ConnState::Online } else { ConnState::Closed }
        }
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let s = self.stream.as_mut().ok_or(io::ErrorKind::NotConnected)?;
            match s.read(buf) {
                Ok(0) => Err(io::ErrorKind::ConnectionAborted.into()),
                Ok(n) => Ok(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(e),
            }
        }
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let s = self.stream.as_mut().ok_or(io::ErrorKind::NotConnected)?;
            match s.write(buf) {
                Ok(n) => Ok(n),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(e),
            }
        }
        fn close(&mut self) {
            self.stream = None;
        }
    }
}
