//! `RandomSupport` and `Mth.getSeed`.

use md5::{Digest, Md5};

pub const GOLDEN_RATIO_64: i64 = 0x9E3779B97F4A7C15_u64 as i64;
pub const SILVER_RATIO_64: i64 = 0x6A09E667F3BCC909;

/// `RandomSupport.Seed128bit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Seed128 {
    pub lo: i64,
    pub hi: i64,
}

impl Seed128 {
    pub const fn new(lo: i64, hi: i64) -> Self {
        Self { lo, hi }
    }

    pub const fn xor(self, lo: i64, hi: i64) -> Self {
        Self::new(self.lo ^ lo, self.hi ^ hi)
    }

    pub const fn mixed(self) -> Self {
        Self::new(mix_stafford13(self.lo), mix_stafford13(self.hi))
    }
}

/// `RandomSupport.mixStafford13` (SplitMix64 finalizer, "Mix13" variant).
pub const fn mix_stafford13(mut z: i64) -> i64 {
    z = (z ^ ((z as u64) >> 30) as i64).wrapping_mul(0xBF58476D1CE4E5B9_u64 as i64);
    z = (z ^ ((z as u64) >> 27) as i64).wrapping_mul(0x94D049BB133111EB_u64 as i64);
    z ^ ((z as u64) >> 31) as i64
}

/// `RandomSupport.upgradeSeedTo128bitUnmixed`.
pub const fn upgrade_seed_to_128bit_unmixed(seed: i64) -> Seed128 {
    let lo = seed ^ SILVER_RATIO_64;
    Seed128::new(lo, lo.wrapping_add(GOLDEN_RATIO_64))
}

/// `RandomSupport.upgradeSeedTo128bit`.
pub const fn upgrade_seed_to_128bit(seed: i64) -> Seed128 {
    upgrade_seed_to_128bit_unmixed(seed).mixed()
}

/// `RandomSupport.seedFromHashOf`: MD5 of the UTF-8 string, as two
/// big-endian longs.
pub fn seed_from_hash_of(name: &str) -> Seed128 {
    let digest = Md5::digest(name.as_bytes());
    let lo = i64::from_be_bytes(digest[..8].try_into().unwrap());
    let hi = i64::from_be_bytes(digest[8..].try_into().unwrap());
    Seed128::new(lo, hi)
}

/// `Mth.getSeed(int x, int y, int z)`: per-position seed used by positional
/// random factories.
pub const fn get_seed(x: i32, y: i32, z: i32) -> i64 {
    let mut l = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    l = l
        .wrapping_mul(l)
        .wrapping_mul(42317861)
        .wrapping_add(l.wrapping_mul(11));
    l >> 16
}
