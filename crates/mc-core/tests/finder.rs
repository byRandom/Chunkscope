//! The seed finder against a direct computation: every structure that
//! generates near the center, found without the finder's shortcuts.

use mc_core::finder::{Center, Query, StructureFilter, Target, check};
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSets};
use mc_core::world::{Dimension, World};

/// Structures of `set` that generate with their locate position within
/// `radius` of (0, 0), by scanning every region of a generous square.
fn direct(seed: i64, set_name: &str, radius: i32) -> usize {
    let set = STRUCTURE_SETS.find_set(set_name).unwrap();
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        unreachable!()
    };
    let world = World::new(seed, Dimension::Overworld);
    let generator = world.structures();
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
    for min_count in [1u32, 2] {
        let query = Query {
            center: Center::Zero,
            structures: vec![StructureFilter {
                min_count,
                ..StructureFilter::new(Target::find("villages").unwrap(), 400)
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
                assert!(m.found.iter().all(|f| f.distance <= 400.0));
            }
        }
        assert!(matches > 0, "the test needs some matches");
    }
}

/// Nether filters, measured from the Nether's (0, 0) with the Overworld
/// center at the spawn, and split by structure (fortresses, not bastions).
#[test]
fn nether_filters_agree_with_a_direct_scan() {
    let set = STRUCTURE_SETS
        .find_set("minecraft:nether_complexes")
        .unwrap();
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        unreachable!()
    };
    let radius = 300;
    let query = Query {
        center: Center::Spawn,
        structures: vec![StructureFilter::new(
            Target::find("fortresses").unwrap(),
            radius,
        )],
        center_biomes: None,
    };
    let mut matches = 0;
    for seed in 0..40i64 {
        let world = World::new(seed, Dimension::Nether);
        let generator = world.structures();
        let mut fortresses = 0;
        for rz in -3..=3 {
            for rx in -3..=3 {
                let (cx, cz) = p.region_chunk(seed, rx, rz);
                if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) != Some(true) {
                    continue;
                }
                let (x, _, z) = p.locate_pos(cx, cz);
                if ((x as f64).powi(2) + (z as f64).powi(2)).sqrt() > radius as f64 {
                    continue;
                }
                let outcome = generator.attempt(set, cx, cz);
                if outcome
                    .generated()
                    .is_some_and(|(s, _)| s.name == "minecraft:fortress")
                {
                    fortresses += 1;
                }
            }
        }
        let got = check(seed, &query);
        assert_eq!(got.is_some(), fortresses > 0, "seed {seed}");
        if let Some(m) = got {
            matches += 1;
            assert_eq!(m.found[0].name, "minecraft:fortress");
            assert_eq!(m.found[0].dimension, Dimension::Nether);
        }
    }
    assert!(matches > 0 && matches < 40, "the test needs both outcomes");
}

/// End filters, measured from the End's (0, 0).
#[test]
fn end_filters_agree_with_a_direct_scan() {
    let set = STRUCTURE_SETS.find_set("minecraft:end_cities").unwrap();
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        unreachable!()
    };
    let radius = 1250;
    let query = Query {
        center: Center::Spawn,
        structures: vec![StructureFilter::new(
            Target::find("end_cities").unwrap(),
            radius,
        )],
        center_biomes: None,
    };
    let mut matches = 0;
    for seed in 0..16i64 {
        let world = World::new(seed, Dimension::End);
        let generator = world.structures();
        let mut cities = 0;
        for rz in -6..=6 {
            for rx in -6..=6 {
                let (cx, cz) = p.region_chunk(seed, rx, rz);
                if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) != Some(true) {
                    continue;
                }
                let (x, _, z) = p.locate_pos(cx, cz);
                if ((x as f64).powi(2) + (z as f64).powi(2)).sqrt() > radius as f64 {
                    continue;
                }
                cities += usize::from(generator.attempt(set, cx, cz).generated().is_some());
            }
        }
        let got = check(seed, &query);
        assert_eq!(got.is_some(), cities > 0, "seed {seed}");
        if let Some(m) = got {
            matches += 1;
            assert_eq!(m.found[0].name, "minecraft:end_city");
            assert_eq!(m.found[0].dimension, Dimension::End);
        }
    }
    assert!(
        matches > 0 && matches < 16,
        "the test needs both outcomes ({matches})"
    );
}

/// The ship filter keeps only end cities with a ship.
#[test]
fn end_city_ship_filter() {
    use mc_core::structure::end_city::end_city_has_ship;

    let (mut with, mut without) = (0, 0);
    for seed in 0..24i64 {
        let filter = |ship| Query {
            center: Center::Zero,
            structures: vec![StructureFilter {
                ship_only: ship,
                ..StructureFilter::new(Target::find("end_cities").unwrap(), 1400)
            }],
            center_biomes: None,
        };
        let any = check(seed, &filter(false));
        let shipped = check(seed, &filter(true));
        if let Some(m) = &shipped {
            let f = &m.found[0];
            assert!(end_city_has_ship(seed, f.x >> 4, f.z >> 4), "seed {seed}");
            assert!(any.is_some());
            with += 1;
        } else if any.is_some() {
            without += 1;
        }
    }
    assert!(with > 0 && without > 0, "{with} with, {without} without");
}

/// The text form round-trips, and the CLI's shorthand fills in defaults.
#[test]
fn query_text_form() {
    let text = "center=zero villages:300:2 end_cities+ship:1000 fortresses:200 biome=plains,meadow";
    let query: Query = text.parse().unwrap();
    assert_eq!(query.center, Center::Zero);
    assert_eq!(query.structures.len(), 3);
    let cities = query.structures[1];
    assert_eq!(cities.target.id, "end_cities");
    assert_eq!(cities.dimension(), Dimension::End);
    assert!(cities.ship_only && cities.select().ship_only);
    assert_eq!((cities.radius, cities.min_count), (1000, 1));
    assert_eq!(query.structures[0].min_count, 2);
    assert_eq!(
        query.structures[2].select().only,
        Some("minecraft:fortress")
    );
    assert_eq!(query.to_string(), text);
    assert_eq!(text.parse::<Query>().unwrap(), query);

    // Defaults, extra spaces, explicit `:1`, `minecraft:` biome names, repeats.
    let short: Query = "  villages:300:1   biome=minecraft:plains biome=plains,forest "
        .parse()
        .unwrap();
    assert_eq!(
        short.to_string(),
        "center=spawn villages:300 biome=plains,forest"
    );
    assert_eq!("".parse::<Query>().unwrap(), Query::default());
}

#[test]
fn bad_queries_say_why() {
    for (text, why) in [
        ("nope:1", "unknown structure `nope`"),
        ("strongholds:100", "unknown structure `strongholds`"),
        ("villages", "needs a radius"),
        ("villages:", "bad radius"),
        ("villages:-5", "bad radius"),
        ("villages:300:0", "at least 1"),
        ("villages:300:2:4", "too many"),
        ("villages:99999999999", "bad radius"),
        ("villages:3000000000", "too large"),
        ("bastions+ship:100", "does not apply"),
        ("center=moon", "`spawn` or `zero`"),
        ("biome=nether_wastes", "unknown Overworld biome"),
        ("biome=nope", "unknown Overworld biome"),
    ] {
        let err = text.parse::<Query>().unwrap_err().to_string();
        assert!(err.contains(why), "{text}: {err}");
    }
}

/// Every target names a set that exists in its dimension, ids are unique,
/// and narrowing (`only`) matches some structure of the set.
#[test]
fn targets_are_consistent() {
    use mc_core::finder::TARGETS;
    for (i, t) in TARGETS.iter().enumerate() {
        assert_eq!(Target::find(t.id), Some(t), "{}", t.id);
        assert!(TARGETS[..i].iter().all(|o| o.id != t.id));
        let set = t.set();
        assert!(t.dimension.has_set(set), "{}", t.id);
        if let Some(only) = t.only {
            assert!(set.structures.iter().any(|(n, _)| n.starts_with(only)));
        }
        let f: StructureFilter = format!("{}:10", t.id).parse().unwrap();
        assert_eq!(f.target, t);
    }
}

#[test]
fn matches_print_on_one_line() {
    use mc_core::biome::Biome;
    use mc_core::finder::{Found, Match};
    let m = Match {
        seed: -42,
        center: (8, -24),
        center_biome: Biome::from_name("minecraft:plains").unwrap(),
        found: vec![
            Found {
                name: "minecraft:village_plains",
                dimension: Dimension::Overworld,
                x: 120,
                z: -48,
                distance: 129.6,
            },
            Found {
                name: "minecraft:fortress",
                dimension: Dimension::Nether,
                x: 64,
                z: 200,
                distance: 210.0,
            },
        ],
    };
    assert_eq!(
        m.to_string(),
        "-42  center 8, -24  plains  village_plains 120, -48 (130); fortress (nether) 64, 200 (210)"
    );
}

/// `ship_only` set on a target without ships (as the app's form can leave
/// it) is ignored: not printed, not filtered on.
#[test]
fn ship_only_needs_ships() {
    let f = StructureFilter {
        ship_only: true,
        ..StructureFilter::new(Target::find("bastions").unwrap(), 100)
    };
    assert_eq!(f.to_string(), "bastions:100");
    assert!(!f.select().ship_only);
    assert!(f.to_string().parse::<StructureFilter>().is_ok());
}
