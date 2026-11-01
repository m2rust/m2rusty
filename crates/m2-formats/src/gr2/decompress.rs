//! Oodle1 section decompressor: port of `neoeter/src/model/grn/Gr2Decompress.c`
//! (itself derived from nwn2mdk's `gr2_decompress.cpp`).
//!
//! An arithmetic decoder feeds adaptive frequency tables ("weigh windows");
//! the output is a stream of literals and back references, split in three
//! steps that each start with fresh tables. The C code's integer widths are
//! kept (u16 counters wrap, u32 decoder state wraps), since the bitstream
//! depends on them.

/// The input is truncated, inconsistent or corrupt (`gr2_decompress` returning 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecompressError;

/// Index that `WeightWindow_Try_Decode` returns for "value already known".
const KNOWN: usize = 0xFFFF;

/// `TParameter`: three of them (one per step) start the compressed buffer.
#[derive(Clone, Copy)]
struct Parameter {
    decoded_value_max: u32,
    backref_value_max: u32,
    decoded_count: u32,
    highbit_count: u32,
    sizes_count: [u8; 4],
}

impl Parameter {
    const SIZE: usize = 12;

    /// The C struct's bitfields as GCC lays them out (first field in the
    /// low bits): `decoded_value_max:9, backref_value_max:23` then
    /// `decoded_count:9, padding:10, highbit_count:13`.
    fn parse(b: &[u8; Self::SIZE]) -> Self {
        let w0 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let w1 = u32::from_le_bytes([b[4], b[5], b[6], b[7]]);
        Parameter {
            decoded_value_max: w0 & 0x1ff,
            backref_value_max: w0 >> 9,
            decoded_count: w1 & 0x1ff,
            highbit_count: w1 >> 19,
            sizes_count: [b[8], b[9], b[10], b[11]],
        }
    }
}

/// `TDecoder`: the arithmetic decoder.
struct Decoder<'a> {
    numer: u32,
    denom: u32,
    next_denom: u32,
    stream: &'a [u8],
    pos: usize,
    error: bool,
}

impl<'a> Decoder<'a> {
    /// `Decoder_Init`.
    fn new(stream: &'a [u8]) -> Self {
        Decoder {
            numer: stream.first().map_or(0, |&b| u32::from(b >> 1)),
            denom: 0x80,
            next_denom: 0,
            stream,
            pos: 0,
            error: stream.is_empty(),
        }
    }

    /// `Decode`.
    fn decode(&mut self, max: u16) -> u16 {
        if self.error {
            return 0;
        }
        while self.denom <= 0x80_0000 {
            if self.stream.len() - self.pos < 2 {
                self.error = true;
                return 0;
            }
            self.numer <<= 8;
            self.numer |= (u32::from(self.stream[self.pos]) << 7) & 0x80;
            self.numer |= (u32::from(self.stream[self.pos + 1]) >> 1) & 0x7f;
            self.pos += 1;
            self.denom <<= 8;
        }
        if max == 0 {
            self.error = true;
            return 0;
        }
        self.next_denom = self.denom / u32::from(max);
        if self.next_denom == 0 {
            self.error = true;
            return 0;
        }
        (self.numer / self.next_denom).min(u32::from(max) - 1) as u16
    }

    /// `Commit`.
    fn commit(&mut self, max: u16, val: u16, err: u16) -> u16 {
        self.numer = self.numer.wrapping_sub(self.next_denom.wrapping_mul(u32::from(val)));
        if u32::from(val) + u32::from(err) < u32::from(max) {
            self.denom = self.next_denom.wrapping_mul(u32::from(err));
        } else {
            self.denom = self.denom.wrapping_sub(self.next_denom.wrapping_mul(u32::from(val)));
        }
        val
    }

    /// `Decode_Commit`.
    fn decode_commit(&mut self, max: u16) -> u16 {
        let val = self.decode(max);
        self.commit(max, val, 1)
    }
}

/// `TWeighWindow`: an adaptive frequency table over the values seen so far.
/// `values` and `weights` always have the same length (`valuesLength` and
/// `weightsLength` in C).
struct WeighWindow {
    count_cap: u16,
    ranges: Vec<u16>,
    values: Vec<u16>,
    weights: Vec<u16>,
    weight_total: u16,
    thresh_increase: u16,
    thresh_increase_cap: u16,
    thresh_range_rebuild: u16,
    thresh_weight_rebuild: u16,
}

impl WeighWindow {
    /// `WeighWindow_Init`. `count_cap` is a `uint16_t` parameter in C, so
    /// callers truncate.
    fn new(max_value: u32, count_cap: u16) -> Self {
        let thresh_weight_rebuild = max_value.wrapping_mul(32).clamp(256, 15160);
        let thresh_increase_cap = if max_value > 64 {
            max_value.wrapping_mul(2).min(thresh_weight_rebuild / 2 - 32)
        } else {
            128
        };
        WeighWindow {
            count_cap: count_cap.wrapping_add(1),
            ranges: vec![0, 0x4000],
            values: vec![0],
            weights: vec![4],
            weight_total: 4,
            thresh_increase: 4,
            thresh_increase_cap: thresh_increase_cap as u16,
            thresh_range_rebuild: 8,
            thresh_weight_rebuild: thresh_weight_rebuild as u16,
        }
    }

    /// `WeightWindow_Rebuild_Ranges`. A zero `weight_total` is a division by
    /// zero (SIGFPE) in C; here it fails the decode.
    fn rebuild_ranges(&mut self) -> Result<(), DecompressError> {
        if self.weight_total == 0 {
            return Err(DecompressError);
        }
        self.ranges.resize(self.weights.len() + 1, 0);
        // 0x20000 / total, stored in a uint16_t (so 1 and 2 give 0).
        let range_weight = (8 * 0x4000 / u32::from(self.weight_total)) as u16;
        let mut range_start: u16 = 0;
        for (range, &weight) in self.ranges.iter_mut().zip(&self.weights) {
            *range = range_start;
            range_start = range_start.wrapping_add(((u32::from(weight) * u32::from(range_weight)) / 8) as u16);
        }
        *self.ranges.last_mut().unwrap() = 0x4000;

        if self.thresh_increase > self.thresh_increase_cap / 2 {
            self.thresh_range_rebuild = self.weight_total.wrapping_add(self.thresh_increase_cap);
        } else {
            self.thresh_increase = self.thresh_increase.wrapping_mul(2);
            self.thresh_range_rebuild = self.weight_total.wrapping_add(self.thresh_increase);
        }
        Ok(())
    }

    /// `WeightWindow_Rebuild_Weights`.
    fn rebuild_weights(&mut self) {
        let mut total: u16 = 0;
        for weight in &mut self.weights {
            *weight /= 2;
            total = total.wrapping_add(*weight);
        }
        self.weight_total = total;

        // Drop zero weights (but never the first), filling from the end.
        let mut i = 1;
        while i < self.weights.len() {
            while i < self.weights.len() && self.weights[i] == 0 {
                self.weights.swap_remove(i);
                self.values.swap_remove(i);
            }
            i += 1;
        }

        // `max_element(weights, len, 1)`: first maximum from index 1, or 1.
        let mut it = 1;
        let mut max = 0;
        for (i, &w) in self.weights.iter().enumerate().skip(1) {
            if w > max {
                max = w;
                it = i;
            }
        }
        if it < self.weights.len() {
            let last = self.weights.len() - 1;
            self.weights.swap(it, last);
            self.values.swap(it, last);
        }

        if self.weights.len() < usize::from(self.count_cap) && self.weights[0] == 0 {
            self.weights[0] = 1;
            self.weight_total = self.weight_total.wrapping_add(1);
        }
    }

    /// `WeightWindow_Try_Decode`: `(KNOWN, value)` for a value already in the
    /// table, or `(index, 0)` for a new slot the caller fills. Failures set
    /// `decoder.error`.
    fn try_decode(&mut self, decoder: &mut Decoder) -> (usize, u16) {
        if self.weight_total >= self.thresh_range_rebuild {
            if self.thresh_range_rebuild >= self.thresh_weight_rebuild {
                self.rebuild_weights();
            }
            if self.rebuild_ranges().is_err() {
                decoder.error = true;
                return (KNOWN, 0);
            }
        }

        let value = decoder.decode(0x4000);
        // upper_bound(ranges, value) - 1, not below 0.
        let mut range = self
            .ranges
            .iter()
            .position(|&r| r > value)
            .unwrap_or(self.ranges.len() - 1);
        range = range.saturating_sub(1);
        let (Some(&lo), Some(&hi)) = (self.ranges.get(range), self.ranges.get(range + 1)) else {
            decoder.error = true;
            return (KNOWN, 0);
        };
        decoder.commit(0x4000, lo, hi.wrapping_sub(lo));

        let Some(weight) = self.weights.get_mut(range) else {
            decoder.error = true;
            return (KNOWN, 0);
        };
        *weight = weight.wrapping_add(1);
        self.weight_total = self.weight_total.wrapping_add(1);

        if range > 0 {
            return (KNOWN, self.values[range]);
        }

        if self.weights.len() >= self.ranges.len() && decoder.decode_commit(2) == 1 {
            // The count is a uint16_t argument in C.
            let extra = (self.weights.len() - self.ranges.len() + 1) as u16;
            let mut index = self.ranges.len() + usize::from(decoder.decode_commit(extra)) - 1;
            if index >= self.weights.len() {
                index = self.weights.len() - 1;
            }
            self.weights[index] = self.weights[index].wrapping_add(2);
            self.weight_total = self.weight_total.wrapping_add(2);
            return (KNOWN, self.values[index]);
        }

        self.values.push(0);
        self.weights.push(2);
        self.weight_total = self.weight_total.wrapping_add(2);
        if self.weights.len() == usize::from(self.count_cap) {
            self.weight_total = self.weight_total.wrapping_sub(self.weights[0]);
            self.weights[0] = 0;
        }
        (self.values.len() - 1, 0)
    }

    /// `Try_Decode`, then for a new slot decode its value with
    /// `Decode_Commit(max)` and remember it (the pattern repeated in
    /// `Dictionary_Decompress_Block`).
    fn decode_value(&mut self, decoder: &mut Decoder, max: impl FnOnce() -> u16) -> u16 {
        let (index, value) = self.try_decode(decoder);
        if index == KNOWN {
            return value;
        }
        let value = decoder.decode_commit(max());
        self.values[index] = value;
        value
    }
}

/// `TDictionary`: the tables of one step.
struct Dictionary {
    decoded_size: u32,
    backref_size: u32,
    decoded_value_max: u32,
    backref_value_max: u32,
    lowbit_value_max: u32,
    highbit_value_max: u32,
    lowbit_window: WeighWindow,
    highbit_window: WeighWindow,
    midbit_windows: Vec<WeighWindow>,
    decoded_windows: Vec<WeighWindow>,
    size_windows: Vec<WeighWindow>,
}

impl Dictionary {
    /// `Dictionary_Init`.
    fn new(p: &Parameter) -> Self {
        let lowbit_value_max = (p.backref_value_max + 1).min(4);
        let midbit_value_max = (p.backref_value_max / 4 + 1).min(256);
        let highbit_value_max = p.backref_value_max / 1024 + 1;

        // Size windows 0..64 take sizes_count[3 - i / 16]; the 65th takes
        // sizes_count[0].
        let size_windows = (0..65)
            .map(|i| WeighWindow::new(64, u16::from(p.sizes_count[if i < 64 { 3 - i / 16 } else { 0 }])))
            .collect();
        Dictionary {
            decoded_size: 0,
            backref_size: 0,
            decoded_value_max: p.decoded_value_max,
            backref_value_max: p.backref_value_max,
            lowbit_value_max,
            highbit_value_max,
            lowbit_window: WeighWindow::new(lowbit_value_max.wrapping_sub(1), lowbit_value_max as u16),
            highbit_window: WeighWindow::new(highbit_value_max - 1, (p.highbit_count + 1) as u16),
            midbit_windows: (0..highbit_value_max)
                .map(|_| WeighWindow::new(midbit_value_max - 1, midbit_value_max as u16))
                .collect(),
            decoded_windows: (0..4)
                .map(|_| WeighWindow::new(p.decoded_value_max.wrapping_sub(1), p.decoded_count as u16))
                .collect(),
            size_windows,
        }
    }

    /// `Dictionary_Decompress_Block`: writes a literal or a back reference at
    /// `out[pos..]`, returning the byte count (0 on failure).
    ///
    /// C picks the literal window by the output *address* modulo 4; the
    /// section buffers are `std::vector` storage (16-byte aligned), so that
    /// is the offset modulo 4.
    fn decompress_block(&mut self, dec: &mut Decoder, out: &mut [u8], pos: usize) -> usize {
        if dec.error {
            return 0;
        }

        // Always 0..=64: every stored size comes from Decode_Commit(65).
        let Some(window) = self.size_windows.get_mut(self.backref_size as usize) else {
            return 0;
        };
        self.backref_size = u32::from(window.decode_value(dec, || 65));
        if dec.error {
            return 0;
        }

        if self.backref_size > 0 {
            const SIZES: [u32; 4] = [128, 192, 256, 512];
            if self.backref_size > 64 {
                return 0;
            }
            let size = if self.backref_size < 61 {
                self.backref_size + 1
            } else {
                SIZES[(self.backref_size - 61) as usize]
            } as usize;
            let range = self.backref_value_max.min(self.decoded_size);
            if range == 0 {
                return 0;
            }

            let lowbit_max = self.lowbit_value_max as u16;
            let d3 = self.lowbit_window.decode_value(dec, || lowbit_max);
            let d4 = self.highbit_window.decode_value(dec, || (range / 1024 + 1) as u16);
            if dec.error || u32::from(d4) >= self.highbit_value_max {
                return 0;
            }
            let d5 = self.midbit_windows[usize::from(d4)].decode_value(dec, || (range / 4 + 1).min(256) as u16);
            if dec.error {
                return 0;
            }

            let offset = (u32::from(d4) << 10) + (u32::from(d5) << 2) + u32::from(d3) + 1;
            // decoded_size never exceeds pos (the dictionary starts empty at
            // each step), so pos - offset stays in the buffer.
            if offset > self.decoded_size || out.len() - pos < size {
                return 0;
            }
            self.decoded_size += size as u32;

            // memcpy of the `offset` bytes before pos, repeated.
            let offset = offset as usize;
            let src = pos - offset;
            for chunk in (0..size).step_by(offset) {
                let n = offset.min(size - chunk);
                out.copy_within(src..src + n, pos + chunk);
            }
            return size;
        }

        let decoded_max = self.decoded_value_max as u16;
        let d2 = self.decoded_windows[pos % 4].decode_value(dec, || decoded_max);
        if dec.error || pos >= out.len() {
            return 0;
        }
        out[pos] = d2 as u8;
        self.decoded_size = self.decoded_size.wrapping_add(1);
        1
    }
}

/// `gr2_decompress`: decompresses `compressed` into all of `out`, whose
/// first `step1` bytes, up to `step2`, and the rest each use their own
/// parameters.
pub fn decompress(compressed: &[u8], step1: u32, step2: u32, out: &mut [u8]) -> Result<(), DecompressError> {
    if compressed.len() < 3 * Parameter::SIZE || out.is_empty() {
        return Err(DecompressError);
    }
    if step1 > step2 || step2 as usize > out.len() {
        return Err(DecompressError);
    }
    let params: Vec<Parameter> = compressed
        .as_chunks::<{ Parameter::SIZE }>()
        .0
        .iter()
        .take(3)
        .map(Parameter::parse)
        .collect();
    let mut dec = Decoder::new(&compressed[3 * Parameter::SIZE..]);

    let steps = [step1 as usize, step2 as usize, out.len()];
    let mut pos = 0;
    for (param, &step) in params.iter().zip(&steps) {
        if pos >= step {
            continue;
        }
        let mut dic = Dictionary::new(param);
        while pos < step {
            let written = dic.decompress_block(&mut dec, out, pos);
            if written == 0 || dec.error {
                return Err(DecompressError);
            }
            pos += written;
        }
    }
    if pos == out.len() { Ok(()) } else { Err(DecompressError) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_or_inconsistent_input() {
        let mut out = [0u8; 16];
        assert!(decompress(&[], 0, 0, &mut out).is_err());
        assert!(decompress(&[0; 35], 0, 0, &mut out).is_err());
        assert!(decompress(&[0; 64], 0, 0, &mut []).is_err());
        assert!(decompress(&[0; 64], 8, 4, &mut out).is_err());
        assert!(decompress(&[0; 64], 0, 17, &mut out).is_err());
        // Parameters only: the decoder has no stream.
        assert!(decompress(&[0; 36], 0, 0, &mut out).is_err());
    }

    #[test]
    fn garbage_fails_or_fills_without_panicking() {
        // Deterministic pseudo-random inputs; any result is fine as long as
        // it terminates without panicking.
        let mut seed = 0x1234_5678u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as u8
        };
        for len in [36usize, 37, 40, 100, 1000] {
            for _ in 0..50 {
                let input: Vec<u8> = (0..len).map(|_| next()).collect();
                let mut out = vec![0u8; 4096];
                let _ = decompress(&input, 100, 2000, &mut out);
            }
        }
    }

    #[test]
    fn zero_weight_total_fails_instead_of_dividing_by_zero() {
        let mut w = WeighWindow::new(64, 4);
        w.weight_total = 0;
        assert!(w.rebuild_ranges().is_err());
    }

    #[test]
    fn rebuild_weights_drops_zero_weights_and_moves_the_largest_last() {
        let mut w = WeighWindow::new(64, 100);
        w.weights = vec![6, 1, 8, 1, 4];
        w.values = vec![0, 10, 20, 30, 40];
        w.rebuild_weights();
        // Halved: [3, 0, 4, 0, 2] -> zeros swapped out -> [3, 2, 4]; the
        // largest (4, value 20) is already last.
        assert_eq!(w.weights, [3, 2, 4]);
        assert_eq!(w.values, [0, 40, 20]);
        assert_eq!(w.weight_total, 9);
    }

    #[test]
    fn parameter_bitfields() {
        let w0: u32 = 0x1ab | (0x12345 << 9);
        let w1: u32 = 0x77 | (0x3ff << 9) | (0x1234 << 19);
        let mut b = Vec::new();
        b.extend_from_slice(&w0.to_le_bytes());
        b.extend_from_slice(&w1.to_le_bytes());
        b.extend_from_slice(&[1, 2, 3, 4]);
        let p = Parameter::parse(b.as_slice().try_into().unwrap());
        assert_eq!(
            (
                p.decoded_value_max,
                p.backref_value_max,
                p.decoded_count,
                p.highbit_count,
                p.sizes_count
            ),
            (0x1ab, 0x12345, 0x77, 0x1234, [1, 2, 3, 4])
        );
    }
}
