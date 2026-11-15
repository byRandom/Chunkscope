//! Seed finder: search seeds for structures near the spawn and for biomes.
//!
//! Overworld structures are measured from the query's center; Nether and
//! End structures from (0, 0) in their dimension.
//!
//! Each seed is checked with the cheap tests first: where structure sets
//! try to place structures near the center (a few random numbers per region),
//! then the biome at the center, and only then the full structure validation
//! (biome and terrain), which is what makes a match exact.
//!
//! A [`Query`] has a text form, shared by the command line and the app (which
//! can copy and paste it):
//!
//! ```text
//! center=spawn villages:300:2 end_cities+ship:1000 biome=plains,meadow
//! ```
//!
//! - `center=spawn` or `center=zero`: where distances are measured from
//!   (default: the spawn);
//! - `<target>[+ship]:<radius>[:<count>]`: at least `count` (default 1)
//!   structures of a target (see [`TARGETS`]) within `radius` blocks;
//!   `+ship` keeps end cities with a ship;
//! - `biome=<biome>,...`: the biome at the center is one of these.
//!
//! A [`Match`] prints as one line (see its `Display`).

use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::biome::{Biome, OverworldClimate};
use crate::spawn::spawn_origin;
use crate::structure::StructureSet;
use crate::structure::scan::{Candidate, ChunkRect, SetFilter, candidates};
pub use crate::structure::target::{TARGETS, Target};
use crate::terrain::Heightmap;
use crate::world::World;

/// Where distances are measured from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Center {
    /// The world spawn origin (exact; the spawn block is within a few chunks).
    Spawn,
    /// The block (0, 0).
    Zero,
}

/// The dimension a structure filter searches. Nether and End filters are
/// measured from (0, 0) in their dimension, whatever the query's center.
pub use crate::world::Dimension;

/// At least `min_count` structures of a target within `radius` blocks of
/// the center (measured to the position `/locate` reports).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructureFilter {
    pub target: &'static Target,
    pub radius: i32,
    pub min_count: u32,
    /// End cities: only those with a ship. Ignored for other targets.
    pub ship_only: bool,
}

impl StructureFilter {
    /// At least one structure of `target` within `radius` blocks.
    pub fn new(target: &'static Target, radius: i32) -> Self {
        Self {
            target,
            radius,
            min_count: 1,
            ship_only: false,
        }
    }

    pub fn dimension(&self) -> Dimension {
        self.target.dimension
    }

    /// The structures that count.
    pub fn select(&self) -> SetFilter {
        self.target.select(self.ship_only)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub center: Center,
    pub structures: Vec<StructureFilter>,
    /// The biome at the center, on top of the terrain, must be one of these.
    pub center_biomes: Option<Vec<Biome>>,
}

impl Default for Query {
    fn default() -> Self {
        Self {
            center: Center::Spawn,
            structures: Vec::new(),
            center_biomes: None,
        }
    }
}

/// Why a query's text form does not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError(pub String);

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for QueryError {}

fn error(message: String) -> QueryError {
    QueryError(message)
}

/// A name without its `minecraft:` namespace.
fn short(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

impl FromStr for StructureFilter {
    type Err = QueryError;

    /// `<target>[+ship]:<radius>[:<count>]`
    fn from_str(s: &str) -> Result<Self, QueryError> {
        let mut parts = s.split(':');
        let name = parts.next().unwrap_or_default();
        let (id, ship_only) = match name.strip_suffix("+ship") {
            Some(id) => (id, true),
            None => (name, false),
        };
        let target = Target::find(id).ok_or_else(|| {
            let ids: Vec<&str> = TARGETS.iter().map(|t| t.id).collect();
            error(format!(
                "unknown structure `{id}` (one of: {})",
                ids.join(", ")
            ))
        })?;
        if ship_only && !target.has_ships() {
            return Err(error(format!("`+ship` does not apply to `{id}`")));
        }
        let number = |what: &str, v: Option<&str>| {
            v.ok_or_else(|| error(format!("`{s}` needs a {what}: `{id}:<radius>`")))?
                .trim()
                .parse::<u32>()
                .map_err(|_| error(format!("bad {what} in `{s}`")))
        };
        let radius = number("radius", parts.next())?;
        let radius =
            i32::try_from(radius).map_err(|_| error(format!("radius too large in `{s}`")))?;
        let min_count = match parts.next() {
            None => 1,
            v => number("count", v)?,
        };
        if min_count == 0 {
            return Err(error(format!("the count in `{s}` must be at least 1")));
        }
        if parts.next().is_some() {
            return Err(error(format!("too many `:` in `{s}`")));
        }
        Ok(Self {
            target,
            radius,
            min_count,
            ship_only,
        })
    }
}

impl fmt::Display for StructureFilter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ship = if self.ship_only && self.target.has_ships() {
            "+ship"
        } else {
            ""
        };
        write!(f, "{}{ship}:{}", self.target.id, self.radius)?;
        if self.min_count != 1 {
            write!(f, ":{}", self.min_count)?;
        }
        Ok(())
    }
}

impl FromStr for Query {
    type Err = QueryError;

    /// Words separated by spaces, in any order (see the module docs).
    fn from_str(s: &str) -> Result<Self, QueryError> {
        let mut query = Query::default();
        for word in s.split_whitespace() {
            if let Some(center) = word.strip_prefix("center=") {
                query.center = match center {
                    "spawn" => Center::Spawn,
                    "zero" | "0,0" => Center::Zero,
                    _ => {
                        return Err(error(format!(
                            "`center` is `spawn` or `zero`, not `{center}`"
                        )));
                    }
                };
            } else if let Some(names) = word.strip_prefix("biome=") {
                let biomes = query.center_biomes.get_or_insert_with(Vec::new);
                for name in names.split(',').filter(|n| !n.is_empty()) {
                    let biome = Biome::from_name(&format!("minecraft:{}", short(name)))
                        .filter(|b| Dimension::Overworld.biomes().contains(b))
                        .ok_or_else(|| error(format!("unknown Overworld biome `{name}`")))?;
                    if !biomes.contains(&biome) {
                        biomes.push(biome);
                    }
                }
            } else {
                query.structures.push(word.parse()?);
            }
        }
        Ok(query)
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self.center {
            Center::Spawn => "center=spawn",
            Center::Zero => "center=zero",
        })?;
        for s in &self.structures {
            write!(f, " {s}")?;
        }
        if let Some(biomes) = &self.center_biomes {
            let names: Vec<&str> = biomes.iter().map(|b| short(b.name())).collect();
            write!(f, " biome={}", names.join(","))?;
        }
        Ok(())
    }
}

/// A structure that satisfies a filter.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The structure that generates (e.g. `minecraft:village_plains`).
    pub name: &'static str,
    pub dimension: Dimension,
    pub x: i32,
    pub z: i32,
    pub distance: f64,
}

/// `village_plains 120, -48 (130)`, with the dimension when not the
/// Overworld: `fortress (nether) 64, 200 (210)`.
impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", short(self.name))?;
        if self.dimension != Dimension::Overworld {
            write!(f, " ({})", self.dimension.id())?;
        }
        write!(f, " {}, {} ({:.0})", self.x, self.z, self.distance)
    }
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

/// One line: the seed, the center, its biome and the structures found.
///
/// ```text
/// 12345  center 8, -24  plains  village_plains 120, -48 (130); ...
/// ```
impl fmt::Display for Match {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}  center {}, {}  {}",
            self.seed,
            self.center.0,
            self.center.1,
            short(self.center_biome.name())
        )?;
        for (i, found) in self.found.iter().enumerate() {
            write!(f, "{}{found}", if i == 0 { "  " } else { "; " })?;
        }
        Ok(())
    }
}

/// The candidates of `set` whose locate position is within `radius` of
/// `center`, nearest first, with their distance.
fn candidates_near(
    set: &'static StructureSet,
    seed: i64,
    center: (i32, i32),
    radius: i32,
) -> Vec<(Candidate, f64)> {
    let rect = ChunkRect::around_block(center.0, center.1, radius);
    let mut out: Vec<_> = candidates(set, seed, rect)
        .map(|c| (c, c.distance(center)))
        .filter(|&(_, d)| d <= radius as f64)
        .collect();
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}

/// Check one seed against a query.
pub fn check(seed: i64, query: &Query) -> Option<Match> {
    let center = match query.center {
        Center::Spawn => spawn_origin(&OverworldClimate::new(seed)),
        Center::Zero => (0, 0),
    };
    let filter_center = |f: &StructureFilter| match f.dimension() {
        Dimension::Overworld => center,
        Dimension::Nether | Dimension::End => (0, 0),
    };
    // Cheap pass: enough candidates near the center for every filter?
    let mut near = Vec::with_capacity(query.structures.len());
    for f in &query.structures {
        let a = candidates_near(f.target.set(), seed, filter_center(f), f.radius);
        if (a.len() as u32) < f.min_count {
            return None;
        }
        near.push(a);
    }
    let overworld = World::new(seed, Dimension::Overworld);
    // The biome where a player stands: on top of the terrain.
    let surface = overworld.base_height(center.0, center.1, Heightmap::WorldSurface);
    let center_biome = overworld.biome_at_block(center.0, surface, center.1);
    if let Some(allowed) = &query.center_biomes
        && !allowed.contains(&center_biome)
    {
        return None;
    }
    // Full pass: the structures that really generate. The other dimensions
    // are built only if a filter needs them.
    let others: Vec<World> = [Dimension::Nether, Dimension::End]
        .into_iter()
        .filter(|&d| query.structures.iter().any(|f| f.dimension() == d))
        .map(|d| World::new(seed, d))
        .collect();
    let world = |d: Dimension| {
        std::iter::once(&overworld)
            .chain(&others)
            .find(|w| w.dimension() == d)
            .expect("built for every filter's dimension")
    };
    let mut found = Vec::new();
    for (f, near) in query.structures.iter().zip(near) {
        let generator = world(f.dimension()).structures();
        let select = f.select();
        let mut count = 0;
        for (candidate, distance) in near {
            let Some(hit) = generator.hit(&select, candidate) else {
                continue;
            };
            let (x, z) = candidate.locate;
            let entry = Found {
                name: hit.generated.name,
                dimension: f.dimension(),
                x,
                z,
                distance,
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
