//! Nether biomes (Minecraft 26.x).
//!
//! The Nether uses the same multi-noise biome lookup as the overworld, with
//! its own parameter list and only two climate values: temperature and
//! vegetation, from noises without shift. The other four are 0. The Nether
//! uses the legacy random source, so these two noises are built by the old
//! `createForLegacyNetherBiome` initializer, seeded with `seed` and
//! `seed + 1`.

use super::climate::{ParameterList, TargetPoint, quantize};
use super::generated;
use super::overworld::Biome;
use crate::noise::{NoiseStack, NormalNoise, vanilla};
use crate::random::LegacyRandomSource;

/// `NoiseFunction` scale of both Nether climate noises (`xz_scale`; `y_scale`
/// is 0).
const XZ_SCALE: f64 = 0.25;

/// The two Nether climate noises of one world seed.
pub struct NetherClimate {
    temperature: NoiseStack,
    vegetation: NoiseStack,
}

impl NetherClimate {
    pub fn new(world_seed: i64) -> Self {
        // RandomState's legacy wiring: TEMPERATURE_NETHER from seed,
        // VEGETATION_NETHER from seed + 1.
        Self {
            temperature: NormalNoise::create_for_legacy_nether_biome(
                &vanilla::NETHER_TEMPERATURE,
                &mut LegacyRandomSource::new(world_seed),
            ),
            vegetation: NormalNoise::create_for_legacy_nether_biome(
                &vanilla::NETHER_VEGETATION,
                &mut LegacyRandomSource::new(world_seed.wrapping_add(1)),
            ),
        }
    }

    /// `NoiseFunction.Sampler` at a block position: `noise(x * 0.25, y * 0, z * 0.25)`.
    fn noise(noise: &NoiseStack, x: i32, y: i32, z: i32) -> f32 {
        noise.get(x as f64 * XZ_SCALE, y as f64 * 0.0, z as f64 * XZ_SCALE)
    }

    pub fn temperature(&self, x: i32, y: i32, z: i32) -> f32 {
        Self::noise(&self.temperature, x, y, z)
    }

    pub fn vegetation(&self, x: i32, y: i32, z: i32) -> f32 {
        Self::noise(&self.vegetation, x, y, z)
    }

    /// `Climate.Sampler.sample(quartX, quartY, quartZ)`.
    pub fn sample(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> TargetPoint {
        let (x, y, z) = (quart_x << 2, quart_y << 2, quart_z << 2);
        TargetPoint {
            temperature: quantize(self.temperature(x, y, z)),
            humidity: quantize(self.vegetation(x, y, z)),
            continentalness: 0,
            erosion: 0,
            depth: 0,
            weirdness: 0,
        }
    }
}

/// The `minecraft:nether` multi-noise parameter list.
pub fn nether_parameters() -> ParameterList<Biome> {
    super::overworld::parameter_list(&generated::NETHER)
}

/// Nether biome source for one seed.
pub struct NetherBiomeSource {
    climate: NetherClimate,
    parameters: ParameterList<Biome>,
}

impl NetherBiomeSource {
    pub fn new(world_seed: i64) -> Self {
        Self {
            climate: NetherClimate::new(world_seed),
            parameters: nether_parameters(),
        }
    }

    pub fn climate(&self) -> &NetherClimate {
        &self.climate
    }

    /// Forget the biome R-tree's previous result, like a fresh game thread.
    pub fn reset_search(&self) {
        self.parameters.reset();
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

/// The biomes a Nether biome source can return, in id order.
pub fn nether_biomes() -> impl Iterator<Item = Biome> {
    let mut seen: Vec<u16> = generated::NETHER.iter().map(|e| e.2).collect();
    seen.sort_unstable();
    seen.dedup();
    seen.into_iter().map(Biome)
}
