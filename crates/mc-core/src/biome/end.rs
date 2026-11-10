//! `TheEndBiomeSource` (26.x): the main island within 64 chunks of the
//! origin; elsewhere one of four biomes by the End islands' density at the
//! center of the chunk.

use super::Biome;
use crate::density::generated::END_EROSION;
use crate::density::{DensitySampler, EvalContext};

/// The End's biomes, in `TheEndBiomeSource` order.
pub const END_BIOME_NAMES: [&str; 5] = [
    "minecraft:the_end",
    "minecraft:end_highlands",
    "minecraft:end_midlands",
    "minecraft:small_end_islands",
    "minecraft:end_barrens",
];

/// The End's biomes.
pub fn end_biomes() -> impl Iterator<Item = Biome> {
    END_BIOME_NAMES
        .iter()
        .map(|n| Biome::from_name(n).expect("End biome in the table"))
}

/// The End's biome source for one world seed.
pub struct EndBiomeSource {
    /// The noise router's `erosion`: the islands (`end/islands`).
    erosion: DensitySampler,
    biomes: [Biome; 5],
}

impl EndBiomeSource {
    pub fn new(seed: i64) -> Self {
        let mut biomes = end_biomes();
        Self {
            erosion: DensitySampler::new(END_EROSION, seed),
            biomes: std::array::from_fn(|_| biomes.next().expect("five End biomes")),
        }
    }

    /// The islands' density (the erosion sampler) at a block.
    pub fn island_density(&self, x: i32, y: i32, z: i32) -> f32 {
        let root = self.erosion.tree().root;
        self.erosion
            .sample_value(&EvalContext::uncached(), root, x, y, z)
    }

    /// `TheEndBiomeSource.getNoiseBiome(quartX, quartY, quartZ, sampler)`.
    pub fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        let [the_end, highlands, midlands, islands, barrens] = self.biomes;
        // QuartPos.toBlock, then SectionPos.blockToSectionCoord.
        let (bx, by, bz) = (quart_x << 2, quart_y << 2, quart_z << 2);
        let (sx, sz) = (bx >> 4, bz >> 4);
        if (sx as i64) * (sx as i64) + (sz as i64) * (sz as i64) <= 4096 {
            return the_end;
        }
        // The section's center block.
        let x = sx.wrapping_mul(2).wrapping_add(1).wrapping_mul(8);
        let z = sz.wrapping_mul(2).wrapping_add(1).wrapping_mul(8);
        let density = self.island_density(x, by, z) as f64;
        if density > 0.25 {
            highlands
        } else if density >= -0.0625 {
            midlands
        } else if density < -0.21875 {
            islands
        } else {
            barrens
        }
    }
}
