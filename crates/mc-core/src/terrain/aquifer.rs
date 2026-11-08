//! `Aquifer.NoiseBasedAquifer` (26.x): decides what fills each non-solid
//! block (air, water or lava) and where barrier stone separates aquifers.

use std::collections::HashMap;

use java_compat::math::min_f64;

use crate::density::{DensitySampler, DensityVolume, EvalContext, NodeId};
use crate::mth;
use crate::random::{PositionalRandomFactory, RandomSource, XoroshiroPositionalRandomFactory};

/// What `computeSubstance` puts in a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Substance {
    /// `null`: the dimension's default block (stone).
    Solid,
    Air,
    Water,
    Lava,
}

impl Substance {
    /// `Heightmap.Types.WORLD_SURFACE_WG`: anything but air.
    pub fn is_world_surface(self) -> bool {
        self != Substance::Air
    }

    /// `Heightmap.Types.OCEAN_FLOOR_WG`: blocks that block motion (not fluids).
    pub fn is_ocean_floor(self) -> bool {
        self == Substance::Solid
    }
}

/// `Aquifer.FluidStatus`: below `fluid_level` the fluid, above it air.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FluidStatus {
    pub fluid_level: i32,
    pub fluid: Substance,
}

impl FluidStatus {
    /// `FluidStatus.at(y)`.
    pub fn at(&self, y: i32) -> Substance {
        if y < self.fluid_level {
            self.fluid
        } else {
            Substance::Air
        }
    }
}

/// `DimensionType.WAY_BELOW_MIN_Y` (`MIN_Y << 4`): "no fluid here".
const WAY_BELOW_MIN_Y: i32 = -2032 << 4;

/// The overworld's global fluid picker (`NoiseBasedChunkGenerator.createFluidPicker`):
/// lava below `min(-54, sea level)`, otherwise water up to sea level.
#[derive(Clone, Copy, Debug)]
pub struct GlobalFluidPicker {
    pub sea_level: i32,
}

impl GlobalFluidPicker {
    pub fn compute_fluid(&self, _x: i32, y: i32, _z: i32) -> FluidStatus {
        if y < (-54).min(self.sea_level) {
            FluidStatus {
                fluid_level: -54,
                fluid: Substance::Lava,
            }
        } else {
            FluidStatus {
                fluid_level: self.sea_level,
                fluid: Substance::Water,
            }
        }
    }
}

/// Root nodes of the aquifer's density functions (`Aquifer.Config`).
#[derive(Clone, Copy, Debug)]
pub struct AquiferRoots {
    pub barrier: NodeId,
    pub floodedness: NodeId,
    pub spread: NodeId,
    pub lava: NodeId,
    pub exclusion: NodeId,
    pub surface_level: NodeId,
}

/// `SURFACE_SAMPLING_OFFSETS_IN_CHUNKS`.
const SURFACE_SAMPLING_OFFSETS: [(i32, i32); 13] = [
    (0, 0),
    (-2, -1),
    (-1, -1),
    (0, -1),
    (1, -1),
    (-3, 0),
    (-2, 0),
    (-1, 0),
    (1, 0),
    (-2, 1),
    (-1, 1),
    (0, 1),
    (1, 1),
];

fn grid_x(x: i32) -> i32 {
    x >> 4
}
fn from_grid_x(g: i32, offset: i32) -> i32 {
    (g << 4) + offset
}
fn grid_y(y: i32) -> i32 {
    y.div_euclid(12)
}
fn from_grid_y(g: i32, offset: i32) -> i32 {
    g * 12 + offset
}
fn grid_z(z: i32) -> i32 {
    z >> 4
}
fn from_grid_z(g: i32, offset: i32) -> i32 {
    (g << 4) + offset
}

/// `similarity(a, b)`: `1 - (b - a) / 25`.
fn similarity(a: i32, b: i32) -> f64 {
    1.0 - (b - a) as f64 / 25.0
}

/// `Mth.inverseLerp`, `Mth.lerp(double)`, `Mth.clampedLerp`, `Mth.map`, `Mth.clampedMap`.
fn inverse_lerp(v: f64, a: f64, b: f64) -> f64 {
    (v - a) / (b - a)
}
fn lerp_f64(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}
fn clamped_lerp(t: f64, a: f64, b: f64) -> f64 {
    if t < 0.0 {
        a
    } else if t > 1.0 {
        b
    } else {
        lerp_f64(t, a, b)
    }
}
fn map(v: f64, a: f64, b: f64, c: f64, d: f64) -> f64 {
    lerp_f64(inverse_lerp(v, a, b), c, d)
}
fn clamped_map(v: f64, a: f64, b: f64, c: f64, d: f64) -> f64 {
    clamped_lerp(inverse_lerp(v, a, b), c, d)
}
/// `Mth.clamp(double, double, double)`.
fn clamp_f64(v: f64, min: f64, max: f64) -> f64 {
    if v < min { min } else { min_f64(v, max) }
}

/// `Aquifer.NoiseBasedAquifer`, bound to one `NoiseChunk`'s volume and
/// sampling context.
pub struct Aquifer<'a> {
    sampler: &'a DensitySampler,
    ctx: &'a EvalContext,
    roots: AquiferRoots,
    random: XoroshiroPositionalRandomFactory,
    picker: GlobalFluidPicker,
    skip_sampling_above_y: i32,
    locations: HashMap<(i32, i32, i32), (i32, i32, i32)>,
    statuses: HashMap<(i32, i32, i32), FluidStatus>,
    surface_level_cache: HashMap<(i32, i32), i32>,
}

impl<'a> Aquifer<'a> {
    /// `NoiseBasedAquifer(samplers, config, random, volume, picker)`. Like the
    /// game, this samples the surface level over the volume's area (volume
    /// mode, through `ctx`) to know above which Y aquifers can be skipped.
    pub fn new(
        sampler: &'a DensitySampler,
        ctx: &'a EvalContext,
        roots: AquiferRoots,
        random: XoroshiroPositionalRandomFactory,
        volume: &DensityVolume,
        picker: GlobalFluidPicker,
    ) -> Self {
        let mut aquifer = Self {
            sampler,
            ctx,
            roots,
            random,
            picker,
            skip_sampling_above_y: 0,
            locations: HashMap::new(),
            statuses: HashMap::new(),
            surface_level_cache: HashMap::new(),
        };
        let min_grid_x = grid_x(volume.min_block_x - 5);
        let max_grid_x = grid_x(volume.max_block_x() - 5) + 1;
        let min_grid_z = grid_z(volume.min_block_z - 5);
        let max_grid_z = grid_z(volume.max_block_z() - 5) + 1;
        let surface = aquifer.max_surface_level(
            from_grid_x(min_grid_x, 0),
            from_grid_z(min_grid_z, 0),
            from_grid_x(max_grid_x, 9),
            from_grid_z(max_grid_z, 9),
        ) + 8;
        aquifer.skip_sampling_above_y = from_grid_y(grid_y(surface + 12) - 1, 11) - 1;
        aquifer
    }

    /// `surfaceLevel(x, z)`: point mode, cached per quart-aligned column.
    fn surface_level(&mut self, x: i32, z: i32) -> i32 {
        let key = ((x >> 2) << 2, (z >> 2) << 2);
        if let Some(&v) = self.surface_level_cache.get(&key) {
            return v;
        }
        let v = mth::floor(self.sampler.sample_value(
            self.ctx,
            self.roots.surface_level,
            key.0,
            0,
            key.1,
        ) as f64);
        self.surface_level_cache.insert(key, v);
        v
    }

    /// `maxSurfaceLevel(x0, z0, x1, z1)`: volume mode over a quart grid,
    /// filling the surface level cache.
    fn max_surface_level(&mut self, x0: i32, z0: i32, x1: i32, z1: i32) -> i32 {
        let (qx0, qx1, qz0, qz1) = (x0 >> 2, x1 >> 2, z0 >> 2, z1 >> 2);
        let volume = DensityVolume::with_steps(
            qx1 - qx0 + 1,
            1,
            qz1 - qz0 + 1,
            qx0 << 2,
            0,
            qz0 << 2,
            4,
            1,
            4,
        );
        let mut buffer = vec![0.0f32; volume.size()];
        self.sampler
            .sample_node(self.ctx, self.roots.surface_level, &mut buffer, &volume);
        let mut max = i32::MIN;
        for zi in 0..volume.size_z {
            for xi in 0..volume.size_x {
                let v = mth::floor(buffer[volume.index(xi, 0, zi)] as f64);
                self.surface_level_cache
                    .insert((volume.block_x(xi), volume.block_z(zi)), v);
                max = max.max(v);
            }
        }
        max
    }

    /// The aquifer center of grid cell `(gx, gy, gz)`.
    fn location(&mut self, gx: i32, gy: i32, gz: i32) -> (i32, i32, i32) {
        if let Some(&l) = self.locations.get(&(gx, gy, gz)) {
            return l;
        }
        let mut r = self.random.at(gx, gy, gz);
        let x = from_grid_x(gx, r.next_int_bounded(10));
        let y = from_grid_y(gy, r.next_int_bounded(9));
        let z = from_grid_z(gz, r.next_int_bounded(10));
        self.locations.insert((gx, gy, gz), (x, y, z));
        (x, y, z)
    }

    /// `getAquiferStatus`: the fluid of an aquifer cell, computed at its center.
    fn status(&mut self, cell: (i32, i32, i32)) -> FluidStatus {
        if let Some(&s) = self.statuses.get(&cell) {
            return s;
        }
        let (x, y, z) = self.location(cell.0, cell.1, cell.2);
        let s = self.compute_fluid(x, y, z);
        self.statuses.insert(cell, s);
        s
    }

    /// `computeSubstance(x, y, z, density)`.
    pub fn compute_substance(&mut self, x: i32, y: i32, z: i32, density: f64) -> Substance {
        if density > 0.0 {
            return Substance::Solid;
        }
        let global = self.picker.compute_fluid(x, y, z);
        if y > self.skip_sampling_above_y {
            return global.at(y);
        }
        if global.at(y) == Substance::Lava {
            return Substance::Lava;
        }
        let (gx, gy, gz) = (grid_x(x - 5), grid_y(y + 1), grid_z(z - 5));
        // the 4 nearest aquifer centers; ties put the newer one first
        let mut d = [i32::MAX; 4];
        let mut cells = [(0, 0, 0); 4];
        for dx in 0..=1 {
            for dy in -1..=1 {
                for dz in 0..=1 {
                    let cell = (gx + dx, gy + dy, gz + dz);
                    let (lx, ly, lz) = self.location(cell.0, cell.1, cell.2);
                    let (ox, oy, oz) = (lx - x, ly - y, lz - z);
                    let dist = ox * ox + oy * oy + oz * oz;
                    if let Some(k) = d.iter().position(|&e| e >= dist) {
                        d.copy_within(k..3, k + 1);
                        cells.copy_within(k..3, k + 1);
                        d[k] = dist;
                        cells[k] = cell;
                    }
                }
            }
        }
        let s1 = self.status(cells[0]);
        let sim12 = similarity(d[0], d[1]);
        let block = s1.at(y);
        if sim12 <= 0.0 {
            return block;
        }
        if block == Substance::Water
            && self.picker.compute_fluid(x, y - 1, z).at(y - 1) == Substance::Lava
        {
            return block;
        }
        let mut barrier = f64::NAN;
        let s2 = self.status(cells[1]);
        let p = sim12 * self.pressure(x, y, z, &mut barrier, s1, s2);
        if density + p > 0.0 {
            return Substance::Solid;
        }
        let s3 = self.status(cells[2]);
        let sim13 = similarity(d[0], d[2]);
        if sim13 > 0.0 {
            let p = sim12 * sim13 * self.pressure(x, y, z, &mut barrier, s1, s3);
            if density + p > 0.0 {
                return Substance::Solid;
            }
        }
        let sim23 = similarity(d[1], d[2]);
        if sim23 > 0.0 {
            let p = sim12 * sim23 * self.pressure(x, y, z, &mut barrier, s2, s3);
            if density + p > 0.0 {
                return Substance::Solid;
            }
        }
        block
    }

    /// `calculatePressure`. `barrier` caches the barrier noise for this block.
    fn pressure(
        &self,
        x: i32,
        y: i32,
        z: i32,
        barrier: &mut f64,
        a: FluidStatus,
        b: FluidStatus,
    ) -> f64 {
        let (fa, fb) = (a.at(y), b.at(y));
        if (fa == Substance::Lava && fb == Substance::Water)
            || (fa == Substance::Water && fb == Substance::Lava)
        {
            return 2.0;
        }
        let diff = (a.fluid_level - b.fluid_level).abs();
        if diff == 0 {
            return 0.0;
        }
        let mid = 0.5 * (a.fluid_level + b.fluid_level) as f64;
        let rel = y as f64 + 0.5 - mid;
        let half = diff as f64 / 2.0;
        let t = half - rel.abs();
        let v = if rel > 0.0 {
            let u = 0.0 + t;
            if u > 0.0 { u / 1.5 } else { u / 2.5 }
        } else {
            let u = 3.0 + t;
            if u > 0.0 { u / 3.0 } else { u / 10.0 }
        };
        // written like the game's comparisons, so NaN samples the barrier
        // (`!(-2.0..=2.0).contains(&v)` would not)
        #[allow(clippy::manual_range_contains)]
        let noise = if v < -2.0 || v > 2.0 {
            0.0
        } else {
            if barrier.is_nan() {
                *barrier = self
                    .sampler
                    .sample_value(self.ctx, self.roots.barrier, x, y, z)
                    as f64;
            }
            *barrier
        };
        2.0 * (noise + v)
    }

    /// `computeFluid`: the fluid of an aquifer centered at `(x, y, z)`.
    fn compute_fluid(&mut self, x: i32, y: i32, z: i32) -> FluidStatus {
        let global = self.picker.compute_fluid(x, y, z);
        let mut min_surface = i32::MAX;
        let (up, down) = (y + 12, y - 12);
        let mut fluid_present = false;
        for (ox, oz) in SURFACE_SAMPLING_OFFSETS {
            let (sx, sz) = (x + (ox << 4), z + (oz << 4));
            let surface = self.surface_level(sx, sz);
            let adjusted = surface + 8;
            let center = ox == 0 && oz == 0;
            if center && down > adjusted {
                return global;
            }
            let above_top = up > adjusted;
            if above_top || center {
                let status = self.picker.compute_fluid(sx, adjusted, sz);
                if status.at(adjusted) != Substance::Air {
                    if center {
                        fluid_present = true;
                    }
                    if above_top {
                        return status;
                    }
                }
            }
            min_surface = min_surface.min(surface);
        }
        let level = self.compute_surface_level(x, y, z, global, min_surface, fluid_present);
        FluidStatus {
            fluid_level: level,
            fluid: self.compute_fluid_type(x, y, z, global, level),
        }
    }

    /// `computeSurfaceLevel`.
    fn compute_surface_level(
        &self,
        x: i32,
        y: i32,
        z: i32,
        global: FluidStatus,
        min_surface: i32,
        fluid_present: bool,
    ) -> i32 {
        let (a, b);
        if self
            .sampler
            .sample_value(self.ctx, self.roots.exclusion, x, y, z) as f64
            > 0.0
        {
            a = -1.0;
            b = -1.0;
        } else {
            let distance = min_surface + 8 - y;
            let t = if fluid_present {
                clamped_map(distance as f64, 0.0, 64.0, 1.0, 0.0)
            } else {
                0.0
            };
            let flood = clamp_f64(
                self.sampler
                    .sample_value(self.ctx, self.roots.floodedness, x, y, z) as f64,
                -1.0,
                1.0,
            );
            let m1 = map(t, 1.0, 0.0, -0.3, 0.8);
            let m2 = map(t, 1.0, 0.0, -0.8, 0.4);
            a = flood - m2;
            b = flood - m1;
        }
        if b > 0.0 {
            global.fluid_level
        } else if a > 0.0 {
            self.compute_randomized_fluid_surface_level(x, y, z, min_surface)
        } else {
            WAY_BELOW_MIN_Y
        }
    }

    /// `computeRandomizedFluidSurfaceLevel`.
    fn compute_randomized_fluid_surface_level(
        &self,
        x: i32,
        y: i32,
        z: i32,
        min_surface: i32,
    ) -> i32 {
        let (gx, gy, gz) = (x.div_euclid(16), y.div_euclid(40), z.div_euclid(16));
        let base = gy * 40 + 20;
        let spread = (self
            .sampler
            .sample_value(self.ctx, self.roots.spread, gx, gy, gz)
            * 10.0) as f64;
        let quantized = mth::floor(spread / 3.0) * 3; // Mth.quantize(spread, 3)
        min_surface.min(base + quantized)
    }

    /// `computeFluidType`: deep aquifers turn to lava where the lava noise is strong.
    fn compute_fluid_type(
        &self,
        x: i32,
        y: i32,
        z: i32,
        global: FluidStatus,
        level: i32,
    ) -> Substance {
        if level <= -10 && level != WAY_BELOW_MIN_Y && global.fluid != Substance::Lava {
            let lava = self.sampler.sample_value(
                self.ctx,
                self.roots.lava,
                x.div_euclid(64),
                y.div_euclid(40),
                z.div_euclid(64),
            );
            if (lava as f64).abs() > 0.3 {
                return Substance::Lava;
            }
        }
        global.fluid
    }
}
