use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::{presets::preset::Preset, project::project_state::ProjectState};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WorkflowFile {
    pub project: ProjectState,
}

impl WorkflowFile {
    pub fn from_project(project: ProjectState) -> Self {
        Self { project }
    }
}

pub fn save_workflow(path: &Path, workflow: &WorkflowFile) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(workflow).context("failed to serialize workflow")?;
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path.to_path_buf())
}

pub fn load_workflow_or_preset(path: &Path) -> Result<WorkflowFile> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid json {}", path.display()))?;

    if value.get("project").is_some() {
        let mut workflow: WorkflowFile = serde_json::from_value(value)
            .with_context(|| format!("invalid workflow {}", path.display()))?;
        workflow.project.sanitize();
        return Ok(workflow);
    }

    let preset: Preset = serde_json::from_value(value)
        .with_context(|| format!("invalid workflow or preset {}", path.display()))?;
    preset
        .validate_asset_type()
        .with_context(|| format!("invalid workflow preset {}", path.display()))?;
    Ok(workflow_from_preset(preset))
}

pub fn load_bundled_workflow_preset(path: &Path) -> Result<WorkflowFile> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("invalid json {}", path.display()))?;

    if value.get("project").is_some() {
        bail!(
            "bundled preset {} must use workflow_preset asset format, not raw workflow format",
            path.display()
        );
    }

    let preset: Preset = serde_json::from_value(value)
        .with_context(|| format!("invalid bundled workflow preset {}", path.display()))?;
    preset
        .validate_bundled_asset_type()
        .with_context(|| format!("invalid bundled workflow preset {}", path.display()))?;
    Ok(workflow_from_preset(preset))
}

fn workflow_from_preset(preset: Preset) -> WorkflowFile {
    let mut project = ProjectState {
        render_params: preset.render_params,
        ..ProjectState::default()
    };
    project.sanitize();
    WorkflowFile::from_project(project)
}

#[cfg(test)]
mod tests {
    use super::{WorkflowFile, load_workflow_or_preset, save_workflow};
    use crate::{
        export::export_settings::ExportSettings,
        project::project_state::ProjectState,
        render::{
            color::{ColorTransition, CustomGradient, PaletteKind},
            formula::{FormulaBlendMode, FormulaControl, FormulaLayer, FormulaSource},
            renderer::{EffectBlendMode, EffectLayer, PatternLayer, RenderParams},
        },
    };

    #[test]
    fn workflow_roundtrip_preserves_project_and_asset_names() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("workflow.json");
        let mut workflow = WorkflowFile::default();
        workflow.project.export_settings.width = 384;
        workflow.project.export_settings.height = 216;

        save_workflow(&path, &workflow).expect("save workflow");
        let loaded = load_workflow_or_preset(&path).expect("load workflow");

        assert_eq!(loaded.project.export_settings.width, 384);
        assert_eq!(loaded.project.export_settings.height, 216);
    }

    #[test]
    fn workflow_json_does_not_persist_runtime_status_fields() {
        let workflow = WorkflowFile::from_project(ProjectState::default());

        let json = serde_json::to_string_pretty(&workflow).expect("serialize workflow");

        for forbidden in [
            "status_message",
            "runtime_message",
            "export_progress",
            "export_in_progress",
            "export_cancel",
            "export_receiver",
            "last_preview_backend",
            "last_export_backend",
            "preview_texture",
            "preview_generation",
        ] {
            assert!(
                !json.contains(forbidden),
                "workflow JSON must not persist runtime field {forbidden}: {json}"
            );
        }
        assert!(
            !json.contains("\"render_mode\""),
            "GPU-only rendering is runtime behavior and workflow JSON must not persist render_mode: {json}"
        );
    }

    #[test]
    fn workflow_loader_accepts_user_legacy_preset_files() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("preset.json");
        std::fs::write(
            &path,
            r#"{
                "name": "User Legacy Preset",
                "render_params": {
                    "zoom": 2.25
                }
            }"#,
        )
        .expect("write preset");

        let loaded = load_workflow_or_preset(&path).expect("load preset through workflow loader");

        assert_eq!(loaded.project.render_params.zoom, 2.25);
    }

    #[test]
    fn legacy_palette_only_preset_seeds_active_custom_gradient() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("legacy-palette-only.json");
        std::fs::write(
            &path,
            r#"{
                "name": "Legacy Fire",
                "render_params": {
                    "palette": "fire",
                    "patterns": []
                }
            }"#,
        )
        .expect("write legacy preset");

        let loaded = load_workflow_or_preset(&path).expect("load legacy preset");

        assert_eq!(loaded.project.render_params.palette, PaletteKind::Fire);
        assert!(loaded.project.render_params.custom_gradient.enabled);
        assert_eq!(
            loaded.project.render_params.custom_gradient.stops()[0],
            [0.02, 0.00, 0.02]
        );
        assert_eq!(
            loaded.project.render_params.custom_gradient.stops()[3],
            [1.00, 0.82, 0.24]
        );
    }

    #[test]
    fn legacy_disabled_custom_gradient_migrates_to_palette_seed() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("legacy-disabled-gradient.json");
        std::fs::write(
            &path,
            r#"{
                "project": {
                    "render_params": {
                        "palette": "mono_chrome",
                        "custom_gradient": {
                            "enabled": false,
                            "colors": [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
                        }
                    }
                }
            }"#,
        )
        .expect("write legacy workflow");

        let loaded = load_workflow_or_preset(&path).expect("load legacy workflow");

        assert_eq!(
            loaded.project.render_params.palette,
            PaletteKind::MonoChrome
        );
        assert!(loaded.project.render_params.custom_gradient.enabled);
        assert_eq!(
            loaded.project.render_params.custom_gradient.stops()[0],
            [0.0, 0.0, 0.0]
        );
        assert_eq!(
            loaded.project.render_params.custom_gradient.stops()[3],
            [1.0, 1.0, 1.0]
        );
    }

    #[test]
    fn workflow_full_roundtrip_preserves_creative_state() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("full-workflow.json");
        let workflow = WorkflowFile::from_project(complex_project());

        save_workflow(&path, &workflow).expect("save workflow");
        let loaded = load_workflow_or_preset(&path).expect("load workflow");

        assert_eq!(loaded.project.export_settings.width, 377);
        assert_eq!(loaded.project.export_settings.height, 289);
        assert_eq!(loaded.project.export_settings.fps, 37);
        assert_eq!(loaded.project.export_settings.duration_seconds, 7.5);
        assert_eq!(loaded.project.export_settings.lossy_quality, 73);
        assert!(!loaded.project.export_settings.fast);

        let params = &loaded.project.render_params;
        assert_eq!(params.patterns.len(), 2);
        assert!(params.patterns[0].enabled);
        assert!(!params.patterns[1].enabled);
        assert_eq!(params.patterns[0].source.controls.len(), 4);
        assert_eq!(params.patterns[0].source.controls[0].value, 1.25);
        assert_eq!(params.patterns[0].source.controls[1].value, -0.5);
        assert_eq!(params.patterns[0].source.controls[2].value, 2.75);
        assert_eq!(params.patterns[0].source.controls[3].value, 0.33);
        assert_eq!(params.patterns[0].source.layers.len(), 2);
        assert!(
            params.patterns[0].source.layers[1]
                .expression
                .contains("prev")
        );
        assert!(
            params.patterns[0].source.layers[0]
                .expression
                .contains("p1")
        );
        assert!(
            params.patterns[0].source.layers[0]
                .expression
                .contains("p2")
        );
        assert!(
            params.patterns[0].source.layers[1]
                .expression
                .contains("p3")
        );
        assert!(
            params.patterns[0].source.layers[1]
                .expression
                .contains("p4")
        );
        assert_eq!(
            params.patterns[0].source.layers[0].domain_x,
            "x + sin(y * p1) * p2"
        );
        assert_eq!(
            params.patterns[0].source.layers[0].domain_y,
            "y + cos(x * p3) * p4"
        );
        assert_eq!(params.patterns[0].source.layers[0].domain_influence, 0.64);

        assert_eq!(params.effects.len(), 2);
        assert_eq!(params.effects[0].blend_mode, EffectBlendMode::Difference);
        assert_eq!(params.effects[1].blend_mode, EffectBlendMode::Displace);
        assert!(!params.effects[1].enabled);
        assert_eq!(params.effects[0].strength, 0.82);
        assert_eq!(params.effects[0].scale, 3.4);
        assert_eq!(params.effects[0].motion, -1.2);
        assert_eq!(params.effects[1].camera_zoom_loop, 1.4);

        assert_eq!(params.palette, PaletteKind::Fire);
        assert!(params.custom_gradient.enabled);
        assert_eq!(params.custom_gradient.colors.len(), 10);
        assert_eq!(params.custom_gradient.color_transitions.len(), 10);
        assert_eq!(
            params.custom_gradient.color_transitions[8][1].color,
            [0.91, 0.21, 0.31]
        );
        assert_eq!(params.color_speed, -2.25);
        assert_eq!(params.color_phase, 0.42);
        assert_eq!(params.brightness, 1.37);
        assert_eq!(params.contrast, 2.4);
    }

    #[test]
    fn legacy_minimal_workflow_without_render_mode_loads_project_defaults() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("legacy-workflow.json");
        std::fs::write(
            &path,
            r#"{
                "project": {
                    "export_settings": {
                        "width": 320,
                        "height": 180
                    }
                }
            }"#,
        )
        .expect("write legacy workflow");

        let loaded = load_workflow_or_preset(&path).expect("load legacy workflow");

        assert_eq!(loaded.project.export_settings.width, 320);
        assert_eq!(loaded.project.export_settings.height, 180);
        assert_eq!(
            loaded.project.export_settings.fps,
            ExportSettings::default().fps
        );
    }

    #[test]
    fn workflow_effect_modes_roundtrip_preserves_all_modes_and_parameters() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let path = temp_dir.path().join("effect-matrix-workflow.json");
        let workflow = WorkflowFile::from_project(effect_matrix_project());

        save_workflow(&path, &workflow).expect("save workflow");
        let loaded = load_workflow_or_preset(&path).expect("load workflow");

        assert_eq!(
            loaded.project.render_params.effects.len(),
            EffectBlendMode::ALL.len()
        );
        for (index, mode) in EffectBlendMode::ALL.iter().copied().enumerate() {
            let effect = &loaded.project.render_params.effects[index];
            assert_eq!(effect.blend_mode, mode);
            assert_eq!(effect.source.effect_blend_mode, Some(mode));
            assert_eq!(effect.enabled, index != 1);
            assert_eq!(effect.strength, 0.2 + index as f32 * 0.07);
            assert_eq!(effect.scale, 0.6 + index as f32 * 0.3);
            assert_eq!(effect.motion, -2.0 + index as f32 * 0.45);
            assert_eq!(effect.morph, index as f32 * 0.05);
            assert_eq!(effect.camera_zoom_loop, index as f32 * 0.08);
            assert_eq!(effect.camera_orbit, -0.7 + index as f32 * 0.16);
        }
    }

    fn complex_project() -> ProjectState {
        ProjectState {
            render_params: RenderParams {
                patterns: vec![
                    PatternLayer {
                        name: "Primary controlled pattern".to_owned(),
                        enabled: true,
                        source: complex_formula_source(None),
                        strength: 0.91,
                        scale: 4.2,
                        motion: -2.1,
                        morph: 0.37,
                        camera_zoom_loop: 0.75,
                        camera_orbit: -0.45,
                    },
                    PatternLayer {
                        name: "Disabled secondary pattern".to_owned(),
                        enabled: false,
                        source: FormulaSource::fractal_b(),
                        strength: 0.44,
                        scale: 2.8,
                        motion: 1.6,
                        morph: 0.22,
                        camera_zoom_loop: 0.1,
                        camera_orbit: 0.9,
                    },
                ],
                effects: vec![
                    EffectLayer {
                        name: "Difference grain".to_owned(),
                        enabled: true,
                        source: complex_formula_source(Some(EffectBlendMode::Difference)),
                        blend_mode: EffectBlendMode::Difference,
                        strength: 0.82,
                        scale: 3.4,
                        motion: -1.2,
                        morph: 0.18,
                        camera_zoom_loop: 0.35,
                        camera_orbit: -0.2,
                    },
                    EffectLayer {
                        name: "Disabled displace".to_owned(),
                        enabled: false,
                        source: FormulaSource::pattern(),
                        blend_mode: EffectBlendMode::Displace,
                        strength: 0.56,
                        scale: 5.5,
                        motion: 2.2,
                        morph: 0.47,
                        camera_zoom_loop: 1.4,
                        camera_orbit: 0.65,
                    },
                ],
                seed: 77_777,
                zoom: 5.25,
                center_x: -0.23,
                center_y: 0.41,
                rotation_speed: -1.7,
                color_speed: -2.25,
                color_phase: 0.42,
                palette: PaletteKind::Fire,
                custom_gradient: ten_color_gradient(),
                symmetry: 13,
                distortion: 2.2,
                detail: 3.3,
                smoothing: 12.0,
                smoothing_radius_pixels: 4.5,
                brightness: 1.37,
                contrast: 2.4,
            },
            export_settings: ExportSettings {
                width: 377,
                height: 289,
                fps: 37,
                duration_seconds: 7.5,
                lossy_quality: 73,
                fast: false,
                output_path: temp_export_path(),
            },
        }
    }

    fn effect_matrix_project() -> ProjectState {
        ProjectState {
            render_params: RenderParams {
                effects: EffectBlendMode::ALL
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, mode)| EffectLayer {
                        name: format!("{} effect", mode.label()),
                        enabled: index != 1,
                        source: FormulaSource {
                            expression: "0.5 + 0.5 * sin(x * scale + t * motion)".to_owned(),
                            gain: 1.0,
                            bias: 0.0,
                            effect_blend_mode: Some(mode),
                            controls: Vec::new(),
                            layers: vec![FormulaLayer {
                                name: "Effect layer".to_owned(),
                                expression: "0.5 + 0.5 * sin(x * scale + y + t * motion)"
                                    .to_owned(),
                                ..FormulaLayer::default()
                            }],
                        },
                        blend_mode: mode,
                        strength: 0.2 + index as f32 * 0.07,
                        scale: 0.6 + index as f32 * 0.3,
                        motion: -2.0 + index as f32 * 0.45,
                        morph: index as f32 * 0.05,
                        camera_zoom_loop: index as f32 * 0.08,
                        camera_orbit: -0.7 + index as f32 * 0.16,
                    })
                    .collect(),
                ..RenderParams::default()
            },
            export_settings: ExportSettings {
                output_path: temp_export_path(),
                ..ExportSettings::default()
            },
        }
    }

    fn complex_formula_source(effect_blend_mode: Option<EffectBlendMode>) -> FormulaSource {
        FormulaSource {
            expression: "sin(x * p1 + y * p2 + t)".to_owned(),
            gain: 1.1,
            bias: -0.1,
            effect_blend_mode,
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
                    name: "Domain carrier".to_owned(),
                    expression: "0.5 + 0.5 * sin(x * p1 + y * p2 + t)".to_owned(),
                    gain: 0.9,
                    bias: 0.05,
                    opacity: 0.88,
                    blend_mode: FormulaBlendMode::Replace,
                    repeat_x: 1.7,
                    repeat_y: 2.3,
                    warp_x: 0.4,
                    warp_y: -0.6,
                    offset_x: 0.12,
                    offset_y: -0.34,
                    motion_x: 0.7,
                    motion_y: -0.8,
                    domain_x: "x + sin(y * p1) * p2".to_owned(),
                    domain_y: "y + cos(x * p3) * p4".to_owned(),
                    domain_influence: 0.64,
                    enabled: true,
                },
                FormulaLayer {
                    name: "Prev detail".to_owned(),
                    expression: "prev * 0.6 + smoothstep(0.1, 0.9, abs(sin(r * p3 + p4)))"
                        .to_owned(),
                    gain: 1.2,
                    bias: -0.2,
                    opacity: 0.53,
                    blend_mode: FormulaBlendMode::Screen,
                    repeat_x: 0.75,
                    repeat_y: 1.25,
                    warp_x: -0.3,
                    warp_y: 0.2,
                    offset_x: -0.22,
                    offset_y: 0.18,
                    motion_x: -0.45,
                    motion_y: 0.38,
                    domain_x: "x + prev * 0.1".to_owned(),
                    domain_y: "y - prev * 0.2".to_owned(),
                    domain_influence: 0.28,
                    enabled: true,
                },
            ],
        }
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

    fn temp_export_path() -> std::path::PathBuf {
        std::path::PathBuf::from("exports/full-roundtrip.gif")
    }
}
