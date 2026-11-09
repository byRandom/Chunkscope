//! The seed finder against a direct computation: every structure that
//! generates near the center, found without the finder's shortcuts.

use mc_core::biome::OverworldBiomeSource;
use mc_core::finder::{Center, Query, StructureFilter, check};
use mc_core::structure::validate::StructureGenerator;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSets};
use mc_core::terrain::OverworldTerrain;

/// Structures of `set` that generate with their locate position within
/// `radius` of (0, 0), by scanning every region of a generous square.
fn direct(seed: i64, set_name: &str, radius: i32) -> usize {
    let set = STRUCTURE_SETS.find_set(set_name).unwrap();
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        unreachable!()
    };
    let biomes = OverworldBiomeSource::new(seed);
    let terrain = OverworldTerrain::new(seed);
    let generator = StructureGenerator::new(seed, &biomes, &terrain);
    let span = (radius / 16) / p.spacing + 2;
    let mut n = 0;
    for rz in -span..=span {
        for rx in -span..=span {
            let (cx, cz) = p.region_chunk(seed, rx, rz);
            if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) != Some(true) {
                continue;
            }
            let (x, _, z) = p.locate_pos(cx, cz);
            if ((x as f64).powi(2) + (z as f64).powi(2)).sqrt() > radius as f64 {
                continue;
            }
            if generator.attempt(set, cx, cz).generated().is_some() {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn finder_agrees_with_a_direct_scan() {
    let set = STRUCTURE_SETS.find_set("minecraft:villages").unwrap();
    for min_count in [1u32, 2] {
        let query = Query {
            center: Center::Zero,
            structures: vec![StructureFilter {
                set,
                radius: 400,
                min_count,
            }],
            center_biomes: None,
        };
        let mut matches = 0;
        for seed in 0..150i64 {
            let expected = direct(seed, "minecraft:villages", 400) >= min_count as usize;
            let got = check(seed, &query);
            assert_eq!(got.is_some(), expected, "seed {seed}, at least {min_count}");
            if let Some(m) = got {
                matches += 1;
                assert_eq!(m.found.len(), min_count as usize);
                assert!(m.found.iter().all(|f| f.exact && f.distance <= 400.0));
            }
        }
        assert!(matches > 0, "the test needs some matches");
    }
}
