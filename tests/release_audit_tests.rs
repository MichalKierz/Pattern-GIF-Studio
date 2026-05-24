use std::{io::Cursor, path::PathBuf, sync::mpsc};

use pattern_gif_studio::{
    animation::loop_time::LoopTime,
    export::{
        gif_exporter::export_gif, gif_quality::apply_gif_quality_preview, progress::ExportProgress,
    },
    project::{
        project_state::ProjectState, render_settings::RenderBackend,
        workflow::load_bundled_workflow_preset,
    },
    render::{
        backend::{BackendRenderers, FrameRenderSpec},
        frame_buffer::FrameBuffer,
        gpu_renderer::GpuRenderer,
        renderer::{RenderParams, Renderer},
    },
};

#[test]
fn release_audit_exports_decodes_and_matches_preview_reference() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let preset_path = manifest
        .join("presets")
        .join("workflows")
        .join("deep-water-glass.json");
    let output_dir = manifest.join("target").join("release_audit");
    let output_path = output_dir.join("deep-water-glass-release-audit.gif");
    let _ = std::fs::remove_file(&output_path);

    let mut workflow = load_bundled_workflow_preset(&preset_path).expect("load release preset");
    workflow.project.export_settings.width = 64;
    workflow.project.export_settings.height = 64;
    workflow.project.export_settings.fps = 5;
    workflow.project.export_settings.duration_seconds = 1.0;
    workflow.project.export_settings.lossy_quality = 100;
    workflow.project.export_settings.fast = false;
    workflow.project.export_settings.output_path = output_path.clone();
    workflow.project.sanitize();

    let total_frames = workflow.project.export_settings.total_frames();
    let first_reference = preview_reference_frame(&workflow.project, 0);
    let (tx, rx) = mpsc::channel();

    export_gif(
        workflow.project.clone(),
        tx,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .expect("export release audit gif");

    let progress: Vec<_> = rx.try_iter().collect();
    assert!(
        progress
            .iter()
            .any(|event| matches!(event, ExportProgress::Finished { .. })),
        "release export should report Finished"
    );
    assert!(output_path.exists(), "release export artifact should exist");
    assert!(
        std::fs::metadata(&output_path)
            .expect("release gif metadata")
            .len()
            > 0,
        "release export artifact should be non-empty"
    );

    let decoded = decode_gif(&std::fs::read(&output_path).expect("read release gif"));

    assert_eq!(decoded.repeat, gif::Repeat::Infinite);
    assert_eq!(decoded.frames.len() as u32, total_frames);
    assert!(
        decoded.frames.iter().all(|frame| frame.delay == 20),
        "5 FPS / 1 second should encode every frame with 20 cs delay"
    );
    assert!(
        decoded
            .frames
            .iter()
            .all(|frame| frame.width == 64 && frame.height == 64),
        "decoded release GIF frames should keep export dimensions"
    );
    assert_frame_close_to_reference(&decoded.frames[0].pixels, &first_reference, 16.0);
}

#[test]
fn release_audit_gpu_export_decodes_and_reports_backend() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let preset_path = manifest
        .join("presets")
        .join("workflows")
        .join("ghastly-mandelbrot.json");
    let output_path = manifest
        .join("target")
        .join("release_audit")
        .join("orbit-trap-gpu-release-audit.gif");
    let _ = std::fs::remove_file(&output_path);

    let mut workflow = load_bundled_workflow_preset(&preset_path).expect("load GPU release preset");
    workflow.project.export_settings.width = 32;
    workflow.project.export_settings.height = 32;
    workflow.project.export_settings.fps = 3;
    workflow.project.export_settings.duration_seconds = 1.0;
    workflow.project.export_settings.lossy_quality = 100;
    workflow.project.export_settings.fast = false;
    workflow.project.export_settings.output_path = output_path.clone();
    workflow.project.sanitize();

    let total_frames = workflow.project.export_settings.total_frames();
    let first_reference = preview_reference_frame(&workflow.project, 0);
    let (tx, rx) = mpsc::channel();

    export_gif(
        workflow.project.clone(),
        tx,
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .expect("export GPU release audit gif");

    let progress: Vec<_> = rx.try_iter().collect();
    let backend_status = progress
        .iter()
        .find_map(|event| match event {
            ExportProgress::BackendStatus { status } => Some(status),
            _ => None,
        })
        .expect("GPU export should report backend status");

    assert_eq!(backend_status.used, RenderBackend::Gpu);
    assert!(backend_status.error.is_none());

    let decoded = decode_gif(&std::fs::read(&output_path).expect("read GPU release gif"));
    assert_eq!(decoded.repeat, gif::Repeat::Infinite);
    assert_eq!(decoded.frames.len() as u32, total_frames);
    assert_frame_close_to_reference(&decoded.frames[0].pixels, &first_reference, 18.0);
}

#[test]
fn release_audit_reports_gpu_adapter_status() {
    match GpuRenderer::new() {
        Ok(mut renderer) => {
            let params = RenderParams::default();
            let frame = renderer.render_frame(&params, LoopTime::from_frame(0, 24), 16, 16);
            println!("GPU_ADAPTER_AVAILABLE=true");
            assert_eq!(frame.width, 16);
            assert_eq!(frame.height, 16);
            assert_eq!(frame.pixels.len(), 16 * 16 * 4);
        }
        Err(error) => {
            println!("GPU_ADAPTER_AVAILABLE=false; reason={error}");
        }
    }
}

fn preview_reference_frame(project: &ProjectState, frame_index: u32) -> FrameBuffer {
    let settings = project.export_settings.clone();
    let spec = FrameRenderSpec::new(
        frame_index,
        settings.total_frames(),
        settings.width,
        settings.height,
    );
    let mut renderers = BackendRenderers::new();
    let rendered = renderers
        .render_indexed_frame(&project.render_params, spec)
        .expect("render release preview reference");
    assert_eq!(rendered.status.used, RenderBackend::Gpu);
    apply_gif_quality_preview(rendered.frame, &settings)
}

#[derive(Debug)]
struct DecodedFrame {
    width: u16,
    height: u16,
    delay: u16,
    pixels: Vec<u8>,
}

#[derive(Debug)]
struct DecodedGif {
    repeat: gif::Repeat,
    frames: Vec<DecodedFrame>,
}

fn decode_gif(bytes: &[u8]) -> DecodedGif {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options
        .read_info(Cursor::new(bytes))
        .expect("decode release gif header");
    let repeat = decoder.repeat();
    let mut frames = Vec::new();

    while let Some(frame) = decoder.read_next_frame().expect("decode release gif frame") {
        frames.push(DecodedFrame {
            width: frame.width,
            height: frame.height,
            delay: frame.delay,
            pixels: frame.buffer.to_vec(),
        });
    }

    DecodedGif { repeat, frames }
}

fn assert_frame_close_to_reference(decoded: &[u8], reference: &FrameBuffer, max_mean_delta: f64) {
    assert_eq!(decoded.len(), reference.pixels.len());
    let total_delta: u64 = decoded
        .iter()
        .zip(reference.pixels.iter())
        .map(|(decoded, expected)| decoded.abs_diff(*expected) as u64)
        .sum();
    let mean_delta = total_delta as f64 / decoded.len() as f64;

    assert!(
        mean_delta <= max_mean_delta,
        "decoded GIF frame differs from preview reference too much: mean channel delta {mean_delta:.2}, limit {max_mean_delta:.2}"
    );
}
