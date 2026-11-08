//! Volume-mode evaluation of a compiled [`Tree`] (`DensitySampler.sampleVolume`).

use java_compat::math::{max_f32, min_f32};

use super::tree::{Axis, Node, NodeId, NoiseSource, Tree};
use super::volume::DensityVolume;
use crate::mth;
use crate::noise::blended::FbmSet;
use crate::noise::{NoiseStack, NormalNoise, vanilla};

/// The evaluation state the game keeps in a `SamplerContext`: one cache
/// cell per `CachingDensitySampler` id. Results depend on the order of
/// queries made through the same context, exactly as in the game.
pub struct EvalContext {
    caching: bool,
    cells: std::cell::RefCell<Vec<CacheCell>>,
}

#[derive(Clone, Debug)]
struct CacheCell {
    volume: Option<DensityVolume>,
    buffer: Vec<f32>,
    value_key: i64,
    value: f32,
}

impl Default for CacheCell {
    fn default() -> Self {
        Self {
            volume: None,
            buffer: Vec::new(),
            value_key: 0,
            value: f32::NAN,
        }
    }
}

impl EvalContext {
    /// A context with caches enabled (`SamplerContext.builder().enableCaches()`).
    pub fn caching() -> Self {
        Self {
            caching: true,
            cells: Default::default(),
        }
    }

    /// A context without caches (`SamplerContext.EMPTY_UNCACHED`).
    pub fn uncached() -> Self {
        Self {
            caching: false,
            cells: Default::default(),
        }
    }

    fn cell<T>(&self, id: u32, f: impl FnOnce(&mut CacheCell) -> T) -> T {
        let mut cells = self.cells.borrow_mut();
        if cells.len() <= id as usize {
            cells.resize_with(id as usize + 1, CacheCell::default);
        }
        f(&mut cells[id as usize])
    }
}

/// `BlockPos.asLong`: 26 bits of X, 12 of Y, 26 of Z.
fn block_pos_as_long(x: i32, y: i32, z: i32) -> i64 {
    const HORIZONTAL_MASK: i64 = (1 << 26) - 1;
    ((x as i64 & HORIZONTAL_MASK) << 38) | (y as i64 & 0xFFF) | ((z as i64 & HORIZONTAL_MASK) << 12)
}

/// A tree bound to one world seed: its noises instantiated.
pub struct DensitySampler {
    tree: Tree,
    noises: Vec<NoiseStack>,
}

impl DensitySampler {
    /// Instantiates the tree's noises like `RandomState` does for a world
    /// using the Xoroshiro algorithm (every vanilla dimension).
    pub fn new(tree: Tree, world_seed: i64) -> Self {
        let blended = tree.blended.map(|p| FbmSet::for_world(&p, world_seed));
        let noises = tree
            .noises
            .iter()
            .map(|source| match source {
                NoiseSource::Vanilla(name) => {
                    let (_, params) = vanilla::ALL
                        .iter()
                        .find(|(n, _)| n == name)
                        .unwrap_or_else(|| panic!("unknown noise {name}"));
                    NormalNoise::instantiate(world_seed, name, params)
                }
                NoiseSource::BlendedMinLimit => {
                    blended.as_ref().expect("blended noise").min_limit.clone()
                }
                NoiseSource::BlendedMaxLimit => {
                    blended.as_ref().expect("blended noise").max_limit.clone()
                }
                NoiseSource::BlendedMain => blended.as_ref().expect("blended noise").main.clone(),
            })
            .collect();
        Self { tree, noises }
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// The tree's root over `volume`.
    pub fn sample_volume(&self, ctx: &EvalContext, volume: &DensityVolume) -> Vec<f32> {
        let mut out = vec![0.0; volume.size()];
        self.sample_node(ctx, self.tree.root, &mut out, volume);
        out
    }

    /// One node over `volume` into `out` (`out.len() == volume.size()`).
    pub fn sample_node(&self, ctx: &EvalContext, id: NodeId, out: &mut [f32], v: &DensityVolume) {
        let temp = |child: NodeId| {
            let mut b = vec![0.0f32; v.size()];
            self.sample_node(ctx, child, &mut b, v);
            b
        };
        match self.tree.nodes[id as usize] {
            Node::Constant(c) => out.fill(c),
            Node::Noise {
                noise,
                xz_scale,
                y_scale,
            } => {
                out.fill(0.0);
                self.noises[noise as usize].add_to_volume(out, v, xz_scale, y_scale, 1.0);
            }
            Node::ShiftedXz {
                shift_x,
                shift_z,
                noise,
                xz_scale,
                y_scale,
            } => {
                self.sample_node(ctx, shift_x, out, v);
                let sz = temp(shift_z);
                let noise = &self.noises[noise as usize];
                let mut i = 0;
                for zi in 0..v.size_z {
                    let z = v.block_z(zi) as f64 * xz_scale;
                    for xi in 0..v.size_x {
                        let x = v.block_x(xi) as f64 * xz_scale;
                        for yi in 0..v.size_y {
                            let nx = x + out[i] as f64;
                            let ny = v.block_y(yi) as f64 * y_scale;
                            let nz = z + sz[i] as f64;
                            out[i] = noise.get(nx, ny, nz);
                            i += 1;
                        }
                    }
                }
            }
            Node::ShiftB { noise } => {
                // noise sampled on a volume with X->Z and Y->X swapped
                let swapped = DensityVolume::with_steps(
                    v.size_z,
                    v.size_x,
                    1,
                    v.min_block_z,
                    v.min_block_x,
                    0,
                    v.step_block_z,
                    v.step_block_x,
                    1,
                );
                let mut b = vec![0.0f32; swapped.size()];
                self.noises[noise as usize].add_to_volume(&mut b, &swapped, 0.25, 0.25, 4.0);
                for zi in 0..v.size_z {
                    for xi in 0..v.size_x {
                        let value = b[swapped.index(zi, xi, 0)];
                        let start = v.index(xi, 0, zi);
                        out[start..start + v.size_y as usize].fill(value);
                    }
                }
            }
            Node::ClampedGradient {
                axis,
                from_coordinate,
                min_coordinate,
                max_coordinate,
                from_value,
                coordinate_factor,
            } => {
                let compute = |c: i32| {
                    from_value
                        + (c.clamp(min_coordinate, max_coordinate) - from_coordinate) as f32
                            * coordinate_factor
                };
                let mut i = 0;
                for zi in 0..v.size_z {
                    for xi in 0..v.size_x {
                        for yi in 0..v.size_y {
                            let c = match axis {
                                Axis::X => v.block_x(xi),
                                Axis::Y => v.block_y(yi),
                                Axis::Z => v.block_z(zi),
                            };
                            out[i] = compute(c);
                            i += 1;
                        }
                    }
                }
            }
            Node::Add(a, b) => {
                self.sample_node(ctx, a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, r)| *o += r);
            }
            Node::Mul(a, b) => {
                // no zero short-circuit in volume mode
                self.sample_node(ctx, a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, r)| *o *= r);
            }
            Node::Min { left, right, .. } => {
                self.sample_node(ctx, left, out, v);
                let r = temp(right);
                out.iter_mut().zip(&r).for_each(|(o, &r)| {
                    if r < *o {
                        *o = r
                    }
                });
            }
            Node::Max { left, right, .. } => {
                self.sample_node(ctx, left, out, v);
                let r = temp(right);
                out.iter_mut().zip(&r).for_each(|(o, &r)| {
                    if r > *o {
                        *o = r
                    }
                });
            }
            Node::Sub(a, b) => {
                self.sample_node(ctx, a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, r)| *o += -r);
            }
            Node::ConstSub(c, a) => self.map(ctx, a, out, v, |x| c - x),
            Node::ConstDiv(c, a) => self.map(ctx, a, out, v, |x| c / x),
            Node::ConstMin(a, c) => self.map(ctx, a, out, v, |x| if c < x { c } else { x }),
            Node::ConstMax(a, c) => self.map(ctx, a, out, v, |x| if c > x { c } else { x }),
            Node::FindTopSurface {
                density,
                upper_bound,
                lower_bound,
                cell_height,
            } => {
                assert_eq!(v.size_y, 1, "FindTopSurface needs a single-layer volume");
                self.sample_node(ctx, upper_bound, out, v);
                let mut i = 0;
                for zi in 0..v.size_z {
                    let z = v.block_z(zi);
                    for xi in 0..v.size_x {
                        let x = v.block_x(xi);
                        out[i] = self.find_surface_from(
                            ctx,
                            density,
                            lower_bound,
                            cell_height,
                            x,
                            z,
                            out[i],
                        ) as f32;
                        i += 1;
                    }
                }
            }
            Node::ConstAdd(a, c) => {
                self.sample_node(ctx, a, out, v);
                out.iter_mut().for_each(|o| *o += c);
            }
            Node::ConstMul(a, c) => {
                self.sample_node(ctx, a, out, v);
                out.iter_mut().for_each(|o| *o *= c);
            }
            Node::Abs(a) => self.map(ctx, a, out, v, f32::abs),
            Node::Cube(a) => self.map(ctx, a, out, v, |x| x * x * x),
            Node::Square(a) => self.map(ctx, a, out, v, |x| x * x),
            Node::Squeeze(a) => self.map(ctx, a, out, v, |x| {
                let c = clamp(x, -1.0, 1.0);
                c / 2.0 - c * c * c / 24.0
            }),
            Node::LeakyRelu {
                input,
                negative_factor,
            } => self.map(ctx, input, out, v, |x| {
                if x > 0.0 { x } else { x * negative_factor }
            }),
            Node::Clamp { input, min, max } => self.map(ctx, input, out, v, |x| clamp(x, min, max)),
            Node::Lerp {
                alpha,
                first,
                second,
            } => {
                self.sample_node(ctx, alpha, out, v);
                let f = temp(first);
                let s = temp(second);
                for i in 0..out.len() {
                    out[i] = lerp_alpha(out[i], f[i], s[i]);
                }
            }
            Node::LerpConstFirst {
                alpha,
                first,
                second,
            } => {
                self.sample_node(ctx, alpha, out, v);
                let s = temp(second);
                for i in 0..out.len() {
                    out[i] = lerp_alpha(out[i], first, s[i]);
                }
            }
            Node::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                self.sample_node(ctx, when_in_range, out, v);
                let x = temp(input);
                let other = temp(when_out_of_range);
                for i in 0..out.len() {
                    if !(x[i] >= min_inclusive && x[i] < max_exclusive) {
                        out[i] = other[i];
                    }
                }
            }
            Node::IntervalSelect {
                input,
                thresholds,
                samplers,
            } => {
                self.sample_node(ctx, input, out, v);
                let buffers: Vec<Vec<f32>> = samplers.iter().map(|&s| temp(s)).collect();
                for (i, o) in out.iter_mut().enumerate() {
                    let k = thresholds
                        .iter()
                        .position(|&t| *o < t)
                        .unwrap_or(samplers.len() - 1);
                    *o = buffers[k][i];
                }
            }
            Node::Interpolated {
                input,
                cell_size_xz,
                cell_size_y,
                cell_size_xz_inv,
                cell_size_y_inv,
            } => {
                let cell = Cell {
                    xz: cell_size_xz,
                    y: cell_size_y,
                    xz_inv: cell_size_xz_inv,
                    y_inv: cell_size_y_inv,
                };
                self.interpolate(ctx, input, &cell, out, v);
            }
            Node::SliceY { input, y } => {
                if v.size_y == 1 && v.min_block_y == y {
                    self.sample_node(ctx, input, out, v);
                } else {
                    let layer = DensityVolume::with_steps(
                        v.size_x,
                        1,
                        v.size_z,
                        v.min_block_x,
                        y,
                        v.min_block_z,
                        v.step_block_x,
                        v.step_block_y,
                        v.step_block_z,
                    );
                    let mut b = vec![0.0f32; layer.size()];
                    self.sample_node(ctx, input, &mut b, &layer);
                    for zi in 0..v.size_z {
                        for xi in 0..v.size_x {
                            let start = v.index(xi, 0, zi);
                            out[start..start + v.size_y as usize].fill(b[layer.index(xi, 0, zi)]);
                        }
                    }
                }
            }
            Node::Cache { input, id } => self.sample_volume_cached(ctx, id, input, out, v),
            Node::Blend(a) | Node::ContextBound { fallback: a, .. } => {
                self.sample_node(ctx, a, out, v)
            }
            Node::Spline {
                coordinates,
                spline,
            } => {
                let buffers: Vec<Vec<f32>> = coordinates.iter().map(|&c| temp(c)).collect();
                for (i, o) in out.iter_mut().enumerate() {
                    *o = spline.sample(&|c: u16| buffers[c as usize][i]);
                }
            }
        }
    }

    /// `SamplerContext.sampleVolumeCached`: the cell keeps the last volume
    /// it was asked for; asking again for the same volume reuses it.
    fn sample_volume_cached(
        &self,
        ctx: &EvalContext,
        id: u32,
        input: NodeId,
        out: &mut [f32],
        v: &DensityVolume,
    ) {
        if !ctx.caching {
            return self.sample_node(ctx, input, out, v);
        }
        let hit = ctx.cell(id, |c| {
            if c.volume == Some(*v) {
                Some(c.buffer.clone())
            } else {
                None
            }
        });
        let buffer = match hit {
            Some(b) => b,
            None => {
                let mut b = vec![0.0f32; v.size()];
                self.sample_node(ctx, input, &mut b, v);
                ctx.cell(id, |c| {
                    c.volume = Some(*v);
                    c.buffer = b.clone();
                });
                b
            }
        };
        out.copy_from_slice(&buffer);
    }

    /// `SamplerContext.sampleValueCached`: the last point's value, else the
    /// cell's cached volume if it contains the block (a volume-mode value),
    /// else computed in point mode and remembered.
    fn sample_value_cached(
        &self,
        ctx: &EvalContext,
        id: u32,
        input: NodeId,
        x: i32,
        y: i32,
        z: i32,
    ) -> f32 {
        if !ctx.caching {
            return self.sample_value(ctx, input, x, y, z);
        }
        let key = block_pos_as_long(x, y, z);
        let cached = ctx.cell(id, |c| {
            if c.value_key == key && !c.value.is_nan() {
                return Some(c.value);
            }
            let volume = c.volume?;
            volume.index_of_block(x, y, z).map(|i| c.buffer[i])
        });
        if let Some(v) = cached {
            return v;
        }
        let value = self.sample_value(ctx, input, x, y, z);
        ctx.cell(id, |c| {
            c.value_key = key;
            c.value = value;
        });
        value
    }

    /// `FindTopSurfaceFunction.Sampler.findSurfaceFrom`.
    #[allow(clippy::too_many_arguments)]
    fn find_surface_from(
        &self,
        ctx: &EvalContext,
        density: NodeId,
        lower_bound: i32,
        cell_height: i32,
        x: i32,
        z: i32,
        upper: f32,
    ) -> i32 {
        let start = mth::floor((upper / cell_height as f32) as f64) * cell_height;
        if start <= lower_bound {
            return lower_bound;
        }
        let mut y = start;
        while y >= lower_bound {
            if self.sample_value(ctx, density, x, y, z) > 0.0 {
                return y;
            }
            y -= cell_height;
        }
        lower_bound
    }

    /// One node at one block (`DensitySampler.sampleValue`, point mode).
    pub fn sample_value(&self, ctx: &EvalContext, id: NodeId, x: i32, y: i32, z: i32) -> f32 {
        let at = |n: NodeId| self.sample_value(ctx, n, x, y, z);
        match self.tree.nodes[id as usize] {
            Node::Constant(c) => c,
            Node::Noise {
                noise,
                xz_scale,
                y_scale,
            } => self.noises[noise as usize].get(
                x as f64 * xz_scale,
                y as f64 * y_scale,
                z as f64 * xz_scale,
            ),
            Node::ShiftedXz {
                shift_x,
                shift_z,
                noise,
                xz_scale,
                y_scale,
            } => {
                let nx = x as f64 * xz_scale + at(shift_x) as f64;
                let ny = y as f64 * y_scale;
                let nz = z as f64 * xz_scale + at(shift_z) as f64;
                self.noises[noise as usize].get(nx, ny, nz)
            }
            Node::ShiftB { noise } => {
                self.noises[noise as usize].get(z as f64 * 0.25, x as f64 * 0.25, 0.0) * 4.0
            }
            Node::ClampedGradient {
                axis,
                from_coordinate,
                min_coordinate,
                max_coordinate,
                from_value,
                coordinate_factor,
            } => {
                let c = match axis {
                    Axis::X => x,
                    Axis::Y => y,
                    Axis::Z => z,
                };
                from_value
                    + (c.clamp(min_coordinate, max_coordinate) - from_coordinate) as f32
                        * coordinate_factor
            }
            Node::Add(a, b) => at(a) + at(b),
            Node::Sub(a, b) => at(a) - at(b),
            Node::Mul(a, b) => {
                let l = at(a);
                if l == 0.0 { 0.0 } else { l * at(b) }
            }
            Node::Min {
                left,
                right,
                right_min_value,
            } => {
                let l = at(left);
                if l <= right_min_value {
                    l
                } else {
                    min_f32(l, at(right))
                }
            }
            Node::Max {
                left,
                right,
                right_max_value,
            } => {
                let l = at(left);
                if l >= right_max_value {
                    l
                } else {
                    max_f32(l, at(right))
                }
            }
            Node::ConstAdd(a, c) => at(a) + c,
            Node::ConstMul(a, c) => at(a) * c,
            Node::ConstSub(c, a) => c - at(a),
            Node::ConstDiv(c, a) => c / at(a),
            Node::ConstMin(a, c) => min_f32(at(a), c),
            Node::ConstMax(a, c) => max_f32(at(a), c),
            Node::Abs(a) => at(a).abs(),
            Node::Cube(a) => {
                let v = at(a);
                v * v * v
            }
            Node::Square(a) => {
                let v = at(a);
                v * v
            }
            Node::Squeeze(a) => {
                let c = clamp(at(a), -1.0, 1.0);
                c / 2.0 - c * c * c / 24.0
            }
            Node::LeakyRelu {
                input,
                negative_factor,
            } => {
                let v = at(input);
                if v > 0.0 { v } else { v * negative_factor }
            }
            Node::Clamp { input, min, max } => clamp(at(input), min, max),
            Node::Lerp {
                alpha,
                first,
                second,
            } => {
                let a = at(alpha);
                if a == 0.0 {
                    at(first)
                } else if a == 1.0 {
                    at(second)
                } else {
                    mth::lerp(a, at(first), at(second))
                }
            }
            Node::LerpConstFirst {
                alpha,
                first,
                second,
            } => {
                let a = at(alpha);
                if a == 0.0 {
                    first
                } else if a == 1.0 {
                    at(second)
                } else {
                    mth::lerp(a, first, at(second))
                }
            }
            Node::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                let v = at(input);
                if v >= min_inclusive && v < max_exclusive {
                    at(when_in_range)
                } else {
                    at(when_out_of_range)
                }
            }
            Node::IntervalSelect {
                input,
                thresholds,
                samplers,
            } => {
                let v = at(input);
                let k = thresholds
                    .iter()
                    .position(|&t| v < t)
                    .unwrap_or(samplers.len() - 1);
                at(samplers[k])
            }
            Node::Interpolated {
                input,
                cell_size_xz,
                cell_size_y,
                ..
            } => {
                let (mx, my, mz) = (
                    x.rem_euclid(cell_size_xz),
                    y.rem_euclid(cell_size_y),
                    z.rem_euclid(cell_size_xz),
                );
                if mx == 0 && my == 0 && mz == 0 {
                    return at(input);
                }
                // the 8 corners in volume mode, then lerp3
                let corners = DensityVolume::with_steps(
                    2,
                    2,
                    2,
                    x - mx,
                    y - my,
                    z - mz,
                    cell_size_xz,
                    cell_size_y,
                    cell_size_xz,
                );
                let mut c = vec![0.0f32; 8];
                self.sample_node(ctx, input, &mut c, &corners);
                let g = |a: i32, b: i32, d: i32| c[corners.index(a, b, d)];
                mth::lerp3(
                    mx as f32 / cell_size_xz as f32,
                    my as f32 / cell_size_y as f32,
                    mz as f32 / cell_size_xz as f32,
                    g(0, 0, 0),
                    g(1, 0, 0),
                    g(0, 1, 0),
                    g(1, 1, 0),
                    g(0, 0, 1),
                    g(1, 0, 1),
                    g(0, 1, 1),
                    g(1, 1, 1),
                )
            }
            Node::SliceY { input, y: slice_y } => self.sample_value(ctx, input, x, slice_y, z),
            Node::Cache { input, id } => self.sample_value_cached(ctx, id, input, x, y, z),
            Node::Blend(a) | Node::ContextBound { fallback: a, .. } => at(a),
            Node::Spline {
                coordinates,
                spline,
            } => {
                // coordinates are evaluated lazily (and at most once) per point
                let values: Vec<std::cell::Cell<Option<f32>>> = coordinates
                    .iter()
                    .map(|_| std::cell::Cell::new(None))
                    .collect();
                spline.sample(&|c: u16| {
                    let cell = &values[c as usize];
                    cell.get().unwrap_or_else(|| {
                        let v = at(coordinates[c as usize]);
                        cell.set(Some(v));
                        v
                    })
                })
            }
            Node::FindTopSurface {
                density,
                upper_bound,
                lower_bound,
                cell_height,
            } => {
                let upper = at(upper_bound);
                self.find_surface_from(ctx, density, lower_bound, cell_height, x, z, upper) as f32
            }
        }
    }

    fn map(
        &self,
        ctx: &EvalContext,
        input: NodeId,
        out: &mut [f32],
        v: &DensityVolume,
        f: impl Fn(f32) -> f32,
    ) {
        self.sample_node(ctx, input, out, v);
        out.iter_mut().for_each(|o| *o = f(*o));
    }

    /// `InterpolatedFunction.Sampler.sampleVolume`.
    fn interpolate(
        &self,
        ctx: &EvalContext,
        input: NodeId,
        cell: &Cell,
        out: &mut [f32],
        v: &DensityVolume,
    ) {
        let aligned = (v.step_block_x == cell.xz || v.size_x == 1)
            && (v.step_block_y == cell.y || v.size_y == 1)
            && (v.step_block_z == cell.xz || v.size_z == 1)
            && v.min_block_x.rem_euclid(cell.xz) == 0
            && v.min_block_y.rem_euclid(cell.y) == 0
            && v.min_block_z.rem_euclid(cell.xz) == 0;
        if aligned {
            self.sample_node(ctx, input, out, v);
        } else if v.step_block_x == 1 && v.step_block_y == 1 && v.step_block_z == 1 {
            self.interpolate_block_step(ctx, input, cell, out, v);
        } else {
            // sampleWithNonBlockStep: interpolate every block, then pick
            let full = DensityVolume::new(
                v.size_x * v.step_block_x,
                v.size_y * v.step_block_y,
                v.size_z * v.step_block_z,
                v.min_block_x,
                v.min_block_y,
                v.min_block_z,
            );
            let mut b = vec![0.0f32; full.size()];
            self.interpolate_block_step(ctx, input, cell, &mut b, &full);
            for zi in 0..v.size_z {
                for xi in 0..v.size_x {
                    for yi in 0..v.size_y {
                        out[v.index(xi, yi, zi)] = b[full.index(
                            xi * v.step_block_x,
                            yi * v.step_block_y,
                            zi * v.step_block_z,
                        )];
                    }
                }
            }
        }
    }

    /// `InterpolatedFunction.Sampler.sampleWithBlockStep`.
    fn interpolate_block_step(
        &self,
        ctx: &EvalContext,
        input: NodeId,
        cell: &Cell,
        out: &mut [f32],
        v: &DensityVolume,
    ) {
        let (cx0, cy0, cz0) = (
            v.min_block_x.div_euclid(cell.xz),
            v.min_block_y.div_euclid(cell.y),
            v.min_block_z.div_euclid(cell.xz),
        );
        let (cx1, cy1, cz1) = (
            v.max_block_x().div_euclid(cell.xz),
            v.max_block_y().div_euclid(cell.y),
            v.max_block_z().div_euclid(cell.xz),
        );
        let (nx, ny, nz) = (cx1 - cx0 + 1, cy1 - cy0 + 1, cz1 - cz0 + 1);
        let corners = DensityVolume::with_steps(
            if v.max_block_x().rem_euclid(cell.xz) == 0 {
                nx
            } else {
                nx + 1
            },
            if v.max_block_y().rem_euclid(cell.y) == 0 {
                ny
            } else {
                ny + 1
            },
            if v.max_block_z().rem_euclid(cell.xz) == 0 {
                nz
            } else {
                nz + 1
            },
            cx0 * cell.xz,
            cy0 * cell.y,
            cz0 * cell.xz,
            cell.xz,
            cell.y,
            cell.xz,
        );
        let mut c = vec![0.0f32; corners.size()];
        self.sample_node(ctx, input, &mut c, &corners);
        let at = |x: i32, y: i32, z: i32| c[corners.index(x, y, z)];
        for z in 0..nz {
            let z1 = (z + 1).min(corners.size_z - 1);
            for x in 0..nx {
                let x1 = (x + 1).min(corners.size_x - 1);
                let (mut v000, mut v100, mut v001, mut v101) =
                    (at(x, 0, z), at(x1, 0, z), at(x, 0, z1), at(x1, 0, z1));
                for y in 0..ny {
                    let y1 = (y + 1).min(corners.size_y - 1);
                    let (v010, v110, v011, v111) =
                        (at(x, y1, z), at(x1, y1, z), at(x, y1, z1), at(x1, y1, z1));
                    fill_cell(
                        out,
                        v,
                        &corners,
                        cell,
                        (x, y, z),
                        [v000, v100, v010, v110, v001, v101, v011, v111],
                    );
                    (v000, v100, v001, v101) = (v010, v110, v011, v111);
                }
            }
        }
    }
}

struct Cell {
    xz: i32,
    y: i32,
    xz_inv: f32,
    y_inv: f32,
}

/// `InterpolatedFunction.Sampler.fillCell`. Corners are ordered
/// `[v000, v100, v010, v110, v001, v101, v011, v111]` (x, y, z bits).
/// Along Y the game accumulates `value += step` instead of interpolating
/// each block, which rounds differently.
fn fill_cell(
    out: &mut [f32],
    v: &DensityVolume,
    corners: &DensityVolume,
    cell: &Cell,
    (x, y, z): (i32, i32, i32),
    c: [f32; 8],
) {
    let [v000, v100, v010, v110, v001, v101, v011, v111] = c;
    let ox = corners.block_x(x) - v.min_block_x;
    let oy = corners.block_y(y) - v.min_block_y;
    let oz = corners.block_z(z) - v.min_block_z;
    let (sx, sy, sz) = (0.max(-ox), 0.max(-oy), 0.max(-oz));
    let ex = cell.xz.min(v.size_x - ox) - 1;
    let ey = cell.y.min(v.size_y - oy) - 1;
    let ez = cell.xz.min(v.size_z - oz) - 1;
    for dz in sz..=ez {
        let zi = oz + dz;
        let tz = dz as f32 * cell.xz_inv;
        let a = mth::lerp(tz, v000, v001);
        let b = mth::lerp(tz, v010, v011);
        let c = mth::lerp(tz, v100, v101);
        let d = mth::lerp(tz, v110, v111);
        for dx in sx..=ex {
            let xi = ox + dx;
            let tx = dx as f32 * cell.xz_inv;
            let lo = mth::lerp(tx, a, c);
            let hi = mth::lerp(tx, b, d);
            let step = (hi - lo) * cell.y_inv;
            let mut value = lo + step * sy as f32;
            let start = v.index(xi, oy + sy, zi);
            for o in &mut out[start..start + (ey - sy + 1) as usize] {
                *o = value;
                value += step;
            }
        }
    }
}

/// `Mth.clamp(float, float, float)`.
fn clamp(x: f32, min: f32, max: f32) -> f32 {
    if x < min { min } else { min_f32(x, max) }
}

/// `LerpFunction` per element: exact ends at alpha 0 and 1.
fn lerp_alpha(alpha: f32, first: f32, second: f32) -> f32 {
    if alpha == 0.0 {
        first
    } else if alpha == 1.0 {
        second
    } else {
        mth::lerp(alpha, first, second)
    }
}
