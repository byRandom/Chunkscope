//! The main window: title bar, tool rail, world panel, map canvas,
//! inspector and status bar.

use std::hash::{BuildHasher, RandomState};

use crate::dimension::{BiomeSource, Dimension};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, UiBuilder, Vec2, pos2, vec2,
};
use mc_core::biome::Biome;
use mc_core::{is_slime_chunk, parse_seed};

use crate::biome_colors;
use crate::icons::{self, Icon};
use crate::structures::{
    self, BLOCK_CHUNKS, BlockKey, KINDS, Marker, SPAWN, STRONGHOLDS, Spawn, Strongholds,
    StructureCache,
};
use crate::theme::{
    self, ACCENT, ACCENT_DIM, BG, BORDER, FIELD, ON_ACCENT, PANEL, TEXT, TEXT_WEAK,
};
use crate::tiles::{MAX_LEVEL, TileCache, TileKey};

/// Version the generation logic matches.
const GAME_VERSION: &str = "Java 26.3";
/// Zoom limits, in screen points per block.
const MIN_ZOOM: f32 = 1.0 / 32.0;
const MAX_ZOOM: f32 = 16.0;
/// Structure markers closer than this on screen are not drawn.
const MARKER_SPACING_PX: f64 = 48.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    World,
    Finder,
    About,
}

pub struct ChunkscopeApp {
    tab: Tab,
    seed_text: String,
    seed: i64,
    y: i32,
    /// Map center in blocks.
    center: (f64, f64),
    /// Screen points per block.
    zoom: f32,

    show_biomes: bool,
    show_slime: bool,
    show_grid: bool,
    kinds_on: Vec<bool>,

    tiles: TileCache,
    strongholds: Strongholds,
    spawn: Spawn,
    finder: crate::finder_ui::FinderUi,
    markers: StructureCache,
    dimension: Dimension,
    source: BiomeSource,

    hover: Option<(i64, i64)>,
    hover_biome: Option<((i32, i32, i32), Biome)>,
    hover_marker: Option<Marker>,
    selected: Option<Marker>,
    goto: (String, String),
    /// Block position the map's context menu was opened at.
    menu_at: Option<(i64, i64)>,
    toast: Option<(String, f64)>,
}

impl ChunkscopeApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let seed_text = "Chunkscope".to_owned();
        let seed = parse_seed(&seed_text).unwrap_or(0);
        Self {
            tab: Tab::World,
            seed_text,
            seed,
            y: 64,
            center: (0.0, 0.0),
            zoom: 0.25,
            show_biomes: true,
            show_slime: false,
            show_grid: true,
            kinds_on: KINDS.iter().map(|k| k.default_on).collect(),
            tiles: TileCache::new(&cc.egui_ctx),
            strongholds: Strongholds::new(seed, &cc.egui_ctx),
            spawn: Spawn::new(seed, &cc.egui_ctx),
            finder: Default::default(),
            markers: StructureCache::new(&cc.egui_ctx),
            dimension: Dimension::Overworld,
            source: BiomeSource::new(Dimension::Overworld, seed),
            hover: None,
            hover_biome: None,
            hover_marker: None,
            selected: None,
            goto: (String::new(), String::new()),
            toast: None,
            menu_at: None,
        }
    }

    fn set_seed(&mut self, seed: i64, ctx: &egui::Context) {
        if seed == self.seed {
            return;
        }
        self.seed = seed;
        self.strongholds = Strongholds::new(seed, ctx);
        self.spawn = Spawn::new(seed, ctx);
        self.source = BiomeSource::new(self.dimension, seed);
        self.hover_biome = None;
        self.selected = None;
    }

    fn set_dimension(&mut self, dimension: Dimension) {
        if dimension == self.dimension {
            return;
        }
        self.dimension = dimension;
        self.source = BiomeSource::new(dimension, self.seed);
        self.hover_biome = None;
        self.selected = None;
    }

    fn notify(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let now = ctx.input(|i| i.time);
        self.toast = Some((text.into(), now + 2.0));
    }
}

impl eframe::App for ChunkscopeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.tiles.receive(&ctx);
        self.markers.receive();
        self.strongholds.is_loading();
        self.spawn.is_loading();

        egui::Panel::top("title")
            .exact_size(40.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| self.title_bar(ui));
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(12, 0)),
            )
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("rail")
            .exact_size(56.0)
            .resizable(false)
            .frame(theme::panel_frame())
            .show(ui, |ui| self.rail(ui));
        egui::Panel::left("world")
            .exact_size(264.0)
            .resizable(false)
            .frame(theme::panel_frame())
            .show(ui, |ui| match self.tab {
                Tab::World => self.world_panel(ui),
                Tab::Finder => {
                    if let Some(crate::finder_ui::Action::Open { seed, center }) =
                        self.finder.ui(ui)
                    {
                        let ctx = ui.ctx().clone();
                        self.seed_text = seed.to_string();
                        self.set_seed(seed, &ctx);
                        self.center = (center.0 as f64, center.1 as f64);
                        self.zoom = self.zoom.max(0.25);
                    }
                }
                Tab::About => about_panel(ui),
            });
        egui::Panel::right("inspector")
            .exact_size(272.0)
            .resizable(false)
            .frame(theme::panel_frame())
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ui, |ui| self.map(ui));
    }
}

// ---------------------------------------------------------------- chrome

impl ChunkscopeApp {
    fn title_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            // App mark: a mint tile with a 2x2 chunk grid.
            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            ui.painter().rect_filled(r, 6.0, ACCENT);
            let c = r.center();
            for (dx, dy) in [(-3.5, -3.5), (3.5, -3.5), (-3.5, 3.5), (3.5, 3.5)] {
                let cell = Rect::from_center_size(c + vec2(dx, dy), vec2(5.5, 5.5));
                ui.painter().rect_filled(cell, 1.5, ON_ACCENT);
            }
            ui.add_space(4.0);
            ui.label(RichText::new("Chunkscope").strong().color(TEXT));
            ui.add_space(14.0);

            // The open "document": the current seed.
            let tab = egui::Frame::new()
                .fill(PANEL)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(10, 5));
            tab.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::paint(ui.painter(), r, Icon::Map, ACCENT);
                    ui.label(RichText::new(format!("Seed {}", self.seed)).color(TEXT));
                    ui.label(
                        RichText::new(format!("· {}", self.dimension.label())).color(TEXT_WEAK),
                    );
                });
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                pill(ui, GAME_VERSION);
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            let hint = |ui: &mut egui::Ui, key: &str, what: &str| {
                ui.label(RichText::new(key).strong().color(TEXT).small());
                ui.label(RichText::new(what).color(TEXT_WEAK).small());
                ui.add_space(6.0);
            };
            hint(ui, "Drag", "to pan");
            hint(ui, "Scroll", "to zoom");
            hint(ui, "Click", "a marker to select it");
            hint(ui, "Right-click", "to copy a position");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let pending = self.tiles.pending() + self.markers.pending();
                let text = format!(
                    "{} tiles · {} pending · {:.0} ms per tile · {} threads",
                    self.tiles.loaded(),
                    pending,
                    self.tiles.tile_ms(),
                    self.tiles.workers()
                );
                ui.label(RichText::new(text).color(TEXT_WEAK).small());
                if pending > 0 || self.strongholds_loading() {
                    ui.add(egui::Spinner::new().size(11.0).color(ACCENT));
                }
            });
        });
    }

    fn strongholds_loading(&mut self) -> bool {
        self.strongholds.is_loading()
    }

    fn rail(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            for (tab, icon, label) in [
                (Tab::World, Icon::Map, "Map"),
                (Tab::Finder, Icon::Search, "Finder"),
                (Tab::About, Icon::Info, "About"),
            ] {
                let active = self.tab == tab;
                if rail_button(ui, icon, label, active, true).clicked() {
                    self.tab = tab;
                }
                ui.add_space(6.0);
            }
        });
    }

    fn world_panel(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        panel_header(ui, "World", Some(GAME_VERSION));
        section(ui, |ui| {
            theme::field_label(ui, "Seed");
            ui.horizontal(|ui| {
                let edit = egui::TextEdit::singleline(&mut self.seed_text)
                    .desired_width(ui.available_width() - 72.0)
                    .hint_text("Number or text");
                if ui.add(edit).changed()
                    && let Some(seed) = parse_seed(&self.seed_text)
                {
                    self.set_seed(seed, &ctx);
                }
                if icon_button(ui, Icon::Copy, false)
                    .on_hover_text("Copy the numeric seed")
                    .clicked()
                {
                    ctx.copy_text(self.seed.to_string());
                    self.notify(&ctx, format!("Copied seed {}", self.seed));
                }
                if icon_button(ui, Icon::Dice, false)
                    .on_hover_text("Random seed")
                    .clicked()
                {
                    let seed = RandomState::new().hash_one(ctx.input(|i| i.time).to_bits()) as i64;
                    self.seed_text = seed.to_string();
                    self.set_seed(seed, &ctx);
                }
            });
            let numeric = self.seed_text.trim().parse::<i64>().ok() == Some(self.seed);
            if !numeric && !self.seed_text.is_empty() {
                fit_label(
                    ui,
                    RichText::new(format!("Text seed, hashes to {}", self.seed))
                        .color(TEXT_WEAK)
                        .small(),
                );
            }
            ui.add_space(4.0);
            ui.columns(2, |cols| {
                theme::field_label(&mut cols[0], "Dimension");
                let mut dimension = self.dimension;
                egui::ComboBox::from_id_salt("dimension")
                    .width(cols[0].available_width())
                    .selected_text(dimension.label())
                    .show_ui(&mut cols[0], |ui| {
                        for d in Dimension::ALL {
                            ui.selectable_value(&mut dimension, d, d.label());
                        }
                    });
                self.set_dimension(dimension);
                theme::field_label(&mut cols[1], "Biome Y");
                cols[1].add(
                    egui::DragValue::new(&mut self.y)
                        .range(-64..=319)
                        .speed(1.0),
                );
            });
        });
        theme::divider(ui);

        egui::ScrollArea::vertical().show(ui, |ui| {
            section(ui, |ui| {
                theme::section_title(ui, "Layers");
                ui.add_space(2.0);
                layer_row(ui, &mut self.show_biomes, Icon::Map, None, "Biomes", None);
                if self.dimension == Dimension::Overworld {
                    layer_row(
                        ui,
                        &mut self.show_slime,
                        Icon::Slime,
                        None,
                        "Slime chunks",
                        None,
                    );
                }
                layer_row(ui, &mut self.show_grid, Icon::Grid, None, "Grid", None);
            });
            theme::divider(ui);
            section(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::section_title(ui, "Structures");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dimension = self.dimension;
                        let here = |i: &usize| KINDS[*i].dimension == dimension;
                        let all = (0..KINDS.len()).filter(here).all(|i| self.kinds_on[i]);
                        let text = if all { "Hide all" } else { "Show all" };
                        if ui.small_button(text).clicked() {
                            for i in (0..KINDS.len()).filter(here) {
                                self.kinds_on[i] = !all;
                            }
                        }
                    });
                });
                ui.add_space(2.0);
                for (i, kind) in KINDS.iter().enumerate() {
                    if kind.dimension != self.dimension {
                        continue;
                    }
                    // A layer too dense for this zoom is not drawn: say so, and
                    // from which zoom it shows.
                    let min_zoom = structures::spacing_blocks(i).map(|s| MARKER_SPACING_PX / s);
                    let note = if kind.set == SPAWN {
                        Some("estimate".to_owned())
                    } else if let Some(z) = min_zoom
                        && self.kinds_on[i]
                        && (self.zoom as f64) < z
                    {
                        Some(format!("zoom in ({:.0}%+)", (z * 100.0).ceil()))
                    } else {
                        None
                    };
                    layer_row(
                        ui,
                        &mut self.kinds_on[i],
                        Icon::Pin,
                        Some((kind.glyph, kind.color)),
                        kind.label,
                        note.as_deref(),
                    );
                }
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "Structures are checked against biome and terrain like the game \
                         does. The spawn is an estimate: trees and lakes can move it.",
                    )
                    .color(TEXT_WEAK)
                    .small(),
                );
                if self.dimension == Dimension::End {
                    ui.label(
                        RichText::new("A green dot marks end cities with a ship (an elytra).")
                            .color(TEXT_WEAK)
                            .small(),
                    );
                }
            });
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        panel_header(ui, "Inspector", None);
        section(ui, |ui| {
            theme::section_title(ui, "Cursor");
            ui.add_space(2.0);
            match self.hover {
                Some((x, z)) => {
                    xz_row(ui, ("X", &x.to_string()), ("Z", &z.to_string()));
                    labelled_box(ui, "Chunk", &format!("{}, {}", x >> 4, z >> 4));
                    labelled_box(ui, "Region", &format!("{}, {}", x >> 9, z >> 9));
                    if let Some((_, biome)) = self.hover_biome {
                        theme::field_label(ui, "Biome");
                        let color = biome_colors::color_of(biome.name());
                        swatch_box(ui, color, &biome_colors::display_name(biome.name()));
                    }
                    if self.dimension == Dimension::Overworld {
                        let slime = is_slime_chunk(self.seed, (x >> 4) as i32, (z >> 4) as i32);
                        theme::field_label(ui, "Slime chunk");
                        field_box(ui, if slime { "Yes" } else { "No" });
                    }
                }
                None => {
                    ui.label(RichText::new("Hover the map").color(TEXT_WEAK));
                }
            }
        });
        theme::divider(ui);
        section(ui, |ui| {
            theme::section_title(ui, "Selection");
            ui.add_space(2.0);
            match self.selected {
                Some(m) => {
                    let kind = &KINDS[m.kind];
                    ui.horizontal(|ui| {
                        marker_badge(
                            ui.painter(),
                            ui.cursor().min + vec2(10.0, 10.0),
                            kind.glyph,
                            kind.color,
                            10.0,
                        );
                        ui.add_space(26.0);
                        ui.label(RichText::new(singular(kind.label)).color(TEXT).strong());
                    });
                    xz_row(ui, ("X", &m.x.to_string()), ("Z", &m.z.to_string()));
                    if kind.set == SPAWN {
                        if let Some(((ox, oz), (sx, sy, sz))) = self.spawn.get() {
                            theme::field_label(ui, "Estimated spawn block");
                            field_box(ui, &format!("{sx}, {sy}, {sz}"));
                            theme::field_label(ui, "Searched around (exact)");
                            field_box(ui, &format!("{ox}, {oz}  (chunk {}, {})", ox >> 4, oz >> 4));
                        }
                        theme::field_label(ui, "Status");
                        field_box(ui, "Estimate: trees and lakes can move it");
                    } else {
                        if let Some((name, sx, sy, sz)) = m.start {
                            theme::field_label(ui, "Structure");
                            field_box(ui, &biome_colors::display_name(name));
                            theme::field_label(ui, "Start");
                            field_box(ui, &format!("{sx}, {sy}, {sz}"));
                        }
                        if let Some(ship) = m.ship {
                            theme::field_label(ui, "Ship");
                            field_box(ui, if ship { "Yes (elytra)" } else { "No" });
                        }
                        theme::field_label(ui, "Status");
                        field_box(ui, "Generates (checked like the game)");
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if accent_button(ui, "Copy /tp").clicked() {
                            ctx.copy_text(format!("/tp @s {} ~ {}", m.x, m.z));
                            self.notify(&ctx, "Copied teleport command");
                        }
                        if ui.button("Center").clicked() {
                            self.center = (m.x as f64, m.z as f64);
                        }
                    });
                }
                None => {
                    ui.label(RichText::new("Click a marker on the map").color(TEXT_WEAK));
                }
            }
        });
        theme::divider(ui);
        section(ui, |ui| {
            theme::section_title(ui, "Go to");
            ui.add_space(2.0);
            ui.columns(2, |cols| {
                theme::field_label(&mut cols[0], "X");
                cols[0].add(egui::TextEdit::singleline(&mut self.goto.0).hint_text("0"));
                theme::field_label(&mut cols[1], "Z");
                cols[1].add(egui::TextEdit::singleline(&mut self.goto.1).hint_text("0"));
            });
            if ui.button("Go").clicked() {
                let x = self.goto.0.trim().parse::<f64>().unwrap_or(0.0);
                let z = self.goto.1.trim().parse::<f64>().unwrap_or(0.0);
                self.center = (x, z);
            }
        });
        theme::divider(ui);
        section(ui, |ui| {
            theme::section_title(ui, "Biome colors");
            ui.add_space(2.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                for biome in self.dimension.biomes() {
                    let color = biome_colors::color_of(biome.name());
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
                        ui.painter().rect_filled(r, 3.0, color);
                        ui.label(
                            RichText::new(biome_colors::display_name(biome.name()))
                                .color(TEXT_WEAK)
                                .small(),
                        );
                    });
                }
            });
        });
    }
}

// ---------------------------------------------------------------- map

impl ChunkscopeApp {
    fn to_screen(&self, rect: Rect, x: f64, z: f64) -> Pos2 {
        let c = rect.center();
        pos2(
            c.x + ((x - self.center.0) * self.zoom as f64) as f32,
            c.y + ((z - self.center.1) * self.zoom as f64) as f32,
        )
    }

    fn to_world(&self, rect: Rect, p: Pos2) -> (f64, f64) {
        let c = rect.center();
        (
            self.center.0 + ((p.x - c.x) / self.zoom) as f64,
            self.center.1 + ((p.y - c.y) / self.zoom) as f64,
        )
    }

    fn zoom_around(&mut self, rect: Rect, anchor: Pos2, factor: f32) {
        let before = self.to_world(rect, anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.to_world(rect, anchor);
        self.center.0 += before.0 - after.0;
        self.center.1 += before.1 - after.1;
    }

    fn map(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        let response = ui.allocate_rect(rect, Sense::click_and_drag());

        // Input: drag to pan, scroll or pinch to zoom around the pointer.
        if response.dragged() {
            let d = response.drag_delta();
            self.center.0 -= (d.x / self.zoom) as f64;
            self.center.1 -= (d.y / self.zoom) as f64;
        }
        if response.hovered() {
            let (scroll, pinch, pointer) = ui.input(|i| {
                (
                    i.smooth_scroll_delta.y,
                    i.zoom_delta(),
                    i.pointer.hover_pos(),
                )
            });
            let factor = (scroll * 0.0025).exp() * pinch;
            if factor != 1.0
                && let Some(p) = pointer
            {
                self.zoom_around(rect, p, factor);
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, BG);
        if self.show_biomes {
            self.draw_tiles(ui, rect);
        } else {
            self.tiles.request(&[]);
        }
        if self.show_slime && self.dimension == Dimension::Overworld {
            self.draw_slime(&painter, rect);
        }
        if self.show_grid {
            self.draw_grid(&painter, rect);
        }
        self.draw_markers(ui, rect, &response);

        // Hover readout. It keeps the last position when the pointer leaves
        // the map, so the values can be selected and copied in the inspector.
        if let Some(p) = response.hover_pos() {
            let (x, z) = self.to_world(rect, p);
            self.hover = Some((x.floor() as i64, z.floor() as i64));
        }
        self.context_menu(&response, rect);
        if let Some((x, z)) = self.hover {
            let quart = ((x >> 2) as i32, self.y >> 2, (z >> 2) as i32);
            if self.hover_biome.is_none_or(|(q, _)| q != quart) {
                let b = self.source.biome(quart.0, quart.1, quart.2);
                self.hover_biome = Some((quart, b));
            }
        }

        self.floating_toolbar(ui, rect);
        self.draw_toast(ui, rect);
    }

    /// Right click on the map: copy the position or a teleport command.
    fn context_menu(&mut self, response: &egui::Response, rect: Rect) {
        if response.secondary_clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let (x, z) = self.to_world(rect, p);
            self.menu_at = Some((x.floor() as i64, z.floor() as i64));
        }
        let Some((x, z)) = self.menu_at else {
            return;
        };
        let ctx = response.ctx.clone();
        response.context_menu(|ui| {
            ui.label(RichText::new(format!("{x}, {z}")).color(TEXT_WEAK).small());
            if ui.button("Copy coordinates").clicked() {
                ctx.copy_text(format!("{x} {z}"));
                self.notify(&ctx, "Copied coordinates");
                ui.close();
            }
            if ui.button("Copy /tp command").clicked() {
                ctx.copy_text(format!("/tp @s {x} ~ {z}"));
                self.notify(&ctx, "Copied teleport command");
                ui.close();
            }
            if ui.button("Center here").clicked() {
                self.center = (x as f64 + 0.5, z as f64 + 0.5);
                ui.close();
            }
        });
    }

    fn level(&self, ppp: f32) -> u8 {
        let blocks_per_pixel = 1.0 / (self.zoom * ppp);
        let level = (blocks_per_pixel / 4.0).log2().floor();
        level.clamp(0.0, MAX_LEVEL as f32) as u8
    }

    fn draw_tiles(&mut self, ui: &egui::Ui, rect: Rect) {
        let level = self.level(ui.ctx().pixels_per_point());
        let tb = TileKey::tile_blocks(level) as f64;
        let (x0, z0) = self.to_world(rect, rect.min);
        let (x1, z1) = self.to_world(rect, rect.max);
        let (tx0, tz0) = ((x0 / tb).floor() as i32, (z0 / tb).floor() as i32);
        let (tx1, tz1) = ((x1 / tb).floor() as i32, (z1 / tb).floor() as i32);
        let key = |level: u8, tx: i32, tz: i32| TileKey {
            seed: self.seed,
            dimension: self.dimension,
            y: self.y,
            level,
            tx,
            tz,
        };

        // Request missing tiles, nearest to the center first.
        let (ctx_, ctz) = (self.center.0 / tb, self.center.1 / tb);
        let mut wanted: Vec<TileKey> = (tz0..=tz1)
            .flat_map(|tz| (tx0..=tx1).map(move |tx| (tx, tz)))
            .map(|(tx, tz)| key(level, tx, tz))
            .collect();
        wanted.sort_by(|a, b| {
            let d = |k: &TileKey| {
                (k.tx as f64 + 0.5 - ctx_).powi(2) + (k.tz as f64 + 0.5 - ctz).powi(2)
            };
            d(a).total_cmp(&d(b))
        });
        self.tiles.request(&wanted);

        let painter = ui.painter_at(rect);
        let full_uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        for k in wanted {
            let min = self.to_screen(rect, k.tx as f64 * tb, k.tz as f64 * tb);
            let max = self.to_screen(rect, (k.tx + 1) as f64 * tb, (k.tz + 1) as f64 * tb);
            let screen = Rect::from_min_max(min, max);
            if let Some(tex) = self.tiles.get(&k) {
                painter.image(tex.id(), screen, full_uv, Color32::WHITE);
                continue;
            }
            // Not ready: show a coarser tile scaled up, if there is one.
            let mut drawn = false;
            for up in 1..=4u8 {
                let parent_level = level + up;
                if parent_level > MAX_LEVEL {
                    break;
                }
                let n = 1i32 << up;
                let parent = key(parent_level, k.tx.div_euclid(n), k.tz.div_euclid(n));
                if let Some(tex) = self.tiles.get(&parent) {
                    let (ox, oz) = (k.tx.rem_euclid(n) as f32, k.tz.rem_euclid(n) as f32);
                    let f = 1.0 / n as f32;
                    let uv = Rect::from_min_size(pos2(ox * f, oz * f), vec2(f, f));
                    painter.image(tex.id(), screen, uv, Color32::WHITE);
                    drawn = true;
                    break;
                }
            }
            if !drawn {
                painter.rect_filled(screen.shrink(0.5), 0.0, Color32::from_rgb(0x26, 0x26, 0x26));
            }
        }
    }

    fn draw_slime(&self, painter: &egui::Painter, rect: Rect) {
        let chunk_px = 16.0 * self.zoom;
        if chunk_px < 3.0 {
            return;
        }
        let (x0, z0) = self.to_world(rect, rect.min);
        let (x1, z1) = self.to_world(rect, rect.max);
        let fill = Color32::from_rgba_unmultiplied(0x6c, 0xe0, 0x5a, 110);
        for cz in (z0 / 16.0).floor() as i32..=(z1 / 16.0).floor() as i32 {
            for cx in (x0 / 16.0).floor() as i32..=(x1 / 16.0).floor() as i32 {
                if is_slime_chunk(self.seed, cx, cz) {
                    let min = self.to_screen(rect, cx as f64 * 16.0, cz as f64 * 16.0);
                    let r = Rect::from_min_size(min, Vec2::splat(chunk_px));
                    painter.rect_filled(r, 0.0, fill);
                }
            }
        }
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let (x0, z0) = self.to_world(rect, rect.min);
        let (x1, z1) = self.to_world(rect, rect.max);
        let lines = |step: f64, color: Color32| {
            if step * (self.zoom as f64) < 24.0 {
                return false;
            }
            let stroke = Stroke::new(1.0, color);
            let mut x = (x0 / step).floor() * step;
            while x <= x1 {
                let sx = self.to_screen(rect, x, 0.0).x;
                painter.vline(sx, rect.y_range(), stroke);
                x += step;
            }
            let mut z = (z0 / step).floor() * step;
            while z <= z1 {
                let sz = self.to_screen(rect, 0.0, z).y;
                painter.hline(rect.x_range(), sz, stroke);
                z += step;
            }
            true
        };
        lines(16.0, Color32::from_black_alpha(28));
        if !lines(512.0, Color32::from_black_alpha(70)) {
            lines(512.0 * 16.0, Color32::from_black_alpha(70));
        }
        // The axes through 0, 0.
        let o = self.to_screen(rect, 0.0, 0.0);
        let axis = Stroke::new(1.0, ACCENT.linear_multiply(0.6));
        painter.vline(o.x, rect.y_range(), axis);
        painter.hline(rect.x_range(), o.y, axis);
    }

    fn draw_markers(&mut self, ui: &egui::Ui, rect: Rect, response: &egui::Response) {
        let min_spacing = MARKER_SPACING_PX / self.zoom as f64;
        let (x0, z0) = self.to_world(rect, rect.min);
        let (x1, z1) = self.to_world(rect, rect.max);
        let span = (BLOCK_CHUNKS * 16) as f64;
        let mut visible: Vec<Marker> = Vec::new();
        let mut wanted: Vec<BlockKey> = Vec::new();
        for (kind, info) in KINDS.iter().enumerate() {
            if !self.kinds_on[kind]
                || info.dimension != self.dimension
                || info.set == STRONGHOLDS
                || info.set == SPAWN
            {
                continue;
            }
            if structures::spacing_blocks(kind).is_none_or(|s| s < min_spacing) {
                continue;
            }
            for bz in (z0 / span).floor() as i32..=(z1 / span).floor() as i32 {
                for bx in (x0 / span).floor() as i32..=(x1 / span).floor() as i32 {
                    let key = BlockKey {
                        seed: self.seed,
                        kind,
                        bx,
                        bz,
                    };
                    match self.markers.get(&key) {
                        Some(found) => visible.extend(found),
                        None => wanted.push(key),
                    }
                }
            }
        }
        // Nearest squares first.
        let (ccx, ccz) = (self.center.0 / span - 0.5, self.center.1 / span - 0.5);
        wanted.sort_by(|a, b| {
            let d = |k: &BlockKey| (k.bx as f64 - ccx).powi(2) + (k.bz as f64 - ccz).powi(2);
            d(a).total_cmp(&d(b))
        });
        self.markers.request(&wanted);
        if let Some(i) = KINDS.iter().position(|k| k.set == SPAWN)
            && self.dimension == Dimension::Overworld
            && self.kinds_on[i]
            && let Some((_, (x, y, z))) = self.spawn.get()
        {
            visible.push(Marker {
                kind: i,
                x,
                z,
                start: Some(("chunkscope:spawn", x, y, z)),
                ship: None,
            });
        }
        if let Some(i) = KINDS.iter().position(|k| k.set == STRONGHOLDS)
            && self.dimension == Dimension::Overworld
            && self.kinds_on[i]
        {
            visible.extend(self.strongholds.positions().map(|(x, z)| Marker {
                kind: i,
                x,
                z,
                start: None,
                ship: None,
            }));
        }

        // Hit test, nearest marker under the pointer.
        let pointer = response.hover_pos();
        let mut hovered: Option<(f32, Marker)> = None;
        let painter = ui.painter_at(rect);
        let r = if self.zoom >= 0.5 { 11.0 } else { 9.0 };
        for m in &visible {
            let p = self.to_screen(rect, m.x as f64, m.z as f64);
            if !rect.expand(r).contains(p) {
                continue;
            }
            if let Some(ptr) = pointer {
                let d = p.distance(ptr);
                if d <= r + 2.0 && hovered.is_none_or(|(best, _)| d < best) {
                    hovered = Some((d, *m));
                }
            }
            marker_badge(&painter, p, KINDS[m.kind].glyph, KINDS[m.kind].color, r);
            if m.ship == Some(true) {
                // End cities with a ship: a small accent dot on the badge.
                let dot = p + vec2(r * 0.75, -r * 0.75);
                painter.circle(
                    dot,
                    r * 0.38,
                    ACCENT,
                    Stroke::new(1.5, Color32::from_rgb(0x1a, 0x1a, 0x1a)),
                );
            }
        }
        self.hover_marker = hovered.map(|(_, m)| m);
        for (m, color) in [(self.hover_marker, Color32::WHITE), (self.selected, ACCENT)] {
            // Only ring markers that are drawn at this zoom.
            if let Some(m) = m.filter(|m| visible.contains(m)) {
                let p = self.to_screen(rect, m.x as f64, m.z as f64);
                painter.circle_stroke(p, r + 3.0, Stroke::new(2.0, color));
            }
        }
        if let Some(m) = self.hover_marker {
            let p = self.to_screen(rect, m.x as f64, m.z as f64);
            let kind = &KINDS[m.kind];
            let name = match m.start {
                _ if kind.set == SPAWN => kind.label.to_owned(),
                Some((structure, ..)) => biome_colors::display_name(structure),
                None => singular(kind.label),
            };
            let estimate = if kind.set == SPAWN { " (estimate)" } else { "" };
            let ship = if m.ship == Some(true) {
                " with a ship"
            } else {
                ""
            };
            let label = format!("{name}{ship}{estimate}  {}, {}", m.x, m.z);
            let galley = painter.layout_no_wrap(label, FontId::proportional(12.0), TEXT);
            let pos = p + vec2(r + 8.0, -galley.size().y / 2.0);
            let bg = Rect::from_min_size(pos, galley.size()).expand2(vec2(8.0, 5.0));
            painter.rect(
                bg,
                6.0,
                PANEL,
                Stroke::new(1.0, BORDER),
                StrokeKind::Outside,
            );
            painter.galley(pos, galley, TEXT);
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            self.selected = self.hover_marker;
        }
    }

    fn floating_toolbar(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let size = vec2(330.0, 44.0);
        let bar = Rect::from_center_size(
            pos2(rect.center().x, rect.bottom() - 16.0 - size.y / 2.0),
            size,
        );
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(bar)
                .layout(Layout::left_to_right(Align::Center)),
        );
        theme::floating_frame().show(&mut child, |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            icon_button(ui, Icon::Hand, true).on_hover_text("Pan (drag the map)");
            separator(ui);
            if icon_button(ui, Icon::Minus, false)
                .on_hover_text("Zoom out")
                .clicked()
            {
                self.zoom_around(rect, rect.center(), 0.5);
            }
            let pct = self.zoom * 100.0;
            let text = if pct >= 10.0 {
                format!("{pct:.0}%")
            } else {
                format!("{pct:.1}%")
            };
            ui.add_sized(
                vec2(52.0, 28.0),
                egui::Label::new(RichText::new(text).color(TEXT)),
            );
            if icon_button(ui, Icon::Plus, false)
                .on_hover_text("Zoom in")
                .clicked()
            {
                self.zoom_around(rect, rect.center(), 2.0);
            }
            separator(ui);
            if icon_button(ui, Icon::Target, false)
                .on_hover_text("Back to 0, 0")
                .clicked()
            {
                self.center = (0.0, 0.0);
            }
            if icon_button(ui, Icon::Grid, self.show_grid)
                .on_hover_text("Grid")
                .clicked()
            {
                self.show_grid ^= true;
            }
            if self.dimension == Dimension::Overworld
                && icon_button(ui, Icon::Slime, self.show_slime)
                    .on_hover_text("Slime chunks")
                    .clicked()
            {
                self.show_slime ^= true;
            }
            if icon_button(ui, Icon::Layers, self.show_biomes)
                .on_hover_text("Biomes")
                .clicked()
            {
                self.show_biomes ^= true;
            }
        });
    }

    fn draw_toast(&mut self, ui: &egui::Ui, rect: Rect) {
        let Some((text, until)) = &self.toast else {
            return;
        };
        let now = ui.input(|i| i.time);
        if now > *until {
            self.toast = None;
            return;
        }
        ui.ctx().request_repaint();
        let painter = ui.painter_at(rect);
        let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(13.0), ON_ACCENT);
        let pos = pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 18.0);
        let bg = Rect::from_min_size(pos, galley.size()).expand2(vec2(12.0, 7.0));
        painter.rect_filled(bg, 8.0, ACCENT);
        painter.galley(pos, galley, ON_ACCENT);
    }
}

// ---------------------------------------------------------------- widgets

fn about_panel(ui: &mut egui::Ui) {
    panel_header(ui, "About", None);
    section(ui, |ui| {
        ui.label(RichText::new("Chunkscope").heading().color(TEXT));
        ui.label(
            RichText::new(
                "A local, open-source seed map for Minecraft: Java Edition. \
                 World generation is ported bit for bit and checked against the game.",
            )
            .color(TEXT_WEAK),
        );
        ui.add_space(6.0);
        theme::field_label(ui, "Version");
        field_box(ui, concat!("Chunkscope ", env!("CARGO_PKG_VERSION")));
        theme::field_label(ui, "Matches");
        field_box(ui, GAME_VERSION);
        theme::field_label(ui, "License");
        field_box(ui, "MIT OR Apache-2.0");
        ui.add_space(6.0);
        ui.hyperlink_to(
            "github.com/byRandom/Chunkscope",
            "https://github.com/byRandom/Chunkscope",
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new("Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.")
                .color(TEXT_WEAK)
                .small(),
        );
    });
}

pub(crate) fn panel_header(ui: &mut egui::Ui, title: &str, subtitle: Option<&str>) {
    egui::Frame::new()
        .inner_margin(Margin {
            left: 14,
            right: 14,
            top: 12,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).heading().color(TEXT));
            if let Some(s) = subtitle {
                ui.label(RichText::new(s).color(TEXT_WEAK).small());
            }
        });
    theme::divider(ui);
}

pub(crate) fn section(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .inner_margin(Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn pill(ui: &mut egui::Ui, text: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(11.0), ACCENT);
    let size = galley.size() + vec2(18.0, 8.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 10.0, ACCENT_DIM);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ACCENT);
}

/// A read-only value in an input-like box.
pub(crate) fn field_box(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(FIELD)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            fit_label(ui, RichText::new(text).color(TEXT));
        });
}

fn swatch_box(ui: &mut egui::Ui, color: Color32, text: &str) {
    egui::Frame::new()
        .fill(FIELD)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                ui.painter().rect_filled(r, 3.0, color);
                ui.painter().rect_stroke(
                    r,
                    3.0,
                    Stroke::new(1.0, Color32::from_white_alpha(40)),
                    StrokeKind::Inside,
                );
                fit_label(ui, RichText::new(text).color(TEXT));
            });
        });
}

/// A label cut to the width left in `ui` (with an ellipsis), so a long value
/// never makes a side panel wider than it is. egui gives the extra width of
/// an overflowing right panel to the map, which then draws over the panel.
/// Selecting and copying the label still copies the whole text.
pub(crate) fn fit_label(ui: &mut egui::Ui, text: RichText) -> egui::Response {
    ui.add(egui::Label::new(text).truncate())
}

/// A full-width value with its label inside the box.
fn labelled_box(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::new()
        .fill(FIELD)
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::symmetric(8, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).color(TEXT_WEAK).small());
                fit_label(ui, RichText::new(value).color(TEXT));
            });
        });
}

/// Two labelled values side by side, like the W/H fields of a design tool.
fn xz_row(ui: &mut egui::Ui, a: (&str, &str), b: (&str, &str)) {
    ui.columns(2, |cols| {
        for (col, (label, value)) in cols.iter_mut().zip([a, b]) {
            egui::Frame::new()
                .fill(FIELD)
                .corner_radius(CornerRadius::same(theme::RADIUS))
                .inner_margin(Margin::symmetric(8, 5))
                .show(col, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(label).color(TEXT_WEAK).small());
                        fit_label(ui, RichText::new(value).color(TEXT));
                    });
                });
        }
    });
}

fn rail_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    active: bool,
    enabled: bool,
) -> egui::Response {
    let size = vec2(48.0, 50.0);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    let response = if enabled {
        response
    } else {
        response.on_hover_cursor(egui::CursorIcon::NotAllowed)
    };
    let p = ui.painter();
    let tile = Rect::from_center_size(rect.center_top() + vec2(0.0, 16.0), vec2(32.0, 30.0));
    if active {
        p.rect_filled(tile, 7.0, ACCENT_DIM);
    } else if response.hovered() && enabled {
        p.rect_filled(tile, 7.0, FIELD);
    }
    let color = match (active, enabled) {
        (true, _) => ACCENT,
        (false, true) => TEXT,
        (false, false) => Color32::from_gray(0x6a),
    };
    icons::paint(
        p,
        Rect::from_center_size(tile.center(), vec2(17.0, 17.0)),
        icon,
        color,
    );
    p.text(
        rect.center_bottom() - vec2(0.0, 6.0),
        Align2::CENTER_BOTTOM,
        label,
        FontId::proportional(10.5),
        if enabled {
            TEXT_WEAK
        } else {
            Color32::from_gray(0x6a)
        },
    );
    if enabled {
        response
    } else {
        response.on_hover_text("Seed finder: coming soon")
    }
}

/// A square icon button; `active` fills it with the accent, like the
/// selected tool of a design app.
pub(crate) fn icon_button(ui: &mut egui::Ui, icon: Icon, active: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
    let p = ui.painter();
    let color = if active {
        p.rect_filled(rect, 7.0, ACCENT);
        ON_ACCENT
    } else {
        if response.hovered() {
            p.rect_filled(rect, 7.0, FIELD);
        }
        TEXT
    };
    icons::paint(
        p,
        Rect::from_center_size(rect.center(), vec2(16.0, 16.0)),
        icon,
        color,
    );
    response
}

pub(crate) fn accent_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).color(ON_ACCENT).strong())
        .fill(ACCENT)
        .corner_radius(CornerRadius::same(theme::RADIUS));
    ui.add(button)
}

fn separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter()
        .vline(rect.center().x, rect.y_range(), Stroke::new(1.0, BORDER));
}

/// A layer row: icon or colored badge, name, optional note, eye toggle.
fn layer_row(
    ui: &mut egui::Ui,
    on: &mut bool,
    icon: Icon,
    badge: Option<(&str, Color32)>,
    label: &str,
    note: Option<&str>,
) {
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, 28.0), Sense::click());
    let p = ui.painter();
    if response.hovered() {
        p.rect_filled(rect.expand2(vec2(6.0, 0.0)), 6.0, FIELD);
    }
    let dim = |c: Color32| if *on { c } else { c.linear_multiply(0.4) };
    let icon_rect = Rect::from_center_size(rect.left_center() + vec2(9.0, 0.0), vec2(16.0, 16.0));
    match badge {
        Some((glyph, color)) => marker_badge(p, icon_rect.center(), glyph, dim(color), 9.0),
        None => icons::paint(p, icon_rect, icon, dim(TEXT_WEAK)),
    }
    let text_pos = rect.left_center() + vec2(26.0, 0.0);
    let name = p.text(
        text_pos,
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(13.0),
        dim(TEXT),
    );
    if let Some(note) = note {
        p.text(
            pos2(name.right() + 6.0, text_pos.y),
            Align2::LEFT_CENTER,
            note,
            FontId::proportional(10.5),
            Color32::from_gray(0x70),
        );
    }
    let eye = Rect::from_center_size(rect.right_center() - vec2(10.0, 0.0), vec2(16.0, 16.0));
    let eye_color = if response.hovered() { TEXT } else { TEXT_WEAK };
    icons::paint(
        p,
        eye,
        if *on { Icon::Eye } else { Icon::EyeOff },
        dim(eye_color),
    );
    if response.clicked() {
        *on = !*on;
    }
}

/// A round structure marker with its glyph.
/// A layer's name in the singular: "End cities" -> "End city".
fn singular(label: &str) -> String {
    match label.strip_suffix("ies") {
        Some(stem) => format!("{stem}y"),
        None => label.trim_end_matches('s').to_owned(),
    }
}

fn marker_badge(painter: &egui::Painter, center: Pos2, glyph: &str, color: Color32, radius: f32) {
    painter.circle_filled(
        center + vec2(0.0, 1.0),
        radius + 1.0,
        Color32::from_black_alpha(90),
    );
    painter.circle(
        center,
        radius,
        color,
        Stroke::new(1.5, Color32::from_rgb(0x1a, 0x1a, 0x1a)),
    );
    let size = if glyph.len() > 1 {
        radius * 0.95
    } else {
        radius * 1.25
    };
    painter.text(
        center,
        Align2::CENTER_CENTER,
        glyph,
        FontId::proportional(size),
        Color32::from_rgb(0x14, 0x14, 0x14),
    );
}
