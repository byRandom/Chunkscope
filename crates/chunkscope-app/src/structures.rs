//! Structure layers: the structures that generate in each overworld
//! structure set, and stronghold positions.
//!
//! Each set attempts a structure at one chunk per region; whether it really
//! generates there (and which structure of the set) is checked against
//! biome and terrain like the game does, on background threads. Mineshafts
//! and ruined portals are not validated yet: their attempts are shown as
//! possible.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;

use eframe::egui::{self, Color32};
use mc_core::biome::OverworldBiomeSource;
use mc_core::structure::stronghold::ring_positions;
use mc_core::structure::validate::{Outcome, StructureGenerator};
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSet, StructureSets};
use mc_core::terrain::OverworldTerrain;

use crate::dimension::Dimension;

/// One structure layer of the map.
pub struct Kind {
    pub dimension: Dimension,
    /// Structure set registry name.
    pub set: &'static str,
    /// Only the structures of the set whose name starts with this (the
    /// Nether's fortresses and bastions share one set).
    pub only: Option<&'static str>,
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

pub const STRONGHOLDS: &str = "minecraft:strongholds";
/// The world spawn layer (not a structure set).
pub const SPAWN: &str = "chunkscope:spawn";

/// Overworld structure layers, in legend order.
pub static KINDS: &[Kind] = &[
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: SPAWN,
        label: "World spawn",
        glyph: "W",
        color: rgb(0xf4f4f4),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:villages",
        label: "Villages",
        glyph: "V",
        color: rgb(0xe0a458),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: STRONGHOLDS,
        label: "Strongholds",
        glyph: "S",
        color: rgb(0xb67cf2),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:woodland_mansions",
        label: "Woodland mansions",
        glyph: "WM",
        color: rgb(0x9a6b3f),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:ocean_monuments",
        label: "Ocean monuments",
        glyph: "OM",
        color: rgb(0x4fd1c5),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:pillager_outposts",
        label: "Pillager outposts",
        glyph: "PO",
        color: rgb(0x9aa0a6),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:desert_pyramids",
        label: "Desert pyramids",
        glyph: "DP",
        color: rgb(0xf2d16b),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:jungle_temples",
        label: "Jungle temples",
        glyph: "JT",
        color: rgb(0x7bc96f),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:swamp_huts",
        label: "Swamp huts",
        glyph: "SH",
        color: rgb(0x6f8f5f),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:igloos",
        label: "Igloos",
        glyph: "IG",
        color: rgb(0xcfe8ff),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:ancient_cities",
        label: "Ancient cities",
        glyph: "AC",
        color: rgb(0x3aa7a0),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:trial_chambers",
        label: "Trial chambers",
        glyph: "TC",
        color: rgb(0xd9824a),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:trail_ruins",
        label: "Trail ruins",
        glyph: "TR",
        color: rgb(0xc98b6b),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:abandoned_camp",
        label: "Abandoned camps",
        glyph: "AB",
        color: rgb(0xb5a36a),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:ruined_portals",
        label: "Ruined portals",
        glyph: "RP",
        color: rgb(0xa77bd9),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:shipwrecks",
        label: "Shipwrecks",
        glyph: "SW",
        color: rgb(0x8c6d4f),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:ocean_ruins",
        label: "Ocean ruins",
        glyph: "OR",
        color: rgb(0x5d8fb3),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:buried_treasures",
        label: "Buried treasure",
        glyph: "BT",
        color: rgb(0xf5c542),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Overworld,
        only: None,
        set: "minecraft:mineshafts",
        label: "Mineshafts",
        glyph: "M",
        color: rgb(0x8f7a66),
        default_on: false,
    },
    Kind {
        dimension: Dimension::Nether,
        only: Some("minecraft:fortress"),
        set: "minecraft:nether_complexes",
        label: "Fortresses",
        glyph: "F",
        color: rgb(0xd9534f),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Nether,
        only: Some("minecraft:bastion"),
        set: "minecraft:nether_complexes",
        label: "Bastions",
        glyph: "B",
        color: rgb(0x8c8c96),
        default_on: true,
    },
    Kind {
        dimension: Dimension::Nether,
        only: None,
        set: "minecraft:nether_fossils",
        label: "Nether fossils",
        glyph: "NF",
        color: rgb(0xe8e0c8),
        default_on: false,
    },
];

/// A structure marker on the map, at the position `/locate` reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub kind: usize,
    pub x: i32,
    pub z: i32,
    /// `false` for attempts that are not validated yet (mineshafts, ruined
    /// portals): the game may skip them.
    pub exact: bool,
    /// The structure that generates (e.g. `minecraft:village_plains`) and
    /// its start position, when validated.
    pub start: Option<(&'static str, i32, i32, i32)>,
}

/// Whether the structures of a layer are validated against biome and
/// terrain (strongholds are exact too).
pub fn validated(kind: usize) -> bool {
    use mc_core::structure::validate::{Structure, StructureKind};
    if KINDS[kind].set == SPAWN {
        return false; // an estimate, see `Spawn`
    }
    let Some(set) = set(KINDS[kind].set) else {
        return false;
    };
    match set.placement {
        StructurePlacement::ConcentricRings { .. } => true,
        StructurePlacement::RandomSpread(_) => set.structures.iter().all(|(name, _)| {
            Structure::find(name).is_some_and(|s| s.kind != StructureKind::NotImplemented)
        }),
    }
}

pub fn set(name: &str) -> Option<&'static StructureSet> {
    STRUCTURE_SETS.find_set(name)
}

/// Expected distance between two attempts of a random-spread set, in
/// blocks (`None` for strongholds).
pub fn spacing_blocks(kind: usize) -> Option<f64> {
    let StructurePlacement::RandomSpread(p) = &set(KINDS[kind].set)?.placement else {
        return None;
    };
    Some(p.spacing as f64 * 16.0 / f64::from(p.frequency).sqrt())
}

/// The markers of `kind` whose chunk lies in `cx0..=cx1`, `cz0..=cz1`:
/// attempts that generate, plus attempts that cannot be validated yet.
pub fn markers(
    kind: usize,
    seed: i64,
    (cx0, cz0, cx1, cz1): (i32, i32, i32, i32),
    generator: &StructureGenerator,
) -> Vec<Marker> {
    let mut out = Vec::new();
    let Some(set) = set(KINDS[kind].set) else {
        return out;
    };
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        return out;
    };
    let mut push = |cx: i32, cz: i32| {
        if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) != Some(true) {
            return;
        }
        let (x, _, z) = p.locate_pos(cx, cz);
        let outcome = generator.attempt(set, cx, cz);
        if let Some((structure, (sx, sy, sz))) = outcome.generated() {
            if KINDS[kind]
                .only
                .is_some_and(|prefix| !structure.name.starts_with(prefix))
            {
                return;
            }
            out.push(Marker {
                kind,
                x,
                z,
                exact: true,
                start: Some((structure.name, sx, sy, sz)),
            });
        } else if let Outcome::Unknown(_) = outcome {
            out.push(Marker {
                kind,
                x,
                z,
                exact: false,
                start: None,
            });
        }
    };
    if p.spacing == 1 {
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                push(cx, cz);
            }
        }
        return out;
    }
    let s = p.spacing;
    for rz in cz0.div_euclid(s)..=cz1.div_euclid(s) {
        for rx in cx0.div_euclid(s)..=cx1.div_euclid(s) {
            let (cx, cz) = p.region_chunk(seed, rx, rz);
            if (cx0..=cx1).contains(&cx) && (cz0..=cz1).contains(&cz) {
                push(cx, cz);
            }
        }
    }
    out
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

struct Queue {
    /// Requests, most urgent last.
    pending: Vec<BlockKey>,
    working: HashSet<BlockKey>,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}

/// Validated structure markers, computed on background threads.
pub struct StructureCache {
    shared: Arc<Shared>,
    results: mpsc::Receiver<(BlockKey, Vec<Marker>)>,
    blocks: HashMap<BlockKey, Vec<Marker>>,
    in_flight: usize,
}

/// How many squares stay in memory.
const CACHE_LIMIT: usize = 20_000;

impl StructureCache {
    pub fn new(ctx: &egui::Context) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                pending: Vec::new(),
                working: HashSet::new(),
            }),
            wake: Condvar::new(),
        });
        let (tx, rx) = mpsc::channel();
        let workers = thread::available_parallelism()
            .map_or(2, |n| n.get())
            .div_ceil(2)
            .max(1);
        for i in 0..workers {
            let shared = shared.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            thread::Builder::new()
                .name(format!("structure-worker-{i}"))
                .spawn(move || worker(&shared, &tx, &ctx))
                .expect("spawn structure worker");
        }
        Self {
            shared,
            results: rx,
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
        while let Ok((key, markers)) = self.results.try_recv() {
            self.blocks.insert(key, markers);
        }
        if self.blocks.len() > CACHE_LIMIT {
            self.blocks.clear();
        }
    }

    pub fn get(&self, key: &BlockKey) -> Option<&[Marker]> {
        self.blocks.get(key).map(Vec::as_slice)
    }

    /// Replace the work queue with the squares still missing from `wanted`
    /// (most urgent first).
    pub fn request(&mut self, wanted: &[BlockKey]) {
        let mut q = self.shared.queue.lock().expect("structure queue");
        let mut pending: Vec<BlockKey> = wanted
            .iter()
            .filter(|k| !self.blocks.contains_key(k) && !q.working.contains(k))
            .copied()
            .collect();
        self.in_flight = pending.len() + q.working.len();
        pending.reverse();
        if q.pending != pending {
            q.pending = pending;
            self.shared.wake.notify_all();
        }
    }
}

/// The sources structure validation needs, for one seed.
struct World {
    overworld: OverworldBiomeSource,
    terrain: OverworldTerrain,
    nether: mc_core::biome::NetherBiomeSource,
    nether_terrain: mc_core::terrain::NetherTerrain,
}

impl World {
    fn new(seed: i64) -> Self {
        Self {
            overworld: OverworldBiomeSource::new(seed),
            terrain: OverworldTerrain::new(seed),
            nether: mc_core::biome::NetherBiomeSource::new(seed),
            nether_terrain: mc_core::terrain::NetherTerrain::new(seed),
        }
    }
}

fn worker(shared: &Shared, tx: &mpsc::Sender<(BlockKey, Vec<Marker>)>, ctx: &egui::Context) {
    let mut world: Option<(i64, World)> = None;
    loop {
        let key = {
            let mut q = shared.queue.lock().expect("structure queue");
            loop {
                if let Some(key) = q.pending.pop() {
                    q.working.insert(key);
                    break key;
                }
                q = shared.wake.wait(q).expect("structure queue");
            }
        };
        if world.as_ref().is_none_or(|w| w.0 != key.seed) {
            world = Some((key.seed, World::new(key.seed)));
        }
        let (_, w) = world.as_ref().expect("world just set");
        let generator = match KINDS[key.kind].dimension {
            Dimension::Overworld => StructureGenerator::new(key.seed, &w.overworld, &w.terrain),
            Dimension::Nether => StructureGenerator::nether(key.seed, &w.nether, &w.nether_terrain),
        };
        let c = (key.bx * BLOCK_CHUNKS, key.bz * BLOCK_CHUNKS);
        let bounds = (c.0, c.1, c.0 + BLOCK_CHUNKS - 1, c.1 + BLOCK_CHUNKS - 1);
        let found = markers(key.kind, key.seed, bounds, &generator);
        let closed = tx.send((key, found)).is_err();
        shared
            .queue
            .lock()
            .expect("structure queue")
            .working
            .remove(&key);
        if closed {
            return;
        }
        ctx.request_repaint();
    }
}

/// The climate origin `(x, z)` the game searches the spawn around (exact),
/// and the estimated spawn block `(x, y, z)`.
pub type SpawnPoints = ((i32, i32), (i32, i32, i32));

/// The world spawn: its origin (exact) and the spawn block (an estimate),
/// computed on a background thread.
pub struct Spawn {
    rx: Option<mpsc::Receiver<SpawnPoints>>,
    value: Option<SpawnPoints>,
}

impl Spawn {
    pub fn new(seed: i64, ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::Builder::new()
            .name("spawn".into())
            .spawn(move || {
                let biomes = OverworldBiomeSource::new(seed);
                let terrain = OverworldTerrain::new(seed);
                let origin = mc_core::spawn::spawn_origin(biomes.climate());
                let block = mc_core::spawn::spawn_estimate(biomes.climate(), &terrain);
                let _ = tx.send((origin, block));
                ctx.request_repaint();
            })
            .expect("spawn spawn thread");
        Self {
            rx: Some(rx),
            value: None,
        }
    }

    pub fn is_loading(&mut self) -> bool {
        if let Some(rx) = &self.rx
            && let Ok(v) = rx.try_recv()
        {
            self.value = Some(v);
            self.rx = None;
        }
        self.rx.is_some()
    }

    pub fn get(&self) -> Option<SpawnPoints> {
        self.value
    }
}

/// Stronghold positions, computed on a background thread (they need a
/// biome search around each of the 128 ring positions).
pub struct Strongholds {
    rx: Option<mpsc::Receiver<Vec<(i32, i32)>>>,
    positions: Vec<(i32, i32)>,
}

impl Strongholds {
    pub fn new(seed: i64, ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::Builder::new()
            .name("strongholds".into())
            .spawn(move || {
                let positions = set(STRONGHOLDS)
                    .and_then(|s| s.concentric_rings())
                    .map(|rings| ring_positions(seed, &rings, &OverworldBiomeSource::new(seed)))
                    .unwrap_or_default();
                let _ = tx.send(positions);
                ctx.request_repaint();
            })
            .expect("spawn stronghold thread");
        Self {
            rx: Some(rx),
            positions: Vec::new(),
        }
    }

    pub fn is_loading(&mut self) -> bool {
        if let Some(rx) = &self.rx
            && let Ok(p) = rx.try_recv()
        {
            self.positions = p;
            self.rx = None;
        }
        self.rx.is_some()
    }

    /// Block positions, as `/locate` reports them (the chunk's corner).
    pub fn positions(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.positions.iter().map(|&(cx, cz)| (cx << 4, cz << 4))
    }
}
