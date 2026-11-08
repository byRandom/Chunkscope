//! `Xoroshiro128PlusPlus`, `XoroshiroRandomSource` and its positional factory.

use super::support::{
    GOLDEN_RATIO_64, SILVER_RATIO_64, Seed128, get_seed, seed_from_hash_of, upgrade_seed_to_128bit,
};
use super::{PositionalRandomFactory, RandomSource};

const FLOAT_UNIT: f32 = 1.0 / (1u32 << 24) as f32; // 5.9604645E-8f
const DOUBLE_UNIT: f64 = 1.0 / (1u64 << 53) as f64; // 1.1102230246251565E-16

/// The raw xoroshiro128++ generator, with Minecraft's all-zero-state fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Xoroshiro128PlusPlus {
    lo: i64,
    hi: i64,
}

impl Xoroshiro128PlusPlus {
    pub const fn new(lo: i64, hi: i64) -> Self {
        if lo | hi == 0 {
            Self {
                lo: GOLDEN_RATIO_64,
                hi: SILVER_RATIO_64,
            }
        } else {
            Self { lo, hi }
        }
    }

    pub const fn from_seed128(seed: Seed128) -> Self {
        Self::new(seed.lo, seed.hi)
    }

    #[inline]
    pub fn next_long(&mut self) -> i64 {
        let (l, mut m) = (self.lo, self.hi);
        let n = l.wrapping_add(m).rotate_left(17).wrapping_add(l);
        m ^= l;
        self.lo = l.rotate_left(49) ^ m ^ (m << 21);
        self.hi = m.rotate_left(28);
        n
    }
}

/// `XoroshiroRandomSource`.
#[derive(Clone, Debug)]
pub struct XoroshiroRandomSource {
    rng: Xoroshiro128PlusPlus,
}

impl XoroshiroRandomSource {
    /// `new XoroshiroRandomSource(long seed)`.
    pub const fn new(seed: i64) -> Self {
        Self::from_seed128(upgrade_seed_to_128bit(seed))
    }

    /// `new XoroshiroRandomSource(long lo, long hi)` (no mixing).
    pub const fn from_parts(lo: i64, hi: i64) -> Self {
        Self {
            rng: Xoroshiro128PlusPlus::new(lo, hi),
        }
    }

    pub const fn from_seed128(seed: Seed128) -> Self {
        Self::from_parts(seed.lo, seed.hi)
    }

    /// `nextBits(int bits)`: the top `bits` bits of the next long.
    #[inline]
    fn next_bits(&mut self, bits: u32) -> u64 {
        (self.rng.next_long() as u64) >> (64 - bits)
    }
}

impl RandomSource for XoroshiroRandomSource {
    type Fork = Self;
    type Positional = XoroshiroPositionalRandomFactory;

    fn fork(&mut self) -> Self {
        let lo = self.rng.next_long();
        let hi = self.rng.next_long();
        Self::from_parts(lo, hi)
    }

    fn fork_positional(&mut self) -> XoroshiroPositionalRandomFactory {
        let lo = self.rng.next_long();
        let hi = self.rng.next_long();
        XoroshiroPositionalRandomFactory { lo, hi }
    }

    fn set_seed(&mut self, seed: i64) {
        *self = Self::new(seed);
    }

    #[inline]
    fn next_int(&mut self) -> i32 {
        self.rng.next_long() as i32
    }

    /// Lemire's multiply-shift with rejection (not the LCG algorithm).
    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "Bound must be positive");
        let bound64 = bound as i64;
        let mut m = (self.next_int() as u32 as i64) * bound64;
        let mut low = m & 0xFFFF_FFFF;
        if low < bound64 {
            // Integer.remainderUnsigned(-bound, bound)
            let threshold = (bound.wrapping_neg() as u32 % bound as u32) as i64;
            while low < threshold {
                m = (self.next_int() as u32 as i64) * bound64;
                low = m & 0xFFFF_FFFF;
            }
        }
        (m >> 32) as i32
    }

    #[inline]
    fn next_long(&mut self) -> i64 {
        self.rng.next_long()
    }

    fn next_boolean(&mut self) -> bool {
        self.rng.next_long() & 1 != 0
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * FLOAT_UNIT
    }

    fn next_double(&mut self) -> f64 {
        self.next_bits(53) as f64 * DOUBLE_UNIT
    }

    fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.rng.next_long();
        }
    }
}

/// `XoroshiroRandomSource.XoroshiroPositionalRandomFactory`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct XoroshiroPositionalRandomFactory {
    lo: i64,
    hi: i64,
}

impl XoroshiroPositionalRandomFactory {
    pub const fn new(lo: i64, hi: i64) -> Self {
        Self { lo, hi }
    }
}

impl PositionalRandomFactory for XoroshiroPositionalRandomFactory {
    type Source = XoroshiroRandomSource;

    fn at(&self, x: i32, y: i32, z: i32) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_parts(get_seed(x, y, z) ^ self.lo, self.hi)
    }

    fn from_hash_of(&self, name: &str) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_seed128(seed_from_hash_of(name).xor(self.lo, self.hi))
    }

    fn from_seed(&self, seed: i64) -> XoroshiroRandomSource {
        XoroshiroRandomSource::from_parts(seed ^ self.lo, seed ^ self.hi)
    }
}
