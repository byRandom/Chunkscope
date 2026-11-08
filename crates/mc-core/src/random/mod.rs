//! Minecraft's random number generators (`net.minecraft.util.RandomSource`
//! and `net.minecraft.world.level.levelgen.*`), bit-exact with Java Edition.
//!
//! - [`XoroshiroRandomSource`]: modern world generation (noise, biomes, features).
//! - [`LegacyRandomSource`]: the `java.util.Random` LCG, used for structure placement.
//! - [`WorldgenRandom`]: wrapper with the seeding helpers for features/structures.
//!
//! Gaussian sampling is intentionally not provided: Minecraft's
//! `MarsagliaPolarGaussian` uses `Math.log`, whose result depends on the CPU
//! and JVM (see docs/PARITY.md).

mod legacy;
mod support;
mod worldgen;
mod xoroshiro;

pub use legacy::{LegacyPositionalRandomFactory, LegacyRandomSource};
pub use support::{Seed128, get_seed, mix_stafford13, seed_from_hash_of, upgrade_seed_to_128bit};
pub use worldgen::{WorldgenBits, WorldgenRandom};
pub use xoroshiro::{
    Xoroshiro128PlusPlus, XoroshiroPositionalRandomFactory, XoroshiroRandomSource,
};

/// `net.minecraft.util.RandomSource` (without gaussians).
pub trait RandomSource {
    /// What `fork()` returns.
    type Fork: RandomSource;
    /// What `forkPositional()` returns.
    type Positional: PositionalRandomFactory;

    fn fork(&mut self) -> Self::Fork;
    fn fork_positional(&mut self) -> Self::Positional;
    fn set_seed(&mut self, seed: i64);

    fn next_int(&mut self) -> i32;
    /// `nextInt(int bound)`. Panics if `bound <= 0`.
    fn next_int_bounded(&mut self, bound: i32) -> i32;
    fn next_long(&mut self) -> i64;
    fn next_boolean(&mut self) -> bool;
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;

    /// `nextIntBetweenInclusive(min, max)`.
    fn next_int_between_inclusive(&mut self, min: i32, max: i32) -> i32 {
        self.next_int_bounded(max.wrapping_sub(min).wrapping_add(1))
            .wrapping_add(min)
    }

    /// `nextInt(int origin, int bound)`. Panics if `origin >= bound`.
    fn next_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        assert!(origin < bound, "bound - origin is non positive");
        origin.wrapping_add(self.next_int_bounded(bound.wrapping_sub(origin)))
    }

    /// `triangle(double mean, double spread)`.
    fn triangle(&mut self, mean: f64, spread: f64) -> f64 {
        mean + spread * (self.next_double() - self.next_double())
    }

    /// `triangle(float mean, float spread)`.
    fn triangle_f32(&mut self, mean: f32, spread: f32) -> f32 {
        mean + spread * (self.next_float() - self.next_float())
    }

    /// `consumeCount(int count)`.
    fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.next_int();
        }
    }
}

/// `net.minecraft.world.level.levelgen.PositionalRandomFactory`.
// Method names mirror the game (`fromHashOf`, `fromSeed`).
#[allow(clippy::wrong_self_convention)]
pub trait PositionalRandomFactory {
    type Source: RandomSource;

    fn at(&self, x: i32, y: i32, z: i32) -> Self::Source;
    fn from_hash_of(&self, name: &str) -> Self::Source;
    fn from_seed(&self, seed: i64) -> Self::Source;
}
