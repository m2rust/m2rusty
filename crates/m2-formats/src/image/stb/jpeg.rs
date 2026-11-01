//! stb_image's JPEG decoder (`stbi__jpeg_*`, `load_jpeg_image`): baseline and
//! progressive Huffman JPEG, 1/3/4 components, restart intervals, JFIF/Adobe
//! colour handling, and stb's own integer IDCT, upsampling and YCbCr
//! conversion. Ported from the scalar code; the SSE2 kernels the client build
//! uses are documented by stb as bit-identical.
//!
//! Corrupt files behave as in stb: a damaged marker after the first scan ends
//! the decode with what has been decoded so far (the rest of the image is 0
//! here, uninitialised memory in C).

use super::{Ctx, Info, MAX_DIMENSIONS, Rgba, mad3sizes_valid};

const FAST_BITS: usize = 9;

const MARKER_NONE: u8 = 0xff;

const DEZIGZAG: [u8; 64 + 15] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54,
    47, 55, 62, 63, // let corrupt input sample past end
    63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
];

const BMASK: [u32; 17] = [0, 1, 3, 7, 15, 31, 63, 127, 255, 511, 1023, 2047, 4095, 8191, 16383, 32767, 65535];
const JBIAS: [i32; 16] = [0, -1, -3, -7, -15, -31, -63, -127, -255, -511, -1023, -2047, -4095, -8191, -16383, -32767];

#[derive(Clone)]
struct Huffman {
    fast: [u8; 1 << FAST_BITS],
    code: [u16; 256],
    values: [u8; 256],
    size: [u8; 257],
    maxcode: [u32; 18],
    delta: [i32; 17],
}

impl Default for Huffman {
    fn default() -> Self {
        Huffman {
            fast: [0; 1 << FAST_BITS],
            code: [0; 256],
            values: [0; 256],
            size: [0; 257],
            maxcode: [0; 18],
            delta: [0; 17],
        }
    }
}

/// `stbi__build_huffman`.
fn build_huffman(h: &mut Huffman, count: &[i32; 16]) -> bool {
    let mut k = 0usize;
    for (i, &c) in count.iter().enumerate() {
        for _ in 0..c {
            h.size[k] = (i + 1) as u8;
            k += 1;
            if k >= 257 {
                return false; // bad size list
            }
        }
    }
    h.size[k] = 0;

    let mut code = 0u32;
    k = 0;
    for j in 1..=16usize {
        h.delta[j] = (k as u32).wrapping_sub(code) as i32;
        if usize::from(h.size[k]) == j {
            while usize::from(h.size[k]) == j {
                h.code[k] = code as u16;
                k += 1;
                code += 1;
            }
            if code - 1 >= (1u32 << j) {
                return false; // bad code lengths
            }
        }
        h.maxcode[j] = code << (16 - j);
        code <<= 1;
    }
    h.maxcode[17] = 0xffff_ffff;

    h.fast.fill(255);
    for i in 0..k {
        let s = usize::from(h.size[i]);
        if s <= FAST_BITS {
            let c = usize::from(h.code[i]) << (FAST_BITS - s);
            let m = 1usize << (FAST_BITS - s);
            for j in 0..m {
                h.fast[c + j] = i as u8;
            }
        }
    }
    true
}

/// `stbi__build_fast_ac`.
fn build_fast_ac(fast_ac: &mut [i16; 1 << FAST_BITS], h: &Huffman) {
    for i in 0..(1usize << FAST_BITS) {
        let fast = h.fast[i];
        fast_ac[i] = 0;
        if fast < 255 {
            let rs = i32::from(h.values[usize::from(fast)]);
            let run = (rs >> 4) & 15;
            let magbits = rs & 15;
            let len = i32::from(h.size[usize::from(fast)]);

            if magbits != 0 && len + magbits <= FAST_BITS as i32 {
                // magnitude code followed by receive_extend code
                let mut k = (((i as i32) << len) & ((1 << FAST_BITS) - 1)) >> (FAST_BITS as i32 - magbits);
                let m = 1 << (magbits - 1);
                if k < m {
                    k += ((!0u32) << magbits).wrapping_add(1) as i32;
                }
                // if the result is small enough, we can fit it in fast_ac table
                if (-128..=127).contains(&k) {
                    fast_ac[i] = ((k * 256) + (run * 16) + (len + magbits)) as i16;
                }
            }
        }
    }
}

#[derive(Default, Clone)]
struct Comp {
    id: i32,
    h: i32,
    v: i32,
    tq: usize,
    hd: usize,
    ha: usize,
    dc_pred: i32,
    x: i32,
    y: i32,
    w2: usize,
    h2: usize,
    data: Vec<u8>,
    coeff: Vec<i16>,
    coeff_w: usize,
}

/// Entropy decoder state (`code_buffer` & co.) with the byte source.
struct Bits<'a> {
    s: Ctx<'a>,
    code_buffer: u32,
    code_bits: i32,
    marker: u8,
    nomore: bool,
}

impl Bits<'_> {
    /// `stbi__grow_buffer_unsafe`.
    fn grow_buffer(&mut self) {
        loop {
            let mut b = if self.nomore { 0 } else { u32::from(self.s.get8()) };
            if b == 0xff {
                let mut c = self.s.get8();
                while c == 0xff {
                    c = self.s.get8(); // consume fill bytes
                }
                if c != 0 {
                    self.marker = c;
                    self.nomore = true;
                    return;
                }
                b = 0xff;
            }
            let shift = (24 - self.code_bits) as u32;
            self.code_buffer |= b.checked_shl(shift).unwrap_or(0);
            self.code_bits += 8;
            if self.code_bits > 24 {
                break;
            }
        }
    }

    /// `stbi__jpeg_huff_decode`: `-1` on error.
    fn huff_decode(&mut self, h: &Huffman) -> i32 {
        if self.code_bits < 16 {
            self.grow_buffer();
        }

        // look at the top FAST_BITS and determine what symbol ID it is,
        // if the code is <= FAST_BITS
        let c = ((self.code_buffer >> (32 - FAST_BITS)) & ((1 << FAST_BITS) - 1)) as usize;
        let k = h.fast[c];
        if k < 255 {
            let s = i32::from(h.size[usize::from(k)]);
            if s > self.code_bits {
                return -1;
            }
            self.code_buffer <<= s;
            self.code_bits -= s;
            return i32::from(h.values[usize::from(k)]);
        }

        let temp = self.code_buffer >> 16;
        let mut k = FAST_BITS + 1;
        while temp >= h.maxcode[k] {
            k += 1;
        }
        if k == 17 {
            // error! code not found
            self.code_bits -= 16;
            return -1;
        }
        if k as i32 > self.code_bits {
            return -1;
        }

        // convert the huffman code to the symbol id
        let c = (((self.code_buffer >> (32 - k)) & BMASK[k]) as i32).wrapping_add(h.delta[k]);
        if !(0..256).contains(&c) {
            return -1; // symbol id out of bounds!
        }
        self.code_bits -= k as i32;
        self.code_buffer <<= k;
        i32::from(h.values[c as usize])
    }

    /// `stbi__extend_receive`.
    fn extend_receive(&mut self, n: i32) -> i32 {
        if self.code_bits < n {
            self.grow_buffer();
        }
        if self.code_bits < n {
            return 0; // ran out of bits from stream, return 0s instead of continuing
        }
        let sgn = (self.code_buffer >> 31) as i32; // 0 if MSB clear (positive), 1 if set
        let mut k = self.code_buffer.rotate_left(n as u32);
        self.code_buffer = k & !BMASK[n as usize];
        k &= BMASK[n as usize];
        self.code_bits -= n;
        (k as i32).wrapping_add(JBIAS[n as usize] & (sgn - 1))
    }

    /// `stbi__jpeg_get_bits`.
    fn get_bits(&mut self, n: i32) -> i32 {
        if self.code_bits < n {
            self.grow_buffer();
        }
        if self.code_bits < n {
            return 0;
        }
        let mut k = self.code_buffer.rotate_left(n as u32);
        self.code_buffer = k & !BMASK[n as usize];
        k &= BMASK[n as usize];
        self.code_bits -= n;
        k as i32
    }

    /// `stbi__jpeg_get_bit`.
    fn get_bit(&mut self) -> bool {
        if self.code_bits < 1 {
            self.grow_buffer();
        }
        if self.code_bits < 1 {
            return false;
        }
        let k = self.code_buffer;
        self.code_buffer <<= 1;
        self.code_bits -= 1;
        k & 0x8000_0000 != 0
    }

    /// `stbi__get_marker`.
    fn get_marker(&mut self) -> u8 {
        if self.marker != MARKER_NONE {
            let x = self.marker;
            self.marker = MARKER_NONE;
            return x;
        }
        let mut x = self.s.get8();
        if x != 0xff {
            return MARKER_NONE;
        }
        while x == 0xff {
            x = self.s.get8(); // consume repeated 0xff fill bytes
        }
        x
    }
}

fn addints_valid(a: i32, b: i32) -> bool {
    a.checked_add(b).is_some()
}

/// `stbi__mul2shorts_valid`.
fn mul2shorts_valid(a: i32, b: i32) -> bool {
    const SHRT_MAX: i32 = i16::MAX as i32;
    const SHRT_MIN: i32 = i16::MIN as i32;
    if b == 0 || b == -1 {
        return true;
    }
    if (a >= 0) == (b >= 0) {
        return a <= SHRT_MAX / b;
    }
    if b < 0 {
        return a <= SHRT_MIN / b;
    }
    a >= SHRT_MIN / b
}

/// `stbi__jpeg_decode_block`: one baseline block, dequantised, in natural order.
#[allow(clippy::too_many_arguments)]
fn decode_block(
    b: &mut Bits,
    data: &mut [i16; 64],
    hdc: &Huffman,
    hac: &Huffman,
    fac: &[i16; 1 << FAST_BITS],
    dc_pred: &mut i32,
    dequant: &[u16; 64],
) -> bool {
    if b.code_bits < 16 {
        b.grow_buffer();
    }
    let t = b.huff_decode(hdc);
    if !(0..=15).contains(&t) {
        return false; // bad huffman code
    }

    data.fill(0);

    let diff = if t != 0 { b.extend_receive(t) } else { 0 };
    if !addints_valid(*dc_pred, diff) {
        return false; // bad delta
    }
    let dc = *dc_pred + diff;
    *dc_pred = dc;
    if !mul2shorts_valid(dc, i32::from(dequant[0])) {
        return false; // can't merge dc and ac
    }
    data[0] = dc.wrapping_mul(i32::from(dequant[0])) as i16;

    // decode AC components, see JPEG spec
    let mut k = 1usize;
    loop {
        if b.code_bits < 16 {
            b.grow_buffer();
        }
        let c = ((b.code_buffer >> (32 - FAST_BITS)) & ((1 << FAST_BITS) - 1)) as usize;
        let r = i32::from(fac[c]);
        if r != 0 {
            // fast-AC path
            k += ((r >> 4) & 15) as usize; // run
            let s = r & 15; // combined length
            if s > b.code_bits {
                return false;
            }
            b.code_buffer <<= s;
            b.code_bits -= s;
            let zig = usize::from(DEZIGZAG[k]);
            k += 1;
            data[zig] = ((r >> 8).wrapping_mul(i32::from(dequant[zig]))) as i16;
        } else {
            let rs = b.huff_decode(hac);
            if rs < 0 {
                return false;
            }
            let s = rs & 15;
            let r = rs >> 4;
            if s == 0 {
                if rs != 0xf0 {
                    break; // end block
                }
                k += 16;
            } else {
                k += r as usize;
                let zig = usize::from(DEZIGZAG[k]);
                k += 1;
                data[zig] = (b.extend_receive(s).wrapping_mul(i32::from(dequant[zig]))) as i16;
            }
        }
        if k >= 64 {
            break;
        }
    }
    true
}

/// Progressive scan parameters (`spec_start` & co.).
#[derive(Default)]
struct Scan {
    spec_start: i32,
    spec_end: i32,
    succ_high: i32,
    succ_low: i32,
    eob_run: i32,
}

/// `stbi__jpeg_decode_block_prog_dc`.
fn decode_block_prog_dc(b: &mut Bits, sc: &Scan, data: &mut [i16], hdc: &Huffman, dc_pred: &mut i32) -> bool {
    if sc.spec_end != 0 {
        return false;
    }
    if b.code_bits < 16 {
        b.grow_buffer();
    }

    if sc.succ_high == 0 {
        // first scan for DC coefficient, must be first
        data[..64].fill(0);
        let t = b.huff_decode(hdc);
        if !(0..=15).contains(&t) {
            return false;
        }
        let diff = if t != 0 { b.extend_receive(t) } else { 0 };
        if !addints_valid(*dc_pred, diff) {
            return false;
        }
        let dc = *dc_pred + diff;
        *dc_pred = dc;
        if !mul2shorts_valid(dc, 1 << sc.succ_low) {
            return false;
        }
        data[0] = dc.wrapping_mul(1 << sc.succ_low) as i16;
    } else if b.get_bit() {
        // refinement scan for DC coefficient
        data[0] = data[0].wrapping_add((1i32 << sc.succ_low) as i16);
    }
    true
}

/// `stbi__jpeg_decode_block_prog_ac`.
fn decode_block_prog_ac(
    b: &mut Bits,
    sc: &mut Scan,
    data: &mut [i16],
    hac: &Huffman,
    fac: &[i16; 1 << FAST_BITS],
) -> bool {
    if sc.spec_start == 0 {
        return false;
    }

    if sc.succ_high == 0 {
        let shift = sc.succ_low;

        if sc.eob_run != 0 {
            sc.eob_run -= 1;
            return true;
        }

        let mut k = sc.spec_start as usize;
        loop {
            if b.code_bits < 16 {
                b.grow_buffer();
            }
            let c = ((b.code_buffer >> (32 - FAST_BITS)) & ((1 << FAST_BITS) - 1)) as usize;
            let r = i32::from(fac[c]);
            if r != 0 {
                // fast-AC path
                k += ((r >> 4) & 15) as usize; // run
                let s = r & 15; // combined length
                if s > b.code_bits {
                    return false;
                }
                b.code_buffer <<= s;
                b.code_bits -= s;
                let zig = usize::from(DEZIGZAG[k]);
                k += 1;
                data[zig] = ((r >> 8).wrapping_mul(1 << shift)) as i16;
            } else {
                let rs = b.huff_decode(hac);
                if rs < 0 {
                    return false;
                }
                let s = rs & 15;
                let r = rs >> 4;
                if s == 0 {
                    if r < 15 {
                        sc.eob_run = 1 << r;
                        if r != 0 {
                            sc.eob_run += b.get_bits(r);
                        }
                        sc.eob_run -= 1;
                        break;
                    }
                    k += 16;
                } else {
                    k += r as usize;
                    let zig = usize::from(DEZIGZAG[k]);
                    k += 1;
                    data[zig] = (b.extend_receive(s).wrapping_mul(1 << shift)) as i16;
                }
            }
            if k as i32 > sc.spec_end {
                break;
            }
        }
    } else {
        // refinement scan for these AC coefficients
        let bit = (1i32 << sc.succ_low) as i16;

        let refine = |b: &mut Bits, p: &mut i16| {
            if *p != 0 && b.get_bit() && (*p & bit) == 0 {
                if *p > 0 {
                    *p = p.wrapping_add(bit);
                } else {
                    *p = p.wrapping_sub(bit);
                }
            }
        };

        if sc.eob_run != 0 {
            sc.eob_run -= 1;
            for k in sc.spec_start..=sc.spec_end {
                refine(b, &mut data[usize::from(DEZIGZAG[k as usize])]);
            }
        } else {
            let mut k = sc.spec_start;
            loop {
                let rs = b.huff_decode(hac);
                if rs < 0 {
                    return false;
                }
                let mut s = rs & 15;
                let mut r = rs >> 4;
                if s == 0 {
                    if r < 15 {
                        sc.eob_run = (1 << r) - 1;
                        if r != 0 {
                            sc.eob_run += b.get_bits(r);
                        }
                        r = 64; // force end of block
                    }
                    // r=15 s=0 should write 16 0s, so we just do a run of 15 0s
                    // and then write s (which is 0), nothing special to do
                } else {
                    if s != 1 {
                        return false;
                    }
                    // sign bit
                    s = if b.get_bit() { i32::from(bit) } else { -i32::from(bit) };
                }

                // advance by r
                while k <= sc.spec_end {
                    let p = &mut data[usize::from(DEZIGZAG[k as usize])];
                    k += 1;
                    if *p != 0 {
                        refine(b, p);
                    } else {
                        if r == 0 {
                            *p = s as i16;
                            break;
                        }
                        r -= 1;
                    }
                }
                if k > sc.spec_end {
                    break;
                }
            }
        }
    }
    true
}

#[allow(clippy::excessive_precision)]
const fn f2f(x: f32) -> i32 {
    ((x * 4096.0) as f64 + 0.5) as i32
}

/// `STBI__IDCT_1D`: returns `(x0, x1, x2, x3, t0, t1, t2, t3)` (i64 so corrupt
/// coefficients cannot overflow).
fn idct_1d(s: [i64; 8]) -> [i64; 8] {
    let p2 = s[2];
    let p3 = s[6];
    let p1 = (p2 + p3) * i64::from(f2f(0.5411961));
    let t2 = p1 + p3 * i64::from(f2f(-1.847759065));
    let t3 = p1 + p2 * i64::from(f2f(0.765366865));
    let p2 = s[0];
    let p3 = s[4];
    let t0 = (p2 + p3) * 4096;
    let t1 = (p2 - p3) * 4096;
    let x0 = t0 + t3;
    let x3 = t0 - t3;
    let x1 = t1 + t2;
    let x2 = t1 - t2;
    let mut t0 = s[7];
    let mut t1 = s[5];
    let mut t2 = s[3];
    let mut t3 = s[1];
    let mut p3 = t0 + t2;
    let mut p4 = t1 + t3;
    let mut p1 = t0 + t3;
    let mut p2 = t1 + t2;
    let p5 = (p3 + p4) * i64::from(f2f(1.175875602));
    t0 *= i64::from(f2f(0.298631336));
    t1 *= i64::from(f2f(2.053119869));
    t2 *= i64::from(f2f(3.072711026));
    t3 *= i64::from(f2f(1.501321110));
    p1 = p5 + p1 * i64::from(f2f(-0.899976223));
    p2 = p5 + p2 * i64::from(f2f(-2.562915447));
    p3 *= i64::from(f2f(-1.961570560));
    p4 *= i64::from(f2f(-0.390180644));
    t3 += p1 + p4;
    t2 += p2 + p3;
    t1 += p2 + p4;
    t0 += p1 + p3;
    [x0, x1, x2, x3, t0, t1, t2, t3]
}

fn clamp(x: i64) -> u8 {
    x.clamp(0, 255) as u8
}

/// `stbi__idct_block`: `out` starts at the block's top-left, rows `stride` apart.
fn idct_block(out: &mut [u8], stride: usize, data: &[i16; 64]) {
    let mut val = [0i32; 64];

    // columns
    for i in 0..8 {
        let d = |r: usize| i64::from(data[r * 8 + i]);
        if (1..8).all(|r| d(r) == 0) {
            // all zeroes, shortcut
            let dcterm = d(0) as i32 * 4;
            for r in 0..8 {
                val[r * 8 + i] = dcterm;
            }
        } else {
            let [x0, x1, x2, x3, t0, t1, t2, t3] = idct_1d([d(0), d(1), d(2), d(3), d(4), d(5), d(6), d(7)]);
            // constants scaled things up by 1<<12; bring them back down, but keep 2 extra bits of precision
            let (x0, x1, x2, x3) = (x0 + 512, x1 + 512, x2 + 512, x3 + 512);
            val[i] = ((x0 + t3) >> 10) as i32;
            val[56 + i] = ((x0 - t3) >> 10) as i32;
            val[8 + i] = ((x1 + t2) >> 10) as i32;
            val[48 + i] = ((x1 - t2) >> 10) as i32;
            val[16 + i] = ((x2 + t1) >> 10) as i32;
            val[40 + i] = ((x2 - t1) >> 10) as i32;
            val[24 + i] = ((x3 + t0) >> 10) as i32;
            val[32 + i] = ((x3 - t0) >> 10) as i32;
        }
    }

    for i in 0..8 {
        let v = &val[i * 8..i * 8 + 8];
        let [x0, x1, x2, x3, t0, t1, t2, t3] = idct_1d([
            i64::from(v[0]),
            i64::from(v[1]),
            i64::from(v[2]),
            i64::from(v[3]),
            i64::from(v[4]),
            i64::from(v[5]),
            i64::from(v[6]),
            i64::from(v[7]),
        ]);
        // 1<<12 scale, 1<<2 from the first loop, sqrt(8) per direction: 1<<17 to remove; round and add 128.
        let bias = 65536 + (128 << 17);
        let (x0, x1, x2, x3) = (x0 + bias, x1 + bias, x2 + bias, x3 + bias);
        let o = &mut out[i * stride..i * stride + 8];
        o[0] = clamp((x0 + t3) >> 17);
        o[7] = clamp((x0 - t3) >> 17);
        o[1] = clamp((x1 + t2) >> 17);
        o[6] = clamp((x1 - t2) >> 17);
        o[2] = clamp((x2 + t1) >> 17);
        o[5] = clamp((x2 - t1) >> 17);
        o[3] = clamp((x3 + t0) >> 17);
        o[4] = clamp((x3 - t0) >> 17);
    }
}

fn is_restart(m: u8) -> bool {
    (0xd0..=0xd7).contains(&m)
}

struct Jpeg<'a> {
    b: Bits<'a>,
    img_x: u32,
    img_y: u32,
    img_n: usize,
    huff_dc: [Huffman; 4],
    huff_ac: [Huffman; 4],
    dequant: [[u16; 64]; 4],
    fast_ac: [[i16; 1 << FAST_BITS]; 4],
    img_h_max: i32,
    img_v_max: i32,
    img_mcu_x: i32,
    img_mcu_y: i32,
    comps: [Comp; 4],
    progressive: bool,
    sc: Scan,
    jfif: bool,
    app14_color_transform: i32,
    rgb: i32,
    scan_n: usize,
    order: [usize; 4],
    restart_interval: i32,
    todo: i32,
}

#[derive(PartialEq, Clone, Copy)]
enum ScanKind {
    Load,
    Type,
    Header,
}

impl<'a> Jpeg<'a> {
    fn new(data: &'a [u8]) -> Self {
        Jpeg {
            b: Bits { s: Ctx::new(data), code_buffer: 0, code_bits: 0, marker: 0, nomore: false },
            img_x: 0,
            img_y: 0,
            img_n: 0,
            huff_dc: Default::default(),
            huff_ac: Default::default(),
            dequant: [[0; 64]; 4],
            fast_ac: [[0; 1 << FAST_BITS]; 4],
            img_h_max: 0,
            img_v_max: 0,
            img_mcu_x: 0,
            img_mcu_y: 0,
            comps: Default::default(),
            progressive: false,
            sc: Scan::default(),
            jfif: false,
            app14_color_transform: 0,
            rgb: 0,
            scan_n: 0,
            order: [0; 4],
            restart_interval: 0,
            todo: 0,
        }
    }

    /// `stbi__jpeg_reset`.
    fn reset(&mut self) {
        self.b.code_bits = 0;
        self.b.code_buffer = 0;
        self.b.nomore = false;
        for c in &mut self.comps {
            c.dc_pred = 0;
        }
        self.b.marker = MARKER_NONE;
        self.todo = if self.restart_interval != 0 { self.restart_interval } else { 0x7fff_ffff };
        self.sc.eob_run = 0;
    }

    /// The end-of-MCU restart handling shared by all four scan loops. `true`
    /// means "stop decoding this scan, successfully".
    fn after_mcu(&mut self) -> bool {
        self.todo -= 1;
        if self.todo <= 0 {
            if self.b.code_bits < 24 {
                self.b.grow_buffer();
            }
            // if it's NOT a restart, then just bail, so we get corrupt data rather than no data
            if !is_restart(self.b.marker) {
                return true;
            }
            self.reset();
        }
        false
    }

    /// `stbi__parse_entropy_coded_data`.
    fn parse_entropy_coded_data(&mut self) -> bool {
        self.reset();
        let mut data = [0i16; 64];
        if !self.progressive {
            if self.scan_n == 1 {
                let n = self.order[0];
                // non-interleaved data: one block at a time, trivial scanline order
                let w = (self.comps[n].x + 7) >> 3;
                let h = (self.comps[n].y + 7) >> 3;
                for j in 0..h as usize {
                    for i in 0..w as usize {
                        if !self.baseline_block(n, &mut data) {
                            return false;
                        }
                        let c = &mut self.comps[n];
                        let off = c.w2 * j * 8 + i * 8;
                        let stride = c.w2;
                        idct_block(&mut c.data[off..], stride, &data);
                        // every data block is an MCU, so countdown the restart interval
                        if self.after_mcu() {
                            return true;
                        }
                    }
                }
                true
            } else {
                for j in 0..self.img_mcu_y as usize {
                    for i in 0..self.img_mcu_x as usize {
                        for k in 0..self.scan_n {
                            let n = self.order[k];
                            for y in 0..self.comps[n].v as usize {
                                for x in 0..self.comps[n].h as usize {
                                    let x2 = (i * self.comps[n].h as usize + x) * 8;
                                    let y2 = (j * self.comps[n].v as usize + y) * 8;
                                    if !self.baseline_block(n, &mut data) {
                                        return false;
                                    }
                                    let c = &mut self.comps[n];
                                    let stride = c.w2;
                                    idct_block(&mut c.data[stride * y2 + x2..], stride, &data);
                                }
                            }
                        }
                        if self.after_mcu() {
                            return true;
                        }
                    }
                }
                true
            }
        } else if self.scan_n == 1 {
            let n = self.order[0];
            let w = (self.comps[n].x + 7) >> 3;
            let h = (self.comps[n].y + 7) >> 3;
            for j in 0..h as usize {
                for i in 0..w as usize {
                    let off = 64 * (i + j * self.comps[n].coeff_w);
                    if !self.progressive_block(n, off) {
                        return false;
                    }
                    if self.after_mcu() {
                        return true;
                    }
                }
            }
            true
        } else {
            for j in 0..self.img_mcu_y as usize {
                for i in 0..self.img_mcu_x as usize {
                    for k in 0..self.scan_n {
                        let n = self.order[k];
                        for y in 0..self.comps[n].v as usize {
                            for x in 0..self.comps[n].h as usize {
                                let x2 = i * self.comps[n].h as usize + x;
                                let y2 = j * self.comps[n].v as usize + y;
                                let off = 64 * (x2 + y2 * self.comps[n].coeff_w);
                                let c = &mut self.comps[n];
                                if !decode_block_prog_dc(
                                    &mut self.b,
                                    &self.sc,
                                    &mut c.coeff[off..off + 64],
                                    &self.huff_dc[c.hd],
                                    &mut c.dc_pred,
                                ) {
                                    return false;
                                }
                            }
                        }
                    }
                    if self.after_mcu() {
                        return true;
                    }
                }
            }
            true
        }
    }

    fn baseline_block(&mut self, n: usize, data: &mut [i16; 64]) -> bool {
        let c = &mut self.comps[n];
        decode_block(
            &mut self.b,
            data,
            &self.huff_dc[c.hd],
            &self.huff_ac[c.ha],
            &self.fast_ac[c.ha],
            &mut c.dc_pred,
            &self.dequant[c.tq],
        )
    }

    /// One block of a non-interleaved progressive scan (DC or AC by `spec_start`).
    fn progressive_block(&mut self, n: usize, off: usize) -> bool {
        let c = &mut self.comps[n];
        let data = &mut c.coeff[off..off + 64];
        if self.sc.spec_start == 0 {
            decode_block_prog_dc(&mut self.b, &self.sc, data, &self.huff_dc[c.hd], &mut c.dc_pred)
        } else {
            decode_block_prog_ac(&mut self.b, &mut self.sc, data, &self.huff_ac[c.ha], &self.fast_ac[c.ha])
        }
    }

    /// `stbi__jpeg_finish`: dequantise and IDCT the progressive coefficients.
    fn finish(&mut self) {
        if !self.progressive {
            return;
        }
        for n in 0..self.img_n {
            let w = (self.comps[n].x + 7) >> 3;
            let h = (self.comps[n].y + 7) >> 3;
            for j in 0..h as usize {
                for i in 0..w as usize {
                    let c = &mut self.comps[n];
                    let off = 64 * (i + j * c.coeff_w);
                    let mut block = [0i16; 64];
                    for (k, v) in block.iter_mut().enumerate() {
                        *v = c.coeff[off + k].wrapping_mul(self.dequant[c.tq][k] as i16);
                    }
                    let stride = c.w2;
                    idct_block(&mut c.data[stride * j * 8 + i * 8..], stride, &block);
                }
            }
        }
    }

    /// `stbi__process_marker`.
    fn process_marker(&mut self, m: u8) -> bool {
        match m {
            MARKER_NONE => return false, // expected marker
            0xDD => {
                // DRI - specify restart interval
                if self.b.s.get16be() != 4 {
                    return false;
                }
                self.restart_interval = self.b.s.get16be();
                return true;
            }
            0xDB => {
                // DQT - define quantization table
                let mut l = self.b.s.get16be() - 2;
                while l > 0 {
                    let q = i32::from(self.b.s.get8());
                    let p = q >> 4;
                    let sixteen = p != 0;
                    let t = (q & 15) as usize;
                    if p != 0 && p != 1 {
                        return false;
                    }
                    if t > 3 {
                        return false;
                    }
                    for i in 0..64 {
                        let v = if sixteen { self.b.s.get16be() } else { i32::from(self.b.s.get8()) };
                        self.dequant[t][usize::from(DEZIGZAG[i])] = v as u16;
                    }
                    l -= if sixteen { 129 } else { 65 };
                }
                return l == 0;
            }
            0xC4 => {
                // DHT - define huffman table
                let mut l = self.b.s.get16be() - 2;
                while l > 0 {
                    let mut sizes = [0i32; 16];
                    let mut n = 0;
                    let q = i32::from(self.b.s.get8());
                    let tc = q >> 4;
                    let th = (q & 15) as usize;
                    if tc > 1 || th > 3 {
                        return false;
                    }
                    for s in &mut sizes {
                        *s = i32::from(self.b.s.get8());
                        n += *s;
                    }
                    if n > 256 {
                        return false; // loop over i < n would write past end of values
                    }
                    l -= 17;
                    let h = if tc == 0 { &mut self.huff_dc[th] } else { &mut self.huff_ac[th] };
                    if !build_huffman(h, &sizes) {
                        return false;
                    }
                    for i in 0..n as usize {
                        h.values[i] = self.b.s.get8();
                    }
                    if tc != 0 {
                        build_fast_ac(&mut self.fast_ac[th], &self.huff_ac[th]);
                    }
                    l -= n;
                }
                return l == 0;
            }
            _ => {}
        }

        // check for comment block or APP blocks
        if (0xE0..=0xEF).contains(&m) || m == 0xFE {
            let mut l = self.b.s.get16be();
            if l < 2 {
                return false;
            }
            l -= 2;

            if m == 0xE0 && l >= 5 {
                // JFIF APP0 segment
                let mut ok = true;
                for &t in b"JFIF\0" {
                    if self.b.s.get8() != t {
                        ok = false;
                    }
                }
                l -= 5;
                if ok {
                    self.jfif = true;
                }
            } else if m == 0xEE && l >= 12 {
                // Adobe APP14 segment
                let mut ok = true;
                for &t in b"Adobe\0" {
                    if self.b.s.get8() != t {
                        ok = false;
                    }
                }
                l -= 6;
                if ok {
                    self.b.s.get8(); // version
                    self.b.s.get16be(); // flags0
                    self.b.s.get16be(); // flags1
                    self.app14_color_transform = i32::from(self.b.s.get8());
                    l -= 6;
                }
            }

            self.b.s.skip(l);
            return true;
        }

        false // unknown marker
    }

    /// `stbi__process_scan_header`.
    fn process_scan_header(&mut self) -> bool {
        let ls = self.b.s.get16be();
        let scan_n = i32::from(self.b.s.get8());
        if scan_n < 1 || scan_n > 4 || scan_n > self.img_n as i32 {
            return false;
        }
        self.scan_n = scan_n as usize;
        if ls != 6 + 2 * scan_n {
            return false;
        }
        for i in 0..self.scan_n {
            let id = i32::from(self.b.s.get8());
            let q = i32::from(self.b.s.get8());
            let Some(which) = self.comps[..self.img_n].iter().position(|c| c.id == id) else {
                return false; // no match
            };
            self.comps[which].hd = (q >> 4) as usize;
            if self.comps[which].hd > 3 {
                return false;
            }
            self.comps[which].ha = (q & 15) as usize;
            if self.comps[which].ha > 3 {
                return false;
            }
            self.order[i] = which;
        }

        self.sc.spec_start = i32::from(self.b.s.get8());
        self.sc.spec_end = i32::from(self.b.s.get8()); // should be 63, but might be 0
        let aa = i32::from(self.b.s.get8());
        self.sc.succ_high = aa >> 4;
        self.sc.succ_low = aa & 15;
        if self.progressive {
            if self.sc.spec_start > 63
                || self.sc.spec_end > 63
                || self.sc.spec_start > self.sc.spec_end
                || self.sc.succ_high > 13
                || self.sc.succ_low > 13
            {
                return false;
            }
        } else {
            if self.sc.spec_start != 0 {
                return false;
            }
            if self.sc.succ_high != 0 || self.sc.succ_low != 0 {
                return false;
            }
            self.sc.spec_end = 63;
        }
        true
    }

    /// `stbi__process_frame_header`.
    fn process_frame_header(&mut self, scan: ScanKind) -> bool {
        let lf = self.b.s.get16be();
        if lf < 11 {
            return false;
        }
        if self.b.s.get8() != 8 {
            return false; // only 8-bit
        }
        self.img_y = self.b.s.get16be() as u32;
        if self.img_y == 0 {
            return false; // delayed height
        }
        self.img_x = self.b.s.get16be() as u32;
        if self.img_x == 0 {
            return false;
        }
        if self.img_y > MAX_DIMENSIONS || self.img_x > MAX_DIMENSIONS {
            return false;
        }
        let c = i32::from(self.b.s.get8());
        if c != 3 && c != 1 && c != 4 {
            return false;
        }
        self.img_n = c as usize;
        for comp in &mut self.comps[..self.img_n] {
            comp.data = Vec::new();
        }

        if lf != 8 + 3 * self.img_n as i32 {
            return false;
        }

        self.rgb = 0;
        for i in 0..self.img_n {
            self.comps[i].id = i32::from(self.b.s.get8());
            if self.img_n == 3 && self.comps[i].id == i32::from(b"RGB"[i]) {
                self.rgb += 1;
            }
            let q = i32::from(self.b.s.get8());
            self.comps[i].h = q >> 4;
            if self.comps[i].h == 0 || self.comps[i].h > 4 {
                return false;
            }
            self.comps[i].v = q & 15;
            if self.comps[i].v == 0 || self.comps[i].v > 4 {
                return false;
            }
            self.comps[i].tq = usize::from(self.b.s.get8());
            if self.comps[i].tq > 3 {
                return false;
            }
        }

        if scan != ScanKind::Load {
            return true;
        }

        if !mad3sizes_valid(self.img_x as i32, self.img_y as i32, self.img_n as i32) {
            return false; // too large
        }

        let (mut h_max, mut v_max) = (1, 1);
        for c in &self.comps[..self.img_n] {
            h_max = h_max.max(c.h);
            v_max = v_max.max(c.v);
        }

        // plane subsampling factors must be integer ratios
        for c in &self.comps[..self.img_n] {
            if h_max % c.h != 0 || v_max % c.v != 0 {
                return false;
            }
        }

        // compute interleaved mcu info
        self.img_h_max = h_max;
        self.img_v_max = v_max;
        let mcu_w = h_max * 8;
        let mcu_h = v_max * 8;
        self.img_mcu_x = (self.img_x as i32 + mcu_w - 1) / mcu_w;
        self.img_mcu_y = (self.img_y as i32 + mcu_h - 1) / mcu_h;

        for i in 0..self.img_n {
            let (mcu_x, mcu_y) = (self.img_mcu_x, self.img_mcu_y);
            let c = &mut self.comps[i];
            // number of effective pixels (e.g. for non-interleaved MCU)
            c.x = (self.img_x as i32 * c.h + h_max - 1) / h_max;
            c.y = (self.img_y as i32 * c.v + v_max - 1) / v_max;
            // enough memory for the oversized data of interleaved MCUs
            c.w2 = (mcu_x * c.h * 8) as usize;
            c.h2 = (mcu_y * c.v * 8) as usize;
            c.data = Vec::new();
            if c.data.try_reserve_exact(c.w2 * c.h2).is_err() {
                return false; // out of memory
            }
            c.data.resize(c.w2 * c.h2, 0);
            if self.progressive {
                c.coeff_w = c.w2 / 8;
                let mut coeff = Vec::new();
                if coeff.try_reserve_exact(c.w2 * c.h2).is_err() {
                    return false;
                }
                coeff.resize(c.w2 * c.h2, 0i16);
                c.coeff = coeff;
            }
        }
        true
    }

    /// `stbi__decode_jpeg_header`.
    fn decode_header(&mut self, scan: ScanKind) -> bool {
        self.jfif = false;
        self.app14_color_transform = -1; // valid values are 0,1,2
        self.b.marker = MARKER_NONE; // initialize cached marker to empty
        let m = self.b.get_marker();
        if m != 0xd8 {
            return false; // no SOI
        }
        if scan == ScanKind::Type {
            return true;
        }
        let mut m = self.b.get_marker();
        while !matches!(m, 0xc0..=0xc2) {
            if !self.process_marker(m) {
                return false;
            }
            m = self.b.get_marker();
            while m == MARKER_NONE {
                // some files have extra padding after their blocks, so ok, we'll scan
                if self.b.s.at_eof() {
                    return false; // no SOF
                }
                m = self.b.get_marker();
            }
        }
        self.progressive = m == 0xc2;
        self.process_frame_header(scan)
    }

    /// `stbi__skip_jpeg_junk_at_end`.
    fn skip_junk_at_end(&mut self) -> u8 {
        // some JPEGs have junk at end, skip over it but if we find what looks
        // like a valid marker, resume there
        while !self.b.s.at_eof() {
            let mut x = self.b.s.get8();
            while x == 0xff {
                // might be a marker
                if self.b.s.at_eof() {
                    return MARKER_NONE;
                }
                x = self.b.s.get8();
                if x != 0x00 && x != 0xff {
                    // not a stuffed zero or lead-in to another marker: an actual marker
                    return x;
                }
            }
        }
        MARKER_NONE
    }

    /// `stbi__decode_jpeg_image`: decodes to per-component planes.
    fn decode_image(&mut self) -> bool {
        self.restart_interval = 0;
        if !self.decode_header(ScanKind::Load) {
            return false;
        }
        let mut m = self.b.get_marker();
        while m != 0xd9 {
            if m == 0xda {
                if !self.process_scan_header() {
                    return false;
                }
                if !self.parse_entropy_coded_data() {
                    return false;
                }
                if self.b.marker == MARKER_NONE {
                    self.b.marker = self.skip_junk_at_end();
                    // if we reach eof without hitting a marker, get_marker() below will fail and we'll eventually return 0
                }
                m = self.b.get_marker();
                if is_restart(m) {
                    m = self.b.get_marker();
                }
            } else if m == 0xdc {
                let ld = self.b.s.get16be();
                let nl = self.b.s.get16be() as u32;
                if ld != 4 {
                    return false;
                }
                if nl != self.img_y {
                    return false;
                }
                m = self.b.get_marker();
            } else {
                if !self.process_marker(m) {
                    return true; // as stb: keep what has been decoded
                }
                m = self.b.get_marker();
            }
        }
        self.finish();
        true
    }
}

/// `stbi__resample_row_v_2`.
fn resample_v_2(out: &mut [u8], near: &[u8], far: &[u8], w: usize) {
    for i in 0..w {
        out[i] = ((3 * u32::from(near[i]) + u32::from(far[i]) + 2) >> 2) as u8;
    }
}

/// `stbi__resample_row_h_2`.
fn resample_h_2(out: &mut [u8], input: &[u8], w: usize) {
    let div4 = |x: u32| (x >> 2) as u8;
    if w == 1 {
        // if only one sample, can't do any interpolation
        out[0] = input[0];
        out[1] = input[0];
        return;
    }
    out[0] = input[0];
    out[1] = div4(u32::from(input[0]) * 3 + u32::from(input[1]) + 2);
    let mut i = 1;
    while i < w - 1 {
        let n = 3 * u32::from(input[i]) + 2;
        out[i * 2] = div4(n + u32::from(input[i - 1]));
        out[i * 2 + 1] = div4(n + u32::from(input[i + 1]));
        i += 1;
    }
    out[i * 2] = div4(u32::from(input[w - 2]) * 3 + u32::from(input[w - 1]) + 2);
    out[i * 2 + 1] = input[w - 1];
}

/// `stbi__resample_row_hv_2` (the SSE2 variant computes the same values).
fn resample_hv_2(out: &mut [u8], near: &[u8], far: &[u8], w: usize) {
    if w == 1 {
        let v = ((3 * u32::from(near[0]) + u32::from(far[0]) + 2) >> 2) as u8;
        out[0] = v;
        out[1] = v;
        return;
    }
    let col = |i: usize| 3 * u32::from(near[i]) + u32::from(far[i]);
    let mut t1 = col(0);
    out[0] = ((t1 + 2) >> 2) as u8;
    for i in 1..w {
        let t0 = t1;
        t1 = col(i);
        out[i * 2 - 1] = ((3 * t0 + t1 + 8) >> 4) as u8;
        out[i * 2] = ((3 * t1 + t0 + 8) >> 4) as u8;
    }
    out[w * 2 - 1] = ((t1 + 2) >> 2) as u8;
}

/// `stbi__resample_row_generic`.
fn resample_generic(out: &mut [u8], near: &[u8], w: usize, hs: usize) {
    for i in 0..w {
        for j in 0..hs {
            out[i * hs + j] = near[i];
        }
    }
}

const fn float2fixed(x: f32) -> i32 {
    (((x * 4096.0 + 0.5) as i32) << 8) as i32
}

/// `stbi__YCbCr_to_RGB_row` for `step == 4`: writes R G B 255.
fn ycbcr_to_rgb_row(out: &mut [u8], y: &[u8], pcb: &[u8], pcr: &[u8], count: usize) {
    const CR_R: i32 = float2fixed(1.402_00);
    const CR_G: i32 = -float2fixed(0.714_14);
    const CB_G: i32 = -float2fixed(0.344_14);
    const CB_B: i32 = float2fixed(1.772_00);
    for i in 0..count {
        let y_fixed = (i32::from(y[i]) << 20) + (1 << 19); // rounding
        let cr = i32::from(pcr[i]) - 128;
        let cb = i32::from(pcb[i]) - 128;
        let mut r = y_fixed.wrapping_add(cr.wrapping_mul(CR_R));
        let mut g =
            y_fixed.wrapping_add(cr.wrapping_mul(CR_G)).wrapping_add(cb.wrapping_mul(CB_G) & 0xffff_0000u32 as i32);
        let mut b = y_fixed.wrapping_add(cb.wrapping_mul(CB_B));
        r >>= 20;
        g >>= 20;
        b >>= 20;
        out[i * 4] = r.clamp(0, 255) as u8;
        out[i * 4 + 1] = g.clamp(0, 255) as u8;
        out[i * 4 + 2] = b.clamp(0, 255) as u8;
        out[i * 4 + 3] = 255;
    }
}

/// `stbi__blinn_8x8`: rounded `x * y / 255`.
fn blinn_8x8(x: u8, y: u8) -> u8 {
    let t = u32::from(x) * u32::from(y) + 128;
    ((t + (t >> 8)) >> 8) as u8
}

struct Resample {
    hs: usize,
    vs: usize,
    ystep: usize,
    w_lores: usize,
    ypos: i32,
    line0: usize,
    line1: usize,
}

/// `load_jpeg_image` with `req_comp == 4`.
fn load_jpeg_image(z: &mut Jpeg) -> Option<Rgba> {
    if !z.decode_image() {
        return None;
    }

    // n = req_comp = 4
    let img_n = z.img_n;
    let is_rgb = img_n == 3 && (z.rgb == 3 || (z.app14_color_transform == 0 && !z.jfif));
    let decode_n = img_n;
    if decode_n == 0 {
        return None;
    }
    let (img_x, img_y) = (z.img_x as usize, z.img_y as usize);

    let mut res: Vec<Resample> = Vec::new();
    let mut linebuf: Vec<Vec<u8>> = Vec::new();
    for k in 0..decode_n {
        let c = &z.comps[k];
        let hs = (z.img_h_max / c.h) as usize;
        let vs = (z.img_v_max / c.v) as usize;
        res.push(Resample { hs, vs, ystep: vs >> 1, w_lores: (img_x + hs - 1) / hs, ypos: 0, line0: 0, line1: 0 });
        linebuf.push(vec![0u8; img_x + 3]);
    }

    let mut output = Vec::new();
    output.try_reserve_exact(img_x * img_y * 4).ok()?;
    output.resize(img_x * img_y * 4, 0u8);

    let mut coutput: [Vec<u8>; 4] = Default::default();
    for j in 0..img_y {
        let out = &mut output[4 * img_x * j..4 * img_x * (j + 1)];
        for k in 0..decode_n {
            let r = &mut res[k];
            let c = &z.comps[k];
            let y_bot = r.ystep >= (r.vs >> 1);
            let (near_off, far_off) = if y_bot { (r.line1, r.line0) } else { (r.line0, r.line1) };
            let near = &c.data[near_off..];
            let far = &c.data[far_off..];
            let lb = &mut linebuf[k];
            match (r.hs, r.vs) {
                (1, 1) => lb[..r.w_lores].copy_from_slice(&near[..r.w_lores]),
                (1, 2) => resample_v_2(lb, near, far, r.w_lores),
                (2, 1) => resample_h_2(lb, near, r.w_lores),
                (2, 2) => resample_hv_2(lb, near, far, r.w_lores),
                (hs, _) => resample_generic(lb, near, r.w_lores, hs),
            }
            coutput[k].clear();
            coutput[k].extend_from_slice(&lb[..img_x]);
            r.ystep += 1;
            if r.ystep >= r.vs {
                r.ystep = 0;
                r.line0 = r.line1;
                r.ypos += 1;
                if r.ypos < c.y {
                    r.line1 += c.w2;
                }
            }
        }

        if img_n == 3 {
            if is_rgb {
                for i in 0..img_x {
                    out[i * 4] = coutput[0][i];
                    out[i * 4 + 1] = coutput[1][i];
                    out[i * 4 + 2] = coutput[2][i];
                    out[i * 4 + 3] = 255;
                }
            } else {
                ycbcr_to_rgb_row(out, &coutput[0], &coutput[1], &coutput[2], img_x);
            }
        } else if img_n == 4 {
            if z.app14_color_transform == 0 {
                // CMYK
                for i in 0..img_x {
                    let m = coutput[3][i];
                    out[i * 4] = blinn_8x8(coutput[0][i], m);
                    out[i * 4 + 1] = blinn_8x8(coutput[1][i], m);
                    out[i * 4 + 2] = blinn_8x8(coutput[2][i], m);
                    out[i * 4 + 3] = 255;
                }
            } else if z.app14_color_transform == 2 {
                // YCCK
                ycbcr_to_rgb_row(out, &coutput[0], &coutput[1], &coutput[2], img_x);
                for i in 0..img_x {
                    let m = coutput[3][i];
                    out[i * 4] = blinn_8x8(255 - out[i * 4], m);
                    out[i * 4 + 1] = blinn_8x8(255 - out[i * 4 + 1], m);
                    out[i * 4 + 2] = blinn_8x8(255 - out[i * 4 + 2], m);
                }
            } else {
                // YCbCr + alpha? Ignore the fourth channel for now
                ycbcr_to_rgb_row(out, &coutput[0], &coutput[1], &coutput[2], img_x);
            }
        } else {
            for i in 0..img_x {
                let y = coutput[0][i];
                out[i * 4] = y;
                out[i * 4 + 1] = y;
                out[i * 4 + 2] = y;
                out[i * 4 + 3] = 255;
            }
        }
    }

    Some(Rgba { width: z.img_x, height: z.img_y, components: if img_n >= 3 { 3 } else { 1 }, pixels: output })
}

/// `stbi__jpeg_test`.
pub(super) fn test(data: &[u8]) -> bool {
    Jpeg::new(data).decode_header(ScanKind::Type)
}

/// `stbi__jpeg_load` with `req_comp == 4`.
pub(super) fn load(data: &[u8]) -> Option<Rgba> {
    let mut z = Jpeg::new(data);
    load_jpeg_image(&mut z)
}

/// `stbi__jpeg_info`.
pub(super) fn info(data: &[u8]) -> Option<Info> {
    let mut z = Jpeg::new(data);
    if !z.decode_header(ScanKind::Header) {
        return None;
    }
    Some(Info { width: z.img_x, height: z.img_y, components: if z.img_n >= 3 { 3 } else { 1 } })
}
