//! Ruined portals against the real game, in both dimensions: the Nether's
//! `ruined_portals` attempts (`parity/fixtures/game/ruined_portals.txt`; the
//! overworld's are in `structures.txt`) and every variant's generation point
//! on clusters of chunks, without the biome check
//! (`parity/fixtures/game/ruined_portal_points.txt`).

use mc_core::structure::validate::Structure;
use mc_core::structure::{STRUCTURE_SETS, StructureSets};
use mc_core::world::{Dimension, World};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/game/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/game-oracle/run.sh")
}

fn point(p: Option<(i32, i32, i32)>) -> String {
    match p {
        Some((x, y, z)) => format!("{x},{y},{z}"),
        None => "no".to_owned(),
    }
}

#[test]
fn nether_ruined_portal_attempts_match_the_game() {
    let text = fixture("ruined_portals.txt");
    let mut world: Option<World> = None;
    let (mut checked, mut generates, mut mismatches) = (0, 0, Vec::new());
    for line in text.lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let set = STRUCTURE_SETS.find_set(parts.next().unwrap()).unwrap();
        let cx: i32 = parts.next().unwrap().parse().unwrap();
        let cz: i32 = parts.next().unwrap().parse().unwrap();
        let expected: Vec<&str> = parts.collect();
        if world.as_ref().is_none_or(|w| w.seed() != seed) {
            world = Some(World::new(seed, Dimension::Nether));
        }
        let outcome = world.as_ref().unwrap().structures().attempt(set, cx, cz);
        let got: Vec<String> = outcome
            .attempts()
            .iter()
            .map(|a| {
                let name = a.structure.name.trim_start_matches("minecraft:");
                format!("{name}:{}", point(a.point))
            })
            .collect();
        checked += 1;
        generates += usize::from(outcome.generated().is_some());
        if got != expected {
            mismatches.push(format!("{line}\n    got {}", got.join(" ")));
        }
    }
    println!(
        "{checked} attempts, {generates} generate, {} mismatches",
        mismatches.len()
    );
    for m in mismatches.iter().take(5) {
        println!("  {m}");
    }
    assert!(checked > 500 && generates > 100);
    assert!(
        mismatches.is_empty(),
        "Nether ruined portals differ from the game"
    );
}

#[test]
fn ruined_portal_points_match_the_game() {
    let text = fixture("ruined_portal_points.txt");
    let mut failed = false;
    for line in text.lines() {
        let mut parts = line.split(' ');
        let dimension = parts.next().unwrap();
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let name = format!("minecraft:{}", parts.next().unwrap());
        let structure = Structure::find(&name).expect("known structure");
        let world = World::new(seed, Dimension::from_id(dimension).expect("a dimension"));
        let generator = world.structures();
        let (mut total, mut bad) = (0, Vec::new());
        for entry in parts {
            let (chunk, expected) = entry.split_once(':').unwrap();
            let (cx, cz) = chunk.split_once(',').unwrap();
            let (cx, cz): (i32, i32) = (cx.parse().unwrap(), cz.parse().unwrap());
            let got = generator.generation_point_any_biome(structure, cx, cz);
            total += 1;
            if point(got) != expected {
                bad.push(format!(
                    "{cx},{cz}: expected {expected}, got {}",
                    point(got)
                ));
            }
        }
        println!(
            "{dimension} {name}: {total} points, {} mismatches",
            bad.len()
        );
        for b in bad.iter().take(5) {
            println!("  {b}");
        }
        failed |= !bad.is_empty();
    }
    assert!(!failed, "ruined portal points differ from the game");
}
