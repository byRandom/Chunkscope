//! Slime chunks (Java Edition). From `WorldgenRandom.seedSlimeChunk`:
//!
//! ```java
//! new Random(seed + (long)(x * x * 4987142) + (long)(x * 5947611)
//!                 + (long)(z * z) * 4392871L + (long)(z * 389711) ^ 987234911L)
//!     .nextInt(10) == 0
//! ```
//!
//! Note the mix of `int` and `long` arithmetic: `x*x*4987142`, `x*5947611`,
//! `z*z` and `z*389711` overflow as 32-bit ints, while `* 4392871L` is 64-bit.
//! The final `^` applies to the whole sum (Java operator precedence).

use java_compat::JavaRandom;

pub const SLIME_SALT: i64 = 987234911;

pub fn is_slime_chunk(world_seed: i64, chunk_x: i32, chunk_z: i32) -> bool {
    let (x, z) = (chunk_x, chunk_z);
    let a = x.wrapping_mul(x).wrapping_mul(4987142) as i64;
    let b = x.wrapping_mul(5947611) as i64;
    let c = (z.wrapping_mul(z) as i64).wrapping_mul(4392871);
    let d = z.wrapping_mul(389711) as i64;
    let seed = world_seed
        .wrapping_add(a)
        .wrapping_add(b)
        .wrapping_add(c)
        .wrapping_add(d)
        ^ SLIME_SALT;
    JavaRandom::new(seed).next_int_bounded(10) == 0
}
