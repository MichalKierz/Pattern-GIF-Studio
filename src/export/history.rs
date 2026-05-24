use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    export::export_settings::ExportSettings,
    utils::portable_paths::{is_inside_root, portable_file_or_default},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportHistoryEntry {
    pub output_path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_seconds: f32,
    pub lossy_quality: u8,
    pub fast: bool,
    pub created_unix_seconds: u64,
}

impl ExportHistoryEntry {
    pub fn from_settings(settings: &ExportSettings) -> Self {
        Self {
            output_path: settings.output_path.clone(),
            width: settings.width,
            height: settings.height,
            fps: settings.fps,
            duration_seconds: settings.duration_seconds,
            lossy_quality: settings.lossy_quality,
            fast: settings.fast,
            created_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default(),
        }
    }

    pub fn label(&self) -> String {
        format!(
            "{} - {}x{} {} FPS {:.1}s",
            self.output_path.display(),
            self.width,
            self.height,
            self.fps,
            self.duration_seconds
        )
    }
}

impl Default for ExportHistoryEntry {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("exports/pattern-loop.gif"),
            width: 800,
            height: 800,
            fps: 24,
            duration_seconds: 4.0,
            lossy_quality: 100,
            fast: false,
            created_unix_seconds: 0,
        }
    }
}

pub fn history_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("export_history.json")
}

pub fn load_history(app_data_dir: &Path) -> Vec<ExportHistoryEntry> {
    let path = history_path(app_data_dir);
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(entries) = serde_json::from_str::<Vec<ExportHistoryEntry>>(&text) else {
        return Vec::new();
    };
    let root = portable_root_from_app_data(app_data_dir);
    let default_output = root.join("exports").join("pattern-loop.gif");
    let (entries, changed) = sanitize_history_entries(entries, &root, &default_output);
    if changed {
        let _ = save_history(app_data_dir, &entries);
    }
    entries
}

pub fn save_history(app_data_dir: &Path, entries: &[ExportHistoryEntry]) -> Result<()> {
    fs::create_dir_all(app_data_dir)
        .with_context(|| format!("failed to create {}", app_data_dir.display()))?;
    let path = history_path(app_data_dir);
    let json =
        serde_json::to_string_pretty(entries).context("failed to serialize export history")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))
}

pub fn push_history_entry(
    app_data_dir: &Path,
    entries: &mut Vec<ExportHistoryEntry>,
    entry: ExportHistoryEntry,
) -> Result<()> {
    entries.retain(|existing| existing.output_path != entry.output_path);
    entries.insert(0, entry);
    entries.truncate(20);
    save_history(app_data_dir, entries)
}

fn portable_root_from_app_data(app_data_dir: &Path) -> PathBuf {
    if app_data_dir
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "app_data")
    {
        app_data_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| app_data_dir.to_path_buf())
    } else {
        app_data_dir.to_path_buf()
    }
}

fn sanitize_history_entries(
    entries: Vec<ExportHistoryEntry>,
    root: &Path,
    default_output: &Path,
) -> (Vec<ExportHistoryEntry>, bool) {
    let mut changed = false;
    let mut sanitized = Vec::with_capacity(entries.len());
    for mut entry in entries {
        let original = entry.output_path.clone();
        entry.output_path = portable_file_or_default(entry.output_path, root, default_output);
        changed |= entry.output_path != original || !is_inside_root(&entry.output_path, root);
        if !sanitized
            .iter()
            .any(|existing: &ExportHistoryEntry| existing.output_path == entry.output_path)
        {
            sanitized.push(entry);
        } else {
            changed = true;
        }
    }
    sanitized.truncate(20);
    (sanitized, changed)
}
