//! Overworld terrain: block columns and heightmaps, as the game computes them
//! before any chunk exists (`NoiseBasedChunkGenerator.getBaseColumn` /
//! `getBaseHeight`), which is what structure placement checks use.

mod aquifer;
mod end;
mod end_islands;
mod nether;

pub use aquifer::{Aquifer, AquiferRoots, FluidStatus, GlobalFluidPicker, Substance};
pub use end::{END_HEIGHT, END_MIN_Y, EndTerrain};
pub use end_islands::EndIslands;
pub use nether::{NETHER_HEIGHT, NETHER_MIN_Y, NETHER_SEA_LEVEL, NetherTerrain};

use crate::density::generated::{
    OVERWORLD_AQUIFER_BARRIER, OVERWORLD_AQUIFER_EXCLUSION, OVERWORLD_AQUIFER_FLOODEDNESS,
    OVERWORLD_AQUIFER_LAVA, OVERWORLD_AQUIFER_SPREAD, OVERWORLD_AQUIFER_SURFACE_LEVEL,
    OVERWORLD_FINAL_DENSITY,
};
use crate::density::{DensitySampler, DensityVolume, EvalContext};
use crate::random::{PositionalRandomFactory, RandomSource, XoroshiroRandomSource};

/// `noise_settings/overworld.json`: `noise.min_y`, `noise.height`, `sea_level`.
pub const MIN_Y: i32 = -64;
pub const HEIGHT: i32 = 384;
pub const SEA_LEVEL: i32 = 63;

/// `Heightmap.Types` computed during world generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Heightmap {
    /// `WORLD_SURFACE_WG`: the first non-air block from the top (water counts).
    WorldSurface,
    /// `OCEAN_FLOOR_WG`: the first motion-blocking block (fluids don't count).
    OceanFloor,
}

impl Heightmap {
    fn is_opaque(self, s: Substance) -> bool {
        match self {
            Heightmap::WorldSurface => s.is_world_surface(),
            Heightmap::OceanFloor => s.is_ocean_floor(),
        }
    }
}

/// Overworld terrain of one world seed.
pub struct OverworldTerrain {
    sampler: DensitySampler,
    aquifer_random: crate::random::XoroshiroPositionalRandomFactory,
}

impl OverworldTerrain {
    pub fn new(world_seed: i64) -> Self {
        // RandomState.getOrCreateRandomFactory("minecraft:aquifer")
        let aquifer_random = XoroshiroRandomSource::new(world_seed)
            .fork_positional()
            .from_hash_of("minecraft:aquifer")
            .fork_positional();
        Self {
            sampler: DensitySampler::new(OVERWORLD_FINAL_DENSITY, world_seed),
            aquifer_random,
        }
    }

    fn roots() -> AquiferRoots {
        AquiferRoots {
            barrier: OVERWORLD_AQUIFER_BARRIER.root,
            floodedness: OVERWORLD_AQUIFER_FLOODEDNESS.root,
            spread: OVERWORLD_AQUIFER_SPREAD.root,
            lava: OVERWORLD_AQUIFER_LAVA.root,
            exclusion: OVERWORLD_AQUIFER_EXCLUSION.root,
            surface_level: OVERWORLD_AQUIFER_SURFACE_LEVEL.root,
        }
    }

    /// `iterateNoiseColumn`: a fresh `NoiseChunk` for the column (its aquifer
    /// samples the surface level first), the final density over the column,
    /// then each block from the top down until `stop` returns true.
    fn iterate_column(
        &self,
        x: i32,
        z: i32,
        mut visit: impl FnMut(i32, Substance) -> bool,
    ) -> Option<i32> {
        let ctx = EvalContext::caching();
        let volume = DensityVolume::column(x, MIN_Y, HEIGHT, z);
        let mut aquifer = Aquifer::new(
            &self.sampler,
            &ctx,
            Self::roots(),
            self.aquifer_random,
            &volume,
            GlobalFluidPicker {
                sea_level: SEA_LEVEL,
            },
        );
        let density = self.sampler.sample_volume(&ctx, &volume);
        for yi in (0..volume.size_y).rev() {
            let y = volume.block_y(yi);
            let substance = aquifer.compute_substance(x, y, z, density[yi as usize] as f64);
            if visit(y, substance) {
                return Some(y);
            }
        }
        None
    }

    /// The aquifer a fresh `NoiseChunk` for column `(x, z)` would have, passed
    /// to `f` (it borrows that chunk's sampling context).
    pub fn with_column_aquifer<T>(&self, x: i32, z: i32, f: impl FnOnce(&mut Aquifer) -> T) -> T {
        let ctx = EvalContext::caching();
        let volume = DensityVolume::column(x, MIN_Y, HEIGHT, z);
        let mut aquifer = Aquifer::new(
            &self.sampler,
            &ctx,
            Self::roots(),
            self.aquifer_random,
            &volume,
            GlobalFluidPicker {
                sea_level: SEA_LEVEL,
            },
        );
        f(&mut aquifer)
    }

    /// `getBaseHeight(x, z, heightmap)`: one above the topmost block that
    /// counts for the heightmap, or `MIN_Y` if none does.
    pub fn base_height(&self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        self.iterate_column(x, z, |_, s| heightmap.is_opaque(s))
            .map_or(MIN_Y, |y| y + 1)
    }

    /// `getBaseColumn(x, z)`: every block of the column, bottom to top.
    pub fn base_column(&self, x: i32, z: i32) -> Vec<Substance> {
        let mut column = vec![Substance::Air; HEIGHT as usize];
        self.iterate_column(x, z, |y, s| {
            column[(y - MIN_Y) as usize] = s;
            false
        });
        column
    }
}
