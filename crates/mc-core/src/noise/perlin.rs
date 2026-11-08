//! `GradientNoise` and `PerlinNoise` (26.x): single-octave gradient noise,
//! evaluated in `float` precision after an exact `double` lattice lookup.

use crate::mth;
use crate::random::RandomSource;

/// `GradientNoise.GRADIENT`.
const GRADIENT: [(i32, i32, i32); 16] = [
    (1, 1, 0),
    (-1, 1, 0),
    (1, -1, 0),
    (-1, -1, 0),
    (1, 0, 1),
    (-1, 0, 1),
    (1, 0, -1),
    (-1, 0, -1),
    (0, 1, 1),
    (0, -1, 1),
    (0, 1, -1),
    (0, -1, -1),
    (1, 1, 0),
    (0, -1, 1),
    (-1, 1, 0),
    (0, -1, -1),
];

/// `GradientNoise.ROUND_OFF`: inputs are wrapped into a window of this size
/// so that far coordinates keep their fractional precision.
const ROUND_OFF: f64 = 33_554_432.0; // 2^25
/// `Math.nextDown(2^24)`.
const HALF_ROUND_OFF: f64 = f64::from_bits(16_777_216f64.to_bits() - 1);

/// `GradientNoise.wrap(double)`.
#[inline]
pub fn wrap(v: f64) -> f64 {
    if (-HALF_ROUND_OFF..HALF_ROUND_OFF).contains(&v) {
        v
    } else {
        v - (v / ROUND_OFF + 0.5).floor() * ROUND_OFF
    }
}

/// `GradientNoise.gradDot(int, float, float, float)`.
#[inline]
fn grad_dot(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    let (gx, gy, gz) = GRADIENT[(hash & 15) as usize];
    gx as f32 * x + gy as f32 * y + gz as f32 * z
}

/// `PerlinNoise`. Output range is `[-2, 2]`.
#[derive(Clone, Debug)]
pub struct PerlinNoise {
    perms: [u8; 256],
    offset_x: f64,
    offset_y: f64,
    offset_z: f64,
}

impl PerlinNoise {
    /// `new PerlinNoise(RandomSource)`: 3 doubles for the offset, then a
    /// Fisher-Yates shuffle of the permutation table.
    pub fn new(random: &mut impl RandomSource) -> Self {
        Self::with_scale(random, 256.0)
    }

    /// `GradientNoise(RandomSource, double offsetScale)`.
    pub fn with_scale(random: &mut impl RandomSource, scale: f64) -> Self {
        let offset_x = random.next_double() * scale;
        let offset_y = random.next_double() * scale;
        let offset_z = random.next_double() * scale;
        let mut perms = [0u8; 256];
        for (i, p) in perms.iter_mut().enumerate() {
            *p = i as u8;
        }
        for i in 0..256 {
            let j = random.next_int_bounded(256 - i as i32) as usize;
            perms.swap(i, i + j);
        }
        Self {
            perms,
            offset_x,
            offset_y,
            offset_z,
        }
    }

    pub fn offsets(&self) -> (f64, f64, f64) {
        (self.offset_x, self.offset_y, self.offset_z)
    }

    /// `GradientNoise.permute(int)`.
    #[inline]
    fn permute(&self, i: i32) -> i32 {
        self.perms[(i & 255) as usize] as i32
    }

    /// `PerlinNoise.get(double, double, double)`.
    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let x = wrap(x) + self.offset_x;
        let y = wrap(y) + self.offset_y;
        let z = wrap(z) + self.offset_z;
        let (xi, yi, zi) = (mth::floor(x), mth::floor(y), mth::floor(z));
        let xf = (x - xi as f64) as f32;
        let yf = (y - yi as f64) as f32;
        let zf = (z - zi as f64) as f32;
        self.sample_and_lerp(xi, yi, zi, xf, yf, zf, yf)
    }

    /// `PerlinNoise.get(double x, double z)`: a horizontal slice at `y = 0`.
    pub fn get_2d(&self, x: f64, z: f64) -> f32 {
        self.get(wrap(x), 0.0, wrap(z))
    }

    /// `PerlinNoise.sampleAndLerp`.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn sample_and_lerp(
        &self,
        x: i32,
        y: i32,
        z: i32,
        xf: f32,
        yf: f32,
        zf: f32,
        yf_lerp: f32,
    ) -> f32 {
        let a = self.permute(x);
        let b = self.permute(x.wrapping_add(1));
        let aa = self.permute(a.wrapping_add(y));
        let ab = self.permute(a.wrapping_add(y).wrapping_add(1));
        let ba = self.permute(b.wrapping_add(y));
        let bb = self.permute(b.wrapping_add(y).wrapping_add(1));

        let d000 = grad_dot(self.permute(aa.wrapping_add(z)), xf, yf, zf);
        let d100 = grad_dot(self.permute(ba.wrapping_add(z)), xf - 1.0, yf, zf);
        let d010 = grad_dot(self.permute(ab.wrapping_add(z)), xf, yf - 1.0, zf);
        let d110 = grad_dot(self.permute(bb.wrapping_add(z)), xf - 1.0, yf - 1.0, zf);
        let d001 = grad_dot(
            self.permute(aa.wrapping_add(z).wrapping_add(1)),
            xf,
            yf,
            zf - 1.0,
        );
        let d101 = grad_dot(
            self.permute(ba.wrapping_add(z).wrapping_add(1)),
            xf - 1.0,
            yf,
            zf - 1.0,
        );
        let d011 = grad_dot(
            self.permute(ab.wrapping_add(z).wrapping_add(1)),
            xf,
            yf - 1.0,
            zf - 1.0,
        );
        let d111 = grad_dot(
            self.permute(bb.wrapping_add(z).wrapping_add(1)),
            xf - 1.0,
            yf - 1.0,
            zf - 1.0,
        );

        let sx = mth::smoothstep(xf);
        let sy = mth::smoothstep(yf_lerp);
        let sz = mth::smoothstep(zf);
        mth::lerp3(sx, sy, sz, d000, d100, d010, d110, d001, d101, d011, d111)
    }
}
