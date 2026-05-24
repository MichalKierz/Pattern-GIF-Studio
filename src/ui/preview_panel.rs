use crate::{project::render_settings::RenderBackendStatus, ui::style};

pub fn show_preview_panel(
    ui: &mut egui::Ui,
    texture: Option<&egui::TextureHandle>,
    backend_status: Option<&RenderBackendStatus>,
    preview_size: egui::Vec2,
) {
    ui.label(egui::RichText::new("Live Preview").strong().size(18.0));
    if let Some(status) = backend_status {
        style::muted_label(ui, format!("Renderer: {}", status.short_label()));
    }
    ui.add_space(8.0);

    let available = ui.available_size();
    let max_width = available.x.min(520.0);
    let max_height = available.y.min(520.0);
    let scale = (max_width / preview_size.x)
        .min(max_height / preview_size.y)
        .clamp(0.1, 1.0);
    let size = egui::vec2(
        (preview_size.x * scale).max(160.0).min(max_width),
        (preview_size.y * scale).max(120.0).min(max_height),
    );

    match texture {
        Some(texture) => {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(8, 10, 13))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(48, 57, 70)))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.add(egui::Image::new((texture.id(), size)));
                });
        }
        None => {
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Preparing preview",
                egui::FontId::proportional(16.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
}
