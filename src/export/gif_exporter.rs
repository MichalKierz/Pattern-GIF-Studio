use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
};

use anyhow::{Context, Result};
use gif::{Encoder, Frame, Repeat};

use crate::{
    export::{
        export_settings::ExportSettings, gif_quality::quantization_speed, progress::ExportProgress,
    },
    project::project_state::ProjectState,
    render::backend::{BackendRenderers, FrameRenderSpec},
};

pub fn export_gif(
    project: ProjectState,
    report: Sender<ExportProgress>,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    export_gif_with_renderers(project, report, cancel, BackendRenderers::new())
}

fn export_gif_with_renderers(
    project: ProjectState,
    report: Sender<ExportProgress>,
    cancel: Arc<AtomicBool>,
    mut renderers: BackendRenderers,
) -> Result<()> {
    let mut settings = project.export_settings.clone();
    settings.sanitize();
    ensure_parent_dir(&settings.output_path)?;

    let total_frames = settings.total_frames();
    let _ = report.send(ExportProgress::Started { total_frames });

    let temp_path = temporary_export_path(&settings.output_path);
    let result = export_gif_to_temp(TempExportRun {
        project: &project,
        settings: &settings,
        temp_path: &temp_path,
        total_frames,
        report: &report,
        cancel: &cancel,
        renderers: &mut renderers,
    });
    match result {
        Ok(ExportCompletion::Finished) => {
            if let Err(error) = persist_temp_file(&temp_path, &settings.output_path) {
                let _ = std::fs::remove_file(&temp_path);
                return Err(error);
            }
            let _ = report.send(ExportProgress::Finished {
                output_path: settings.output_path,
            });
            Ok(())
        }
        Ok(ExportCompletion::Cancelled) => {
            let _ = std::fs::remove_file(&temp_path);
            let _ = report.send(ExportProgress::Cancelled);
            Ok(())
        }
        Err(error) => {
            let _ = std::fs::remove_file(&temp_path);
            Err(error)
        }
    }
}

enum ExportCompletion {
    Finished,
    Cancelled,
}

struct TempExportRun<'a> {
    project: &'a ProjectState,
    settings: &'a ExportSettings,
    temp_path: &'a Path,
    total_frames: u32,
    report: &'a Sender<ExportProgress>,
    cancel: &'a Arc<AtomicBool>,
    renderers: &'a mut BackendRenderers,
}

fn export_gif_to_temp(run: TempExportRun<'_>) -> Result<ExportCompletion> {
    let TempExportRun {
        project,
        settings,
        temp_path,
        total_frames,
        report,
        cancel,
        renderers,
    } = run;

    if cancel.load(Ordering::Relaxed) {
        return Ok(ExportCompletion::Cancelled);
    }

    let output = File::create(temp_path)
        .with_context(|| format!("failed to create temporary export {}", temp_path.display()))?;
    let mut output = BufWriter::new(output);
    let mut encoder = Encoder::new(
        &mut output,
        settings.width as u16,
        settings.height as u16,
        &[],
    )
    .context("failed to create GIF encoder")?;
    encoder
        .set_repeat(Repeat::Infinite)
        .context("failed to configure GIF loop")?;

    let render_params = project.render_params.clone();
    let quantization_speed = quantization_speed(settings);
    let mut last_backend_status = None;

    for frame_index in 0..total_frames {
        if cancel.load(Ordering::Relaxed) {
            drop(encoder);
            drop(output);
            return Ok(ExportCompletion::Cancelled);
        }

        let spec = FrameRenderSpec::new(frame_index, total_frames, settings.width, settings.height);
        let rendered = renderers
            .render_indexed_frame(&render_params, spec)
            .with_context(|| format!("failed to render GIF frame {frame_index}"))?;
        if last_backend_status.as_ref() != Some(&rendered.status) {
            let _ = report.send(ExportProgress::BackendStatus {
                status: rendered.status.clone(),
            });
            last_backend_status = Some(rendered.status);
        }

        let mut pixels = rendered.frame.pixels;
        let progress_frame = frame_index + 1;
        let _ = report.send(ExportProgress::Rendering {
            frame: progress_frame,
            total_frames,
        });
        if cancel.load(Ordering::Relaxed) {
            drop(encoder);
            drop(output);
            return Ok(ExportCompletion::Cancelled);
        }

        let mut gif_frame = Frame::from_rgba_speed(
            settings.width as u16,
            settings.height as u16,
            &mut pixels,
            quantization_speed,
        );
        gif_frame.delay =
            frame_delay_centiseconds(frame_index, total_frames, settings.duration_seconds);
        encoder
            .write_frame(&gif_frame)
            .with_context(|| format!("failed to encode GIF frame {frame_index}"))?;
        let _ = report.send(ExportProgress::Encoding {
            frame: progress_frame,
            total_frames,
        });
    }
    drop(encoder);
    output
        .flush()
        .context("failed to flush temporary GIF export")?;
    drop(output);

    if cancel.load(Ordering::Relaxed) {
        return Ok(ExportCompletion::Cancelled);
    }

    Ok(ExportCompletion::Finished)
}

fn frame_delay_centiseconds(frame_index: u32, total_frames: u32, duration_seconds: f32) -> u16 {
    let total_frames = total_frames.max(1);
    let total_centiseconds = ((duration_seconds.max(0.25) as f64) * 100.0)
        .round()
        .max(total_frames as f64 * 2.0) as u64;
    let start =
        ((frame_index as f64 * total_centiseconds as f64) / total_frames as f64).round() as u64;
    let end = (((frame_index + 1) as f64 * total_centiseconds as f64) / total_frames as f64).round()
        as u64;
    end.saturating_sub(start).clamp(2, u16::MAX as u64) as u16
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

fn temporary_export_path(output_path: &Path) -> PathBuf {
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("pattern-loop.gif");
    output_path.with_file_name(format!("{file_name}.part"))
}

fn persist_temp_file(temp_path: &Path, output_path: &Path) -> Result<()> {
    match std::fs::rename(temp_path, output_path) {
        Ok(()) => Ok(()),
        Err(rename_error) if output_path.exists() => {
            std::fs::copy(temp_path, output_path).with_context(|| {
                format!(
                    "failed to replace {} from temporary export {} after rename failed: {rename_error}",
                    output_path.display(),
                    temp_path.display()
                )
            })?;
            std::fs::remove_file(temp_path).with_context(|| {
                format!("failed to remove temporary export {}", temp_path.display())
            })?;
            Ok(())
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "failed to move temporary export {} to {}",
                temp_path.display(),
                output_path.display()
            )
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, Cursor},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
    };

    use anyhow::{Context, anyhow};
    use gif::Encoder;

    use crate::{
        animation::loop_time::LoopTime,
        export::{
            gif_exporter::{export_gif, export_gif_with_renderers, temporary_export_path},
            gif_quality::apply_gif_quality_preview,
            progress::ExportProgress,
        },
        project::{project_state::ProjectState, render_settings::RenderBackend},
        render::{
            backend::{BackendRenderers, FrameRenderSpec},
            color::{ColorTransition, CustomGradient, PaletteKind},
            formula::{FormulaBlendMode, FormulaControl, FormulaLayer, FormulaSource},
            frame_buffer::FrameBuffer,
            renderer::{EffectBlendMode, EffectLayer, PatternLayer, RenderParams},
        },
        ui::gif_output_panel::estimated_file_size_bytes,
    };

    #[test]
    fn exported_gif_duration_matches_settings() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        project.export_settings.output_path = temp_dir.path().join("timing.gif");
        project.export_settings.width = 64;
        project.export_settings.height = 64;
        project.export_settings.fps = 10;
        project.export_settings.duration_seconds = 2.0;
        project.render_params.color_speed = 1.5;
        project.render_params.rotation_speed = 1.0;

        let (tx, _rx) = mpsc::channel();
        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let timing =
            gif_timing(&std::fs::read(&project.export_settings.output_path).expect("read gif"));

        assert!(
            (timing.duration_seconds - 2.0).abs() <= 0.08,
            "expected about 2.0s, got {}s",
            timing.duration_seconds
        );
    }

    #[test]
    fn exported_six_second_gif_duration_matches_settings() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        project.export_settings.output_path = temp_dir.path().join("six-seconds.gif");
        project.export_settings.width = 64;
        project.export_settings.height = 64;
        project.export_settings.fps = 24;
        project.export_settings.duration_seconds = 6.0;
        project.render_params.color_speed = 1.5;
        project.render_params.rotation_speed = 1.0;

        let (tx, _rx) = mpsc::channel();
        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let timing =
            gif_timing(&std::fs::read(&project.export_settings.output_path).expect("read gif"));

        assert!(
            (timing.duration_seconds - 6.0).abs() <= 0.12,
            "expected about 6.0s, got {}s",
            timing.duration_seconds
        );
    }

    #[test]
    fn exported_thirty_fps_six_second_gif_duration_matches_settings() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        project.export_settings.output_path = temp_dir.path().join("thirty-fps-six-seconds.gif");
        project.export_settings.width = 64;
        project.export_settings.height = 64;
        project.export_settings.fps = 30;
        project.export_settings.duration_seconds = 6.0;
        project.render_params.color_speed = 1.5;
        project.render_params.rotation_speed = 1.0;

        let (tx, _rx) = mpsc::channel();
        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let timing =
            gif_timing(&std::fs::read(&project.export_settings.output_path).expect("read gif"));

        assert!(
            (timing.duration_seconds - 6.0).abs() <= 0.12,
            "expected about 6.0s, got {}s",
            timing.duration_seconds
        );
    }

    #[test]
    fn exported_thirty_fps_four_second_gif_has_expected_frame_count_and_duration() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        project.export_settings.output_path = temp_dir.path().join("thirty-fps-four-seconds.gif");
        project.export_settings.width = 64;
        project.export_settings.height = 64;
        project.export_settings.fps = 30;
        project.export_settings.duration_seconds = 4.0;
        project.render_params.color_speed = 1.5;
        project.render_params.rotation_speed = 1.0;

        let (tx, _rx) = mpsc::channel();
        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let bytes = std::fs::read(&project.export_settings.output_path).expect("read gif");
        let timing = gif_timing(&bytes);

        assert_eq!(timing.frames, 120);
        assert!(
            (timing.duration_seconds - 4.0).abs() <= 0.04,
            "expected about 4.0s, got {}s",
            timing.duration_seconds
        );
    }

    #[test]
    fn exported_high_fps_gif_is_sanitized_to_supported_duration() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = ProjectState::default();
        project.export_settings.output_path = temp_dir.path().join("high-fps.gif");
        project.export_settings.width = 64;
        project.export_settings.height = 64;
        project.export_settings.fps = 60;
        project.export_settings.duration_seconds = 6.0;
        project.render_params.color_speed = 1.5;
        project.render_params.rotation_speed = 1.0;

        let (tx, _rx) = mpsc::channel();
        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let timing =
            gif_timing(&std::fs::read(&project.export_settings.output_path).expect("read gif"));

        assert!(
            (timing.duration_seconds - 6.0).abs() <= 0.12,
            "expected about 6.0s, got {}s",
            timing.duration_seconds
        );
    }

    #[test]
    fn gpu_export_reports_gpu_backend() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = tiny_export_project(temp_dir.path().join("gpu-backend.gif"));
        project.export_settings.fps = 3;
        project.export_settings.duration_seconds = 1.0;
        let (tx, rx) = mpsc::channel();

        export_gif(project, tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let statuses = backend_statuses(rx.try_iter());
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].used, RenderBackend::Gpu);
        assert!(statuses[0].error.is_none());
    }

    #[test]
    fn gpu_export_failure_returns_error_without_fallback() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = tiny_export_project(temp_dir.path().join("gpu-failure.gif"));
        project.export_settings.fps = 3;
        project.export_settings.duration_seconds = 1.0;
        let renderers =
            BackendRenderers::with_gpu_factory(|| Err(anyhow!("forced GPU init failure")));
        let (tx, rx) = mpsc::channel();

        let error =
            export_gif_with_renderers(project, tx, Arc::new(AtomicBool::new(false)), renderers)
                .expect_err("GPU-only export should fail when GPU cannot initialize");

        let statuses = backend_statuses(rx.try_iter());
        assert!(
            statuses.is_empty(),
            "failed first frame should not report a fake fallback backend"
        );
        assert!(
            error.to_string().contains("failed to render GIF frame 0"),
            "error should keep frame context: {error:?}"
        );
        assert!(
            error
                .chain()
                .any(|cause| cause.to_string().contains("forced GPU init failure")),
            "error chain should contain the GPU initialization failure: {error:?}"
        );
    }

    #[test]
    fn export_progress_is_monotonic_and_reports_encoding_backend_and_completion() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let mut project = tiny_export_project(temp_dir.path().join("progress.gif"));
        project.export_settings.fps = 4;
        project.export_settings.duration_seconds = 1.0;
        let total_frames = project.export_settings.total_frames();
        let (tx, rx) = mpsc::channel();

        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");

        let progress: Vec<_> = rx.try_iter().collect();
        assert!(matches!(
            progress.first(),
            Some(ExportProgress::Started { total_frames: frames }) if *frames == total_frames
        ));
        assert!(matches!(
            progress.last(),
            Some(ExportProgress::Finished { output_path }) if output_path == &project.export_settings.output_path
        ));
        assert!(
            progress
                .iter()
                .any(|progress| matches!(progress, ExportProgress::BackendStatus { .. })),
            "export should report the renderer backend"
        );
        assert_eq!(
            progress
                .iter()
                .filter(|progress| matches!(progress, ExportProgress::Rendering { .. }))
                .count(),
            total_frames as usize
        );
        assert_eq!(
            progress
                .iter()
                .filter(|progress| matches!(progress, ExportProgress::Encoding { .. }))
                .count(),
            total_frames as usize
        );

        let mut previous = 0.0;
        for fraction in progress.iter().filter_map(ExportProgress::fraction) {
            assert!(
                fraction >= previous,
                "progress fraction must be monotonic: {fraction} after {previous}"
            );
            assert!(
                fraction <= 1.0,
                "progress fraction must not exceed 100%, got {fraction}"
            );
            previous = fraction;
        }
        assert_eq!(previous, 1.0);
    }

    #[test]
    fn cancelled_export_removes_partial_file_and_allows_next_export() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_path = temp_dir.path().join("cancel-then-success.gif");
        let mut project = tiny_export_project(output_path.clone());
        project.export_settings.width = 128;
        project.export_settings.height = 128;
        project.export_settings.fps = 30;
        project.export_settings.duration_seconds = 4.0;
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        let (tx, rx) = mpsc::channel();
        let worker_project = project.clone();
        let handle = thread::spawn(move || {
            export_gif(worker_project, tx, worker_cancel).expect("cancel export")
        });

        while let Ok(progress) = rx.recv() {
            if matches!(progress, ExportProgress::Rendering { .. }) {
                cancel.store(true, Ordering::Relaxed);
            }
            if matches!(progress, ExportProgress::Cancelled) {
                break;
            }
        }
        handle.join().expect("export thread");

        assert!(
            !output_path.exists(),
            "cancelled export must not leave the final GIF path behind"
        );
        assert!(
            !temporary_export_path(&output_path).exists(),
            "cancelled export must remove the temporary partial file"
        );

        let (success_tx, success_rx) = mpsc::channel();
        export_gif(
            project.clone(),
            success_tx,
            Arc::new(AtomicBool::new(false)),
        )
        .expect("second export after cancel");
        let success_progress: Vec<_> = success_rx.try_iter().collect();

        assert!(output_path.exists());
        assert!(
            success_progress
                .iter()
                .any(|progress| matches!(progress, ExportProgress::Finished { .. })),
            "a new export should complete after a cancelled export"
        );
    }

    #[test]
    fn preset_cancelled_export_never_creates_final_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output_path = temp_dir.path().join("pre-cancelled.gif");
        let project = tiny_export_project(output_path.clone());
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(true));

        export_gif(project, tx, cancel).expect("pre-cancelled export");
        let progress: Vec<_> = rx.try_iter().collect();

        assert!(
            progress
                .iter()
                .any(|progress| matches!(progress, ExportProgress::Cancelled))
        );
        assert!(!output_path.exists());
        assert!(!temporary_export_path(&output_path).exists());
    }

    #[test]
    fn export_worker_reports_invalid_output_path_as_failed_without_panic() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("blocked parent file");
        let mut project = tiny_export_project(blocked_parent.join("out.gif"));
        project.export_settings.fps = 2;
        project.export_settings.duration_seconds = 1.0;
        let (tx, rx) = mpsc::channel();
        let fail_tx = tx.clone();
        let handle = thread::spawn(move || {
            if let Err(error) = export_gif(project, tx, Arc::new(AtomicBool::new(false))) {
                let _ = fail_tx.send(ExportProgress::Failed {
                    message: error.to_string(),
                });
            }
        });

        handle.join().expect("failed export thread");
        let progress: Vec<_> = rx.try_iter().collect();
        let failure = progress
            .iter()
            .find_map(|progress| match progress {
                ExportProgress::Failed { message } => Some(message),
                _ => None,
            })
            .expect("failed progress");

        assert!(
            failure.contains("failed to create"),
            "failure should describe the output path problem, got: {failure}"
        );
    }

    #[test]
    fn gif_encoder_create_error_is_reported_with_context() {
        struct FailingWriter;

        impl std::io::Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("forced encoder writer failure"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let mut writer = FailingWriter;
        let result = Encoder::new(&mut writer, 8, 8, &[]).context("failed to create GIF encoder");
        let Err(error) = result else {
            panic!("failing writer should fail encoder creation");
        };
        let message = error.to_string();

        assert!(
            message.contains("failed to create GIF encoder"),
            "encoder error should keep production context: {message}"
        );
    }

    #[test]
    fn size_estimate_is_a_rough_sanity_bound_for_multiple_scene_classes() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let scenes = [
            size_estimate_scene_flat(temp_dir.path().join("flat.gif")),
            size_estimate_scene_high_detail(temp_dir.path().join("detail.gif")),
            size_estimate_scene_many_colors(temp_dir.path().join("many-colors.gif")),
            size_estimate_scene_many_frames(temp_dir.path().join("many-frames.gif")),
            size_estimate_scene_effects(temp_dir.path().join("effects.gif")),
        ];

        for scene in scenes {
            assert_estimate_sanity(scene);
        }
    }

    #[test]
    fn decoded_gif_frames_match_preview_reference_with_timing_and_loop() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let project = parity_project(temp_dir.path().join("preview-export-parity.gif"));
        let settings = project.export_settings.clone();
        let total_frames = settings.total_frames();
        let middle_frame = total_frames / 2;
        assert_eq!(
            project.render_params.effects[0].blend_mode,
            EffectBlendMode::Difference
        );
        assert_eq!(project.render_params.patterns.len(), 2);
        assert_eq!(project.render_params.patterns[0].source.layers.len(), 2);
        assert_eq!(project.render_params.effects.len(), 2);
        assert!(project.render_params.custom_gradient.colors.len() > 8);
        assert_eq!(
            project.render_params.patterns[0].source.layers[0].domain_x,
            "x + sin(y * p3 + t) * p4"
        );
        assert!(
            project.render_params.patterns[0].source.layers[1]
                .expression
                .contains("prev")
        );
        assert_eq!(project.render_params.effects[0].strength, 0.42);
        assert_eq!(project.render_params.effects[0].scale, 2.5);
        assert_eq!(project.render_params.effects[0].motion, 1.25);

        assert_eq!(total_frames, 5);
        assert_eq!(
            FrameRenderSpec::new(0, total_frames, 64, 64).time.phase,
            0.0
        );
        assert!(
            (FrameRenderSpec::new(middle_frame, total_frames, 64, 64)
                .time
                .phase
                - 0.4)
                .abs()
                < f32::EPSILON
        );

        let first_reference = preview_reference_frame(&project, 0);
        let middle_reference = preview_reference_frame(&project, middle_frame);
        assert_ne!(
            first_reference.pixels, middle_reference.pixels,
            "test scene must animate so frame-index timing is observable"
        );

        let (tx, rx) = mpsc::channel();
        export_gif(project.clone(), tx, Arc::new(AtomicBool::new(false))).expect("export gif");
        let statuses = backend_statuses(rx.try_iter());
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].used, RenderBackend::Gpu);

        let decoded =
            decode_gif_rgba(&std::fs::read(&settings.output_path).expect("read exported gif"));

        assert_eq!(decoded.repeat, gif::Repeat::Infinite);
        assert_eq!(decoded.frames.len() as u32, total_frames);
        assert!(
            decoded.frames.iter().all(|frame| frame.delay == 20),
            "5 FPS over 1 second must encode every frame with 20 cs delay"
        );
        assert!(
            decoded
                .frames
                .iter()
                .all(|frame| frame.width == 64 && frame.height == 64),
            "decoded GIF frames must preserve export resolution"
        );

        assert_frame_close_to_reference(&decoded.frames[0].pixels, &first_reference, 12.0);
        assert_frame_close_to_reference(
            &decoded.frames[middle_frame as usize].pixels,
            &middle_reference,
            12.0,
        );
    }

    #[test]
    fn frame_render_spec_uses_single_frame_index_timing_rule() {
        let total_frames = 5;

        let first = FrameRenderSpec::new(0, total_frames, 64, 64);
        let middle = FrameRenderSpec::new(2, total_frames, 64, 64);
        let last = FrameRenderSpec::new(4, total_frames, 64, 64);
        let wrapped = FrameRenderSpec::new(5, total_frames, 64, 64);

        assert_eq!(first.time, LoopTime::from_frame(0, total_frames));
        assert_eq!(middle.time, LoopTime::from_frame(2, total_frames));
        assert_eq!(last.time, LoopTime::from_frame(4, total_frames));
        assert_eq!(wrapped.frame_index, 0);
        assert_eq!(wrapped.time, LoopTime::from_frame(5, total_frames));
        assert!((last.time.phase - 0.8).abs() < f32::EPSILON);
    }

    struct SizeEstimateScene {
        name: &'static str,
        project: ProjectState,
    }

    fn assert_estimate_sanity(scene: SizeEstimateScene) {
        let estimated = estimated_file_size_bytes(&scene.project.export_settings).max(1);
        let (tx, _rx) = mpsc::channel();

        export_gif(scene.project.clone(), tx, Arc::new(AtomicBool::new(false)))
            .expect("export gif for size estimate calibration");

        let actual = std::fs::metadata(&scene.project.export_settings.output_path)
            .expect("export metadata")
            .len()
            .max(1);
        let ratio = actual as f64 / estimated as f64;
        assert!(
            (0.02..=8.0).contains(&ratio),
            "{} estimate should be a rough sanity bound: estimated {estimated}, actual {actual}, ratio {ratio:.3}",
            scene.name
        );
    }

    fn size_estimate_scene_flat(path: std::path::PathBuf) -> SizeEstimateScene {
        let mut project = size_estimate_base_project(path, 40, 40, 6, 1.0);
        project.render_params.patterns = vec![PatternLayer {
            name: "Flat calibration pattern".to_owned(),
            source: formula_source("Flat", "0.5", 1.0, 0.0, None),
            strength: 1.0,
            ..PatternLayer::default()
        }];
        project.render_params.effects.clear();
        SizeEstimateScene {
            name: "flat",
            project,
        }
    }

    fn size_estimate_scene_high_detail(path: std::path::PathBuf) -> SizeEstimateScene {
        let mut project = size_estimate_base_project(path, 48, 48, 8, 1.0);
        project.render_params.patterns = vec![PatternLayer {
            name: "High detail calibration pattern".to_owned(),
            source: formula_source(
                "High frequency field",
                "sin(x * scale * 38 + sin(y * 31 + t * motion) * 7) * cos(y * scale * 29 - u)",
                0.5,
                0.5,
                None,
            ),
            strength: 1.0,
            scale: 2.8,
            motion: 1.7,
            ..PatternLayer::default()
        }];
        project.render_params.effects.clear();
        project.render_params.custom_gradient = calibration_gradient(vec![
            [0.02, 0.02, 0.10],
            [0.00, 0.35, 0.95],
            [0.00, 0.95, 0.75],
            [0.95, 0.90, 0.10],
            [0.95, 0.12, 0.55],
        ]);
        SizeEstimateScene {
            name: "high_detail",
            project,
        }
    }

    fn size_estimate_scene_many_colors(path: std::path::PathBuf) -> SizeEstimateScene {
        let mut project = size_estimate_base_project(path, 48, 48, 8, 1.0);
        project.render_params.patterns = vec![PatternLayer {
            name: "Many colors calibration pattern".to_owned(),
            source: formula_source(
                "Color sweep",
                "0.5 + 0.5 * sin((x + y) * scale * 12 + t * motion)",
                1.0,
                0.0,
                None,
            ),
            strength: 1.0,
            scale: 2.2,
            motion: 1.0,
            ..PatternLayer::default()
        }];
        project.render_params.effects.clear();
        project.render_params.custom_gradient = calibration_gradient(vec![
            [0.01, 0.02, 0.09],
            [0.10, 0.15, 0.80],
            [0.00, 0.75, 0.95],
            [0.05, 0.85, 0.25],
            [0.92, 0.90, 0.12],
            [1.00, 0.48, 0.05],
            [0.95, 0.12, 0.18],
            [0.85, 0.08, 0.60],
            [0.42, 0.08, 0.90],
            [0.98, 0.98, 0.92],
        ]);
        project.render_params.color_speed = 1.1;
        SizeEstimateScene {
            name: "many_colors",
            project,
        }
    }

    fn size_estimate_scene_many_frames(path: std::path::PathBuf) -> SizeEstimateScene {
        let mut project = size_estimate_base_project(path, 32, 32, 16, 1.5);
        project.render_params.patterns = vec![PatternLayer {
            name: "Many frames calibration pattern".to_owned(),
            source: FormulaSource::pattern(),
            strength: 1.0,
            scale: 2.0,
            motion: 1.4,
            ..PatternLayer::default()
        }];
        project.render_params.effects.clear();
        project.render_params.color_speed = 1.6;
        SizeEstimateScene {
            name: "many_frames",
            project,
        }
    }

    fn size_estimate_scene_effects(path: std::path::PathBuf) -> SizeEstimateScene {
        let mut project = size_estimate_base_project(path, 48, 48, 8, 1.0);
        project.render_params.patterns = vec![PatternLayer {
            name: "Effect baseline calibration pattern".to_owned(),
            source: FormulaSource::fractal_b(),
            strength: 1.0,
            scale: 1.9,
            motion: 0.9,
            ..PatternLayer::default()
        }];
        project.render_params.effects = vec![
            EffectLayer {
                name: "Difference calibration effect".to_owned(),
                source: formula_source(
                    "Difference stripes",
                    "0.5 + 0.5 * sin((x - y) * scale * 9 + t * motion)",
                    1.0,
                    0.0,
                    Some(EffectBlendMode::Difference),
                ),
                blend_mode: EffectBlendMode::Difference,
                strength: 0.62,
                scale: 2.6,
                motion: 1.2,
                ..EffectLayer::default()
            },
            EffectLayer {
                name: "Screen calibration effect".to_owned(),
                source: formula_source(
                    "Screen rings",
                    "0.5 + 0.5 * cos(r * scale * 18 - u * motion)",
                    1.0,
                    0.0,
                    Some(EffectBlendMode::Screen),
                ),
                blend_mode: EffectBlendMode::Screen,
                strength: 0.48,
                scale: 1.8,
                motion: 0.8,
                ..EffectLayer::default()
            },
        ];
        project.render_params.custom_gradient = calibration_gradient(vec![
            [0.02, 0.01, 0.05],
            [0.12, 0.02, 0.35],
            [0.00, 0.45, 0.86],
            [0.10, 0.95, 0.80],
            [0.96, 0.92, 0.18],
            [0.98, 0.28, 0.08],
            [0.90, 0.10, 0.55],
            [0.95, 0.95, 0.95],
        ]);
        SizeEstimateScene {
            name: "effects",
            project,
        }
    }

    fn size_estimate_base_project(
        path: std::path::PathBuf,
        width: u32,
        height: u32,
        fps: u32,
        duration_seconds: f32,
    ) -> ProjectState {
        let mut project = tiny_export_project(path);
        project.export_settings.width = width;
        project.export_settings.height = height;
        project.export_settings.fps = fps;
        project.export_settings.duration_seconds = duration_seconds;
        project.export_settings.lossy_quality = 90;
        project.export_settings.fast = true;
        project.render_params.zoom = 1.4;
        project.render_params.brightness = 1.0;
        project.render_params.contrast = 1.12;
        project
    }

    fn formula_source(
        name: &str,
        expression: &str,
        gain: f32,
        bias: f32,
        effect_blend_mode: Option<EffectBlendMode>,
    ) -> FormulaSource {
        FormulaSource {
            expression: expression.to_owned(),
            gain,
            bias,
            effect_blend_mode,
            layers: vec![FormulaLayer {
                name: name.to_owned(),
                expression: expression.to_owned(),
                gain,
                bias,
                ..FormulaLayer::default()
            }],
            ..FormulaSource::default()
        }
    }

    fn calibration_gradient(colors: Vec<[f32; 3]>) -> CustomGradient {
        let mut gradient = CustomGradient {
            enabled: true,
            colors,
            ..CustomGradient::default()
        };
        gradient.ensure_color_stops();
        gradient
    }

    fn tiny_export_project(path: std::path::PathBuf) -> ProjectState {
        let mut project = ProjectState::default();
        project.export_settings.output_path = path;
        project.export_settings.width = 16;
        project.export_settings.height = 16;
        project
    }

    fn parity_project(path: std::path::PathBuf) -> ProjectState {
        ProjectState {
            export_settings: crate::export::export_settings::ExportSettings {
                width: 64,
                height: 64,
                fps: 5,
                duration_seconds: 1.0,
                lossy_quality: 100,
                fast: false,
                output_path: path,
            },
            render_params: RenderParams {
                patterns: vec![
                    PatternLayer {
                        name: "Preview parity domain pattern".to_owned(),
                        source: FormulaSource {
                            expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t)".to_owned(),
                            gain: 1.0,
                            bias: 0.0,
                            effect_blend_mode: None,
                            controls: vec![
                                FormulaControl {
                                    name: "P1".to_owned(),
                                    value: 2.6,
                                },
                                FormulaControl {
                                    name: "P2".to_owned(),
                                    value: -1.8,
                                },
                                FormulaControl {
                                    name: "P3".to_owned(),
                                    value: 4.1,
                                },
                                FormulaControl {
                                    name: "P4".to_owned(),
                                    value: 0.32,
                                },
                            ],
                            layers: vec![
                                FormulaLayer {
                                    name: "Domain driver".to_owned(),
                                    expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t)".to_owned(),
                                    domain_x: "x + sin(y * p3 + t) * p4".to_owned(),
                                    domain_y: "y + cos(x * p3 + u) * p4".to_owned(),
                                    domain_influence: 0.58,
                                    opacity: 0.9,
                                    ..FormulaLayer::default()
                                },
                                FormulaLayer {
                                    name: "Dependent detail".to_owned(),
                                    expression: "prev * 0.6 + 0.4 * (0.5 + 0.5 * cos(r * p3 + u))"
                                        .to_owned(),
                                    blend_mode: FormulaBlendMode::Screen,
                                    opacity: 0.72,
                                    ..FormulaLayer::default()
                                },
                            ],
                        },
                        strength: 0.78,
                        scale: 1.45,
                        motion: 0.8,
                        ..PatternLayer::default()
                    },
                    PatternLayer {
                        name: "Preview parity secondary pattern".to_owned(),
                        source: FormulaSource {
                            expression: "0.5 + 0.5 * cos((x * x - y * y) * scale * p1 + u)"
                                .to_owned(),
                            gain: 1.0,
                            bias: 0.0,
                            effect_blend_mode: None,
                            controls: vec![
                                FormulaControl {
                                    name: "P1".to_owned(),
                                    value: 2.9,
                                },
                                FormulaControl {
                                    name: "P2".to_owned(),
                                    value: 1.7,
                                },
                            ],
                            layers: vec![
                                FormulaLayer {
                                    name: "Quadratic secondary".to_owned(),
                                    expression: "0.5 + 0.5 * cos((x * x - y * y) * scale * p1 + u)"
                                        .to_owned(),
                                    ..FormulaLayer::default()
                                },
                                FormulaLayer {
                                    name: "Angular secondary".to_owned(),
                                    expression: "0.5 + 0.5 * sin(a * p2 + r * scale * 7 + t)"
                                        .to_owned(),
                                    blend_mode: FormulaBlendMode::Difference,
                                    opacity: 0.55,
                                    ..FormulaLayer::default()
                                },
                            ],
                        },
                        strength: 0.62,
                        scale: 1.9,
                        motion: -1.1,
                        ..PatternLayer::default()
                    },
                ],
                effects: vec![
                    EffectLayer {
                        source: FormulaSource {
                            expression: "0.5 + 0.5 * sin((x - y) * scale + t * motion)".to_owned(),
                            gain: 1.0,
                            bias: 0.0,
                            effect_blend_mode: Some(EffectBlendMode::Difference),
                            controls: Vec::new(),
                            layers: Vec::new(),
                        },
                        blend_mode: EffectBlendMode::Difference,
                        strength: 0.42,
                        scale: 2.5,
                        motion: 1.25,
                        ..EffectLayer::default()
                    },
                    EffectLayer {
                        source: FormulaSource {
                            expression: "0.6 + 0.4 * cos((x + y) * scale + u * motion)".to_owned(),
                            gain: 1.0,
                            bias: 0.0,
                            effect_blend_mode: Some(EffectBlendMode::Multiply),
                            controls: Vec::new(),
                            layers: Vec::new(),
                        },
                        blend_mode: EffectBlendMode::Multiply,
                        strength: 0.38,
                        scale: 3.2,
                        motion: 0.9,
                        ..EffectLayer::default()
                    },
                ],
                palette: PaletteKind::Aurora,
                custom_gradient: CustomGradient {
                    enabled: true,
                    colors: vec![
                        [0.02, 0.03, 0.18],
                        [0.00, 0.38, 0.78],
                        [0.05, 0.74, 0.58],
                        [0.93, 0.86, 0.18],
                        [0.95, 0.22, 0.62],
                        [0.32, 0.06, 0.52],
                        [0.02, 0.80, 0.92],
                        [0.16, 0.95, 0.36],
                        [1.00, 0.48, 0.05],
                        [0.72, 0.02, 0.12],
                    ],
                    color_transitions: vec![
                        vec![ColorTransition {
                            color: [0.18, 0.76, 0.95],
                        }],
                        vec![ColorTransition {
                            color: [0.85, 0.18, 0.72],
                        }],
                        vec![ColorTransition {
                            color: [0.92, 0.95, 0.24],
                        }],
                        vec![ColorTransition {
                            color: [0.18, 0.08, 0.82],
                        }],
                        vec![ColorTransition {
                            color: [0.95, 0.34, 0.18],
                        }],
                        vec![ColorTransition {
                            color: [0.07, 0.72, 0.42],
                        }],
                        vec![ColorTransition {
                            color: [0.82, 0.12, 0.86],
                        }],
                        vec![ColorTransition {
                            color: [0.12, 0.42, 0.95],
                        }],
                        vec![ColorTransition {
                            color: [0.97, 0.88, 0.11],
                        }],
                        vec![ColorTransition {
                            color: [0.42, 0.08, 0.17],
                        }],
                    ],
                    color_a: [0.02, 0.03, 0.18],
                    color_b: [0.00, 0.38, 0.78],
                    color_c: [0.05, 0.74, 0.58],
                    color_d: [0.93, 0.86, 0.18],
                    transition: 0.5,
                },
                color_speed: 0.7,
                color_phase: 0.1,
                brightness: 1.0,
                contrast: 1.15,
                zoom: 1.25,
                rotation_speed: 0.3,
                ..RenderParams::default()
            },
        }
    }

    fn preview_reference_frame(project: &ProjectState, frame_index: u32) -> FrameBuffer {
        let mut settings = project.export_settings.clone();
        settings.sanitize();
        let spec = FrameRenderSpec::new(
            frame_index,
            settings.total_frames(),
            settings.width,
            settings.height,
        );
        let mut renderers = BackendRenderers::new();
        let rendered = renderers
            .render_indexed_frame(&project.render_params, spec)
            .expect("render preview reference frame");
        apply_gif_quality_preview(rendered.frame, &settings)
    }

    #[derive(Debug)]
    struct DecodedGifFrame {
        width: u16,
        height: u16,
        delay: u16,
        pixels: Vec<u8>,
    }

    #[derive(Debug)]
    struct DecodedGif {
        repeat: gif::Repeat,
        frames: Vec<DecodedGifFrame>,
    }

    fn decode_gif_rgba(bytes: &[u8]) -> DecodedGif {
        let mut options = gif::DecodeOptions::new();
        options.set_color_output(gif::ColorOutput::RGBA);
        let mut decoder = options
            .read_info(Cursor::new(bytes))
            .expect("decode gif header");
        let repeat = decoder.repeat();
        let mut frames = Vec::new();
        while let Some(frame) = decoder.read_next_frame().expect("decode gif frame") {
            frames.push(DecodedGifFrame {
                width: frame.width,
                height: frame.height,
                delay: frame.delay,
                pixels: frame.buffer.to_vec(),
            });
        }

        DecodedGif { repeat, frames }
    }

    fn assert_frame_close_to_reference(
        decoded_pixels: &[u8],
        reference: &FrameBuffer,
        tolerance: f32,
    ) {
        assert_eq!(decoded_pixels.len(), reference.pixels.len());
        let average_delta = decoded_pixels
            .iter()
            .zip(&reference.pixels)
            .map(|(decoded, expected)| decoded.abs_diff(*expected) as u64)
            .sum::<u64>() as f32
            / decoded_pixels.len().max(1) as f32;

        assert!(
            average_delta <= tolerance,
            "decoded GIF frame drift {average_delta:.2} exceeds tolerance {tolerance:.2}"
        );
    }

    fn backend_statuses(
        progress: impl Iterator<Item = ExportProgress>,
    ) -> Vec<crate::project::render_settings::RenderBackendStatus> {
        progress
            .filter_map(|progress| match progress {
                ExportProgress::BackendStatus { status } => Some(status),
                _ => None,
            })
            .collect()
    }

    #[derive(Debug)]
    struct GifTiming {
        frames: usize,
        duration_seconds: f64,
    }

    fn gif_timing(bytes: &[u8]) -> GifTiming {
        let mut options = gif::DecodeOptions::new();
        options.skip_frame_decoding(true);
        let mut decoder = options
            .read_info(Cursor::new(bytes))
            .expect("decode gif header");
        let mut frames = 0usize;
        let mut centiseconds = 0u32;
        while let Some(frame) = decoder
            .read_next_frame()
            .expect("decode gif frame metadata")
        {
            frames += 1;
            centiseconds += frame.delay as u32;
        }

        GifTiming {
            frames,
            duration_seconds: centiseconds as f64 / 100.0,
        }
    }
}
