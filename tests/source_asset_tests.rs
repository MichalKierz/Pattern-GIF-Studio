use pattern_gif_studio::{
    render::{
        color::{ColorTransition, CustomGradient, PaletteKind},
        formula::{FormulaBlendMode, FormulaControl, FormulaLayer, FormulaSource},
        renderer::{EffectBlendMode, RenderParams},
    },
    source_assets::{
        asset::{ASSET_SCHEMA_VERSION, AssetType, CustomColorSet, FormulaSourceAsset},
        storage::{load_asset, save_asset},
    },
};

#[test]
fn custom_color_set_roundtrip_applies_color_fields() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut params = RenderParams {
        palette: PaletteKind::Aurora,
        color_speed: 2.0,
        color_phase: 0.35,
        brightness: 1.4,
        contrast: 1.7,
        ..RenderParams::default()
    };
    params.custom_gradient.enabled = true;
    params.custom_gradient = ten_color_gradient();
    params.custom_gradient.ensure_color_stops();

    let asset = CustomColorSet::from_render_params("Color One", &params);
    let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save color set");

    let mut target = RenderParams::default();
    let loaded: CustomColorSet = load_asset(&path).expect("load color set");
    loaded.apply_to_render_params(&mut target);

    assert_eq!(loaded.asset_type, Some(AssetType::ColorSet));
    assert_eq!(loaded.schema_version, ASSET_SCHEMA_VERSION);
    assert_eq!(target.palette, PaletteKind::Aurora);
    assert!(target.custom_gradient.enabled);
    assert_eq!(target.custom_gradient.stops().len(), 10);
    assert_eq!(target.custom_gradient.stops()[0], [0.01, 0.02, 0.03]);
    assert_eq!(
        target.custom_gradient.color_transitions[8][1].color,
        [0.91, 0.21, 0.31]
    );
    assert_eq!(target.color_speed, 2.0);
    assert_eq!(target.color_phase, 0.35);
    assert_eq!(target.brightness, 1.4);
    assert_eq!(target.contrast, 1.7);
}

#[test]
fn custom_color_set_normalizes_legacy_disabled_gradient_to_palette_seed() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let params = RenderParams {
        palette: PaletteKind::MonoChrome,
        custom_gradient: CustomGradient {
            enabled: false,
            colors: vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            ..CustomGradient::default()
        },
        ..RenderParams::default()
    };

    let asset = CustomColorSet::from_render_params("Legacy Mono", &params);
    let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save color set");
    let loaded: CustomColorSet = load_asset(&path).expect("load color set");
    let mut target = RenderParams::default();
    loaded.apply_to_render_params(&mut target);

    assert_eq!(loaded.asset_type, Some(AssetType::ColorSet));
    assert_eq!(loaded.schema_version, ASSET_SCHEMA_VERSION);
    assert!(target.custom_gradient.enabled);
    assert_eq!(target.custom_gradient.stops()[0], [0.0, 0.0, 0.0]);
    assert_eq!(target.custom_gradient.stops()[3], [1.0, 1.0, 1.0]);
}

#[test]
fn formula_source_asset_roundtrip_preserves_layers() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let source = FormulaSource {
        expression: "sin(x * 4 + t)".to_owned(),
        gain: 0.5,
        bias: 0.5,
        effect_blend_mode: None,
        controls: vec![
            FormulaControl {
                name: "P1 density".to_owned(),
                value: 1.25,
            },
            FormulaControl {
                name: "P2 warp".to_owned(),
                value: -0.5,
            },
            FormulaControl {
                name: "P3 phase".to_owned(),
                value: 2.75,
            },
            FormulaControl {
                name: "P4 softness".to_owned(),
                value: 0.33,
            },
        ],
        layers: vec![
            FormulaLayer {
                name: "Editable source layer".to_owned(),
                expression: "sin(x * p1 + y * p2 + t)".to_owned(),
                gain: 0.73,
                bias: -0.18,
                opacity: 0.8,
                repeat_x: 2.5,
                repeat_y: 0.75,
                warp_x: 1.2,
                warp_y: -0.6,
                offset_x: 0.33,
                offset_y: -0.27,
                motion_x: 1.4,
                motion_y: -1.1,
                domain_x: "x + sin(y * p3) * p4".to_owned(),
                domain_y: "y + cos(x * p2) * p1".to_owned(),
                domain_influence: 0.72,
                ..Default::default()
            },
            FormulaLayer {
                name: "Prev detail".to_owned(),
                expression: "prev * 0.4 + cos(r * p3)".to_owned(),
                blend_mode: FormulaBlendMode::Difference,
                opacity: 0.35,
                ..Default::default()
            },
        ],
    };
    let asset = FormulaSourceAsset::new_pattern("Source One", source);
    let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save source");

    let loaded: FormulaSourceAsset = load_asset(&path).expect("load source");
    loaded
        .validate_for_type(AssetType::PatternSource)
        .expect("canonical pattern source should validate");
    let json = std::fs::read_to_string(&path).expect("read saved source JSON");

    assert_eq!(loaded.asset_type, Some(AssetType::PatternSource));
    assert_eq!(loaded.schema_version, ASSET_SCHEMA_VERSION);
    assert_eq!(loaded.name, "Source One");
    assert_eq!(loaded.source.effect_blend_mode, None);
    assert!(
        !json.contains("\"patterns\""),
        "pattern source asset should not serialize a pattern set wrapper"
    );
    assert!(
        !json.contains("\"effects\""),
        "pattern source asset should not serialize an effect set wrapper"
    );
    assert_eq!(loaded.source.controls.len(), 4);
    assert_eq!(loaded.source.controls[1].value, -0.5);
    assert_eq!(loaded.source.layers.len(), 2);
    assert_eq!(loaded.source.layers[0].name, "Editable source layer");
    assert_eq!(
        loaded.source.layers[0].expression,
        "sin(x * p1 + y * p2 + t)"
    );
    assert_eq!(loaded.source.gain, 0.5);
    assert_eq!(loaded.source.bias, 0.5);
    assert_eq!(loaded.source.layers[0].gain, 0.73);
    assert_eq!(loaded.source.layers[0].bias, -0.18);
    assert_eq!(loaded.source.layers[0].opacity, 0.8);
    assert_eq!(loaded.source.layers[0].repeat_x, 2.5);
    assert_eq!(loaded.source.layers[0].repeat_y, 0.75);
    assert_eq!(loaded.source.layers[0].warp_x, 1.2);
    assert_eq!(loaded.source.layers[0].warp_y, -0.6);
    assert_eq!(loaded.source.layers[0].offset_x, 0.33);
    assert_eq!(loaded.source.layers[0].offset_y, -0.27);
    assert_eq!(loaded.source.layers[0].motion_x, 1.4);
    assert_eq!(loaded.source.layers[0].motion_y, -1.1);
    assert_eq!(loaded.source.layers[0].domain_x, "x + sin(y * p3) * p4");
    assert_eq!(loaded.source.layers[0].domain_y, "y + cos(x * p2) * p1");
    assert_eq!(loaded.source.layers[0].domain_influence, 0.72);
    assert_eq!(
        loaded.source.layers[1].blend_mode,
        FormulaBlendMode::Difference
    );
    assert!(loaded.source.layers[1].expression.contains("prev"));
}

#[test]
fn typed_pattern_source_rejects_effect_blend_mode() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let source = FormulaSource {
        effect_blend_mode: Some(EffectBlendMode::Contrast),
        ..FormulaSource::pattern()
    };
    let asset =
        FormulaSourceAsset::new_with_type("Wrong pattern source", source, AssetType::PatternSource);
    let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save source");
    let loaded: FormulaSourceAsset = load_asset(&path).expect("load source");

    let error = loaded
        .validate_for_type(AssetType::PatternSource)
        .expect_err("pattern source must not carry an effect blend mode");

    assert!(error.to_string().contains("effect_blend_mode"));
}

#[test]
fn typed_effect_source_requires_effect_blend_mode() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let source = FormulaSource {
        effect_blend_mode: None,
        ..FormulaSource::pattern()
    };
    let asset = FormulaSourceAsset::new_effect("Wrong effect source", source);
    let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save source");
    let loaded: FormulaSourceAsset = load_asset(&path).expect("load source");

    let error = loaded
        .validate_for_type(AssetType::EffectSource)
        .expect_err("effect source must declare an effect blend mode");

    assert!(error.to_string().contains("effect_blend_mode"));
}

#[test]
fn removed_source_set_asset_types_are_rejected_as_noncanonical() {
    for removed_asset_type in ["pattern_set", "effect_set"] {
        let json = serde_json::json!({
            "asset_type": removed_asset_type,
            "schema_version": ASSET_SCHEMA_VERSION,
            "name": "Old source set",
            "source": {
                "expression": "sin(x * scale)",
                "layers": [{ "expression": "sin(x * scale)" }]
            }
        })
        .to_string();

        let error = serde_json::from_str::<FormulaSourceAsset>(&json)
            .expect_err("removed set asset type must not deserialize as source asset");

        assert!(
            error.to_string().contains(removed_asset_type),
            "error should name removed asset type {removed_asset_type}: {error}"
        );
    }
}

#[test]
fn effect_source_asset_roundtrip_preserves_each_blend_mode() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    for mode in EffectBlendMode::ALL {
        let source = FormulaSource {
            expression: "0.5 + 0.5 * sin(x * scale + t * motion)".to_owned(),
            gain: 1.25,
            bias: -0.15,
            effect_blend_mode: Some(mode),
            controls: Vec::new(),
            layers: vec![FormulaLayer {
                name: format!("{} source layer", mode.label()),
                expression: "0.5 + 0.5 * sin(x * scale + y + t * motion)".to_owned(),
                opacity: 0.75,
                ..FormulaLayer::default()
            }],
        };
        let asset = FormulaSourceAsset::new_effect(format!("{} Source", mode.label()), source);
        let path = save_asset(temp_dir.path(), &asset.name, &asset).expect("save source");

        let loaded: FormulaSourceAsset = load_asset(&path).expect("load source");
        loaded
            .validate_for_type(AssetType::EffectSource)
            .expect("canonical effect source should validate");

        assert_eq!(loaded.asset_type, Some(AssetType::EffectSource));
        assert_eq!(loaded.schema_version, ASSET_SCHEMA_VERSION);
        assert_eq!(loaded.source.effect_blend_mode, Some(mode));
        assert_eq!(loaded.source.gain, 1.25);
        assert_eq!(loaded.source.bias, -0.15);
        assert_eq!(loaded.source.layers[0].opacity, 0.75);
        assert!(loaded.source.layers[0].expression.contains("motion"));
    }
}

#[test]
fn bundled_pattern_and_effect_sources_are_valid_and_separate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("presets");
    let pattern_count = assert_formula_sources_are_valid(&root.join("patterns"), "Pattern");
    let effect_count = assert_formula_sources_are_valid(&root.join("effects"), "Effect");

    assert!(
        pattern_count >= 15,
        "expected expanded pattern source library, got {pattern_count}"
    );
    assert!(
        effect_count >= 13,
        "expected expanded effect source library, got {effect_count}"
    );
}

#[test]
fn bundled_color_sets_are_valid_and_expanded() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("presets")
        .join("color_sets");
    let mut count = 0;
    let mut unique_color_sets = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&path).expect("color set preset directory should exist") {
        let entry = entry.expect("color set dir entry should read");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let asset: CustomColorSet = load_asset(&path).expect("color set should load");
        assert_eq!(
            asset.asset_type,
            Some(AssetType::ColorSet),
            "color set preset must declare asset_type: {}",
            path.display()
        );
        assert_eq!(
            asset.schema_version,
            ASSET_SCHEMA_VERSION,
            "color set preset must declare current schema_version: {}",
            path.display()
        );
        assert!(
            !asset.name.trim().is_empty(),
            "color set name should not be empty: {}",
            path.display()
        );
        assert!(
            asset.custom_gradient.enabled,
            "bundled color set should load as an active gradient: {}",
            path.display()
        );
        assert!(
            asset.custom_gradient.colors.len() >= 4,
            "bundled color set must define explicit colors instead of relying on legacy fields: {}",
            path.display()
        );
        assert_eq!(
            asset.custom_gradient.color_transitions.len(),
            asset.custom_gradient.colors.len(),
            "bundled color set must define transition groups for each visible color: {}",
            path.display()
        );
        assert!(
            asset
                .custom_gradient
                .color_transitions
                .iter()
                .all(|transitions| !transitions.is_empty()),
            "each bundled color must include at least one transition: {}",
            path.display()
        );
        assert!(
            (0.0..=1.0).contains(&asset.custom_gradient.transition),
            "color set transition should be normalized: {}",
            path.display()
        );
        let gradient = asset.custom_gradient.clone();
        assert!(
            gradient.stops().len() >= 4,
            "color set should expose at least A/B/C/D stops: {}",
            path.display()
        );
        for color in gradient.stops() {
            for channel in color {
                assert!(
                    (0.0..=1.0).contains(channel),
                    "color channel should be normalized: {}",
                    path.display()
                );
            }
        }
        unique_color_sets.insert(color_signature(gradient.stops()));

        let mut target = RenderParams::default();
        asset.apply_to_render_params(&mut target);
        assert_eq!(
            target.custom_gradient.stops(),
            gradient.stops(),
            "Load Colors must update the visible A/B/C/D color stops: {}",
            path.display()
        );
        count += 1;
    }

    assert!(
        count >= 11,
        "expected expanded color set library, got {count}"
    );
    assert_eq!(
        unique_color_sets.len(),
        count,
        "bundled color sets should not collapse to the same A/B/C/D colors"
    );
}

fn color_signature(stops: &[[f32; 3]]) -> String {
    stops
        .iter()
        .map(|color| {
            format!(
                "{:.3},{:.3},{:.3}",
                color[0].clamp(0.0, 1.0),
                color[1].clamp(0.0, 1.0),
                color[2].clamp(0.0, 1.0)
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn ten_color_gradient() -> CustomGradient {
    CustomGradient {
        enabled: true,
        colors: vec![
            [0.01, 0.02, 0.03],
            [0.11, 0.12, 0.13],
            [0.21, 0.22, 0.23],
            [0.31, 0.32, 0.33],
            [0.41, 0.42, 0.43],
            [0.51, 0.52, 0.53],
            [0.61, 0.62, 0.63],
            [0.71, 0.72, 0.73],
            [0.81, 0.82, 0.83],
            [0.91, 0.92, 0.93],
        ],
        color_transitions: vec![
            vec![ColorTransition {
                color: [0.02, 0.12, 0.22],
            }],
            vec![ColorTransition {
                color: [0.12, 0.22, 0.32],
            }],
            vec![ColorTransition {
                color: [0.22, 0.32, 0.42],
            }],
            vec![ColorTransition {
                color: [0.32, 0.42, 0.52],
            }],
            vec![ColorTransition {
                color: [0.42, 0.52, 0.62],
            }],
            vec![ColorTransition {
                color: [0.52, 0.62, 0.72],
            }],
            vec![ColorTransition {
                color: [0.62, 0.72, 0.82],
            }],
            vec![ColorTransition {
                color: [0.72, 0.82, 0.92],
            }],
            vec![
                ColorTransition {
                    color: [0.82, 0.12, 0.22],
                },
                ColorTransition {
                    color: [0.91, 0.21, 0.31],
                },
            ],
            vec![ColorTransition {
                color: [0.92, 0.02, 0.12],
            }],
        ],
        color_a: [0.01, 0.02, 0.03],
        color_b: [0.11, 0.12, 0.13],
        color_c: [0.21, 0.22, 0.23],
        color_d: [0.31, 0.32, 0.33],
        transition: 0.67,
    }
}

fn assert_formula_sources_are_valid(path: &std::path::Path, label: &str) -> usize {
    let mut count = 0;
    for entry in std::fs::read_dir(path).expect("preset source directory should exist") {
        let entry = entry.expect("preset source dir entry should read");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let json = std::fs::read_to_string(&path).expect("read source preset JSON");
        assert!(
            !json.contains("\"pattern_set\"") && !json.contains("\"effect_set\""),
            "bundled source assets must not use legacy set wrapper asset types: {}",
            path.display()
        );
        assert!(
            !json.contains("\"patterns\"") && !json.contains("\"effects\""),
            "bundled source assets must serialize one source, not a source set: {}",
            path.display()
        );
        let asset: FormulaSourceAsset =
            load_asset(&path).expect("formula source asset should load");
        let expected_type = match label {
            "Pattern" => AssetType::PatternSource,
            "Effect" => AssetType::EffectSource,
            _ => panic!("unexpected source label {label}"),
        };
        assert_eq!(
            asset.asset_type,
            Some(expected_type),
            "source asset must declare correct asset_type: {}",
            path.display()
        );
        assert_eq!(
            asset.schema_version,
            ASSET_SCHEMA_VERSION,
            "source asset must declare current schema_version: {}",
            path.display()
        );
        asset
            .validate_for_type(expected_type)
            .unwrap_or_else(|error| {
                panic!(
                    "source asset must match canonical source shape: {} / {}",
                    path.display(),
                    error
                )
            });
        assert!(
            !asset.name.trim().is_empty(),
            "source asset name should not be empty: {}",
            path.display()
        );
        let issues = asset.source.validate(label);
        assert!(
            issues.is_empty(),
            "source asset should validate without formula issues: {} / {:?}",
            path.display(),
            issues
        );
        count += 1;
    }
    count
}
