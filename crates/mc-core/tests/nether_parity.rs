//! Nether biomes against the real game (`parity/fixtures/game/nether_biomes.txt`):
//! the legacy climate noises, the quantized climate and the biome lookup.

use mc_core::biome::NetherBiomeSource;

#[test]
fn nether_biomes_match_the_game() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fixtures/game/nether_biomes.txt"
    ))
    .expect("run parity/game-oracle/run.sh");
    let mut source: Option<(i64, NetherBiomeSource)> = None;
    let (mut checked, mut mismatches) = (0, Vec::new());
    for line in text.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        let seed: i64 = parts[0].parse().unwrap();
        let q: Vec<i32> = parts[1..4].iter().map(|v| v.parse().unwrap()).collect();
        let climate: Vec<i64> = parts[4..10].iter().map(|v| v.parse().unwrap()).collect();
        let biome = parts[10];
        // A fresh biome search per seed, as the oracle does.
        if source.as_ref().is_none_or(|s| s.0 != seed) {
            source = Some((seed, NetherBiomeSource::new(seed)));
        }
        let (_, src) = source.as_ref().unwrap();
        let t = src.climate().sample(q[0], q[1], q[2]);
        let got_climate = vec![
            t.temperature,
            t.humidity,
            t.continentalness,
            t.erosion,
            t.depth,
            t.weirdness,
        ];
        let got_biome = src.biome(q[0], q[1], q[2]);
        checked += 1;
        if got_climate != climate || got_biome.name() != biome {
            mismatches.push(format!(
                "{line}\n    got {got_climate:?} {}",
                got_biome.name()
            ));
        }
    }
    for m in mismatches.iter().take(10) {
        println!("{m}");
    }
    assert!(
        mismatches.is_empty(),
        "{} of {checked} Nether biome lookups differ",
        mismatches.len()
    );
}
