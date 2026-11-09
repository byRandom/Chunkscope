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

/// Nether base columns and WORLD_SURFACE_WG heights against the game's own
/// `getBaseColumn` / `getBaseHeight` (`parity/fixtures/game/nether_terrain.txt`).
#[test]
fn nether_terrain_matches_the_game() {
    use mc_core::terrain::{Heightmap, NetherTerrain, Substance};
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fixtures/game/nether_terrain.txt"
    ))
    .expect("run parity/game-oracle/run.sh");
    let mut terrain: Option<(i64, NetherTerrain)> = None;
    let (mut checked, mut mismatches) = (0, Vec::new());
    for line in text.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        let seed: i64 = parts[0].parse().unwrap();
        let (x, z): (i32, i32) = (parts[1].parse().unwrap(), parts[2].parse().unwrap());
        let surface: i32 = parts[3].parse().unwrap();
        if terrain.as_ref().is_none_or(|t| t.0 != seed) {
            terrain = Some((seed, NetherTerrain::new(seed)));
        }
        let (_, t) = terrain.as_ref().unwrap();
        let blocks: String = t
            .base_column(x, z)
            .into_iter()
            .map(|s| match s {
                Substance::Solid => 's',
                Substance::Air => 'a',
                Substance::Water => 'w',
                Substance::Lava => 'l',
            })
            .collect();
        let got_surface = t.base_height(x, z, Heightmap::WorldSurface);
        checked += 1;
        if blocks != parts[4] || got_surface != surface {
            mismatches.push(format!(
                "seed {seed} at {x}, {z}: game {surface} {}\n   got {got_surface} {blocks}",
                parts[4]
            ));
        }
    }
    for m in mismatches.iter().take(5) {
        println!("{m}");
    }
    assert!(
        mismatches.is_empty(),
        "{} of {checked} Nether columns differ",
        mismatches.len()
    );
}
