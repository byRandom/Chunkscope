//! Volume-mode evaluation of a compiled [`Tree`] (`DensitySampler.sampleVolume`).

use java_compat::math::min_f32;

use super::tree::{Axis, Node, NodeId, NoiseSource, Tree};
use super::volume::DensityVolume;
use crate::mth;
use crate::noise::blended::FbmSet;
use crate::noise::{NoiseStack, NormalNoise, vanilla};

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
    pub fn sample_volume(&self, volume: &DensityVolume) -> Vec<f32> {
        let mut out = vec![0.0; volume.size()];
        self.sample_node(self.tree.root, &mut out, volume);
        out
    }

    /// One node over `volume` into `out` (`out.len() == volume.size()`).
    pub fn sample_node(&self, id: NodeId, out: &mut [f32], v: &DensityVolume) {
        let temp = |child: NodeId| {
            let mut b = vec![0.0f32; v.size()];
            self.sample_node(child, &mut b, v);
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
                self.sample_node(shift_x, out, v);
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
                self.sample_node(a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, r)| *o += r);
            }
            Node::Mul(a, b) => {
                // no zero short-circuit in volume mode
                self.sample_node(a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, r)| *o *= r);
            }
            Node::Min(a, b) => {
                self.sample_node(a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, &r)| {
                    if r < *o {
                        *o = r
                    }
                });
            }
            Node::Max(a, b) => {
                self.sample_node(a, out, v);
                let r = temp(b);
                out.iter_mut().zip(&r).for_each(|(o, &r)| {
                    if r > *o {
                        *o = r
                    }
                });
            }
            Node::ConstAdd(a, c) => {
                self.sample_node(a, out, v);
                out.iter_mut().for_each(|o| *o += c);
            }
            Node::ConstMul(a, c) => {
                self.sample_node(a, out, v);
                out.iter_mut().for_each(|o| *o *= c);
            }
            Node::Abs(a) => self.map(a, out, v, f32::abs),
            Node::Cube(a) => self.map(a, out, v, |x| x * x * x),
            Node::Square(a) => self.map(a, out, v, |x| x * x),
            Node::Squeeze(a) => self.map(a, out, v, |x| {
                let c = clamp(x, -1.0, 1.0);
                c / 2.0 - c * c * c / 24.0
            }),
            Node::LeakyRelu {
                input,
                negative_factor,
            } => self.map(
                input,
                out,
                v,
                |x| if x > 0.0 { x } else { x * negative_factor },
            ),
            Node::Clamp { input, min, max } => self.map(input, out, v, |x| clamp(x, min, max)),
            Node::Lerp {
                alpha,
                first,
                second,
            } => {
                self.sample_node(alpha, out, v);
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
                self.sample_node(alpha, out, v);
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
                self.sample_node(when_in_range, out, v);
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
                self.sample_node(input, out, v);
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
                self.interpolate(input, &cell, out, v);
            }
            Node::SliceY { input, y } => {
                if v.size_y == 1 && v.min_block_y == y {
                    self.sample_node(input, out, v);
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
                    self.sample_node(input, &mut b, &layer);
                    for zi in 0..v.size_z {
                        for xi in 0..v.size_x {
                            let start = v.index(xi, 0, zi);
                            out[start..start + v.size_y as usize].fill(b[layer.index(xi, 0, zi)]);
                        }
                    }
                }
            }
            Node::Cache(a) | Node::Blend(a) | Node::ContextBound { fallback: a, .. } => {
                self.sample_node(a, out, v)
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

    fn map(&self, input: NodeId, out: &mut [f32], v: &DensityVolume, f: impl Fn(f32) -> f32) {
        self.sample_node(input, out, v);
        out.iter_mut().for_each(|o| *o = f(*o));
    }

    /// `InterpolatedFunction.Sampler.sampleVolume`.
    fn interpolate(&self, input: NodeId, cell: &Cell, out: &mut [f32], v: &DensityVolume) {
        let aligned = (v.step_block_x == cell.xz || v.size_x == 1)
            && (v.step_block_y == cell.y || v.size_y == 1)
            && (v.step_block_z == cell.xz || v.size_z == 1)
            && v.min_block_x.rem_euclid(cell.xz) == 0
            && v.min_block_y.rem_euclid(cell.y) == 0
            && v.min_block_z.rem_euclid(cell.xz) == 0;
        if aligned {
            self.sample_node(input, out, v);
        } else if v.step_block_x == 1 && v.step_block_y == 1 && v.step_block_z == 1 {
            self.interpolate_block_step(input, cell, out, v);
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
            self.interpolate_block_step(input, cell, &mut b, &full);
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
        self.sample_node(input, &mut c, &corners);
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
