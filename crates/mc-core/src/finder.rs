//! Seed finder: search seeds for structures near the spawn and for biomes.
//!
//! Overworld structures are measured from the query's center; Nether and
//! End structures from (0, 0) in their dimension.
//!
//! Each seed is checked with the cheap tests first: where structure sets
//! *attempt* structures near the center (a few random numbers per region),
//! then the biome at the center, and only then the full structure validation
//! (biome and terrain), which is what makes a match exact.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::biome::{
    Biome, EndBiomeSource, NetherBiomeSource, OverworldBiomeSource, OverworldClimate,
};
use crate::spawn::spawn_origin;
use crate::structure::validate::{Outcome, StructureGenerator};
use crate::structure::{STRUCTURE_SETS, StructurePlacement, StructureSet};
use crate::terrain::{EndTerrain, Heightmap, NetherTerrain, OverworldTerrain};

/// Where distances are measured from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Center {
    /// The world spawn origin (exact; the spawn block is within a few chunks).
    Spawn,
    /// The block (0, 0).
    Zero,
}

/// The dimension a structure filter searches.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Dimension {
    #[default]
    Overworld,
    /// Measured from the Nether's (0, 0), whatever the query's center.
    Nether,
    /// Measured from the End's (0, 0), whatever the query's center.
    End,
}

/// At least `min_count` structures of `set` within `radius` blocks of the
/// center (measured to the position `/locate` reports).
#[derive(Clone, Copy, Debug)]
pub struct StructureFilter {
    pub set: &'static StructureSet,
    pub radius: i32,
    pub min_count: u32,
    pub dimension: Dimension,
    /// Only structures whose name starts with this (e.g. `minecraft:bastion`
    /// in the set they share with fortresses).
    pub only: Option<&'static str>,
    /// End cities: only those with a ship.
    pub ship: bool,
}

impl StructureFilter {
    /// A filter on every structure of an overworld set.
    pub fn overworld(set: &'static StructureSet, radius: i32, min_count: u32) -> Self {
        Self {
            set,
            radius,
            min_count,
            dimension: Dimension::Overworld,
            only: None,
            ship: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Query {
    pub center: Center,
    pub structures: Vec<StructureFilter>,
    /// The biome at the center, on top of the terrain, must be one of these.
    pub center_biomes: Option<Vec<Biome>>,
}

/// A structure that satisfies a filter.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The structure that generates, or the set's name when its structures
    /// are not validated yet (mineshafts).
    pub name: &'static str,
    pub dimension: Dimension,
    pub x: i32,
    pub z: i32,
    pub distance: f64,
    /// `false` when the structure is not validated yet.
    pub exact: bool,
}

/// A seed that matches a query.
#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    pub seed: i64,
    pub center: (i32, i32),
    pub center_biome: Biome,
    /// The structures found, filter by filter, nearest first.
    pub found: Vec<Found>,
}

/// An attempt chunk, its locate position and its distance to the center.
type Attempt = ((i32, i32), (i32, i32), f64);

/// Attempt chunks of `set` whose locate position is within `radius` of
/// `center`, nearest first, with their locate position and distance.
fn attempts_near(set: &StructureSet, seed: i64, (x, z): (i32, i32), radius: i32) -> Vec<Attempt> {
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let r = radius as i64;
    let (cx0, cz0) = (
        ((x as i64 - r) >> 4) as i32 - 1,
        ((z as i64 - r) >> 4) as i32 - 1,
    );
    let (cx1, cz1) = (
        ((x as i64 + r) >> 4) as i32 + 1,
        ((z as i64 + r) >> 4) as i32 + 1,
    );
    let mut push = |cx: i32, cz: i32| {
        if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) != Some(true) {
            return;
        }
        let (bx, _, bz) = p.locate_pos(cx, cz);
        let d = (((bx - x) as f64).powi(2) + ((bz - z) as f64).powi(2)).sqrt();
        if d <= radius as f64 {
            out.push(((cx, cz), (bx, bz), d));
        }
    };
    if p.spacing == 1 {
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                push(cx, cz);
            }
        }
    } else {
        let s = p.spacing;
        for rz in cz0.div_euclid(s)..=cz1.div_euclid(s) {
            for rx in cx0.div_euclid(s)..=cx1.div_euclid(s) {
                let (cx, cz) = p.region_chunk(seed, rx, rz);
                if (cx0..=cx1).contains(&cx) && (cz0..=cz1).contains(&cz) {
                    push(cx, cz);
                }
            }
        }
    }
    out.sort_by(|a, b| a.2.total_cmp(&b.2));
    out
}

/// Check one seed against a query.
pub fn check(seed: i64, query: &Query) -> Option<Match> {
    let center = match query.center {
        Center::Spawn => spawn_origin(&OverworldClimate::new(seed)),
        Center::Zero => (0, 0),
    };
    let filter_center = |f: &StructureFilter| match f.dimension {
        Dimension::Overworld => center,
        Dimension::Nether | Dimension::End => (0, 0),
    };
    // Cheap pass: enough attempts near the center for every filter?
    let mut attempts = Vec::with_capacity(query.structures.len());
    for f in &query.structures {
        let a = attempts_near(f.set, seed, filter_center(f), f.radius);
        if (a.len() as u32) < f.min_count {
            return None;
        }
        attempts.push(a);
    }
    let biomes = OverworldBiomeSource::new(seed);
    let terrain = OverworldTerrain::new(seed);
    // The biome where a player stands: on top of the terrain.
    let surface = terrain.base_height(center.0, center.1, Heightmap::WorldSurface);
    let center_biome = biomes.biome_at_block(center.0, surface, center.1);
    if let Some(allowed) = &query.center_biomes
        && !allowed.contains(&center_biome)
    {
        return None;
    }
    // Full pass: the structures that really generate.
    let generator = StructureGenerator::new(seed, &biomes, &terrain);
    let nether = query
        .structures
        .iter()
        .any(|f| f.dimension == Dimension::Nether)
        .then(|| (NetherBiomeSource::new(seed), NetherTerrain::new(seed)));
    let nether_generator = nether
        .as_ref()
        .map(|(b, t)| StructureGenerator::nether(seed, b, t));
    let end = query
        .structures
        .iter()
        .any(|f| f.dimension == Dimension::End)
        .then(|| (EndBiomeSource::new(seed), EndTerrain::new(seed)));
    let end_generator = end
        .as_ref()
        .map(|(b, t)| StructureGenerator::end(seed, b, t));
    let mut found = Vec::new();
    for (f, attempts) in query.structures.iter().zip(attempts) {
        let generator = match f.dimension {
            Dimension::Overworld => &generator,
            Dimension::Nether => nether_generator.as_ref().expect("built for Nether filters"),
            Dimension::End => end_generator.as_ref().expect("built for End filters"),
        };
        let mut count = 0;
        for ((cx, cz), (x, z), distance) in attempts {
            let entry = match generator.attempt(f.set, cx, cz) {
                outcome @ Outcome::Generates(_) => {
                    let (s, _) = outcome.generated().expect("generates");
                    if f.only.is_some_and(|prefix| !s.name.starts_with(prefix)) {
                        continue;
                    }
                    if f.ship
                        && s.name == "minecraft:end_city"
                        && !crate::structure::end_city::end_city_has_ship(seed, cx, cz)
                    {
                        continue;
                    }
                    Found {
                        name: s.name,
                        dimension: f.dimension,
                        x,
                        z,
                        distance,
                        exact: true,
                    }
                }
                Outcome::Nothing(_) => continue,
                Outcome::Unknown(_) => Found {
                    name: f.set.name,
                    dimension: f.dimension,
                    x,
                    z,
                    distance,
                    exact: false,
                },
            };
            found.push(entry);
            count += 1;
            if count >= f.min_count {
                break;
            }
        }
        if count < f.min_count {
            return None;
        }
    }
    Some(Match {
        seed,
        center,
        center_biome,
        found,
    })
}

/// Progress and cancellation of a running search, shared between threads.
#[derive(Debug, Default)]
pub struct SearchState {
    pub checked: AtomicU64,
    pub cancel: AtomicBool,
}

/// Seeds to search.
#[derive(Clone, Copy, Debug)]
pub enum Seeds {
    /// `start`, `start + 1`, ... (wrapping).
    Sequential { start: i64, count: u64 },
    /// Pseudo-random seeds (SplitMix64 of `salt + i`), for sampling the
    /// whole seed space.
    Random { salt: u64, count: u64 },
}

impl Seeds {
    pub fn count(&self) -> u64 {
        match *self {
            Seeds::Sequential { count, .. } | Seeds::Random { count, .. } => count,
        }
    }

    pub fn get(&self, i: u64) -> i64 {
        match *self {
            Seeds::Sequential { start, .. } => start.wrapping_add(i as i64),
            Seeds::Random { salt, .. } => {
                let mut z = salt.wrapping_add(i).wrapping_add(0x9e37_79b9_7f4a_7c15);
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                (z ^ (z >> 31)) as i64
            }
        }
    }
}

/// Search `seeds` on `threads` threads, calling `on_match` for every match
/// (from the worker threads). Stops early when `state.cancel` is set.
pub fn search(
    seeds: Seeds,
    query: &Query,
    threads: usize,
    state: &SearchState,
    on_match: &(dyn Fn(Match) + Sync),
) {
    let next = AtomicU64::new(0);
    const BATCH: u64 = 64;
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| {
                loop {
                    if state.cancel.load(Ordering::Relaxed) {
                        return;
                    }
                    let start = next.fetch_add(BATCH, Ordering::Relaxed);
                    if start >= seeds.count() {
                        return;
                    }
                    let end = (start + BATCH).min(seeds.count());
                    for i in start..end {
                        if let Some(m) = check(seeds.get(i), query) {
                            on_match(m);
                        }
                        state.checked.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
}
