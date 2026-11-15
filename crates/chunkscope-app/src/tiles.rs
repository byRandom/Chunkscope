//! The biome map, cut into square tiles computed on background threads.
//!
//! A tile at level `L` samples one biome every `2^L` quarts (4 blocks at
//! level 0), [`TILE_PX`] samples per side. The UI asks for the tiles it
//! needs each frame, nearest first; workers (a [`WorkPool`]) take the most
//! urgent request, so panning away drops stale work.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use crate::dimension::{BiomeSource, Dimension};
use crate::work::WorkPool;
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

struct Tile {
    texture: TextureHandle,
    last_used: u64,
}

pub struct TileCache {
    /// Computes tiles: their pixels and how long they took (ms).
    pool: WorkPool<TileKey, (ColorImage, f32)>,
    tiles: HashMap<TileKey, Tile>,
    in_flight: usize,
    frame: u64,
    /// Moving average of the time to compute one tile.
    tile_ms: f32,
}

/// How many tiles stay in memory (64 KiB of texture each).
const CACHE_LIMIT: usize = 1200;

/// A worker's biome source, kept while the seed and dimension stay.
type WorkerSource = Option<((i64, Dimension), BiomeSource)>;

impl TileCache {
    pub fn new(ctx: &egui::Context) -> Self {
        let workers = thread::available_parallelism()
            .map_or(2, |n| n.get())
            .saturating_sub(1)
            .max(1);
        let palette = crate::biome_colors::palette();
        let ctx = ctx.clone();
        let pool = WorkPool::new(
            "tile-worker",
            workers,
            Arc::new(move || ctx.request_repaint()),
            move |source: &mut WorkerSource, key: TileKey| {
                let world = (key.seed, key.dimension);
                if source.as_ref().is_none_or(|(w, _)| *w != world) {
                    *source = Some((world, BiomeSource::new(key.dimension, key.seed)));
                }
                let (_, src) = source.as_ref().expect("source just set");
                let start = std::time::Instant::now();
                let image = compute(src, key, &palette);
                (image, start.elapsed().as_secs_f32() * 1000.0)
            },
        );
        Self {
            pool,
            tiles: HashMap::new(),
            in_flight: 0,
            frame: 0,
            tile_ms: 0.0,
        }
    }

    pub fn workers(&self) -> usize {
        self.pool.workers()
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
        for (key, (image, millis)) in self.pool.receive() {
            self.tile_ms = if self.tile_ms == 0.0 {
                millis
            } else {
                self.tile_ms * 0.9 + millis * 0.1
            };
            let name = format!("tile {key:?}");
            let texture = ctx.load_texture(name, image, TextureOptions::NEAREST);
            self.tiles.insert(
                key,
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

    /// Ask for the tiles still missing from `wanted` (most urgent first),
    /// dropping earlier requests.
    pub fn request(&mut self, wanted: &[TileKey]) {
        self.in_flight = self.pool.request(wanted, |k| self.tiles.contains_key(k));
    }
}

fn compute(source: &BiomeSource, key: TileKey, palette: &[Color32]) -> ColorImage {
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
            let mut color = palette.get(b as usize).copied().unwrap_or(Color32::RED);
            // The End: the void between the islands, darkened.
            if source.end_land(clamp_i32(qx * 4), clamp_i32(qz * 4)) == Some(false) {
                color = void_shade(color);
            }
            pixels[pz * n + px] = color;
        }
    }
    ColorImage::new([n, n], pixels)
}

/// A biome color darkened for the End's void.
fn void_shade(c: Color32) -> Color32 {
    let f = |v: u8| (v as u16 * 7 / 20) as u8;
    Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

fn clamp_i32(v: i64) -> i32 {
    v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}
