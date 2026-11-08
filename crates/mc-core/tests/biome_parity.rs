//! Climate and biome parity against the real game
//! (parity/game-oracle/GameOracle.java).

use mc_core::biome::{OverworldBiomeSource, OverworldClimate};

const CLIMATE: &str = include_str!("../../../parity/fixtures/game/climate.txt");
const BIOMES: &str = include_str!("../../../parity/fixtures/game/biomes.txt");

fn bits(f: f32) -> String {
    format!("{:x}", f.to_bits())
}

#[test]
fn overworld_climate_matches_game() {
    let mut current: Option<(i64, OverworldClimate)> = None;
    for line in CLIMATE.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let (x, y, z): (i32, i32, i32) = (
            t[1].parse().unwrap(),
            t[2].parse().unwrap(),
            t[3].parse().unwrap(),
        );
        if current.as_ref().is_none_or(|(s, _)| *s != seed) {
            current = Some((seed, OverworldClimate::new(seed)));
        }
        let climate = &current.as_ref().unwrap().1;
        let c = climate.sample_block(x, y, z);
        let names = [
            "temperature",
            "humidity",
            "continentalness",
            "erosion",
            "depth",
            "weirdness",
        ];
        let got = [
            c.temperature,
            c.humidity,
            c.continentalness,
            c.erosion,
            c.depth,
            c.weirdness,
        ];
        for ((name, value), want) in names.iter().zip(got).zip(&t[4..]) {
            assert_eq!(
                bits(value),
                *want,
                "{name} for seed {seed} at block ({x}, {y}, {z})"
            );
        }
    }
}

#[test]
fn overworld_biomes_match_game() {
    let mut checked = 0;
    for line in BIOMES.lines() {
        let mut t = line.split(' ');
        let seed: i64 = t.next().unwrap().parse().unwrap();
        // fresh source = fresh R-tree, queried in the same order as the game
        let source = OverworldBiomeSource::new(seed);
        for query in t {
            let (pos, want) = query.split_once('=').unwrap();
            let q: Vec<i32> = pos.split(',').map(|v| v.parse().unwrap()).collect();
            let got = source.biome(q[0], q[1], q[2]);
            assert_eq!(
                got.name().strip_prefix("minecraft:").unwrap(),
                want,
                "seed {seed} at quart ({}, {}, {}), climate {:?}",
                q[0],
                q[1],
                q[2],
                source.climate().sample(q[0], q[1], q[2])
            );
            checked += 1;
        }
    }
    assert!(checked > 20_000);
}
