//! The dimensions the map can show, and a biome source for each.

use std::cell::RefCell;
use std::collections::HashMap;

use mc_core::biome::{Biome, EndBiomeSource, NetherBiomeSource, OverworldBiomeSource};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
}

impl Dimension {
    pub const ALL: [Dimension; 3] = [Dimension::Overworld, Dimension::Nether, Dimension::End];

    pub fn label(self) -> &'static str {
        match self {
            Dimension::Overworld => "Overworld",
            Dimension::Nether => "Nether",
            Dimension::End => "End",
        }
    }

    /// The biomes this dimension's biome source can return.
    pub fn biomes(self) -> Vec<Biome> {
        match self {
            Dimension::Overworld => mc_core::biome::overworld_biomes().collect(),
            Dimension::Nether => mc_core::biome::nether_biomes().collect(),
            Dimension::End => mc_core::biome::end_biomes().collect(),
        }
    }
}

/// The biome source of one dimension and seed.
pub enum BiomeSource {
    Overworld(OverworldBiomeSource),
    Nether(NetherBiomeSource),
    End(Box<EndBiomeSource>, RefCell<EndCache>),
}

/// The End's biome depends only on the section (16 blocks) and its island
/// density on 8-block cells: a tile reuses both.
#[derive(Default)]
pub struct EndCache {
    biomes: HashMap<(i32, i32), Biome>,
    land: HashMap<(i32, i32), bool>,
}

impl BiomeSource {
    pub fn new(dimension: Dimension, seed: i64) -> Self {
        match dimension {
            Dimension::Overworld => BiomeSource::Overworld(OverworldBiomeSource::new(seed)),
            Dimension::Nether => BiomeSource::Nether(NetherBiomeSource::new(seed)),
            Dimension::End => {
                BiomeSource::End(Box::new(EndBiomeSource::new(seed)), RefCell::default())
            }
        }
    }

    pub fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        match self {
            BiomeSource::Overworld(s) => s.biome(quart_x, quart_y, quart_z),
            BiomeSource::Nether(s) => s.biome(quart_x, quart_y, quart_z),
            BiomeSource::End(s, cache) => {
                let section = (quart_x >> 2, quart_z >> 2);
                *cache
                    .borrow_mut()
                    .biomes
                    .entry(section)
                    .or_insert_with(|| s.biome(quart_x, quart_y, quart_z))
            }
        }
    }

    pub fn reset_search(&self) {
        match self {
            BiomeSource::Overworld(s) => s.reset_search(),
            BiomeSource::Nether(s) => s.reset_search(),
            BiomeSource::End(_, cache) => *cache.borrow_mut() = EndCache::default(),
        }
    }

    /// In the End, whether the islands' density is positive at the block:
    /// the shape of the islands (their density before the 3D noise, so the
    /// edges are approximate). `None` in other dimensions.
    pub fn end_land(&self, x: i32, z: i32) -> Option<bool> {
        let BiomeSource::End(s, cache) = self else {
            return None;
        };
        // The density is constant over 8 x 8 cells (Java's truncating x / 8).
        let cell = (x / 8, z / 8);
        Some(
            *cache
                .borrow_mut()
                .land
                .entry(cell)
                .or_insert_with(|| s.island_density(x, 0, z) > 0.0),
        )
    }
}
