use crate::{
    render::{
        color::{ColorTransition, CustomGradient},
        renderer::RenderParams,
    },
    ui::{UiAction, controls::slider, style},
};

pub fn show_colors(ui: &mut egui::Ui, params: &mut RenderParams, actions: &mut Vec<UiAction>) {
    style::section(ui, "Colors", |ui| {
        params.normalize_color_source();
        color_asset_row(ui, actions);
        custom_color_controls(ui, &mut params.custom_gradient);
        slider(
            ui,
            &mut params.color_speed,
            -6.0..=6.0,
            "Looped color motion",
        );
        slider(ui, &mut params.color_phase, 0.0..=1.0, "Color phase");
        slider(ui, &mut params.brightness, 0.1..=2.0, "Brightness");
        slider(ui, &mut params.contrast, 0.1..=3.0, "Contrast");
    });
}

fn color_asset_row(ui: &mut egui::Ui, actions: &mut Vec<UiAction>) {
    ui.horizontal_wrapped(|ui| {
        if style::primary_button(ui, "Save").clicked() {
            actions.push(UiAction::SaveCustomColorSet);
        }
        if ui.button("Load").clicked() {
            actions.push(UiAction::LoadCustomColorSet);
        }
    });
}

fn custom_color_controls(ui: &mut egui::Ui, gradient: &mut CustomGradient) {
    gradient.enabled = true;
    gradient.ensure_color_stops();
    if style::primary_button(ui, "Add").clicked() {
        gradient.add_color();
    }
    let mut remove_index = None;
    let mut add_transition_index = None;
    let mut remove_transition_index = None;
    for index in 0..gradient.colors.len() {
        let can_remove_color = gradient.colors.len() > 2;
        color_stop_row(
            ui,
            index,
            &mut gradient.colors[index],
            can_remove_color,
            &mut add_transition_index,
            &mut remove_index,
        );
        transition_strip(
            ui,
            index,
            &mut gradient.color_transitions[index],
            &mut remove_transition_index,
        );
        ui.add_space(6.0);
    }
    if let Some(index) = add_transition_index {
        gradient.add_transition(index);
    }
    if let Some((color_index, transition_index)) = remove_transition_index {
        gradient.remove_transition(color_index, transition_index);
    }
    if let Some(index) = remove_index {
        gradient.remove_color(index);
    }
    gradient.ensure_color_stops();
}

fn color_stop_row(
    ui: &mut egui::Ui,
    index: usize,
    color: &mut [f32; 3],
    can_remove: bool,
    add_transition_index: &mut Option<usize>,
    remove_index: &mut Option<usize>,
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [COLOR_LABEL_WIDTH, 20.0],
            egui::Label::new(egui::RichText::new(color_label(index)).strong()),
        );
        gradient_color(ui, color);
        ui.add_space(8.0);
        if ui
            .button("Add Transition")
            .on_hover_text("Add an animated color for this stop")
            .clicked()
        {
            *add_transition_index = Some(index);
        }
        if can_remove && ui.button("Remove").clicked() {
            *remove_index = Some(index);
        }
    });
}

fn transition_strip(
    ui: &mut egui::Ui,
    color_index: usize,
    transitions: &mut [ColorTransition],
    remove_transition_index: &mut Option<(usize, usize)>,
) {
    if transitions.is_empty() {
        return;
    }

    ui.vertical(|ui| {
        for (transition_index, transition) in transitions.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add_space(COLOR_LABEL_WIDTH + 18.0);
                ui.label(
                    egui::RichText::new(format!("T{}", transition_index + 1))
                        .color(egui::Color32::from_rgb(155, 165, 178)),
                );
                gradient_color(ui, &mut transition.color);
                if ui
                    .small_button("X")
                    .on_hover_text("Remove transition")
                    .clicked()
                {
                    *remove_transition_index = Some((color_index, transition_index));
                }
            });
        }
    });
}

const COLOR_LABEL_WIDTH: f32 = 52.0;

fn color_label(index: usize) -> String {
    if index < 26 {
        ((b'A' + index as u8) as char).to_string()
    } else {
        format!("Color {}", index + 1)
    }
}

fn gradient_color(ui: &mut egui::Ui, color: &mut [f32; 3]) {
    let mut color32 = egui::Color32::from_rgb(
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
    );
    if ui.color_edit_button_srgba(&mut color32).changed() {
        color[0] = color32.r() as f32 / 255.0;
        color[1] = color32.g() as f32 / 255.0;
        color[2] = color32.b() as f32 / 255.0;
    }
}
