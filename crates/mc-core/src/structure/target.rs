//! Targets: the named groups of structures that can be searched and shown,
//! each a structure set in one dimension, optionally narrowed to some of
//! its structures.
//!
//! A target's id (`villages`, `bastions`, `end_cities`...) is stable: the
//! finder's queries, the command line and the app's saved settings refer to
//! it, so never rename one.

use super::scan::SetFilter;
use super::{STRUCTURE_SETS, StructureSet, StructureSets};
use crate::world::Dimension;

/// A named group of structures.
#[derive(Debug, PartialEq, Eq)]
pub struct Target {
    /// Stable name (see the module docs).
    pub id: &'static str,
    pub dimension: Dimension,
    /// Structure set registry name.
    pub set: &'static str,
    /// Only the structures of the set whose name starts with this (the
    /// Nether's fortresses and bastions share one set).
    pub only: Option<&'static str>,
}

impl Target {
    /// The target with this id.
    pub fn find(id: &str) -> Option<&'static Target> {
        TARGETS.iter().find(|t| t.id == id)
    }

    pub fn set(&self) -> &'static StructureSet {
        STRUCTURE_SETS
            .find_set(self.set)
            .expect("every target's set exists (tested)")
    }

    /// End cities, which can be filtered on their ship (an elytra).
    pub fn has_ships(&self) -> bool {
        self.set == "minecraft:end_cities"
    }

    /// The structures that count; `ship_only` keeps end cities with a ship.
    pub fn select(&self, ship_only: bool) -> SetFilter {
        SetFilter {
            set: self.set(),
            only: self.only,
            ship_only: ship_only && self.has_ships(),
        }
    }
}

const fn target(id: &'static str, dimension: Dimension, set: &'static str) -> Target {
    Target {
        id,
        dimension,
        set,
        only: None,
    }
}

use Dimension::{End, Nether, Overworld};

/// Every target, Overworld first. Strongholds are not one: their rings
/// are placed by a biome search, not tried chunk by chunk.
pub static TARGETS: &[Target] = &[
    target("villages", Overworld, "minecraft:villages"),
    target(
        "woodland_mansions",
        Overworld,
        "minecraft:woodland_mansions",
    ),
    target("ocean_monuments", Overworld, "minecraft:ocean_monuments"),
    target(
        "pillager_outposts",
        Overworld,
        "minecraft:pillager_outposts",
    ),
    target("desert_pyramids", Overworld, "minecraft:desert_pyramids"),
    target("jungle_temples", Overworld, "minecraft:jungle_temples"),
    target("swamp_huts", Overworld, "minecraft:swamp_huts"),
    target("igloos", Overworld, "minecraft:igloos"),
    target("ancient_cities", Overworld, "minecraft:ancient_cities"),
    target("trial_chambers", Overworld, "minecraft:trial_chambers"),
    target("trail_ruins", Overworld, "minecraft:trail_ruins"),
    target("abandoned_camps", Overworld, "minecraft:abandoned_camp"),
    target("ruined_portals", Overworld, "minecraft:ruined_portals"),
    target("shipwrecks", Overworld, "minecraft:shipwrecks"),
    target("ocean_ruins", Overworld, "minecraft:ocean_ruins"),
    target("buried_treasures", Overworld, "minecraft:buried_treasures"),
    target("mineshafts", Overworld, "minecraft:mineshafts"),
    Target {
        only: Some("minecraft:fortress"),
        ..target("fortresses", Nether, "minecraft:nether_complexes")
    },
    Target {
        only: Some("minecraft:bastion"),
        ..target("bastions", Nether, "minecraft:nether_complexes")
    },
    target("nether_fossils", Nether, "minecraft:nether_fossils"),
    target("nether_ruined_portals", Nether, "minecraft:ruined_portals"),
    target("end_cities", End, "minecraft:end_cities"),
];
