//! Large-scale check against the real game: compares Chunkscope with what
//! `parity/game-oracle/WideOracle.java` wrote for many random seeds (biomes,
//! terrain heights, every structure attempt in an area, end city ships, the
//! spawn, strongholds and the order of the End gateways).
//!
//! Its output is not committed: run `parity/game-oracle/wide.sh`, which sets
//! `CHUNKSCOPE_WIDE` to it. Without that variable these tests do nothing.
//! Every difference is printed with its seed and position, and all of them are
//! written to `report-<check>.txt` next to the oracle's output.

use std::collections::BTreeMap;
use std::path::PathBuf;

use mc_core::biome::OverworldClimate;
use mc_core::spawn::{spawn_chunk, spawn_origin};
use mc_core::structure::end_city::end_city_has_ship;
use mc_core::structure::end_gateway::gateway_order;
use mc_core::structure::validate::Outcome;
use mc_core::structure::{STRUCTURE_SETS, StructureSets};
use mc_core::terrain::Heightmap;
use mc_core::world::{Dimension, World};

/// The oracle's output directory, if a wide run was asked for.
fn wide_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("CHUNKSCOPE_WIDE")?;
    Some(PathBuf::from(dir))
}

/// The lines of one of the oracle's files, or `None` when not running.
fn lines(name: &str) -> Option<Vec<String>> {
    let Some(dir) = wide_dir() else {
        eprintln!("CHUNKSCOPE_WIDE is not set: skipping (run parity/game-oracle/wide.sh)");
        return None;
    };
    let text = std::fs::read_to_string(dir.join(name))
        .unwrap_or_else(|e| panic!("{}: {e}", dir.join(name).display()));
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    assert!(
        !lines.is_empty(),
        "{name} is empty: the oracle wrote nothing"
    );
    Some(lines)
}

/// Differences found by one check, grouped (by dimension, set...).
#[derive(Default)]
struct Report {
    /// group -> (checked, differences)
    groups: BTreeMap<String, (u64, Vec<String>)>,
}

impl Report {
    fn check(&mut self, group: impl Into<String>, ok: bool, difference: impl FnOnce() -> String) {
        let entry = self.groups.entry(group.into()).or_default();
        entry.0 += 1;
        if !ok {
            entry.1.push(difference());
        }
    }

    /// Print a summary (the first differences of each group), write every
    /// difference to `report-<name>.txt`, and fail if there is any.
    fn finish(self, name: &str) {
        let mut all = String::new();
        let mut failed = 0;
        println!("== {name}");
        for (group, (checked, diffs)) in &self.groups {
            println!("{group}: {checked} checked, {} differ", diffs.len());
            for d in diffs.iter().take(10) {
                println!("  {d}");
            }
            for d in diffs {
                all.push_str(&format!("{group}: {d}\n"));
            }
            failed += diffs.len();
        }
        if let Some(dir) = wide_dir() {
            std::fs::write(dir.join(format!("report-{name}.txt")), &all).expect("write the report");
        }
        assert!(failed == 0, "{name}: {failed} differences from the game");
    }
}

fn dimension(id: &str) -> Dimension {
    Dimension::from_id(id).unwrap_or_else(|| panic!("unknown dimension {id}"))
}

/// One world per line group: the oracle writes each (dimension, seed) in a
/// row and, like the game, builds its biome search fresh for each.
struct Worlds(Option<World>);

impl Worlds {
    fn get(&mut self, seed: i64, dim: Dimension) -> &World {
        if self
            .0
            .as_ref()
            .is_none_or(|w| w.seed() != seed || w.dimension() != dim)
        {
            self.0 = Some(World::new(seed, dim));
        }
        self.0.as_ref().unwrap()
    }
}

#[test]
fn wide_biomes() {
    let Some(lines) = lines("biomes.txt") else {
        return;
    };
    let mut worlds = Worlds(None);
    let mut report = Report::default();
    for line in &lines {
        let t: Vec<&str> = line.split(' ').collect();
        let dim = dimension(t[0]);
        let seed: i64 = t[1].parse().unwrap();
        let (qx, qy, qz) = (
            t[2].parse().unwrap(),
            t[3].parse().unwrap(),
            t[4].parse().unwrap(),
        );
        let got = worlds.get(seed, dim).biome(qx, qy, qz).name();
        report.check(dim.id(), got == t[5], || {
            format!("seed {seed} quart {qx} {qy} {qz}: game {}, got {got}", t[5])
        });
    }
    report.finish("biomes");
}

#[test]
fn wide_heights() {
    let Some(lines) = lines("heights.txt") else {
        return;
    };
    let mut worlds = Worlds(None);
    let mut report = Report::default();
    for line in &lines {
        let t: Vec<&str> = line.split(' ').collect();
        let dim = dimension(t[0]);
        let seed: i64 = t[1].parse().unwrap();
        let (x, z): (i32, i32) = (t[2].parse().unwrap(), t[3].parse().unwrap());
        let want: (i32, i32) = (t[4].parse().unwrap(), t[5].parse().unwrap());
        let world = worlds.get(seed, dim);
        let got = (
            world.base_height(x, z, Heightmap::WorldSurface),
            world.base_height(x, z, Heightmap::OceanFloor),
        );
        report.check(dim.id(), got == want, || {
            format!("seed {seed} block {x} {z}: game {want:?} (surface, ocean floor), got {got:?}")
        });
    }
    report.finish("heights");
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
fn wide_structures() {
    let Some(lines) = lines("structures.txt") else {
        return;
    };
    let mut worlds = Worlds(None);
    let mut report = Report::default();
    for line in &lines {
        let mut t = line.split(' ');
        let dim = dimension(t.next().unwrap());
        let seed: i64 = t.next().unwrap().parse().unwrap();
        let set_name = t.next().unwrap();
        let cx: i32 = t.next().unwrap().parse().unwrap();
        let cz: i32 = t.next().unwrap().parse().unwrap();
        let want = t.collect::<Vec<_>>().join(" ");
        let set = STRUCTURE_SETS.find_set(set_name).expect("known set");
        let group = format!("{} {}", dim.id(), set_name.trim_start_matches("minecraft:"));
        // The game tried a structure here: Chunkscope must agree on that
        // (placement) and on what it tried and where it starts (validation).
        let placed = set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) == Some(true);
        let got = if placed {
            trace(&worlds.get(seed, dim).structures().attempt(set, cx, cz)).join(" ")
        } else {
            "(no attempt)".to_owned()
        };
        report.check(group, got == want, || {
            format!("seed {seed} chunk {cx} {cz}: game {want}, got {got}")
        });
    }
    report.finish("structures");
}

#[test]
fn wide_end_city_ships() {
    let Some(lines) = lines("ships.txt") else {
        return;
    };
    let mut report = Report::default();
    for line in &lines {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let (cx, cz): (i32, i32) = (t[1].parse().unwrap(), t[2].parse().unwrap());
        let want = t[3] == "yes";
        let got = end_city_has_ship(seed, cx, cz);
        report.check("end_city ships", got == want, || {
            format!("seed {seed} chunk {cx} {cz}: game ship {want}, got {got}")
        });
    }
    report.finish("ships");
}

#[test]
fn wide_spawn() {
    let Some(lines) = lines("spawn.txt") else {
        return;
    };
    let mut report = Report::default();
    for line in &lines {
        let v: Vec<i64> = line.split(' ').map(|t| t.parse().unwrap()).collect();
        let climate = OverworldClimate::new(v[0]);
        let origin = spawn_origin(&climate);
        let chunk = spawn_chunk(&climate);
        let got = [origin.0, origin.1, chunk.0, chunk.1].map(i64::from);
        report.check("spawn", got[..] == v[1..5], || {
            format!(
                "seed {}: game spawn {} {} origin {} {}, got {got:?}",
                v[0], v[1], v[2], v[3], v[4]
            )
        });
    }
    report.finish("spawn");
}

#[test]
fn wide_strongholds() {
    let Some(lines) = lines("strongholds.txt") else {
        return;
    };
    let mut report = Report::default();
    for line in &lines {
        let mut t = line.split(' ');
        let seed: i64 = t.next().unwrap().parse().unwrap();
        let want: Vec<(i32, i32)> = t
            .map(|p| {
                let (x, z) = p.split_once(',').unwrap();
                (x.parse().unwrap(), z.parse().unwrap())
            })
            .collect();
        let got = World::new(seed, Dimension::Overworld).strongholds();
        report.check("strongholds", got == want, || {
            let first = got
                .iter()
                .zip(&want)
                .position(|(g, w)| g != w)
                .unwrap_or(got.len().min(want.len()));
            format!(
                "seed {seed}: {} of {} differ, first at #{first}: game {:?}, got {:?}",
                got.iter().zip(&want).filter(|(g, w)| g != w).count(),
                want.len(),
                want.get(first),
                got.get(first)
            )
        });
    }
    report.finish("strongholds");
}

#[test]
fn wide_gateways() {
    let Some(lines) = lines("gateways.txt") else {
        return;
    };
    let mut report = Report::default();
    for line in &lines {
        let mut t = line.split(' ');
        let seed: i64 = t.next().unwrap().parse().unwrap();
        let want: Vec<u8> = t.map(|n| n.parse().unwrap()).collect();
        let got = gateway_order(seed).to_vec();
        report.check("end gateways", got == want, || {
            format!("seed {seed}: game order {want:?}, got {got:?}")
        });
    }
    report.finish("gateways");
}
