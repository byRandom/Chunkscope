//! The commands. Each prints text by default and JSON with `--json`.

use std::sync::atomic::Ordering;
use std::time::Instant;

use mc_core::biome::OverworldClimate;
use mc_core::finder::{Match, Query, SearchState, Seeds, TARGETS, search};
use mc_core::parse_seed;
use mc_core::spawn::{spawn_chunk, spawn_estimate, spawn_origin};
use mc_core::structure::end_gateway::gateways;
use mc_core::structure::scan::ChunkRect;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement};
use mc_core::terrain::{Heightmap, OverworldTerrain, SEA_LEVEL, Substance};
use mc_core::world::{Dimension, World};

use crate::args::Args;
use crate::json;

type Result<T = ()> = std::result::Result<T, String>;

pub fn run(command: &str, rest: &[String]) -> Result {
    match command {
        "biome" => biome(rest),
        "height" => height(rest),
        "spawn" => spawn(rest),
        "structures" => structures(rest),
        "find" => find(rest),
        "targets" => targets(rest),
        other => Err(format!("unknown command {other:?}")),
    }
}

const BIOME_USAGE: &str =
    "usage: chunkscope-cli biome <seed> <x> <z> [y] [--dimension <d>] [--json]";
const HEIGHT_USAGE: &str = "usage: chunkscope-cli height <seed> <x> <z> [--dimension <d>] [--json]";
const SPAWN_USAGE: &str = "usage: chunkscope-cli spawn <seed> [--json]";
const STRUCTURES_USAGE: &str =
    "usage: chunkscope-cli structures <seed> <x> <z> [radius] [--dimension <d>] [--json]";
const FIND_USAGE: &str = "\
usage: chunkscope-cli find [options] <query>...
  --from <seed>   first seed of a sequential search (default 0)
  --random        random seeds instead of sequential ones (--from is the salt)
  --count <n>     seeds to check (default 10000)
  --threads <n>   worker threads (default: all cores)
  --json          one JSON object per match, one per line

The query is the finder's text form, the same the app copies and pastes:
  center=spawn|zero  <target>[+ship]:<radius>[:<count>]  biome=<biome>,...
e.g. chunkscope-cli find villages:300:2 pillager_outposts:300 biome=plains
`chunkscope-cli targets` lists the targets.";
const TARGETS_USAGE: &str = "usage: chunkscope-cli targets [--json]";

/// A seed word: a number or any text, never blank.
fn seed(word: &str) -> Result<i64> {
    parse_seed(word).ok_or_else(|| "the seed is empty (the game would pick a random one)".into())
}

fn int(word: &str, what: &str) -> Result<i32> {
    word.parse()
        .map_err(|_| format!("{what} must be an integer, not {word:?}"))
}

/// The positional words, between `min` and `max` of them.
fn words<'a>(a: &'a Args, min: usize, max: usize, usage: &str) -> Result<&'a [String]> {
    if (min..=max).contains(&a.positional.len()) {
        Ok(&a.positional)
    } else {
        Err(usage.to_owned())
    }
}

fn dimension(a: &Args) -> Result<Dimension> {
    match a.option("dimension") {
        None => Ok(Dimension::Overworld),
        Some(id) => Dimension::from_id(id)
            .ok_or_else(|| format!("--dimension: {id:?} is not one of overworld, nether, end")),
    }
}

/// Parse a command's arguments; `Ok(None)` when it printed `--help`.
fn parse(raw: &[String], usage: &str, flags: &[&str], options: &[&str]) -> Result<Option<Args>> {
    let mut all_flags = vec!["help", "json"];
    all_flags.extend_from_slice(flags);
    let a = Args::parse(raw, &all_flags, options)?;
    if a.flag("help") {
        println!("{usage}");
        return Ok(None);
    }
    Ok(Some(a))
}

fn point(x: i32, y: Option<i32>, z: i32) -> String {
    let mut f = vec![("x", x.to_string())];
    if let Some(y) = y {
        f.push(("y", y.to_string()));
    }
    f.push(("z", z.to_string()));
    json::object(&f)
}

fn biome(raw: &[String]) -> Result {
    let Some(a) = parse(raw, BIOME_USAGE, &[], &["dimension"])? else {
        return Ok(());
    };
    let w = words(&a, 3, 4, BIOME_USAGE)?;
    let (seed, dim) = (seed(&w[0])?, dimension(&a)?);
    let (x, z) = (int(&w[1], "x")?, int(&w[2], "z")?);
    let y = w.get(3).map_or(Ok(64), |y| int(y, "y"))?;
    let name = World::new(seed, dim).biome_at_block(x, y, z).name();
    if a.flag("json") {
        println!(
            "{}",
            json::object(&[
                ("seed", seed.to_string()),
                ("dimension", json::string(dim.id())),
                ("x", x.to_string()),
                ("y", y.to_string()),
                ("z", z.to_string()),
                ("biome", json::string(name)),
            ])
        );
    } else {
        println!("seed {seed} ({}): {name} at ({x}, {y}, {z})", dim.id());
    }
    Ok(())
}

fn height(raw: &[String]) -> Result {
    let Some(a) = parse(raw, HEIGHT_USAGE, &[], &["dimension"])? else {
        return Ok(());
    };
    let w = words(&a, 3, 3, HEIGHT_USAGE)?;
    let (seed, dim) = (seed(&w[0])?, dimension(&a)?);
    let (x, z) = (int(&w[1], "x")?, int(&w[2], "z")?);
    let world = World::new(seed, dim);
    let surface = world.base_height(x, z, Heightmap::WorldSurface);
    let floor = world.base_height(x, z, Heightmap::OceanFloor);
    let column = world.base_column(x, z);
    let count = |s: Substance| column.iter().filter(|&&c| c == s).count();
    let (solid, air) = (count(Substance::Solid), count(Substance::Air));
    let (water, lava) = (count(Substance::Water), count(Substance::Lava));
    if a.flag("json") {
        println!(
            "{}",
            json::object(&[
                ("seed", seed.to_string()),
                ("dimension", json::string(dim.id())),
                ("x", x.to_string()),
                ("z", z.to_string()),
                ("world_surface", surface.to_string()),
                ("ocean_floor", floor.to_string()),
                (
                    "column",
                    json::object(&[
                        ("solid", solid.to_string()),
                        ("air", air.to_string()),
                        ("water", water.to_string()),
                        ("lava", lava.to_string()),
                    ])
                ),
            ])
        );
    } else {
        println!("seed {seed} ({}) at ({x}, {z}):", dim.id());
        println!("  WORLD_SURFACE_WG = {surface}");
        println!("  OCEAN_FLOOR_WG   = {floor}");
        if dim == Dimension::Overworld && surface > floor {
            println!(
                "  under {} blocks of water (sea level {SEA_LEVEL})",
                surface - floor
            );
        }
        println!("  column: {solid} stone, {air} air (caves and sky), {water} water, {lava} lava");
    }
    Ok(())
}

fn spawn(raw: &[String]) -> Result {
    let Some(a) = parse(raw, SPAWN_USAGE, &[], &[])? else {
        return Ok(());
    };
    let w = words(&a, 1, 1, SPAWN_USAGE)?;
    let seed = seed(&w[0])?;
    let climate = OverworldClimate::new(seed);
    let (ox, oz) = spawn_origin(&climate);
    let (cx, cz) = spawn_chunk(&climate);
    let (ex, ey, ez) = spawn_estimate(&climate, &OverworldTerrain::new(seed));
    if a.flag("json") {
        println!(
            "{}",
            json::object(&[
                ("seed", seed.to_string()),
                ("origin", point(ox, None, oz)),
                ("chunk", point(cx, None, cz)),
                ("estimate", point(ex, Some(ey), ez)),
            ])
        );
    } else {
        println!("seed {seed}");
        println!("  spawn origin (exact):     {ox}, {oz} (chunk {cx}, {cz})");
        println!("  spawn block (estimate):   {ex}, {ey}, {ez}");
    }
    Ok(())
}

struct Hit {
    distance: f64,
    structure: String,
    x: i32,
    z: i32,
    /// End gateways: which dragon defeat opens it (1 to 20).
    order: Option<u8>,
}

fn structures(raw: &[String]) -> Result {
    let Some(a) = parse(raw, STRUCTURES_USAGE, &[], &["dimension"])? else {
        return Ok(());
    };
    let w = words(&a, 3, 4, STRUCTURES_USAGE)?;
    let (seed, dim) = (seed(&w[0])?, dimension(&a)?);
    let (x, z) = (int(&w[1], "x")?, int(&w[2], "z")?);
    let radius = w.get(3).map_or(Ok(1000), |r| int(r, "radius"))?;
    if radius < 0 {
        return Err("radius must not be negative".into());
    }

    let world = World::new(seed, dim);
    let generator = world.structures();
    let mut found = Vec::new();
    for set in &STRUCTURE_SETS {
        if !matches!(set.placement, StructurePlacement::RandomSpread(_)) || !dim.has_set(set) {
            continue; // strongholds are listed below
        }
        let rect = ChunkRect::around_block(x, z, radius);
        for hit in generator.hits(&mc_core::structure::scan::SetFilter::all(set), rect) {
            let distance = hit.candidate.distance((x, z));
            if distance > radius as f64 {
                continue;
            }
            let (bx, bz) = hit.candidate.locate;
            found.push(Hit {
                distance,
                structure: hit.generated.name.to_owned(),
                x: bx,
                z: bz,
                order: None,
            });
        }
    }
    if dim == Dimension::Overworld {
        // Strongholds need the biome search (about a second). `/locate`
        // reports the chunk's minimum corner.
        for (cx, cz) in world.strongholds() {
            let (bx, bz) = (cx << 4, cz << 4);
            let distance = (((bx - x) as f64).powi(2) + ((bz - z) as f64).powi(2)).sqrt();
            if distance <= radius as f64 {
                found.push(Hit {
                    distance,
                    structure: "minecraft:stronghold".to_owned(),
                    x: bx,
                    z: bz,
                    order: None,
                });
            }
        }
    }
    if dim == Dimension::End {
        // The 20 gateways of the main island (the same blocks in every world;
        // the seed decides the order they open in).
        for g in gateways(seed) {
            let distance = (((g.x - x) as f64).powi(2) + ((g.z - z) as f64).powi(2)).sqrt();
            if distance <= radius as f64 {
                found.push(Hit {
                    distance,
                    structure: "minecraft:end_gateway".to_owned(),
                    x: g.x,
                    z: g.z,
                    order: Some(g.number),
                });
            }
        }
    }
    found.sort_by(|a, b| a.distance.total_cmp(&b.distance));

    if a.flag("json") {
        let items: Vec<String> = found
            .iter()
            .map(|h| {
                let mut fields = vec![
                    ("structure", json::string(&h.structure)),
                    ("x", h.x.to_string()),
                    ("z", h.z.to_string()),
                    ("distance", json::float(h.distance, 1)),
                ];
                if let Some(n) = h.order {
                    fields.push(("order", n.to_string()));
                }
                json::object(&fields)
            })
            .collect();
        println!(
            "{}",
            json::object(&[
                ("seed", seed.to_string()),
                ("dimension", json::string(dim.id())),
                ("center", point(x, None, z)),
                ("radius", radius.to_string()),
                ("structures", json::array(&items)),
            ])
        );
    } else {
        println!(
            "seed {seed} ({}): structures within {radius} blocks of ({x}, {z})",
            dim.id()
        );
        for h in &found {
            let name = h.structure.trim_start_matches("minecraft:");
            let name = match h.order {
                Some(n) => format!("{name} #{n}"),
                None => name.to_owned(),
            };
            println!("{:>7.0}  {:<30} {}, {}", h.distance, name, h.x, h.z);
        }
    }
    Ok(())
}

fn match_json(m: &Match) -> String {
    let found: Vec<String> = m
        .found
        .iter()
        .map(|f| {
            json::object(&[
                ("structure", json::string(f.name)),
                ("dimension", json::string(f.dimension.id())),
                ("x", f.x.to_string()),
                ("z", f.z.to_string()),
                ("distance", json::float(f.distance, 1)),
            ])
        })
        .collect();
    json::object(&[
        ("seed", m.seed.to_string()),
        ("center", point(m.center.0, None, m.center.1)),
        ("biome", json::string(m.center_biome.name())),
        ("found", json::array(&found)),
    ])
}

fn find(raw: &[String]) -> Result {
    let Some(a) = parse(raw, FIND_USAGE, &["random"], &["from", "count", "threads"])? else {
        return Ok(());
    };
    let from = match a.option("from") {
        None => 0,
        Some(v) => parse_seed(v).ok_or("--from: the seed is empty")?,
    };
    let count: u64 = a.number("count", 10_000)?;
    let threads: usize = a.number(
        "threads",
        std::thread::available_parallelism().map_or(4, |n| n.get()),
    )?;
    if threads == 0 {
        return Err("--threads must be at least 1".into());
    }
    let query: Query = a
        .positional
        .join(" ")
        .parse()
        .map_err(|e| format!("{e}\n{FIND_USAGE}"))?;
    eprintln!("query: {query}");
    let seeds = if a.flag("random") {
        Seeds::Random {
            salt: from as u64,
            count,
        }
    } else {
        Seeds::Sequential { start: from, count }
    };
    let as_json = a.flag("json");
    let state = SearchState::default();
    let start = Instant::now();
    search(seeds, &query, threads, &state, &|m| {
        if as_json {
            println!("{}", match_json(&m));
        } else {
            println!("{m}");
        }
    });
    let checked = state.checked.load(Ordering::Relaxed);
    let secs = start.elapsed().as_secs_f64();
    eprintln!(
        "checked {checked} seeds in {secs:.1} s ({:.0} seeds/s, {threads} threads)",
        checked as f64 / secs
    );
    Ok(())
}

fn targets(raw: &[String]) -> Result {
    let Some(a) = parse(raw, TARGETS_USAGE, &[], &[])? else {
        return Ok(());
    };
    if !a.positional.is_empty() {
        return Err(TARGETS_USAGE.to_owned());
    }
    if a.flag("json") {
        let items: Vec<String> = TARGETS
            .iter()
            .map(|t| {
                json::object(&[
                    ("id", json::string(t.id)),
                    ("dimension", json::string(t.dimension.id())),
                ])
            })
            .collect();
        println!("{}", json::array(&items));
    } else {
        for t in TARGETS.iter() {
            println!("{:<24} {}", t.id, t.dimension.id());
        }
    }
    Ok(())
}
