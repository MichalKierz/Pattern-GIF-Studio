use std::path::{Path, PathBuf};

use pattern_gif_studio::{
    animation::loop_time::LoopTime,
    presets::preset::Preset,
    project::workflow::load_bundled_workflow_preset,
    render::{
        color::{CustomGradient, PaletteKind},
        formula::FormulaSource,
        frame_buffer::FrameBuffer,
        gpu_renderer::GpuRenderer,
        renderer::{EffectLayer, PatternLayer, RenderParams, Renderer},
    },
    source_assets::{
        asset::{ASSET_SCHEMA_VERSION, AssetType, CustomColorSet, FormulaSourceAsset},
        storage::load_asset,
    },
};

const AUDIT_SIZE: u32 = 96;
const MIN_LUMA_RANGE: u8 = 28;
const MIN_COLOR_BUCKETS: usize = 10;
const MIN_WORKFLOW_COUNT: usize = 8;
const MIN_WORKFLOW_PAIRWISE_DELTA: f32 = 12.0;
const MIN_SOURCE_PAIRWISE_DELTA: f32 = 5.0;
const MIN_LAYER_IMPACT_DELTA: f32 = 3.0;
const MIN_EFFECT_IMPACT_DELTA: f32 = 5.0;
const MIN_COLOR_TRANSITION_DELTA: f32 = 4.0;
const MIN_COLOR_SET_PAIRWISE_DELTA: f32 = 12.0;
const MIN_WORKFLOW_TEMPORAL_DELTA: f32 = 2.0;
const MIN_LUMA_STDDEV: f32 = 12.0;

#[test]
fn bundled_workflow_presets_are_visual_and_distinct() {
    let mut audits = Vec::new();
    for path in json_files(&workflow_preset_root()) {
        let workflow =
            load_bundled_workflow_preset(&path).expect("load bundled workflow preset for audit");
        let mut params = workflow.project.render_params;
        params.activate_editable_sources();
        let frame = render(&params, LoopTime::from_frame(7, 24));
        let stats = FrameStats::from_frame(&frame);
        let layer_count = params
            .patterns
            .iter()
            .filter(|pattern| pattern.enabled)
            .map(|pattern| {
                pattern
                    .source
                    .layers
                    .iter()
                    .filter(|layer| layer.enabled)
                    .count()
            })
            .sum::<usize>();

        assert!(
            params.patterns.iter().any(|pattern| pattern.enabled),
            "{} must contain at least one enabled pattern",
            path.display()
        );
        assert!(
            layer_count >= 2,
            "{} must use editable formula layers instead of a single filler formula",
            path.display()
        );
        assert!(
            stats.luma_range >= MIN_LUMA_RANGE,
            "{} must not render as a flat or nearly flat workflow preset; luma range {} < {}",
            path.display(),
            stats.luma_range,
            MIN_LUMA_RANGE
        );
        assert!(
            stats.color_buckets >= MIN_COLOR_BUCKETS,
            "{} must produce visible color structure; color buckets {} < {}",
            path.display(),
            stats.color_buckets,
            MIN_COLOR_BUCKETS
        );

        audits.push(RenderAudit {
            name: file_name(&path),
            frame,
        });
    }

    assert!(
        audits.len() >= MIN_WORKFLOW_COUNT,
        "workflow preset library should keep several high-quality starters"
    );
    assert_pairwise_distinct(&audits, MIN_WORKFLOW_PAIRWISE_DELTA, "workflow presets");
}

#[test]
fn bundled_pattern_sources_have_visible_layer_domain_and_distinctness() {
    let mut audits = Vec::new();
    for path in json_files(&preset_root().join("patterns")) {
        let asset: FormulaSourceAsset = load_asset(&path).expect("load pattern source preset");
        let source = asset.source;
        assert_source_is_named_layered_and_valid(&source, &path, "Pattern", 2);

        let params = pattern_source_params(source.clone());
        let full = render(&params, LoopTime::from_frame(8, 30));
        let stats = FrameStats::from_frame(&full);
        assert!(
            stats.luma_range >= MIN_LUMA_RANGE,
            "{} must not render as a flat pattern source; luma range {} < {}",
            path.display(),
            stats.luma_range,
            MIN_LUMA_RANGE
        );

        let first_layer_delta = average_channel_delta(
            &full,
            &render(
                &pattern_source_params(first_enabled_layer_only(source.clone())),
                LoopTime::from_frame(8, 30),
            ),
        );
        assert!(
            first_layer_delta >= MIN_LAYER_IMPACT_DELTA,
            "{} secondary formula layers must visibly affect the pattern; avg delta {first_layer_delta:.2} < {MIN_LAYER_IMPACT_DELTA}",
            path.display()
        );

        if has_domain_pipeline(&source) {
            let no_domain = render(
                &pattern_source_params(disable_domain_pipeline(source.clone())),
                LoopTime::from_frame(8, 30),
            );
            let domain_delta = average_channel_delta(&full, &no_domain);
            assert!(
                domain_delta >= MIN_LAYER_IMPACT_DELTA,
                "{} domain_x/domain_y must visibly affect the pattern; avg delta {domain_delta:.2} < {MIN_LAYER_IMPACT_DELTA}",
                path.display()
            );
        }

        audits.push(RenderAudit {
            name: file_name(&path),
            frame: full,
        });
    }

    assert!(
        audits.len() >= 15,
        "pattern source library should stay expanded"
    );
    assert_pairwise_distinct(&audits, MIN_SOURCE_PAIRWISE_DELTA, "pattern sources");
}

#[test]
fn bundled_effect_sources_have_visible_impact_and_distinctness() {
    let mut audits = Vec::new();
    let base_params = effect_base_params(None, 0.0);
    let baseline = render(&base_params, LoopTime::from_frame(9, 30));

    for path in json_files(&preset_root().join("effects")) {
        let asset: FormulaSourceAsset = load_asset(&path).expect("load effect source preset");
        let source = asset.source;
        assert_source_is_named_layered_and_valid(&source, &path, "Effect", 1);
        let mode = source
            .effect_blend_mode
            .unwrap_or_else(|| panic!("{} must declare effect_blend_mode", path.display()));

        let strong = render(
            &effect_base_params(
                Some(EffectLayer {
                    name: asset.name,
                    source,
                    blend_mode: mode,
                    strength: 0.82,
                    scale: 2.75,
                    motion: 1.15,
                    ..EffectLayer::default()
                }),
                0.0,
            ),
            LoopTime::from_frame(9, 30),
        );
        let strong_delta = average_channel_delta(&baseline, &strong);
        assert!(
            strong_delta >= MIN_EFFECT_IMPACT_DELTA,
            "{} must visibly affect a structured base image; avg delta {strong_delta:.2} < {MIN_EFFECT_IMPACT_DELTA}",
            path.display()
        );

        audits.push(RenderAudit {
            name: file_name(&path),
            frame: strong,
        });
    }

    assert!(
        audits.len() >= 13,
        "effect source library should stay expanded"
    );
    assert_pairwise_distinct(&audits, MIN_SOURCE_PAIRWISE_DELTA, "effect sources");
}

#[test]
fn bundled_color_sets_are_visual_distinct_and_animate_transitions() {
    let mut audits = Vec::new();
    for path in json_files(&preset_root().join("color_sets")) {
        let asset: CustomColorSet = load_asset(&path).expect("load color set preset");
        let mut params = color_test_params();
        asset.apply_to_render_params(&mut params);

        let start = render(&params, LoopTime::from_frame(0, 24));
        let mid = render(&params, LoopTime::from_frame(12, 24));
        let stats = FrameStats::from_frame(&start);
        assert!(
            stats.color_buckets >= MIN_COLOR_BUCKETS,
            "{} must produce visible color variation; color buckets {} < {}",
            path.display(),
            stats.color_buckets,
            MIN_COLOR_BUCKETS
        );

        let transition_delta = average_channel_delta(&start, &mid);
        assert!(
            transition_delta >= MIN_COLOR_TRANSITION_DELTA,
            "{} transitions must visibly animate over time; avg delta {transition_delta:.2} < {MIN_COLOR_TRANSITION_DELTA}",
            path.display()
        );

        audits.push(RenderAudit {
            name: file_name(&path),
            frame: start,
        });
    }

    assert!(audits.len() >= 11, "color set library should stay expanded");
    assert_pairwise_distinct(&audits, MIN_COLOR_SET_PAIRWISE_DELTA, "color sets");
}

#[test]
fn bundled_preset_quality_audit_generates_frames_report_and_passes_gates() {
    let audit_dir = audit_output_dir();
    if audit_dir.exists() {
        std::fs::remove_dir_all(&audit_dir).expect("clear previous preset audit output");
    }
    std::fs::create_dir_all(&audit_dir).expect("create preset audit output");

    let mut report = PresetAuditReport::default();
    audit_workflow_presets(&audit_dir, &mut report);
    audit_pattern_source_presets(&audit_dir, &mut report);
    audit_effect_source_presets(&audit_dir, &mut report);
    audit_color_set_presets(&audit_dir, &mut report);
    report.finish_pairwise_checks();
    report.write_text_report(&audit_dir.join("report.txt"));

    assert!(
        report.workflow_count >= MIN_WORKFLOW_COUNT,
        "workflow preset library must contain at least {MIN_WORKFLOW_COUNT} quality starters, got {}",
        report.workflow_count
    );
    assert!(
        report.pattern_count >= 15,
        "pattern source library should stay expanded"
    );
    assert!(
        report.effect_count >= 13,
        "effect source library should stay expanded"
    );
    assert!(
        report.color_count >= 11,
        "color set library should stay expanded"
    );
    assert!(
        report.too_similar_pairs.is_empty(),
        "preset audit found visually similar pairs: {:?}",
        report.too_similar_pairs
    );
    assert!(
        report.failing_quality_gates.is_empty(),
        "preset audit found failing quality gates: {:?}",
        report.failing_quality_gates
    );
    assert!(
        report.coverage.pattern_stack
            && report.coverage.formula_layers
            && report.coverage.prev
            && report.coverage.domain
            && report.coverage.effects_stack
            && report.coverage.color_gradients
            && report.coverage.transitions
            && report.coverage.loop_animation,
        "preset library feature coverage is incomplete: {:?}",
        report.coverage
    );
}

fn audit_workflow_presets(audit_dir: &Path, report: &mut PresetAuditReport) {
    for path in json_files(&workflow_preset_root()) {
        let preset: Preset =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read workflow preset"))
                .expect("workflow preset JSON");
        assert_eq!(preset.asset_type, Some(AssetType::WorkflowPreset));
        assert_eq!(preset.schema_version, ASSET_SCHEMA_VERSION);
        let workflow =
            load_bundled_workflow_preset(&path).expect("load bundled workflow preset for audit");
        let mut params = workflow.project.render_params;
        params.activate_editable_sources();

        let frames = render_audit_frames(&params);
        write_frames(audit_dir, "workflows", &path, &frames);
        let mut metrics = metrics_for_frames(&frames);
        let features = workflow_features(&params, metrics.temporal_delta);

        if features.domain {
            let no_domain = render_audit_frame(&disable_domains_in_params(params.clone()), 1);
            metrics.domain_delta = Some(average_channel_delta(&frames[1], &no_domain));
        }
        if features.effects {
            let mut no_effects = params.clone();
            no_effects.effects.clear();
            let no_effects_frame = render_audit_frame(&no_effects, 1);
            metrics.effect_delta = Some(average_channel_delta(&frames[1], &no_effects_frame));
        }
        if features.transitions {
            let no_transitions = render_audit_frame(&clear_color_transitions(params.clone()), 2);
            metrics.transition_delta = Some(average_channel_delta(&frames[2], &no_transitions));
        }

        report.add_entry(AuditEntry {
            category: AuditCategory::Workflow,
            file: file_name(&path),
            name: preset.name,
            asset_type: AssetType::WorkflowPreset,
            schema_version: preset.schema_version,
            features,
            metrics,
            action: workflow_action(&path),
        });
        report.workflow_frames.push(RenderSeries {
            name: file_name(&path),
            frames,
        });
    }
}

fn audit_pattern_source_presets(audit_dir: &Path, report: &mut PresetAuditReport) {
    for path in json_files(&preset_root().join("patterns")) {
        let asset: FormulaSourceAsset = load_asset(&path).expect("load pattern source preset");
        assert_eq!(asset.asset_type, Some(AssetType::PatternSource));
        assert_eq!(asset.schema_version, ASSET_SCHEMA_VERSION);
        assert_source_is_named_layered_and_valid(&asset.source, &path, "Pattern", 2);

        let params = pattern_source_params(asset.source.clone());
        let frames = render_audit_frames(&params);
        write_frames(audit_dir, "patterns", &path, &frames);
        let mut metrics = metrics_for_frames(&frames);
        metrics.layer_delta = Some(average_channel_delta(
            &frames[1],
            &render_audit_frame(
                &pattern_source_params(first_enabled_layer_only(asset.source.clone())),
                1,
            ),
        ));
        if has_domain_pipeline(&asset.source) {
            metrics.domain_delta = Some(average_channel_delta(
                &frames[1],
                &render_audit_frame(
                    &pattern_source_params(disable_domain_pipeline(asset.source.clone())),
                    1,
                ),
            ));
        }

        let features = source_features(&asset.source, false, metrics.temporal_delta);
        report.add_entry(AuditEntry {
            category: AuditCategory::PatternSource,
            file: file_name(&path),
            name: asset.name,
            asset_type: AssetType::PatternSource,
            schema_version: asset.schema_version,
            features,
            metrics,
            action: AuditAction::Keep,
        });
        report.pattern_frames.push(RenderSeries {
            name: file_name(&path),
            frames,
        });
    }
}

fn audit_effect_source_presets(audit_dir: &Path, report: &mut PresetAuditReport) {
    let baseline = render_audit_frame(&effect_base_params(None, 0.0), 1);
    for path in json_files(&preset_root().join("effects")) {
        let asset: FormulaSourceAsset = load_asset(&path).expect("load effect source preset");
        assert_eq!(asset.asset_type, Some(AssetType::EffectSource));
        assert_eq!(asset.schema_version, ASSET_SCHEMA_VERSION);
        assert_source_is_named_layered_and_valid(&asset.source, &path, "Effect", 1);
        let mode = asset
            .source
            .effect_blend_mode
            .unwrap_or_else(|| panic!("{} must declare effect_blend_mode", path.display()));
        let params = effect_base_params(
            Some(EffectLayer {
                name: asset.name.clone(),
                source: asset.source.clone(),
                blend_mode: mode,
                strength: 0.82,
                scale: 2.75,
                motion: 1.15,
                ..EffectLayer::default()
            }),
            0.0,
        );

        let frames = render_audit_frames(&params);
        write_frames(audit_dir, "effects", &path, &frames);
        let mut metrics = metrics_for_frames(&frames);
        metrics.effect_delta = Some(average_channel_delta(&baseline, &frames[1]));
        if has_domain_pipeline(&asset.source) {
            let mut no_domain_effect = params.clone();
            no_domain_effect.effects[0].source = disable_domain_pipeline(asset.source.clone());
            metrics.domain_delta = Some(average_channel_delta(
                &frames[1],
                &render_audit_frame(&no_domain_effect, 1),
            ));
        }

        let features = source_features(&asset.source, true, metrics.temporal_delta);
        report.add_entry(AuditEntry {
            category: AuditCategory::EffectSource,
            file: file_name(&path),
            name: asset.name,
            asset_type: AssetType::EffectSource,
            schema_version: asset.schema_version,
            features,
            metrics,
            action: AuditAction::Keep,
        });
        report.effect_frames.push(RenderSeries {
            name: file_name(&path),
            frames,
        });
    }
}

fn audit_color_set_presets(audit_dir: &Path, report: &mut PresetAuditReport) {
    for path in json_files(&preset_root().join("color_sets")) {
        let asset: CustomColorSet = load_asset(&path).expect("load color set preset");
        assert_eq!(asset.asset_type, Some(AssetType::ColorSet));
        assert_eq!(asset.schema_version, ASSET_SCHEMA_VERSION);
        let mut params = color_test_params();
        asset.apply_to_render_params(&mut params);

        let frames = render_audit_frames(&params);
        write_frames(audit_dir, "color_sets", &path, &frames);
        let mut metrics = metrics_for_frames(&frames);
        metrics.transition_delta = Some(average_channel_delta(&frames[0], &frames[2]));

        let features = color_features(&asset, metrics.temporal_delta);
        report.add_entry(AuditEntry {
            category: AuditCategory::ColorSet,
            file: file_name(&path),
            name: asset.name,
            asset_type: AssetType::ColorSet,
            schema_version: asset.schema_version,
            features,
            metrics,
            action: AuditAction::Keep,
        });
        report.color_frames.push(RenderSeries {
            name: file_name(&path),
            frames,
        });
    }
}

fn assert_source_is_named_layered_and_valid(
    source: &FormulaSource,
    path: &Path,
    label: &str,
    minimum_layers: usize,
) {
    assert!(
        source.layers.len() >= minimum_layers,
        "{} must use at least {minimum_layers} formula layer(s)",
        path.display(),
    );
    assert!(
        source
            .layers
            .iter()
            .all(|layer| !layer.name.trim().is_empty()),
        "{} must name formula layers so the preset is editable with intent",
        path.display()
    );
    let issues = source.validate(label);
    assert!(
        issues.is_empty(),
        "{} must validate before quality render audit: {:?}",
        path.display(),
        issues
    );
}

fn pattern_source_params(source: FormulaSource) -> RenderParams {
    RenderParams {
        patterns: vec![PatternLayer {
            name: "Audited pattern".to_owned(),
            source,
            strength: 1.0,
            scale: 1.45,
            motion: 0.85,
            ..PatternLayer::default()
        }],
        effects: Vec::new(),
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        zoom: 1.0,
        contrast: 1.15,
        brightness: 1.0,
        ..RenderParams::default()
    }
}

fn effect_base_params(effect: Option<EffectLayer>, phase_offset: f32) -> RenderParams {
    RenderParams {
        patterns: vec![PatternLayer {
            name: "Structured audit base".to_owned(),
            source: FormulaSource {
                expression: "0.5 + 0.5 * sin(x * scale * 9 + y * 5 + t * motion)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
            strength: 1.0,
            scale: 1.65,
            motion: 0.75,
            ..PatternLayer::default()
        }],
        effects: effect.into_iter().collect(),
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            ..CustomGradient::default()
        },
        color_phase: phase_offset,
        contrast: 1.2,
        ..RenderParams::default()
    }
}

fn color_test_params() -> RenderParams {
    RenderParams {
        patterns: vec![PatternLayer {
            name: "Gradient audit index".to_owned(),
            source: FormulaSource {
                expression: "fract(x * 0.37 + y * 0.29 + 0.5 + sin(r * 6 + t) * 0.12)".to_owned(),
                gain: 1.0,
                bias: 0.0,
                effect_blend_mode: None,
                controls: Vec::new(),
                layers: Vec::new(),
            },
            strength: 1.0,
            scale: 1.0,
            motion: 0.0,
            ..PatternLayer::default()
        }],
        effects: Vec::new(),
        zoom: 1.0,
        color_speed: 1.0,
        color_phase: 0.0,
        brightness: 1.0,
        contrast: 1.0,
        ..RenderParams::default()
    }
}

fn first_enabled_layer_only(mut source: FormulaSource) -> FormulaSource {
    let Some(index) = source.layers.iter().position(|layer| layer.enabled) else {
        return source;
    };
    let layer = source.layers[index].clone();
    source.expression = layer.expression.clone();
    source.layers = vec![layer];
    source
}

fn disable_domain_pipeline(mut source: FormulaSource) -> FormulaSource {
    for layer in &mut source.layers {
        layer.domain_x = "x".to_owned();
        layer.domain_y = "y".to_owned();
        layer.domain_influence = 0.0;
    }
    source
}

fn has_domain_pipeline(source: &FormulaSource) -> bool {
    source.layers.iter().any(|layer| {
        layer.domain_influence.abs() > 0.001
            && (layer.domain_x.trim() != "x" || layer.domain_y.trim() != "y")
    })
}

fn render(params: &RenderParams, time: LoopTime) -> FrameBuffer {
    let mut renderer = GpuRenderer::new().expect("GPU renderer required for preset quality audit");
    renderer.render_frame(params, time, AUDIT_SIZE, AUDIT_SIZE)
}

fn assert_pairwise_distinct(audits: &[RenderAudit], minimum_delta: f32, label: &str) {
    for left_index in 0..audits.len() {
        for right_index in (left_index + 1)..audits.len() {
            let left = &audits[left_index];
            let right = &audits[right_index];
            let delta = average_channel_delta(&left.frame, &right.frame);
            assert!(
                delta >= minimum_delta,
                "{label} '{}' and '{}' are too visually similar; avg delta {delta:.2} < {minimum_delta}",
                left.name,
                right.name
            );
        }
    }
}

fn average_channel_delta(a: &FrameBuffer, b: &FrameBuffer) -> f32 {
    a.pixels
        .iter()
        .zip(&b.pixels)
        .map(|(left, right)| left.abs_diff(*right) as u64)
        .sum::<u64>() as f32
        / a.pixels.len().max(1) as f32
}

struct RenderAudit {
    name: String,
    frame: FrameBuffer,
}

#[derive(Default)]
struct PresetAuditReport {
    entries: Vec<AuditEntry>,
    workflow_frames: Vec<RenderSeries>,
    pattern_frames: Vec<RenderSeries>,
    effect_frames: Vec<RenderSeries>,
    color_frames: Vec<RenderSeries>,
    too_similar_pairs: Vec<String>,
    failing_quality_gates: Vec<String>,
    coverage: FeatureCoverage,
    workflow_count: usize,
    pattern_count: usize,
    effect_count: usize,
    color_count: usize,
}

impl PresetAuditReport {
    fn add_entry(&mut self, entry: AuditEntry) {
        match entry.category {
            AuditCategory::Workflow => self.workflow_count += 1,
            AuditCategory::PatternSource => self.pattern_count += 1,
            AuditCategory::EffectSource => self.effect_count += 1,
            AuditCategory::ColorSet => self.color_count += 1,
        }
        self.coverage.merge(&entry.features);
        self.collect_quality_failures(&entry);
        self.entries.push(entry);
    }

    fn collect_quality_failures(&mut self, entry: &AuditEntry) {
        if entry.metrics.luma_range < MIN_LUMA_RANGE {
            self.failing_quality_gates.push(format!(
                "{} luma range {} < {}",
                entry.file, entry.metrics.luma_range, MIN_LUMA_RANGE
            ));
        }
        if entry.metrics.luma_stddev < MIN_LUMA_STDDEV {
            self.failing_quality_gates.push(format!(
                "{} luma stddev {:.2} < {:.2}",
                entry.file, entry.metrics.luma_stddev, MIN_LUMA_STDDEV
            ));
        }
        if matches!(
            entry.category,
            AuditCategory::Workflow | AuditCategory::ColorSet
        ) && entry.metrics.color_buckets < MIN_COLOR_BUCKETS
        {
            self.failing_quality_gates.push(format!(
                "{} color buckets {} < {}",
                entry.file, entry.metrics.color_buckets, MIN_COLOR_BUCKETS
            ));
        }
        if entry.category == AuditCategory::Workflow
            && entry.metrics.temporal_delta < MIN_WORKFLOW_TEMPORAL_DELTA
        {
            self.failing_quality_gates.push(format!(
                "{} temporal delta {:.2} < {:.2}",
                entry.file, entry.metrics.temporal_delta, MIN_WORKFLOW_TEMPORAL_DELTA
            ));
        }
        if entry.category == AuditCategory::PatternSource
            && entry.metrics.layer_delta.unwrap_or_default() < MIN_LAYER_IMPACT_DELTA
        {
            self.failing_quality_gates.push(format!(
                "{} secondary layer delta {:.2} < {:.2}",
                entry.file,
                entry.metrics.layer_delta.unwrap_or_default(),
                MIN_LAYER_IMPACT_DELTA
            ));
        }
        if entry.features.domain
            && entry.metrics.domain_delta.unwrap_or_default() < MIN_LAYER_IMPACT_DELTA
        {
            self.failing_quality_gates.push(format!(
                "{} domain delta {:.2} < {:.2}",
                entry.file,
                entry.metrics.domain_delta.unwrap_or_default(),
                MIN_LAYER_IMPACT_DELTA
            ));
        }
        if entry.category == AuditCategory::EffectSource
            && entry.metrics.effect_delta.unwrap_or_default() < MIN_EFFECT_IMPACT_DELTA
        {
            self.failing_quality_gates.push(format!(
                "{} effect delta {:.2} < {:.2}",
                entry.file,
                entry.metrics.effect_delta.unwrap_or_default(),
                MIN_EFFECT_IMPACT_DELTA
            ));
        }
        if entry.features.effects
            && entry.category == AuditCategory::Workflow
            && entry.metrics.effect_delta.unwrap_or_default() < MIN_EFFECT_IMPACT_DELTA
        {
            self.failing_quality_gates.push(format!(
                "{} workflow effect delta {:.2} < {:.2}",
                entry.file,
                entry.metrics.effect_delta.unwrap_or_default(),
                MIN_EFFECT_IMPACT_DELTA
            ));
        }
        if entry.features.transitions
            && entry.metrics.transition_delta.unwrap_or_default() < MIN_COLOR_TRANSITION_DELTA
        {
            self.failing_quality_gates.push(format!(
                "{} transition delta {:.2} < {:.2}",
                entry.file,
                entry.metrics.transition_delta.unwrap_or_default(),
                MIN_COLOR_TRANSITION_DELTA
            ));
        }
    }

    fn finish_pairwise_checks(&mut self) {
        self.too_similar_pairs.extend(pairwise_failures(
            "workflow",
            &self.workflow_frames,
            MIN_WORKFLOW_PAIRWISE_DELTA,
        ));
        self.too_similar_pairs.extend(pairwise_failures(
            "pattern_source",
            &self.pattern_frames,
            MIN_SOURCE_PAIRWISE_DELTA,
        ));
        self.too_similar_pairs.extend(pairwise_failures(
            "effect_source",
            &self.effect_frames,
            MIN_SOURCE_PAIRWISE_DELTA,
        ));
        self.too_similar_pairs.extend(pairwise_failures(
            "color_set",
            &self.color_frames,
            MIN_COLOR_SET_PAIRWISE_DELTA,
        ));
    }

    fn write_text_report(&self, path: &Path) {
        let mut text = String::new();
        text.push_str("# Generated Preset Quality Audit\n\n");
        text.push_str("Generated by `cargo test --test preset_quality_tests -- --nocapture`.\n\n");
        text.push_str("## Counts\n\n");
        text.push_str(&format!(
            "- workflow presets: {}\n- pattern sources: {}\n- effect sources: {}\n- color sets: {}\n\n",
            self.workflow_count, self.pattern_count, self.effect_count, self.color_count
        ));
        text.push_str("## Thresholds\n\n");
        text.push_str(&format!(
            "- luma range >= {MIN_LUMA_RANGE}\n- luma stddev >= {MIN_LUMA_STDDEV:.1}\n- color buckets >= {MIN_COLOR_BUCKETS}\n- workflow temporal delta >= {MIN_WORKFLOW_TEMPORAL_DELTA:.1}\n- workflow pairwise delta >= {MIN_WORKFLOW_PAIRWISE_DELTA:.1}\n- source pairwise delta >= {MIN_SOURCE_PAIRWISE_DELTA:.1}\n- color set pairwise delta >= {MIN_COLOR_SET_PAIRWISE_DELTA:.1}\n- layer/domain delta >= {MIN_LAYER_IMPACT_DELTA:.1}\n- effect delta >= {MIN_EFFECT_IMPACT_DELTA:.1}\n- color transition delta >= {MIN_COLOR_TRANSITION_DELTA:.1}\n\n"
        ));
        text.push_str("## Feature Coverage\n\n");
        text.push_str(&format!(
            "| Pattern stack | Formula layers | Prev | Domain | Effects stack | Custom gradients | Transitions | Loop animation |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n| {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
            yes_no(self.coverage.pattern_stack),
            yes_no(self.coverage.formula_layers),
            yes_no(self.coverage.prev),
            yes_no(self.coverage.domain),
            yes_no(self.coverage.effects_stack),
            yes_no(self.coverage.color_gradients),
            yes_no(self.coverage.transitions),
            yes_no(self.coverage.loop_animation),
        ));
        text.push_str("## Inventory And Metrics\n\n");
        text.push_str("| Action | Type | Asset type | File | Name | Schema | Features | Luma range | Luma stddev | Buckets | Temporal | Layer | Domain | Effect | Transition |\n");
        text.push_str("| --- | --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
        for entry in &self.entries {
            text.push_str(&format!(
                "| {} | {} | {} | `{}` | {} | {} | {} | {} | {:.2} | {} | {:.2} | {} | {} | {} | {} |\n",
                entry.action.label(),
                entry.category.label(),
                entry.asset_type.label(),
                entry.file,
                entry.name,
                entry.schema_version,
                entry.features.labels().join(", "),
                entry.metrics.luma_range,
                entry.metrics.luma_stddev,
                entry.metrics.color_buckets,
                entry.metrics.temporal_delta,
                optional_metric(entry.metrics.layer_delta),
                optional_metric(entry.metrics.domain_delta),
                optional_metric(entry.metrics.effect_delta),
                optional_metric(entry.metrics.transition_delta),
            ));
        }
        text.push_str("\n## Pairwise Similarity Failures\n\n");
        if self.too_similar_pairs.is_empty() {
            text.push_str("None.\n\n");
        } else {
            for pair in &self.too_similar_pairs {
                text.push_str(&format!("- {pair}\n"));
            }
            text.push('\n');
        }
        text.push_str("## Quality Gate Failures\n\n");
        if self.failing_quality_gates.is_empty() {
            text.push_str("None.\n\n");
        } else {
            for failure in &self.failing_quality_gates {
                text.push_str(&format!("- {failure}\n"));
            }
            text.push('\n');
        }
        std::fs::write(path, text).expect("write generated preset audit report");
    }
}

#[derive(Debug, Clone)]
struct AuditEntry {
    category: AuditCategory,
    file: String,
    name: String,
    asset_type: AssetType,
    schema_version: u32,
    features: PresetFeatures,
    metrics: PresetMetrics,
    action: AuditAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuditCategory {
    Workflow,
    PatternSource,
    EffectSource,
    ColorSet,
}

impl AuditCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Workflow => "workflow_preset",
            Self::PatternSource => "pattern_source",
            Self::EffectSource => "effect_source",
            Self::ColorSet => "color_set",
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AuditAction {
    Keep,
    Add,
}

impl AuditAction {
    fn label(self) -> &'static str {
        match self {
            Self::Keep => "KEEP",
            Self::Add => "ADD",
        }
    }
}

#[derive(Debug, Default, Clone)]
struct PresetFeatures {
    pattern_stack: bool,
    formula_layers: bool,
    prev: bool,
    domain: bool,
    effects: bool,
    effects_stack: bool,
    custom_gradient: bool,
    transitions: bool,
    animated: bool,
}

impl PresetFeatures {
    fn labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.pattern_stack {
            labels.push("pattern stack");
        }
        if self.formula_layers {
            labels.push("formula layers");
        }
        if self.prev {
            labels.push("prev");
        }
        if self.domain {
            labels.push("domain");
        }
        if self.effects {
            labels.push("effects");
        }
        if self.effects_stack {
            labels.push("effects stack");
        }
        if self.custom_gradient {
            labels.push("custom gradient");
        }
        if self.transitions {
            labels.push("transitions");
        }
        if self.animated {
            labels.push("animated");
        }
        labels
    }
}

#[derive(Debug, Default)]
struct FeatureCoverage {
    pattern_stack: bool,
    formula_layers: bool,
    prev: bool,
    domain: bool,
    effects_stack: bool,
    color_gradients: bool,
    transitions: bool,
    loop_animation: bool,
}

impl FeatureCoverage {
    fn merge(&mut self, features: &PresetFeatures) {
        self.pattern_stack |= features.pattern_stack;
        self.formula_layers |= features.formula_layers;
        self.prev |= features.prev;
        self.domain |= features.domain;
        self.effects_stack |= features.effects_stack;
        self.color_gradients |= features.custom_gradient;
        self.transitions |= features.transitions;
        self.loop_animation |= features.animated;
    }
}

#[derive(Debug, Clone)]
struct PresetMetrics {
    luma_range: u8,
    luma_stddev: f32,
    color_buckets: usize,
    temporal_delta: f32,
    layer_delta: Option<f32>,
    domain_delta: Option<f32>,
    effect_delta: Option<f32>,
    transition_delta: Option<f32>,
}

#[derive(Clone)]
struct RenderSeries {
    name: String,
    frames: Vec<FrameBuffer>,
}

struct FrameStats {
    luma_range: u8,
    luma_stddev: f32,
    color_buckets: usize,
}

impl FrameStats {
    fn from_frame(frame: &FrameBuffer) -> Self {
        let mut min_luma = u8::MAX;
        let mut max_luma = u8::MIN;
        let mut lumas = Vec::with_capacity((frame.width * frame.height) as usize);
        let mut buckets = std::collections::BTreeSet::new();
        for pixel in frame.pixels.chunks_exact(4) {
            let luma = ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3) as u8;
            min_luma = min_luma.min(luma);
            max_luma = max_luma.max(luma);
            lumas.push(luma as f32);
            buckets.insert((pixel[0] / 32, pixel[1] / 32, pixel[2] / 32));
        }
        let mean = lumas.iter().sum::<f32>() / lumas.len().max(1) as f32;
        let variance = lumas
            .iter()
            .map(|luma| {
                let delta = *luma - mean;
                delta * delta
            })
            .sum::<f32>()
            / lumas.len().max(1) as f32;
        Self {
            luma_range: max_luma.saturating_sub(min_luma),
            luma_stddev: variance.sqrt(),
            color_buckets: buckets.len(),
        }
    }
}

fn metrics_for_frames(frames: &[FrameBuffer]) -> PresetMetrics {
    let stats = FrameStats::from_frame(&frames[0]);
    PresetMetrics {
        luma_range: stats.luma_range,
        luma_stddev: stats.luma_stddev,
        color_buckets: stats.color_buckets,
        temporal_delta: temporal_delta(frames),
        layer_delta: None,
        domain_delta: None,
        effect_delta: None,
        transition_delta: None,
    }
}

fn temporal_delta(frames: &[FrameBuffer]) -> f32 {
    if frames.len() < 2 {
        return 0.0;
    }
    frames
        .windows(2)
        .map(|pair| average_channel_delta(&pair[0], &pair[1]))
        .sum::<f32>()
        / (frames.len() - 1) as f32
}

fn render_audit_frames(params: &RenderParams) -> Vec<FrameBuffer> {
    (0..4)
        .map(|frame_index| render_audit_frame(params, frame_index))
        .collect()
}

fn render_audit_frame(params: &RenderParams, frame_index: u32) -> FrameBuffer {
    render(params, LoopTime::from_frame(frame_index, 4))
}

fn workflow_features(params: &RenderParams, temporal_delta: f32) -> PresetFeatures {
    let active_patterns = params
        .patterns
        .iter()
        .filter(|pattern| pattern.enabled)
        .count();
    let active_effects = params
        .effects
        .iter()
        .filter(|effect| effect.enabled)
        .count();
    let sources = params
        .patterns
        .iter()
        .map(|pattern| &pattern.source)
        .chain(params.effects.iter().map(|effect| &effect.source))
        .collect::<Vec<_>>();
    PresetFeatures {
        pattern_stack: active_patterns > 1,
        formula_layers: sources.iter().any(|source| source.layers.len() > 1),
        prev: sources.iter().any(|source| source_uses_prev(source)),
        domain: sources.iter().any(|source| has_domain_pipeline(source)),
        effects: active_effects > 0,
        effects_stack: active_effects > 1,
        custom_gradient: params.custom_gradient.enabled && params.custom_gradient.colors.len() > 2,
        transitions: gradient_has_transitions(&params.custom_gradient),
        animated: temporal_delta >= MIN_WORKFLOW_TEMPORAL_DELTA,
    }
}

fn source_features(source: &FormulaSource, is_effect: bool, temporal_delta: f32) -> PresetFeatures {
    PresetFeatures {
        formula_layers: source.layers.len() > 1,
        prev: source_uses_prev(source),
        domain: has_domain_pipeline(source),
        effects: is_effect,
        animated: temporal_delta >= 1.0,
        ..PresetFeatures::default()
    }
}

fn color_features(asset: &CustomColorSet, temporal_delta: f32) -> PresetFeatures {
    PresetFeatures {
        custom_gradient: asset.custom_gradient.enabled && asset.custom_gradient.colors.len() > 2,
        transitions: gradient_has_transitions(&asset.custom_gradient),
        animated: temporal_delta >= MIN_COLOR_TRANSITION_DELTA,
        ..PresetFeatures::default()
    }
}

fn source_uses_prev(source: &FormulaSource) -> bool {
    source.expression.contains("prev")
        || source.layers.iter().any(|layer| {
            layer.expression.contains("prev")
                || layer.domain_x.contains("prev")
                || layer.domain_y.contains("prev")
        })
}

fn gradient_has_transitions(gradient: &CustomGradient) -> bool {
    gradient
        .color_transitions
        .iter()
        .any(|transitions| !transitions.is_empty())
}

fn disable_domains_in_params(mut params: RenderParams) -> RenderParams {
    for pattern in &mut params.patterns {
        pattern.source = disable_domain_pipeline(pattern.source.clone());
    }
    for effect in &mut params.effects {
        effect.source = disable_domain_pipeline(effect.source.clone());
    }
    params
}

fn clear_color_transitions(mut params: RenderParams) -> RenderParams {
    for transitions in &mut params.custom_gradient.color_transitions {
        transitions.clear();
    }
    params
}

fn write_frames(audit_dir: &Path, category: &str, source_path: &Path, frames: &[FrameBuffer]) {
    let dir = audit_dir.join(category);
    std::fs::create_dir_all(&dir).expect("create preset audit frame directory");
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("preset");
    for (index, frame) in frames.iter().enumerate() {
        write_ppm(&dir.join(format!("{stem}_t{index}.ppm")), frame);
    }
}

fn write_ppm(path: &Path, frame: &FrameBuffer) {
    let mut bytes = format!("P6\n{} {}\n255\n", frame.width, frame.height).into_bytes();
    for pixel in frame.pixels.chunks_exact(4) {
        bytes.extend_from_slice(&pixel[..3]);
    }
    std::fs::write(path, bytes).expect("write preset audit ppm frame");
}

fn pairwise_failures(label: &str, series: &[RenderSeries], minimum_delta: f32) -> Vec<String> {
    let mut failures = Vec::new();
    for left_index in 0..series.len() {
        for right_index in (left_index + 1)..series.len() {
            let left = &series[left_index];
            let right = &series[right_index];
            let delta = average_series_delta(&left.frames, &right.frames);
            if delta < minimum_delta {
                failures.push(format!(
                    "{label}: {} vs {} avg delta {delta:.2} < {minimum_delta}",
                    left.name, right.name
                ));
            }
        }
    }
    failures
}

fn average_series_delta(left: &[FrameBuffer], right: &[FrameBuffer]) -> f32 {
    let len = left.len().min(right.len()).max(1);
    left.iter()
        .zip(right)
        .map(|(left, right)| average_channel_delta(left, right))
        .sum::<f32>()
        / len as f32
}

fn workflow_action(path: &Path) -> AuditAction {
    match path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
    {
        "circuit-glitch-bloom.json"
        | "deep-water-glass.json"
        | "ghastly-mandelbrot.json"
        | "woven-topographic-gold.json" => AuditAction::Add,
        _ => AuditAction::Keep,
    }
}

fn optional_metric(metric: Option<f32>) -> String {
    metric
        .map(|value| format!("{value:.2}"))
        .unwrap_or_else(|| "-".to_owned())
}

fn yes_no(value: bool) -> &'static str {
    if value { "PASS" } else { "FAIL" }
}

fn preset_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("presets")
}

fn workflow_preset_root() -> PathBuf {
    preset_root().join("workflows")
}

fn audit_output_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("preset_audit")
}

fn json_files(path: &Path) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(path)
        .unwrap_or_else(|error| panic!("read preset directory {}: {error}", path.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            (path.extension().and_then(|ext| ext.to_str()) == Some("json")).then_some(path)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("preset")
        .to_owned()
}
