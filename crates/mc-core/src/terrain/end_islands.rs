//! `EndIslandFunction` (26.x): the density of the End's outer islands, from
//! a 2D simplex noise sampled on a grid of 16 blocks.

use java_compat::math::max_f32;

use crate::noise::SimplexNoise;
use crate::random::{LegacyRandomSource, RandomSource};

/// The End islands of one world seed.
#[derive(Clone, Debug)]
pub struct EndIslands {
    noise: SimplexNoise,
}

impl EndIslands {
    /// `EndIslandFunction.compileSampler`: a legacy random seeded with the
    /// world seed, 17292 values skipped, then the simplex noise (offsets 0).
    pub fn new(seed: i64) -> Self {
        let mut random = LegacyRandomSource::new(seed);
        random.consume_count(17292);
        Self {
            noise: SimplexNoise::new(&mut random, true),
        }
    }

    pub fn noise(&self) -> &SimplexNoise {
        &self.noise
    }

    /// `EndIslandFunction.getHeightValue(noise, x, z)`, in units of 8
    /// blocks: the highest island within 12 grid cells (24 units), from
    /// cells outside 64 cells of the origin whose noise is below -0.9. The
    /// main island is not part of it.
    pub fn height_value(&self, x: i32, z: i32) -> f32 {
        // Java's `/` and `%` truncate toward zero, as Rust's do.
        let (cx, cz) = (x / 2, z / 2);
        let (fx, fz) = (x % 2, z % 2);
        let mut height = -100.0f32;
        for dx in -12..=12 {
            for dz in -12..=12 {
                let gx = (cx + dx) as i64;
                let gz = (cz + dz) as i64;
                if gx * gx + gz * gz > 4096 && self.noise.get_2d(gx as f64, gz as f64) < -0.9 {
                    let size =
                        ((gx as f32).abs() * 3439.0 + (gz as f32).abs() * 147.0) % 13.0 + 9.0;
                    let ox = (fx - dx * 2) as f32;
                    let oz = (fz - dz * 2) as f32;
                    let h = 100.0 - ((ox * ox + oz * oz) as f64).sqrt() as f32 * size;
                    // Mth.clamp(h, -100, 80)
                    let h = if h < -100.0 { -100.0 } else { h.min(80.0) };
                    height = max_f32(height, h);
                }
            }
        }
        height
    }

    /// `EndIslandFunction.Sampler.sampleValue(x, y, z)`.
    pub fn sample(&self, x: i32, z: i32) -> f32 {
        (self.height_value(x / 8, z / 8) - 8.0) / 128.0
    }
}
