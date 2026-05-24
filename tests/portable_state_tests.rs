use pattern_gif_studio::{
    export::{
        export_settings::ExportSettings,
        history::{ExportHistoryEntry, load_history, push_history_entry},
    },
    project::{
        project_state::ProjectState,
        session_state::{SessionState, load_session, save_session, session_path},
    },
    utils::app_log::{append_log, log_path},
};

#[test]
fn session_state_roundtrip_stays_inside_app_data() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut project = ProjectState::default();
    project.export_settings.width = 420;
    let session = SessionState::from_parts(project);

    let path = save_session(temp_dir.path(), &session).expect("save session");
    assert_eq!(path, session_path(temp_dir.path()));
    let json = std::fs::read_to_string(&path).expect("read session json");

    let loaded = load_session(temp_dir.path())
        .expect("load session")
        .expect("session exists");
    assert_eq!(loaded.project.export_settings.width, 420);
    assert!(
        !json.contains("custom_assets")
            && !json.contains("custom_pattern_source_dir")
            && !json.contains("custom_effect_source_dir")
            && !json.contains("custom_color_set_dir"),
        "session must not persist obsolete source asset directories: {json}"
    );
}

#[test]
fn legacy_session_render_modes_are_ignored_by_gpu_only_session() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let session_json = r#"{
      "project": {},
      "active_tab": "Settings",
      "preset_name": "Legacy",
      "gif_render_mode": "legacy",
      "ui_render_mode": "gpu"
    }"#;
    std::fs::write(session_path(temp_dir.path()), session_json).expect("write legacy session");

    let loaded = load_session(temp_dir.path())
        .expect("load session")
        .expect("session exists");

    assert_eq!(
        loaded.project.export_settings.width,
        ProjectState::default().export_settings.width
    );
}

#[test]
fn export_history_keeps_latest_entry_first_and_deduplicates_paths() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let mut history = Vec::new();
    let mut settings = ExportSettings {
        output_path: temp_dir.path().join("a.gif"),
        ..ExportSettings::default()
    };

    push_history_entry(
        temp_dir.path(),
        &mut history,
        ExportHistoryEntry::from_settings(&settings),
    )
    .expect("push first");
    settings.width = 512;
    push_history_entry(
        temp_dir.path(),
        &mut history,
        ExportHistoryEntry::from_settings(&settings),
    )
    .expect("push duplicate");

    let loaded = load_history(temp_dir.path());
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].width, 512);
}

#[test]
fn export_history_load_migrates_external_paths_into_portable_exports() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let app_data_dir = temp_dir.path().join("app_data");
    std::fs::create_dir_all(&app_data_dir).expect("app data");
    std::fs::write(
        app_data_dir.join("export_history.json"),
        serde_json::json!([
            {
                "output_path": "C:\\Users\\me\\Desktop\\outside.gif",
                "width": 512,
                "height": 512,
                "fps": 24,
                "duration_seconds": 2.0,
                "lossy_quality": 100,
                "fast": true,
                "created_unix_seconds": 1
            }
        ])
        .to_string(),
    )
    .expect("write history");

    let loaded = load_history(&app_data_dir);

    assert_eq!(loaded.len(), 1);
    assert_eq!(
        loaded[0].output_path,
        temp_dir.path().join("exports").join("outside.gif")
    );
    assert!(loaded[0].output_path.starts_with(temp_dir.path()));
    let migrated = std::fs::read_to_string(app_data_dir.join("export_history.json"))
        .expect("read migrated history");
    assert!(!migrated.contains("C:\\\\Users"));
}

#[test]
fn export_history_load_deduplicates_after_path_migration() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let app_data_dir = temp_dir.path().join("app_data");
    std::fs::create_dir_all(&app_data_dir).expect("app data");
    std::fs::write(
        app_data_dir.join("export_history.json"),
        serde_json::json!([
            {
                "output_path": "C:\\Users\\me\\Desktop\\same.gif",
                "width": 512,
                "height": 512,
                "fps": 24,
                "duration_seconds": 2.0,
                "lossy_quality": 100,
                "fast": true,
                "created_unix_seconds": 2
            },
            {
                "output_path": "D:\\Other\\same.gif",
                "width": 256,
                "height": 256,
                "fps": 12,
                "duration_seconds": 1.0,
                "lossy_quality": 80,
                "fast": false,
                "created_unix_seconds": 1
            }
        ])
        .to_string(),
    )
    .expect("write history");

    let loaded = load_history(&app_data_dir);

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].width, 512);
    assert_eq!(
        loaded[0].output_path,
        temp_dir.path().join("exports").join("same.gif")
    );
}

#[test]
fn app_log_writes_inside_app_data_logs_folder() {
    let temp_dir = tempfile::tempdir().expect("temp dir");

    append_log(temp_dir.path(), "portable log entry").expect("append log");

    let path = log_path(temp_dir.path());
    assert!(path.starts_with(temp_dir.path()));
    let text = std::fs::read_to_string(path).expect("read log");
    assert!(text.contains("portable log entry"));
}
