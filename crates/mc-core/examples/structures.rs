//! List the chunks near a position where structure sets *attempt* to
//! generate. Biome and terrain checks are not implemented yet, so an attempt
//! is a candidate, not a guaranteed structure.
//!
//! ```text
//! cargo run -p mc-core --example structures -- <seed text> <x> <z> [radius in blocks]
//! ```

use mc_core::parse_seed;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: structures <seed> <x> <z> [radius]");
        std::process::exit(2);
    }
    let seed = parse_seed(&args[0]).expect("an empty seed means a random world");
    let x: i32 = args[1].parse().expect("x must be an integer");
    let z: i32 = args[2].parse().expect("z must be an integer");
    let radius: i32 = args
        .get(3)
        .map_or(1000, |r| r.parse().expect("radius must be an integer"));

    let mut found = Vec::new();
    for set in &STRUCTURE_SETS {
        let StructurePlacement::RandomSpread(p) = &set.placement else {
            continue; // strongholds: not implemented yet
        };
        // Sets don't record their dimension (their structures' biomes do, and
        // structure biome checks are not implemented yet), so skip these by name.
        if [
            "minecraft:nether_complexes",
            "minecraft:nether_fossils",
            "minecraft:end_cities",
        ]
        .contains(&set.name)
        {
            continue;
        }
        if p.spacing == 1 {
            continue; // mineshafts and buried treasure: almost every chunk is a candidate
        }
        let span = (radius >> 4) / p.spacing + 1;
        let (rx0, rz0) = (
            (x >> 4).div_euclid(p.spacing),
            (z >> 4).div_euclid(p.spacing),
        );
        for rz in rz0 - span..=rz0 + span {
            for rx in rx0 - span..=rx0 + span {
                let (cx, cz) = p.region_chunk(seed, rx, rz);
                if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) != Some(true) {
                    continue;
                }
                let (bx, _, bz) = p.locate_pos(cx, cz);
                let d = (((bx - x) as f64).powi(2) + ((bz - z) as f64).powi(2)).sqrt();
                if d <= radius as f64 {
                    found.push((d, set.name, bx, bz));
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("seed {seed}: structure attempts within {radius} blocks of ({x}, {z})");
    println!("(candidates: biome and terrain are not checked yet)");
    for (d, name, bx, bz) in found {
        println!(
            "{:>7.0}  {:<30} {bx}, {bz}",
            d,
            name.trim_start_matches("minecraft:")
        );
    }
}
