use crate::{
    render::renderer::{EffectBlendMode, EffectLayer, RenderParams},
    ui::{
        FormulaSourceTarget, UiAction,
        controls::{layer_header, slider},
        formula_source_panel::{FormulaEditorContext, formula_editor, source_buttons},
        style,
    },
};

pub fn show_effects(ui: &mut egui::Ui, params: &mut RenderParams, actions: &mut Vec<UiAction>) {
    style::section(ui, "Effects", |ui| {
        layer_header(ui, "Add Effect", || actions.push(UiAction::AddEffect));
        for (index, layer) in params.effects.iter_mut().enumerate() {
            effect_layer_editor(ui, index, layer, actions);
            ui.add_space(8.0);
        }
    });
}

fn effect_layer_editor(
    ui: &mut egui::Ui,
    index: usize,
    layer: &mut EffectLayer,
    actions: &mut Vec<UiAction>,
) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut layer.enabled, "")
            .on_hover_text("Enable effect");
        ui.label(egui::RichText::new(format!("Effect {}", index + 1)).strong());
        ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut layer.name));
        source_buttons(ui, FormulaSourceTarget::Effect(index), actions);
        if style::sized_button(ui, "Remove", style::REMOVE_BUTTON_SIZE).clicked() {
            actions.push(UiAction::RemoveEffect(index));
        }
    });

    ui.separator();
    ui.label(egui::RichText::new("Effect-specific").strong().size(13.0));
    slider(ui, &mut layer.strength, 0.0..=1.0, "Strength");
    ui.horizontal(|ui| {
        ui.label("Effect mode");
        egui::ComboBox::from_id_salt(format!("effect_blend_mode_{index}"))
            .selected_text(layer.blend_mode.label())
            .show_ui(ui, |ui| {
                for blend_mode in EffectBlendMode::ALL {
                    ui.selectable_value(&mut layer.blend_mode, blend_mode, blend_mode.label());
                }
            });
    });

    ui.separator();
    ui.label(
        egui::RichText::new("Transform / Motion")
            .strong()
            .size(13.0),
    );
    slider(ui, &mut layer.scale, 0.1..=8.0, "Scale");
    slider(ui, &mut layer.motion, -4.0..=4.0, "Looped motion");
    egui::CollapsingHeader::new("Morph / camera loop")
        .id_salt(format!("effect_morph_camera_{index}"))
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
        &format!("effect_source_{index}"),
        FormulaEditorContext::Effect {
            blend_mode: layer.blend_mode,
        },
    );
}
