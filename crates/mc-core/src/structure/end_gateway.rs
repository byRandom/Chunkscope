//! End gateways: the 20 that appear around the main island, one after each
//! dragon defeat (`EnderDragonFight`).
//!
//! Their positions are the same in every world: points on a circle of radius
//! 96 around (0, 0), at height 75. The seed decides only the order they
//! appear in: a shuffle of 0 to 19 with the game's single-threaded random
//! source (the `java.util.Random` generator) seeded with the world seed.

use crate::biome::Biome;
use crate::random::{LegacyRandomSource, RandomSource, WorldgenRandom, XoroshiroRandomSource};
use crate::structure::end_city::end_city_pieces;
use crate::structure::scan::{ChunkRect, SetFilter};
use crate::structure::{STRUCTURE_SETS, StructureSets};
use crate::terrain::Heightmap;
use crate::world::{Dimension, World};

/// How many gateways the game places.
pub const GATEWAY_COUNT: usize = 20;
/// Height of every gateway (the block of the gateway itself).
pub const GATEWAY_Y: i32 = 75;

/// The block `(x, z)` of gateway `index` (0 to 19):
/// `floor(96 * cos(a))`, `floor(96 * sin(a))` with
/// `a = 2 * (-π + (π / 20) * index)`.
///
/// A table, not the formula: `x` and `z` of some gateways are a hair above
/// or below a whole number (`cos` of a right angle is `±6e-17`), so the sign
/// of the last bit decides the block, and a platform's `cos` must not. A test
/// checks the table against the formula and the game's fixture.
pub fn gateway_position(index: usize) -> (i32, i32) {
    POSITIONS[index]
}

const POSITIONS: [(i32, i32); GATEWAY_COUNT] = [
    (96, 0),
    (91, 29),
    (77, 56),
    (56, 77),
    (29, 91),
    (-1, 96),
    (-30, 91),
    (-57, 77),
    (-78, 56),
    (-92, 29),
    (-96, -1),
    (-92, -30),
    (-78, -57),
    (-57, -78),
    (-30, -92),
    (0, -96),
    (29, -92),
    (56, -78),
    (77, -57),
    (91, -30),
];

/// The order the gateways appear in: element `n` is the index (into
/// [`gateway_position`]) of the gateway the `n + 1`th dragon defeat opens.
/// `Util.shuffle` of `0..20` with `RandomSource.createThreadLocalInstance(seed)`.
pub fn gateway_order(seed: i64) -> [u8; GATEWAY_COUNT] {
    let mut order: [u8; GATEWAY_COUNT] = std::array::from_fn(|i| i as u8);
    let mut random = LegacyRandomSource::new(seed);
    // Util.shuffle: for i from the size down to 2, swap element i - 1 with
    // a random one among the first i.
    for i in (2..=GATEWAY_COUNT).rev() {
        let j = random.next_int_bounded(i as i32) as usize;
        order.swap(i - 1, j);
    }
    order
}

/// A gateway of a world: its block, and which dragon defeat opens it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gateway {
    pub x: i32,
    pub z: i32,
    /// 1 for the first gateway (after the first dragon defeat), up to 20.
    pub number: u8,
}

/// The 20 gateways of a world, in the order they appear.
pub fn gateways(seed: i64) -> Vec<Gateway> {
    gateway_order(seed)
        .iter()
        .enumerate()
        .map(|(n, &i)| {
            let (x, z) = gateway_position(i as usize);
            Gateway {
                x,
                z,
                number: n as u8 + 1,
            }
        })
        .collect()
}

/// The generation step (`SURFACE_STRUCTURES`) and the index within it the game
/// gives the `end_gateway_return` placed feature. Checked against the
/// game's own sorting by the fixture (`feature 4 0`).
pub const RETURN_STEP: i32 = 4;
pub const RETURN_INDEX: i32 = 0;

/// A gateway that leads back from the outer islands to the main island.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReturnGateway {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// The bounding box of a piece of an end city covers this column. The
    /// city's blocks are placed before the gateway and can raise the
    /// surface it is placed on, so `y` is then the least it can be. `x` and
    /// `z` are exact either way.
    pub under_city: bool,
}

/// The return gateway the End's outer islands get in chunk `(cx, cz)`, if
/// any: the placed feature `end_gateway_return`, as the game decorates a
/// chunk.
///
/// The game seeds a `WorldgenRandom` with the decoration seed of the chunk's
/// corner and the feature's step and index, then applies the feature's
/// placement: one chunk in 700 (`RarityFilter`), a random block of the chunk
/// (`InSquarePlacement`), the terrain surface there
/// (`WORLD_SURFACE_WG`, nothing where the column is empty), 3 to 9 blocks
/// above it, and only where the biome has the feature (End Highlands).
///
/// `world` must be an End world. `y` is the height of the terrain as
/// generated; an end city placed earlier in the chunk can raise the surface
/// (see [`ReturnGateway::under_city`]).
pub fn return_gateway(world: &World, cx: i32, cz: i32) -> Option<ReturnGateway> {
    debug_assert_eq!(world.dimension(), Dimension::End);
    let mut random = WorldgenRandom::new(XoroshiroRandomSource::new(0));
    let decoration = random.set_decoration_seed(world.seed(), cx * 16, cz * 16);
    random.set_feature_seed(decoration, RETURN_INDEX, RETURN_STEP);
    // RarityFilter.onAverageOnceEvery(700)
    if random.next_float() >= 1.0f32 / 700.0f32 {
        return None;
    }
    // InSquarePlacement.spread()
    let x = random.next_int_bounded(16) + cx * 16;
    let z = random.next_int_bounded(16) + cz * 16;
    // PlacementUtils.HEIGHTMAP: the heightmap WORLD_SURFACE_WG, if above the
    // dimension's bottom
    let surface = world.base_height(x, z, Heightmap::WorldSurface);
    if surface <= 0 {
        return None;
    }
    // OffsetPlacement.vertical(UniformInt.of(3, 9))
    let y = surface + random.next_int_bounded(9 - 3 + 1) + 3;
    // BiomeFilter.biome(): the biome there must have the feature
    let highlands = Biome::from_name("minecraft:end_highlands").expect("a biome");
    if world.biome(x >> 2, y >> 2, z >> 2) != highlands {
        return None;
    }
    Some(ReturnGateway {
        x,
        y,
        z,
        under_city: city_covers(world, x, z),
    })
}

/// Whether the bounding box of a piece of a generating end city covers the
/// column (`x`, `z`): the pieces are placed in the same chunk generation,
/// before the feature, and the heightmap the gateway uses sees their blocks.
fn city_covers(world: &World, x: i32, z: i32) -> bool {
    let set = STRUCTURE_SETS
        .find_set("minecraft:end_cities")
        .expect("end cities");
    // A city's pieces reach less than 96 blocks from its start chunk.
    let rect = ChunkRect::around_block(x, z, 160);
    world
        .structures()
        .hits(&SetFilter::all(set), rect)
        .any(|hit| {
            let (cx, cz) = hit.candidate.chunk;
            let (sx, sy, sz) = hit.generated.start;
            end_city_pieces(world.seed(), cx, cz, [sx, sy, sz])
                .iter()
                .any(|p| p.bbox[0] <= x && x <= p.bbox[3] && p.bbox[2] <= z && z <= p.bbox[5])
        })
}

/// The return gateways of every chunk in `rect`.
pub fn return_gateways(world: &World, rect: ChunkRect) -> Vec<ReturnGateway> {
    let mut out = Vec::new();
    for cx in rect.x0..=rect.x1 {
        for cz in rect.z0..=rect.z1 {
            out.extend(return_gateway(world, cx, cz));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The game's expression, evaluated here (see `gateway_position`).
    fn formula(index: usize) -> (i32, i32) {
        let a = 2.0 * (-std::f64::consts::PI + 0.15707963267948966 * index as f64);
        (
            (96.0 * a.cos()).floor() as i32,
            (96.0 * a.sin()).floor() as i32,
        )
    }

    #[test]
    fn table_is_the_formula() {
        for i in 0..GATEWAY_COUNT {
            assert_eq!(gateway_position(i), formula(i), "gateway {i}");
        }
    }

    #[test]
    fn every_gateway_appears_once() {
        for seed in [0, 1, -1, 123456789, i64::MIN, i64::MAX] {
            let mut order = gateway_order(seed).to_vec();
            order.sort();
            assert_eq!(order, (0..20).collect::<Vec<u8>>(), "seed {seed}");
            let numbers: Vec<u8> = gateways(seed).iter().map(|g| g.number).collect();
            assert_eq!(numbers, (1..=20).collect::<Vec<u8>>());
        }
    }
}
