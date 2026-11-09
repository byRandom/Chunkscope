//! Print the world spawn of a seed: the climate origin the game searches
//! around (exact) and the estimated spawn block.
//!
//! ```text
//! cargo run --release -p mc-core --example spawn -- <seed text>
//! ```

use mc_core::biome::OverworldClimate;
use mc_core::parse_seed;
use mc_core::spawn::{spawn_estimate, spawn_origin};
use mc_core::terrain::OverworldTerrain;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(text) = args.first() else {
        eprintln!("usage: spawn <seed>");
        std::process::exit(2);
    };
    let seed = parse_seed(text).expect("an empty seed means a random world");
    let climate = OverworldClimate::new(seed);
    let (ox, oz) = spawn_origin(&climate);
    println!("seed {seed}");
    println!(
        "  spawn origin (exact):     {ox}, {oz} (chunk {}, {})",
        ox >> 4,
        oz >> 4
    );
    let (x, y, z) = spawn_estimate(&climate, &OverworldTerrain::new(seed));
    println!("  spawn block (estimate):   {x}, {y}, {z}");
}
