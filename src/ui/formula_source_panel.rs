use crate::{
    render::{
        formula::{CompiledFormula, FormulaBlendMode, FormulaLayer, FormulaSource},
        renderer::EffectBlendMode,
    },
    ui::{FormulaSourceTarget, UiAction, controls::slider, style},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormulaEditorContext {
    Pattern,
    Effect { blend_mode: EffectBlendMode },
}

pub fn formula_editor(
    ui: &mut egui::Ui,
    formula: &mut FormulaSource,
    id_prefix: &str,
    context: FormulaEditorContext,
) {
    normalize_formula_source_for_context(formula, context);
    ensure_formula_layers(formula);

    source_output_section(ui, formula, context);
    formula_layers(ui, formula, id_prefix);
    sync_formula_from_layers(formula);
}

pub fn normalize_formula_source_for_context(
    formula: &mut FormulaSource,
    context: FormulaEditorContext,
) {
    match context {
        FormulaEditorContext::Pattern => formula.effect_blend_mode = None,
        FormulaEditorContext::Effect { blend_mode } => formula.effect_blend_mode = Some(blend_mode),
    }
}

fn ensure_formula_layers(formula: &mut FormulaSource) {
    if formula.layers.is_empty() {
        formula.layers.push(FormulaLayer {
            name: "Layer 1".to_owned(),
            expression: formula.expression.clone(),
            gain: formula.gain,
            bias: formula.bias,
            ..FormulaLayer::default()
        });
    }
}

fn source_output_section(
    ui: &mut egui::Ui,
    formula: &mut FormulaSource,
    _context: FormulaEditorContext,
) {
    ui.separator();
    subsection_title(ui, "Formula Source");
    slider(ui, &mut formula.gain, -4.0..=4.0, "Source gain");
    slider(ui, &mut formula.bias, -2.0..=2.0, "Source bias");
}

fn formula_layers(ui: &mut egui::Ui, formula: &mut FormulaSource, id_prefix: &str) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Formula layers").strong());
        if style::primary_button(ui, "Add Layer").clicked() {
            let index = formula.layers.len() + 1;
            formula.layers.push(FormulaLayer {
                name: format!("Layer {index}"),
                expression: "sin(x * scale + t * motion)".to_owned(),
                gain: 0.5,
                bias: 0.5,
                opacity: 0.5,
                blend_mode: FormulaBlendMode::Screen,
                ..FormulaLayer::default()
            });
        }
    });

    let mut layer_action = None;
    let can_remove = formula.layers.len() > 1;
    for (index, layer) in formula.layers.iter_mut().enumerate() {
        formula_layer_editor(ui, id_prefix, index, layer, can_remove, &mut layer_action);
    }
    if let Some(action) = layer_action {
        apply_formula_layer_action(&mut formula.layers, action);
    }
}

pub fn source_buttons(
    ui: &mut egui::Ui,
    source_target: FormulaSourceTarget,
    actions: &mut Vec<UiAction>,
) {
    if style::primary_sized_button(ui, "Save Source", style::SOURCE_ACTION_BUTTON_SIZE).clicked() {
        actions.push(UiAction::SaveFormulaSource(source_target));
    }
    if style::sized_button(ui, "Load Source", style::SOURCE_ACTION_BUTTON_SIZE).clicked() {
        actions.push(UiAction::LoadFormulaSource(source_target));
    }
}

fn formula_layer_editor(
    ui: &mut egui::Ui,
    id_prefix: &str,
    index: usize,
    layer: &mut FormulaLayer,
    can_remove: bool,
    layer_action: &mut Option<FormulaLayerAction>,
) {
    egui::CollapsingHeader::new(format!("Layer {}", index + 1))
        .id_salt(format!("{id_prefix}_formula_layer_{index}"))
        .default_open(true)
        .show(ui, |ui| {
            formula_layer_identity(ui, id_prefix, index, layer, can_remove, layer_action);
            layer_expression_section(ui, layer);
            layer_output_section(ui, layer);
            layer_transform_section(ui, layer);
            domain_pipeline_section(ui, id_prefix, index, layer);
        });
}

fn formula_layer_identity(
    ui: &mut egui::Ui,
    id_prefix: &str,
    index: usize,
    layer: &mut FormulaLayer,
    can_remove: bool,
    layer_action: &mut Option<FormulaLayerAction>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut layer.enabled, "");
        ui.add_sized([150.0, 20.0], egui::TextEdit::singleline(&mut layer.name));
        if index == 0 {
            ui.label("Base layer");
        } else {
            ui.label("Layer blend");
            egui::ComboBox::from_id_salt(format!("{id_prefix}_formula_layer_blend_{index}"))
                .selected_text(layer.blend_mode.label())
                .show_ui(ui, |ui| {
                    for blend_mode in FormulaBlendMode::ALL {
                        ui.selectable_value(&mut layer.blend_mode, blend_mode, blend_mode.label());
                    }
                });
        }
        if can_remove && ui.button("Remove").clicked() {
            *layer_action = Some(FormulaLayerAction::Remove(index));
        }
        if ui.button("Duplicate").clicked() {
            *layer_action = Some(FormulaLayerAction::Duplicate(index));
        }
        if index > 0 && ui.button("Up").clicked() {
            *layer_action = Some(FormulaLayerAction::MoveUp(index));
        }
        if ui.button("Down").clicked() {
            *layer_action = Some(FormulaLayerAction::MoveDown(index));
        }
    });
}

fn layer_expression_section(ui: &mut egui::Ui, layer: &mut FormulaLayer) {
    ui.separator();
    subsection_title(ui, "Expression");
    let mut expression = layer.expression.clone();
    let desired_rows = if is_fractal_dsl_text(&expression) {
        11
    } else {
        3
    };
    if ui
        .add(
            egui::TextEdit::multiline(&mut expression)
                .desired_rows(desired_rows)
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        layer.expression = expression;
    }
    show_validation(ui, "Expression", &layer.expression);
}

fn layer_output_section(ui: &mut egui::Ui, layer: &mut FormulaLayer) {
    ui.separator();
    subsection_title(ui, "Layer output");
    slider(ui, &mut layer.gain, -4.0..=4.0, "Layer gain");
    slider(ui, &mut layer.bias, -2.0..=2.0, "Layer bias");
    slider(ui, &mut layer.opacity, 0.0..=1.0, "Layer opacity");
}

fn layer_transform_section(ui: &mut egui::Ui, layer: &mut FormulaLayer) {
    ui.separator();
    subsection_title(ui, "Transform / Motion");
    slider(ui, &mut layer.repeat_x, 0.05..=32.0, "Repeat X");
    slider(ui, &mut layer.repeat_y, 0.05..=32.0, "Repeat Y");
    slider(ui, &mut layer.warp_x, -8.0..=8.0, "Warp X");
    slider(ui, &mut layer.warp_y, -8.0..=8.0, "Warp Y");
    slider(ui, &mut layer.offset_x, -8.0..=8.0, "Offset X");
    slider(ui, &mut layer.offset_y, -8.0..=8.0, "Offset Y");
    slider(ui, &mut layer.motion_x, -8.0..=8.0, "Motion X");
    slider(ui, &mut layer.motion_y, -8.0..=8.0, "Motion Y");
}

fn domain_pipeline_section(
    ui: &mut egui::Ui,
    id_prefix: &str,
    index: usize,
    layer: &mut FormulaLayer,
) {
    egui::CollapsingHeader::new("Domain pipeline")
        .id_salt(format!("{id_prefix}_domain_pipeline_{index}"))
        .default_open(true)
        .show(ui, |ui| {
            ui.label("Domain X");
            ui.add(egui::TextEdit::singleline(&mut layer.domain_x).desired_width(f32::INFINITY));
            show_validation(ui, "Domain X", &layer.domain_x);
            ui.label("Domain Y");
            ui.add(egui::TextEdit::singleline(&mut layer.domain_y).desired_width(f32::INFINITY));
            show_validation(ui, "Domain Y", &layer.domain_y);
            slider(
                ui,
                &mut layer.domain_influence,
                0.0..=1.0,
                "Domain influence",
            );
        });
}

#[derive(Debug, Clone, Copy)]
enum FormulaLayerAction {
    Remove(usize),
    Duplicate(usize),
    MoveUp(usize),
    MoveDown(usize),
}

fn apply_formula_layer_action(layers: &mut Vec<FormulaLayer>, action: FormulaLayerAction) {
    match action {
        FormulaLayerAction::Remove(index) if layers.len() > 1 && index < layers.len() => {
            layers.remove(index);
        }
        FormulaLayerAction::Duplicate(index) if index < layers.len() => {
            let mut duplicate = layers[index].clone();
            duplicate.name = format!("{} copy", duplicate.name);
            layers.insert(index + 1, duplicate);
        }
        FormulaLayerAction::MoveUp(index) if index > 0 && index < layers.len() => {
            layers.swap(index, index - 1);
        }
        FormulaLayerAction::MoveDown(index) if index + 1 < layers.len() => {
            layers.swap(index, index + 1);
        }
        _ => {}
    }
}

fn sync_formula_from_layers(formula: &mut FormulaSource) {
    if let Some(layer) = formula.layers.first() {
        formula.expression = layer.expression.clone();
    }
}

fn is_fractal_dsl_text(expression: &str) -> bool {
    expression
        .lines()
        .next()
        .map(|line| {
            let line = line.trim();
            line.eq_ignore_ascii_case("fractal") || line.eq_ignore_ascii_case("fractal {")
        })
        .unwrap_or(false)
}

fn subsection_title(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong().size(13.0));
}

fn formula_validation_error(expression: &str) -> Option<String> {
    CompiledFormula::compile(expression)
        .err()
        .map(|error| error.to_string())
}

fn show_validation(ui: &mut egui::Ui, label: &str, expression: &str) {
    if let Some(message) = formula_validation_error(expression) {
        ui.colored_label(
            egui::Color32::from_rgb(255, 130, 110),
            format!("{label}: {message}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FormulaEditorContext, formula_validation_error, normalize_formula_source_for_context,
    };
    use crate::render::{formula::FormulaSource, renderer::EffectBlendMode};

    #[test]
    fn pattern_source_editor_state_clears_effect_blend_mode() {
        let mut source = FormulaSource {
            effect_blend_mode: Some(EffectBlendMode::Difference),
            ..FormulaSource::pattern()
        };

        normalize_formula_source_for_context(&mut source, FormulaEditorContext::Pattern);

        assert_eq!(source.effect_blend_mode, None);
    }

    #[test]
    fn effect_source_editor_state_requires_effect_blend_mode() {
        let mut source = FormulaSource {
            effect_blend_mode: None,
            ..FormulaSource::pattern()
        };

        normalize_formula_source_for_context(
            &mut source,
            FormulaEditorContext::Effect {
                blend_mode: EffectBlendMode::Screen,
            },
        );

        assert_eq!(source.effect_blend_mode, Some(EffectBlendMode::Screen));
    }

    #[test]
    fn formula_field_validation_reports_expression_errors() {
        let error = formula_validation_error("bad_fn(x)").expect("invalid formula should error");

        assert!(error.contains("Unknown function"));
    }
}
