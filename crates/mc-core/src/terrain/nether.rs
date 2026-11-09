//! Nether terrain: block columns and heightmaps (`getBaseColumn` /
//! `getBaseHeight`) as the game computes them before any chunk exists.

use super::{Heightmap, Substance};
use crate::density::generated::NETHER_FINAL_DENSITY;
use crate::density::{DensitySampler, DensityVolume, EvalContext};

/// `noise_settings/nether.json`: `noise.min_y`, `noise.height`, `sea_level`.
pub const NETHER_MIN_Y: i32 = 0;
pub const NETHER_HEIGHT: i32 = 128;
pub const NETHER_SEA_LEVEL: i32 = 32;

/// Nether terrain of one world seed. The Nether has no aquifers: below the
/// lava sea level every non-solid block is lava (the global fluid picker
/// with lava as the default fluid), above it air.
pub struct NetherTerrain {
    sampler: DensitySampler,
}

impl NetherTerrain {
    pub fn new(world_seed: i64) -> Self {
        Self {
            sampler: DensitySampler::new(NETHER_FINAL_DENSITY, world_seed),
        }
    }

    /// `Aquifer.createDisabled(fluidPicker).computeSubstance`: solid where
    /// the density is positive, else the global fluid picker's fluid at `y`.
    fn substance(y: i32, density: f64) -> Substance {
        if density > 0.0 {
            Substance::Solid
        } else if y < NETHER_SEA_LEVEL {
            // The picker's `y < min(-54, seaLevel)` branch is lava too.
            Substance::Lava
        } else {
            Substance::Air
        }
    }

    /// `iterateNoiseColumn`: the final density over the column (a fresh
    /// `NoiseChunk`), then each block from the top down until `visit`
    /// returns true.
    fn iterate_column(
        &self,
        x: i32,
        z: i32,
        mut visit: impl FnMut(i32, Substance) -> bool,
    ) -> Option<i32> {
        let ctx = EvalContext::caching();
        let volume = DensityVolume::column(x, NETHER_MIN_Y, NETHER_HEIGHT, z);
        let density = self.sampler.sample_volume(&ctx, &volume);
        for yi in (0..volume.size_y).rev() {
            let y = volume.block_y(yi);
            if visit(y, Self::substance(y, density[yi as usize] as f64)) {
                return Some(y);
            }
        }
        None
    }

    /// `getBaseHeight(x, z, heightmap)`: one above the topmost block that
    /// counts for the heightmap, or the bottom of the world if none does.
    pub fn base_height(&self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        self.iterate_column(x, z, |_, s| heightmap.is_opaque(s))
            .map_or(NETHER_MIN_Y, |y| y + 1)
    }

    /// `getBaseColumn(x, z)`: every block of the column, bottom to top.
    pub fn base_column(&self, x: i32, z: i32) -> Vec<Substance> {
        let mut column = vec![Substance::Air; NETHER_HEIGHT as usize];
        self.iterate_column(x, z, |y, s| {
            column[(y - NETHER_MIN_Y) as usize] = s;
            false
        });
        column
    }
}
