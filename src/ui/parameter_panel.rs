use crate::{
    render::renderer::RenderParams,
    ui::{UiAction, colors_panel, effects_panel, patterns_panel, shape_panel, style},
};

pub fn show_parameter_panel(
    ui: &mut egui::Ui,
    params: &mut RenderParams,
    actions: &mut Vec<UiAction>,
) {
    params.activate_editable_sources();

    patterns_panel::show_patterns(ui, params, actions);

    ui.add_space(style::SECTION_SPACING);
    effects_panel::show_effects(ui, params, actions);

    ui.add_space(style::SECTION_SPACING);
    colors_panel::show_colors(ui, params, actions);
}

pub fn show_shape_parameters(ui: &mut egui::Ui, params: &mut RenderParams) {
    shape_panel::show_shape_parameters(ui, params);
}
