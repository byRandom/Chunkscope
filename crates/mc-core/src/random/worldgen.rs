//! `WorldgenRandom`: wraps another source and provides the seeding helpers
//! used by features and structures.

use java_compat::BitRandom;

use super::{LegacyRandomSource, RandomSource, XoroshiroRandomSource};

/// How `WorldgenRandom.next(bits)` pulls bits from the wrapped source.
pub trait WorldgenBits: RandomSource {
    fn worldgen_next(&mut self, bits: u32) -> i32;
}

impl WorldgenBits for LegacyRandomSource {
    /// Delegates to the LCG's own `next(bits)`.
    fn worldgen_next(&mut self, bits: u32) -> i32 {
        self.next(bits)
    }
}

impl WorldgenBits for XoroshiroRandomSource {
    /// `(int)(nextLong() >>> (64 - bits))`.
    fn worldgen_next(&mut self, bits: u32) -> i32 {
        ((self.next_long() as u64) >> (64 - bits)) as i32
    }
}

/// `WorldgenRandom`. All `next*` methods use the `java.util.Random`
/// algorithms on top of [`WorldgenBits::worldgen_next`], whatever the inner
/// source is. `fork`/`fork_positional`/`set_seed` go straight to the inner
/// source.
#[derive(Clone, Debug)]
pub struct WorldgenRandom<R> {
    inner: R,
    count: i32,
}

impl<R: WorldgenBits> WorldgenRandom<R> {
    pub fn new(inner: R) -> Self {
        Self { inner, count: 0 }
    }

    /// Number of `next(bits)` calls so far (`getCount()`).
    pub fn count(&self) -> i32 {
        self.count
    }

    pub fn inner(&self) -> &R {
        &self.inner
    }

    /// `setDecorationSeed(long levelSeed, int blockX, int blockZ)`.
    pub fn set_decoration_seed(&mut self, level_seed: i64, block_x: i32, block_z: i32) -> i64 {
        self.set_seed(level_seed);
        let a = self.next_long() | 1;
        let b = self.next_long() | 1;
        let seed = (block_x as i64)
            .wrapping_mul(a)
            .wrapping_add((block_z as i64).wrapping_mul(b))
            ^ level_seed;
        self.set_seed(seed);
        seed
    }

    /// `setFeatureSeed(long decorationSeed, int index, int step)`.
    pub fn set_feature_seed(&mut self, decoration_seed: i64, index: i32, step: i32) {
        // `10000 * step` is int arithmetic in Java
        let seed = decoration_seed
            .wrapping_add(index as i64)
            .wrapping_add(10000i32.wrapping_mul(step) as i64);
        self.set_seed(seed);
    }

    /// `setLargeFeatureSeed(long seed, int chunkX, int chunkZ)`.
    pub fn set_large_feature_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(seed);
        let a = self.next_long();
        let b = self.next_long();
        let s = (chunk_x as i64).wrapping_mul(a) ^ (chunk_z as i64).wrapping_mul(b) ^ seed;
        self.set_seed(s);
    }

    /// `setLargeFeatureWithSalt(long seed, int regionX, int regionZ, int salt)`.
    /// This is how structure placement seeds its RNG.
    pub fn set_large_feature_with_salt(
        &mut self,
        seed: i64,
        region_x: i32,
        region_z: i32,
        salt: i32,
    ) {
        let s = (region_x as i64)
            .wrapping_mul(341873128712)
            .wrapping_add((region_z as i64).wrapping_mul(132897987541))
            .wrapping_add(seed)
            .wrapping_add(salt as i64);
        self.set_seed(s);
    }
}

/// Adapter so the `java.util.Random` algorithms run on top of `worldgen_next`.
struct Bits<'a, R>(&'a mut WorldgenRandom<R>);

impl<R: WorldgenBits> BitRandom for Bits<'_, R> {
    fn next(&mut self, bits: u32) -> i32 {
        self.0.count = self.0.count.wrapping_add(1);
        self.0.inner.worldgen_next(bits)
    }
}

impl<R: WorldgenBits> RandomSource for WorldgenRandom<R> {
    type Fork = R::Fork;
    type Positional = R::Positional;

    fn fork(&mut self) -> R::Fork {
        self.inner.fork()
    }

    fn fork_positional(&mut self) -> R::Positional {
        self.inner.fork_positional()
    }

    fn set_seed(&mut self, seed: i64) {
        self.inner.set_seed(seed);
    }

    fn next_int(&mut self) -> i32 {
        Bits(self).next_int()
    }

    fn next_int_bounded(&mut self, bound: i32) -> i32 {
        Bits(self).next_int_bounded(bound)
    }

    fn next_long(&mut self) -> i64 {
        Bits(self).next_long()
    }

    fn next_boolean(&mut self) -> bool {
        Bits(self).next_boolean()
    }

    fn next_float(&mut self) -> f32 {
        Bits(self).next_float()
    }

    fn next_double(&mut self) -> f64 {
        Bits(self).next_double()
    }
}
