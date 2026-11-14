//! The structures of one set in an area: the one place that walks a
//! random-spread set's regions, so the map, the finder and the examples
//! cannot disagree on which chunks they look at.
//!
//! Two steps, because the second is expensive:
//! - [`candidates`]: the chunks where the set tries to place a structure
//!   (placement only, a few random numbers per region);
//! - [`StructureGenerator::hit`]: whether that try really generates (biome
//!   and terrain), filtered by [`SetFilter`].
//!
//! [`StructureGenerator::hits`] does both for every candidate in an area.

use super::end_city::end_city_has_ship;
use super::validate::StructureGenerator;
use super::{STRUCTURE_SETS, StructurePlacement, StructureSet};

/// A rectangle of chunks, both ends inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkRect {
    pub x0: i32,
    pub z0: i32,
    pub x1: i32,
    pub z1: i32,
}

impl ChunkRect {
    /// Every chunk whose locate position can lie within `radius` blocks of
    /// the block (`x`, `z`).
    pub fn around_block(x: i32, z: i32, radius: i32) -> Self {
        let r = radius as i64;
        let chunk = |v: i32, d: i64| ((v as i64 + d) >> 4) as i32;
        Self {
            x0: chunk(x, -r) - 1,
            z0: chunk(z, -r) - 1,
            x1: chunk(x, r) + 1,
            z1: chunk(z, r) + 1,
        }
    }

    pub fn contains(&self, cx: i32, cz: i32) -> bool {
        (self.x0..=self.x1).contains(&cx) && (self.z0..=self.z1).contains(&cz)
    }
}

/// A chunk where a structure set tries to place a structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub chunk: (i32, i32),
    /// The block position `/locate` reports (x, z).
    pub locate: (i32, i32),
}

impl Candidate {
    /// The distance from the block (`x`, `z`) to the locate position.
    pub fn distance(&self, (x, z): (i32, i32)) -> f64 {
        let dx = (self.locate.0 - x) as f64;
        let dz = (self.locate.1 - z) as f64;
        (dx * dx + dz * dz).sqrt()
    }
}

/// The candidates of a random-spread `set` whose chunk lies in `rect`, row
/// by row. Empty for concentric rings (strongholds).
pub fn candidates(
    set: &'static StructureSet,
    seed: i64,
    rect: ChunkRect,
) -> impl Iterator<Item = Candidate> {
    let placement = match &set.placement {
        StructurePlacement::RandomSpread(p) => Some(p),
        StructurePlacement::ConcentricRings { .. } => None,
    };
    // Every chunk when a region is one chunk (mineshafts, buried treasure),
    // else the region's one candidate chunk, if it falls in the rect.
    let chunks: Box<dyn Iterator<Item = (i32, i32)>> = match placement {
        None => Box::new(std::iter::empty()),
        Some(p) if p.spacing == 1 => Box::new(
            (rect.z0..=rect.z1).flat_map(move |cz| (rect.x0..=rect.x1).map(move |cx| (cx, cz))),
        ),
        Some(p) => {
            let s = p.spacing;
            let (rx0, rx1) = (rect.x0.div_euclid(s), rect.x1.div_euclid(s));
            Box::new(
                (rect.z0.div_euclid(s)..=rect.z1.div_euclid(s))
                    .flat_map(move |rz| (rx0..=rx1).map(move |rx| p.region_chunk(seed, rx, rz)))
                    .filter(move |&(cx, cz)| rect.contains(cx, cz)),
            )
        }
    };
    chunks.filter_map(move |(cx, cz)| {
        let p = placement?;
        (set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) == Some(true)).then(|| {
            let (x, _, z) = p.locate_pos(cx, cz);
            Candidate {
                chunk: (cx, cz),
                locate: (x, z),
            }
        })
    })
}

/// Which structures of a set count.
#[derive(Clone, Copy, Debug)]
pub struct SetFilter {
    pub set: &'static StructureSet,
    /// Only structures whose name starts with this (e.g. `minecraft:bastion`
    /// in the set they share with fortresses).
    pub only: Option<&'static str>,
    /// End cities: only those with a ship.
    pub ship_only: bool,
}

impl SetFilter {
    /// Every structure of `set`.
    pub fn all(set: &'static StructureSet) -> Self {
        Self {
            set,
            only: None,
            ship_only: false,
        }
    }
}

/// A structure that generates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generated {
    /// E.g. `minecraft:village_plains`.
    pub name: &'static str,
    /// The validated generation point (block position).
    pub start: (i32, i32, i32),
    /// End cities: whether the city has a ship (an elytra).
    pub ship: Option<bool>,
}

/// A candidate whose structure generates and passes a [`SetFilter`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub candidate: Candidate,
    pub generated: Generated,
}

impl StructureGenerator<'_> {
    /// Whether `candidate` of `filter.set` generates a structure that passes
    /// `filter`.
    pub fn hit(&self, filter: &SetFilter, candidate: Candidate) -> Option<Hit> {
        let (cx, cz) = candidate.chunk;
        let (s, start) = self.attempt(filter.set, cx, cz).generated()?;
        if filter
            .only
            .is_some_and(|prefix| !s.name.starts_with(prefix))
        {
            return None;
        }
        let ship = (s.name == "minecraft:end_city").then(|| end_city_has_ship(self.seed(), cx, cz));
        if filter.ship_only && ship == Some(false) {
            return None;
        }
        Some(Hit {
            candidate,
            generated: Generated {
                name: s.name,
                start,
                ship,
            },
        })
    }

    /// Every hit of `filter` whose chunk lies in `rect`.
    pub fn hits(&self, filter: &SetFilter, rect: ChunkRect) -> impl Iterator<Item = Hit> {
        candidates(filter.set, self.seed(), rect).filter_map(move |c| self.hit(filter, c))
    }
}
