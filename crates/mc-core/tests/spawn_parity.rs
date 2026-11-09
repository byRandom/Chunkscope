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

/// The estimated spawn block against the spawn real servers picked
/// (`parity/fixtures/server/spawn.txt`, from `parity/server-oracle/run.sh`).
/// The estimate has no trees, lakes or ice, so it is not always exact: this
/// records how close it is and fails if it gets worse.
#[test]
fn spawn_estimate_against_real_servers() {
    use mc_core::spawn::spawn_estimate;
    use mc_core::terrain::OverworldTerrain;
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fixtures/server/spawn.txt"
    ))
    .expect("run parity/server-oracle/run.sh");
    let (mut total, mut same_xz, mut exact) = (0, 0, 0);
    for line in text.lines() {
        let v: Vec<i64> = line.split(' ').map(|t| t.parse().unwrap()).collect();
        let seed = v[0];
        let (x, y, z) = spawn_estimate(&OverworldClimate::new(seed), &OverworldTerrain::new(seed));
        total += 1;
        if (x as i64, z as i64) == (v[1], v[3]) {
            same_xz += 1;
            if y as i64 == v[2] {
                exact += 1;
            } else {
                println!("seed {seed}: real y {}, estimate {y} (trees)", v[2]);
            }
        } else {
            println!(
                "seed {seed}: real {} {} {}, estimate {x} {y} {z}",
                v[1], v[2], v[3]
            );
        }
    }
    println!("{same_xz} of {total} spawns at the same x, z; {exact} exact");
    assert_eq!(total, 32);
    assert!(
        same_xz >= 31,
        "the spawn estimate got worse: {same_xz} of {total} x, z"
    );
    assert!(
        exact >= 23,
        "the spawn estimate got worse: {exact} of {total} exact"
    );
}
