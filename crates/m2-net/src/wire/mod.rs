//! Wire layer: explicit little-endian codecs for every packet struct of
//! `client/include/Client/UI/Packet.h` (all `#pragma pack(1)`).
//!
//! The struct definitions, header constants and the header map are generated
//! (`tools/differential/net/gen_packets.pl`, output in `generated.rs`); this file
//! holds the codec traits they implement. Nothing here transmutes or borrows
//! from the byte buffer.
//!
//! Platform note: `time_t` fields (`TPlayerSkill::tNextRead`, `TPacketGCTime::time`)
//! are `i64` because the C++ Linux build this port is checked against has a
//! 64-bit `time_t`; the original Win32 client had 32 bits there (open point:
//! which one the target server speaks).

use std::fmt;

#[rustfmt::skip]
mod generated;
pub use generated::*;

pub mod tails;

/// Decode failures. The C++ never validates; the port never panics (D7/D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Fewer bytes than the struct needs.
    Truncated,
    /// Header byte not in the header map.
    UnknownHeader(u8),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("packet truncated"),
            DecodeError::UnknownHeader(h) => write!(f, "unknown packet header {h}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Little-endian output buffer.
#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn put(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }
}

/// Little-endian cursor over a byte slice.
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.pos.checked_add(n).ok_or(DecodeError::Truncated)?;
        let s = self.buf.get(self.pos..end).ok_or(DecodeError::Truncated)?;
        self.pos = end;
        Ok(s)
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
}

/// A fixed-size little-endian wire type.
pub trait Wire: Sized {
    /// Encoded size in bytes (`sizeof` of the packed C++ type).
    const SIZE: usize;
    fn write(&self, w: &mut Writer);
    fn read(r: &mut Reader) -> Result<Self, DecodeError>;
    /// All-zero value (the C++ `memset(&packet, 0, sizeof)` idiom).
    fn zeroed() -> Self;

    /// Encodes into a new buffer of exactly `SIZE` bytes.
    fn encode(&self) -> Vec<u8> {
        let mut w = Writer { buf: Vec::with_capacity(Self::SIZE) };
        self.write(&mut w);
        debug_assert_eq!(w.buf.len(), Self::SIZE);
        w.buf
    }

    /// Decodes from the front of `buf`; trailing bytes are ignored.
    fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        Self::read(&mut Reader::new(buf))
    }
}

macro_rules! wire_int {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            const SIZE: usize = std::mem::size_of::<$t>();
            fn write(&self, w: &mut Writer) {
                w.put(&self.to_le_bytes());
            }
            fn read(r: &mut Reader) -> Result<Self, DecodeError> {
                let b = r.take(Self::SIZE)?;
                Ok(<$t>::from_le_bytes(b.try_into().map_err(|_| DecodeError::Truncated)?))
            }
            fn zeroed() -> Self {
                0 as $t
            }
        }
    )*};
}
wire_int!(u8, u16, u32, i16, i32, i64, f32);

/// C++ `bool` (one byte; any non-zero value is true).
impl Wire for bool {
    const SIZE: usize = 1;
    fn write(&self, w: &mut Writer) {
        w.put(&[*self as u8]);
    }
    fn read(r: &mut Reader) -> Result<Self, DecodeError> {
        Ok(r.take(1)?[0] != 0)
    }
    fn zeroed() -> Self {
        false
    }
}

impl<T: Wire, const N: usize> Wire for [T; N] {
    const SIZE: usize = T::SIZE * N;
    fn write(&self, w: &mut Writer) {
        for x in self {
            x.write(w);
        }
    }
    fn read(r: &mut Reader) -> Result<Self, DecodeError> {
        // Check once so a truncated 8 KiB array does not read element by element.
        if r.remaining() < Self::SIZE {
            return Err(DecodeError::Truncated);
        }
        let mut v = Vec::with_capacity(N);
        for _ in 0..N {
            v.push(T::read(r)?);
        }
        v.try_into().map_err(|_| DecodeError::Truncated)
    }
    fn zeroed() -> Self {
        std::array::from_fn(|_| T::zeroed())
    }
}

/// `char name[N]` as UTF-8: up to the first NUL; legacy code pages are not
/// converted (D14: lossy for now).
pub fn cstr_to_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// `strncpy(dst, s, sizeof(dst) - 1)` followed by a zero fill: truncates at a
/// UTF-8 boundary so a multi-byte character is never split.
pub fn set_cstr(dst: &mut [u8], s: &str) {
    dst.fill(0);
    if dst.is_empty() {
        return;
    }
    let mut n = s.len().min(dst.len() - 1);
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    dst[..n].copy_from_slice(&s.as_bytes()[..n]);
}
