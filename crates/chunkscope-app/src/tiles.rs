//! The biome map, cut into square tiles computed on background threads.
//!
//! A tile at level `L` samples one biome every `2^L` quarts (4 blocks at
//! level 0), [`TILE_PX`] samples per side. The UI asks for the tiles it
//! needs each frame, nearest first; workers take the most urgent request,
//! so panning away drops stale work.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;

use crate::dimension::{BiomeSource, Dimension};
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions};

/// Samples per tile side.
pub const TILE_PX: i32 = 128;
/// Coarsest level: one sample every 2^MAX_LEVEL quarts.
pub const MAX_LEVEL: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub seed: i64,
    pub dimension: Dimension,
    /// Block Y the biomes are sampled at.
    pub y: i32,
    pub level: u8,
    pub tx: i32,
    pub tz: i32,
}

impl TileKey {
    /// Blocks covered by one sample.
    pub fn blocks_per_sample(level: u8) -> i64 {
        4i64 << level
    }

    /// Blocks covered by one tile side.
    pub fn tile_blocks(level: u8) -> i64 {
        Self::blocks_per_sample(level) * TILE_PX as i64
    }
}

struct Queue {
    /// Requests, most urgent last.
    pending: Vec<TileKey>,
    /// Requests a worker has taken and not finished yet.
    working: HashSet<TileKey>,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}

struct Done {
    key: TileKey,
    image: ColorImage,
    millis: f32,
}

struct Tile {
    texture: TextureHandle,
    last_used: u64,
}

pub struct TileCache {
    shared: Arc<Shared>,
    results: mpsc::Receiver<Done>,
    tiles: HashMap<TileKey, Tile>,
    in_flight: usize,
    frame: u64,
    workers: usize,
    /// Moving average of the time to compute one tile.
    tile_ms: f32,
}

/// How many tiles stay in memory (64 KiB of texture each).
const CACHE_LIMIT: usize = 1200;

impl TileCache {
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
            .saturating_sub(1)
            .max(1);
        let palette = Arc::new(crate::biome_colors::palette());
        for i in 0..workers {
            let shared = shared.clone();
            let tx = tx.clone();
            let ctx = ctx.clone();
            let palette = palette.clone();
            thread::Builder::new()
                .name(format!("tile-worker-{i}"))
                .spawn(move || worker(&shared, &tx, &ctx, &palette))
                .expect("spawn tile worker");
        }
        Self {
            shared,
            results: rx,
            tiles: HashMap::new(),
            in_flight: 0,
            frame: 0,
            workers,
            tile_ms: 0.0,
        }
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    pub fn loaded(&self) -> usize {
        self.tiles.len()
    }

    /// Average time to compute one tile, in milliseconds.
    pub fn tile_ms(&self) -> f32 {
        self.tile_ms
    }

    /// Tiles requested but not computed yet.
    pub fn pending(&self) -> usize {
        self.in_flight
    }

    /// Upload finished tiles. Call once per frame, before drawing.
    pub fn receive(&mut self, ctx: &egui::Context) {
        self.frame += 1;
        while let Ok(done) = self.results.try_recv() {
            self.tile_ms = if self.tile_ms == 0.0 {
                done.millis
            } else {
                self.tile_ms * 0.9 + done.millis * 0.1
            };
            let name = format!("tile {:?}", done.key);
            let texture = ctx.load_texture(name, done.image, TextureOptions::NEAREST);
            self.tiles.insert(
                done.key,
                Tile {
                    texture,
                    last_used: self.frame,
                },
            );
        }
        if self.tiles.len() > CACHE_LIMIT {
            let mut ages: Vec<(u64, TileKey)> =
                self.tiles.iter().map(|(k, t)| (t.last_used, *k)).collect();
            ages.sort_unstable_by_key(|(age, _)| *age);
            for (_, key) in ages.iter().take(self.tiles.len() - CACHE_LIMIT * 3 / 4) {
                self.tiles.remove(key);
            }
        }
    }

    /// The texture of a tile, if computed.
    pub fn get(&mut self, key: &TileKey) -> Option<&TextureHandle> {
        let frame = self.frame;
        self.tiles.get_mut(key).map(|t| {
            t.last_used = frame;
            &t.texture
        })
    }

    /// Replace the work queue with the tiles still missing from `wanted`
    /// (most urgent first).
    pub fn request(&mut self, wanted: &[TileKey]) {
        let mut q = self.shared.queue.lock().expect("tile queue");
        let mut pending: Vec<TileKey> = wanted
            .iter()
            .filter(|k| !self.tiles.contains_key(k) && !q.working.contains(k))
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

fn worker(shared: &Shared, tx: &mpsc::Sender<Done>, ctx: &egui::Context, palette: &[Color32]) {
    let mut source: Option<((i64, Dimension), BiomeSource)> = None;
    loop {
        let key = {
            let mut q = shared.queue.lock().expect("tile queue");
            loop {
                if let Some(key) = q.pending.pop() {
                    q.working.insert(key);
                    break key;
                }
                q = shared.wake.wait(q).expect("tile queue");
            }
        };
        let world = (key.seed, key.dimension);
        if source.as_ref().is_none_or(|(w, _)| *w != world) {
            source = Some((world, BiomeSource::new(key.dimension, key.seed)));
        }
        let (_, src) = source.as_ref().expect("source just set");
        let start = std::time::Instant::now();
        let mut done = compute(src, key, palette);
        done.millis = start.elapsed().as_secs_f32() * 1000.0;
        // Send before leaving `working`, so the UI never sees the tile as
        // neither loaded nor in progress.
        let closed = tx.send(done).is_err();
        shared
            .queue
            .lock()
            .expect("tile queue")
            .working
            .remove(&key);
        if closed {
            return; // the app is closing
        }
        ctx.request_repaint();
    }
}

fn compute(source: &BiomeSource, key: TileKey, palette: &[Color32]) -> Done {
    // Start every tile from a fresh search state, so a tile looks the same
    // whichever thread computed it (see docs/PARITY.md on query order).
    source.reset_search();
    let n = TILE_PX as usize;
    let step = 1i64 << key.level;
    let qx0 = key.tx as i64 * n as i64 * step;
    let qz0 = key.tz as i64 * n as i64 * step;
    let qy = key.y >> 2;
    let mut pixels = vec![Color32::BLACK; n * n];
    for pz in 0..n {
        // Serpentine order keeps consecutive queries next to each other.
        for i in 0..n {
            let px = if pz % 2 == 0 { i } else { n - 1 - i };
            let qx = qx0 + px as i64 * step;
            let qz = qz0 + pz as i64 * step;
            let b = source.biome(clamp_i32(qx), qy, clamp_i32(qz)).0;
            pixels[pz * n + px] = palette.get(b as usize).copied().unwrap_or(Color32::RED);
        }
    }
    Done {
        key,
        image: ColorImage::new([n, n], pixels),
        millis: 0.0,
    }
}

fn clamp_i32(v: i64) -> i32 {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}
