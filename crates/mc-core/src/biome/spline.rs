//! `CubicSpline` (26.x): piecewise cubic Hermite interpolation in `float`,
//! where each point's value can itself be a spline over another coordinate.

/// What a spline is evaluated over. The overworld terrain splines use these
/// three density functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SplineCoordinate {
    /// `minecraft:overworld/continents`
    Continents,
    /// `minecraft:overworld/erosion`
    Erosion,
    /// `minecraft:overworld/ridges_folded`
    RidgesFolded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Spline {
    Constant(f32),
    Multipoint(&'static Multipoint),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Multipoint {
    pub coordinate: SplineCoordinate,
    pub locations: &'static [f32],
    pub derivatives: &'static [f32],
    pub values: &'static [Spline],
}

impl Spline {
    /// `CubicSpline.sample`. `coordinate` returns the value of each input
    /// function at the point being sampled.
    pub fn sample(&self, coordinate: &impl Fn(SplineCoordinate) -> f32) -> f32 {
        match self {
            Spline::Constant(v) => *v,
            Spline::Multipoint(m) => m.sample(coordinate),
        }
    }
}

impl Multipoint {
    fn sample(&self, coordinate: &impl Fn(SplineCoordinate) -> f32) -> f32 {
        let x = coordinate(self.coordinate);
        let locations = self.locations;
        let start = find_interval_start(locations, x);
        let last = locations.len() as i32 - 1;
        if start < 0 {
            let v = self.values[0].sample(coordinate);
            return linear_extend(x, locations, v, self.derivatives, 0);
        }
        if start == last {
            let i = last as usize;
            let v = self.values[i].sample(coordinate);
            return linear_extend(x, locations, v, self.derivatives, i);
        }
        let i = start as usize;
        let (l0, l1) = (locations[i], locations[i + 1]);
        let t = (x - l0) / (l1 - l0);
        let v0 = self.values[i].sample(coordinate);
        let v1 = self.values[i + 1].sample(coordinate);
        let (d0, d1) = (self.derivatives[i], self.derivatives[i + 1]);
        let p = d0 * (l1 - l0) - (v1 - v0);
        let q = -d1 * (l1 - l0) + (v1 - v0);
        crate::mth::lerp(t, v0, v1) + t * (1.0 - t) * crate::mth::lerp(t, p, q)
    }
}

/// `CubicSpline.Multipoint.linearExtend`.
fn linear_extend(x: f32, locations: &[f32], value: f32, derivatives: &[f32], i: usize) -> f32 {
    let d = derivatives[i];
    if d == 0.0 {
        value
    } else {
        value + d * (x - locations[i])
    }
}

/// `findIntervalStart`: index of the last location `<= x` (NaN-aware like
/// Java's `fcmpg`), or -1.
fn find_interval_start(locations: &[f32], x: f32) -> i32 {
    // Mth.binarySearch(0, len, i -> x < locations[i]) - 1
    let (mut from, mut len) = (0i32, locations.len() as i32);
    while len > 0 {
        let half = len / 2;
        let mid = from + half;
        if x < locations[mid as usize] {
            len = half;
        } else {
            from = mid + 1;
            len -= half + 1;
        }
    }
    from - 1
}
