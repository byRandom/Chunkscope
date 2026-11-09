//! Structure validation against the real game: for every candidate chunk in
//! `parity/fixtures/game/structures.txt`, the structures the game tried and
//! where the one that generates starts (`Structure.findValidGenerationPoint`).

use std::collections::BTreeMap;

use mc_core::biome::OverworldBiomeSource;
use mc_core::structure::validate::{Outcome, Structure, StructureGenerator};
use mc_core::structure::{STRUCTURE_SETS, StructureSets};
use mc_core::terrain::OverworldTerrain;

fn fixture() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fixtures/game/structures.txt"
    ))
    .expect("run parity/game-oracle/run.sh")
}

/// The game's trace format: `name:x,y,z` for the structure that generates,
/// `name:no` for one that does not.
fn trace(outcome: &Outcome) -> Vec<String> {
    outcome
        .attempts()
        .iter()
        .map(|a| {
            let name = a.structure.name.trim_start_matches("minecraft:");
            match a.point {
                Some((x, y, z)) => format!("{name}:{x},{y},{z}"),
                None => format!("{name}:no"),
            }
        })
        .collect()
}

#[test]
fn structure_attempts_match_the_game() {
    let text = fixture();
    let mut current: Option<(i64, OverworldBiomeSource, OverworldTerrain)> = None;
    // set -> (checked, unknown, mismatches)
    let mut stats: BTreeMap<String, (u32, u32, Vec<String>)> = BTreeMap::new();
    for line in text.lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let set_name = parts.next().unwrap();
        let cx: i32 = parts.next().unwrap().parse().unwrap();
        let cz: i32 = parts.next().unwrap().parse().unwrap();
        let expected: Vec<&str> = parts.collect();
        if current.as_ref().is_none_or(|c| c.0 != seed) {
            current = Some((
                seed,
                OverworldBiomeSource::new(seed),
                OverworldTerrain::new(seed),
            ));
        }
        let (_, biomes, terrain) = current.as_ref().unwrap();
        let generator = StructureGenerator::new(seed, biomes, terrain);
        let set = STRUCTURE_SETS.find_set(set_name).expect("known set");
        assert_eq!(
            set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz),
            Some(true),
            "{line}: the fixture only lists chunks where the set attempts a structure"
        );
        let outcome = generator.attempt(set, cx, cz);
        let entry = stats.entry(set_name.to_owned()).or_default();
        let got = trace(&outcome);
        if let Outcome::Unknown(_) = outcome {
            // Not ported yet; what was tried before must still match.
            entry.1 += 1;
            if expected.len() < got.len() || expected[..got.len()] != got[..] {
                entry
                    .2
                    .push(format!("{line}\n    got (partial) {}", got.join(" ")));
            }
            continue;
        }
        entry.0 += 1;
        if got != expected {
            entry.2.push(format!("{line}\n    got {}", got.join(" ")));
        }
    }
    let mut failed = false;
    for (set, (checked, unknown, mismatches)) in &stats {
        println!(
            "{set}: {checked} checked, {unknown} not ported, {} mismatches",
            mismatches.len()
        );
        for m in mismatches.iter().take(5) {
            println!("  {m}");
        }
        failed |= !mismatches.is_empty();
    }
    assert!(!failed, "structure attempts differ from the game");
}

/// Each structure's generation point on clusters of chunks centered on its
/// biomes (`parity/fixtures/game/structure_points.txt`), whether a set would
/// attempt it there or not: many valid points and edge cases per type.
#[test]
fn generation_points_match_the_game() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/fixtures/game/structure_points.txt"
    ))
    .expect("run parity/game-oracle/run.sh");
    let mut failed = false;
    let mut sources: Option<(i64, OverworldBiomeSource, OverworldTerrain)> = None;
    for line in text.lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let name = format!("minecraft:{}", parts.next().unwrap());
        if sources.as_ref().is_none_or(|s| s.0 != seed) {
            sources = Some((
                seed,
                OverworldBiomeSource::new(seed),
                OverworldTerrain::new(seed),
            ));
        }
        let (_, biomes, terrain) = sources.as_ref().unwrap();
        let generator = StructureGenerator::new(seed, biomes, terrain);
        let structure = Structure::find(&name).expect("known structure");
        let (mut checked, mut mismatches) = (0, Vec::new());
        for entry in parts {
            let (chunk, expected) = entry.split_once(':').unwrap();
            let (cx, cz) = chunk.split_once(',').unwrap();
            let (cx, cz): (i32, i32) = (cx.parse().unwrap(), cz.parse().unwrap());
            let Some(point) = generator.try_structure(structure, cx, cz) else {
                break; // not ported yet
            };
            checked += 1;
            let got = match point {
                Some((x, y, z)) => format!("{x},{y},{z}"),
                None => "no".to_owned(),
            };
            if got != expected {
                mismatches.push(format!("{cx},{cz}: game {expected}, got {got}"));
            }
        }
        println!("{name}: {checked} checked, {} mismatches", mismatches.len());
        for m in mismatches.iter().take(5) {
            println!("  {m}");
        }
        failed |= !mismatches.is_empty();
    }
    assert!(!failed, "generation points differ from the game");
}
