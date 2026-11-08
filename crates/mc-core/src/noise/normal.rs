//! `NormalNoise` (26.x) and the `NoiseStack` it builds: fractal Perlin noise
//! normalized to a target standard deviation.

use std::borrow::Cow;

use super::perlin::PerlinNoise;
use crate::random::{PositionalRandomFactory, RandomSource, XoroshiroRandomSource};

/// `NormalNoise.INPUT_FACTOR`: frequency ratio between the two Perlin
/// noises of each octave.
const INPUT_FACTOR: f64 = 1.0181268882175227;
/// `NormalNoise.TARGET_DEVIATION`.
const TARGET_DEVIATION: f64 = 0.3333333333333333;
/// `PerlinNoise.STANDARD_DEVIATION`.
const PERLIN_STANDARD_DEVIATION: f64 = 0.2702247831245211;

/// `NormalNoise.Normalization`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Normalization {
    Disabled,
    #[default]
    Enabled,
    Legacy,
}

/// `NormalNoise.Parameters`: the content of a `worldgen/noise/*.json` file.
#[derive(Clone, Debug, PartialEq)]
pub struct NoiseParameters {
    pub base_amplitude: f64,
    pub base_octave: i32,
    pub octave_count: i32,
    pub normalize: Normalization,
    /// Empty means "all 1.0". Otherwise one entry per octave; 0.0 skips it.
    pub amplitude_modifiers: Cow<'static, [f64]>,
}

/// `NormalNoise.OctaveInfo`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OctaveInfo {
    pub octave_index: i32,
    pub frequency: f64,
    pub amplitude: f64,
}

impl OctaveInfo {
    /// `OctaveInfo.seed()`: the name hashed to seed this octave's noises.
    pub fn seed(&self) -> String {
        format!("octave_{}", self.octave_index)
    }
}

/// `NormalNoise`: the noise *definition* (no seed yet). Use [`NormalNoise::create`]
/// to get a sampler.
#[derive(Clone, Debug)]
pub struct NormalNoise {
    octaves: Vec<OctaveInfo>,
    normalization_factor: f64,
}

impl NormalNoise {
    pub fn new(params: &NoiseParameters) -> Self {
        let octaves = build_octaves(
            params.base_octave,
            params.base_amplitude,
            params.octave_count,
            params.normalize != Normalization::Disabled,
            &params.amplitude_modifiers,
        );
        // The game normalizes against the (Kahan-summed) total amplitude of
        // the octaves it kept, not against `base_amplitude`.
        let total_amplitude =
            java_compat::stream::double_sum(octaves.iter().map(|o| o.amplitude.abs()));
        let mut normalization_factor = compute_normalization_factor(total_amplitude, &octaves);
        if params.normalize == Normalization::Legacy && normalization_factor != 0.0 {
            normalization_factor = compute_parity_normalization_factor(
                params.base_amplitude,
                params.octave_count,
                &params.amplitude_modifiers,
            );
        }
        Self {
            octaves,
            normalization_factor,
        }
    }

    pub fn octaves(&self) -> &[OctaveInfo] {
        &self.octaves
    }

    pub fn normalization_factor(&self) -> f64 {
        self.normalization_factor
    }

    /// `NormalNoise.create(RandomSource)`.
    pub fn create<R: RandomSource>(&self, random: &mut R) -> NoiseStack {
        let first = random.fork_positional();
        let second = random.fork_positional();
        let mut layers = Vec::with_capacity(self.octaves.len() * 2);
        for octave in &self.octaves {
            let seed = octave.seed();
            let a = PerlinNoise::new(&mut first.from_hash_of(&seed));
            let b = PerlinNoise::new(&mut second.from_hash_of(&seed));
            let amplitude = (self.normalization_factor * octave.amplitude) as f32;
            layers.push(Layer {
                noise: a,
                frequency: octave.frequency,
                amplitude,
            });
            layers.push(Layer {
                noise: b,
                frequency: octave.frequency * INPUT_FACTOR,
                amplitude,
            });
        }
        NoiseStack { layers }
    }

    /// How world generation instantiates a named noise
    /// (`Noises.instantiate` with `RandomState`'s positional factory), for
    /// worlds using the Xoroshiro algorithm (all vanilla dimensions).
    pub fn instantiate(world_seed: i64, name: &str, params: &NoiseParameters) -> NoiseStack {
        let factory = XoroshiroRandomSource::new(world_seed).fork_positional();
        Self::new(params).create(&mut factory.from_hash_of(name))
    }
}

fn amplitude_modifier(modifiers: &[f64], i: usize) -> f64 {
    if modifiers.is_empty() {
        1.0
    } else {
        modifiers[i]
    }
}

/// `NormalNoise.buildOctaves`.
fn build_octaves(
    base_octave: i32,
    base_amplitude: f64,
    octave_count: i32,
    normalize: bool,
    modifiers: &[f64],
) -> Vec<OctaveInfo> {
    let mut frequency = 2f64.powi(base_octave);
    let mut amplitude = base_amplitude;
    if normalize {
        amplitude *= 0.5f64.powi(-(octave_count - 1)) / (0.5f64.powi(-octave_count) - 1.0);
    }
    let mut octaves = Vec::with_capacity(octave_count.max(0) as usize);
    for i in 0..octave_count {
        let modifier = amplitude_modifier(modifiers, i as usize);
        if modifier != 0.0 {
            octaves.push(OctaveInfo {
                octave_index: base_octave + i,
                frequency,
                amplitude: amplitude * modifier,
            });
        }
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    octaves
}

/// `NormalNoise.computeNormalizationFactor`.
fn compute_normalization_factor(total_amplitude: f64, octaves: &[OctaveInfo]) -> f64 {
    let deviation = estimate_deviation(octaves);
    if deviation == 0.0 {
        return 0.0;
    }
    let combined = deviation * 2f64.sqrt();
    let target = total_amplitude * TARGET_DEVIATION;
    target / combined
}

/// `NormalNoise.estimateDeviation`.
fn estimate_deviation(octaves: &[OctaveInfo]) -> f64 {
    let mut sum = 0.0;
    for octave in octaves {
        let d = PERLIN_STANDARD_DEVIATION * octave.amplitude.abs();
        sum += d * d;
    }
    sum.sqrt()
}

/// `NormalNoise.computeParityNormalizationFactor`.
fn compute_parity_normalization_factor(
    base_amplitude: f64,
    octave_count: i32,
    modifiers: &[f64],
) -> f64 {
    let (mut min, mut max) = (i32::MAX, i32::MIN);
    for i in 0..octave_count {
        if amplitude_modifier(modifiers, i as usize) != 0.0 {
            min = min.min(i);
            max = max.max(i);
        }
    }
    base_amplitude * 0.5 * TARGET_DEVIATION / parity_expected_deviation(max.wrapping_sub(min))
}

/// `NormalNoise.parityExpectedDeviation`.
fn parity_expected_deviation(octave_span: i32) -> f64 {
    0.1 * (1.0 + 1.0 / (octave_span + 1) as f64)
}

/// `NoiseStack.Layer`.
#[derive(Clone, Debug)]
pub struct Layer {
    pub noise: PerlinNoise,
    pub frequency: f64,
    pub amplitude: f32,
}

/// `NoiseStack`: a weighted sum of Perlin layers, accumulated in `float`.
#[derive(Clone, Debug)]
pub struct NoiseStack {
    layers: Vec<Layer>,
}

impl NoiseStack {
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// `NoiseStack.get(double, double, double)`.
    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut sum = 0.0f32;
        for layer in &self.layers {
            let f = layer.frequency;
            sum += layer.amplitude * layer.noise.get(x * f, y * f, z * f);
        }
        sum
    }

    /// `NoiseStack.addToVolume(buffer, volume, xzScale, yScale, amplitude)`:
    /// each layer adds `amplitude * layer.amplitude * noise` to `buffer`,
    /// sampled at `xzScale * frequency` (the game multiplies the scales first;
    /// point mode multiplies the coordinate by each in turn).
    pub fn add_to_volume(
        &self,
        buffer: &mut [f32],
        volume: &crate::density::DensityVolume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        for layer in &self.layers {
            let f = layer.frequency;
            layer.noise.add_to_volume(
                buffer,
                volume,
                xz_scale * f,
                y_scale * f,
                amplitude * layer.amplitude,
            );
        }
    }

    /// `NoiseStack.get(double, double)`.
    pub fn get_2d(&self, x: f64, z: f64) -> f32 {
        let mut sum = 0.0f32;
        for layer in &self.layers {
            let f = layer.frequency;
            sum += layer.amplitude * layer.noise.get_2d(x * f, z * f);
        }
        sum
    }
}
