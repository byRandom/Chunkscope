//! Compares against fixtures produced by the real JDK (parity/oracle/Oracle.java).

use java_compat::{BitRandom, JavaRandom, strict_math};

const RANDOM: &str = include_str!("../../../parity/fixtures/random.txt");
const STRICT_LOG: &str = include_str!("../../../parity/fixtures/strict_log.txt");
// Must match `BOUNDS` in Oracle.java (token names catch any drift).
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

fn tok(out: &mut String, name: &str, v: impl std::fmt::Display) {
    out.push_str(&format!(" {name}={v}"));
}

#[test]
fn java_util_random_matches_jdk() {
    for (n, line) in RANDOM.lines().enumerate() {
        let seed: i64 = line.split(' ').next().unwrap().parse().unwrap();
        let mut r = JavaRandom::new(seed);
        let mut s = seed.to_string();
        for _ in 0..4 {
            tok(&mut s, "nextInt", r.next_int());
        }
        for b in BOUNDS {
            tok(&mut s, &format!("nextInt({b})"), r.next_int_bounded(b));
        }
        for _ in 0..3 {
            tok(&mut s, "nextLong", r.next_long());
        }
        for _ in 0..3 {
            tok(
                &mut s,
                "nextFloat",
                format!("{:x}", r.next_float().to_bits()),
            );
        }
        for _ in 0..3 {
            tok(
                &mut s,
                "nextDouble",
                format!("{:x}", r.next_double().to_bits()),
            );
        }
        for _ in 0..4 {
            tok(&mut s, "nextBoolean", r.next_boolean());
        }
        for _ in 0..7 {
            tok(
                &mut s,
                "nextGaussian",
                format!("{:x}", r.next_gaussian().to_bits()),
            );
        }
        r.set_seed(seed.wrapping_add(1));
        tok(
            &mut s,
            "setSeed+nextGaussian",
            format!("{:x}", r.next_gaussian().to_bits()),
        );

        if s != line {
            let first_diff = s.split(' ').zip(line.split(' ')).find(|(a, b)| a != b);
            panic!(
                "line {}: seed {seed}: first mismatch (rust, java) = {first_diff:?}",
                n + 1
            );
        }
    }
}

#[test]
fn strict_math_log_matches_jdk() {
    let mut failures = Vec::new();
    for line in STRICT_LOG.lines() {
        let (x, y) = line.split_once(' ').unwrap();
        let x = f64::from_bits(u64::from_str_radix(x, 16).unwrap());
        let want = f64::from_bits(u64::from_str_radix(y, 16).unwrap());
        let got = strict_math::log(x);
        let ok = if want.is_nan() {
            got.is_nan()
        } else {
            got.to_bits() == want.to_bits()
        };
        if !ok {
            failures.push((x, got, want));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

const STREAM_SUM: &str = include_str!("../../../parity/fixtures/stream_sum.txt");

#[test]
fn double_stream_sum_matches_jdk() {
    for line in STREAM_SUM.lines() {
        let mut bits = line
            .split(' ')
            .map(|t| f64::from_bits(u64::from_str_radix(t, 16).unwrap()));
        let want = bits.next().unwrap();
        let values: Vec<f64> = bits.collect();
        let got = java_compat::stream::double_sum(values.iter().copied());
        let ok = if want.is_nan() {
            got.is_nan()
        } else {
            got.to_bits() == want.to_bits()
        };
        assert!(ok, "sum of {values:?}: rust {got:?}, java {want:?}");
    }
}

const MATH_ROUND: &str = include_str!("../../../parity/fixtures/math_round.txt");

#[test]
fn math_round_matches_jdk() {
    for line in MATH_ROUND.lines() {
        let (x, want) = line.split_once(' ').unwrap();
        let x = f64::from_bits(u64::from_str_radix(x, 16).unwrap());
        assert_eq!(
            java_compat::math::round(x).to_string(),
            want,
            "Math.round({x:?})"
        );
    }
}
