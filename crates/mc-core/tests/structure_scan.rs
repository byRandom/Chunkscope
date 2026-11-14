//! The area scan against brute force: every chunk of a rectangle checked
//! on its own, without the scan's region arithmetic.

use mc_core::biome::OverworldBiomeSource;
use mc_core::structure::scan::{Candidate, ChunkRect, SetFilter, candidates};
use mc_core::structure::validate::StructureGenerator;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSets};
use mc_core::terrain::OverworldTerrain;

/// Rectangles that straddle region edges and the origin, thin ones and a
/// single chunk.
const RECTS: [ChunkRect; 5] = [
    ChunkRect {
        x0: -45,
        z0: -37,
        x1: 38,
        z1: 51,
    },
    ChunkRect {
        x0: 13,
        z0: -7,
        x1: 13,
        z1: 120,
    },
    ChunkRect {
        x0: -200,
        z0: 5,
        x1: -110,
        z1: 6,
    },
    ChunkRect {
        x0: 0,
        z0: 0,
        x1: 0,
        z1: 0,
    },
    ChunkRect {
        x0: 1_874_990,
        z0: -1_875_000,
        x1: 1_875_000,
        z1: -1_874_950,
    },
];

fn brute_force(set_name: &str, seed: i64, rect: ChunkRect) -> Vec<Candidate> {
    let set = STRUCTURE_SETS.find_set(set_name).unwrap();
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        unreachable!()
    };
    let mut out = Vec::new();
    for cz in rect.z0..=rect.z1 {
        for cx in rect.x0..=rect.x1 {
            if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) == Some(true) {
                let (x, _, z) = p.locate_pos(cx, cz);
                out.push(Candidate {
                    chunk: (cx, cz),
                    locate: (x, z),
                });
            }
        }
    }
    out
}

#[test]
fn candidates_match_brute_force() {
    let mut total = 0;
    for set in &STRUCTURE_SETS {
        if !matches!(set.placement, StructurePlacement::RandomSpread(_)) {
            continue;
        }
        for seed in [0i64, -1, 123_456_789, i64::MIN] {
            for rect in RECTS {
                let mut got: Vec<_> = candidates(set, seed, rect).collect();
                let mut want = brute_force(set.name, seed, rect);
                let key = |c: &Candidate| (c.chunk.1, c.chunk.0);
                got.sort_by_key(key);
                want.sort_by_key(key);
                assert_eq!(got, want, "{} seed {seed} {rect:?}", set.name);
                total += want.len();
            }
        }
    }
    assert!(total > 1000, "only {total} candidates");
}

#[test]
fn strongholds_have_no_candidates() {
    let set = STRUCTURE_SETS.find_set("minecraft:strongholds").unwrap();
    assert_eq!(candidates(set, 0, RECTS[0]).count(), 0);
}

#[test]
fn around_block_covers_the_radius() {
    for (x, z, r) in [(0, 0, 0), (-17, 31, 100), (1000, -1000, 4096), (-1, -1, 15)] {
        let rect = ChunkRect::around_block(x, z, r);
        for (bx, bz) in [
            (x - r, z - r),
            (x + r, z + r),
            (x - r, z + r),
            (x + r, z - r),
        ] {
            assert!(rect.contains(bx >> 4, bz >> 4), "({x}, {z}) r {r}");
        }
    }
}

/// Hits are the candidates whose attempt generates, with the filter's
/// prefix and the game's structure.
#[test]
fn hits_are_the_generating_candidates() {
    let seed = 42;
    let biomes = OverworldBiomeSource::new(seed);
    let terrain = OverworldTerrain::new(seed);
    let generator = StructureGenerator::new(seed, &biomes, &terrain);
    let rect = ChunkRect {
        x0: -64,
        z0: -64,
        x1: 63,
        z1: 63,
    };
    let set = STRUCTURE_SETS.find_set("minecraft:villages").unwrap();
    let hits: Vec<_> = generator.hits(&SetFilter::all(set), rect).collect();
    let want: Vec<_> = candidates(set, seed, rect)
        .filter_map(|c| {
            let (s, start) = generator.attempt(set, c.chunk.0, c.chunk.1).generated()?;
            Some((c, s.name, start))
        })
        .collect();
    assert!(!want.is_empty());
    assert_eq!(hits.len(), want.len());
    for (hit, (c, name, start)) in hits.iter().zip(&want) {
        let g = hit.generated;
        assert_eq!(
            (hit.candidate, g.name, g.start, g.ship),
            (*c, *name, *start, None)
        );
    }
    let plains = SetFilter {
        set,
        only: Some("minecraft:village_plains"),
        ship_only: false,
    };
    let n = generator.hits(&plains, rect).count();
    let want = want
        .iter()
        .filter(|w| w.1 == "minecraft:village_plains")
        .count();
    assert_eq!(n, want);
}
