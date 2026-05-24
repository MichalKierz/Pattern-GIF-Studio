use crate::{
    export::progress::ExportProgress,
    project::{project_state::ProjectState, render_settings::RenderBackendStatus},
    ui::{UiAction, gif_output_panel, parameter_panel, preview_panel, style},
};

pub struct CreateTabStatus<'a> {
    pub texture: Option<&'a egui::TextureHandle>,
    pub preview_backend: Option<&'a RenderBackendStatus>,
    pub export_in_progress: bool,
    pub progress: Option<&'a ExportProgress>,
    pub runtime_message: Option<&'a str>,
}

pub fn show_create_tab(
    ui: &mut egui::Ui,
    project: &mut ProjectState,
    status: CreateTabStatus<'_>,
) -> Vec<UiAction> {
    let mut actions = Vec::new();

    ui.horizontal_top(|ui| {
        ui.set_min_height(ui.available_height());
        let gutter = 14.0;
        let column_width = ((ui.available_width() - gutter) * 0.5).max(320.0);

        ui.vertical(|ui| {
            ui.set_width(column_width);
            egui::ScrollArea::vertical()
                .id_salt("create_parameter_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    show_top_controls(ui, &mut actions);
                    ui.add_space(style::SECTION_SPACING);
                    gif_output_panel::show_gif_output_panel(ui, &mut project.export_settings);

                    ui.add_space(style::SECTION_SPACING);
                    parameter_panel::show_shape_parameters(ui, &mut project.render_params);

                    ui.add_space(style::SECTION_SPACING);
                    parameter_panel::show_parameter_panel(
                        ui,
                        &mut project.render_params,
                        &mut actions,
                    );
                });
        });

        ui.add_space(gutter);
        ui.vertical(|ui| {
            ui.set_width(column_width);
            preview_panel::show_preview_panel(
                ui,
                status.texture,
                status.preview_backend,
                egui::vec2(
                    project.export_settings.width as f32,
                    project.export_settings.height as f32,
                ),
            );
            ui.add_space(12.0);
            show_save_gif_row(
                ui,
                &mut actions,
                project,
                status.export_in_progress,
                status.progress,
                status.runtime_message,
            );
        });
    });

    actions
}

fn show_top_controls(ui: &mut egui::Ui, actions: &mut Vec<UiAction>) {
    ui.horizontal_wrapped(|ui| {
        if style::primary_button(ui, "Load Workflow").clicked() {
            actions.push(UiAction::LoadWorkflow);
        }
        if ui.button("Save Workflow").clicked() {
            actions.push(UiAction::SaveWorkflow);
        }
    });
}

fn show_save_gif_row(
    ui: &mut egui::Ui,
    actions: &mut Vec<UiAction>,
    project: &ProjectState,
    export_in_progress: bool,
    progress: Option<&ExportProgress>,
    runtime_message: Option<&str>,
) {
    ui.vertical(|ui| {
        ui.horizontal_wrapped(|ui| {
            if style::primary_button_enabled(ui, !export_in_progress, "Save GIF").clicked() {
                actions.push(UiAction::SaveGif);
            }
            if export_in_progress && ui.button("Cancel").clicked() {
                actions.push(UiAction::CancelExport);
            }
            ui.label(gif_output_panel::estimated_file_size_label(
                &project.export_settings,
            ));
        });

        if export_in_progress || progress.is_some() {
            ui.add_space(4.0);
            show_export_status(ui, progress);
        }
        if let Some(message) = runtime_message.filter(|message| !message.trim().is_empty()) {
            ui.add_space(2.0);
            style::muted_label(ui, format!("Status: {message}"));
        }
    });
}

fn show_export_status(ui: &mut egui::Ui, progress: Option<&ExportProgress>) {
    let label = export_status_label(progress);
    if let Some(fraction) = progress.and_then(ExportProgress::fraction) {
        let width = ui.available_width().min(540.0);
        ui.add_sized(
            [width, 18.0],
            egui::ProgressBar::new(fraction.clamp(0.0, 1.0)).text(label),
        );
    } else if progress.is_some_and(|progress| matches!(progress, ExportProgress::Failed { .. })) {
        ui.colored_label(egui::Color32::from_rgb(255, 130, 110), label);
    } else {
        style::muted_label(ui, label);
    }
}

fn export_status_label(progress: Option<&ExportProgress>) -> String {
    progress
        .map(ExportProgress::label)
        .unwrap_or_else(|| "Preparing export".to_owned())
}

#[cfg(test)]
mod tests {
    use super::export_status_label;
    use crate::export::progress::ExportProgress;

    #[test]
    fn export_status_label_keeps_terminal_states_distinct() {
        assert!(export_status_label(None).contains("Preparing"));

        let finished = ExportProgress::Finished {
            output_path: "done.gif".into(),
        };
        assert!(export_status_label(Some(&finished)).contains("Finished"));
        assert!(export_status_label(Some(&ExportProgress::Cancelled)).contains("cancelled"));

        let failed = ExportProgress::Failed {
            message: "write denied".to_owned(),
        };
        let label = export_status_label(Some(&failed));
        assert!(label.contains("failed"));
        assert!(label.contains("write denied"));
    }
}
