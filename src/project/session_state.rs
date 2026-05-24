use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::project::project_state::ProjectState;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionState {
    pub project: ProjectState,
}

impl SessionState {
    pub fn from_parts(project: ProjectState) -> Self {
        Self { project }
    }
}

pub fn session_path(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("session.json")
}

pub fn load_session(app_data_dir: &Path) -> Result<Option<SessionState>> {
    let path = session_path(app_data_dir);
    if !path.exists() {
        return Ok(None);
    }

    let text =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut session: SessionState =
        serde_json::from_str(&text).with_context(|| format!("invalid {}", path.display()))?;
    session.project.sanitize();
    Ok(Some(session))
}

pub fn save_session(app_data_dir: &Path, session: &SessionState) -> Result<PathBuf> {
    fs::create_dir_all(app_data_dir)
        .with_context(|| format!("failed to create {}", app_data_dir.display()))?;
    let path = session_path(app_data_dir);
    let json = serde_json::to_string_pretty(session).context("failed to serialize session")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}
