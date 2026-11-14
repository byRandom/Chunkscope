//! The per-dimension world: it must answer exactly like the sources it
//! wraps, and know which structure sets belong to it.

use mc_core::biome::{EndBiomeSource, NetherBiomeSource, OverworldBiomeSource};
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSets};
use mc_core::terrain::{EndTerrain, Heightmap, NetherTerrain, OverworldTerrain};
use mc_core::world::{Dimension, World};

#[test]
fn ids_round_trip() {
    for d in Dimension::ALL {
        assert_eq!(Dimension::from_id(d.id()), Some(d));
    }
    assert_eq!(Dimension::from_id("the_moon"), None);
}

#[test]
fn sets_belong_to_their_dimensions() {
    let dims = |name: &str| -> Vec<Dimension> {
        let set = STRUCTURE_SETS.find_set(name).unwrap();
        Dimension::ALL
            .into_iter()
            .filter(|d| d.has_set(set))
            .collect()
    };
    use Dimension::*;
    assert_eq!(dims("minecraft:villages"), [Overworld]);
    assert_eq!(dims("minecraft:mineshafts"), [Overworld]);
    assert_eq!(dims("minecraft:nether_complexes"), [Nether]);
    assert_eq!(dims("minecraft:nether_fossils"), [Nether]);
    assert_eq!(dims("minecraft:end_cities"), [End]);
    // Ruined portals have an Overworld and a Nether variant.
    assert_eq!(dims("minecraft:ruined_portals"), [Overworld, Nether]);
    for set in &STRUCTURE_SETS {
        if let StructurePlacement::RandomSpread(_) = set.placement {
            assert!(
                !dims(set.name).is_empty(),
                "{} is in no dimension",
                set.name
            );
        }
    }
}

#[test]
fn world_answers_like_its_sources() {
    let seed = -4_172_144_997_902_289_642;
    let points = [(0, 0), (1234, -567), (-30_000, 12_345), (100_000, 100_000)];
    let overworld = World::new(seed, Dimension::Overworld);
    let (ob, ot) = (OverworldBiomeSource::new(seed), OverworldTerrain::new(seed));
    let nether = World::new(seed, Dimension::Nether);
    let (nb, nt) = (NetherBiomeSource::new(seed), NetherTerrain::new(seed));
    let end = World::new(seed, Dimension::End);
    let (eb, et) = (EndBiomeSource::new(seed), EndTerrain::new(seed));
    for (x, z) in points {
        for y in [-60, 0, 40, 100] {
            let q = (x >> 2, y >> 2, z >> 2);
            assert_eq!(overworld.biome(q.0, q.1, q.2), ob.biome(q.0, q.1, q.2));
            assert_eq!(nether.biome(q.0, q.1, q.2), nb.biome(q.0, q.1, q.2));
            assert_eq!(end.biome(q.0, q.1, q.2), eb.biome(q.0, q.1, q.2));
        }
        for h in [Heightmap::WorldSurface, Heightmap::OceanFloor] {
            assert_eq!(overworld.base_height(x, z, h), ot.base_height(x, z, h));
            assert_eq!(nether.base_height(x, z, h), nt.base_height(x, z, h));
            assert_eq!(end.base_height(x, z, h), et.base_height(x, z, h));
        }
        assert_eq!(nether.base_column(x, z), nt.base_column(x, z));
    }
    assert_eq!(overworld.level().sea_level, 63);
    assert_eq!(nether.level().sea_level, 32);
    assert!(nether.strongholds().is_empty() && end.strongholds().is_empty());
}
