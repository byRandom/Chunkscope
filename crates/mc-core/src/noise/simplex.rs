//! `SimplexNoise` (26.x), the 2D part the End islands use. Unlike
//! [`super::PerlinNoise`] it works in `double` and only converts the sum to
//! `float`.

use crate::mth;
use crate::random::RandomSource;

/// `GradientNoise.GRADIENT`, the first 12 entries (simplex takes `% 12`).
const GRADIENT: [(i32, i32, i32); 12] = [
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
];

#[derive(Clone, Debug)]
pub struct SimplexNoise {
    perms: [u8; 256],
    offset_x: f64,
    offset_y: f64,
}

impl SimplexNoise {
    /// `new SimplexNoise(RandomSource, boolean zeroOffset)`: three doubles
    /// for the offsets (scaled by 0 or 256), then the permutation shuffle,
    /// as in `GradientNoise`.
    pub fn new(random: &mut impl RandomSource, zero_offset: bool) -> Self {
        let scale = if zero_offset { 0.0 } else { 256.0 };
        let offset_x = random.next_double() * scale;
        let offset_y = random.next_double() * scale;
        let _offset_z = random.next_double() * scale;
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
        }
    }

    #[inline]
    fn permute(&self, i: i32) -> i32 {
        self.perms[(i & 255) as usize] as i32
    }

    /// `getCornerNoise3D(index, x, y, z, base)`.
    #[inline]
    fn corner(index: i32, x: f64, y: f64, z: f64, base: f64) -> f64 {
        let mut t = base - x * x - y * y - z * z;
        if t < 0.0 {
            0.0
        } else {
            t *= t;
            let (gx, gy, gz) = GRADIENT[index as usize];
            t * t * (gx as f64 * x + gy as f64 * y + gz as f64 * z)
        }
    }

    /// `SimplexNoise.get(double, double)`.
    pub fn get_2d(&self, x: f64, y: f64) -> f32 {
        let sqrt3 = 3.0f64.sqrt();
        let f2 = 0.5 * (sqrt3 - 1.0);
        let g2 = (3.0 - sqrt3) / 6.0;
        let x = x + self.offset_x;
        let y = y + self.offset_y;
        let s = (x + y) * f2;
        let i = mth::floor(x + s);
        let j = mth::floor(y + s);
        let t = (i.wrapping_add(j)) as f64 * g2;
        let x0 = x - (i as f64 - t);
        let y0 = y - (j as f64 - t);
        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };
        let x1 = x0 - i1 as f64 + g2;
        let y1 = y0 - j1 as f64 + g2;
        let x2 = x0 - 1.0 + 2.0 * g2;
        let y2 = y0 - 1.0 + 2.0 * g2;
        let ii = i & 255;
        let jj = j & 255;
        let gi0 = self.permute(ii + self.permute(jj)) % 12;
        let gi1 = self.permute(ii + i1 + self.permute(jj + j1)) % 12;
        let gi2 = self.permute(ii + 1 + self.permute(jj + 1)) % 12;
        let n0 = Self::corner(gi0, x0, y0, 0.0, 0.5);
        let n1 = Self::corner(gi1, x1, y1, 0.0, 0.5);
        let n2 = Self::corner(gi2, x2, y2, 0.0, 0.5);
        (70.0 * (n0 + n1 + n2)) as f32
    }
}
