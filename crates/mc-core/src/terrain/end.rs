//! End terrain: block columns and heightmaps (`getBaseColumn` /
//! `getBaseHeight`) as the game computes them before any chunk exists.

use super::{Heightmap, Substance};
use crate::density::generated::END_FINAL_DENSITY;
use crate::density::{DensitySampler, DensityVolume, EvalContext};

/// `noise_settings/end.json`: `noise.min_y`, `noise.height`.
pub const END_MIN_Y: i32 = 0;
pub const END_HEIGHT: i32 = 128;

/// End terrain of one world seed. The End has no aquifers and its default
/// fluid is air (sea level 0), so every non-solid block is air.
pub struct EndTerrain {
    sampler: DensitySampler,
}

impl EndTerrain {
    pub fn new(world_seed: i64) -> Self {
        Self {
            sampler: DensitySampler::new(END_FINAL_DENSITY, world_seed),
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
        let volume = DensityVolume::column(x, END_MIN_Y, END_HEIGHT, z);
        let density = self.sampler.sample_volume(&ctx, &volume);
        for yi in (0..volume.size_y).rev() {
            let y = volume.block_y(yi);
            let substance = if density[yi as usize] > 0.0 {
                Substance::Solid
            } else {
                Substance::Air
            };
            if visit(y, substance) {
                return Some(y);
            }
        }
        None
    }

    /// `getBaseHeight(x, z, heightmap)`: one above the topmost block that
    /// counts for the heightmap, or the bottom of the world if none does.
    pub fn base_height(&self, x: i32, z: i32, heightmap: Heightmap) -> i32 {
        self.iterate_column(x, z, |_, s| heightmap.is_opaque(s))
            .map_or(END_MIN_Y, |y| y + 1)
    }

    /// `getBaseColumn(x, z)`: every block of the column, bottom to top.
    pub fn base_column(&self, x: i32, z: i32) -> Vec<Substance> {
        let mut column = vec![Substance::Air; END_HEIGHT as usize];
        self.iterate_column(x, z, |y, s| {
            column[(y - END_MIN_Y) as usize] = s;
            false
        });
        column
    }
}
