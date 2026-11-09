//! A compiled density-function tree, as the game's `DensityFunctionCompiler`
//! produces it: one [`Node`] per sampler, referring to children by index.
//!
//! Vanilla trees are exported from the game by the oracle (see
//! `generated.rs`), so the compiler's rewrites (constant folding, slices,
//! caches...) are reproduced exactly instead of being re-derived from JSON.

use crate::biome::spline::Spline;
use crate::noise::blended::BlendedNoiseParams;

/// Index of a node in its tree.
pub type NodeId = u32;

/// `Direction.Axis` of a gradient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

/// Where a node's noise comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoiseSource {
    /// A vanilla `worldgen/noise` entry, by registry name.
    Vanilla(&'static str),
    /// The min-limit stack of the tree's `old_blended_noise`.
    BlendedMinLimit,
    /// The max-limit stack of the tree's `old_blended_noise`.
    BlendedMaxLimit,
    /// The main stack of the tree's `old_blended_noise`.
    BlendedMain,
}

/// One compiled sampler. Names follow the game's sampler classes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Node {
    /// `ConstantFunction.Sampler`
    Constant(f32),
    /// `NoiseFunction.Sampler`: `noise(x * xz_scale, y * y_scale, z * xz_scale)`.
    Noise {
        noise: u16,
        xz_scale: f64,
        y_scale: f64,
    },
    /// `NoiseFunction.ShiftedXzSampler`
    ShiftedXz {
        shift_x: NodeId,
        shift_z: NodeId,
        noise: u16,
        xz_scale: f64,
        y_scale: f64,
    },
    /// `ShiftNoiseFunction.ShiftB` (compiled): `noise(z / 4, x / 4, 0) * 4`.
    ShiftB { noise: u16 },
    /// `GradientFunction.ClampedSampler`
    ClampedGradient {
        axis: Axis,
        from_coordinate: i32,
        min_coordinate: i32,
        max_coordinate: i32,
        from_value: f32,
        coordinate_factor: f32,
    },
    /// `BinaryFunction.AddSampler`
    Add(NodeId, NodeId),
    /// `BinaryFunction.MulSampler`
    Mul(NodeId, NodeId),
    /// `BinaryFunction.MinSampler`. In point mode, `right` is skipped when
    /// `left <= right_min_value` (the smallest value `right` can take).
    Min {
        left: NodeId,
        right: NodeId,
        right_min_value: f32,
    },
    /// `BinaryFunction.MaxSampler`. In point mode, `right` is skipped when
    /// `left >= right_max_value`.
    Max {
        left: NodeId,
        right: NodeId,
        right_max_value: f32,
    },
    /// `BinaryFunction.SubSampler`: `left - right`.
    Sub(NodeId, NodeId),
    /// `BinaryFunction.ConstAddSampler`: `input + value`.
    ConstAdd(NodeId, f32),
    /// `BinaryFunction.ConstMulSampler`: `input * value`.
    ConstMul(NodeId, f32),
    /// `BinaryFunction.ConstSubSampler`: `value - input`.
    ConstSub(f32, NodeId),
    /// `BinaryFunction.ConstDivSampler`: `value / input`.
    ConstDiv(f32, NodeId),
    /// `BinaryFunction.ConstMinSampler`: `min(input, value)`.
    ConstMin(NodeId, f32),
    /// `BinaryFunction.ConstMaxSampler`: `max(input, value)`.
    ConstMax(NodeId, f32),
    /// `FindTopSurfaceFunction.Sampler`: from `upper_bound` down in steps of
    /// `cell_height`, the first Y where `density` (always point mode) is
    /// positive, else `lower_bound`. Volume mode needs a single-layer volume.
    FindTopSurface {
        density: NodeId,
        upper_bound: NodeId,
        lower_bound: i32,
        cell_height: i32,
    },
    /// `UnaryFunction.AbsSampler`
    Abs(NodeId),
    /// `UnaryFunction.CubeSampler`
    Cube(NodeId),
    /// `UnaryFunction.SquareSampler`
    Square(NodeId),
    /// `UnaryFunction.SqueezeSampler`
    Squeeze(NodeId),
    /// `UnaryFunction.LeakyReLUSampler`
    LeakyRelu { input: NodeId, negative_factor: f32 },
    /// `ClampFunction.Sampler`
    Clamp { input: NodeId, min: f32, max: f32 },
    /// `LerpFunction.Sampler`
    Lerp {
        alpha: NodeId,
        first: NodeId,
        second: NodeId,
    },
    /// `LerpFunction.ConstFirstSampler`
    LerpConstFirst {
        alpha: NodeId,
        first: f32,
        second: NodeId,
    },
    /// `RangeChoiceFunction.Sampler`
    RangeChoice {
        input: NodeId,
        min_inclusive: f32,
        max_exclusive: f32,
        when_in_range: NodeId,
        when_out_of_range: NodeId,
    },
    /// `IntervalSelectFunction.Sampler`
    IntervalSelect {
        input: NodeId,
        thresholds: &'static [f32],
        samplers: &'static [NodeId],
    },
    /// `InterpolatedFunction.Sampler`: trilinear interpolation between cell
    /// corners (volume mode only).
    Interpolated {
        input: NodeId,
        cell_size_xz: i32,
        cell_size_y: i32,
        cell_size_xz_inv: f32,
        cell_size_y_inv: f32,
    },
    /// `SliceFunction.YSampler`: the input at a fixed Y.
    SliceY { input: NodeId, y: i32 },
    /// `CachingDensitySampler`: values go through the context's cache cell
    /// `id`, so a point query can return a value cached in volume mode.
    Cache { input: NodeId, id: u32 },
    /// `BlendDensityFunction.Sampler`: identity without old-chunk blending.
    Blend(NodeId),
    /// `ContextBoundSampler`: a context-provided sampler, or `fallback`.
    ContextBound { key: &'static str, fallback: NodeId },
    /// `SplineFunction.Sampler`: spline coordinates are indices into
    /// `coordinates`.
    Spline {
        coordinates: &'static [NodeId],
        spline: &'static Spline<u16>,
    },
}

/// A whole compiled tree.
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub nodes: &'static [Node],
    pub root: NodeId,
    /// Noise sources, indexed by the `noise` fields of the nodes.
    pub noises: &'static [NoiseSource],
    /// Parameters of the `old_blended_noise` the blended sources come from.
    pub blended: Option<BlendedNoiseParams>,
    /// The dimension uses the legacy random source (`legacy_random_source`):
    /// noises come from a legacy positional factory and the blended noise is
    /// seeded with `LegacyRandomSource(seed)`.
    pub legacy_random: bool,
}
