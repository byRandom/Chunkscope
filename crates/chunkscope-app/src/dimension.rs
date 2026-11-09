//! The dimensions the map can show, and a biome source for each.

use mc_core::biome::{Biome, NetherBiomeSource, OverworldBiomeSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dimension {
    Overworld,
    Nether,
}

impl Dimension {
    pub const ALL: [Dimension; 2] = [Dimension::Overworld, Dimension::Nether];

    pub fn label(self) -> &'static str {
        match self {
            Dimension::Overworld => "Overworld",
            Dimension::Nether => "Nether",
        }
    }

    /// The biomes this dimension's biome source can return.
    pub fn biomes(self) -> Vec<Biome> {
        match self {
            Dimension::Overworld => mc_core::biome::overworld_biomes().collect(),
            Dimension::Nether => mc_core::biome::nether_biomes().collect(),
        }
    }
}

/// The biome source of one dimension and seed.
pub enum BiomeSource {
    Overworld(OverworldBiomeSource),
    Nether(NetherBiomeSource),
}

impl BiomeSource {
    pub fn new(dimension: Dimension, seed: i64) -> Self {
        match dimension {
            Dimension::Overworld => BiomeSource::Overworld(OverworldBiomeSource::new(seed)),
            Dimension::Nether => BiomeSource::Nether(NetherBiomeSource::new(seed)),
        }
    }

    pub fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        match self {
            BiomeSource::Overworld(s) => s.biome(quart_x, quart_y, quart_z),
            BiomeSource::Nether(s) => s.biome(quart_x, quart_y, quart_z),
        }
    }

    pub fn reset_search(&self) {
        match self {
            BiomeSource::Overworld(s) => s.reset_search(),
            BiomeSource::Nether(s) => s.reset_search(),
        }
    }
}
