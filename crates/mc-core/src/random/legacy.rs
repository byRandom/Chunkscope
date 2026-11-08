//! `LegacyRandomSource` (and `SingleThreadedRandomSource`, which is the same
//! generator without the thread check): the `java.util.Random` LCG.

use java_compat::{BitRandom, JavaRandom};

use super::support::get_seed;
use super::{PositionalRandomFactory, RandomSource};

#[derive(Clone, Debug)]
pub struct LegacyRandomSource {
    lcg: JavaRandom,
}

impl LegacyRandomSource {
    pub fn new(seed: i64) -> Self {
        Self {
            lcg: JavaRandom::new(seed),
        }
    }

    /// `next(int bits)`.
    #[inline]
    pub fn next(&mut self, bits: u32) -> i32 {
        self.lcg.next(bits)
    }
}

impl RandomSource for LegacyRandomSource {
    type Fork = Self;
    type Positional = LegacyPositionalRandomFactory;

    fn fork(&mut self) -> Self {
        Self::new(self.next_long())
    }

    fn fork_positional(&mut self) -> LegacyPositionalRandomFactory {
        LegacyPositionalRandomFactory::new(self.next_long())
    }

    fn set_seed(&mut self, seed: i64) {
        self.lcg.set_seed(seed);
    }

    fn next_int(&mut self) -> i32 {
        self.lcg.next_int()
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        self.lcg.next_int_bounded(bound)
    }

    fn next_long(&mut self) -> i64 {
        self.lcg.next_long()
    }

    fn next_boolean(&mut self) -> bool {
        self.lcg.next_boolean()
    }

    fn next_float(&mut self) -> f32 {
        self.lcg.next_float()
    }

    fn next_double(&mut self) -> f64 {
        self.lcg.next_double()
    }
}

/// `LegacyRandomSource.LegacyPositionalRandomFactory`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegacyPositionalRandomFactory {
    seed: i64,
}

impl LegacyPositionalRandomFactory {
    pub const fn new(seed: i64) -> Self {
        Self { seed }
    }
}

impl PositionalRandomFactory for LegacyPositionalRandomFactory {
    type Source = LegacyRandomSource;

    fn at(&self, x: i32, y: i32, z: i32) -> LegacyRandomSource {
        LegacyRandomSource::new(get_seed(x, y, z) ^ self.seed)
    }

    /// Uses Java's `String.hashCode`, not MD5.
    fn from_hash_of(&self, name: &str) -> LegacyRandomSource {
        LegacyRandomSource::new(java_compat::string::hash_code(name) as i64 ^ self.seed)
    }

    /// Ignores the factory seed entirely (this is what the game does).
    fn from_seed(&self, seed: i64) -> LegacyRandomSource {
        LegacyRandomSource::new(seed)
    }
}
