//! One dimension of one world seed: its biome source, its terrain and the
//! level heights the game uses there, behind one interface.
//!
//! Everything that depends on the dimension lives here, so callers (the
//! seed finder, the map, the examples) do not switch on it themselves.

use crate::biome::{
    Biome, EndBiomeSource, NetherBiomeSource, OverworldBiomeSource, end_biomes, nether_biomes,
    overworld_biomes,
};
use crate::structure::stronghold::ring_positions;
use crate::structure::validate::{Structure, StructureGenerator};
use crate::structure::{STRUCTURE_SETS, StructureSet, StructureSets};
use crate::terrain::{EndTerrain, Heightmap, NetherTerrain, OverworldTerrain, Substance};

/// A dimension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dimension {
    #[default]
    Overworld,
    Nether,
    End,
}

/// The heights of a dimension: its level (`dimension_type`) and the range
/// its terrain is generated in (the noise settings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    /// The level's lowest block (`LevelHeightAccessor.getMinY`).
    pub min_y: i32,
    /// The level's height in blocks.
    pub height: i32,
    pub sea_level: i32,
    /// `WorldGenerationContext.getMinGenY`
    pub gen_min_y: i32,
    /// `WorldGenerationContext.getGenDepth`
    pub gen_height: i32,
}

impl Dimension {
    pub const ALL: [Dimension; 3] = [Dimension::Overworld, Dimension::Nether, Dimension::End];

    /// Stable name (`overworld`, `nether`, `end`), for files and the
    /// command line.
    pub fn id(self) -> &'static str {
        match self {
            Dimension::Overworld => "overworld",
            Dimension::Nether => "nether",
            Dimension::End => "end",
        }
    }

    pub fn from_id(id: &str) -> Option<Dimension> {
        Dimension::ALL.into_iter().find(|d| d.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            Dimension::Overworld => "Overworld",
            Dimension::Nether => "Nether",
            Dimension::End => "End",
        }
    }

    pub fn level(self) -> Level {
        use crate::terrain as t;
        match self {
            Dimension::Overworld => Level {
                min_y: t::MIN_Y,
                height: t::HEIGHT,
                sea_level: t::SEA_LEVEL,
                gen_min_y: t::MIN_Y,
                gen_height: t::HEIGHT,
            },
            // `dimension_type/the_nether.json`: y 0 to 255; terrain from 0
            // to 127, with the lava sea at 32.
            Dimension::Nether => Level {
                min_y: 0,
                height: 256,
                sea_level: t::NETHER_SEA_LEVEL,
                gen_min_y: t::NETHER_MIN_Y,
                gen_height: t::NETHER_HEIGHT,
            },
            // `dimension_type/the_end.json`: y 0 to 255; terrain from 0 to
            // 127, sea level 0.
            Dimension::End => Level {
                min_y: 0,
                height: 256,
                sea_level: 0,
                gen_min_y: t::END_MIN_Y,
                gen_height: t::END_HEIGHT,
            },
        }
    }

    /// The biomes this dimension's biome source can return, in id order.
    pub fn biomes(self) -> Vec<Biome> {
        match self {
            Dimension::Overworld => overworld_biomes().collect(),
            Dimension::Nether => nether_biomes().collect(),
            Dimension::End => end_biomes().collect(),
        }
    }

    /// Whether some structure of `set` can generate in this dimension (one
    /// of its biomes is here). Ruined portals are in two dimensions.
    pub fn has_set(self, set: &StructureSet) -> bool {
        let here = self.biomes();
        set.structures.iter().any(|(name, _)| {
            Structure::find(name).is_some_and(|s| {
                s.biomes
                    .iter()
                    .any(|b| Biome::from_name(b).is_some_and(|b| here.contains(&b)))
            })
        })
    }
}

enum Sources {
    Overworld(OverworldBiomeSource, OverworldTerrain),
    Nether(NetherBiomeSource, NetherTerrain),
    End(Box<EndBiomeSource>, EndTerrain),
}

/// One dimension of one world seed.
pub struct World {
    seed: i64,
    dimension: Dimension,
    sources: Sources,
}

impl World {
    /// Builds the dimension's biome source and terrain (their noises): do
    /// it once per seed and dimension, not per query.
    pub fn new(seed: i64, dimension: Dimension) -> Self {
        let sources = match dimension {
            Dimension::Overworld => {
                Sources::Overworld(OverworldBiomeSource::new(seed), OverworldTerrain::new(seed))
            }
            Dimension::Nether => {
                Sources::Nether(NetherBiomeSource::new(seed), NetherTerrain::new(seed))
            }
            Dimension::End => {
                Sources::End(Box::new(EndBiomeSource::new(seed)), EndTerrain::new(seed))
            }
        };
        Self {
            seed,
            dimension,
            sources,
        }
    }

    pub fn seed(&self) -> i64 {
        self.seed
    }

    pub fn dimension(&self) -> Dimension {
        self.dimension
    }

    pub fn level(&self) -> Level {
        self.dimension.level()
    }

    /// `getNoiseBiome(quartX, quartY, quartZ)`
    pub fn biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> Biome {
        match &self.sources {
            Sources::Overworld(b, _) => b.biome(quart_x, quart_y, quart_z),
            Sources::Nether(b, _) => b.biome(quart_x, quart_y, quart_z),
            Sources::End(b, _) => b.biome(quart_x, quart_y, quart_z),
        }
    }

    /// The biome at a block (its quart cell).
    pub fn biome_at_block(&self, x: i32, y: i32, z: i32) -> Biome {
        self.biome(x >> 2, y >> 2, z >> 2)
    }

    /// `ChunkGenerator.getBaseHeight`: the first free block above the
    /// terrain at a column.
    pub fn base_height(&self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        match &self.sources {
            Sources::Overworld(_, t) => t.base_height(x, z, heightmap),
            Sources::Nether(_, t) => t.base_height(x, z, heightmap),
            Sources::End(_, t) => t.base_height(x, z, heightmap),
        }
    }

    /// `ChunkGenerator.getBaseColumn`: the column's blocks from
    /// `level().gen_min_y` up, before features.
    pub fn base_column(&self, x: i32, z: i32) -> Vec<Substance> {
        match &self.sources {
            Sources::Overworld(_, t) => t.base_column(x, z),
            Sources::Nether(_, t) => t.base_column(x, z),
            Sources::End(_, t) => t.base_column(x, z),
        }
    }

    /// Stronghold chunks (`ring_positions` of the `strongholds` set): all
    /// of them in the Overworld, none elsewhere. Slow (a biome search per
    /// stronghold, about a second in all).
    pub fn strongholds(&self) -> Vec<(i32, i32)> {
        let Sources::Overworld(biomes, _) = &self.sources else {
            return Vec::new();
        };
        STRUCTURE_SETS
            .find_set("minecraft:strongholds")
            .and_then(|s| s.concentric_rings())
            .map(|rings| ring_positions(self.seed, &rings, biomes))
            .unwrap_or_default()
    }

    /// Structure validation in this dimension.
    pub fn structures(&self) -> StructureGenerator<'_> {
        StructureGenerator::new(self)
    }
}
