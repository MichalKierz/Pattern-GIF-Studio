use crate::{export::export_settings::ExportSettings, ui::style};

pub fn show_gif_output_panel(ui: &mut egui::Ui, settings: &mut ExportSettings) {
    style::section(ui, "GIF Output", |ui| {
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut settings.width)
                    .range(64..=ExportSettings::MAX_DIMENSION)
                    .prefix("W ")
                    .speed(8.0),
            );
            ui.add(
                egui::DragValue::new(&mut settings.height)
                    .range(64..=ExportSettings::MAX_DIMENSION)
                    .prefix("H ")
                    .speed(8.0),
            );
        });
        ui.add(egui::Slider::new(&mut settings.fps, 1..=ExportSettings::MAX_GIF_FPS).text("FPS"));
        ui.add(egui::Slider::new(&mut settings.duration_seconds, 0.25..=30.0).text("Seconds"));
        ui.add(egui::Slider::new(&mut settings.lossy_quality, 1..=100).text("Lossy quality"))
            .on_hover_text(
                "Affects the GIF palette approximation used by Live Preview and export.",
            );
        ui.checkbox(&mut settings.fast, "Fast encode")
            .on_hover_text("Uses a faster palette encoder. Export is quicker, but fine gradients can lose quality.");
    });
}

pub fn estimated_file_size_label(settings: &ExportSettings) -> String {
    format!(
        "Approx. {}",
        format_bytes(estimated_file_size_bytes(settings))
    )
}

pub fn estimated_file_size_bytes(settings: &ExportSettings) -> u64 {
    let pixels = settings.width as f64 * settings.height as f64;
    let frames = settings.total_frames() as f64;
    let quality = settings.lossy_quality.clamp(1, 100) as f64 / 100.0;
    let bytes_per_pixel = 0.72 + quality * 0.24 + if settings.fast { 0.02 } else { 0.0 };
    let frame_overhead = 1_250.0 * frames;
    let file_overhead = 4_096.0;
    (pixels * frames * bytes_per_pixel + frame_overhead + file_overhead)
        .max(1024.0)
        .round() as u64
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    if bytes as f64 >= MB {
        format!("{:.1} MB", bytes as f64 / MB)
    } else {
        format!("{:.0} KB", bytes as f64 / KB)
    }
}

#[cfg(test)]
mod tests {
    use super::{estimated_file_size_bytes, estimated_file_size_label};
    use crate::export::export_settings::ExportSettings;

    #[test]
    fn estimate_is_calibrated_for_high_detail_500px_fast_gif() {
        let settings = ExportSettings {
            width: 500,
            height: 500,
            fps: 24,
            duration_seconds: 2.0,
            lossy_quality: 100,
            fast: true,
            ..ExportSettings::default()
        };

        let estimated = estimated_file_size_bytes(&settings);

        assert!(
            (11_300_000..=12_300_000).contains(&estimated),
            "estimate should be close to the observed 11.7 MB high-detail export, got {estimated}"
        );
    }

    #[test]
    fn estimate_scales_with_frame_count_and_resolution() {
        let base = ExportSettings {
            width: 250,
            height: 250,
            fps: 12,
            duration_seconds: 1.0,
            ..ExportSettings::default()
        };
        let larger = ExportSettings {
            width: 500,
            height: 500,
            fps: 24,
            duration_seconds: 2.0,
            ..base.clone()
        };

        assert!(estimated_file_size_bytes(&larger) > estimated_file_size_bytes(&base) * 12);
    }

    #[test]
    fn estimate_label_is_marked_as_approximate() {
        let label = estimated_file_size_label(&ExportSettings::default());

        assert!(
            label.starts_with("Approx. "),
            "size estimate label must not imply byte-accurate precision: {label}"
        );
    }
}
