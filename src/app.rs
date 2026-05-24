use std::{
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;

use crate::{
    export::{
        export_settings::ExportSettings,
        gif_exporter::export_gif,
        gif_quality::apply_gif_quality_preview,
        history::{ExportHistoryEntry, load_history, push_history_entry},
        progress::ExportProgress,
    },
    project::{
        app_settings::AppSettings,
        project_state::ProjectState,
        render_settings::RenderBackendStatus,
        session_state::{SessionState, load_session, save_session},
        workflow::{WorkflowFile, load_workflow_or_preset, save_workflow},
    },
    render::backend::BackendRenderers,
    render::backend::FrameRenderSpec,
    render::renderer::{EffectLayer, PatternLayer, RenderParams},
    source_assets::{
        asset::{AssetType, CustomColorSet, FormulaSourceAsset, validate_legacy_formula_source},
        storage::save_asset_to_path,
    },
    ui::{FormulaSourceTarget, UiAction, create_tab, style},
    utils::{
        app_log::append_log,
        file_dialog::save_json_file,
        file_dialog::{pick_gif_output, pick_json_file},
        image_utils::frame_to_color_image,
        portable_paths::portable_file_or_default,
    },
};

const MAX_PREVIEW_RENDER_DIMENSION: u32 = 512;

pub struct PatternGifApp {
    pub project: ProjectState,
    pub preview_texture: Option<egui::TextureHandle>,
    pub export_progress: Option<ExportProgress>,

    app_settings: AppSettings,
    started_at: Instant,
    last_preview_request: Option<Instant>,
    preview_request_in_flight: bool,
    preview_generation: u64,
    last_preview_fingerprint: Option<String>,
    preview_request_sender: mpsc::Sender<PreviewRequest>,
    preview_frame_receiver: Receiver<PreviewFrame>,
    export_receiver: Option<Receiver<ExportProgress>>,
    export_cancel: Option<Arc<AtomicBool>>,
    export_in_progress: bool,
    export_history: Vec<ExportHistoryEntry>,
    status_message: String,
    last_preview_backend: Option<RenderBackendStatus>,
    last_export_backend: Option<RenderBackendStatus>,
    last_session_save: Instant,
}

impl PatternGifApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::apply_visuals(&cc.egui_ctx);

        let app_settings = AppSettings::portable().unwrap_or_else(|_| fallback_settings());

        let session = load_session(&app_settings.app_data_dir).ok().flatten();
        let mut project = session
            .as_ref()
            .map(|session| session.project.clone())
            .unwrap_or_default();
        if session.is_none() {
            project.export_settings.output_path = app_settings.exports_dir.join("pattern-loop.gif");
        }
        ensure_project_paths_portable(&mut project, &app_settings);

        let (preview_request_sender, preview_request_receiver) = mpsc::channel();
        let (preview_frame_sender, preview_frame_receiver) = mpsc::channel();
        spawn_preview_worker(preview_request_receiver, preview_frame_sender);

        let export_history = load_history(&app_settings.app_data_dir);

        Self {
            project,
            preview_texture: None,
            export_progress: None,
            app_settings,
            started_at: Instant::now(),
            last_preview_request: None,
            preview_request_in_flight: false,
            preview_generation: 0,
            last_preview_fingerprint: None,
            preview_request_sender,
            preview_frame_receiver,
            export_receiver: None,
            export_cancel: None,
            export_in_progress: false,
            export_history,
            status_message: String::new(),
            last_preview_backend: None,
            last_export_backend: None,
            last_session_save: Instant::now(),
        }
    }

    fn render_preview(&mut self, ctx: &egui::Context) {
        self.project.sanitize();
        self.invalidate_preview_if_state_changed();
        while let Ok(preview_frame) = self.preview_frame_receiver.try_recv() {
            if preview_frame.generation != self.preview_generation {
                continue;
            }
            self.last_preview_backend = Some(preview_frame.backend_status);
            let image = frame_to_color_image(&preview_frame.frame);
            match &mut self.preview_texture {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    self.preview_texture =
                        Some(ctx.load_texture("live-preview", image, egui::TextureOptions::LINEAR));
                }
            }
            self.preview_request_in_flight = false;
        }

        let now = Instant::now();
        let frame_interval = Duration::from_secs_f32(1.0 / self.project.export_settings.fps as f32);
        if self.preview_request_in_flight {
            return;
        }
        if self
            .last_preview_request
            .as_ref()
            .is_some_and(|last_request| {
                now.duration_since(*last_request) < frame_interval && self.preview_texture.is_some()
            })
        {
            return;
        }
        self.last_preview_request = Some(now);

        let total_frames = self.project.export_settings.total_frames();
        let elapsed = self.started_at.elapsed().as_secs_f64();
        let duration = self.project.export_settings.duration_seconds.max(0.25) as f64;
        let phase = (elapsed / duration).rem_euclid(1.0);
        let frame_index = ((phase * total_frames as f64).floor() as u32).min(total_frames - 1);
        let (width, height) = preview_render_dimensions(
            self.project.export_settings.width,
            self.project.export_settings.height,
        );
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let request = PreviewRequest {
            generation: self.preview_generation,
            render_params: self.project.render_params.clone(),
            export_settings: self.project.export_settings.clone(),
            frame_index,
            total_frames,
            width,
            height,
        };
        if self.preview_request_sender.send(request).is_ok() {
            self.preview_request_in_flight = true;
        }
    }

    fn invalidate_preview_if_state_changed(&mut self) {
        let fingerprint = preview_state_fingerprint(&self.project);
        if self.last_preview_fingerprint.as_ref() == Some(&fingerprint) {
            return;
        }

        self.last_preview_fingerprint = Some(fingerprint);
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.preview_request_in_flight = false;
        self.last_preview_request = None;
    }

    fn handle_action(&mut self, action: UiAction) {
        match action {
            UiAction::SaveWorkflow => self.save_workflow_via_dialog(),
            UiAction::LoadWorkflow => self.load_workflow_via_dialog(),
            UiAction::SaveFormulaSource(target) => self.save_formula_source(target),
            UiAction::LoadFormulaSource(target) => self.load_formula_source(target),
            UiAction::AddPattern => self.add_pattern_layer(),
            UiAction::RemovePattern(index) => self.remove_pattern_layer(index),
            UiAction::AddEffect => self.add_effect_layer(),
            UiAction::RemoveEffect(index) => self.remove_effect_layer(index),
            UiAction::SaveCustomColorSet => self.save_custom_color_set(),
            UiAction::LoadCustomColorSet => self.load_custom_color_set(),
            UiAction::SaveGif => self.save_gif_via_dialog(),
            UiAction::CancelExport => self.cancel_export(),
        }
    }

    fn save_gif_via_dialog(&mut self) {
        if self.export_in_progress {
            return;
        }

        if let Some(path) = pick_gif_output(self.project.export_settings.output_path.clone()) {
            self.project.export_settings.output_path = path;
            self.start_export();
        }
    }

    fn save_workflow_via_dialog(&mut self) {
        let default_path = self
            .app_settings
            .bundled_workflow_presets_dir
            .join("workflow.json");
        let Some(path) = save_json_file(default_path, "Workflow JSON") else {
            return;
        };
        let path = portable_file_or_default(
            path,
            &self.app_settings.portable_root,
            &self
                .app_settings
                .bundled_workflow_presets_dir
                .join("workflow.json"),
        );
        let mut project = self.project.clone();
        project.sanitize();
        ensure_project_paths_portable(&mut project, &self.app_settings);
        let workflow = WorkflowFile::from_project(project);
        match save_workflow(&path, &workflow) {
            Ok(path) => {
                let _ = path;
                self.status_message.clear();
            }
            Err(error) => {
                let _ = append_log(
                    &self.app_settings.app_data_dir,
                    format!("workflow save: {error}"),
                );
                self.status_message = format!("Workflow save failed: {error}");
            }
        }
    }

    fn load_workflow_via_dialog(&mut self) {
        let Some(path) = pick_json_file(
            self.app_settings.bundled_workflow_presets_dir.clone(),
            "Workflow JSON",
        ) else {
            return;
        };
        match load_workflow_or_preset(&path) {
            Ok(workflow) => {
                self.project = workflow.project;
                ensure_project_paths_portable(&mut self.project, &self.app_settings);
                self.status_message.clear();
            }
            Err(error) => {
                let _ = append_log(
                    &self.app_settings.app_data_dir,
                    format!("workflow load: {error}"),
                );
                self.status_message = format!("Workflow load failed: {error}");
            }
        }
    }

    fn save_formula_source(&mut self, target: FormulaSourceTarget) {
        let Some(mut source) = self.formula_source(target).cloned() else {
            return;
        };
        if let FormulaSourceTarget::Effect(index) = target {
            source.effect_blend_mode = self
                .project
                .render_params
                .effects
                .get(index)
                .map(|layer| layer.blend_mode);
        }
        let label = formula_source_label(target);
        let issues = source.validate(label);
        if let Some(issue) = issues.first() {
            self.status_message = format!("Fix source formula before saving: {}", issue.message);
            return;
        }

        let default_path = self
            .formula_source_dir(target)
            .join(format!("{}.json", formula_source_file_stem(target)));
        let Some(path) = save_json_file(default_path, "Formula Source JSON") else {
            return;
        };
        let path = portable_file_or_default(
            path,
            &self.app_settings.portable_root,
            &self
                .formula_source_dir(target)
                .join(format!("{}.json", formula_source_file_stem(target))),
        );
        let name = file_stem_or(&path, label);
        let asset = match target {
            FormulaSourceTarget::Pattern(_) => FormulaSourceAsset::new_pattern(name, source),
            FormulaSourceTarget::Effect(_) => FormulaSourceAsset::new_effect(name, source),
        };
        match save_asset_to_path(&path, &asset) {
            Ok(path) => {
                let _ = path;
                self.status_message.clear();
            }
            Err(error) => self.report_asset_error("formula source save", error),
        }
    }

    fn load_formula_source(&mut self, target: FormulaSourceTarget) {
        let Some(path) = pick_json_file(
            self.formula_source_load_dir(target),
            "Preset or Source JSON",
        ) else {
            return;
        };
        match load_formula_source_from_path(&path, target) {
            Ok((name, source)) => {
                self.apply_formula_source(target, name.clone(), source);
                self.status_message.clear();
            }
            Err(error) => self.report_asset_error("formula source load", error),
        }
    }

    fn save_custom_color_set(&mut self) {
        let default_path = self
            .app_settings
            .bundled_color_set_presets_dir
            .join("color-set.json");
        let Some(path) = save_json_file(default_path, "Color Set JSON") else {
            return;
        };
        let path = portable_file_or_default(
            path,
            &self.app_settings.portable_root,
            &self
                .app_settings
                .bundled_color_set_presets_dir
                .join("color-set.json"),
        );
        let name = file_stem_or(&path, "Color Set");
        let asset = CustomColorSet::from_render_params(name, &self.project.render_params);
        match save_asset_to_path(&path, &asset) {
            Ok(path) => {
                let _ = path;
                self.status_message.clear();
            }
            Err(error) => self.report_asset_error("custom color set save", error),
        }
    }

    fn load_custom_color_set(&mut self) {
        let Some(path) = pick_json_file(self.color_set_load_dir(), "Preset or Color Set JSON")
        else {
            return;
        };
        match load_color_set_from_path(&path) {
            Ok(asset) => {
                asset.apply_to_render_params(&mut self.project.render_params);
                self.status_message.clear();
            }
            Err(error) => self.report_asset_error("custom color set load", error),
        }
    }

    fn formula_source(
        &self,
        target: FormulaSourceTarget,
    ) -> Option<&crate::render::formula::FormulaSource> {
        match target {
            FormulaSourceTarget::Pattern(index) => self
                .project
                .render_params
                .patterns
                .get(index)
                .map(|layer| &layer.source),
            FormulaSourceTarget::Effect(index) => self
                .project
                .render_params
                .effects
                .get(index)
                .map(|layer| &layer.source),
        }
    }

    fn apply_formula_source(
        &mut self,
        target: FormulaSourceTarget,
        name: String,
        source: crate::render::formula::FormulaSource,
    ) {
        apply_formula_source_to_project(&mut self.project, target, name, source);
    }

    fn formula_source_dir(&self, target: FormulaSourceTarget) -> std::path::PathBuf {
        match target {
            FormulaSourceTarget::Pattern(_) => {
                self.app_settings.bundled_pattern_presets_dir.clone()
            }
            FormulaSourceTarget::Effect(_) => self.app_settings.bundled_effect_presets_dir.clone(),
        }
    }

    fn formula_source_load_dir(&self, target: FormulaSourceTarget) -> std::path::PathBuf {
        self.formula_source_dir(target)
    }

    fn color_set_load_dir(&self) -> std::path::PathBuf {
        self.app_settings.bundled_color_set_presets_dir.clone()
    }

    fn add_pattern_layer(&mut self) {
        let index = self.project.render_params.patterns.len() + 1;
        self.project.render_params.patterns.push(PatternLayer::new(
            format!("Pattern {index}"),
            crate::render::formula::FormulaSource::fractal_a(),
        ));
    }

    fn remove_pattern_layer(&mut self, index: usize) {
        if self.project.render_params.patterns.len() > 1
            && index < self.project.render_params.patterns.len()
        {
            self.project.render_params.patterns.remove(index);
        }
    }

    fn add_effect_layer(&mut self) {
        let index = self.project.render_params.effects.len() + 1;
        self.project.render_params.effects.push(EffectLayer::new(
            format!("Effect {index}"),
            crate::render::formula::FormulaSource::pattern(),
        ));
    }

    fn remove_effect_layer(&mut self, index: usize) {
        if index < self.project.render_params.effects.len() {
            self.project.render_params.effects.remove(index);
        }
    }

    fn report_asset_error(&mut self, context: &str, error: anyhow::Error) {
        let _ = append_log(
            &self.app_settings.app_data_dir,
            format!("{context}: {error}"),
        );
        self.status_message = format!("{context} failed: {error}");
    }

    fn start_export(&mut self) {
        if self.export_in_progress {
            return;
        }

        self.project.sanitize();
        let formula_issues = self.project.render_params.formula_issues();
        if !formula_issues.is_empty() {
            self.status_message = format!(
                "Export blocked by formula error in {}: {}",
                formula_issues[0].label, formula_issues[0].message
            );
            return;
        }
        let project = self.project.clone();
        let (tx, rx) = mpsc::channel();
        let fail_tx = tx.clone();
        let cancel = Arc::new(AtomicBool::new(false));

        self.export_receiver = Some(rx);
        self.export_cancel = Some(Arc::clone(&cancel));
        self.export_progress = Some(ExportProgress::Started {
            total_frames: project.export_settings.total_frames(),
        });
        self.export_in_progress = true;
        self.last_export_backend = None;
        self.status_message.clear();

        thread::spawn(move || {
            if let Err(error) = export_gif(project, tx, cancel) {
                let _ = fail_tx.send(ExportProgress::Failed {
                    message: error.to_string(),
                });
            }
        });
    }

    fn cancel_export(&mut self) {
        if let Some(cancel) = &self.export_cancel {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.status_message.clear();
        }
    }

    fn poll_export_progress(&mut self) {
        let Some(receiver) = &self.export_receiver else {
            return;
        };

        let mut terminal = false;
        while let Ok(progress) = receiver.try_recv() {
            if let ExportProgress::BackendStatus { status } = &progress {
                self.last_export_backend = Some(status.clone());
                continue;
            }
            match &progress {
                ExportProgress::Finished { .. }
                | ExportProgress::Cancelled
                | ExportProgress::Failed { .. } => {
                    self.export_in_progress = false;
                    terminal = true;
                }
                _ => {}
            }
            if matches!(&progress, ExportProgress::Finished { .. }) {
                let entry = portable_history_entry(
                    ExportHistoryEntry::from_settings(&self.project.export_settings),
                    &self.app_settings,
                );
                if let Err(error) = push_history_entry(
                    &self.app_settings.app_data_dir,
                    &mut self.export_history,
                    entry,
                ) {
                    let _ = append_log(
                        &self.app_settings.app_data_dir,
                        format!("export history save: {error}"),
                    );
                    self.status_message = format!("Export history save failed: {error}");
                }
            }
            self.export_progress = Some(progress);
        }
        if terminal {
            self.export_cancel = None;
            self.export_receiver = None;
        }
    }

    fn maybe_save_session(&mut self) {
        if self.last_session_save.elapsed() < Duration::from_secs(2) {
            return;
        }

        let mut project = self.project.clone();
        ensure_project_paths_portable(&mut project, &self.app_settings);
        let session = SessionState::from_parts(project);
        if let Err(error) = save_session(&self.app_settings.app_data_dir, &session) {
            let _ = append_log(
                &self.app_settings.app_data_dir,
                format!("session save: {error}"),
            );
            self.status_message = format!("Session save failed: {error}");
        }
        self.last_session_save = Instant::now();
    }
}

impl eframe::App for PatternGifApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_export_progress();
        self.render_preview(&ctx);
        self.maybe_save_session();

        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 8,
                right: 0,
                top: 6,
                bottom: 2,
            })
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Pattern GIF Studio")
                        .strong()
                        .size(20.0),
                );
            });
        ui.separator();

        let actions = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 16,
                right: 14,
                top: 14,
                bottom: 12,
            })
            .show(ui, |ui| {
                create_tab::show_create_tab(
                    ui,
                    &mut self.project,
                    create_tab::CreateTabStatus {
                        texture: self.preview_texture.as_ref(),
                        preview_backend: self.last_preview_backend.as_ref(),
                        export_in_progress: self.export_in_progress,
                        progress: self.export_progress.as_ref(),
                        runtime_message: Some(self.status_message.as_str()),
                    },
                )
            })
            .inner;

        for action in actions {
            self.handle_action(action);
        }

        ctx.request_repaint();
    }
}

fn fallback_settings() -> AppSettings {
    let root = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        .expect("failed to identify portable executable directory");
    AppSettings::for_root(root).expect("failed to initialize portable app directories")
}

fn ensure_project_paths_portable(project: &mut ProjectState, settings: &AppSettings) {
    project.export_settings.output_path = portable_file_or_default(
        project.export_settings.output_path.clone(),
        &settings.portable_root,
        &settings.exports_dir.join("pattern-loop.gif"),
    );
}

fn apply_formula_source_to_project(
    project: &mut ProjectState,
    target: FormulaSourceTarget,
    name: String,
    source: crate::render::formula::FormulaSource,
) {
    match target {
        FormulaSourceTarget::Pattern(index) => {
            if let Some(layer) = project.render_params.patterns.get_mut(index) {
                if !name.trim().is_empty() {
                    layer.name = name;
                }
                layer.source = source;
            }
        }
        FormulaSourceTarget::Effect(index) => {
            if let Some(layer) = project.render_params.effects.get_mut(index) {
                if !name.trim().is_empty() {
                    layer.name = name;
                }
                if let Some(blend_mode) = source.effect_blend_mode {
                    layer.blend_mode = blend_mode;
                }
                layer.source = source;
            }
        }
    }
}

fn portable_history_entry(
    mut entry: ExportHistoryEntry,
    settings: &AppSettings,
) -> ExportHistoryEntry {
    entry.output_path = portable_file_or_default(
        entry.output_path,
        &settings.portable_root,
        &settings.exports_dir.join("pattern-loop.gif"),
    );
    entry
}

#[derive(Serialize)]
struct PreviewStateFingerprint<'a> {
    render_params: &'a RenderParams,
    export_settings: PreviewExportFingerprint,
}

#[derive(Serialize)]
struct PreviewExportFingerprint {
    width: u32,
    height: u32,
    fps: u32,
    duration_seconds: f32,
    lossy_quality: u8,
    fast: bool,
}

fn preview_state_fingerprint(project: &ProjectState) -> String {
    let mut project = project.clone();
    project.sanitize();
    let fingerprint = PreviewStateFingerprint {
        render_params: &project.render_params,
        export_settings: PreviewExportFingerprint {
            width: project.export_settings.width,
            height: project.export_settings.height,
            fps: project.export_settings.fps,
            duration_seconds: project.export_settings.duration_seconds,
            lossy_quality: project.export_settings.lossy_quality,
            fast: project.export_settings.fast,
        },
    };
    serde_json::to_string(&fingerprint)
        .unwrap_or_else(|error| format!("preview-fingerprint-serialization-error:{error}"))
}

struct PreviewRequest {
    generation: u64,
    render_params: RenderParams,
    export_settings: ExportSettings,
    frame_index: u32,
    total_frames: u32,
    width: u32,
    height: u32,
}

struct PreviewFrame {
    generation: u64,
    frame: crate::render::frame_buffer::FrameBuffer,
    backend_status: RenderBackendStatus,
}

fn spawn_preview_worker(receiver: Receiver<PreviewRequest>, sender: mpsc::Sender<PreviewFrame>) {
    thread::spawn(move || {
        let mut renderers = BackendRenderers::new();

        while let Ok(mut request) = receiver.recv() {
            while let Ok(newer_request) = receiver.try_recv() {
                request = newer_request;
            }

            let spec = FrameRenderSpec::new(
                request.frame_index,
                request.total_frames,
                request.width,
                request.height,
            );
            let rendered = renderers.render_indexed_frame(&request.render_params, spec);
            let (frame, backend_status) = match rendered {
                Ok(rendered) => (
                    apply_gif_quality_preview(rendered.frame, &request.export_settings),
                    rendered.status,
                ),
                Err(error) => (
                    crate::render::frame_buffer::FrameBuffer::new(request.width, request.height),
                    RenderBackendStatus::gpu_error(error.to_string()),
                ),
            };
            if sender
                .send(PreviewFrame {
                    generation: request.generation,
                    frame,
                    backend_status,
                })
                .is_err()
            {
                break;
            }
        }
    });
}

fn load_formula_source_from_path(
    path: &std::path::Path,
    target: FormulaSourceTarget,
) -> anyhow::Result<(String, crate::render::formula::FormulaSource)> {
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;
    if let Some(asset_type) = value.get("asset_type") {
        let asset_type: AssetType = serde_json::from_value(asset_type.clone())?;
        let expected = formula_source_asset_type(target);
        if asset_type != expected {
            anyhow::bail!(
                "wrong asset type: expected {}, got {}",
                expected.label(),
                asset_type.label()
            );
        }
    }

    if value.get("source").is_some() {
        let asset: FormulaSourceAsset = serde_json::from_value(value)?;
        asset.validate_for_type(formula_source_asset_type(target))?;
        return Ok((asset.name, asset.source));
    }

    if value.get("render_params").is_some() || value.get("project").is_some() {
        let workflow = load_workflow_or_preset(path)?;
        return Ok(formula_source_from_render_params(
            &workflow.project.render_params,
            target,
        ));
    }

    if !looks_like_formula_source(&value) {
        anyhow::bail!(
            "wrong asset type: expected {}, but JSON is not a formula source asset",
            formula_source_asset_type(target).label()
        );
    }
    let source: crate::render::formula::FormulaSource = serde_json::from_value(value)?;
    validate_legacy_formula_source(&source, formula_source_asset_type(target))?;
    Ok((file_stem_or(path, formula_source_label(target)), source))
}

fn load_color_set_from_path(path: &std::path::Path) -> anyhow::Result<CustomColorSet> {
    let text = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&text)?;

    if let Some(asset_type) = value.get("asset_type") {
        let asset_type: AssetType = serde_json::from_value(asset_type.clone())?;
        if asset_type != AssetType::ColorSet {
            anyhow::bail!(
                "wrong asset type: expected {}, got {}",
                AssetType::ColorSet.label(),
                asset_type.label()
            );
        }
    }

    if value.get("custom_gradient").is_some() && value.get("palette").is_some() {
        let asset: CustomColorSet = serde_json::from_value(value)?;
        asset.validate_asset_type()?;
        return Ok(asset);
    }

    if value.get("render_params").is_some() || value.get("project").is_some() {
        let workflow = load_workflow_or_preset(path)?;
        return Ok(CustomColorSet::from_render_params(
            file_stem_or(path, "Color Set"),
            &workflow.project.render_params,
        ));
    }

    if !looks_like_custom_gradient(&value) {
        anyhow::bail!("wrong asset type: expected color_set, but JSON is not a color set");
    }
    let source: crate::render::color::CustomGradient = serde_json::from_value(value)?;
    let params = crate::render::renderer::RenderParams {
        custom_gradient: source,
        ..crate::render::renderer::RenderParams::default()
    };
    Ok(CustomColorSet::from_render_params(
        file_stem_or(path, "Color Set"),
        &params,
    ))
}

fn formula_source_asset_type(target: FormulaSourceTarget) -> AssetType {
    match target {
        FormulaSourceTarget::Pattern(_) => AssetType::PatternSource,
        FormulaSourceTarget::Effect(_) => AssetType::EffectSource,
    }
}

fn looks_like_formula_source(value: &serde_json::Value) -> bool {
    value.get("expression").is_some()
        || value.get("layers").is_some()
        || value.get("controls").is_some()
        || value.get("effect_blend_mode").is_some()
}

fn looks_like_custom_gradient(value: &serde_json::Value) -> bool {
    value.get("colors").is_some()
        || value.get("color_transitions").is_some()
        || value.get("color_a").is_some()
        || value.get("color_b").is_some()
        || value.get("color_c").is_some()
        || value.get("color_d").is_some()
}

fn formula_source_from_render_params(
    params: &crate::render::renderer::RenderParams,
    target: FormulaSourceTarget,
) -> (String, crate::render::formula::FormulaSource) {
    match target {
        FormulaSourceTarget::Pattern(index) => params
            .patterns
            .get(index)
            .map(|layer| (layer.name.clone(), layer.source.clone()))
            .unwrap_or_else(|| {
                (
                    formula_source_label(target).to_owned(),
                    crate::render::formula::FormulaSource::fractal_a(),
                )
            }),
        FormulaSourceTarget::Effect(index) => params
            .effects
            .get(index)
            .map(|layer| (layer.name.clone(), layer.source.clone()))
            .unwrap_or_else(|| {
                (
                    formula_source_label(target).to_owned(),
                    crate::render::formula::FormulaSource::pattern(),
                )
            }),
    }
}

fn formula_source_label(target: FormulaSourceTarget) -> &'static str {
    match target {
        FormulaSourceTarget::Pattern(_) => "Pattern source",
        FormulaSourceTarget::Effect(_) => "Effect source",
    }
}

fn formula_source_file_stem(target: FormulaSourceTarget) -> String {
    match target {
        FormulaSourceTarget::Pattern(index) => format!("pattern-source-{}", index + 1),
        FormulaSourceTarget::Effect(index) => format!("effect-source-{}", index + 1),
    }
}

fn file_stem_or(path: &std::path::Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| fallback.to_owned())
}

fn preview_render_dimensions(width: u32, height: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= MAX_PREVIEW_RENDER_DIMENSION {
        return (width, height);
    }

    let scale = MAX_PREVIEW_RENDER_DIMENSION as f32 / longest as f32;
    (
        ((width as f32 * scale).round() as u32).max(64),
        ((height as f32 * scale).round() as u32).max(64),
    )
}

#[cfg(test)]
mod portable_state_tests {
    use super::{
        apply_formula_source_to_project, ensure_project_paths_portable, load_color_set_from_path,
        load_formula_source_from_path, portable_history_entry, preview_state_fingerprint,
    };
    use crate::{
        export::history::ExportHistoryEntry,
        presets::preset::Preset,
        project::{app_settings::AppSettings, project_state::ProjectState},
        render::color::PaletteKind,
        render::{
            formula::{FormulaControl, FormulaLayer, FormulaSource},
            renderer::{EffectBlendMode, EffectLayer, RenderParams},
        },
        source_assets::{
            asset::{CustomColorSet, FormulaSourceAsset},
            storage::save_asset,
        },
        ui::FormulaSourceTarget,
    };

    #[test]
    fn session_project_output_path_is_moved_inside_portable_exports() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let settings = AppSettings::for_root(temp_dir.path().to_path_buf()).expect("settings");
        let mut project = ProjectState::default();
        project.export_settings.output_path =
            std::path::PathBuf::from(r"C:\Users\me\Desktop\outside.gif");

        ensure_project_paths_portable(&mut project, &settings);

        assert_eq!(
            project.export_settings.output_path,
            settings.exports_dir.join("outside.gif")
        );
        assert!(
            project
                .export_settings
                .output_path
                .starts_with(&settings.portable_root)
        );
    }

    #[test]
    fn project_output_path_traversal_is_moved_inside_portable_exports() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let settings = AppSettings::for_root(temp_dir.path().to_path_buf()).expect("settings");
        let mut project = ProjectState::default();
        project.export_settings.output_path = settings
            .exports_dir
            .join("..")
            .join("..")
            .join("outside.gif");

        ensure_project_paths_portable(&mut project, &settings);

        assert_eq!(
            project.export_settings.output_path,
            settings.exports_dir.join("outside.gif")
        );
        assert!(
            project
                .export_settings
                .output_path
                .starts_with(&settings.portable_root)
        );
    }

    #[test]
    fn export_history_entry_does_not_persist_external_output_path() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let settings = AppSettings::for_root(temp_dir.path().to_path_buf()).expect("settings");
        let entry = ExportHistoryEntry {
            output_path: std::path::PathBuf::from(r"C:\Users\me\Desktop\outside.gif"),
            ..ExportHistoryEntry::default()
        };

        let entry = portable_history_entry(entry, &settings);

        assert_eq!(entry.output_path, settings.exports_dir.join("outside.gif"));
        assert!(entry.output_path.starts_with(&settings.portable_root));
    }

    #[test]
    fn formula_source_load_preserves_bundled_source_layers() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let source = FormulaSource {
            expression: "sin(x * scale)".to_owned(),
            layers: vec![
                FormulaLayer {
                    name: "Base layer".to_owned(),
                    expression: "sin(x * scale)".to_owned(),
                    ..FormulaLayer::default()
                },
                FormulaLayer {
                    name: "Second layer".to_owned(),
                    expression: "cos(y * scale)".to_owned(),
                    ..FormulaLayer::default()
                },
            ],
            ..FormulaSource::default()
        };
        let asset = FormulaSourceAsset::new_pattern("Layered source", source);
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save asset");

        let (_name, loaded) = load_formula_source_from_path(&path, FormulaSourceTarget::Pattern(0))
            .expect("load source");

        assert_eq!(loaded.layers.len(), 2);
    }

    #[test]
    fn pattern_source_loader_rejects_typed_effect_asset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut source = FormulaSource::pattern();
        source.effect_blend_mode = Some(crate::render::renderer::EffectBlendMode::Mask);
        let asset = FormulaSourceAsset::new_effect("Effect source", source);
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save asset");

        let error = load_formula_source_from_path(&path, FormulaSourceTarget::Pattern(0))
            .expect_err("effect source must not load as pattern source");

        assert!(error.to_string().contains("wrong asset type"));
    }

    #[test]
    fn effect_source_loader_rejects_typed_pattern_asset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let asset = FormulaSourceAsset::new_pattern("Pattern source", FormulaSource::pattern());
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save asset");

        let error = load_formula_source_from_path(&path, FormulaSourceTarget::Effect(0))
            .expect_err("pattern source must not load as effect source");

        assert!(error.to_string().contains("wrong asset type"));
    }

    #[test]
    fn color_set_loader_rejects_typed_formula_source_asset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let asset = FormulaSourceAsset::new_pattern("Pattern source", FormulaSource::pattern());
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save asset");

        let error =
            load_color_set_from_path(&path).expect_err("pattern source must not load as color set");

        assert!(error.to_string().contains("wrong asset type"));
    }

    #[test]
    fn formula_source_loader_rejects_typed_color_set_asset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let project = ProjectState::default();
        let asset = CustomColorSet::from_render_params("Color set", &project.render_params);
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save color set");

        let error = load_formula_source_from_path(&path, FormulaSourceTarget::Pattern(0))
            .expect_err("color set must not load as pattern source");

        assert!(error.to_string().contains("wrong asset type"));
    }

    #[test]
    fn formula_source_loader_rejects_typed_workflow_preset_asset() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let preset = Preset::new("Workflow preset", RenderParams::default());
        let path = temp_dir.path().join("workflow-preset.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&preset).expect("serialize preset"),
        )
        .expect("write workflow preset");

        let error = load_formula_source_from_path(&path, FormulaSourceTarget::Pattern(0))
            .expect_err("workflow preset must not load as pattern source");

        assert!(error.to_string().contains("wrong asset type"));
    }

    #[test]
    fn legacy_source_asset_without_type_is_context_checked() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let pattern_path = temp_dir.path().join("legacy-pattern.json");
        std::fs::write(
            &pattern_path,
            serde_json::json!({
                "name": "Legacy pattern",
                "source": {
                    "expression": "sin(x * scale)",
                    "layers": [{ "expression": "sin(x * scale)" }]
                }
            })
            .to_string(),
        )
        .expect("write legacy pattern");
        let (_name, pattern_source) =
            load_formula_source_from_path(&pattern_path, FormulaSourceTarget::Pattern(0))
                .expect("legacy pattern should load as pattern");
        assert!(pattern_source.effect_blend_mode.is_none());

        let error = load_formula_source_from_path(&pattern_path, FormulaSourceTarget::Effect(0))
            .expect_err("ambiguous legacy pattern should not load as effect");
        assert!(error.to_string().contains("no asset_type"));
    }

    #[test]
    fn color_set_loader_rejects_json_without_color_shape() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("not-a-color-set.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "name": "Wrong",
                "source": { "expression": "sin(x)" }
            })
            .to_string(),
        )
        .expect("write wrong json");

        let error = load_color_set_from_path(&path).expect_err("wrong JSON should not load");

        assert!(error.to_string().contains("wrong asset type"));
    }

    #[test]
    fn typed_color_set_loads_with_schema_metadata() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        project.render_params.palette = PaletteKind::Fire;
        let asset = CustomColorSet::from_render_params("Fire set", &project.render_params);
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save color set");

        let loaded = load_color_set_from_path(&path).expect("load typed color set");

        assert_eq!(loaded.name, "Fire set");
        assert_eq!(loaded.palette, PaletteKind::Fire);
    }

    #[test]
    fn legacy_disabled_gradient_color_set_loads_as_active_default_palette() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("legacy-gradient.json");
        std::fs::write(
            &path,
            serde_json::json!({
                "enabled": false,
                "colors": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                "color_transitions": []
            })
            .to_string(),
        )
        .expect("write legacy gradient");

        let loaded = load_color_set_from_path(&path).expect("load legacy color set");

        assert!(loaded.custom_gradient.enabled);
        assert_eq!(loaded.palette, PaletteKind::Neon);
        assert_eq!(loaded.custom_gradient.stops()[0], [0.05, 0.02, 0.18]);
        assert_eq!(loaded.custom_gradient.stops()[3], [1.00, 0.86, 0.18]);
    }

    #[test]
    fn preview_fingerprint_changes_for_creative_state_and_export_settings() {
        let project = ProjectState::default();
        let baseline = preview_state_fingerprint(&project);

        let mut formula_changed = project.clone();
        formula_changed.render_params.patterns[0].source = FormulaSource {
            expression: "sin(x * scale + p1)".to_owned(),
            layers: vec![FormulaLayer {
                expression: "sin(x * scale + p1)".to_owned(),
                ..FormulaLayer::default()
            }],
            controls: vec![FormulaControl {
                name: "P1".to_owned(),
                value: 0.25,
            }],
            ..FormulaSource::default()
        };
        assert_ne!(
            baseline,
            preview_state_fingerprint(&formula_changed),
            "pattern formula and formula controls are preview inputs"
        );

        let mut control_changed = formula_changed.clone();
        control_changed.render_params.patterns[0].source.controls[0].value = 0.9;
        assert_ne!(
            preview_state_fingerprint(&formula_changed),
            preview_state_fingerprint(&control_changed),
            "p1..p4 values are preview inputs"
        );

        let mut domain_changed = formula_changed.clone();
        domain_changed.render_params.patterns[0].source.layers[0].domain_x =
            "x + sin(y * p1) * 0.25".to_owned();
        domain_changed.render_params.patterns[0].source.layers[0].domain_y =
            "y + cos(x * p1) * 0.25".to_owned();
        domain_changed.render_params.patterns[0].source.layers[0].domain_influence = 0.7;
        assert_ne!(
            preview_state_fingerprint(&formula_changed),
            preview_state_fingerprint(&domain_changed),
            "domain pipeline fields are preview inputs"
        );

        let mut effect_changed = project.clone();
        effect_changed.render_params.effects.push(EffectLayer {
            blend_mode: EffectBlendMode::Difference,
            strength: 0.72,
            source: FormulaSource::pattern(),
            ..EffectLayer::default()
        });
        assert_ne!(
            baseline,
            preview_state_fingerprint(&effect_changed),
            "effect stack is a preview input"
        );

        let mut color_changed = project.clone();
        color_changed
            .render_params
            .custom_gradient
            .ensure_color_stops();
        color_changed.render_params.custom_gradient.colors[0] = [0.91, 0.05, 0.18];
        color_changed
            .render_params
            .custom_gradient
            .add_transition(0);
        color_changed
            .render_params
            .custom_gradient
            .color_transitions[0][0]
            .color = [0.12, 0.62, 0.95];
        assert_ne!(
            baseline,
            preview_state_fingerprint(&color_changed),
            "custom colors and color transitions are preview inputs"
        );

        let mut export_changed = project.clone();
        export_changed.export_settings.width = 333;
        export_changed.export_settings.height = 222;
        export_changed.export_settings.fps = 17;
        export_changed.export_settings.duration_seconds = 3.75;
        export_changed.export_settings.lossy_quality = 42;
        export_changed.export_settings.fast = false;
        assert_ne!(
            baseline,
            preview_state_fingerprint(&export_changed),
            "GIF output settings used by live preview are preview inputs"
        );
    }

    #[test]
    fn preview_fingerprint_ignores_output_path() {
        let mut project = ProjectState::default();
        project.export_settings.output_path = std::path::PathBuf::from("exports/a.gif");
        let baseline = preview_state_fingerprint(&project);

        project.export_settings.output_path = std::path::PathBuf::from(r"C:\Users\me\b.gif");

        assert_eq!(
            baseline,
            preview_state_fingerprint(&project),
            "output_path is not a visual preview input"
        );
    }

    #[test]
    fn project_sanitize_keeps_color_phase_one_editable() {
        let mut project = ProjectState::default();

        project.render_params.color_phase = 1.0;
        project.sanitize();
        assert_eq!(project.render_params.color_phase, 1.0);

        project.render_params.color_phase = 1.2;
        project.sanitize();
        assert_eq!(project.render_params.color_phase, 1.0);

        project.render_params.color_phase = -0.2;
        project.sanitize();
        assert_eq!(project.render_params.color_phase, 0.0);
    }

    #[test]
    fn source_and_color_load_results_update_model_used_by_preview() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        let baseline = preview_state_fingerprint(&project);

        let pattern_source = FormulaSource {
            expression: "cos(x * scale + y * p1)".to_owned(),
            layers: vec![FormulaLayer {
                expression: "cos(x * scale + y * p1)".to_owned(),
                ..FormulaLayer::default()
            }],
            controls: vec![FormulaControl {
                name: "P1".to_owned(),
                value: 1.4,
            }],
            ..FormulaSource::default()
        };
        let pattern_asset = FormulaSourceAsset::new_pattern("Loaded pattern", pattern_source);
        let pattern_path =
            save_asset(temp_dir.path(), &pattern_asset.name, &pattern_asset).expect("save source");
        let (pattern_name, loaded_pattern) =
            load_formula_source_from_path(&pattern_path, FormulaSourceTarget::Pattern(0))
                .expect("load source");
        apply_formula_source_to_project(
            &mut project,
            FormulaSourceTarget::Pattern(0),
            pattern_name,
            loaded_pattern,
        );
        assert_eq!(project.render_params.patterns[0].name, "Loaded pattern");

        assert_ne!(
            baseline,
            preview_state_fingerprint(&project),
            "loaded pattern source changes the render model used by preview"
        );

        project.render_params.effects.push(EffectLayer::default());
        let mut effect_source = FormulaSource::pattern();
        effect_source.effect_blend_mode = Some(EffectBlendMode::Screen);
        let effect_asset = FormulaSourceAsset::new_effect("Loaded effect", effect_source);
        let effect_path =
            save_asset(temp_dir.path(), &effect_asset.name, &effect_asset).expect("save effect");
        let (effect_name, loaded_effect) =
            load_formula_source_from_path(&effect_path, FormulaSourceTarget::Effect(0))
                .expect("load effect source");
        apply_formula_source_to_project(
            &mut project,
            FormulaSourceTarget::Effect(0),
            effect_name,
            loaded_effect,
        );
        assert_eq!(project.render_params.effects[0].name, "Loaded effect");
        assert_eq!(
            project.render_params.effects[0].blend_mode,
            EffectBlendMode::Screen
        );

        let before_color = preview_state_fingerprint(&project);
        let mut color_project = ProjectState::default();
        color_project
            .render_params
            .custom_gradient
            .ensure_color_stops();
        color_project.render_params.custom_gradient.colors[0] = [0.02, 0.25, 0.96];
        color_project.render_params.custom_gradient.colors[1] = [0.98, 0.72, 0.03];
        let color_asset =
            CustomColorSet::from_render_params("Loaded colors", &color_project.render_params);
        let color_path =
            save_asset(temp_dir.path(), &color_asset.name, &color_asset).expect("save colors");
        let loaded_color_set = load_color_set_from_path(&color_path).expect("load colors");
        loaded_color_set.apply_to_render_params(&mut project.render_params);

        assert_ne!(
            before_color,
            preview_state_fingerprint(&project),
            "loaded color set changes the render model used by preview"
        );
    }
}
