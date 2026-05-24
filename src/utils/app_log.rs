use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

pub fn log_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("logs").join("pattern-gif-studio.log")
}

pub fn append_log(app_data_dir: &Path, message: impl AsRef<str>) -> Result<()> {
    let path = log_path(app_data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "[{}] {}", unix_seconds(), message.as_ref())
        .with_context(|| format!("failed to write {}", path.display()))
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
