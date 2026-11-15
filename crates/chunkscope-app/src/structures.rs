//! Structure layers: the structures that generate in each structure set of
//! the Overworld and the Nether, and stronghold positions.
//!
//! Each set tries to place a structure at one chunk per region; whether it really
//! generates there (and which structure of the set) is checked against
//! biome and terrain like the game does, on background threads.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use eframe::egui::{self, Color32};
use mc_core::biome::OverworldBiomeSource;
use mc_core::structure::scan::{ChunkRect, SetFilter};
use mc_core::structure::validate::StructureGenerator;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSet, StructureSets};
use mc_core::terrain::OverworldTerrain;
use mc_core::world::World;

use crate::dimension::Dimension;
use crate::work::{Job, Wake, WorkPool};

/// What a layer shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The world spawn (an estimate, see [`Spawn`]).
    Spawn,
    /// Stronghold positions (see [`Strongholds`]).
    Strongholds,
    /// The structures of a structure set that generate.
    Set {
        /// Structure set registry name.
        set: &'static str,
        /// Only the structures of the set whose name starts with this (the
        /// Nether's fortresses and bastions share one set).
        only: Option<&'static str>,
    },
}

/// One structure layer of the map.
pub struct Kind {
    /// Stable name, kept in saved settings: never rename one.
    pub id: &'static str,
    pub dimension: Dimension,
    pub source: Source,
    pub label: &'static str,
    /// Up to two letters drawn on the marker.
    pub glyph: &'static str,
    pub color: Color32,
    /// Shown by default.
    pub default_on: bool,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Kind {
    pub fn is_spawn(&self) -> bool {
        self.source == Source::Spawn
    }

    /// Whether the seed finder can search this layer (structure sets only).
    pub fn searchable(&self) -> bool {
        matches!(self.source, Source::Set { .. })
    }

    /// End cities, whose markers and filters know about ships.
    pub fn has_ships(&self) -> bool {
        matches!(
            self.source,
            Source::Set {
                set: "minecraft:end_cities",
                ..
            }
        )
    }

    /// The structures of this layer that count, for a structure set layer.
    /// `ship_only` keeps end cities with a ship.
    pub fn select(&self, ship_only: bool) -> Option<SetFilter> {
        let Source::Set { set: name, only } = self.source else {
            return None;
        };
        Some(SetFilter {
            set: set(name)?,
            only,
            ship_only,
        })
    }
}

/// The index in [`KINDS`] of the layer with this id.
pub fn kind_index(id: &str) -> Option<usize> {
    KINDS.iter().position(|k| k.id == id)
}

/// Overworld structure layers, in legend order.
pub static KINDS: &[Kind] = &[
    Kind {
        id: "spawn",
        dimension: Dimension::Overworld,
        source: Source::Spawn,
        label: "World spawn",
        glyph: "W",
        color: rgb(0xf4f4f4),
        default_on: true,
    },
    Kind {
        id: "villages",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:villages",
            only: None,
        },
        label: "Villages",
        glyph: "V",
        color: rgb(0xe0a458),
        default_on: true,
    },
    Kind {
        id: "strongholds",
        dimension: Dimension::Overworld,
        source: Source::Strongholds,
        label: "Strongholds",
        glyph: "S",
        color: rgb(0xb67cf2),
        default_on: true,
    },
    Kind {
        id: "woodland_mansions",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:woodland_mansions",
            only: None,
        },
        label: "Woodland mansions",
        glyph: "WM",
        color: rgb(0x9a6b3f),
        default_on: true,
    },
    Kind {
        id: "ocean_monuments",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:ocean_monuments",
            only: None,
        },
        label: "Ocean monuments",
        glyph: "OM",
        color: rgb(0x4fd1c5),
        default_on: true,
    },
    Kind {
        id: "pillager_outposts",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:pillager_outposts",
            only: None,
        },
        label: "Pillager outposts",
        glyph: "PO",
        color: rgb(0x9aa0a6),
        default_on: true,
    },
    Kind {
        id: "desert_pyramids",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:desert_pyramids",
            only: None,
        },
        label: "Desert pyramids",
        glyph: "DP",
        color: rgb(0xf2d16b),
        default_on: false,
    },
    Kind {
        id: "jungle_temples",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:jungle_temples",
            only: None,
        },
        label: "Jungle temples",
        glyph: "JT",
        color: rgb(0x7bc96f),
        default_on: false,
    },
    Kind {
        id: "swamp_huts",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:swamp_huts",
            only: None,
        },
        label: "Swamp huts",
        glyph: "SH",
        color: rgb(0x6f8f5f),
        default_on: false,
    },
    Kind {
        id: "igloos",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:igloos",
            only: None,
        },
        label: "Igloos",
        glyph: "IG",
        color: rgb(0xcfe8ff),
        default_on: false,
    },
    Kind {
        id: "ancient_cities",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:ancient_cities",
            only: None,
        },
        label: "Ancient cities",
        glyph: "AC",
        color: rgb(0x3aa7a0),
        default_on: false,
    },
    Kind {
        id: "trial_chambers",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:trial_chambers",
            only: None,
        },
        label: "Trial chambers",
        glyph: "TC",
        color: rgb(0xd9824a),
        default_on: false,
    },
    Kind {
        id: "trail_ruins",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:trail_ruins",
            only: None,
        },
        label: "Trail ruins",
        glyph: "TR",
        color: rgb(0xc98b6b),
        default_on: false,
    },
    Kind {
        id: "abandoned_camps",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:abandoned_camp",
            only: None,
        },
        label: "Abandoned camps",
        glyph: "AB",
        color: rgb(0xb5a36a),
        default_on: false,
    },
    Kind {
        id: "ruined_portals",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:ruined_portals",
            only: None,
        },
        label: "Ruined portals",
        glyph: "RP",
        color: rgb(0xa77bd9),
        default_on: false,
    },
    Kind {
        id: "shipwrecks",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:shipwrecks",
            only: None,
        },
        label: "Shipwrecks",
        glyph: "SW",
        color: rgb(0x8c6d4f),
        default_on: false,
    },
    Kind {
        id: "ocean_ruins",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:ocean_ruins",
            only: None,
        },
        label: "Ocean ruins",
        glyph: "OR",
        color: rgb(0x5d8fb3),
        default_on: false,
    },
    Kind {
        id: "buried_treasures",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:buried_treasures",
            only: None,
        },
        label: "Buried treasure",
        glyph: "BT",
        color: rgb(0xf5c542),
        default_on: false,
    },
    Kind {
        id: "mineshafts",
        dimension: Dimension::Overworld,
        source: Source::Set {
            set: "minecraft:mineshafts",
            only: None,
        },
        label: "Mineshafts",
        glyph: "M",
        color: rgb(0x8f7a66),
        default_on: false,
    },
    Kind {
        id: "fortresses",
        dimension: Dimension::Nether,
        source: Source::Set {
            set: "minecraft:nether_complexes",
            only: Some("minecraft:fortress"),
        },
        label: "Fortresses",
        glyph: "F",
        color: rgb(0xd9534f),
        default_on: true,
    },
    Kind {
        id: "bastions",
        dimension: Dimension::Nether,
        source: Source::Set {
            set: "minecraft:nether_complexes",
            only: Some("minecraft:bastion"),
        },
        label: "Bastions",
        glyph: "B",
        color: rgb(0x8c8c96),
        default_on: true,
    },
    Kind {
        id: "nether_fossils",
        dimension: Dimension::Nether,
        source: Source::Set {
            set: "minecraft:nether_fossils",
            only: None,
        },
        label: "Nether fossils",
        glyph: "NF",
        color: rgb(0xe8e0c8),
        default_on: false,
    },
    Kind {
        id: "nether_ruined_portals",
        dimension: Dimension::Nether,
        source: Source::Set {
            set: "minecraft:ruined_portals",
            only: None,
        },
        label: "Ruined portals",
        glyph: "RP",
        color: rgb(0xa77bd9),
        default_on: false,
    },
    Kind {
        id: "end_cities",
        dimension: Dimension::End,
        source: Source::Set {
            set: "minecraft:end_cities",
            only: None,
        },
        label: "End cities",
        glyph: "EC",
        color: rgb(0xd8c6f0),
        default_on: true,
    },
];

/// A structure marker on the map, at the position `/locate` reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub kind: usize,
    pub x: i32,
    pub z: i32,
    /// The structure that generates (e.g. `minecraft:village_plains`) and
    /// its start position (`None` for strongholds).
    pub start: Option<(&'static str, i32, i32, i32)>,
    /// End cities: whether the city has a ship (an elytra).
    pub ship: Option<bool>,
}

pub fn set(name: &str) -> Option<&'static StructureSet> {
    STRUCTURE_SETS.find_set(name)
}

/// Expected distance between two attempts of a random-spread set, in
/// blocks (`None` for strongholds).
pub fn spacing_blocks(kind: usize) -> Option<f64> {
    let StructurePlacement::RandomSpread(p) = &KINDS[kind].select(false)?.set.placement else {
        return None;
    };
    Some(p.spacing as f64 * 16.0 / f64::from(p.frequency).sqrt())
}

/// The markers of `kind` whose chunk lies in `rect`: the structures that
/// generate.
pub fn markers(kind: usize, rect: ChunkRect, generator: &StructureGenerator) -> Vec<Marker> {
    let Some(select) = KINDS[kind].select(false) else {
        return Vec::new();
    };
    generator
        .hits(&select, rect)
        .map(|hit| Marker {
            kind,
            x: hit.candidate.locate.0,
            z: hit.candidate.locate.1,
            start: Some((
                hit.generated.name,
                hit.generated.start.0,
                hit.generated.start.1,
                hit.generated.start.2,
            )),
            ship: hit.generated.ship,
        })
        .collect()
}

/// Structure markers are computed in squares of this many chunks.
pub const BLOCK_CHUNKS: i32 = 32;

/// One square of one structure layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockKey {
    pub seed: i64,
    pub kind: usize,
    pub bx: i32,
    pub bz: i32,
}

/// Validated structure markers, computed on background threads.
pub struct StructureCache {
    pool: WorkPool<BlockKey, Vec<Marker>>,
    blocks: HashMap<BlockKey, Vec<Marker>>,
    in_flight: usize,
}

/// How many squares stay in memory.
const CACHE_LIMIT: usize = 20_000;

impl StructureCache {
    pub fn new(ctx: &egui::Context) -> Self {
        let workers = thread::available_parallelism()
            .map_or(2, |n| n.get())
            .div_ceil(2);
        Self {
            pool: WorkPool::new("structure-worker", workers, repaint(ctx), block_markers),
            blocks: HashMap::new(),
            in_flight: 0,
        }
    }

    /// Squares requested but not computed yet.
    pub fn pending(&self) -> usize {
        self.in_flight
    }

    /// Collect finished squares. Call once per frame.
    pub fn receive(&mut self) {
        self.blocks.extend(self.pool.receive());
        if self.blocks.len() > CACHE_LIMIT {
            self.blocks.clear();
        }
    }

    pub fn get(&self, key: &BlockKey) -> Option<&[Marker]> {
        self.blocks.get(key).map(Vec::as_slice)
    }

    /// Ask for the squares still missing from `wanted` (most urgent first),
    /// dropping earlier requests.
    pub fn request(&mut self, wanted: &[BlockKey]) {
        self.in_flight = self.pool.request(wanted, |k| self.blocks.contains_key(k));
    }
}

/// `egui::Context::request_repaint`, as a pool's wake.
fn repaint(ctx: &egui::Context) -> Wake {
    let ctx = ctx.clone();
    Arc::new(move || ctx.request_repaint())
}

/// The markers of one square. `worlds` holds one world per dimension of the
/// current seed, built when first needed.
fn block_markers(worlds: &mut Vec<World>, key: BlockKey) -> Vec<Marker> {
    let dimension = KINDS[key.kind].dimension;
    worlds.retain(|w| w.seed() == key.seed);
    if !worlds.iter().any(|w| w.dimension() == dimension) {
        worlds.push(World::new(key.seed, dimension));
    }
    let world = worlds
        .iter()
        .find(|w| w.dimension() == dimension)
        .expect("world just built");
    let c = (key.bx * BLOCK_CHUNKS, key.bz * BLOCK_CHUNKS);
    let rect = ChunkRect {
        x0: c.0,
        z0: c.1,
        x1: c.0 + BLOCK_CHUNKS - 1,
        z1: c.1 + BLOCK_CHUNKS - 1,
    };
    markers(key.kind, rect, &world.structures())
}

/// The climate origin `(x, z)` the game searches the spawn around (exact),
/// and the estimated spawn block `(x, y, z)`.
pub type SpawnPoints = ((i32, i32), (i32, i32, i32));

/// The world spawn: its origin (exact) and the spawn block (an estimate),
/// computed on a background thread.
pub struct Spawn(Job<SpawnPoints>);

impl Spawn {
    pub fn new(seed: i64, ctx: &egui::Context) -> Self {
        Self(Job::spawn("spawn", repaint(ctx), move || {
            let biomes = OverworldBiomeSource::new(seed);
            let terrain = OverworldTerrain::new(seed);
            let origin = mc_core::spawn::spawn_origin(biomes.climate());
            let block = mc_core::spawn::spawn_estimate(biomes.climate(), &terrain);
            (origin, block)
        }))
    }

    pub fn is_loading(&mut self) -> bool {
        self.0.is_loading()
    }

    pub fn get(&self) -> Option<SpawnPoints> {
        self.0.get().copied()
    }
}

/// Stronghold positions, computed on a background thread (they need a
/// biome search around each of the 128 ring positions).
pub struct Strongholds(Job<Vec<(i32, i32)>>);

impl Strongholds {
    pub fn new(seed: i64, ctx: &egui::Context) -> Self {
        Self(Job::spawn("strongholds", repaint(ctx), move || {
            World::new(seed, Dimension::Overworld).strongholds()
        }))
    }

    pub fn is_loading(&mut self) -> bool {
        self.0.is_loading()
    }

    /// Block positions, as `/locate` reports them (the chunk's corner).
    pub fn positions(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.0
            .get()
            .into_iter()
            .flatten()
            .map(|&(cx, cz)| (cx << 4, cz << 4))
    }
}
