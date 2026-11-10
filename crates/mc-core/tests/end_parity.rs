//! The End against the real game (`parity/fixtures/game/end_*.txt`).

use mc_core::noise::SimplexNoise;
use mc_core::random::{LegacyRandomSource, RandomSource};
use mc_core::terrain::EndIslands;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/game/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/game-oracle/run.sh")
}

/// `SimplexNoise.get(x, y)` and `EndIslandFunction.getHeightValue`, by raw bits.
#[test]
fn end_islands_match_the_game() {
    let (mut simplex, mut heights, mut islands) = (0, 0, 0);
    for line in fixture("end_islands.txt").lines() {
        let mut parts = line.split(' ');
        match parts.next().unwrap() {
            "simplex" => {
                let seed: i64 = parts.next().unwrap().parse().unwrap();
                let zero: bool = parts.next().unwrap().parse().unwrap();
                let mut random = LegacyRandomSource::new(seed);
                random.consume_count(17292);
                let noise = SimplexNoise::new(&mut random, zero);
                for entry in parts {
                    let (xy, v) = entry.split_once('=').unwrap();
                    let (x, y) = xy.split_once(',').unwrap();
                    let x = f64::from_bits(x.parse::<i64>().unwrap() as u64);
                    let y = f64::from_bits(y.parse::<i64>().unwrap() as u64);
                    let expected = v.parse::<i32>().unwrap() as u32;
                    let got = noise.get_2d(x, y);
                    assert_eq!(
                        got.to_bits(),
                        expected,
                        "seed {seed} zero {zero} at {x}, {y}: {got} vs {}",
                        f32::from_bits(expected)
                    );
                    simplex += 1;
                }
            }
            "height" => {
                let seed: i64 = parts.next().unwrap().parse().unwrap();
                let end = EndIslands::new(seed);
                for entry in parts {
                    let (xz, v) = entry.split_once('=').unwrap();
                    let (x, z) = xz.split_once(',').unwrap();
                    let (x, z): (i32, i32) = (x.parse().unwrap(), z.parse().unwrap());
                    let expected = v.parse::<i32>().unwrap() as u32;
                    let got = end.height_value(x, z);
                    assert_eq!(
                        got.to_bits(),
                        expected,
                        "seed {seed} at {x}, {z}: {got} vs {}",
                        f32::from_bits(expected)
                    );
                    heights += 1;
                    islands += usize::from(got > -100.0);
                }
            }
            other => panic!("unknown line kind {other}"),
        }
    }
    println!("{simplex} simplex values, {heights} heights ({islands} on islands)");
    assert!(simplex > 1000 && heights > 3000 && islands > 300);
}

/// `TheEndBiomeSource` at quart positions near the main island's edge and far out.
#[test]
fn end_biomes_match_the_game() {
    use mc_core::biome::EndBiomeSource;
    let mut checked = 0;
    let mut mismatches = Vec::new();
    for line in fixture("end_biomes.txt").lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let source = EndBiomeSource::new(seed);
        for entry in parts {
            let (pos, expected) = entry.split_once('=').unwrap();
            let q: Vec<i32> = pos.split(',').map(|v| v.parse().unwrap()).collect();
            let got = source.biome(q[0], q[1], q[2]);
            let got = got.name().trim_start_matches("minecraft:");
            checked += 1;
            if got != expected {
                mismatches.push(format!(
                    "seed {seed} at {pos}: expected {expected}, got {got}"
                ));
            }
        }
    }
    println!("{checked} End biomes, {} mismatches", mismatches.len());
    for m in mismatches.iter().take(10) {
        println!("  {m}");
    }
    assert!(mismatches.is_empty());
}
