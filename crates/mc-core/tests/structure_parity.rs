//! Structure placement parity against the real game
//! (parity/game-oracle/GameOracle.java).

use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSets};

const PLACEMENT: &str = include_str!("../../../parity/fixtures/game/structure_placement.txt");

#[test]
fn random_spread_placement_matches_game() {
    let mut checked = 0;
    let mut attempts = 0;
    for line in PLACEMENT.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let set = STRUCTURE_SETS
            .find_set(t[1])
            .unwrap_or_else(|| panic!("unknown set {}", t[1]));
        let StructurePlacement::RandomSpread(p) = &set.placement else {
            panic!("{} is not random_spread", set.name);
        };
        if t[2] == "grid" {
            let (lo, hi): (i32, i32) = (t[3].parse().unwrap(), t[4].parse().unwrap());
            let mut bits = t[5].bytes();
            for cz in lo..hi {
                for cx in lo..hi {
                    let want = bits.next().unwrap() == b'1';
                    let got = set
                        .is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz)
                        .unwrap();
                    assert_eq!(got, want, "{} seed {seed} chunk ({cx}, {cz})", set.name);
                    checked += 1;
                    attempts += got as usize;
                }
            }
            continue;
        }
        for region in &t[2..] {
            let (r, want) = region.split_once('=').unwrap();
            let r: Vec<i32> = r.split(',').map(|v| v.parse().unwrap()).collect();
            let (rx, rz) = (r[0], r[1]);
            let (cx, cz) = p.region_chunk(seed, rx, rz);
            assert_eq!(
                p.potential_chunk(seed, rx.wrapping_mul(p.spacing), rz.wrapping_mul(p.spacing)),
                (cx, cz)
            );
            let attempt = set
                .is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz)
                .unwrap();
            let (lx, _, lz) = p.locate_pos(cx, cz);
            let got = format!("{cx},{cz},{},{lx},{lz}", attempt as u8);
            assert_eq!(got, want, "{} seed {seed} region ({rx}, {rz})", set.name);
            checked += 1;
            attempts += attempt as usize;
        }
    }
    assert!(checked > 50_000, "checked {checked}");
    assert!(attempts > 1_000, "only {attempts} positive cases");
}

const FREQUENCY: &str = include_str!("../../../parity/fixtures/game/frequency_reduction.txt");

#[test]
fn frequency_reduction_matches_game() {
    use mc_core::structure::FrequencyReduction;
    let frequencies = [0.004f32, 0.01, 0.2, 0.25, 0.5, 0.75, 0.999];
    for line in FREQUENCY.lines() {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: i64 = t[0].parse().unwrap();
        let salt: i32 = t[1].parse().unwrap();
        let (x, z): (i32, i32) = (t[2].parse().unwrap(), t[3].parse().unwrap());
        for entry in &t[4..] {
            let (name, want) = entry.split_once('=').unwrap();
            let method = match name {
                "default" => FrequencyReduction::Default,
                "legacy_type_1" => FrequencyReduction::LegacyType1,
                "legacy_type_2" => FrequencyReduction::LegacyType2,
                "legacy_type_3" => FrequencyReduction::LegacyType3,
                other => panic!("unknown method {other}"),
            };
            let mut got = String::new();
            for f in frequencies {
                let mut hits = 0u32;
                for d in 0..32 {
                    hits = hits << 1 | method.should_generate(seed, salt, x + d, z - d, f) as u32;
                }
                got.push_str(&format!("{hits:x},"));
            }
            assert_eq!(got, want, "{name} seed {seed} salt {salt} at ({x}, {z})");
        }
    }
}

const STRONGHOLDS: &str = include_str!("../../../parity/fixtures/game/strongholds.txt");

#[test]
fn stronghold_positions_match_game() {
    use mc_core::biome::OverworldBiomeSource;
    use mc_core::structure::stronghold::ring_positions;
    let rings = STRUCTURE_SETS
        .find_set("minecraft:strongholds")
        .and_then(|s| s.concentric_rings())
        .expect("strongholds use concentric rings");
    let mut mismatches = Vec::new();
    for line in STRONGHOLDS.lines() {
        let mut t = line.split(' ');
        let seed: i64 = t.next().unwrap().parse().unwrap();
        let want: Vec<(i32, i32)> = t
            .map(|p| {
                let (x, z) = p.split_once(',').unwrap();
                (x.parse().unwrap(), z.parse().unwrap())
            })
            .collect();
        assert_eq!(want.len(), rings.count as usize);
        let got = ring_positions(seed, &rings, &OverworldBiomeSource::new(seed));
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            if g != w {
                mismatches.push((seed, i, *g, *w));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "(seed, index, rust, game): {mismatches:?}"
    );
}
