//! World spawn origin against the real game: `NoiseSpawnFinder.findSpawnPosition`
//! and `NoiseBasedChunkGenerator.getOrigin` (`parity/fixtures/game/spawn_origin.txt`).

use mc_core::biome::OverworldClimate;
use mc_core::spawn::{spawn_chunk, spawn_origin};

#[test]
fn spawn_origin_matches_the_game() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fixtures/game/spawn_origin.txt"
    ))
    .expect("run parity/game-oracle/run.sh");
    let mut mismatches = Vec::new();
    let mut checked = 0;
    for line in text.lines() {
        let v: Vec<i64> = line.split(' ').map(|t| t.parse().unwrap()).collect();
        let climate = OverworldClimate::new(v[0]);
        let origin = spawn_origin(&climate);
        let chunk = spawn_chunk(&climate);
        checked += 1;
        if (origin.0 as i64, origin.1 as i64) != (v[1], v[2])
            || (chunk.0 as i64, chunk.1 as i64) != (v[3], v[4])
        {
            mismatches.push(format!("{line}: got {origin:?} chunk {chunk:?}"));
        }
    }
    for m in &mismatches {
        println!("{m}");
    }
    assert!(
        mismatches.is_empty(),
        "{} of {checked} spawn origins differ",
        mismatches.len()
    );
}
