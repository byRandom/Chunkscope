//! The seed finder panel: filters, a background search with progress, and
//! the matching seeds (click one to open it on the map).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use eframe::egui::{self, RichText};
use mc_core::biome::Biome;
use mc_core::finder::{
    Center, Dimension as FinderDimension, Match, Query, SearchState, Seeds, StructureFilter, search,
};
use mc_core::parse_seed;

use crate::app::{accent_button, fit_label, icon_button, panel_header, section};
use crate::biome_colors;
use crate::dimension::Dimension;
use crate::icons::Icon;
use crate::structures::{self, KINDS, SPAWN, STRONGHOLDS};
use crate::theme::{self, TEXT, TEXT_WEAK};

/// The finder stops collecting matches past this many.
const MAX_RESULTS: usize = 500;

struct FilterRow {
    /// Index into `KINDS`.
    kind: usize,
    radius: i32,
    count: u32,
    /// End cities: only those with a ship.
    ship: bool,
}

struct Running {
    state: Arc<SearchState>,
    total: u64,
    started: Instant,
    thread: Option<thread::JoinHandle<()>>,
}

/// What the panel asks the app to do.
pub enum Action {
    /// Show this seed on the map, centered on this block.
    Open { seed: i64, center: (i32, i32) },
}

pub struct FinderUi {
    center_spawn: bool,
    filters: Vec<FilterRow>,
    biome: Option<Biome>,
    from: String,
    count: u64,
    random: bool,
    running: Option<Running>,
    results: Arc<Mutex<Vec<Match>>>,
}

/// Structure layers the finder can search (not the spawn, not strongholds).
fn searchable(kind: usize) -> bool {
    let k = &KINDS[kind];
    k.set != SPAWN && k.set != STRONGHOLDS
}

/// A layer's name in the finder, with its dimension when not the Overworld.
fn kind_label(kind: usize) -> String {
    let k = &KINDS[kind];
    match k.dimension {
        Dimension::Overworld => k.label.to_owned(),
        Dimension::Nether => format!("{} (Nether)", k.label),
        Dimension::End => format!("{} (End)", k.label),
    }
}

impl Default for FinderUi {
    fn default() -> Self {
        let village = KINDS
            .iter()
            .position(|k| k.set == "minecraft:villages")
            .unwrap_or(0);
        Self {
            center_spawn: true,
            filters: vec![FilterRow {
                kind: village,
                radius: 300,
                count: 1,
                ship: false,
            }],
            biome: None,
            from: "0".to_owned(),
            count: 100_000,
            random: false,
            running: None,
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl FinderUi {
    pub fn is_running(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|r| r.thread.as_ref().is_some_and(|t| !t.is_finished()))
    }

    fn query(&self) -> Option<Query> {
        let mut structures_ = Vec::new();
        for f in &self.filters {
            let kind = &KINDS[f.kind];
            structures_.push(StructureFilter {
                set: structures::set(kind.set)?,
                radius: f.radius,
                min_count: f.count,
                dimension: match kind.dimension {
                    Dimension::Overworld => FinderDimension::Overworld,
                    Dimension::Nether => FinderDimension::Nether,
                    Dimension::End => FinderDimension::End,
                },
                only: kind.only,
                ship: f.ship && kind.set == "minecraft:end_cities",
            });
        }
        Some(Query {
            center: if self.center_spawn {
                Center::Spawn
            } else {
                Center::Zero
            },
            structures: structures_,
            center_biomes: self.biome.map(|b| vec![b]),
        })
    }

    fn start(&mut self, ctx: &egui::Context) {
        let Some(query) = self.query() else {
            return;
        };
        let from = parse_seed(&self.from).unwrap_or(0);
        let seeds = if self.random {
            Seeds::Random {
                salt: from as u64,
                count: self.count,
            }
        } else {
            Seeds::Sequential {
                start: from,
                count: self.count,
            }
        };
        self.results.lock().expect("results").clear();
        let state = Arc::new(SearchState::default());
        let results = self.results.clone();
        let ctx = ctx.clone();
        let st = state.clone();
        let threads = thread::available_parallelism().map_or(2, |n| n.get());
        let handle = thread::Builder::new()
            .name("seed-finder".into())
            .spawn(move || {
                search(seeds, &query, threads, &st, &|m| {
                    let mut r = results.lock().expect("results");
                    // Other threads finish their batch after the cancel.
                    if r.len() < MAX_RESULTS {
                        r.push(m);
                    }
                    if r.len() >= MAX_RESULTS {
                        st.cancel.store(true, Ordering::Relaxed);
                    }
                    ctx.request_repaint();
                });
                ctx.request_repaint();
            })
            .expect("spawn seed finder");
        self.running = Some(Running {
            state,
            total: seeds.count(),
            started: Instant::now(),
            thread: Some(handle),
        });
    }

    fn stop(&mut self) {
        if let Some(r) = &self.running {
            r.state.cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        let ctx = ui.ctx().clone();
        let mut action = None;
        let running = self.is_running();
        if running {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        panel_header(ui, "Seed finder", Some("Structures near the spawn"));
        egui::ScrollArea::vertical().show(ui, |ui| {
            section(ui, |ui| {
                ui.add_enabled_ui(!running, |ui| self.filters_ui(ui));
            });
            theme::divider(ui);
            section(ui, |ui| {
                ui.add_enabled_ui(!running, |ui| self.seeds_ui(ui));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if running {
                        if ui.button("Stop").clicked() {
                            self.stop();
                        }
                    } else if accent_button(ui, "Search").clicked() {
                        self.start(&ctx);
                    }
                });
                self.progress_ui(ui);
            });
            theme::divider(ui);
            section(ui, |ui| action = self.results_ui(ui));
        });
        action
    }

    fn filters_ui(&mut self, ui: &mut egui::Ui) {
        theme::section_title(ui, "Center");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.center_spawn, true, "World spawn");
            ui.radio_value(&mut self.center_spawn, false, "0, 0");
        });
        ui.add_space(6.0);
        theme::section_title(ui, "Structures");
        let mut remove = None;
        for (i, f) in self.filters.iter_mut().enumerate() {
            egui::Frame::new()
                .fill(theme::FIELD)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(("finder-kind", i))
                            .width(ui.available_width() - 40.0)
                            .selected_text(kind_label(f.kind))
                            .show_ui(ui, |ui| {
                                for k in (0..KINDS.len()).filter(|&k| searchable(k)) {
                                    ui.selectable_value(&mut f.kind, k, kind_label(k));
                                }
                            });
                        if icon_button(ui, Icon::Minus, false)
                            .on_hover_text("Remove")
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("within").color(TEXT_WEAK).small());
                        ui.add(
                            egui::DragValue::new(&mut f.radius)
                                .range(16..=20_000)
                                .speed(16.0)
                                .suffix(" blocks"),
                        );
                        ui.label(RichText::new("at least").color(TEXT_WEAK).small());
                        ui.add(egui::DragValue::new(&mut f.count).range(1..=20));
                    });
                    if KINDS[f.kind].set == "minecraft:end_cities" {
                        ui.checkbox(&mut f.ship, "With a ship (elytra)");
                    }
                });
            ui.add_space(4.0);
        }
        if let Some(i) = remove {
            self.filters.remove(i);
        }
        if self
            .filters
            .iter()
            .any(|f| KINDS[f.kind].dimension != Dimension::Overworld)
        {
            ui.label(
                RichText::new(
                    "Nether and End structures are measured from 0, 0 in their dimension.",
                )
                .color(TEXT_WEAK)
                .small(),
            );
        }
        if ui.button("+ Add structure").clicked() {
            let kind = (0..KINDS.len()).find(|&k| searchable(k)).unwrap_or(0);
            self.filters.push(FilterRow {
                kind,
                radius: 500,
                count: 1,
                ship: false,
            });
        }
        ui.add_space(6.0);
        theme::section_title(ui, "Biome at the center");
        let label = self
            .biome
            .map_or("Any".to_owned(), |b| biome_colors::display_name(b.name()));
        egui::ComboBox::from_id_salt("finder-biome")
            .width(ui.available_width())
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.biome, None, "Any");
                for b in Dimension::Overworld.biomes() {
                    ui.selectable_value(
                        &mut self.biome,
                        Some(b),
                        biome_colors::display_name(b.name()),
                    );
                }
            });
    }

    fn seeds_ui(&mut self, ui: &mut egui::Ui) {
        theme::section_title(ui, "Seeds");
        ui.columns(2, |cols| {
            theme::field_label(&mut cols[0], if self.random { "Salt" } else { "From" });
            cols[0].add(egui::TextEdit::singleline(&mut self.from).hint_text("0"));
            theme::field_label(&mut cols[1], "How many");
            cols[1].add(
                egui::DragValue::new(&mut self.count)
                    .range(1..=1_000_000_000u64)
                    .speed(1000.0),
            );
        });
        ui.checkbox(&mut self.random, "Random seeds");
    }

    fn progress_ui(&self, ui: &mut egui::Ui) {
        let Some(r) = &self.running else {
            return;
        };
        let checked = r.state.checked.load(Ordering::Relaxed);
        let matches = self.results.lock().expect("results").len();
        let secs = r.started.elapsed().as_secs_f64().max(0.001);
        ui.add_space(4.0);
        ui.add(
            egui::ProgressBar::new(checked as f32 / r.total.max(1) as f32)
                .desired_width(ui.available_width())
                .fill(theme::ACCENT),
        );
        fit_label(
            ui,
            RichText::new(format!(
                "{checked} of {} seeds · {:.0} seeds/s · {matches} found",
                r.total,
                checked as f64 / secs
            ))
            .color(TEXT_WEAK)
            .small(),
        );
        if matches >= MAX_RESULTS {
            fit_label(
                ui,
                RichText::new(format!("Stopped at {MAX_RESULTS} results"))
                    .color(TEXT_WEAK)
                    .small(),
            );
        }
    }

    fn results_ui(&self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        let mut results = self.results.lock().expect("results");
        // Matches arrive in whatever order the threads finish: keep the list
        // stable, in seed order.
        results.sort_by_key(|m| m.seed);
        theme::section_title(ui, &format!("Results ({})", results.len()));
        if results.is_empty() {
            ui.label(
                RichText::new("Matching seeds appear here. Click one to open it.")
                    .color(TEXT_WEAK)
                    .small(),
            );
        }
        for m in results.iter() {
            let summary: Vec<String> = m
                .found
                .iter()
                .map(|f| {
                    let dimension = match f.dimension {
                        FinderDimension::Overworld => "",
                        FinderDimension::Nether => " (Nether)",
                        FinderDimension::End => " (End)",
                    };
                    format!(
                        "{}{dimension} {:.0}m",
                        biome_colors::display_name(f.name),
                        f.distance
                    )
                })
                .collect();
            egui::Frame::new()
                .fill(theme::FIELD)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS))
                .inner_margin(egui::Margin::symmetric(8, 5))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        fit_label(ui, RichText::new(m.seed.to_string()).color(TEXT).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, Icon::Copy, false)
                                .on_hover_text("Copy seed")
                                .clicked()
                            {
                                ui.ctx().copy_text(m.seed.to_string());
                            }
                            if ui.small_button("Open").clicked() {
                                action = Some(Action::Open {
                                    seed: m.seed,
                                    center: m.center,
                                });
                            }
                        });
                    });
                    fit_label(
                        ui,
                        RichText::new(format!(
                            "{} · {}",
                            biome_colors::display_name(m.center_biome.name()),
                            summary.join(", ")
                        ))
                        .color(TEXT_WEAK)
                        .small(),
                    );
                });
            ui.add_space(4.0);
        }
        action
    }
}
