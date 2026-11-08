//! `java.lang.Math` functions whose result differs from Rust's std.

/// `Math.round(double)`: rounds half up (towards positive infinity), so
/// `-2.5` becomes `-2` where `f64::round` gives `-3`. NaN becomes 0 and
/// out-of-range values saturate. Ported from the JDK's bit-level version,
/// which, unlike `floor(a + 0.5)`, has no rounding error for values such as
/// `0.49999999999999994`.
pub fn round(a: f64) -> i64 {
    const EXP_BIT_MASK: i64 = 0x7FF0_0000_0000_0000;
    const SIGNIF_BIT_MASK: i64 = 0x000F_FFFF_FFFF_FFFF;
    let bits = a.to_bits() as i64;
    let biased_exp = (bits & EXP_BIT_MASK) >> 52;
    let shift = (53 - 2 + 1023) - biased_exp;
    if shift & -64 == 0 {
        let mut r = (bits & SIGNIF_BIT_MASK) | (SIGNIF_BIT_MASK + 1);
        if bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        // Java's (long) cast: saturating, NaN -> 0 (same as Rust's `as`).
        a as i64
    }
}
