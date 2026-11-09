//! Structure layers: where each overworld structure set attempts to generate,
//! and stronghold positions.
//!
//! Biome and terrain checks are not implemented yet, so a random-spread
//! position is a *candidate*: the game may still skip it. Strongholds are
//! exact.

use std::sync::mpsc;
use std::thread;

use eframe::egui::{self, Color32};
use mc_core::biome::OverworldBiomeSource;
use mc_core::structure::stronghold::ring_positions;
use mc_core::structure::{STRUCTURE_SETS, StructurePlacement, StructureSet, StructureSets};

/// One structure layer of the map.
pub struct Kind {
    /// Structure set registry name.
    pub set: &'static str,
    pub label: &'static str,
    /// Up to two letters drawn on the marker.
    pub glyph: &'static str,
    pub color: Color32,
    /// Shown by default.
    pub default_on: bool,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const STRONGHOLDS: &str = "minecraft:strongholds";

/// Overworld structure layers, in legend order.
pub static KINDS: &[Kind] = &[
    Kind {
        set: "minecraft:villages",
        label: "Villages",
        glyph: "V",
        color: rgb(0xe0a458),
        default_on: true,
    },
    Kind {
        set: STRONGHOLDS,
        label: "Strongholds",
        glyph: "S",
        color: rgb(0xb67cf2),
        default_on: true,
    },
    Kind {
        set: "minecraft:woodland_mansions",
        label: "Woodland mansions",
        glyph: "WM",
        color: rgb(0x9a6b3f),
        default_on: true,
    },
    Kind {
        set: "minecraft:ocean_monuments",
        label: "Ocean monuments",
        glyph: "OM",
        color: rgb(0x4fd1c5),
        default_on: true,
    },
    Kind {
        set: "minecraft:pillager_outposts",
        label: "Pillager outposts",
        glyph: "PO",
        color: rgb(0x9aa0a6),
        default_on: true,
    },
    Kind {
        set: "minecraft:desert_pyramids",
        label: "Desert pyramids",
        glyph: "DP",
        color: rgb(0xf2d16b),
        default_on: false,
    },
    Kind {
        set: "minecraft:jungle_temples",
        label: "Jungle temples",
        glyph: "JT",
        color: rgb(0x7bc96f),
        default_on: false,
    },
    Kind {
        set: "minecraft:swamp_huts",
        label: "Swamp huts",
        glyph: "SH",
        color: rgb(0x6f8f5f),
        default_on: false,
    },
    Kind {
        set: "minecraft:igloos",
        label: "Igloos",
        glyph: "IG",
        color: rgb(0xcfe8ff),
        default_on: false,
    },
    Kind {
        set: "minecraft:ancient_cities",
        label: "Ancient cities",
        glyph: "AC",
        color: rgb(0x3aa7a0),
        default_on: false,
    },
    Kind {
        set: "minecraft:trial_chambers",
        label: "Trial chambers",
        glyph: "TC",
        color: rgb(0xd9824a),
        default_on: false,
    },
    Kind {
        set: "minecraft:trail_ruins",
        label: "Trail ruins",
        glyph: "TR",
        color: rgb(0xc98b6b),
        default_on: false,
    },
    Kind {
        set: "minecraft:abandoned_camp",
        label: "Abandoned camps",
        glyph: "AB",
        color: rgb(0xb5a36a),
        default_on: false,
    },
    Kind {
        set: "minecraft:ruined_portals",
        label: "Ruined portals",
        glyph: "RP",
        color: rgb(0xa77bd9),
        default_on: false,
    },
    Kind {
        set: "minecraft:shipwrecks",
        label: "Shipwrecks",
        glyph: "SW",
        color: rgb(0x8c6d4f),
        default_on: false,
    },
    Kind {
        set: "minecraft:ocean_ruins",
        label: "Ocean ruins",
        glyph: "OR",
        color: rgb(0x5d8fb3),
        default_on: false,
    },
    Kind {
        set: "minecraft:buried_treasures",
        label: "Buried treasure",
        glyph: "BT",
        color: rgb(0xf5c542),
        default_on: false,
    },
    Kind {
        set: "minecraft:mineshafts",
        label: "Mineshafts",
        glyph: "M",
        color: rgb(0x8f7a66),
        default_on: false,
    },
];

/// A structure marker on the map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub kind: usize,
    pub x: i32,
    pub z: i32,
    /// `false` for candidates whose biome and terrain are not checked yet.
    pub exact: bool,
}

pub fn set(name: &str) -> Option<&'static StructureSet> {
    STRUCTURE_SETS.find_set(name)
}

/// Expected distance between two attempts of a random-spread set, in
/// blocks (`None` for strongholds).
pub fn spacing_blocks(kind: usize) -> Option<f64> {
    let StructurePlacement::RandomSpread(p) = &set(KINDS[kind].set)?.placement else {
        return None;
    };
    Some(p.spacing as f64 * 16.0 / f64::from(p.frequency).sqrt())
}

/// Candidate positions of `kind` whose chunk lies in `cx0..=cx1`,
/// `cz0..=cz1`.
pub fn candidates(
    kind: usize,
    seed: i64,
    (cx0, cz0, cx1, cz1): (i32, i32, i32, i32),
) -> Vec<Marker> {
    let mut out = Vec::new();
    let Some(set) = set(KINDS[kind].set) else {
        return out;
    };
    let StructurePlacement::RandomSpread(p) = &set.placement else {
        return out;
    };
    let mut push = |cx: i32, cz: i32| {
        if set.is_structure_chunk(&STRUCTURE_SETS[..], seed, cx, cz) == Some(true) {
            let (x, _, z) = p.locate_pos(cx, cz);
            out.push(Marker {
                kind,
                x,
                z,
                exact: false,
            });
        }
    };
    if p.spacing == 1 {
        for cz in cz0..=cz1 {
            for cx in cx0..=cx1 {
                push(cx, cz);
            }
        }
        return out;
    }
    let s = p.spacing;
    for rz in cz0.div_euclid(s)..=cz1.div_euclid(s) {
        for rx in cx0.div_euclid(s)..=cx1.div_euclid(s) {
            let (cx, cz) = p.region_chunk(seed, rx, rz);
            if (cx0..=cx1).contains(&cx) && (cz0..=cz1).contains(&cz) {
                push(cx, cz);
            }
        }
    }
    out
}

/// Stronghold positions, computed on a background thread (they need a
/// biome search around each of the 128 ring positions).
pub struct Strongholds {
    rx: Option<mpsc::Receiver<Vec<(i32, i32)>>>,
    positions: Vec<(i32, i32)>,
}

impl Strongholds {
    pub fn new(seed: i64, ctx: &egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        thread::Builder::new()
            .name("strongholds".into())
            .spawn(move || {
                let positions = set(STRONGHOLDS)
                    .and_then(|s| s.concentric_rings())
                    .map(|rings| ring_positions(seed, &rings, &OverworldBiomeSource::new(seed)))
                    .unwrap_or_default();
                let _ = tx.send(positions);
                ctx.request_repaint();
            })
            .expect("spawn stronghold thread");
        Self {
            rx: Some(rx),
            positions: Vec::new(),
        }
    }

    pub fn is_loading(&mut self) -> bool {
        if let Some(rx) = &self.rx
            && let Ok(p) = rx.try_recv()
        {
            self.positions = p;
            self.rx = None;
        }
        self.rx.is_some()
    }

    /// Block positions, as `/locate` reports them (the chunk's corner).
    pub fn positions(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.positions.iter().map(|&(cx, cz)| (cx << 4, cz << 4))
    }
}
