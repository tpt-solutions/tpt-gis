//! Minimal, dependency-free PRNG used only for deterministic test-fixture
//! generation (the `generate-fixtures` subcommand and benchmarks).
//!
//! State is `xoshiro256**`; the 64-bit seed is expanded with `splitmix64`.
//! This replaces the external `rand` crate for our narrow use case.

#[doc(hidden)]
pub struct SmallRng {
    s: [u64; 4],
}

impl SmallRng {
    pub fn seed_from_u64(seed: u64) -> Self {
        let mut sm = SplitMix64 { state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15) };
        SmallRng { s: [sm.next(), sm.next(), sm.next(), sm.next()] }
    }

    /// Uniform `f64` in `[low, high)`.
    pub fn gen_range(&mut self, low: f64, high: f64) -> f64 {
        let unit = self.next_u64() as f64 / (u64::MAX as f64 + 1.0);
        low + unit * (high - low)
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.s[0].wrapping_add(self.s[3]).rotate_left(23).wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::SmallRng;

    #[test]
    fn deterministic_per_seed() {
        let mut a = SmallRng::seed_from_u64(42);
        let mut b = SmallRng::seed_from_u64(42);
        for _ in 0..100 {
            assert_eq!(a.gen_range(-1000.0, 1000.0), b.gen_range(-1000.0, 1000.0));
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = SmallRng::seed_from_u64(1);
        let mut b = SmallRng::seed_from_u64(2);
        assert_ne!(a.gen_range(0.0, 1.0), b.gen_range(0.0, 1.0));
    }

    #[test]
    fn stays_in_range() {
        let mut r = SmallRng::seed_from_u64(7);
        for _ in 0..10_000 {
            let v = r.gen_range(-900.0, 900.0);
            assert!((-900.0..900.0).contains(&v));
        }
    }
}
