use crate::{
    render::renderer::{PatternLayer, RenderParams},
    ui::{
        FormulaSourceTarget, UiAction,
        controls::{layer_header, slider},
        formula_source_panel::{FormulaEditorContext, formula_editor, source_buttons},
        style,
    },
};

pub fn show_patterns(ui: &mut egui::Ui, params: &mut RenderParams, actions: &mut Vec<UiAction>) {
    style::section(ui, "Patterns", |ui| {
        layer_header(ui, "Add Pattern", || actions.push(UiAction::AddPattern));
        for (index, layer) in params.patterns.iter_mut().enumerate() {
            pattern_layer_editor(ui, index, layer, actions);
            ui.add_space(8.0);
        }
    });
}

fn pattern_layer_editor(
    ui: &mut egui::Ui,
    index: usize,
    layer: &mut PatternLayer,
    actions: &mut Vec<UiAction>,
) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut layer.enabled, "")
            .on_hover_text("Enable pattern");
        ui.label(egui::RichText::new(format!("Pattern {}", index + 1)).strong());
        ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut layer.name));
        source_buttons(ui, FormulaSourceTarget::Pattern(index), actions);
        if index > 0 && style::sized_button(ui, "Remove", style::REMOVE_BUTTON_SIZE).clicked() {
            actions.push(UiAction::RemovePattern(index));
        }
    });
    ui.add_space(4.0);

    ui.separator();
    ui.label(egui::RichText::new("Source mix").strong().size(13.0));
    slider(ui, &mut layer.strength, 0.0..=1.0, "Strength");

    ui.separator();
    ui.label(
        egui::RichText::new("Transform / Motion")
            .strong()
            .size(13.0),
    );
    slider(ui, &mut layer.scale, 0.1..=8.0, "Scale");
    slider(ui, &mut layer.motion, -4.0..=4.0, "Looped motion");
    egui::CollapsingHeader::new("Morph / camera loop")
        .id_salt(format!("pattern_morph_camera_{index}"))
        .show(ui, |ui| {
            slider(ui, &mut layer.morph, 0.0..=1.0, "Morph");
            slider(
                ui,
                &mut layer.camera_zoom_loop,
                -0.95..=4.0,
                "Camera zoom loop",
            );
            slider(ui, &mut layer.camera_orbit, -4.0..=4.0, "Camera orbit");
        });
    formula_editor(
        ui,
        &mut layer.source,
        &format!("pattern_source_{index}"),
        FormulaEditorContext::Pattern,
    );
}
