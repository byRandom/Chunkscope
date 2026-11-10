//! Map colors for overworld biomes, and their display names.

use eframe::egui::Color32;
use mc_core::biome::Biome;

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// The map color of a biome, by registry name.
pub fn color_of(name: &str) -> Color32 {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    rgb(match name {
        // Oceans, from warm to frozen, deep variants darker.
        "warm_ocean" => 0x2a9db8,
        "lukewarm_ocean" => 0x2681b0,
        "deep_lukewarm_ocean" => 0x1d6594,
        "ocean" => 0x22609e,
        "deep_ocean" => 0x1a4a80,
        "cold_ocean" => 0x2e5590,
        "deep_cold_ocean" => 0x243f74,
        "frozen_ocean" => 0x6f8fbf,
        "deep_frozen_ocean" => 0x52709e,
        "river" => 0x3a7fd0,
        "frozen_river" => 0x9db8e0,
        // Shores.
        "beach" => 0xe6d59a,
        "snowy_beach" => 0xeae6d2,
        "stony_shore" => 0x8a8a84,
        // Plains and grasslands.
        "plains" => 0x8db360,
        "sunflower_plains" => 0xb5c45a,
        "meadow" => 0x7fc37a,
        "cherry_grove" => 0xf0a8c8,
        "savanna" => 0xbfb755,
        "savanna_plateau" => 0xa79d4c,
        "windswept_savanna" => 0xcfc678,
        // Forests.
        "forest" => 0x3f8a3a,
        "flower_forest" => 0x6aa34a,
        "birch_forest" => 0x5fa05a,
        "old_growth_birch_forest" => 0x4f9150,
        "dark_forest" => 0x2d5a24,
        "pale_garden" => 0x9aa39a,
        "dappled_forest" => 0x56813a,
        "windswept_forest" => 0x4e7a5c,
        // Taigas.
        "taiga" => 0x3c6e55,
        "old_growth_pine_taiga" => 0x4d6248,
        "old_growth_spruce_taiga" => 0x445a43,
        "snowy_taiga" => 0x5f8578,
        // Jungles and swamps.
        "jungle" => 0x3d8b1a,
        "sparse_jungle" => 0x62a12f,
        "bamboo_jungle" => 0x6aa81e,
        "swamp" => 0x4b6b45,
        "mangrove_swamp" => 0x3f6f3a,
        // Dry biomes.
        "desert" => 0xe3c97c,
        "badlands" => 0xc76a32,
        "wooded_badlands" => 0xa8693a,
        "eroded_badlands" => 0xd9773a,
        // Cold and mountains.
        "snowy_plains" => 0xe8eef2,
        "ice_spikes" => 0xb8dcef,
        "grove" => 0xa3c2b2,
        "snowy_slopes" => 0xd3dde4,
        "frozen_peaks" => 0xbfd6ea,
        "jagged_peaks" => 0xdfe7ee,
        "stony_peaks" => 0x9c9c94,
        "windswept_hills" => 0x7d8f7b,
        "windswept_gravelly_hills" => 0x8a928a,
        // Special and underground.
        "mushroom_fields" => 0xa47fb8,
        "dripstone_caves" => 0x8a6a4c,
        "lush_caves" => 0x5aa34a,
        "deep_dark" => 0x14303a,
        "sulfur_caves" => 0xbfae3a,
        // Nether.
        "nether_wastes" => 0x8a3a32,
        "crimson_forest" => 0xc0283a,
        "warped_forest" => 0x2a8f86,
        "soul_sand_valley" => 0x6b5a4a,
        "basalt_deltas" => 0x5c5a5e,
        // The End: end stone for the islands, darker toward the void.
        "the_end" => 0xd8d29a,
        "end_highlands" => 0xbcb47e,
        "end_midlands" => 0x9a9468,
        "end_barrens" => 0x5e5a6e,
        "small_end_islands" => 0x262333,
        _ => 0xff00ff,
    })
}

/// "minecraft:old_growth_pine_taiga" -> "Old Growth Pine Taiga".
pub fn display_name(name: &str) -> String {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    name.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Colors of every biome, indexed by [`Biome`] id.
pub fn palette() -> Vec<Color32> {
    Biome::all().map(|b| color_of(b.name())).collect()
}
