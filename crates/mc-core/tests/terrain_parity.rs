//! Terrain parity against the real game's `NoiseBasedChunkGenerator`
//! (parity/game-oracle/GameOracle.java): base heightmaps and full block
//! columns (stone, air, water, lava) including aquifers.

use mc_core::terrain::{Heightmap, OverworldTerrain, Substance};

const TERRAIN: &str = include_str!("../../../parity/fixtures/game/terrain.txt");

fn letter(s: Substance) -> char {
    match s {
        Substance::Solid => 's',
        Substance::Air => 'a',
        Substance::Water => 'w',
        Substance::Lava => 'l',
    }
}

#[test]
fn base_heights_and_columns_match_game() {
    let mut terrain: Option<(i64, OverworldTerrain)> = None;
    let mut failures = Vec::new();
    let mut columns = 0;
    for line in TERRAIN.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let (x, z): (i32, i32) = (t[1].parse().unwrap(), t[2].parse().unwrap());
        if terrain.as_ref().is_none_or(|(s, _)| *s != seed) {
            terrain = Some((seed, OverworldTerrain::new(seed)));
        }
        let tr = &terrain.as_ref().unwrap().1;
        let world_surface = tr.base_height(x, z, Heightmap::WorldSurface);
        let ocean_floor = tr.base_height(x, z, Heightmap::OceanFloor);
        let column: String = tr.base_column(x, z).into_iter().map(letter).collect();
        let got = format!("{world_surface} {ocean_floor} {column}");
        let want = t[3..].join(" ");
        if got != want {
            let first = got.chars().zip(want.chars()).position(|(a, b)| a != b);
            failures.push(format!(
                "seed {seed} column ({x}, {z}): heights rust {world_surface}/{ocean_floor} game {}/{}, first difference at char {first:?}",
                t[3], t[4]
            ));
        }
        columns += 1;
    }
    assert!(
        failures.is_empty(),
        "{} of {columns} columns differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

const AQUIFER: &str = include_str!("../../../parity/fixtures/game/aquifer.txt");

/// `computeSubstance` with densities near 0, where barriers and pressure
/// decide, at every Y from 120 down to -60.
#[test]
fn aquifer_substances_match_game() {
    let densities = [-0.6, -0.2, -0.08, -0.03, -0.01, -0.002];
    let mut terrain: Option<(i64, OverworldTerrain)> = None;
    let mut failures = Vec::new();
    for line in AQUIFER.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let (x, z): (i32, i32) = (t[1].parse().unwrap(), t[2].parse().unwrap());
        if terrain.as_ref().is_none_or(|(s, _)| *s != seed) {
            terrain = Some((seed, OverworldTerrain::new(seed)));
        }
        let got: String = terrain
            .as_ref()
            .unwrap()
            .1
            .with_column_aquifer(x, z, |aquifer| {
                let mut out = String::new();
                for y in (-60..=120).rev() {
                    for d in densities {
                        out.push(letter(aquifer.compute_substance(x, y, z, d)));
                    }
                }
                out
            });
        if got != t[3] {
            let first = got
                .chars()
                .zip(t[3].chars())
                .position(|(a, b)| a != b)
                .unwrap();
            let (yi, di) = (first / densities.len(), first % densities.len());
            failures.push(format!(
                "seed {seed} column ({x}, {z}): y {} density {}: rust {} game {}",
                120 - yi as i32,
                densities[di],
                got.as_bytes()[first] as char,
                t[3].as_bytes()[first] as char
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} columns differ, first ones:\n{}",
        failures.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
