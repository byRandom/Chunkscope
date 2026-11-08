//! `BlendedNoise` (`minecraft:old_blended_noise`): the overworld's base 3D
//! terrain noise, built from three stacks of smeared Perlin noise.

use super::normal::{Layer, NoiseStack};
use super::perlin::PerlinNoise;
use crate::random::{PositionalRandomFactory, RandomSource, XoroshiroRandomSource};

/// `BlendedNoise.NOISE_SEED`: the name its random source is derived from.
pub const NOISE_SEED: &str = "minecraft:terrain";
/// `BlendedNoise.BASE_SCALE`.
const BASE_SCALE: f64 = 684.412;

/// The parameters of an `old_blended_noise` density function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlendedNoiseParams {
    pub xz_scale: f64,
    pub y_scale: f64,
    pub xz_factor: f64,
    pub y_factor: f64,
    pub smear_scale_multiplier: f64,
}

impl BlendedNoiseParams {
    /// `xzMultiplier()`.
    pub fn xz_multiplier(&self) -> f64 {
        BASE_SCALE * self.xz_scale
    }

    /// `yMultiplier()`.
    pub fn y_multiplier(&self) -> f64 {
        BASE_SCALE * self.y_scale
    }
}

/// `BlendedNoise.FbmSet`.
#[derive(Clone, Debug)]
pub struct FbmSet {
    pub min_limit: NoiseStack,
    pub max_limit: NoiseStack,
    pub main: NoiseStack,
}

impl FbmSet {
    /// `BlendedNoise.createFbmSet(random)`: the three stacks, drawn in this
    /// order from the same random source.
    pub fn new(params: &BlendedNoiseParams, random: &mut impl RandomSource) -> Self {
        let smear = params.y_multiplier() * params.smear_scale_multiplier;
        let main_smear = smear / params.y_factor;
        let min_limit = create_fbm(random, -15, smear, 0.9999847412109375);
        let max_limit = create_fbm(random, -15, smear, 0.9999847412109375);
        let main = create_fbm(random, -7, main_smear, 12.75);
        Self {
            min_limit,
            max_limit,
            main,
        }
    }

    /// The set a world with this seed uses (the Xoroshiro algorithm, like
    /// every vanilla dimension): seeded from `minecraft:terrain`.
    pub fn for_world(params: &BlendedNoiseParams, world_seed: i64) -> Self {
        let factory = XoroshiroRandomSource::new(world_seed).fork_positional();
        Self::new(params, &mut factory.from_hash_of(NOISE_SEED))
    }
}

/// `BlendedNoise.createFbm(random, firstOctave, smearScale, amplitudeSum)`:
/// octaves from frequency 1 down, each a smeared Perlin noise whose smear
/// shrinks with its frequency; amplitudes double per octave and sum to
/// `amplitude_sum`.
pub fn create_fbm(
    random: &mut impl RandomSource,
    first_octave: i32,
    smear_scale: f64,
    amplitude_sum: f64,
) -> NoiseStack {
    assert!(first_octave <= 0, "firstOctave>0");
    let octaves = -first_octave + 1;
    let mut frequency = 1.0f64;
    let mut amplitude = amplitude_sum / (2f64.powi(octaves) - 1.0);
    let mut layers = Vec::with_capacity(octaves as usize);
    for _ in 0..octaves {
        let noise = PerlinNoise::smeared(random, smear_scale * frequency);
        layers.push(Layer {
            noise,
            frequency,
            amplitude: amplitude as f32,
        });
        frequency /= 2.0;
        amplitude *= 2.0;
    }
    NoiseStack::from_layers(layers)
}
