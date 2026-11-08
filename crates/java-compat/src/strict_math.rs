//! Ports of `java.lang.StrictMath` functions (fdlibm), bit-exact.
//!
//! Rust's `f64::ln` delegates to the platform libm, which is not guaranteed to
//! match fdlibm in the last ulp. Java's `StrictMath` *is* fdlibm, so anything
//! that must match Java bit-for-bit has to go through here.

const LN2_HI: f64 = f64::from_bits(0x3FE62E42FEE00000);
const LN2_LO: f64 = f64::from_bits(0x3DEA39EF35793C76);
const TWO54: f64 = f64::from_bits(0x4350000000000000);
const LG1: f64 = f64::from_bits(0x3FE5555555555593);
const LG2: f64 = f64::from_bits(0x3FD999999997FA04);
const LG3: f64 = f64::from_bits(0x3FD2492494229359);
const LG4: f64 = f64::from_bits(0x3FCC71C51D8E78AF);
const LG5: f64 = f64::from_bits(0x3FC7466496CB03DE);
const LG6: f64 = f64::from_bits(0x3FC39A09D078C69F);
const LG7: f64 = f64::from_bits(0x3FC2F112DF3E5244);
/// fdlibm writes `0.33333333333333333`, which rounds to this value.
const ONE_THIRD: f64 = f64::from_bits(0x3FD5555555555555);

#[inline]
fn hi(x: f64) -> i32 {
    (x.to_bits() >> 32) as i32
}
#[inline]
fn lo(x: f64) -> u32 {
    x.to_bits() as u32
}
#[inline]
fn with_hi(x: f64, hi: i32) -> f64 {
    f64::from_bits(((hi as u32 as u64) << 32) | lo(x) as u64)
}

/// `StrictMath.log` — fdlibm `__ieee754_log`.
pub fn log(mut x: f64) -> f64 {
    let mut hx = hi(x);
    let lx = lo(x);
    let mut k: i32 = 0;

    if hx < 0x0010_0000 {
        if ((hx & 0x7fff_ffff) as u32 | lx) == 0 {
            return f64::NEG_INFINITY; // log(+-0)
        }
        if hx < 0 {
            return f64::NAN; // log(negative)
        }
        k -= 54;
        x *= TWO54; // subnormal, scale up
        hx = hi(x);
    }
    if hx >= 0x7ff0_0000 {
        return x + x;
    }
    k += (hx >> 20) - 1023;
    hx &= 0x000f_ffff;
    let i = (hx + 0x95f64) & 0x10_0000;
    x = with_hi(x, hx | (i ^ 0x3ff0_0000)); // normalize x or x/2
    k += i >> 20;
    let f = x - 1.0;

    if (0x000f_ffff & (2 + hx)) < 3 {
        // |f| < 2**-20
        if f == 0.0 {
            if k == 0 {
                return 0.0;
            }
            let dk = k as f64;
            return dk * LN2_HI + dk * LN2_LO;
        }
        let r = f * f * (0.5 - ONE_THIRD * f);
        if k == 0 {
            return f - r;
        }
        let dk = k as f64;
        return dk * LN2_HI - ((r - dk * LN2_LO) - f);
    }

    let s = f / (2.0 + f);
    let dk = k as f64;
    let z = s * s;
    let mut i = hx - 0x6147a;
    let w = z * z;
    let j = 0x6b851 - hx;
    let t1 = w * (LG2 + w * (LG4 + w * LG6));
    let t2 = z * (LG1 + w * (LG3 + w * (LG5 + w * LG7)));
    i |= j;
    let r = t2 + t1;
    if i > 0 {
        let hfsq = 0.5 * f * f;
        if k == 0 {
            f - (hfsq - s * (hfsq + r))
        } else {
            dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - f)
        }
    } else if k == 0 {
        f - s * (f - r)
    } else {
        dk * LN2_HI - ((s * (f - r) - dk * LN2_LO) - f)
    }
}
