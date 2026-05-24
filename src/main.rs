#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use pattern_gif_studio::app::PatternGifApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([980.0, 640.0]),
        renderer: eframe::Renderer::Wgpu,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        ..Default::default()
    };

    eframe::run_native(
        "Pattern GIF Studio",
        options,
        Box::new(|cc| Ok(Box::new(PatternGifApp::new(cc)))),
    )
}
