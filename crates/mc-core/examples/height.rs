//! Print the generated terrain height at a block column (before any
//! decoration or structure), as structure placement sees it.
//!
//! ```text
//! cargo run --release -p mc-core --example height -- <seed text> <x> <z>
//! ```

use mc_core::parse_seed;
use mc_core::terrain::{Heightmap, OverworldTerrain, SEA_LEVEL, Substance};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: height <seed> <x> <z>");
        std::process::exit(2);
    }
    let seed = parse_seed(&args[0]).expect("an empty seed means a random world");
    let x: i32 = args[1].parse().expect("x must be an integer");
    let z: i32 = args[2].parse().expect("z must be an integer");
    let terrain = OverworldTerrain::new(seed);
    let surface = terrain.base_height(x, z, Heightmap::WorldSurface);
    let floor = terrain.base_height(x, z, Heightmap::OceanFloor);
    println!("seed {seed} at ({x}, {z}):");
    println!("  WORLD_SURFACE_WG = {surface}");
    println!("  OCEAN_FLOOR_WG   = {floor}");
    if surface > floor {
        println!(
            "  under {} blocks of water (sea level {SEA_LEVEL})",
            surface - floor
        );
    }
    let column = terrain.base_column(x, z);
    let count = |s: Substance| column.iter().filter(|&&c| c == s).count();
    println!(
        "  column: {} stone, {} air (caves and sky), {} water, {} lava",
        count(Substance::Solid),
        count(Substance::Air),
        count(Substance::Water),
        count(Substance::Lava)
    );
}
