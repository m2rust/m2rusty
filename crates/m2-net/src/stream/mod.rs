//! Sans-IO port of `CNetworkStream`'s buffers (`neoeter/src/render/NetStream.cpp`)
//! plus `CPythonNetworkStream::CheckPacket` (`client/src/ui/PythonNetworkStream.cpp`).
//!
//! Bytes from the socket go in with [`NetStream::feed`]; complete packets come out of
//! [`NetStream::poll_frame`]; bytes to write are queued with [`NetStream::send`] and
//! taken with [`NetStream::take_output`].
//!
//! There is no stream cipher: the C++ `Send` has its TEA encryption commented out and
//! `Recv` never decrypts (the TEA code in the client only protects pack/proto files,
//! see `m2_formats::crypt`). The only per-packet transformation is the *sequence
//! byte*: when `SetPacketSequenceMode(true)` was called (the Python `net` module does
//! it after login), every client packet is followed by one byte from a fixed
//! 32768-entry table ([`NetStream::send_sequence`]).

mod seq_table;

use crate::wire::{self, GC_HEADER_MAP, HeaderInfo};

pub use seq_table::SEQUENCE_TABLE;

/// `SEQUENCE_TABLE_SIZE`.
pub const SEQUENCE_TABLE_SIZE: usize = 32768;

/// Size of `TDynamicSizePacketHeader` (header byte + u16 size).
pub const DYNAMIC_HEADER_SIZE: usize = 3;

/// `CNetworkPacketHeaderMap::Get` for the main stream's map.
pub fn header_info(header: u8) -> Option<&'static HeaderInfo> {
    GC_HEADER_MAP.iter().find(|h| h.header == header)
}

/// Headers whose C++ handler reads more bytes than the header map announces (each is a
/// C++ inconsistency; see the crate docs). The C++ `CheckPacket` only waits for the map
/// size and the handler's `Recv` then fails (and the whole buffer is dropped) if the rest
/// has not arrived yet; the port waits for the handler's size instead.
///
/// `HEADER_GC_PLAYER_DELETE_SUCCESS` is mapped to `TPacketGCBlank` but read as
/// `TPacketGCDestroyCharacterSuccess`; `SAFEBOX_SET`/`MALL_SET` are mapped to
/// `TPacketGCItemSet` but read as `TPacketGCItemSet2`.
///
/// `HEADER_GC_WHISPER` is mapped as a static packet but carries its length in `wSize` and
/// a text tail (`RecvWhisperPacket`); the stream frames it as a dynamic packet.
pub fn handler_size_override(header: u8) -> Option<usize> {
    use crate::wire::{TPacketGCDestroyCharacterSuccess, TPacketGCItemSet2, gc};
    match header {
        gc::PLAYER_DELETE_SUCCESS => Some(TPacketGCDestroyCharacterSuccess::SIZE),
        gc::SAFEBOX_SET | gc::MALL_SET => Some(TPacketGCItemSet2::SIZE),
        _ => None,
    }
}

/// One complete packet: the header byte followed by the body (for dynamic packets the
/// whole frame including the 3-byte dynamic header).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub header: u8,
    pub bytes: Vec<u8>,
}

/// Framing errors. The C++ answers all of them with `ClearRecvBuffer()` (and, for an
/// unknown header, `PostQuitMessage`); this stream clears its buffer too and reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// "Unknown packet header" - not in the header map.
    UnknownHeader(u8),
    /// A dynamic packet whose size field is smaller than its 3-byte header (C++: the
    /// handler would read past the frame), or larger than the receive buffer (C++:
    /// waits forever).
    BadDynamicSize { header: u8, size: usize },
}

/// Port of the byte-level state of `CNetworkStream`.
pub struct NetStream {
    recv: Vec<u8>,
    recv_pos: usize,
    recv_cap: usize,
    send: Vec<u8>,
    send_cap: usize,
    use_sequence: bool,
    sequence: usize,
}

impl NetStream {
    /// `SetRecvBufferSize` / `SetSendBufferSize` take their values from the Python
    /// scripts; 128 KiB / 64 KiB are the `AccountConnector` style defaults.
    pub fn new(recv_cap: usize, send_cap: usize) -> Self {
        NetStream {
            recv: Vec::new(),
            recv_pos: 0,
            recv_cap,
            send: Vec::new(),
            send_cap,
            use_sequence: false,
            sequence: 0,
        }
    }

    /// `Clear()` (on disconnect): drops both buffers and restarts the sequence.
    pub fn clear(&mut self) {
        self.recv.clear();
        self.recv_pos = 0;
        self.send.clear();
        self.sequence = 0;
    }

    /// `ClearRecvBuffer()`.
    pub fn clear_recv(&mut self) {
        self.recv.clear();
        self.recv_pos = 0;
    }

    /// `GetRecvBufferSize()`: bytes buffered and not yet consumed.
    pub fn recv_len(&self) -> usize {
        self.recv.len() - self.recv_pos
    }

    /// `SetPacketSequenceMode`.
    pub fn set_sequence_mode(&mut self, on: bool) {
        self.use_sequence = on;
    }

    /// Input side of `__RecvInternalBuffer`: appends as much of `data` as fits in the
    /// receive buffer and returns the number of bytes taken (like `recv()` into the
    /// remaining space; the caller keeps the rest).
    pub fn feed(&mut self, data: &[u8]) -> usize {
        // The C++ compacts (memmove) before every recv.
        if self.recv_pos > 0 {
            self.recv.drain(..self.recv_pos);
            self.recv_pos = 0;
        }
        let room = self.recv_cap.saturating_sub(self.recv.len());
        let n = room.min(data.len());
        self.recv.extend_from_slice(&data[..n]);
        n
    }

    /// `Peek(size)`.
    fn peek(&self, size: usize) -> bool {
        self.recv_len() >= size
    }

    /// `CheckPacket` + `Recv`: returns the next complete packet, or `None` when more
    /// bytes are needed. Leading zero bytes are skipped like the C++ does.
    pub fn poll_frame(&mut self) -> Result<Option<Frame>, FrameError> {
        // Zero headers are consumed (the C++ `Recv`s them inside CheckPacket).
        while self.recv_pos < self.recv.len() && self.recv[self.recv_pos] == 0 {
            self.recv_pos += 1;
        }
        if !self.peek(1) {
            return Ok(None);
        }
        let header = self.recv[self.recv_pos];
        let Some(info) = header_info(header) else {
            self.clear_recv();
            return Err(FrameError::UnknownHeader(header));
        };
        let size = if info.dynamic || header == crate::wire::gc::WHISPER {
            if !self.peek(DYNAMIC_HEADER_SIZE) {
                return Ok(None);
            }
            let p = self.recv_pos;
            let size = u16::from_le_bytes([self.recv[p + 1], self.recv[p + 2]]) as usize;
            let min = if info.dynamic { DYNAMIC_HEADER_SIZE } else { info.size };
            if size < min || size > self.recv_cap {
                self.clear_recv();
                return Err(FrameError::BadDynamicSize { header, size });
            }
            size
        } else {
            handler_size_override(header).unwrap_or(info.size)
        };
        if !self.peek(size) {
            return Ok(None);
        }
        let bytes = self.recv[self.recv_pos..self.recv_pos + size].to_vec();
        self.recv_pos += size;
        Ok(Some(Frame { header, bytes }))
    }

    /// `Send(size, buf)`: fails (false) when the send buffer cannot hold the data plus
    /// one spare byte, like the C++ (`(size + 1) > rest`).
    pub fn send(&mut self, data: &[u8]) -> bool {
        if data.len() + 1 > self.send_cap.saturating_sub(self.send.len()) {
            return false;
        }
        self.send.extend_from_slice(data);
        true
    }

    /// `SendSequence()`: appends the next sequence byte when sequence mode is on.
    pub fn send_sequence(&mut self) -> bool {
        if !self.use_sequence {
            return true;
        }
        let b = SEQUENCE_TABLE[self.sequence];
        self.sequence += 1;
        let ok = self.send(&[b]);
        if self.sequence == SEQUENCE_TABLE_SIZE {
            self.sequence = 0;
        }
        ok
    }

    /// Encodes `packet` and queues it followed by the sequence byte (the pattern
    /// `Send(...); SendSequence();` of every C++ sender).
    pub fn send_packet<P: wire::Wire>(&mut self, packet: &P) -> bool {
        self.send(&packet.encode()) && self.send_sequence()
    }

    /// Bytes queued for the socket.
    pub fn pending_output(&self) -> usize {
        self.send.len()
    }

    /// Puts bytes a transport did not accept back at the front of the queue.
    pub fn requeue_front(&mut self, data: &[u8]) {
        self.send.splice(0..0, data.iter().copied());
    }

    /// Takes up to `max` queued bytes (a partial `send()` leaves the rest queued).
    pub fn take_output(&mut self, max: usize) -> Vec<u8> {
        let n = max.min(self.send.len());
        self.send.drain(..n).collect()
    }
}
