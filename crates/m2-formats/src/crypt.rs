//! TEA and `CLZObject` (`neoeter/src/platform/base/tea.cpp`, `lzo.cpp`): the
//! container of the client's binary tables, an LZO1X stream that may be
//! TEA-encrypted.
//!
//! Layout of a `CLZObject` buffer (all words little-endian):
//!
//! ```text
//! 0   "MCOZ"            THeader::dwFourCC
//! 4   encrypt size      0 when not encrypted
//! 8   compressed size   LZO1X stream length
//! 12  real size         decompressed length
//! 16  "MCOZ" + stream   TEA-encrypted as a whole when encrypt size != 0
//! ```
//!
//! What the C++ calls TEA is XTEA (32 rounds). Only reading is ported; the
//! client never writes these (`CLZO::Compress*` wraps liblzo's lzo1x_999).
//! CRC32.cpp is not ported here: none of the table loaders use it.

use std::fmt;

/// `CLZObject::ms_dwFourCC`, `MAKEFOURCC('M', 'C', 'O', 'Z')`.
pub const LZ_FOURCC: u32 = u32::from_le_bytes(*b"MCOZ");

/// `sizeof(CLZObject::THeader)`.
pub const LZ_HEADER_SIZE: usize = 16;

/// `s_adwItemProtoKey` (`client/src/game/ItemManager.cpp`).
pub const ITEM_PROTO_KEY: [u32; 4] = [173_217, 72_619_434, 408_587_239, 27_973_291];

/// `s_adwMobProtoKey` (`CPythonNonPlayer::LoadNonPlayerData`).
pub const MOB_PROTO_KEY: [u32; 4] = [4_813_894, 18_955, 552_631, 6_822_045];

const TEA_ROUNDS: u32 = 32;
const TEA_DELTA: u32 = 0x9E37_79B9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The buffer is shorter than what its header says.
    Truncated,
    /// `"LZObject: not a valid data"`.
    BadFourCC(u32),
    /// `"LZObject: key incorrect"`: the decrypted stream does not start with
    /// `MCOZ` (or no key was given for an encrypted object).
    BadKey,
    /// The LZO1X stream is malformed (C++: `lzo1x_decompress` != `LZO_E_OK`).
    Lzo(LzoError),
    /// `"LZObject: Size differs"`.
    SizeDiffers { expected: u32, actual: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated => write!(f, "truncated LZ object"),
            Error::BadFourCC(c) => write!(f, "not an LZ object (fourcc {c:08x})"),
            Error::BadKey => write!(f, "key incorrect"),
            Error::Lzo(e) => write!(f, "lzo: {e}"),
            Error::SizeDiffers { expected, actual } => write!(f, "size differs: header {expected}, got {actual}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<LzoError> for Error {
    fn from(e: LzoError) -> Self {
        Error::Lzo(e)
    }
}

/// `tea_code`: encrypts the block `(y, z)`.
pub fn tea_code(mut y: u32, mut z: u32, key: &[u32; 4]) -> (u32, u32) {
    let mut sum: u32 = 0;
    for _ in 0..TEA_ROUNDS {
        y = y.wrapping_add(((z << 4 ^ z >> 5).wrapping_add(z)) ^ sum.wrapping_add(key[(sum & 3) as usize]));
        sum = sum.wrapping_add(TEA_DELTA);
        z = z.wrapping_add(((y << 4 ^ y >> 5).wrapping_add(y)) ^ sum.wrapping_add(key[(sum >> 11 & 3) as usize]));
    }
    (y, z)
}

/// `tea_decode`: decrypts the block `(y, z)`.
pub fn tea_decode(mut y: u32, mut z: u32, key: &[u32; 4]) -> (u32, u32) {
    let mut sum = TEA_DELTA.wrapping_mul(TEA_ROUNDS);
    for _ in 0..TEA_ROUNDS {
        z = z.wrapping_sub(((y << 4 ^ y >> 5).wrapping_add(y)) ^ sum.wrapping_add(key[(sum >> 11 & 3) as usize]));
        sum = sum.wrapping_sub(TEA_DELTA);
        y = y.wrapping_sub(((z << 4 ^ z >> 5).wrapping_add(z)) ^ sum.wrapping_add(key[(sum & 3) as usize]));
    }
    (y, z)
}

fn tea_blocks(src: &[u8], key: &[u32; 4], f: fn(u32, u32, &[u32; 4]) -> (u32, u32)) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len().div_ceil(8) * 8);
    for chunk in src.chunks(8) {
        let mut block = [0u8; 8];
        block[..chunk.len()].copy_from_slice(chunk);
        let (y, z) = f(le32(&block, 0), le32(&block, 4), key);
        out.extend_from_slice(&y.to_le_bytes());
        out.extend_from_slice(&z.to_le_bytes());
    }
    out
}

/// `tea_encrypt`: 8-byte blocks of little-endian words; a partial last block
/// is zero-padded, so the output is `src.len()` rounded up to 8.
pub fn tea_encrypt(src: &[u8], key: &[u32; 4]) -> Vec<u8> {
    tea_blocks(src, key, tea_code)
}

/// `tea_decrypt`. C++ reads a partial last block past the end of `src`;
/// here it is zero-padded instead.
pub fn tea_decrypt(src: &[u8], key: &[u32; 4]) -> Vec<u8> {
    tea_blocks(src, key, tea_decode)
}

/// `CLZObject::THeader`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LzHeader {
    pub fourcc: u32,
    pub encrypt_size: u32,
    pub compressed_size: u32,
    pub real_size: u32,
}

impl LzHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < LZ_HEADER_SIZE {
            return Err(Error::Truncated);
        }
        Ok(Self {
            fourcc: le32(buf, 0),
            encrypt_size: le32(buf, 4),
            compressed_size: le32(buf, 8),
            real_size: le32(buf, 12),
        })
    }
}

/// `CLZO::Decompress` (`BeginDecompress` + `CLZObject::Decompress`): the
/// decompressed contents of the `CLZObject` at the start of `buf`.
///
/// An encrypted object needs `key`; a plain one ignores it. Where C++ reads
/// past its buffers on a corrupt object (sizes larger than the data), this
/// returns [`Error::Truncated`] or an LZO error instead.
pub fn lz_decompress(buf: &[u8], key: Option<&[u32; 4]>) -> Result<Vec<u8>, Error> {
    let header = LzHeader::parse(buf)?;
    if header.fourcc != LZ_FOURCC {
        return Err(Error::BadFourCC(header.fourcc));
    }
    let body = &buf[LZ_HEADER_SIZE..];

    let out = if header.encrypt_size != 0 {
        let key = key.ok_or(Error::BadKey)?;
        // tea_decrypt works on whole blocks, so it reads the size rounded up.
        let encrypted = body
            .get(..(header.encrypt_size as usize).div_ceil(8) * 8)
            .ok_or(Error::Truncated)?;
        let decrypted = tea_decrypt(encrypted, key);
        if le32(&decrypted, 0) != LZ_FOURCC {
            return Err(Error::BadKey);
        }
        let stream = decrypted
            .get(4..4 + header.compressed_size as usize)
            .ok_or(Error::Truncated)?;
        lzo1x_decompress(stream, header.real_size as usize)?
    } else {
        let stream = body
            .get(4..4 + header.compressed_size as usize)
            .ok_or(Error::Truncated)?;
        lzo1x_decompress(stream, header.real_size as usize)?
    };

    if out.len() != header.real_size as usize {
        return Err(Error::SizeDiffers {
            expected: header.real_size,
            actual: out.len(),
        });
    }
    Ok(out)
}

/// Why an LZO1X stream was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LzoError {
    /// `LZO_E_INPUT_OVERRUN`: the stream ends inside an instruction.
    InputOverrun,
    /// The output would exceed the expected size (`LZO_E_OUTPUT_OVERRUN`).
    OutputOverrun,
    /// `LZO_E_LOOKBEHIND_OVERRUN`: a match reaches before the output start.
    LookbehindOverrun,
    /// `LZO_E_INPUT_NOT_CONSUMED`: bytes follow the end-of-stream marker.
    InputNotConsumed,
}

impl fmt::Display for LzoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            LzoError::InputOverrun => "input overrun",
            LzoError::OutputOverrun => "output overrun",
            LzoError::LookbehindOverrun => "lookbehind overrun",
            LzoError::InputNotConsumed => "input not consumed",
        };
        f.write_str(s)
    }
}

impl std::error::Error for LzoError {}

/// Bounds-checked reader over the compressed stream.
struct Input<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl Input<'_> {
    fn byte(&mut self) -> Result<usize, LzoError> {
        let b = *self.buf.get(self.pos).ok_or(LzoError::InputOverrun)?;
        self.pos += 1;
        Ok(usize::from(b))
    }

    /// The run-length extension: each zero byte adds 255, the first non-zero
    /// byte ends it and is added plus `base`.
    fn run(&mut self, base: usize) -> Result<usize, LzoError> {
        let mut t = 0usize;
        loop {
            match self.byte()? {
                0 => t += 255,
                b => return Ok(t + base + b),
            }
        }
    }

    fn literals(&mut self, out: &mut Vec<u8>, n: usize, limit: usize) -> Result<(), LzoError> {
        let src = self.buf.get(self.pos..self.pos + n).ok_or(LzoError::InputOverrun)?;
        if out.len() + n > limit {
            return Err(LzoError::OutputOverrun);
        }
        out.extend_from_slice(src);
        self.pos += n;
        Ok(())
    }
}

/// Copies `len` bytes starting `distance` bytes back, byte by byte so that
/// overlapping matches repeat.
fn copy_match(out: &mut Vec<u8>, distance: usize, len: usize, limit: usize) -> Result<(), LzoError> {
    if distance == 0 || distance > out.len() {
        return Err(LzoError::LookbehindOverrun);
    }
    if out.len() + len > limit {
        return Err(LzoError::OutputOverrun);
    }
    let start = out.len() - distance;
    for i in 0..len {
        out.push(out[start + i]);
    }
    Ok(())
}

/// `lzo1x_decompress`, ported from miniLZO's `lzo1x_d.ch`, with the checks of
/// `lzo1x_decompress_safe`; `max_out` bounds the output. Valid streams decode
/// identically; C++ calls the unchecked variant, which has undefined
/// behaviour on corrupt ones.
pub fn lzo1x_decompress(src: &[u8], max_out: usize) -> Result<Vec<u8>, LzoError> {
    const M2_MAX_OFFSET: usize = 0x0800;

    #[derive(Clone, Copy)]
    enum State {
        /// Top of the loop: a literal run or a match.
        Loop,
        /// After a literal run: a short M1 match is coded differently here.
        FirstLiteralRun,
        /// Decode the match instruction `t`.
        Match(usize),
        /// After a match: `t` trailing literals (from the low bits of the
        /// instruction two bytes back), then the next instruction.
        MatchNext(usize),
    }

    let mut out = Vec::with_capacity(max_out);
    let mut ip = Input { buf: src, pos: 0 };

    let mut state = State::Loop;
    if let Some(&first) = src.first()
        && first > 17
    {
        ip.pos = 1;
        let t = usize::from(first) - 17;
        if t < 4 {
            state = State::MatchNext(t);
        } else {
            ip.literals(&mut out, t, max_out)?;
            state = State::FirstLiteralRun;
        }
    }

    loop {
        state = match state {
            State::Loop => {
                let mut t = ip.byte()?;
                if t >= 16 {
                    State::Match(t)
                } else {
                    if t == 0 {
                        t = ip.run(15)?;
                    }
                    ip.literals(&mut out, t + 3, max_out)?;
                    State::FirstLiteralRun
                }
            }
            State::FirstLiteralRun => {
                let t = ip.byte()?;
                if t >= 16 {
                    State::Match(t)
                } else {
                    let distance = 1 + M2_MAX_OFFSET + (t >> 2) + (ip.byte()? << 2);
                    copy_match(&mut out, distance, 3, max_out)?;
                    State::MatchNext(src[ip.pos - 2] as usize & 3)
                }
            }
            State::Match(t) => {
                if t >= 64 {
                    // M2: 3..8 bytes, distance up to 2 KiB.
                    let distance = 1 + ((t >> 2) & 7) + (ip.byte()? << 3);
                    copy_match(&mut out, distance, (t >> 5) + 1, max_out)?;
                } else if t >= 32 {
                    // M3: distance up to 16 KiB.
                    let mut len = t & 31;
                    if len == 0 {
                        len = ip.run(31)?;
                    }
                    let (b0, b1) = (ip.byte()?, ip.byte()?);
                    copy_match(&mut out, 1 + (b0 >> 2) + (b1 << 6), len + 2, max_out)?;
                } else if t >= 16 {
                    // M4: distance 16..48 KiB, or the end-of-stream marker.
                    let mut len = t & 7;
                    if len == 0 {
                        len = ip.run(7)?;
                    }
                    let (b0, b1) = (ip.byte()?, ip.byte()?);
                    let distance = ((t & 8) << 11) + (b0 >> 2) + (b1 << 6);
                    if distance == 0 {
                        return if ip.pos == src.len() {
                            Ok(out)
                        } else {
                            Err(LzoError::InputNotConsumed)
                        };
                    }
                    copy_match(&mut out, distance + 0x4000, len + 2, max_out)?;
                } else {
                    // M1 after a match's trailing literals: 2 bytes.
                    let distance = 1 + (t >> 2) + (ip.byte()? << 2);
                    copy_match(&mut out, distance, 2, max_out)?;
                }
                State::MatchNext(src[ip.pos - 2] as usize & 3)
            }
            State::MatchNext(0) => State::Loop,
            State::MatchNext(t) => {
                ip.literals(&mut out, t, max_out)?;
                State::Match(ip.byte()?)
            }
        };
    }
}

fn le32(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(buf[at..at + 4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A valid LZO1X stream of literals only (one run, then the end marker).
    fn literal_stream(data: &[u8]) -> Vec<u8> {
        let mut s = Vec::new();
        match data.len() {
            0 => {}
            // A first byte > 17 codes the length; below 4 it is read as a
            // match's trailing literals, which the end marker may follow too.
            n @ 1..=238 => s.push((n + 17) as u8),
            n => {
                s.push(0);
                let mut rest = n - 3 - 15;
                while rest > 255 {
                    s.push(0);
                    rest -= 255;
                }
                s.push(rest as u8);
            }
        }
        s.extend_from_slice(data);
        s.extend_from_slice(&[0x11, 0, 0]);
        s
    }

    #[test]
    fn tea_round_trip() {
        let key = [1, 2, 3, 4];
        let data: Vec<u8> = (0..37u8).collect();
        let enc = tea_encrypt(&data, &key);
        assert_eq!(enc.len(), 40);
        let dec = tea_decrypt(&enc, &key);
        assert_eq!(&dec[..37], &data[..]);
        assert_eq!(&dec[37..], &[0, 0, 0]);
    }

    #[test]
    fn tea_known_vector() {
        // tea.cpp is really XTEA (key[sum & 3], key[sum >> 11 & 3]): the XTEA
        // reference vector for key 0, plaintext 0, 32 rounds.
        assert_eq!(tea_code(0, 0, &[0; 4]), (0xdee9_d4d8, 0xf713_1ed9));
        assert_eq!(tea_decode(0xdee9_d4d8, 0xf713_1ed9, &[0; 4]), (0, 0));
    }

    #[test]
    fn literal_runs() {
        for len in [0usize, 1, 2, 3, 4, 17, 18, 19, 238, 239, 300, 1000] {
            let data: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();
            assert_eq!(
                lzo1x_decompress(&literal_stream(&data), len).unwrap(),
                data,
                "len {len}"
            );
        }
    }

    #[test]
    fn overlapping_match() {
        // Literals "abcd", then an M3 match (t = 32 | len - 2) of 6 bytes at
        // distance 2 ((b0 >> 2) + (b1 << 6) = distance - 1).
        let mut s = vec![17 + 4];
        s.extend_from_slice(b"abcd");
        // M3, len 6 (t&31 = 4), distance 2: (b0>>2)+(b1<<6) = 1.
        s.extend_from_slice(&[32 | 4, 1 << 2, 0]);
        s.extend_from_slice(&[0x11, 0, 0]);
        assert_eq!(lzo1x_decompress(&s, 100).unwrap(), b"abcdcdcdcd");
    }

    #[test]
    fn corrupt_streams() {
        assert_eq!(lzo1x_decompress(&[], 10), Err(LzoError::InputOverrun));
        // Literal run longer than the input.
        assert_eq!(lzo1x_decompress(&[17 + 10, b'a'], 100), Err(LzoError::InputOverrun));
        // Output limit.
        assert_eq!(
            lzo1x_decompress(&literal_stream(b"abcdef"), 5),
            Err(LzoError::OutputOverrun)
        );
        // Match before the start.
        let mut s = vec![17 + 4];
        s.extend_from_slice(b"abcd");
        s.extend_from_slice(&[32 | 4, 10 << 2, 0, 0x11, 0, 0]);
        assert_eq!(lzo1x_decompress(&s, 100), Err(LzoError::LookbehindOverrun));
        // Trailing garbage.
        let mut s = literal_stream(b"abcdef");
        s.push(0);
        assert_eq!(lzo1x_decompress(&s, 100), Err(LzoError::InputNotConsumed));
    }

    fn lz_object(data: &[u8], key: Option<&[u32; 4]>) -> Vec<u8> {
        let stream = literal_stream(data);
        let mut body = LZ_FOURCC.to_le_bytes().to_vec();
        body.extend_from_slice(&stream);
        let encrypt_size = match key {
            // CLZObject::Encrypt encrypts compressed size + 19 bytes.
            Some(key) => {
                body.resize(stream.len() + 19, 0);
                body = tea_encrypt(&body, key);
                body.len() as u32
            }
            None => 0,
        };
        let mut buf = Vec::new();
        for word in [LZ_FOURCC, encrypt_size, stream.len() as u32, data.len() as u32] {
            buf.extend_from_slice(&word.to_le_bytes());
        }
        buf.extend_from_slice(&body);
        buf
    }

    #[test]
    fn lz_objects() {
        let data = b"the quick brown fox jumps over the lazy dog";
        assert_eq!(lz_decompress(&lz_object(data, None), None).unwrap(), data);
        let enc = lz_object(data, Some(&ITEM_PROTO_KEY));
        assert_eq!(lz_decompress(&enc, Some(&ITEM_PROTO_KEY)).unwrap(), data);
        assert_eq!(lz_decompress(&enc, Some(&MOB_PROTO_KEY)), Err(Error::BadKey));
        assert_eq!(lz_decompress(&enc, None), Err(Error::BadKey));
        assert_eq!(lz_decompress(&enc[..20], Some(&ITEM_PROTO_KEY)), Err(Error::Truncated));

        let mut bad = lz_object(data, None);
        bad[0] = b'X';
        assert!(matches!(lz_decompress(&bad, None), Err(Error::BadFourCC(_))));

        // Header real size disagreeing with the stream.
        let mut bad = lz_object(data, None);
        bad[12..16].copy_from_slice(&100u32.to_le_bytes());
        assert_eq!(
            lz_decompress(&bad, None),
            Err(Error::SizeDiffers {
                expected: 100,
                actual: data.len()
            })
        );
    }
}
