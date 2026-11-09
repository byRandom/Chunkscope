//! The app's look: a dark charcoal theme with a mint accent, flat panels,
//! thin borders and small rounded corners.

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Vec2,
};

/// Canvas and title bar.
pub const BG: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x1e);
/// Side panels, toolbars.
pub const PANEL: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x2c);
/// Inputs and raised controls.
pub const FIELD: Color32 = Color32::from_rgb(0x38, 0x38, 0x38);
/// Hovered controls.
pub const HOVER: Color32 = Color32::from_rgb(0x44, 0x44, 0x44);
/// Panel borders and separators.
pub const BORDER: Color32 = Color32::from_rgb(0x3d, 0x3d, 0x3d);
/// Main text.
pub const TEXT: Color32 = Color32::from_rgb(0xe8, 0xe8, 0xe8);
/// Secondary text and labels.
pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x9e, 0x9e, 0x9e);
/// The accent: primary buttons, active tools, selection.
pub const ACCENT: Color32 = Color32::from_rgb(0x03, 0xbc, 0x82);
/// A dark tint of the accent, behind active icons.
pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x0a, 0x3d, 0x2e);
/// Text on accent-filled buttons.
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x06, 0x1a, 0x13);

pub const RADIUS: u8 = 6;

pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();

    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(8.0, 6.0);
    s.button_padding = Vec2::new(10.0, 5.0);
    s.interact_size = Vec2::new(28.0, 26.0);
    s.window_margin = Margin::same(12);
    s.menu_margin = Margin::same(6);
    s.text_edit_width = 160.0;

    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    v.popup_shadow = v.window_shadow;
    v.extreme_bg_color = FIELD;
    v.faint_bg_color = Color32::from_rgb(0x31, 0x31, 0x31);
    v.code_bg_color = FIELD;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT.linear_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.text_cursor.stroke = Stroke::new(1.5, ACCENT);
    v.button_frame = true;
    v.indent_has_left_vline = false;

    let radius = CornerRadius::same(RADIUS);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = FIELD;
    w.inactive.weak_bg_fill = FIELD;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = HOVER;
    w.hovered.weak_bg_fill = HOVER;
    w.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x55, 0x55, 0x55));
    w.hovered.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = ACCENT_DIM;
    w.active.weak_bg_fill = ACCENT_DIM;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.5, Color32::WHITE);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open = w.active;

    ctx.set_global_style(style);
}

/// A frame for the side panels.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(Margin::same(0))
}

/// A floating card over the map (toolbar, legend).
pub fn floating_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(1.0, Color32::from_rgb(0x47, 0x47, 0x47)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(6))
        .shadow(Shadow {
            offset: [0, 4],
            blur: 16,
            spread: 0,
            color: Color32::from_black_alpha(120),
        })
}

/// A small uppercase-free section title, as in the inspector panels.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(TEXT).strong());
}

/// A dim field label above a value.
pub fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).color(TEXT_WEAK).small());
}

/// A thin full-width separator line.
pub fn divider(ui: &mut egui::Ui) {
    let rect = ui.available_rect_before_wrap();
    let y = rect.top();
    ui.painter()
        .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, BORDER));
    ui.add_space(1.0);
}
