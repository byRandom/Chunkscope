//! The overworld climate sampler: what `RandomState.createClimateSampler`
//! compiles the `noise_router` climate functions of
//! `noise_settings/overworld.json` into, evaluated one point at a time.
//!
//! Without old-world blending, `blend_alpha` is 1 and `blend_offset` is 0, so
//! the `lerp` in `overworld/offset` returns its second argument unchanged.

use super::climate::{ParameterList, TargetPoint, quantize};
use super::generated;
use super::spline::SplineCoordinate;
use crate::noise::{NoiseStack, NormalNoise, vanilla};

/// `xz_scale` of the climate noises and of `shift_a` / `shift_b`.
const XZ_SCALE: f64 = 0.25;
/// `shift_a` / `shift_b` multiply the offset noise by this.
const SHIFT_AMPLITUDE: f32 = 4.0;

/// The six climate values of one point, before quantization.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Climate {
    pub temperature: f32,
    pub humidity: f32,
    pub continentalness: f32,
    pub erosion: f32,
    pub depth: f32,
    pub weirdness: f32,
}

impl Climate {
    /// `Climate.target(...)`.
    pub fn quantize(&self) -> TargetPoint {
        TargetPoint {
            temperature: quantize(self.temperature),
            humidity: quantize(self.humidity),
            continentalness: quantize(self.continentalness),
            erosion: quantize(self.erosion),
            depth: quantize(self.depth),
            weirdness: quantize(self.weirdness),
        }
    }
}

/// Climate noises of one overworld seed.
#[derive(Clone, Debug)]
pub struct OverworldClimate {
    offset: NoiseStack,
    temperature: NoiseStack,
    vegetation: NoiseStack,
    continentalness: NoiseStack,
    erosion: NoiseStack,
    ridge: NoiseStack,
}

impl OverworldClimate {
    pub fn new(world_seed: i64) -> Self {
        let noise = |name, params| NormalNoise::instantiate(world_seed, name, params);
        Self {
            offset: noise("minecraft:offset", &vanilla::OFFSET),
            temperature: noise("minecraft:temperature", &vanilla::TEMPERATURE),
            vegetation: noise("minecraft:vegetation", &vanilla::VEGETATION),
            continentalness: noise("minecraft:continentalness", &vanilla::CONTINENTALNESS),
            erosion: noise("minecraft:erosion", &vanilla::EROSION),
            ridge: noise("minecraft:ridge", &vanilla::RIDGE),
        }
    }

    /// `minecraft:shift_x` (`shift_a`), sliced at y = 0.
    fn shift_x(&self, x: i32, z: i32) -> f32 {
        self.offset
            .get(x as f64 * XZ_SCALE, 0.0, z as f64 * XZ_SCALE)
            * SHIFT_AMPLITUDE
    }

    /// `minecraft:shift_z` (`shift_b`): note the swapped axes.
    fn shift_z(&self, x: i32, z: i32) -> f32 {
        self.offset
            .get(z as f64 * XZ_SCALE, x as f64 * XZ_SCALE, 0.0)
            * SHIFT_AMPLITUDE
    }

    /// A `minecraft:noise` function with `shift_x`/`shift_z`, `y_scale` 0,
    /// sliced at y = 0 (`NoiseFunction.ShiftedXzSampler`).
    fn shifted(&self, noise: &NoiseStack, x: i32, z: i32) -> f32 {
        let sx = x as f64 * XZ_SCALE + self.shift_x(x, z) as f64;
        let sz = z as f64 * XZ_SCALE + self.shift_z(x, z) as f64;
        noise.get(sx, 0.0, sz)
    }

    pub fn temperature(&self, x: i32, z: i32) -> f32 {
        self.shifted(&self.temperature, x, z)
    }

    pub fn humidity(&self, x: i32, z: i32) -> f32 {
        self.shifted(&self.vegetation, x, z)
    }

    pub fn continentalness(&self, x: i32, z: i32) -> f32 {
        self.shifted(&self.continentalness, x, z)
    }

    pub fn erosion(&self, x: i32, z: i32) -> f32 {
        self.shifted(&self.erosion, x, z)
    }

    /// `minecraft:overworld/ridges` (the "weirdness" climate parameter).
    pub fn weirdness(&self, x: i32, z: i32) -> f32 {
        self.shifted(&self.ridge, x, z)
    }

    /// `minecraft:overworld/ridges_folded`:
    /// `(|(|ridges| - 0.6666667)| - 0.33333334) * -3`.
    pub fn ridges_folded(ridges: f32) -> f32 {
        ((ridges.abs() + -0.6666667).abs() + -0.33333334) * -3.0
    }

    /// `minecraft:overworld/offset` (without blending).
    pub fn offset(&self, continentalness: f32, erosion: f32, weirdness: f32) -> f32 {
        let ridges_folded = Self::ridges_folded(weirdness);
        let spline = generated::OFFSET.sample(&|c| match c {
            SplineCoordinate::Continents => continentalness,
            SplineCoordinate::Erosion => erosion,
            SplineCoordinate::RidgesFolded => ridges_folded,
        });
        spline + generated::OFFSET_ADD
    }

    /// `minecraft:overworld/depth`: a vertical gradient from 1.5 at y = -64
    /// to -1.5 at y = 320, plus the terrain offset.
    pub fn depth(y: i32, offset: f32) -> f32 {
        let gradient = 1.5 + (y.clamp(-64, 320) - -64) as f32 * -0.0078125;
        gradient + offset
    }

    /// All six climate values at a block position.
    pub fn sample_block(&self, x: i32, y: i32, z: i32) -> Climate {
        let continentalness = self.continentalness(x, z);
        let erosion = self.erosion(x, z);
        let weirdness = self.weirdness(x, z);
        Climate {
            temperature: self.temperature(x, z),
            humidity: self.humidity(x, z),
            continentalness,
            erosion,
            depth: Self::depth(y, self.offset(continentalness, erosion, weirdness)),
            weirdness,
        }
    }

    /// `Climate.Sampler.sample(quartX, quartY, quartZ)`: biomes are sampled
    /// every 4 blocks ("quart" coordinates).
    pub fn sample(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> TargetPoint {
        self.sample_block(quart_x << 2, quart_y << 2, quart_z << 2)
            .quantize()
    }
}

/// A vanilla biome, as an index into [`generated::BIOMES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Biome(pub u16);

impl Biome {
    /// The registry name, e.g. `"minecraft:plains"`.
    pub fn name(&self) -> &'static str {
        generated::BIOMES[self.0 as usize]
    }

    pub fn from_name(name: &str) -> Option<Biome> {
        generated::BIOMES
            .iter()
            .position(|b| *b == name)
            .map(|i| Biome(i as u16))
    }
}

/// The `minecraft:overworld` multi-noise biome parameter list.
pub fn overworld_parameters() -> ParameterList<Biome> {
    use super::climate::Parameter;
    let entries = generated::OVERWORLD
        .iter()
        .map(|&(params, offset, biome)| {
            let p = |i: usize| generated::PARAMETERS[params[i] as usize];
            (
                [
                    p(0),
                    p(1),
                    p(2),
                    p(3),
                    p(4),
                    p(5),
                    Parameter::new(offset, offset),
                ],
                Biome(biome),
            )
        })
        .collect();
    ParameterList::new(entries)
}

/// Overworld biome source for one seed: climate noises + biome lookup.
pub struct OverworldBiomeSource {
    climate: OverworldClimate,
    parameters: ParameterList<Biome>,
}

impl OverworldBiomeSource {
    pub fn new(world_seed: i64) -> Self {
        Self {
            climate: OverworldClimate::new(world_seed),
            parameters: overworld_parameters(),
        }
    }

    pub fn climate(&self) -> &OverworldClimate {
        &self.climate
    }

    /// `MultiNoiseBiomeSource.getNoiseBiome(quartX, quartY, quartZ, sampler)`.
    pub fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        *self
            .parameters
            .find_value(&self.climate.sample(quart_x, quart_y, quart_z))
    }

    /// The biome at a block position (block coordinates are divided by 4).
    pub fn biome_at_block(&self, x: i32, y: i32, z: i32) -> Biome {
        self.biome(x >> 2, y >> 2, z >> 2)
    }
}
