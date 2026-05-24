use crate::ui::style;

pub fn layer_header(ui: &mut egui::Ui, button: &str, mut action: impl FnMut()) {
    ui.horizontal_wrapped(|ui| {
        if style::primary_button(ui, button).clicked() {
            action();
        }
    });
    ui.add_space(4.0);
}

pub fn slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
) {
    ui.add_sized([330.0, 20.0], egui::Slider::new(value, range).text(label));
}
