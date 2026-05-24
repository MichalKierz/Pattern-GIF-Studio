use pattern_gif_studio::{
    presets::preset::Preset,
    project::workflow::{load_bundled_workflow_preset, load_workflow_or_preset},
    render::{
        formula::FormulaSource,
        renderer::{EffectBlendMode, EffectLayer, PatternLayer, RenderParams},
    },
    source_assets::asset::{ASSET_SCHEMA_VERSION, AssetType},
};

#[test]
fn bundled_presets_are_valid_json() {
    for path in bundled_workflow_preset_paths() {
        let preset: Preset =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read preset"))
                .expect("bundled preset should load");
        assert_eq!(preset.asset_type, Some(AssetType::WorkflowPreset));
        assert_eq!(preset.schema_version, ASSET_SCHEMA_VERSION);
        preset
            .validate_bundled_asset_type()
            .expect("bundled preset metadata");
        assert!(!preset.name.trim().is_empty());
        assert!(preset.render_params.zoom > 0.0);
        assert!(preset.render_params.detail > 0.0);
        assert!(preset.render_params.brightness > 0.0);
        assert!(preset.render_params.contrast > 0.0);
    }
}

#[test]
fn render_params_do_not_serialize_removed_shape_controls() {
    let json = serde_json::to_value(RenderParams::default()).expect("render params json");
    assert!(json.get("softness").is_none());
    assert!(json.get("loop_seconds").is_none());
    assert_eq!(
        json.get("smoothing").and_then(|value| value.as_f64()),
        Some(0.0)
    );
    assert_eq!(
        json.get("smoothing_radius_pixels")
            .and_then(|value| value.as_f64()),
        Some(1.0)
    );
}

#[test]
fn bundled_workflow_presets_use_layered_universal_patterns() {
    for path in bundled_workflow_preset_paths() {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workflow preset")
            .to_owned();
        let workflow =
            load_bundled_workflow_preset(&path).expect("bundled preset should load as typed asset");

        for (index, pattern) in workflow.project.render_params.patterns.iter().enumerate() {
            assert!(
                pattern.source.layers.len() >= 2,
                "{name} pattern {} should use editable layered sources",
                index + 1
            );
            assert!(
                !pattern
                    .source
                    .layers
                    .iter()
                    .any(|layer| layer.expression.contains("escape_value * 2 + zx * 3")),
                "{name} pattern {} should not use the old radial fractal output kernel",
                index + 1
            );
        }
    }
}

#[test]
fn preset_roundtrip_preserves_render_params() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("round-trip-preset.json");
    let mut params = RenderParams::default();
    params.patterns[0].source = FormulaSource::pattern();
    params.seed = 42;
    params.zoom = 2.25;
    let preset = Preset::new("Round Trip Preset", params.clone());

    std::fs::write(
        &path,
        serde_json::to_string_pretty(&preset).expect("serialize preset"),
    )
    .expect("write preset");
    let loaded: Preset =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read preset"))
            .expect("load preset");

    assert_eq!(loaded.asset_type, Some(AssetType::WorkflowPreset));
    assert_eq!(loaded.schema_version, ASSET_SCHEMA_VERSION);
    assert_eq!(loaded.name, "Round Trip Preset");
    assert_eq!(
        loaded.render_params.patterns[0].source.layers[0].expression,
        FormulaSource::pattern().layers[0].expression
    );
    assert_eq!(loaded.render_params.seed, 42);
    assert!((loaded.render_params.zoom - 2.25).abs() < f32::EPSILON);
}

#[test]
fn workflow_preset_roundtrip_preserves_full_pattern_and_effect_stacks() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("stacked-workflow-preset.json");
    let mut effect_source = FormulaSource::pattern();
    effect_source.effect_blend_mode = Some(EffectBlendMode::Difference);
    let params = RenderParams {
        patterns: vec![
            PatternLayer::new("Pattern A", FormulaSource::fractal_a()),
            PatternLayer::new("Pattern B", FormulaSource::fractal_b()),
        ],
        effects: vec![
            EffectLayer {
                name: "Difference effect".to_owned(),
                source: effect_source,
                blend_mode: EffectBlendMode::Difference,
                strength: 0.75,
                ..EffectLayer::default()
            },
            EffectLayer {
                name: "Screen effect".to_owned(),
                blend_mode: EffectBlendMode::Screen,
                strength: 0.35,
                ..EffectLayer::default()
            },
        ],
        ..RenderParams::default()
    };
    let preset = Preset::new("Stacked Workflow Preset", params);

    std::fs::write(
        &path,
        serde_json::to_string_pretty(&preset).expect("serialize preset"),
    )
    .expect("write preset");
    let loaded: Preset =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read preset"))
            .expect("load preset");

    assert_eq!(loaded.asset_type, Some(AssetType::WorkflowPreset));
    assert_eq!(loaded.schema_version, ASSET_SCHEMA_VERSION);
    assert_eq!(loaded.render_params.patterns.len(), 2);
    assert_eq!(loaded.render_params.effects.len(), 2);
    assert_eq!(
        loaded.render_params.effects[0].source.effect_blend_mode,
        Some(EffectBlendMode::Difference)
    );
    assert_eq!(
        loaded.render_params.effects[1].blend_mode,
        EffectBlendMode::Screen
    );
}

#[test]
fn user_legacy_preset_files_load_through_general_workflow_loader() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let valid_path = temp_dir.path().join("valid-preset.json");
    std::fs::write(
        &valid_path,
        serde_json::json!({
            "name": "User legacy preset",
            "render_params": {
                "patterns": []
            }
        })
        .to_string(),
    )
    .expect("write legacy user preset");

    let workflow = load_workflow_or_preset(&valid_path).expect("load user legacy preset");

    assert_eq!(workflow.project.render_params.patterns.len(), 1);
}

#[test]
fn bundled_workflow_preset_loader_rejects_missing_asset_type() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("missing-type.json");
    std::fs::write(
        &path,
        serde_json::json!({
            "name": "Invalid bundled preset",
            "schema_version": ASSET_SCHEMA_VERSION,
            "render_params": {}
        })
        .to_string(),
    )
    .expect("write invalid bundled preset");

    let error = load_bundled_workflow_preset(&path).expect_err("missing type should fail");

    assert!(
        error_chain_contains(&error, "missing asset_type"),
        "expected missing asset_type in error chain, got: {error:?}"
    );
}

#[test]
fn bundled_workflow_preset_loader_rejects_wrong_asset_type() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let path = temp_dir.path().join("wrong-type.json");
    std::fs::write(
        &path,
        serde_json::json!({
            "asset_type": "color_set",
            "schema_version": ASSET_SCHEMA_VERSION,
            "name": "Invalid bundled preset",
            "render_params": {}
        })
        .to_string(),
    )
    .expect("write invalid bundled preset");

    let error = load_bundled_workflow_preset(&path).expect_err("wrong type should fail");

    assert!(
        error_chain_contains(&error, "wrong asset type"),
        "expected wrong asset type in error chain, got: {error:?}"
    );
}

fn error_chain_contains(error: &anyhow::Error, needle: &str) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains(needle))
}

fn bundled_workflow_preset_paths() -> Vec<std::path::PathBuf> {
    let mut paths = std::fs::read_dir(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("presets")
            .join("workflows"),
    )
    .expect("bundled workflow preset dir")
    .filter_map(|entry| {
        let path = entry.ok()?.path();
        (path.extension().and_then(|ext| ext.to_str()) == Some("json")).then_some(path)
    })
    .collect::<Vec<_>>();
    paths.sort();
    paths
}
