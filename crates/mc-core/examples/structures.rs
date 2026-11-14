//! List the structures near a position: where each overworld structure set
//! tries to place a structure, checked against biome and terrain like the game
//! does.
//!
//! ```text
//! cargo run -p mc-core --example structures -- <seed text> <x> <z> [radius in blocks]
//! ```

use mc_core::biome::OverworldBiomeSource;
use mc_core::parse_seed;
use mc_core::structure::scan::{ChunkRect, SetFilter};
use mc_core::structure::stronghold::ring_positions;
use mc_core::structure::validate::StructureGenerator;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSets};
use mc_core::terrain::OverworldTerrain;

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

    let biomes = OverworldBiomeSource::new(seed);
    let terrain = OverworldTerrain::new(seed);
    let generator = StructureGenerator::new(seed, &biomes, &terrain);
    let mut found = Vec::new();
    for set in &STRUCTURE_SETS {
        let StructurePlacement::RandomSpread(_) = &set.placement else {
            continue; // strongholds are listed below
        };
        // Sets don't record their dimension (their structures' biomes do), so
        // skip the Nether and End ones by name.
        if [
            "minecraft:nether_complexes",
            "minecraft:nether_fossils",
            "minecraft:end_cities",
        ]
        .contains(&set.name)
        {
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
    if let Some(rings) = STRUCTURE_SETS
        .find_set("minecraft:strongholds")
        .and_then(|s| s.concentric_rings())
    {
        for (cx, cz) in ring_positions(seed, &rings, &OverworldBiomeSource::new(seed)) {
            // what /locate reports: the chunk's minimum corner (no locate offset)
            let (bx, bz) = (cx << 4, cz << 4);
            let d = (((bx - x) as f64).powi(2) + ((bz - z) as f64).powi(2)).sqrt();
            if d <= radius as f64 {
                found.push((d, "minecraft:stronghold".to_owned(), bx, bz));
            }
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
