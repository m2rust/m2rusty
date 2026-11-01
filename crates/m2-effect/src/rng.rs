//! The random numbers of the effect code: glibc's `random()`.
//!
//! `Platform/Base/Random.cpp` only defines its own generator on Windows; on
//! Linux `frandom` and `random_range` sit on glibc `srandom`/`random`
//! (additive feedback generator, `TYPE_3`, degree 31). The C++ keeps one
//! global stream shared by everything, so the simulation takes the stream as
//! a parameter instead: pass the same [`GlibcRandom`] to every update of
//! every effect to get the C++ sequence.

/// glibc `srandom`/`random` (`TYPE_3`).
#[derive(Debug, Clone)]
pub struct GlibcRandom {
    state: [i32; 31],
    front: usize,
    rear: usize,
}

impl Default for GlibcRandom {
    /// glibc starts as `srandom(1)`.
    fn default() -> Self {
        Self::new(1)
    }
}

impl GlibcRandom {
    /// `srandom(seed)`.
    pub fn new(seed: u32) -> Self {
        let seed = if seed == 0 { 1 } else { seed };
        let mut state = [0i32; 31];
        let mut word = seed as i32;
        state[0] = word;
        for slot in state.iter_mut().skip(1) {
            // state[i] = 16807 * state[i - 1] % 2147483647 without overflow.
            let hi = i64::from(word) / 127773;
            let lo = i64::from(word) % 127773;
            let mut w = 16807 * lo - 2836 * hi;
            if w < 0 {
                w += 2147483647;
            }
            word = w as i32;
            *slot = word;
        }
        let mut rng = Self { state, front: 3, rear: 0 };
        for _ in 0..310 {
            rng.random();
        }
        rng
    }

    /// `random()`: uniform in `[0, 2^31 - 1]`.
    pub fn random(&mut self) -> u32 {
        let value = self.state[self.front].wrapping_add(self.state[self.rear]);
        self.state[self.front] = value;
        let result = (value as u32) >> 1;
        self.front = (self.front + 1) % 31;
        self.rear = (self.rear + 1) % 31;
        result
    }

    /// `frandom(low, high)`: a float in `[low, high)`.
    pub fn frandom(&mut self, low: f32, high: f32) -> f32 {
        let fl = self.random() as f32 / 2147483648.0f32;
        fl * (high - low) + low
    }

    /// `random_range(from, to)`: an integer in `[from, to]`.
    pub fn random_range(&mut self, from: i64, to: i64) -> i64 {
        (i64::from(self.random()) % (to - from + 1)) + from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_glibc() {
        // The well-known first values of glibc's rand()/random() for seed 1.
        let mut rng = GlibcRandom::new(1);
        assert_eq!(rng.random(), 1804289383);
        assert_eq!(rng.random(), 846930886);
        assert_eq!(rng.random(), 1681692777);
        assert_eq!(rng.random(), 1714636915);
        // Seed 0 is seed 1.
        assert_eq!(GlibcRandom::new(0).random(), 1804289383);
    }

    #[test]
    fn range_is_inclusive() {
        let mut rng = GlibcRandom::new(7);
        let mut seen = [false; 4];
        for _ in 0..200 {
            seen[rng.random_range(0, 3) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }
}
