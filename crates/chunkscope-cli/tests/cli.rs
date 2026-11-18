//! Runs the real binary: output formats, exit codes and agreement with the
//! library (which the oracle tests check against the game).

use std::process::{Command, Output};

use mc_core::biome::OverworldClimate;
use mc_core::finder::{Query, check};
use mc_core::spawn::spawn_origin;
use mc_core::world::{Dimension, World};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_chunkscope-cli"))
        .args(args)
        .output()
        .expect("run chunkscope-cli")
}

fn ok(args: &[&str]) -> String {
    let out = run(args);
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// A usage mistake: exit code 2, the reason on stderr, nothing on stdout.
fn usage_error(args: &[&str]) -> String {
    let out = run(args);
    assert_eq!(out.status.code(), Some(2), "{args:?}");
    assert!(out.stdout.is_empty(), "{args:?}");
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn biome_matches_the_library() {
    for dim in Dimension::ALL {
        let want = World::new(1, dim).biome_at_block(100, 64, -300).name();
        let json = ok(&[
            "biome",
            "1",
            "100",
            "-300",
            "--dimension",
            dim.id(),
            "--json",
        ]);
        assert_eq!(
            json.trim(),
            format!(
                r#"{{"seed":1,"dimension":"{}","x":100,"y":64,"z":-300,"biome":"{want}"}}"#,
                dim.id()
            )
        );
    }
    let text = ok(&["biome", "1", "100", "-300"]);
    assert!(text.starts_with("seed 1 (overworld): minecraft:"), "{text}");
}

#[test]
fn text_seeds_hash_like_the_game() {
    // "hello".hashCode() == 99162322
    assert!(ok(&["biome", "hello", "0", "0"]).starts_with("seed 99162322 "));
}

#[test]
fn height_has_both_heightmaps_and_the_column() {
    let json = ok(&["height", "42", "100", "-200", "--json"]);
    assert!(
        json.contains(r#""world_surface":68,"ocean_floor":68,"#),
        "{json}"
    );
    assert!(
        json.contains(r#""column":{"solid":132,"air":252,"#),
        "{json}"
    );
    // Under the sea the two heightmaps differ, and the text says by how much.
    let sea = ok(&["height", "1", "100", "100", "--json"]);
    assert!(
        sea.contains(r#""world_surface":63,"ocean_floor":51,"#),
        "{sea}"
    );
    assert!(ok(&["height", "1", "100", "100"]).contains("under 12 blocks of water"));
}

#[test]
fn spawn_origin_matches_the_library() {
    let (ox, oz) = spawn_origin(&OverworldClimate::new(1));
    let json = ok(&["spawn", "1", "--json"]);
    assert!(
        json.starts_with(&format!(
            r#"{{"seed":1,"origin":{{"x":{ox},"z":{oz}}},"chunk":{{"x":{},"z":{}}}"#,
            ox >> 4,
            oz >> 4
        )),
        "{json}"
    );
}

#[test]
fn structures_are_sorted_by_distance() {
    let text = ok(&["structures", "7", "0", "0", "500"]);
    let distances: Vec<f64> = text
        .lines()
        .skip(1)
        .map(|l| l.split_whitespace().next().unwrap().parse().unwrap())
        .collect();
    assert!(!distances.is_empty() && distances.windows(2).all(|w| w[0] <= w[1]));
    // The radius is a hard limit (the text rounds the distance to a block).
    assert!(distances.iter().all(|&d| d <= 500.5), "{distances:?}");
    let json = ok(&["structures", "7", "0", "0", "500", "--json"]);
    assert!(json.starts_with(
        r#"{"seed":7,"dimension":"overworld","center":{"x":0,"z":0},"radius":500,"structures":[{"#
    ));
    // Nether sets only in the Nether.
    let nether = ok(&["structures", "7", "0", "0", "400", "--dimension", "nether"]);
    let names: Vec<&str> = nether
        .lines()
        .skip(1)
        .map(|l| l.split_whitespace().nth(1).unwrap())
        .collect();
    assert!(!names.is_empty());
    let nether_names = [
        "nether_fossil",
        "fortress",
        "bastion_remnant",
        "ruined_portal_nether",
    ];
    assert!(names.iter().all(|n| nether_names.contains(n)), "{names:?}");
}

#[test]
fn find_prints_what_the_library_matches() {
    let query = "villages:250 biome=plains";
    let parsed: Query = query.parse().unwrap();
    let want: Vec<String> = (0..200)
        .filter_map(|seed| check(seed, &parsed))
        .map(|m| m.to_string())
        .collect();
    assert!(!want.is_empty());
    let mut got: Vec<String> = ok(&[
        "find",
        "--count",
        "200",
        "--threads",
        "2",
        "villages:250",
        "biome=plains",
    ])
    .lines()
    .map(str::to_owned)
    .collect();
    got.sort();
    let mut want = want;
    want.sort();
    assert_eq!(got, want);

    let json = ok(&["find", "--count", "200", "--json", query]);
    for line in json.lines() {
        assert!(
            line.starts_with(r#"{"seed":"#) || line.starts_with("{\"seed\":-"),
            "{line}"
        );
        assert!(
            line.contains(r#""biome":"minecraft:plains""#) && line.ends_with("}]}"),
            "{line}"
        );
    }
    assert_eq!(json.lines().count(), got.len());
}

#[test]
fn targets_list_every_target() {
    let text = ok(&["targets"]);
    assert_eq!(text.lines().count(), mc_core::finder::TARGETS.len());
    assert!(
        ok(&["targets", "--json"]).starts_with(r#"[{"id":"villages","dimension":"overworld"}"#)
    );
}

#[test]
fn mistakes_exit_with_2_and_say_why() {
    assert!(usage_error(&[]).contains("usage"));
    assert!(usage_error(&["nope"]).contains("unknown command"));
    assert!(usage_error(&["biome", "1", "x", "0"]).contains("x must be an integer"));
    assert!(usage_error(&["biome", "", "0", "0"]).contains("seed is empty"));
    assert!(
        usage_error(&["biome", "1", "0", "0", "--dimension", "moon"])
            .contains("overworld, nether, end")
    );
    assert!(usage_error(&["height", "1", "0", "0", "--bogus"]).contains("unknown option"));
    assert!(usage_error(&["find", "nope:3"]).contains("unknown structure"));
    assert!(usage_error(&["find", "--count", "x", "villages:100"]).contains("not a valid number"));
}

#[test]
fn help_and_version() {
    assert!(ok(&["help"]).contains("commands:"));
    assert!(ok(&["find", "--help"]).contains("--random"));
    assert!(ok(&["version"]).starts_with("chunkscope-cli "));
}

#[test]
fn random_seeds_come_from_the_salt() {
    // One seed per run: neighbouring salts share most of their seeds, so
    // only single seeds tell them apart.
    let mut matched = 0;
    for salt in 0..40u64 {
        let seeds = mc_core::finder::Seeds::Random { salt, count: 1 };
        let query: Query = "villages:250".parse().unwrap();
        let want = check(seeds.get(0), &query).map(|m| m.to_string());
        let got = ok(&[
            "find",
            "--random",
            "--from",
            &salt.to_string(),
            "--count",
            "1",
            "villages:250",
        ]);
        assert_eq!(got.trim(), want.clone().unwrap_or_default(), "salt {salt}");
        matched += usize::from(want.is_some());
    }
    assert!(matched > 3, "the query should match some of these seeds");
}
