//! Strongholds: `ConcentricRingsStructurePlacement`, with positions computed
//! as in `ChunkGeneratorStructureState.generateRingPositions`.

use crate::biome::{Biome, OverworldBiomeSource};
use crate::random::{LegacyRandomSource, RandomSource};

use super::placement::{StructurePlacement, StructureSet};

/// Ring parameters of a `concentric_rings` structure set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConcentricRings {
    pub distance: i32,
    pub spread: i32,
    pub count: i32,
    /// Biome names the position is moved towards (the resolved
    /// `preferred_biomes` tag).
    pub preferred_biomes: &'static [&'static str],
}

impl StructureSet {
    /// The ring parameters, if this set uses `concentric_rings`.
    pub fn concentric_rings(&self) -> Option<ConcentricRings> {
        match self.placement {
            StructurePlacement::ConcentricRings {
                distance,
                spread,
                count,
                preferred_biome_list,
                ..
            } => Some(ConcentricRings {
                distance,
                spread,
                count,
                preferred_biomes: preferred_biome_list,
            }),
            StructurePlacement::RandomSpread(_) => None,
        }
    }
}

/// `BiomeSource.findBiomeHorizontal(x, y, z, radius, 1, predicate, random,
/// false, randomState)`: scans the whole `radius` square (in quart steps, Z
/// outer, X inner) and keeps a uniformly random match (reservoir sampling).
/// Returns the matching block position, or `None`.
pub fn find_biome_horizontal(
    source: &OverworldBiomeSource,
    x: i32,
    y: i32,
    z: i32,
    radius: i32,
    predicate: impl Fn(Biome) -> bool,
    random: &mut impl RandomSource,
) -> Option<(i32, i32, i32)> {
    let (qx, qz, r, qy) = (x >> 2, z >> 2, radius >> 2, y >> 2);
    let mut found = None;
    let mut count = 0;
    for dz in -r..=r {
        for dx in -r..=r {
            let (bx, bz) = (qx + dx, qz + dz);
            if predicate(source.biome(bx, qy, bz)) {
                if found.is_none() || random.next_int_bounded(count + 1) == 0 {
                    found = Some((bx << 2, y, bz << 2));
                }
                count += 1;
            }
        }
    }
    found
}

/// Chunk positions of all rings of a `concentric_rings` placement, in the
/// game's order. `seed` is the world seed (`concentricRingsSeed`).
///
/// The game computes each position on a background thread, each with its
/// own R-tree search state (see docs/PARITY.md), so in rare distance ties
/// the game itself can differ between runs. Here they are computed in order
/// on one fresh search tree per position.
pub fn ring_positions(
    seed: i64,
    rings: &ConcentricRings,
    source: &OverworldBiomeSource,
) -> Vec<(i32, i32)> {
    let preferred: Vec<Biome> = rings
        .preferred_biomes
        .iter()
        .filter_map(|n| Biome::from_name(n))
        .collect();
    ring_candidates(seed, rings)
        .into_iter()
        .map(|(x, z, mut random)| {
            source.reset_search();
            let center = |c: i32| (c << 4) + 8; // SectionPos.sectionToBlockCoord(c, 8)
            match find_biome_horizontal(
                source,
                center(x),
                0,
                center(z),
                112,
                |b| preferred.contains(&b),
                &mut random,
            ) {
                Some((bx, _, bz)) => (bx >> 4, bz >> 4),
                None => (x, z),
            }
        })
        .collect()
}

/// The ring positions before the biome search, each with the forked random
/// the game hands to that search.
pub fn ring_candidates(seed: i64, rings: &ConcentricRings) -> Vec<(i32, i32, LegacyRandomSource)> {
    use std::f64::consts::PI;
    let (distance, count) = (rings.distance, rings.count);
    let mut spread = rings.spread;
    let mut out = Vec::with_capacity(count.max(0) as usize);
    if count == 0 {
        return out;
    }
    let mut random = LegacyRandomSource::new(seed); // RandomSource.create() + setSeed(seed)
    let mut angle = random.next_double() * PI * 2.0;
    let mut in_ring = 0;
    let mut ring = 0;
    for i in 0..count {
        let dist = (4 * distance + distance * ring * 6) as f64
            + (random.next_double() - 0.5) * (distance as f64 * 2.5);
        // Math.cos/Math.sin are HotSpot intrinsics; see docs/PARITY.md.
        let x = java_compat::math::round(angle.cos() * dist) as i32;
        let z = java_compat::math::round(angle.sin() * dist) as i32;
        out.push((x, z, random.fork()));
        angle += PI * 2.0 / spread as f64;
        in_ring += 1;
        if in_ring == spread {
            ring += 1;
            in_ring = 0;
            spread += 2 * spread / (ring + 1);
            spread = spread.min(count - i);
            angle += random.next_double() * PI * 2.0;
        }
    }
    out
}
