//! What the app remembers between runs: the seed, the view, the layers that
//! are shown and the finder's last query.
//!
//! Saved through eframe's storage (a RON file in the platform's app data
//! folder). Every field has a default, so a file from an older or newer
//! version still loads: missing fields take their default and unknown ones
//! are ignored. Layers are saved by their stable id (`Kind::id`), never by
//! their position in the list, so adding or reordering layers keeps them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::dimension::Dimension;
use crate::structures::{KINDS, kind_index};

/// The key under which the settings are stored.
pub const KEY: &str = "chunkscope-settings";
/// Bumped when a field changes meaning (new fields need no bump).
pub const VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub seed_text: String,
    /// `overworld`, `nether` or `end`.
    pub dimension: String,
    /// Map center in blocks.
    pub center: (f64, f64),
    /// Screen points per block.
    pub zoom: f32,
    pub y: i32,
    pub show_biomes: bool,
    pub show_slime: bool,
    pub show_grid: bool,
    /// Layer id → shown. Layers missing here take their default.
    pub layers: BTreeMap<String, bool>,
    pub finder: FinderSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            seed_text: "Chunkscope".to_owned(),
            dimension: Dimension::Overworld.id().to_owned(),
            center: (0.0, 0.0),
            zoom: 0.25,
            y: 64,
            show_biomes: true,
            show_slime: false,
            show_grid: true,
            layers: BTreeMap::new(),
            finder: FinderSettings::default(),
        }
    }
}

impl Settings {
    /// Whether each layer of [`KINDS`] is shown.
    pub fn layers_on(&self) -> Vec<bool> {
        KINDS
            .iter()
            .map(|k| self.layers.get(k.id).copied().unwrap_or(k.default_on))
            .collect()
    }

    /// Record which layers of [`KINDS`] are shown.
    pub fn set_layers_on(&mut self, on: &[bool]) {
        self.layers = KINDS
            .iter()
            .zip(on)
            .map(|(k, &on)| (k.id.to_owned(), on))
            .collect();
    }

    pub fn dimension(&self) -> Dimension {
        Dimension::from_id(&self.dimension).unwrap_or(Dimension::Overworld)
    }
}

/// The seed finder's query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FinderSettings {
    /// Measure from the world spawn (else from 0, 0).
    pub center_spawn: bool,
    pub filters: Vec<FilterSettings>,
    /// The biome at the center, e.g. `minecraft:plains`.
    pub biome: Option<String>,
    /// First seed, or the salt of random seeds.
    pub from: String,
    pub count: u64,
    pub random: bool,
}

impl Default for FinderSettings {
    fn default() -> Self {
        Self {
            center_spawn: true,
            filters: vec![FilterSettings::default()],
            biome: None,
            from: "0".to_owned(),
            count: 100_000,
            random: false,
        }
    }
}

/// One structure filter of the finder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FilterSettings {
    /// The layer's id (`Kind::id`).
    pub layer: String,
    pub radius: i32,
    pub count: u32,
    /// End cities: only those with a ship.
    pub ship: bool,
}

impl Default for FilterSettings {
    fn default() -> Self {
        Self {
            layer: "villages".to_owned(),
            radius: 300,
            count: 1,
            ship: false,
        }
    }
}

impl FilterSettings {
    /// The layer this filter searches, if it still exists and is searchable.
    pub fn kind(&self) -> Option<usize> {
        kind_index(&self.layer).filter(|&i| KINDS[i].searchable())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(s: &Settings) -> Settings {
        ron::from_str(&ron::to_string(s).unwrap()).unwrap()
    }

    #[test]
    fn settings_round_trip() {
        let mut s = Settings {
            seed_text: "copy, paste: 123".to_owned(),
            dimension: "nether".to_owned(),
            center: (-1234.5, 99.25),
            zoom: 2.0,
            y: -20,
            show_slime: true,
            ..Settings::default()
        };
        let mut on = s.layers_on();
        on[0] = !on[0];
        s.set_layers_on(&on);
        s.finder.filters.push(FilterSettings {
            layer: "end_cities".to_owned(),
            radius: 1000,
            count: 2,
            ship: true,
        });
        s.finder.biome = Some("minecraft:plains".to_owned());
        assert_eq!(round_trip(&s), s);
        assert_eq!(round_trip(&s).layers_on(), on);
        assert_eq!(s.dimension(), Dimension::Nether);
    }

    #[test]
    fn layers_are_kept_by_id() {
        let mut s = Settings::default();
        let mut on = s.layers_on();
        let mineshafts = kind_index("mineshafts").unwrap();
        on[mineshafts] = !KINDS[mineshafts].default_on;
        s.set_layers_on(&on);
        // A layer added later is missing from old settings: it takes its
        // default. A layer that no longer exists is ignored.
        s.layers.remove("villages");
        s.layers.insert("removed_layer".to_owned(), true);
        let loaded = s.layers_on();
        assert_eq!(loaded[mineshafts], !KINDS[mineshafts].default_on);
        let villages = kind_index("villages").unwrap();
        assert_eq!(loaded[villages], KINDS[villages].default_on);
        assert_eq!(loaded.len(), KINDS.len());
    }

    #[test]
    fn old_and_partial_files_load() {
        // Only some fields, plus one this version does not know.
        let s: Settings =
            ron::from_str(r#"(seed_text: "abc", zoom: 4.0, future_field: 1)"#).unwrap();
        assert_eq!(s.seed_text, "abc");
        assert_eq!(s.zoom, 4.0);
        assert_eq!(s.finder, FinderSettings::default());
        assert_eq!(s.dimension(), Dimension::Overworld);
        let bad: Settings = ron::from_str(r#"(dimension: "the_moon")"#).unwrap();
        assert_eq!(bad.dimension(), Dimension::Overworld);
    }

    #[test]
    fn layer_ids_are_unique() {
        for (i, k) in KINDS.iter().enumerate() {
            assert_eq!(kind_index(k.id), Some(i), "{}", k.id);
        }
    }

    #[test]
    fn filters_need_a_searchable_layer() {
        let filter = |layer: &str| FilterSettings {
            layer: layer.to_owned(),
            ..FilterSettings::default()
        };
        assert!(filter("villages").kind().is_some());
        assert!(filter("spawn").kind().is_none());
        assert!(filter("strongholds").kind().is_none());
        assert!(filter("no_such_layer").kind().is_none());
    }
}
