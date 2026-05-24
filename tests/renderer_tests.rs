use pattern_gif_studio::{
    animation::loop_time::LoopTime,
    export::export_settings::ExportSettings,
    project::{
        project_state::ProjectState,
        workflow::{
            WorkflowFile, load_bundled_workflow_preset, load_workflow_or_preset, save_workflow,
        },
    },
    render::{
        color::{ColorTransition, CustomGradient, PaletteKind},
        formula::{FormulaBlendMode, FormulaControl, FormulaLayer, FormulaSource},
        frame_buffer::{FrameBuffer, pixel_len},
        gpu_renderer::{GPU_GRADIENT_COLOR_LIMIT, GpuRenderer},
        renderer::{EffectBlendMode, EffectLayer, PatternLayer, RenderParams, Renderer},
    },
    source_assets::{asset::FormulaSourceAsset, storage::load_asset},
};

struct TestRenderer {
    gpu: GpuRenderer,
}

impl TestRenderer {
    fn new() -> Self {
        Self {
            gpu: GpuRenderer::new().expect("GPU renderer required for renderer behavior tests"),
        }
    }
}

impl Renderer for TestRenderer {
    fn render_frame(
        &mut self,
        params: &RenderParams,
        time: LoopTime,
        width: u32,
        height: u32,
    ) -> FrameBuffer {
        self.gpu.render_frame(params, time, width, height)
    }
}

#[test]
fn gpu_renderer_outputs_rgba_buffer_with_expected_dimensions() {
    let mut renderer = TestRenderer::new();
    let params = RenderParams {
        center_x: 0.2,
        center_y: -0.15,
        detail: 1.5,
        ..RenderParams::default()
    };

    let frame = renderer.render_frame(&params, LoopTime::from_frame(0, 24), 64, 48);

    assert_eq!(frame.width, 64);
    assert_eq!(frame.height, 48);
    assert_eq!(frame.pixels.len(), pixel_len(64, 48));
    assert!(frame.pixels.chunks_exact(4).all(|px| px[3] == 255));
}

#[test]
fn gpu_renderer_renders_dsl_pattern_sources_when_adapter_is_available() {
    let Ok(mut renderer) = GpuRenderer::new() else {
        return;
    };
    let pattern_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("patterns");
    for entry in std::fs::read_dir(&pattern_dir).expect("pattern preset dir") {
        let path = entry.expect("pattern preset").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pattern source");
        let asset: FormulaSourceAsset = load_asset(&path).expect("load fractal source");
        let mut params = RenderParams {
            patterns: vec![PatternLayer::new("Fractal", asset.source)],
            effects: Vec::new(),
            ..RenderParams::default()
        };
        params.palette = pattern_gif_studio::render::color::PaletteKind::MonoChrome;
        params.custom_gradient.enabled = false;
        params.zoom = 1.0;

        let frame = renderer.render_frame(&params, LoopTime::from_frame(4, 30), 64, 64);
        assert!(
            frame_has_luminance_range(&frame, 12),
            "{file_name} should not render as a flat color on GPU"
        );
    }
}

#[test]
fn gpu_renderer_uses_loaded_custom_gradient_colors_when_adapter_is_available() {
    let mut renderer = TestRenderer::new();
    let params = fixed_custom_gradient_params();
    let mut changed = params.clone();
    changed.custom_gradient.colors[0] = [0.0, 0.95, 0.15];
    changed.custom_gradient.colors[1] = [0.0, 0.2, 1.0];
    let time = LoopTime::from_frame(0, 24);

    let baseline = renderer.render_frame(&params, time, 8, 8);
    let changed = renderer.render_frame(&changed, time, 8, 8);

    assert!(
        frame_pixel_delta(&baseline, &changed) > 600,
        "GPU render must respond to loaded custom gradient colors"
    );
    assert!(
        frame_has_warm_palette(&baseline),
        "loaded brown/orange/yellow palette should not render as green/cyan"
    );
}

#[test]
fn gpu_renderer_uses_more_than_eight_custom_gradient_colors_when_adapter_is_available() {
    assert_eq!(
        GPU_GRADIENT_COLOR_LIMIT,
        pattern_gif_studio::render::color::MAX_GRADIENT_COLORS
    );
    let mut renderer = TestRenderer::new();
    let params = twelve_color_gradient_params();
    let mut truncated = params.clone();
    truncated.custom_gradient.colors.truncate(8);
    truncated.custom_gradient.color_transitions.truncate(8);
    let time = LoopTime::from_frame(5, 24);

    let full = renderer.render_frame(&params, time, 12, 12);
    let eight = renderer.render_frame(&truncated, time, 12, 12);

    assert!(
        average_channel_delta(&full, &eight) > 1.0,
        "GPU must not silently truncate custom gradients above eight colors"
    );
}

#[test]
fn gpu_renderer_renders_small_layered_scene_with_visual_detail() {
    let mut renderer = TestRenderer::new();
    let params = gpu_parity_scene_params();
    let time = LoopTime::from_frame(7, 30);

    let frame = renderer.render_frame(&params, time, 24, 24);

    assert!(
        frame_has_luminance_range(&frame, 20),
        "GPU layered scene should produce visible formula/domain/effect structure"
    );
}

#[test]
fn pattern_stack_composes_multiple_sources_and_order_is_weighted_average() {
    let mut renderer = TestRenderer::new();
    let time = LoopTime::from_frame(9, 30);
    let mut params = full_layer_composition_params();
    params.effects.clear();

    let composite = renderer.render_frame(&params, time, 72, 72);

    let mut first_only = params.clone();
    first_only.patterns.truncate(1);
    let first = renderer.render_frame(&first_only, time, 72, 72);

    let mut second_only = params.clone();
    second_only.patterns.remove(0);
    let second = renderer.render_frame(&second_only, time, 72, 72);

    assert!(
        frame_pixel_delta(&composite, &first) > 120_000,
        "two-pattern weighted composite should differ materially from pattern 1 alone"
    );
    assert!(
        frame_pixel_delta(&composite, &second) > 120_000,
        "two-pattern weighted composite should differ materially from pattern 2 alone"
    );

    let mut disabled_second = params.clone();
    disabled_second.patterns[1].enabled = false;
    let disabled = renderer.render_frame(&disabled_second, time, 72, 72);
    assert!(
        frame_pixel_delta(&composite, &disabled) > 120_000,
        "disabling one pattern should materially change the composed pattern stack"
    );

    let mut reversed = params;
    reversed.patterns.reverse();
    let reversed_frame = renderer.render_frame(&reversed, time, 72, 72);
    assert_eq!(
        composite.pixels, reversed_frame.pixels,
        "pattern order should not affect the weighted-average pattern stack"
    );
}

#[test]
fn formula_prev_domain_and_controls_are_visual_inputs() {
    let mut renderer = TestRenderer::new();
    let time = LoopTime::from_frame(9, 30);
    let mut params = full_layer_composition_params();
    params.effects.clear();

    let composed = renderer.render_frame(&params, time, 72, 72);

    let mut without_prev = params.clone();
    without_prev.patterns[0].source.layers[1].expression =
        "0.20 + 0.80 * (0.5 + 0.5 * cos((x - y) * p3 + u))".to_owned();
    let without_prev_frame = renderer.render_frame(&without_prev, time, 72, 72);
    assert!(
        frame_pixel_delta(&composed, &without_prev_frame) > 80_000,
        "formula layer using prev should materially affect the final pattern stack"
    );

    let mut without_domain = params.clone();
    without_domain.patterns[0].source.layers[0].domain_x = "x".to_owned();
    without_domain.patterns[0].source.layers[0].domain_y = "y".to_owned();
    without_domain.patterns[0].source.layers[0].domain_influence = 0.0;
    let without_domain_frame = renderer.render_frame(&without_domain, time, 72, 72);
    assert!(
        frame_pixel_delta(&composed, &without_domain_frame) > 80_000,
        "domain_x/domain_y should deform later formula layers in the stack"
    );

    let mut changed_controls = params;
    changed_controls.patterns[0].source.controls[0].value = 4.8;
    changed_controls.patterns[0].source.controls[1].value = 0.9;
    changed_controls.patterns[0].source.controls[2].value = 1.2;
    changed_controls.patterns[0].source.controls[3].value = 0.08;
    let changed_controls_frame = renderer.render_frame(&changed_controls, time, 72, 72);
    assert!(
        frame_pixel_delta(&composed, &changed_controls_frame) > 120_000,
        "p1/p2/p3/p4 controls should be visual inputs, not stored-only metadata"
    );
}

#[test]
fn effect_stack_composes_on_pattern_stack_and_order_matters() {
    let mut renderer = TestRenderer::new();
    let time = LoopTime::from_frame(9, 30);
    let params = full_layer_composition_params();

    let full = renderer.render_frame(&params, time, 72, 72);

    let mut no_effects = params.clone();
    no_effects.effects.clear();
    let baseline = renderer.render_frame(&no_effects, time, 72, 72);
    assert!(
        frame_pixel_delta(&baseline, &full) > 150_000,
        "effect stack should materially change the composed pattern stack"
    );

    let mut one_effect = params.clone();
    one_effect.effects.truncate(1);
    let one_effect_frame = renderer.render_frame(&one_effect, time, 72, 72);
    assert!(
        frame_pixel_delta(&one_effect_frame, &full) > 90_000,
        "two effect layers together should differ from only the first effect layer"
    );

    let mut reversed = params;
    reversed.effects.reverse();
    let reversed_frame = renderer.render_frame(&reversed, time, 72, 72);
    assert!(
        frame_pixel_delta(&full, &reversed_frame) > 40_000,
        "effect layer order should matter because effects are applied sequentially"
    );
}

#[test]
fn full_layer_composition_save_load_preserves_rendered_frame() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let workflow_path = temp_dir.path().join("full-composition-workflow.json");
    let project = ProjectState {
        render_params: full_layer_composition_params(),
        export_settings: ExportSettings {
            width: 72,
            height: 72,
            fps: 30,
            duration_seconds: 2.0,
            output_path: temp_dir.path().join("full-composition.gif"),
            ..ExportSettings::default()
        },
    };
    let time = LoopTime::from_frame(9, 30);
    let mut renderer = TestRenderer::new();
    let before = renderer.render_frame(&project.render_params, time, 72, 72);

    let workflow = WorkflowFile::from_project(project);
    save_workflow(&workflow_path, &workflow).expect("save full composition workflow");
    let loaded = load_workflow_or_preset(&workflow_path).expect("load full composition workflow");
    let after = renderer.render_frame(&loaded.project.render_params, time, 72, 72);

    assert_eq!(
        before.pixels, after.pixels,
        "save/load must preserve the rendered output of a full layer composition"
    );
}

#[test]
fn gpu_renderer_renders_full_layer_composition_with_visual_detail() {
    let mut renderer = TestRenderer::new();
    let params = full_layer_composition_params();
    let time = LoopTime::from_frame(9, 30);

    let frame = renderer.render_frame(&params, time, 28, 28);

    assert!(
        frame_has_luminance_range(&frame, 20),
        "GPU full pattern/formula/domain/effect composition should produce visible structure"
    );
}

#[test]
fn gpu_renderer_animates_custom_gradient_transitions() {
    let mut renderer = TestRenderer::new();
    let params = fixed_custom_gradient_params();

    let start = renderer.render_frame(&params, LoopTime::from_frame(0, 24), 8, 8);
    let mid = renderer.render_frame(&params, LoopTime::from_frame(12, 24), 8, 8);

    assert!(
        frame_pixel_delta(&start, &mid) > 4_000,
        "custom gradient transitions should animate colors over the GIF loop"
    );
}

#[test]
fn gpu_color_transition_loop_endpoint_matches_start() {
    let mut renderer = TestRenderer::new();
    let params = fixed_custom_gradient_params();

    let start = renderer.render_frame(&params, LoopTime::from_frame(0, 24), 16, 16);
    let endpoint = renderer.render_frame(&params, LoopTime::from_frame(24, 24), 16, 16);

    assert_eq!(
        start.pixels, endpoint.pixels,
        "color transition animation should close at the loop endpoint"
    );
}

#[test]
fn active_custom_gradient_changes_render_and_palettekind_does_not_override_it() {
    let mut renderer = TestRenderer::new();
    let time = LoopTime::from_frame(3, 24);
    let baseline = fixed_custom_gradient_params();
    let mut changed_gradient = baseline.clone();
    changed_gradient.custom_gradient.colors[1] = [0.0, 0.95, 0.95];
    changed_gradient.custom_gradient.colors[3] = [0.95, 0.05, 0.75];
    let mut changed_palette_kind = baseline.clone();
    changed_palette_kind.palette = PaletteKind::MonoChrome;

    let base_frame = renderer.render_frame(&baseline, time, 32, 32);
    let changed_gradient_frame = renderer.render_frame(&changed_gradient, time, 32, 32);
    let changed_palette_kind_frame = renderer.render_frame(&changed_palette_kind, time, 32, 32);

    assert!(
        frame_pixel_delta(&base_frame, &changed_gradient_frame) > 20_000,
        "changing the active CustomGradient must visibly change render colors"
    );
    assert_eq!(
        base_frame.pixels, changed_palette_kind_frame.pixels,
        "PaletteKind must not override an active CustomGradient"
    );
}

#[test]
fn gpu_renderer_uses_more_than_eight_gradient_colors_and_gradient_order() {
    let mut renderer = TestRenderer::new();
    let params = twelve_color_gradient_params();
    let mut truncated = params.clone();
    truncated.custom_gradient.colors.truncate(8);
    truncated.custom_gradient.color_transitions.truncate(8);
    truncated.custom_gradient.ensure_color_stops();
    let mut reversed = params.clone();
    reversed.custom_gradient.colors.reverse();
    reversed.custom_gradient.color_transitions.reverse();
    reversed.custom_gradient.ensure_color_stops();
    let time = LoopTime::from_frame(5, 24);

    let full = renderer.render_frame(&params, time, 64, 64);
    let eight = renderer.render_frame(&truncated, time, 64, 64);
    let reversed_frame = renderer.render_frame(&reversed, time, 64, 64);

    assert!(
        frame_pixel_delta(&full, &eight) > 60_000,
        "colors 9..12 must influence GPU render instead of being ignored"
    );
    assert!(
        frame_pixel_delta(&full, &reversed_frame) > 60_000,
        "gradient stop order is semantic because gradient sampling interpolates neighboring stops"
    );
}

#[test]
fn adding_and_removing_gradient_colors_changes_gpu_render() {
    let mut renderer = TestRenderer::new();
    let mut params = twelve_color_gradient_params();
    let time = LoopTime::from_frame(7, 24);
    let base = renderer.render_frame(&params, time, 48, 48);

    params.custom_gradient.add_color();
    let last = params.custom_gradient.colors.len() - 1;
    params.custom_gradient.colors[last] = [1.0, 1.0, 1.0];
    params.custom_gradient.ensure_color_stops();
    let added = renderer.render_frame(&params, time, 48, 48);

    params.custom_gradient.remove_color(last);
    params.custom_gradient.remove_color(1);
    let removed = renderer.render_frame(&params, time, 48, 48);

    assert!(
        frame_pixel_delta(&base, &added) > 20_000,
        "adding a color stop should affect the active gradient render"
    );
    assert!(
        frame_pixel_delta(&base, &removed) > 20_000,
        "removing a color stop should affect the active gradient render"
    );
}

#[test]
fn gpu_renderer_animates_custom_gradient_transitions_when_adapter_is_available() {
    let Ok(mut renderer) = GpuRenderer::new() else {
        return;
    };
    let params = fixed_custom_gradient_params();

    let start = renderer.render_frame(&params, LoopTime::from_frame(0, 24), 8, 8);
    let mid = renderer.render_frame(&params, LoopTime::from_frame(12, 24), 8, 8);

    assert_ne!(
        start.pixels, mid.pixels,
        "GPU preview should animate the same custom gradient transitions as export"
    );
}

#[test]
fn dynamic_pattern_layers_render_without_invalid_alpha() {
    let mut renderer = TestRenderer::new();
    let params = RenderParams {
        patterns: vec![
            PatternLayer::new("A", FormulaSource::fractal_a()),
            PatternLayer::new("B", FormulaSource::fractal_b()),
        ],
        ..RenderParams::default()
    };

    let frame = renderer.render_frame(&params, LoopTime::from_frame(7, 30), 32, 32);

    assert!(frame.pixels.chunks_exact(4).all(|px| px[3] == 255));
}

#[test]
fn custom_formula_layers_render_as_perfect_loop() {
    let mut renderer = TestRenderer::new();
    let mut params = RenderParams {
        patterns: vec![
            PatternLayer::new("A", FormulaSource::fractal_a()),
            PatternLayer::new("B", FormulaSource::fractal_b()),
        ],
        effects: vec![EffectLayer::new("Effect", FormulaSource::pattern())],
        ..RenderParams::default()
    };
    params.patterns[0].source.layers[0].expression =
        "sin(r * 11 + t * 4) + noise(x * 8, y * 8)".to_owned();
    params.patterns[1].source.layers[0].expression = "cos((x * x - y * y) * 16 + u * 3)".to_owned();
    params.effects[0].source.layers[0].expression =
        "sin(a * symmetry + r * scale * 9 + t * motion)".to_owned();
    params.effects[0].scale = 2.0;
    params.effects[0].motion = 1.5;

    let start = renderer.render_frame(&params, LoopTime::from_frame(0, 32), 40, 40);
    let endpoint = renderer.render_frame(&params, LoopTime::from_frame(32, 32), 40, 40);

    assert_eq!(start.pixels, endpoint.pixels);
}

#[test]
fn disabled_effect_layer_does_not_override_loaded_patterns() {
    let mut renderer = TestRenderer::new();
    let without_effect = RenderParams {
        patterns: vec![
            PatternLayer::new("A", FormulaSource::fractal_a()),
            PatternLayer::new("B", FormulaSource::fractal_b()),
        ],
        effects: Vec::new(),
        ..RenderParams::default()
    };

    let mut disabled_effect = without_effect.clone();
    disabled_effect.effects.push(EffectLayer {
        enabled: false,
        source: FormulaSource {
            expression: "0.0".to_owned(),
            gain: 1.0,
            bias: 0.0,
            effect_blend_mode: None,
            controls: Vec::new(),
            layers: Vec::new(),
        },
        ..EffectLayer::default()
    });

    let time = LoopTime::from_frame(9, 30);
    let base = renderer.render_frame(&without_effect, time, 48, 48);
    let with_disabled_effect = renderer.render_frame(&disabled_effect, time, 48, 48);

    assert_eq!(base.pixels, with_disabled_effect.pixels);
}

#[test]
fn disabled_pattern_layer_does_not_contribute_to_render() {
    let mut renderer = TestRenderer::new();
    let single_pattern = RenderParams {
        patterns: vec![PatternLayer::new("A", FormulaSource::fractal_a())],
        effects: Vec::new(),
        ..RenderParams::default()
    };

    let mut disabled_extra_pattern = single_pattern.clone();
    disabled_extra_pattern.patterns.push(PatternLayer {
        enabled: false,
        source: FormulaSource {
            expression: "0.0".to_owned(),
            gain: 1.0,
            bias: 0.0,
            effect_blend_mode: None,
            controls: Vec::new(),
            layers: Vec::new(),
        },
        ..PatternLayer::default()
    });

    let time = LoopTime::from_frame(11, 30);
    let base = renderer.render_frame(&single_pattern, time, 48, 48);
    let with_disabled_pattern = renderer.render_frame(&disabled_extra_pattern, time, 48, 48);

    assert_eq!(base.pixels, with_disabled_pattern.pixels);
}

#[test]
fn gpu_smoothing_reduces_high_frequency_pixel_noise() {
    let mut renderer = TestRenderer::new();
    let mut params = high_frequency_smoothing_params();
    params.smoothing = 0.0;
    let aliased = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 96, 96);

    params.smoothing = 20.0;
    let smoothed = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 96, 96);

    assert_ne!(aliased.pixels, smoothed.pixels);
    assert!(
        horizontal_luminance_delta(&smoothed) < horizontal_luminance_delta(&aliased),
        "smoothing should reduce neighbor-to-neighbor luminance jumps"
    );
}

#[test]
fn gpu_smoothing_radius_pixels_expands_filter_reach() {
    let mut renderer = TestRenderer::new();
    let mut params = high_frequency_smoothing_params();
    params.smoothing = 20.0;
    params.smoothing_radius_pixels = 1.0;
    let one_pixel = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 96, 96);

    params.smoothing_radius_pixels = 10.0;
    let ten_pixels = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 96, 96);

    assert_ne!(
        one_pixel.pixels, ten_pixels.pixels,
        "smoothing radius should be an actual render parameter, not a dormant UI slider"
    );
}

#[test]
fn gpu_smoothing_does_not_turn_into_full_image_blur() {
    let mut renderer = TestRenderer::new();
    let mut params = low_frequency_smoothing_params();
    params.smoothing = 0.0;
    let crisp = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 96, 96);

    params.smoothing = 20.0;
    let smoothed = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 96, 96);

    let crisp_delta = horizontal_luminance_delta(&crisp);
    let smoothed_delta = horizontal_luminance_delta(&smoothed);
    assert!(
        smoothed_delta > crisp_delta * 9 / 10,
        "smoothing should preserve low-frequency structure instead of blurring the whole image"
    );
}

#[test]
fn gpu_smoothing_changes_high_frequency_render_when_adapter_is_available() {
    let Ok(mut renderer) = GpuRenderer::new() else {
        return;
    };
    let mut params = high_frequency_smoothing_params();
    params.smoothing = 0.0;
    let aliased = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 64, 64);

    params.smoothing = 20.0;
    let smoothed = renderer.render_frame(&params, LoopTime::from_frame(3, 24), 64, 64);

    assert_ne!(
        aliased.pixels, smoothed.pixels,
        "GPU smoothing should affect high-frequency render output"
    );
}

#[test]
fn bundled_pattern_sources_render_visible_structure_and_respond_to_scale() {
    let pattern_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("patterns");
    for entry in std::fs::read_dir(&pattern_dir).expect("pattern preset dir") {
        let path = entry.expect("pattern preset").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pattern source");
        let asset: FormulaSourceAsset = load_asset(&path).expect("load fractal source");
        let mut renderer = TestRenderer::new();
        let mut params = RenderParams {
            patterns: vec![PatternLayer::new("Fractal", asset.source)],
            effects: Vec::new(),
            ..RenderParams::default()
        };
        params.palette = pattern_gif_studio::render::color::PaletteKind::MonoChrome;
        params.custom_gradient.enabled = false;
        params.zoom = 1.0;

        let base = renderer.render_frame(&params, LoopTime::from_frame(4, 30), 64, 64);
        assert!(
            frame_has_luminance_range(&base, 12),
            "{file_name} should not render as a flat color"
        );

        params.patterns[0].scale = 2.5;
        let scaled = renderer.render_frame(&params, LoopTime::from_frame(4, 30), 64, 64);
        assert_ne!(
            base.pixels, scaled.pixels,
            "{file_name} should respond to scale changes"
        );
    }
}

#[test]
fn bundled_pattern_sources_are_visually_distinct_without_effects() {
    let pattern_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("patterns");
    let mut rendered = Vec::new();
    for entry in std::fs::read_dir(&pattern_dir).expect("pattern preset dir") {
        let path = entry.expect("pattern preset").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pattern source")
            .to_owned();
        let asset: FormulaSourceAsset = load_asset(&path).expect("load pattern source");
        let mut renderer = TestRenderer::new();
        let mut params = RenderParams {
            patterns: vec![PatternLayer::new("Pattern", asset.source)],
            effects: Vec::new(),
            ..RenderParams::default()
        };
        params.palette = pattern_gif_studio::render::color::PaletteKind::MonoChrome;
        params.custom_gradient.enabled = false;
        params.zoom = 1.0;
        rendered.push((
            file_name,
            renderer.render_frame(&params, LoopTime::from_frame(4, 30), 72, 72),
        ));
    }

    for left_index in 0..rendered.len() {
        for right_index in (left_index + 1)..rendered.len() {
            let (left_name, left_frame) = &rendered[left_index];
            let (right_name, right_frame) = &rendered[right_index];
            let delta = frame_pixel_delta(left_frame, right_frame);
            assert!(
                delta > 160_000,
                "{left_name} and {right_name} are too visually similar without effects; delta={delta}"
            );
        }
    }
}

#[test]
fn formula_domain_pipeline_changes_later_layer_coordinates() {
    let mut renderer = TestRenderer::new();
    let base_source = FormulaSource {
        expression: "sin(x * 8)".to_owned(),
        gain: 1.0,
        bias: 0.0,
        effect_blend_mode: None,
        controls: Vec::new(),
        layers: vec![
            FormulaLayer {
                name: "Warp driver".to_owned(),
                expression: "0.5 + 0.5 * sin(x * 6 + y * 2)".to_owned(),
                domain_x: "x + sin(y * 5) * 0.55".to_owned(),
                domain_y: "y + cos(x * 5) * 0.35".to_owned(),
                domain_influence: 0.0,
                ..FormulaLayer::default()
            },
            FormulaLayer {
                name: "Dependent layer".to_owned(),
                expression: "0.5 + 0.5 * sin(x * 10 + y * 3)".to_owned(),
                blend_mode: pattern_gif_studio::render::formula::FormulaBlendMode::Difference,
                opacity: 0.8,
                ..FormulaLayer::default()
            },
        ],
    };
    let mut warped_source = base_source.clone();
    warped_source.layers[0].domain_influence = 1.0;

    let params_without_warp = RenderParams {
        patterns: vec![PatternLayer::new("No domain warp", base_source)],
        effects: Vec::new(),
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        ..RenderParams::default()
    };
    let params_with_warp = RenderParams {
        patterns: vec![PatternLayer::new("Domain warp", warped_source)],
        ..params_without_warp.clone()
    };

    let time = LoopTime::from_frame(5, 30);
    let base = renderer.render_frame(&params_without_warp, time, 64, 64);
    let warped = renderer.render_frame(&params_with_warp, time, 64, 64);

    assert!(
        frame_pixel_delta(&base, &warped) > 20_000,
        "domain pipeline must materially change dependent layer coordinates"
    );
}

#[test]
fn loaded_pattern_sources_stay_visible_at_high_zoom() {
    let pattern_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("patterns");
    for entry in std::fs::read_dir(&pattern_dir).expect("pattern preset dir") {
        let path = entry.expect("pattern preset").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pattern source");
        let asset: FormulaSourceAsset = load_asset(&path).expect("load pattern source");
        let mut renderer = TestRenderer::new();
        let mut params = RenderParams {
            patterns: vec![PatternLayer::new("Pattern", asset.source)],
            effects: Vec::new(),
            ..RenderParams::default()
        };
        params.palette = pattern_gif_studio::render::color::PaletteKind::MonoChrome;
        params.custom_gradient.enabled = false;
        params.zoom = 8.0;
        params.center_x = 0.0;
        params.center_y = 0.0;

        let frame = renderer.render_frame(&params, LoopTime::from_frame(4, 30), 64, 64);
        assert!(
            frame_has_luminance_range(&frame, 8),
            "{file_name} should not collapse into a flat color after Load Source at high zoom"
        );
    }
}

#[test]
fn bundled_effect_sources_visibly_modulate_loaded_patterns() {
    let pattern_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("patterns")
        .join("topographic-map-source.json");
    let pattern_asset: FormulaSourceAsset =
        load_asset(&pattern_path).expect("load base pattern source");

    let effect_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("effects");
    for entry in std::fs::read_dir(&effect_dir).expect("effect preset dir") {
        let path = entry.expect("effect preset").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("effect source");
        let effect_asset: FormulaSourceAsset = load_asset(&path).expect("load effect source");

        let mut renderer = TestRenderer::new();
        let mut base_params = RenderParams {
            patterns: vec![PatternLayer::new("Pattern", pattern_asset.source.clone())],
            effects: Vec::new(),
            ..RenderParams::default()
        };
        base_params.palette = pattern_gif_studio::render::color::PaletteKind::MonoChrome;
        base_params.custom_gradient.enabled = false;
        base_params.zoom = 3.0;

        let time = LoopTime::from_frame(6, 30);
        let base = renderer.render_frame(&base_params, time, 64, 64);

        let mut with_effect = base_params;
        with_effect.effects.push(EffectLayer {
            source: effect_asset.source,
            strength: 0.85,
            scale: 2.0,
            motion: 0.8,
            ..EffectLayer::default()
        });
        let modulated = renderer.render_frame(&with_effect, time, 64, 64);

        let delta = frame_pixel_delta(&base, &modulated);
        assert!(
            delta > 900,
            "{file_name} should visibly modulate the active pattern, delta was {delta}"
        );
    }
}

#[test]
fn effect_blend_modes_are_materially_visible() {
    let time = LoopTime::from_frame(5, 30);
    let mut renderer = TestRenderer::new();
    let base_params = effect_mode_test_params(None);
    let base = renderer.render_frame(&base_params, time, 64, 64);

    for mode in EffectBlendMode::ALL {
        let params = effect_mode_test_params(Some(mode));
        let frame = renderer.render_frame(&params, time, 64, 64);
        let delta = frame_pixel_delta(&base, &frame);
        assert!(
            delta > 12_000,
            "{} effect mode should visibly change the rendered GIF frame, delta={delta}",
            mode.label()
        );

        let mut disabled = params.clone();
        disabled.effects[0].enabled = false;
        let disabled_frame = renderer.render_frame(&disabled, time, 64, 64);
        assert_eq!(
            base.pixels,
            disabled_frame.pixels,
            "{} disabled effect should restore the no-effect render",
            mode.label()
        );

        let mut zero_strength = params;
        zero_strength.effects[0].strength = 0.0;
        let zero_frame = renderer.render_frame(&zero_strength, time, 64, 64);
        assert_eq!(
            base.pixels,
            zero_frame.pixels,
            "{} strength=0 should restore the no-effect render",
            mode.label()
        );
    }
}

#[test]
fn effect_strength_controls_visual_amount_for_each_blend_mode() {
    let time = LoopTime::from_frame(5, 30);
    let mut renderer = TestRenderer::new();
    let base_params = effect_mode_test_params(None);
    let base = renderer.render_frame(&base_params, time, 64, 64);

    for mode in EffectBlendMode::ALL {
        let mut half_strength = effect_mode_test_params(Some(mode));
        half_strength.effects[0].strength = 0.5;
        let half = renderer.render_frame(&half_strength, time, 64, 64);

        let mut full_strength = half_strength;
        full_strength.effects[0].strength = 1.0;
        let full = renderer.render_frame(&full_strength, time, 64, 64);

        let half_delta = frame_pixel_delta(&base, &half);
        let full_delta = frame_pixel_delta(&base, &full);
        assert!(
            half_delta > 1_000,
            "{} strength=0.5 should be visibly different from no effect, delta={half_delta}",
            mode.label()
        );
        assert!(
            full_delta > 1_000,
            "{} strength=1.0 should be visibly different from no effect, delta={full_delta}",
            mode.label()
        );
        assert!(
            frame_pixel_delta(&half, &full) > 1_000,
            "{} strength=0.5 and strength=1.0 should produce visibly different pixels",
            mode.label()
        );
    }
}

#[test]
fn effect_scale_and_motion_parameters_change_rendered_output() {
    let time = LoopTime::from_frame(7, 30);
    let mut renderer = TestRenderer::new();

    let mut low_scale = effect_parameter_test_params();
    low_scale.effects[0].scale = 0.8;
    let low_scale_frame = renderer.render_frame(&low_scale, time, 64, 64);

    let mut high_scale = low_scale.clone();
    high_scale.effects[0].scale = 4.0;
    let high_scale_frame = renderer.render_frame(&high_scale, time, 64, 64);
    let scale_delta = frame_pixel_delta(&low_scale_frame, &high_scale_frame);
    assert!(
        scale_delta > 8_000,
        "effect scale should materially change render output, delta={scale_delta}"
    );

    let mut slow_motion = effect_parameter_test_params();
    slow_motion.effects[0].motion = -1.0;
    let slow_motion_frame = renderer.render_frame(&slow_motion, time, 64, 64);

    let mut fast_motion = slow_motion.clone();
    fast_motion.effects[0].motion = 2.5;
    let fast_motion_frame = renderer.render_frame(&fast_motion, time, 64, 64);
    let motion_delta = frame_pixel_delta(&slow_motion_frame, &fast_motion_frame);
    assert!(
        motion_delta > 5_000,
        "effect looped motion should materially change render output, delta={motion_delta}"
    );
}

#[test]
fn gpu_effect_blend_modes_change_render_output() {
    let mut renderer = TestRenderer::new();
    let time = LoopTime::from_frame(5, 30);
    let baseline = renderer.render_frame(&effect_mode_test_params(None), time, 24, 24);

    for mode in EffectBlendMode::ALL {
        let params = effect_mode_test_params(Some(mode));
        let frame = renderer.render_frame(&params, time, 24, 24);
        let average_delta = average_channel_delta(&baseline, &frame);
        assert!(
            average_delta >= 2.0,
            "{} effect should visibly change GPU output: {average_delta:.2}",
            mode.label()
        );
    }
}

#[test]
fn bundled_presets_render_identical_cycle_endpoint() {
    let preset_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("workflows");
    let mut renderer = TestRenderer::new();

    for entry in std::fs::read_dir(&preset_dir).expect("preset dir") {
        let path = entry.expect("preset entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let workflow = load_bundled_workflow_preset(&path).expect("load bundled preset");
        let start = renderer.render_frame(
            &workflow.project.render_params,
            LoopTime::from_frame(0, 24),
            48,
            48,
        );
        let endpoint = renderer.render_frame(
            &workflow.project.render_params,
            LoopTime::from_frame(24, 24),
            48,
            48,
        );
        assert_eq!(
            start.pixels,
            endpoint.pixels,
            "preset {} must be a perfect loop endpoint",
            path.display()
        );
    }
}

fn fixed_custom_gradient_params() -> RenderParams {
    let mut params = RenderParams {
        patterns: vec![PatternLayer::new(
            "Flat value",
            FormulaSource {
                expression: "0.35".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
        )],
        effects: Vec::new(),
        palette: PaletteKind::Fire,
        custom_gradient: CustomGradient {
            enabled: true,
            colors: vec![
                [0.00, 0.00, 0.00],
                [0.38, 0.08, 0.00],
                [0.86, 0.42, 0.02],
                [1.00, 0.85, 0.16],
                [0.95, 1.00, 0.55],
            ],
            color_transitions: vec![
                vec![ColorTransition {
                    color: [0.12, 0.00, 0.22],
                }],
                vec![ColorTransition {
                    color: [0.65, 0.18, 0.02],
                }],
                vec![ColorTransition {
                    color: [1.00, 0.28, 0.00],
                }],
                vec![ColorTransition {
                    color: [0.95, 1.00, 0.08],
                }],
                vec![ColorTransition {
                    color: [1.00, 0.70, 0.24],
                }],
            ],
            color_a: [0.00, 0.00, 0.00],
            color_b: [0.38, 0.08, 0.00],
            color_c: [0.86, 0.42, 0.02],
            color_d: [1.00, 0.85, 0.16],
            transition: 0.5,
        },
        color_speed: 1.0,
        color_phase: 0.0,
        brightness: 1.0,
        contrast: 1.0,
        ..RenderParams::default()
    };
    params.custom_gradient.enabled = true;
    params
}

fn twelve_color_gradient_params() -> RenderParams {
    RenderParams {
        patterns: vec![PatternLayer::new(
            "Gradient index",
            FormulaSource {
                expression: "fract(x * 0.41 + y * 0.37 + 0.72)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
        )],
        effects: Vec::new(),
        palette: PaletteKind::Fire,
        custom_gradient: CustomGradient {
            enabled: true,
            colors: vec![
                [0.00, 0.00, 0.00],
                [0.12, 0.00, 0.22],
                [0.28, 0.02, 0.48],
                [0.07, 0.18, 0.72],
                [0.00, 0.42, 0.78],
                [0.00, 0.68, 0.62],
                [0.14, 0.84, 0.28],
                [0.58, 0.90, 0.10],
                [0.92, 0.72, 0.04],
                [1.00, 0.42, 0.02],
                [0.92, 0.10, 0.08],
                [0.62, 0.00, 0.00],
            ],
            color_transitions: vec![Vec::new(); 12],
            color_a: [0.00, 0.00, 0.00],
            color_b: [0.12, 0.00, 0.22],
            color_c: [0.28, 0.02, 0.48],
            color_d: [0.07, 0.18, 0.72],
            transition: 0.5,
        },
        color_speed: 0.0,
        color_phase: 0.0,
        brightness: 1.0,
        contrast: 1.0,
        zoom: 1.0,
        ..RenderParams::default()
    }
}

fn gpu_parity_scene_params() -> RenderParams {
    RenderParams {
        patterns: vec![PatternLayer {
            source: FormulaSource {
                expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: vec![
                    FormulaControl {
                        name: "P1".to_owned(),
                        value: 2.4,
                    },
                    FormulaControl {
                        name: "P2".to_owned(),
                        value: -1.7,
                    },
                    FormulaControl {
                        name: "P3".to_owned(),
                        value: 3.2,
                    },
                    FormulaControl {
                        name: "P4".to_owned(),
                        value: 0.35,
                    },
                ],
                layers: vec![
                    FormulaLayer {
                        name: "Domain driver".to_owned(),
                        expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t)".to_owned(),
                        domain_x: "x + sin(y * p3 + t) * p4".to_owned(),
                        domain_y: "y + cos(x * p3 + u) * p4".to_owned(),
                        domain_influence: 0.65,
                        opacity: 0.9,
                        ..FormulaLayer::default()
                    },
                    FormulaLayer {
                        name: "Prev dependent".to_owned(),
                        expression: "prev * 0.55 + 0.45 * (0.5 + 0.5 * cos(x * p2 - y * p1 + u))"
                            .to_owned(),
                        blend_mode: FormulaBlendMode::Screen,
                        opacity: 0.7,
                        ..FormulaLayer::default()
                    },
                ],
            },
            strength: 1.0,
            scale: 1.35,
            motion: 0.8,
            ..PatternLayer::default()
        }],
        effects: vec![EffectLayer {
            source: FormulaSource {
                expression: "0.5 + 0.5 * sin((x + y) * scale + t * motion)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: Some(EffectBlendMode::Screen),
                controls: Vec::new(),
                layers: Vec::new(),
            },
            blend_mode: EffectBlendMode::Screen,
            strength: 0.35,
            scale: 2.0,
            motion: 1.1,
            ..EffectLayer::default()
        }],
        palette: PaletteKind::Aurora,
        custom_gradient: CustomGradient {
            enabled: true,
            colors: vec![
                [0.02, 0.03, 0.18],
                [0.00, 0.42, 0.78],
                [0.05, 0.72, 0.56],
                [0.90, 0.88, 0.20],
                [0.95, 0.22, 0.62],
                [0.30, 0.06, 0.52],
            ],
            color_transitions: vec![Vec::new(); 6],
            color_a: [0.02, 0.03, 0.18],
            color_b: [0.00, 0.42, 0.78],
            color_c: [0.05, 0.72, 0.56],
            color_d: [0.90, 0.88, 0.20],
            transition: 0.5,
        },
        color_speed: 0.4,
        color_phase: 0.12,
        brightness: 1.0,
        contrast: 1.1,
        zoom: 1.2,
        rotation_speed: 0.25,
        ..RenderParams::default()
    }
}

fn full_layer_composition_params() -> RenderParams {
    RenderParams {
        patterns: vec![
            PatternLayer {
                name: "Domain controlled primary".to_owned(),
                source: FormulaSource {
                    expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t * motion)".to_owned(),
                    gain: 1.0,
                    bias: 0.0,
                    effect_blend_mode: None,
                    controls: vec![
                        FormulaControl {
                            name: "P1 density".to_owned(),
                            value: 2.4,
                        },
                        FormulaControl {
                            name: "P2 skew".to_owned(),
                            value: -1.35,
                        },
                        FormulaControl {
                            name: "P3 domain frequency".to_owned(),
                            value: 4.2,
                        },
                        FormulaControl {
                            name: "P4 domain amount".to_owned(),
                            value: 0.34,
                        },
                    ],
                    layers: vec![
                        FormulaLayer {
                            name: "Domain carrier".to_owned(),
                            expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t * motion)".to_owned(),
                            domain_x: "x + sin(y * p3 + t) * p4".to_owned(),
                            domain_y: "y + cos(x * p3 + u) * p4".to_owned(),
                            domain_influence: 0.72,
                            opacity: 0.95,
                            ..FormulaLayer::default()
                        },
                        FormulaLayer {
                            name: "Prev dependent detail".to_owned(),
                            expression:
                                "prev * (0.20 + 0.80 * (0.5 + 0.5 * cos((x - y) * p3 + u)))"
                                    .to_owned(),
                            blend_mode: FormulaBlendMode::Replace,
                            opacity: 0.95,
                            repeat_x: 1.3,
                            repeat_y: 0.9,
                            warp_x: 0.35,
                            warp_y: -0.28,
                            ..FormulaLayer::default()
                        },
                    ],
                },
                strength: 0.78,
                scale: 1.45,
                motion: 0.9,
                camera_orbit: 0.08,
                ..PatternLayer::default()
            },
            PatternLayer {
                name: "Secondary lattice".to_owned(),
                source: FormulaSource {
                    expression: "0.5 + 0.5 * cos((x * x - y * y) * scale * p1 + u)".to_owned(),
                    gain: 1.0,
                    bias: 0.0,
                    effect_blend_mode: None,
                    controls: vec![
                        FormulaControl {
                            name: "P1 lattice frequency".to_owned(),
                            value: 3.1,
                        },
                        FormulaControl {
                            name: "P2 ripple".to_owned(),
                            value: 1.6,
                        },
                    ],
                    layers: vec![
                        FormulaLayer {
                            name: "Quadratic lattice".to_owned(),
                            expression: "0.5 + 0.5 * cos((x * x - y * y) * scale * p1 + u)"
                                .to_owned(),
                            opacity: 1.0,
                            ..FormulaLayer::default()
                        },
                        FormulaLayer {
                            name: "Angular ripple".to_owned(),
                            expression: "0.5 + 0.5 * sin(a * p2 + r * scale * 8 + t)".to_owned(),
                            blend_mode: FormulaBlendMode::Difference,
                            opacity: 0.58,
                            ..FormulaLayer::default()
                        },
                    ],
                },
                strength: 0.62,
                scale: 2.1,
                motion: -1.15,
                camera_zoom_loop: 0.12,
                ..PatternLayer::default()
            },
        ],
        effects: vec![
            EffectLayer {
                name: "Multiply modulation".to_owned(),
                source: FormulaSource {
                    expression: "0.55 + 0.45 * sin((x + y) * scale + t * motion)".to_owned(),
                    gain: 1.0,
                    bias: 0.0,
                    effect_blend_mode: Some(EffectBlendMode::Multiply),
                    controls: Vec::new(),
                    layers: vec![FormulaLayer {
                        name: "Diagonal modulation".to_owned(),
                        expression: "0.55 + 0.45 * sin((x + y) * scale + t * motion)".to_owned(),
                        ..FormulaLayer::default()
                    }],
                },
                blend_mode: EffectBlendMode::Multiply,
                strength: 0.55,
                scale: 3.0,
                motion: 1.4,
                ..EffectLayer::default()
            },
            EffectLayer {
                name: "Difference shimmer".to_owned(),
                source: FormulaSource {
                    expression: "0.5 + 0.5 * cos((x - y) * scale * 1.7 + u * motion)".to_owned(),
                    gain: 1.0,
                    bias: 0.0,
                    effect_blend_mode: Some(EffectBlendMode::Difference),
                    controls: Vec::new(),
                    layers: vec![FormulaLayer {
                        name: "Cross shimmer".to_owned(),
                        expression: "0.5 + 0.5 * cos((x - y) * scale * 1.7 + u * motion)"
                            .to_owned(),
                        ..FormulaLayer::default()
                    }],
                },
                blend_mode: EffectBlendMode::Difference,
                strength: 0.48,
                scale: 2.25,
                motion: 1.2,
                ..EffectLayer::default()
            },
        ],
        palette: PaletteKind::Aurora,
        custom_gradient: CustomGradient {
            enabled: true,
            colors: vec![
                [0.01, 0.02, 0.08],
                [0.04, 0.16, 0.38],
                [0.00, 0.40, 0.72],
                [0.02, 0.68, 0.64],
                [0.32, 0.88, 0.30],
                [0.86, 0.92, 0.12],
                [1.00, 0.62, 0.06],
                [0.96, 0.18, 0.12],
                [0.82, 0.04, 0.48],
                [0.32, 0.02, 0.52],
            ],
            color_transitions: vec![Vec::new(); 10],
            color_a: [0.01, 0.02, 0.08],
            color_b: [0.04, 0.16, 0.38],
            color_c: [0.00, 0.40, 0.72],
            color_d: [0.02, 0.68, 0.64],
            transition: 0.5,
        },
        color_speed: 0.55,
        color_phase: 0.17,
        brightness: 1.05,
        contrast: 1.25,
        zoom: 1.18,
        rotation_speed: 0.28,
        smoothing: 0.0,
        ..RenderParams::default()
    }
}

fn effect_mode_test_params(mode: Option<EffectBlendMode>) -> RenderParams {
    let mut params = RenderParams {
        patterns: vec![PatternLayer::new(
            "Structured base",
            FormulaSource {
                expression: "0.5 + 0.5 * sin(x * scale * 10 + y * 4)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
        )],
        effects: mode
            .map(|blend_mode| {
                vec![EffectLayer {
                    source: FormulaSource {
                        expression: "0.82".to_owned(),
                        gain: 1.0,
                        bias: 0.0,
                        effect_blend_mode: Some(blend_mode),
                        controls: Vec::new(),
                        layers: Vec::new(),
                    },
                    blend_mode,
                    strength: 1.0,
                    scale: 4.0,
                    motion: 0.0,
                    ..EffectLayer::default()
                }]
            })
            .unwrap_or_default(),
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        brightness: 1.0,
        contrast: 1.0,
        ..RenderParams::default()
    };
    params.patterns[0].scale = 1.6;
    params
}

fn effect_parameter_test_params() -> RenderParams {
    let mut params = RenderParams {
        patterns: vec![PatternLayer::new(
            "Structured base",
            FormulaSource {
                expression: "0.5 + 0.5 * sin(x * scale * 7 + y * 3)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
        )],
        effects: vec![EffectLayer {
            source: FormulaSource {
                expression: "0.5 + 0.5 * sin(x * scale * 5 + y * 2 + t * motion)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: Some(EffectBlendMode::Difference),
                controls: Vec::new(),
                layers: Vec::new(),
            },
            blend_mode: EffectBlendMode::Difference,
            strength: 0.9,
            scale: 2.0,
            motion: 1.0,
            ..EffectLayer::default()
        }],
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        brightness: 1.0,
        contrast: 1.0,
        ..RenderParams::default()
    };
    params.patterns[0].scale = 1.6;
    params
}

fn frame_has_luminance_range(
    frame: &pattern_gif_studio::render::frame_buffer::FrameBuffer,
    minimum_range: u8,
) -> bool {
    let mut min_luma = u8::MAX;
    let mut max_luma = u8::MIN;
    for pixel in frame.pixels.chunks_exact(4) {
        let luma = ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3) as u8;
        min_luma = min_luma.min(luma);
        max_luma = max_luma.max(luma);
    }
    max_luma.saturating_sub(min_luma) >= minimum_range
}

fn frame_pixel_delta(
    a: &pattern_gif_studio::render::frame_buffer::FrameBuffer,
    b: &pattern_gif_studio::render::frame_buffer::FrameBuffer,
) -> u64 {
    a.pixels
        .iter()
        .zip(&b.pixels)
        .map(|(left, right)| left.abs_diff(*right) as u64)
        .sum()
}

fn average_channel_delta(
    a: &pattern_gif_studio::render::frame_buffer::FrameBuffer,
    b: &pattern_gif_studio::render::frame_buffer::FrameBuffer,
) -> f32 {
    frame_pixel_delta(a, b) as f32 / a.pixels.len().max(1) as f32
}

fn frame_has_warm_palette(frame: &pattern_gif_studio::render::frame_buffer::FrameBuffer) -> bool {
    frame.pixels.chunks_exact(4).all(|pixel| {
        let red = pixel[0] as u16;
        let green = pixel[1] as u16;
        let blue = pixel[2] as u16;
        red >= green && green >= blue && red > 32
    })
}

fn high_frequency_smoothing_params() -> RenderParams {
    let mut params = RenderParams {
        patterns: vec![PatternLayer::new(
            "High frequency rings",
            FormulaSource {
                expression: "abs(sin(r * scale * 220 + a * symmetry))".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
        )],
        effects: Vec::new(),
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        zoom: 0.35,
        ..RenderParams::default()
    };
    params.patterns[0].scale = 4.0;
    params
}

fn low_frequency_smoothing_params() -> RenderParams {
    let mut params = RenderParams {
        patterns: vec![PatternLayer::new(
            "Low frequency waves",
            FormulaSource {
                expression: "sin(x * scale * 2.4 + t * motion) * 0.5 + cos(y * scale * 2.1 - u * motion) * 0.5".to_owned(),
                gain: 0.5,
                bias: 0.5,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
        )],
        effects: Vec::new(),
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        zoom: 1.0,
        ..RenderParams::default()
    };
    params.patterns[0].scale = 1.0;
    params.patterns[0].motion = 0.5;
    params
}

fn horizontal_luminance_delta(
    frame: &pattern_gif_studio::render::frame_buffer::FrameBuffer,
) -> u64 {
    let width = frame.width as usize;
    frame
        .pixels
        .chunks_exact(4)
        .collect::<Vec<_>>()
        .chunks(width)
        .map(|row| {
            row.windows(2)
                .map(|pair| luma(pair[0]).abs_diff(luma(pair[1])) as u64)
                .sum::<u64>()
        })
        .sum()
}

fn luma(pixel: &[u8]) -> u8 {
    ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3) as u8
}
