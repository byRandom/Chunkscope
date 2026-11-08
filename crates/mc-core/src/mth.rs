//! Ports of `net.minecraft.util.Mth` helpers. Operation order is kept exactly
//! as in the game: float arithmetic is not associative, so `a + b + c` and
//! `a + (b + c)` can differ in the last bit.

/// `Mth.floor(double)`: `(int) Math.floor(v)` (saturating; NaN becomes 0,
/// like Java's `d2i`).
#[inline]
pub fn floor(v: f64) -> i32 {
    v.floor() as i32
}

/// `Mth.smoothstep(float)`: `x * x * x * (x * (x * 6 - 15) + 10)`.
#[inline]
pub fn smoothstep(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// `Mth.lerp(float alpha, float a, float b)`: `a + alpha * (b - a)`.
#[inline]
pub fn lerp(alpha: f32, a: f32, b: f32) -> f32 {
    a + alpha * (b - a)
}

/// `Mth.lerp2(float, float, ...)`.
#[inline]
pub fn lerp2(alpha1: f32, alpha2: f32, x00: f32, x10: f32, x01: f32, x11: f32) -> f32 {
    lerp(alpha2, lerp(alpha1, x00, x10), lerp(alpha1, x01, x11))
}

/// `Mth.lerp3(float, float, float, ...)`.
#[allow(clippy::too_many_arguments)]
#[inline]
pub fn lerp3(
    alpha1: f32,
    alpha2: f32,
    alpha3: f32,
    x000: f32,
    x100: f32,
    x010: f32,
    x110: f32,
    x001: f32,
    x101: f32,
    x011: f32,
    x111: f32,
) -> f32 {
    lerp(
        alpha3,
        lerp2(alpha1, alpha2, x000, x100, x010, x110),
        lerp2(alpha1, alpha2, x001, x101, x011, x111),
    )
}
