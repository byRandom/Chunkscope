//! The seed finder panel: filters, a background search with progress, and
//! the matching seeds (click one to open it on the map).

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use eframe::egui::{self, RichText};
use mc_core::biome::Biome;
use mc_core::finder::{
    Center, Match, Query, SearchState, Seeds, StructureFilter, TARGETS, Target, search,
};
use mc_core::parse_seed;

use crate::app::{accent_button, fit_label, icon_button, panel_header, section};
use crate::biome_colors;
use crate::dimension::Dimension;
use crate::icons::Icon;
use crate::settings::{FilterSettings, FinderSettings};
use crate::structures::{KINDS, kind_index};
use crate::theme::{self, TEXT, TEXT_WEAK};

/// The finder stops collecting matches past this many.
const MAX_RESULTS: usize = 500;

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

/// The query's text field.
const QUERY_ID: &str = "finder-query";

pub struct FinderUi {
    /// What the form shows.
    query: Query,
    /// The query's text form, as the user edits or pastes it.
    query_text: String,
    /// The form's query when `query_text` was last written from it.
    synced: String,
    /// Why `query_text` does not parse.
    query_error: Option<String>,
    from: String,
    count: u64,
    random: bool,
    running: Option<Running>,
    results: Arc<Mutex<Vec<Match>>>,
}

/// A target's name in the finder (its layer's label), with its dimension
/// when not the Overworld.
fn target_label(t: &Target) -> String {
    let label = kind_index(t.id).map_or(t.id, |i| KINDS[i].label);
    match t.dimension {
        Dimension::Overworld => label.to_owned(),
        Dimension::Nether => format!("{label} (Nether)"),
        Dimension::End => format!("{label} (End)"),
    }
}

/// The filter the form starts with, and adds.
fn default_filter() -> StructureFilter {
    let f = FilterSettings::default();
    let target = f.target().unwrap_or(&TARGETS[0]);
    StructureFilter {
        min_count: f.count.max(1),
        ..StructureFilter::new(target, f.radius.max(0))
    }
}

impl FinderUi {
    /// The panel with a saved query. Filters on layers that no longer exist
    /// are dropped.
    pub fn from_settings(s: &FinderSettings) -> Self {
        let mut structures: Vec<StructureFilter> = s
            .filters
            .iter()
            .filter_map(|f| {
                Some(StructureFilter {
                    target: f.target()?,
                    radius: f.radius.max(0),
                    min_count: f.count.max(1),
                    ship_only: f.ship,
                })
            })
            .collect();
        if structures.is_empty() {
            structures.push(default_filter());
        }
        let biomes: Vec<Biome> = s.biomes().filter_map(Biome::from_name).collect();
        let query = Query {
            center: if s.center_spawn {
                Center::Spawn
            } else {
                Center::Zero
            },
            structures,
            center_biomes: (!biomes.is_empty()).then_some(biomes),
        };
        let text = query.to_string();
        Self {
            query,
            query_text: text.clone(),
            synced: text,
            query_error: None,
            from: s.from.clone(),
            count: s.count,
            random: s.random,
            running: None,
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// The query, to save.
    pub fn settings(&self) -> FinderSettings {
        let biomes: Vec<String> = self
            .query
            .center_biomes
            .iter()
            .flatten()
            .map(|b| b.name().to_owned())
            .collect();
        FinderSettings {
            center_spawn: self.query.center == Center::Spawn,
            filters: self
                .query
                .structures
                .iter()
                .map(|f| FilterSettings {
                    layer: f.target.id.to_owned(),
                    radius: f.radius,
                    count: f.min_count,
                    ship: f.ship_only,
                })
                .collect(),
            // Versions before 0.7 read one biome.
            biome: biomes.first().cloned(),
            biomes,
            from: self.from.clone(),
            count: self.count,
            random: self.random,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|r| r.thread.as_ref().is_some_and(|t| !t.is_finished()))
    }

    fn start(&mut self, ctx: &egui::Context) {
        let query = self.query.clone();
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
                // Copyable while a search runs.
                self.query_text_ui(ui, running);
                ui.add_space(6.0);
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

    /// The query as text: copy it, paste one, or edit it. It follows the
    /// form, and the form follows it while it parses; text that does not
    /// parse shows why, and goes back to the form's query once the field
    /// loses focus.
    fn query_text_ui(&mut self, ui: &mut egui::Ui, running: bool) {
        let id = egui::Id::unique(QUERY_ID);
        let current = self.query.to_string();
        if current != self.synced && !ui.memory(|m| m.has_focus(id)) {
            self.query_text = current.clone();
            self.synced = current;
            self.query_error = None;
        }
        theme::section_title(ui, "Query");
        ui.horizontal_top(|ui| {
            let edit = ui.add(
                // Wraps, so the whole query shows in the narrow panel.
                egui::TextEdit::multiline(&mut self.query_text)
                    .id(id)
                    .desired_rows(2)
                    .interactive(!running)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(ui.available_width() - 32.0)
                    .hint_text("villages:300 biome=plains"),
            );
            if edit.changed() {
                match self.query_text.parse::<Query>() {
                    Ok(q) => {
                        self.query = q;
                        self.query_error = None;
                    }
                    Err(e) => self.query_error = Some(e.to_string()),
                }
            }
            if icon_button(ui, Icon::Copy, false)
                .on_hover_text("Copy query")
                .clicked()
            {
                ui.ctx().copy_text(self.query_text.clone());
            }
        });
        if let Some(e) = &self.query_error {
            ui.label(RichText::new(e).color(theme::ERROR).small());
        }
    }

    fn filters_ui(&mut self, ui: &mut egui::Ui) {
        theme::section_title(ui, "Center");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.query.center, Center::Spawn, "World spawn");
            ui.radio_value(&mut self.query.center, Center::Zero, "0, 0");
        });
        ui.add_space(6.0);
        theme::section_title(ui, "Structures");
        let mut remove = None;
        for (i, f) in self.query.structures.iter_mut().enumerate() {
            egui::Frame::new()
                .fill(theme::FIELD)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS))
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt(("finder-kind", i))
                            .width(ui.available_width() - 40.0)
                            .selected_text(target_label(f.target))
                            .show_ui(ui, |ui| {
                                for t in KINDS.iter().filter_map(|k| k.target()) {
                                    ui.selectable_value(&mut f.target, t, target_label(t));
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
                        ui.add(egui::DragValue::new(&mut f.min_count).range(1..=20));
                    });
                    if f.target.has_ships() {
                        ui.checkbox(&mut f.ship_only, "With a ship (elytra)");
                    }
                });
            ui.add_space(4.0);
        }
        if let Some(i) = remove {
            self.query.structures.remove(i);
        }
        if self
            .query
            .structures
            .iter()
            .any(|f| f.dimension() != Dimension::Overworld)
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
            self.query.structures.push(StructureFilter {
                radius: 500,
                ..default_filter()
            });
        }
        ui.add_space(6.0);
        theme::section_title(ui, "Biome at the center");
        let biomes = self.query.center_biomes.get_or_insert_with(Vec::new);
        let label = match biomes.as_slice() {
            [] => "Any".to_owned(),
            [b] => biome_colors::display_name(b.name()),
            [b, rest @ ..] => format!(
                "{} + {} more",
                biome_colors::display_name(b.name()),
                rest.len()
            ),
        };
        egui::ComboBox::from_id_salt("finder-biome")
            .width(ui.available_width())
            .selected_text(label)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                if ui.selectable_label(biomes.is_empty(), "Any").clicked() {
                    biomes.clear();
                }
                for b in Dimension::Overworld.biomes() {
                    let mut on = biomes.contains(&b);
                    if ui
                        .checkbox(&mut on, biome_colors::display_name(b.name()))
                        .changed()
                    {
                        if on {
                            biomes.push(b);
                        } else {
                            biomes.retain(|&x| x != b);
                        }
                    }
                }
            });
        if biomes.is_empty() {
            self.query.center_biomes = None;
        }
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
        ui.horizontal(|ui| {
            theme::section_title(ui, &format!("Results ({})", results.len()));
            if !results.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, Icon::Copy, false)
                        .on_hover_text("Copy all results, one line per seed")
                        .clicked()
                    {
                        let lines: Vec<String> = results.iter().map(Match::to_string).collect();
                        ui.ctx().copy_text(lines.join("\n"));
                    }
                });
            }
        });
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
                        Dimension::Overworld => "",
                        Dimension::Nether => " (Nether)",
                        Dimension::End => " (End)",
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
