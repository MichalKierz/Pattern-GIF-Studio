use pattern_gif_studio::{
    export::export_settings::ExportSettings, project::project_state::ProjectState,
};

#[test]
fn manual_dimensions_are_preserved_during_sanitize() {
    let mut settings = ExportSettings {
        width: 999,
        height: 777,
        ..ExportSettings::default()
    };

    settings.sanitize();

    assert_eq!(settings.width, 999);
    assert_eq!(settings.height, 777);
}

#[test]
fn custom_size_is_clamped_and_gif_extension_is_enforced() {
    let mut settings = ExportSettings {
        width: 32,
        height: 5000,
        fps: 200,
        duration_seconds: 0.01,
        lossy_quality: 200,
        output_path: "exports/render".into(),
        ..ExportSettings::default()
    };

    settings.sanitize();

    assert_eq!(settings.width, 64);
    assert_eq!(settings.height, 1000);
    assert_eq!(settings.fps, ExportSettings::MAX_GIF_FPS);
    assert_eq!(settings.duration_seconds, 0.25);
    assert_eq!(settings.lossy_quality, 100);
    assert_eq!(
        settings
            .output_path
            .extension()
            .and_then(|ext| ext.to_str()),
        Some("gif")
    );
}

#[test]
fn project_sanitize_allows_strong_smoothing() {
    let mut project = ProjectState::default();
    project.render_params.smoothing = 140.0;
    project.render_params.smoothing_radius_pixels = 14.0;

    project.sanitize();

    assert_eq!(project.render_params.smoothing, 20.0);
    assert_eq!(project.render_params.smoothing_radius_pixels, 10.0);
}

#[test]
fn legacy_preview_fields_are_ignored_and_not_serialized() {
    let json = r#"{
        "render_params": {},
        "export_settings": {
            "width": 321,
            "height": 234,
            "fps": 19,
            "duration_seconds": 3.5
        },
        "preview_width": 999,
        "preview_height": 888,
        "preview_fps": 7
    }"#;

    let mut project: ProjectState = serde_json::from_str(json).expect("legacy project JSON");
    project.sanitize();

    assert_eq!(project.export_settings.width, 321);
    assert_eq!(project.export_settings.height, 234);
    assert_eq!(project.export_settings.fps, 19);
    assert_eq!(project.export_settings.duration_seconds, 3.5);

    let serialized = serde_json::to_value(project).expect("serialize project");
    assert!(serialized.get("preview_width").is_none());
    assert!(serialized.get("preview_height").is_none());
    assert!(serialized.get("preview_fps").is_none());
}
