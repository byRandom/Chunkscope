//! World spawn (Minecraft 26.x).
//!
//! The game first picks a spawn *origin* from the climate alone
//! (`NoiseSpawnFinder`): a point near (0, 0) whose climate is inside one of
//! the `spawn_target` ranges (land, away from the most extreme weirdness).
//! That part is ported exactly. The final spawn block is then searched in
//! the chunks around the origin's chunk on fully generated chunks (trees,
//! lakes...), which Chunkscope does not generate; see [`spawn_estimate`].

use crate::biome::OverworldClimate;
use crate::biome::climate::quantize;
use crate::biome::generated::SPAWN_TARGETS;
use crate::terrain::{Heightmap, OverworldTerrain};

/// `NoiseSpawnFinder.MAX_RADIUS`, the weight of climate fitness against the
/// squared distance to (0, 0).
const MAX_RADIUS: i64 = 2048;

/// `NoiseSpawnFinder.getSpawnPositionAndFitness`: the climate is sampled at
/// the quart corner of `(x, z)`; the fitness also adds the squared distance
/// of `(x, z)` itself to (0, 0).
fn fitness(climate: &OverworldClimate, x: i32, z: i32) -> i64 {
    let (qx, qz) = ((x >> 2) << 2, (z >> 2) << 2);
    let values = [
        quantize(climate.temperature(qx, qz)),
        quantize(climate.humidity(qx, qz)),
        quantize(climate.continentalness(qx, qz)),
        quantize(climate.erosion(qx, qz)),
        quantize(climate.weirdness(qx, qz)),
    ];
    let mut best = i64::MAX;
    for target in &SPAWN_TARGETS {
        // SpawnTargetPoint.sampleFitness: sum of squared distances.
        let mut sum = 0i64;
        for (range, &v) in target.iter().zip(&values) {
            let d = range.distance(v);
            sum = sum.wrapping_add(d.wrapping_mul(d));
        }
        best = best.min(sum);
    }
    let (x, z) = (x as i64, z as i64);
    best.wrapping_mul(MAX_RADIUS * MAX_RADIUS)
        .wrapping_add(x * x + z * z)
}

/// `NoiseSpawnFinder.findSpawnPosition`: the block (x, z) the world spawn is
/// searched around. Two radial searches: rings 512 blocks apart up to 2048,
/// then rings 32 blocks apart up to 512 around the best point so far.
pub fn spawn_origin(climate: &OverworldClimate) -> (i32, i32) {
    let mut best = (0, 0);
    let mut best_fitness = fitness(climate, 0, 0);
    for (max_radius, step) in [(2048.0f32, 512.0f32), (512.0, 32.0)] {
        let (cx, cz) = best;
        let mut angle = 0.0f32;
        let mut radius = step;
        while radius <= max_radius {
            // Math.sin/cos are HotSpot intrinsics; see docs/PARITY.md.
            let x = cx + ((angle as f64).sin() * radius as f64) as i32;
            let z = cz + ((angle as f64).cos() * radius as f64) as i32;
            let f = fitness(climate, x, z);
            if f < best_fitness {
                best = (x, z);
                best_fitness = f;
            }
            angle += step / radius;
            if angle as f64 > std::f64::consts::TAU {
                angle = 0.0;
                radius += step;
            }
        }
    }
    best
}

/// `NoiseBasedChunkGenerator.getOrigin`: the chunk of [`spawn_origin`].
pub fn spawn_chunk(climate: &OverworldClimate) -> (i32, i32) {
    let (x, z) = spawn_origin(climate);
    (x >> 4, z >> 4)
}

/// An estimate of the world spawn block: the game's search
/// (`MinecraftServer.setInitialSpawn`) over the chunks around the origin's
/// chunk, in its spiral order, taking the first column whose surface is not
/// covered by a fluid, on Chunkscope's terrain.
///
/// The game runs the same search on fully generated chunks, where trees,
/// lakes, ice and other features can change the result (most often only
/// the height). Returns the block the player stands on.
pub fn spawn_estimate(climate: &OverworldClimate, terrain: &OverworldTerrain) -> (i32, i32, i32) {
    let (ocx, ocz) = spawn_chunk(climate);
    // The spiral of setInitialSpawn: 11 x 11 chunks, from the center outward.
    let (mut x, mut z, mut dx, mut dz) = (0i32, 0i32, 0i32, -1i32);
    for _ in 0..121 {
        if (-5..=5).contains(&x)
            && (-5..=5).contains(&z)
            && let Some(pos) = spawn_in_chunk(terrain, ocx + x, ocz + z)
        {
            return pos;
        }
        if x == z || (x < 0 && x == -z) || (x > 0 && x == 1 - z) {
            (dx, dz) = (-dz, dx);
        }
        x += dx;
        z += dz;
    }
    // No dry column at all: the game keeps the origin chunk's center.
    let (bx, bz) = ((ocx << 4) + 8, (ocz << 4) + 8);
    (bx, terrain.base_height(bx, bz, Heightmap::WorldSurface), bz)
}

/// `PlayerSpawnFinder.getSpawnPosInChunk` on Chunkscope's terrain: the first
/// column (x, then z) whose top is solid, not a fluid.
fn spawn_in_chunk(
    terrain: &OverworldTerrain,
    chunk_x: i32,
    chunk_z: i32,
) -> Option<(i32, i32, i32)> {
    for x in chunk_x << 4..=(chunk_x << 4) + 15 {
        for z in chunk_z << 4..=(chunk_z << 4) + 15 {
            let surface = terrain.base_height(x, z, Heightmap::WorldSurface);
            let floor = terrain.base_height(x, z, Heightmap::OceanFloor);
            if surface <= floor {
                return Some((x, surface, z));
            }
        }
    }
    None
}
