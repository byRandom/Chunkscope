//! Compares against fixtures produced by the real game classes
//! (parity/game-oracle/GameOracle.java).

use mc_core::is_slime_chunk;
use mc_core::random::{
    LegacyRandomSource, PositionalRandomFactory, RandomSource, WorldgenBits, WorldgenRandom,
    XoroshiroRandomSource, get_seed, mix_stafford13, seed_from_hash_of, upgrade_seed_to_128bit,
};

const SUPPORT: &str = include_str!("../../../parity/fixtures/game/support.txt");
const XOROSHIRO: &str = include_str!("../../../parity/fixtures/game/xoroshiro.txt");
const LEGACY: &str = include_str!("../../../parity/fixtures/game/legacy.txt");
const WORLDGEN_LEGACY: &str = include_str!("../../../parity/fixtures/game/worldgen_legacy.txt");
const WORLDGEN_XOROSHIRO: &str =
    include_str!("../../../parity/fixtures/game/worldgen_xoroshiro.txt");
const SLIME: &str = include_str!("../../../parity/fixtures/game/slime.txt");

// Must match `BOUNDS` and `NAMES` in GameOracle.java (token names catch drift).
const BOUNDS: [i32; 13] = [
    1,
    2,
    3,
    5,
    7,
    10,
    16,
    100,
    1000,
    1 << 30,
    (1 << 30) + 1,
    1500000001,
    i32::MAX,
];
const NAMES: [&str; 13] = [
    "",
    "minecraft:offset",
    "minecraft:temperature",
    "minecraft:vegetation",
    "minecraft:continentalness",
    "minecraft:erosion",
    "minecraft:ridge",
    "minecraft:aquifer",
    "minecraft:ore",
    "octave_-3",
    "\u{e9}",
    "\u{65e5}\u{672c}",
    "\u{1f3ae}",
];

fn tok(out: &mut String, name: &str, v: impl std::fmt::Display) {
    out.push_str(&format!(" {name}={v}"));
}

fn hex(s: &str) -> String {
    let mut h = String::from("x");
    for b in s.bytes() {
        h.push_str(&format!("{b:02x}"));
    }
    h
}

fn unhex(s: &str) -> String {
    let b: Vec<u8> = (1..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect();
    String::from_utf8(b).unwrap()
}

/// Mirrors `GameOracle.script`.
fn script(s: &mut String, r: &mut impl RandomSource) {
    for _ in 0..4 {
        tok(s, "nextInt", r.next_int());
    }
    for b in BOUNDS {
        tok(s, &format!("nextInt({b})"), r.next_int_bounded(b));
    }
    for _ in 0..3 {
        tok(s, "nextLong", r.next_long());
    }
    for _ in 0..3 {
        tok(s, "nextFloat", format!("{:x}", r.next_float().to_bits()));
    }
    for _ in 0..3 {
        tok(s, "nextDouble", format!("{:x}", r.next_double().to_bits()));
    }
    for _ in 0..4 {
        tok(s, "nextBoolean", r.next_boolean());
    }
    tok(s, "between(-5,5)", r.next_int_between_inclusive(-5, 5));
    tok(
        s,
        "between(min+2,0)",
        r.next_int_between_inclusive(i32::MIN + 2, 0),
    );
    tok(s, "range(-100,100)", r.next_int_range(-100, 100));
    tok(
        s,
        "triangle",
        format!("{:x}", r.triangle(0.5, 2.0).to_bits()),
    );
    tok(
        s,
        "triangleF",
        format!("{:x}", r.triangle_f32(0.5, 2.0).to_bits()),
    );
    r.consume_count(5);
    tok(s, "afterConsume", r.next_long());
}

fn assert_same(rust: &str, java: &str, what: &str) {
    if rust != java {
        let first = rust.split(' ').zip(java.split(' ')).find(|(a, b)| a != b);
        panic!("{what}: first mismatch (rust, game) = {first:?}");
    }
}

/// Mirrors `GameOracle.sources`.
fn check_sources<R: RandomSource>(fixture: &str, new: impl Fn(i64) -> R) {
    for line in fixture.lines() {
        let seed: i64 = line.split(' ').next().unwrap().parse().unwrap();
        let mut s = seed.to_string();
        let mut r = new(seed);
        script(&mut s, &mut r);

        let mut f = r.fork();
        let (a, b) = (f.next_long(), f.next_long());
        tok(&mut s, "fork", format!("{a}/{b}"));
        tok(&mut s, "parentAfterFork", r.next_long());

        let p = r.fork_positional();
        tok(&mut s, "at(0,0,0)", p.at(0, 0, 0).next_long());
        tok(&mut s, "at(17,-64,-300)", p.at(17, -64, -300).next_long());
        tok(
            &mut s,
            "at(big)",
            p.at(30_000_000, 319, -30_000_000).next_long(),
        );
        for n in NAMES {
            tok(
                &mut s,
                &format!("hash({})", hex(n)),
                p.from_hash_of(n).next_long(),
            );
        }
        tok(&mut s, "fromSeed(123)", p.from_seed(123).next_long());
        tok(
            &mut s,
            &format!("fromSeed({seed})"),
            p.from_seed(seed).next_long(),
        );

        r.set_seed(seed ^ 0x1234_5678);
        tok(&mut s, "setSeed", r.next_long());
        assert_same(&s, line, &format!("seed {seed}"));
    }
}

/// Mirrors `GameOracle.worldgen`.
fn check_worldgen<R: WorldgenBits>(fixture: &str, new: impl Fn(i64) -> R) {
    for line in fixture.lines() {
        let f: Vec<i64> = line
            .split(' ')
            .take(8)
            .map(|t| t.parse().unwrap())
            .collect();
        let seed = f[0];
        let [x, z, index, step, rx, rz, salt] =
            [f[1], f[2], f[3], f[4], f[5], f[6], f[7]].map(|v| v as i32);
        let mut s = f
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let mut r = WorldgenRandom::new(new(0));
        let deco = r.set_decoration_seed(seed, x, z);
        tok(&mut s, "decorationSeed", deco);
        r.set_feature_seed(deco, index, step);
        script(&mut s, &mut r);
        tok(&mut s, "count", r.count());

        r.set_large_feature_seed(seed, x >> 4, z >> 4);
        let (a, b) = (r.next_int(), r.next_int_bounded(16));
        tok(&mut s, "largeFeature", format!("{a}/{b}"));

        r.set_large_feature_with_salt(seed, rx, rz, salt);
        let (a, b, c) = (
            r.next_int_bounded(24),
            r.next_int_bounded(24),
            r.next_float(),
        );
        tok(&mut s, "withSalt", format!("{a}/{b}/{:x}", c.to_bits()));

        tok(&mut s, "fork", r.fork().next_long());
        tok(
            &mut s,
            "forkPositional",
            r.fork_positional().at(x, 0, z).next_long(),
        );
        tok(&mut s, "finalCount", r.count());
        assert_same(&s, line, &format!("seed {seed}"));
    }
}

#[test]
fn random_support_matches_game() {
    for line in SUPPORT.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        match f[0] {
            "mix" => {
                let seed: i64 = f[1].parse().unwrap();
                let up = upgrade_seed_to_128bit(seed);
                let got = format!("{} {} {}", mix_stafford13(seed), up.lo, up.hi);
                assert_eq!(got, f[2..].join(" "), "seed {seed}");
            }
            "md5" => {
                let name = unhex(f[1]);
                let s = seed_from_hash_of(&name);
                assert_eq!(format!("{} {}", s.lo, s.hi), f[2..].join(" "), "{name:?}");
            }
            "getseed" => {
                let v: Vec<i64> = f[1..].iter().map(|t| t.parse().unwrap()).collect();
                let (x, y, z) = (v[0] as i32, v[1] as i32, v[2] as i32);
                assert_eq!(get_seed(x, y, z), v[3], "Mth.getSeed({x}, {y}, {z})");
            }
            other => panic!("unknown record {other}"),
        }
    }
}

#[test]
fn xoroshiro_random_source_matches_game() {
    check_sources(XOROSHIRO, XoroshiroRandomSource::new);
}

#[test]
fn legacy_random_source_matches_game() {
    check_sources(LEGACY, LegacyRandomSource::new);
}

#[test]
fn worldgen_random_over_legacy_matches_game() {
    check_worldgen(WORLDGEN_LEGACY, LegacyRandomSource::new);
}

#[test]
fn worldgen_random_over_xoroshiro_matches_game() {
    check_worldgen(WORLDGEN_XOROSHIRO, XoroshiroRandomSource::new);
}

#[test]
fn slime_chunks_match_game() {
    for line in SLIME.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let seed: i64 = f[1].parse().unwrap();
        let (lo, hi): (i32, i32) = (f[2].parse().unwrap(), f[3].parse().unwrap());
        let mut bits = f[4].bytes();
        for x in lo..hi {
            for z in lo..hi {
                let want = bits.next().unwrap() == b'1';
                assert_eq!(
                    is_slime_chunk(seed, x, z),
                    want,
                    "seed {seed} chunk ({x}, {z})"
                );
            }
        }
    }
}
