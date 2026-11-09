//! Search seeds for structures near the spawn.
//!
//! ```text
//! cargo run --release -p mc-core --example find -- [options] <set>:<radius>[:<count>] ...
//!   --from <seed>     first seed of a sequential search (default 0)
//!   --random          random seeds instead of sequential ones
//!   --count <n>       seeds to check (default 10000)
//!   --zero            measure from (0, 0) instead of the spawn
//!   --biome <name>    the biome at the spawn (repeatable)
//! ```
//!
//! Example: villages and a pillager outpost within 300 blocks of spawn:
//! `find villages:300 pillager_outposts:300`

use std::sync::atomic::Ordering;
use std::time::Instant;

use mc_core::biome::Biome;
use mc_core::finder::{Center, Query, SearchState, Seeds, StructureFilter, search};
use mc_core::structure::{STRUCTURE_SETS, StructureSets};

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut from, mut count, mut random) = (0i64, 10_000u64, false);
    let mut query = Query {
        center: Center::Spawn,
        structures: Vec::new(),
        center_biomes: None,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--from" => {
                from = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--from <seed>")
            }
            "--count" => {
                count = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--count <n>")
            }
            "--random" => random = true,
            "--zero" => query.center = Center::Zero,
            "--biome" => {
                let name = args.next().expect("--biome <name>");
                let biome = Biome::from_name(&format!(
                    "minecraft:{}",
                    name.trim_start_matches("minecraft:")
                ))
                .unwrap_or_else(|| panic!("unknown biome {name}"));
                query.center_biomes.get_or_insert_with(Vec::new).push(biome);
            }
            spec => {
                let mut parts = spec.split(':');
                let name = parts.next().unwrap();
                let set = STRUCTURE_SETS
                    .find_set(&format!(
                        "minecraft:{}",
                        name.trim_start_matches("minecraft:")
                    ))
                    .unwrap_or_else(|| panic!("unknown structure set {name}"));
                let radius = parts
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("<set>:<radius>");
                let min_count = parts.next().map_or(1, |v| v.parse().expect("count"));
                query.structures.push(StructureFilter {
                    set,
                    radius,
                    min_count,
                });
            }
        }
    }
    let seeds = if random {
        Seeds::Random {
            salt: from as u64,
            count,
        }
    } else {
        Seeds::Sequential { start: from, count }
    };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let state = SearchState::default();
    let start = Instant::now();
    search(seeds, &query, threads, &state, &|m| {
        let found: Vec<String> = m
            .found
            .iter()
            .map(|f| {
                format!(
                    "{}{} {}, {} ({:.0})",
                    f.name.trim_start_matches("minecraft:"),
                    if f.exact { "" } else { " (possible)" },
                    f.x,
                    f.z,
                    f.distance
                )
            })
            .collect();
        println!(
            "{}  center {}, {}  {}  {}",
            m.seed,
            m.center.0,
            m.center.1,
            m.center_biome.name().trim_start_matches("minecraft:"),
            found.join("; ")
        );
    });
    let checked = state.checked.load(Ordering::Relaxed);
    eprintln!(
        "checked {checked} seeds in {:.1} s ({:.0} seeds/s, {threads} threads)",
        start.elapsed().as_secs_f64(),
        checked as f64 / start.elapsed().as_secs_f64()
    );
}
