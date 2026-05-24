pub const SECTION_SPACING: f32 = 10.0;
pub const SOURCE_ACTION_BUTTON_SIZE: egui::Vec2 = egui::vec2(96.0, 28.0);
pub const REMOVE_BUTTON_SIZE: egui::Vec2 = egui::vec2(76.0, 28.0);

pub fn apply_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = egui::Color32::from_rgb(18, 20, 24);
    visuals.panel_fill = egui::Color32::from_rgb(14, 16, 20);
    visuals.extreme_bg_color = egui::Color32::from_rgb(8, 10, 13);
    visuals.faint_bg_color = egui::Color32::from_rgb(25, 29, 36);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(22, 25, 31);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(34, 39, 48);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(47, 55, 68);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(60, 73, 92);
    visuals.selection.bg_fill = egui::Color32::from_rgb(0, 128, 170);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.slider_width = 190.0;
    style.spacing.combo_width = 190.0;
    style.visuals = ctx.global_style().visuals.clone();
    ctx.set_global_style(style);
}

pub fn section<R>(
    ui: &mut egui::Ui,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(22, 25, 31))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(46, 53, 65)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(title).strong().size(15.0));
            ui.add_space(6.0);
            add_contents(ui)
        })
        .inner
}

pub fn muted_label(ui: &mut egui::Ui, text: impl Into<String>) {
    ui.label(
        egui::RichText::new(text.into())
            .color(egui::Color32::from_rgb(155, 165, 178))
            .size(12.0),
    );
}

pub fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.button(text)
}

pub fn primary_sized_button(ui: &mut egui::Ui, text: &str, size: egui::Vec2) -> egui::Response {
    ui.add_sized(size, egui::Button::new(text))
}

pub fn primary_button_enabled(ui: &mut egui::Ui, enabled: bool, text: &str) -> egui::Response {
    ui.add_enabled(enabled, egui::Button::new(text))
}

pub fn sized_button(ui: &mut egui::Ui, text: &str, size: egui::Vec2) -> egui::Response {
    ui.add_sized(size, egui::Button::new(text))
}

pub fn sized_button_enabled(
    ui: &mut egui::Ui,
    enabled: bool,
    text: &str,
    size: egui::Vec2,
) -> egui::Response {
    ui.add_enabled_ui(enabled, |ui| ui.add_sized(size, egui::Button::new(text)))
        .inner
}
