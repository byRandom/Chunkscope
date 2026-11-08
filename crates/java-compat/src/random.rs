//! Bit-exact port of `java.util.Random` (JDK 17+/21 behaviour).
//!
//! Every method mirrors the JDK source, including Java's 32-bit integer
//! overflow semantics, so the same seed yields the same sequence.

const MULTIPLIER: i64 = 0x5DEECE66D;
const ADDEND: i64 = 0xB;
const MASK: i64 = (1 << 48) - 1;
const FLOAT_UNIT: f32 = 1.0 / (1u32 << 24) as f32;
const DOUBLE_UNIT: f64 = 1.0 / (1u64 << 53) as f64;

#[derive(Clone, Debug)]
pub struct JavaRandom {
    seed: i64,
    next_next_gaussian: Option<f64>,
}

impl JavaRandom {
    /// Equivalent to `new Random(seed)`.
    pub fn new(seed: i64) -> Self {
        Self {
            seed: scramble(seed),
            next_next_gaussian: None,
        }
    }

    /// Equivalent to `Random.setSeed(seed)` (also clears the cached gaussian).
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = scramble(seed);
        self.next_next_gaussian = None;
    }

    /// The internal 48-bit state (useful for reversal / seed cracking tools).
    pub fn raw_state(&self) -> i64 {
        self.seed
    }

    /// `protected int next(int bits)`.
    #[inline]
    pub fn next(&mut self, bits: u32) -> i32 {
        debug_assert!((1..=32).contains(&bits));
        self.seed = self.seed.wrapping_mul(MULTIPLIER).wrapping_add(ADDEND) & MASK;
        // state is 48-bit non-negative, so `>>>` == `>>`; `(int)` truncates.
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self) -> i32 {
        self.next(32)
    }

    /// `nextInt(int bound)`. Panics if `bound <= 0`, like Java's IllegalArgumentException.
    pub fn next_int_bounded(&mut self, bound: i32) -> i32 {
        assert!(bound > 0, "bound must be positive");
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            // power of two
            ((bound as i64 * r as i64) >> 31) as i32
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                // Java: `u - r + m < 0` with int overflow
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    return r;
                }
                u = self.next(31);
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        let hi = (self.next(32) as i64) << 32;
        hi.wrapping_add(self.next(32) as i64)
    }

    pub fn next_boolean(&mut self) -> bool {
        self.next(1) != 0
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 * FLOAT_UNIT
    }

    pub fn next_double(&mut self) -> f64 {
        let hi = (self.next(26) as i64) << 27;
        (hi + self.next(27) as i64) as f64 * DOUBLE_UNIT
    }

    /// `nextGaussian()`. Java uses `StrictMath.log` (fdlibm); see `strict_math`.
    pub fn next_gaussian(&mut self) -> f64 {
        if let Some(g) = self.next_next_gaussian.take() {
            return g;
        }
        loop {
            let v1 = 2.0 * self.next_double() - 1.0;
            let v2 = 2.0 * self.next_double() - 1.0;
            let s = v1 * v1 + v2 * v2;
            if s < 1.0 && s != 0.0 {
                let multiplier = (-2.0 * crate::strict_math::log(s) / s).sqrt();
                self.next_next_gaussian = Some(v2 * multiplier);
                return v1 * multiplier;
            }
        }
    }
}

#[inline]
fn scramble(seed: i64) -> i64 {
    (seed ^ MULTIPLIER) & MASK
}
