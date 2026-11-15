//! Search seeds for structures near the spawn.
//!
//! ```text
//! cargo run --release -p mc-core --example find -- [options] <query>...
//!   --from <seed>     first seed of a sequential search (default 0)
//!   --random          random seeds instead of sequential ones
//!   --count <n>       seeds to check (default 10000)
//! ```
//!
//! The query is the finder's text form, the same the app copies and pastes
//! (see `mc_core::finder`): `center=spawn|zero`,
//! `<structure>[+ship]:<radius>[:<count>]`, `biome=<biome>,...`.
//!
//! Example: two villages and a pillager outpost within 300 blocks of spawn,
//! in plains: `find villages:300:2 pillager_outposts:300 biome=plains`

use std::sync::atomic::Ordering;
use std::time::Instant;

use mc_core::finder::{Query, SearchState, Seeds, search};
use mc_core::parse_seed;

fn usage(message: &str) -> ! {
    eprintln!("{message}");
    eprintln!("usage: find [--from <seed>] [--count <n>] [--random] <query>...");
    std::process::exit(2)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut from, mut count, mut random) = (0i64, 10_000u64, false);
    let mut words = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--from" => {
                from = args
                    .next()
                    .and_then(|v| parse_seed(&v))
                    .unwrap_or_else(|| usage("--from <seed>"))
            }
            "--count" => {
                count = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage("--count <n>"))
            }
            "--random" => random = true,
            _ => words.push(a),
        }
    }
    let query: Query = words
        .join(" ")
        .parse()
        .unwrap_or_else(|e| usage(&format!("{e}")));
    eprintln!("query: {query}");
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
    search(seeds, &query, threads, &state, &|m| println!("{m}"));
    let checked = state.checked.load(Ordering::Relaxed);
    eprintln!(
        "checked {checked} seeds in {:.1} s ({:.0} seeds/s, {threads} threads)",
        start.elapsed().as_secs_f64(),
        checked as f64 / start.elapsed().as_secs_f64()
    );
}
