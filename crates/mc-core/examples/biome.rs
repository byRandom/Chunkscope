//! Print the overworld biome at a block position.
//!
//! ```text
//! cargo run -p mc-core --example biome -- <seed text> <x> <z> [y]
//! ```

use mc_core::biome::OverworldBiomeSource;
use mc_core::parse_seed;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: biome <seed> <x> <z> [y]");
        std::process::exit(2);
    }
    let seed = parse_seed(&args[0]).expect("an empty seed means a random world");
    let x: i32 = args[1].parse().expect("x must be an integer");
    let z: i32 = args[2].parse().expect("z must be an integer");
    let y: i32 = args
        .get(3)
        .map_or(64, |y| y.parse().expect("y must be an integer"));
    let source = OverworldBiomeSource::new(seed);
    let biome = source.biome_at_block(x, y, z);
    println!("seed {seed}: {} at ({x}, {y}, {z})", biome.name());
}
