//! Compares against fixtures produced by the real JDK (parity/oracle/Oracle.java).

use mc_core::{is_slime_chunk, parse_seed};

const SEEDS: &str = include_str!("../../../parity/fixtures/seeds.txt");
const SLIME: &str = include_str!("../../../parity/fixtures/slime.txt");

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn seed_text_parsing_matches_jdk() {
    let mut failures = Vec::new();
    for line in SEEDS.lines() {
        let (hex, want) = line.split_once(' ').unwrap();
        let input = String::from_utf8(unhex(&hex[1..])).unwrap();
        let got = match parse_seed(&input) {
            None => "random".to_string(),
            Some(v) => v.to_string(),
        };
        if got != want {
            failures.push((input, got, want.to_string()));
        }
    }
    assert!(
        failures.is_empty(),
        "{} mismatches (input, rust, java), first: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

#[test]
fn slime_chunks_match_jdk() {
    let mut checked = 0;
    for line in SLIME.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        match f[0] {
            "grid" => {
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
                        checked += 1;
                    }
                }
            }
            "at" => {
                let seed: i64 = f[1].parse().unwrap();
                let (x, z): (i32, i32) = (f[2].parse().unwrap(), f[3].parse().unwrap());
                assert_eq!(
                    is_slime_chunk(seed, x, z),
                    f[4] == "1",
                    "seed {seed} chunk ({x}, {z})"
                );
                checked += 1;
            }
            other => panic!("unknown record {other}"),
        }
    }
    assert!(checked > 100_000);
}
