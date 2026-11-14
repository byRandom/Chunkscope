//! List the structures near a position: where each overworld structure set
//! tries to place a structure, checked against biome and terrain like the game
//! does.
//!
//! ```text
//! cargo run -p mc-core --example structures -- <seed text> <x> <z> [radius in blocks]
//! ```

use mc_core::parse_seed;
use mc_core::structure::scan::{ChunkRect, SetFilter};
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement};
use mc_core::world::{Dimension, World};

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

    let world = World::new(seed, Dimension::Overworld);
    let generator = world.structures();
    let mut found = Vec::new();
    for set in &STRUCTURE_SETS {
        let StructurePlacement::RandomSpread(_) = &set.placement else {
            continue; // strongholds are listed below
        };
        if !Dimension::Overworld.has_set(set) {
            continue;
        }
        let rect = ChunkRect::around_block(x, z, radius);
        for hit in generator.hits(&SetFilter::all(set), rect) {
            let d = hit.candidate.distance((x, z));
            if d > radius as f64 {
                continue;
            }
            let (bx, bz) = hit.candidate.locate;
            found.push((d, hit.generated.name.to_owned(), bx, bz));
        }
    }
    // Strongholds: exact positions (they need the biome search, ~1 s).
    for (cx, cz) in world.strongholds() {
        // what /locate reports: the chunk's minimum corner (no locate offset)
        let (bx, bz) = (cx << 4, cz << 4);
        let d = (((bx - x) as f64).powi(2) + ((bz - z) as f64).powi(2)).sqrt();
        if d <= radius as f64 {
            found.push((d, "minecraft:stronghold".to_owned(), bx, bz));
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!("seed {seed}: structures within {radius} blocks of ({x}, {z})");
    for (d, name, bx, bz) in found {
        println!(
            "{:>7.0}  {:<30} {bx}, {bz}",
            d,
            name.trim_start_matches("minecraft:")
        );
    }
}
