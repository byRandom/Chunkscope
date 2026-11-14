//! Mineshafts against the game: their pieces
//! (`parity/fixtures/game/mineshaft_pieces.txt`) and their generation
//! points, badlands ones included (`mineshaft_points.txt`).

use mc_core::biome::OverworldBiomeSource;
use mc_core::random::{LegacyRandomSource, RandomSource, WorldgenRandom};
use mc_core::structure::mineshaft::{MineshaftPiece, PieceKind, mineshaft_pieces};
use mc_core::structure::validate::{Structure, StructureGenerator};
use mc_core::terrain::OverworldTerrain;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/../../parity/fixtures/game/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("run parity/game-oracle/run.sh")
}

fn format(p: &MineshaftPiece) -> String {
    let kind = match p.kind {
        PieceKind::Room => "Room",
        PieceKind::Corridor => "Corridor",
        PieceKind::Crossing { two_floored: false } => "Crossing",
        PieceKind::Crossing { two_floored: true } => "Crossing2",
        PieceKind::Stairs => "Stairs",
    };
    let b = p.bbox;
    format!(
        "{kind}:{},{},{},{},{},{}:{}",
        b[0], b[1], b[2], b[3], b[4], b[5], p.gen_depth
    )
}

#[test]
fn pieces_match_the_game() {
    let text = fixture("mineshaft_pieces.txt");
    let mut lines = 0;
    let mut pieces = 0;
    for line in text.lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let cx: i32 = parts.next().unwrap().parse().unwrap();
        let cz: i32 = parts.next().unwrap().parse().unwrap();
        let next: i32 = parts.next().unwrap().parse().unwrap();
        let want: Vec<&str> = parts.collect();
        let mut random = WorldgenRandom::new(LegacyRandomSource::new(0));
        random.set_large_feature_seed(seed, cx, cz);
        random.next_double();
        let got: Vec<String> = mineshaft_pieces(&mut random, cx, cz)
            .iter()
            .map(format)
            .collect();
        assert_eq!(got, want, "{seed} {cx} {cz}");
        assert_eq!(
            random.next_int(),
            next,
            "{seed} {cx} {cz}: random numbers used"
        );
        lines += 1;
        pieces += got.len();
    }
    assert!(
        lines >= 800 && pieces > 50_000,
        "{lines} mineshafts, {pieces} pieces"
    );
}

#[test]
fn generation_points_match_the_game() {
    let text = fixture("mineshaft_points.txt");
    let mut sources: Option<(i64, OverworldBiomeSource, OverworldTerrain)> = None;
    let (mut checked, mut generated) = (0, 0);
    for line in text.lines() {
        let mut parts = line.split(' ');
        let seed: i64 = parts.next().unwrap().parse().unwrap();
        let name = format!("minecraft:{}", parts.next().unwrap());
        if sources.as_ref().is_none_or(|s| s.0 != seed) {
            sources = Some((
                seed,
                OverworldBiomeSource::new(seed),
                OverworldTerrain::new(seed),
            ));
        }
        let (_, biomes, terrain) = sources.as_ref().unwrap();
        let generator = StructureGenerator::new(seed, biomes, terrain);
        let structure = Structure::find(&name).unwrap();
        for entry in parts {
            let (chunk, want) = entry.split_once(':').unwrap();
            let (cx, cz) = chunk.split_once(',').unwrap();
            let (cx, cz) = (cx.parse().unwrap(), cz.parse().unwrap());
            let got = match generator.try_structure(structure, cx, cz) {
                Some((x, y, z)) => format!("{x},{y},{z}"),
                None => "no".to_owned(),
            };
            assert_eq!(got, want, "{name} seed {seed} chunk ({cx}, {cz})");
            checked += 1;
            generated += usize::from(want != "no");
        }
    }
    assert!(
        checked >= 2000 && generated > 1500,
        "{checked} checked, {generated} generate"
    );
}
