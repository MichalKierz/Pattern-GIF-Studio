use crate::{
    render::renderer::RenderParams,
    ui::{controls::slider, style},
};

pub fn show_shape_parameters(ui: &mut egui::Ui, params: &mut RenderParams) {
    style::section(ui, "Shape Parameters", |ui| {
        ui.add(egui::DragValue::new(&mut params.seed).prefix("Seed "));
        slider(ui, &mut params.zoom, 0.1..=12.0, "Zoom");
        slider(ui, &mut params.center_x, -2.0..=2.0, "Center X");
        slider(ui, &mut params.center_y, -2.0..=2.0, "Center Y");
        slider(
            ui,
            &mut params.rotation_speed,
            -4.0..=4.0,
            "Looped rotation",
        );
        slider(ui, &mut params.smoothing, 0.0..=20.0, "Smoothing");
        slider(
            ui,
            &mut params.smoothing_radius_pixels,
            0.0..=10.0,
            "Smoothing radius px",
        );
    });
}
