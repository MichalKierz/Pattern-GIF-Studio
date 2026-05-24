use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub portable_root: PathBuf,
    pub app_data_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub bundled_presets_dir: PathBuf,
    pub bundled_pattern_presets_dir: PathBuf,
    pub bundled_effect_presets_dir: PathBuf,
    pub bundled_color_set_presets_dir: PathBuf,
    pub bundled_workflow_presets_dir: PathBuf,
}

impl AppSettings {
    pub fn portable() -> Result<Self> {
        Self::for_root(portable_root()?)
    }

    pub fn for_root(root: PathBuf) -> Result<Self> {
        let app_data_dir = root.join("app_data");
        let exports_dir = root.join("exports");
        let bundled_presets_dir = root.join("presets");
        let bundled_pattern_presets_dir = bundled_presets_dir.join("patterns");
        let bundled_effect_presets_dir = bundled_presets_dir.join("effects");
        let bundled_color_set_presets_dir = bundled_presets_dir.join("color_sets");
        let bundled_workflow_presets_dir = bundled_presets_dir.join("workflows");

        for dir in [
            &app_data_dir,
            &exports_dir,
            &bundled_presets_dir,
            &bundled_pattern_presets_dir,
            &bundled_effect_presets_dir,
            &bundled_color_set_presets_dir,
            &bundled_workflow_presets_dir,
        ] {
            fs::create_dir_all(dir)?;
        }

        Ok(Self {
            portable_root: root,
            app_data_dir,
            exports_dir,
            bundled_presets_dir,
            bundled_pattern_presets_dir,
            bundled_effect_presets_dir,
            bundled_color_set_presets_dir,
            bundled_workflow_presets_dir,
        })
    }
}

fn portable_root() -> Result<PathBuf> {
    portable_root_from(env::current_exe().ok())
        .context("failed to identify portable executable directory")
}

fn portable_root_from(current_exe: Option<PathBuf>) -> Option<PathBuf> {
    let exe_dir = current_exe.and_then(|path| path.parent().map(Path::to_path_buf))?;
    Some(dev_project_root(&exe_dir).unwrap_or(exe_dir))
}

fn dev_project_root(exe_dir: &Path) -> Option<PathBuf> {
    let profile_dir = exe_dir.file_name()?.to_str()?;
    if profile_dir != "debug" && profile_dir != "release" {
        return None;
    }

    let target_dir = exe_dir.parent()?;
    if target_dir.file_name()?.to_str()? != "target" {
        return None;
    }

    let project_root = target_dir.parent()?;
    if project_root.join("Cargo.toml").is_file() && project_root.join("presets").is_dir() {
        Some(project_root.to_path_buf())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::portable_root_from;

    #[test]
    fn portable_root_prefers_exe_directory_over_current_directory() {
        let exe = std::path::PathBuf::from(r"D:\Apps\PatternGifStudio\pattern-gif-studio.exe");

        assert_eq!(
            portable_root_from(Some(exe)),
            Some(std::path::PathBuf::from(r"D:\Apps\PatternGifStudio"))
        );
    }

    #[test]
    fn portable_root_does_not_fall_back_to_current_directory_without_exe_path() {
        assert_eq!(portable_root_from(None), None);
    }

    #[test]
    fn cargo_run_uses_project_root_instead_of_target_profile_dir() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"app\"\n",
        )
        .expect("cargo toml");
        std::fs::create_dir(temp_dir.path().join("presets")).expect("presets");
        let exe_dir = temp_dir.path().join("target").join("debug");
        std::fs::create_dir_all(&exe_dir).expect("target debug");
        let exe = exe_dir.join("pattern-gif-studio.exe");

        assert_eq!(
            portable_root_from(Some(exe)),
            Some(temp_dir.path().to_path_buf())
        );
    }

    #[test]
    fn for_root_creates_portable_preset_and_data_directories() {
        let temp_dir = tempfile::tempdir().expect("temp dir");

        let settings =
            super::AppSettings::for_root(temp_dir.path().to_path_buf()).expect("portable settings");

        for dir in [
            &settings.app_data_dir,
            &settings.exports_dir,
            &settings.bundled_pattern_presets_dir,
            &settings.bundled_effect_presets_dir,
            &settings.bundled_color_set_presets_dir,
            &settings.bundled_workflow_presets_dir,
        ] {
            assert!(
                dir.is_dir(),
                "portable directory should exist: {}",
                dir.display()
            );
            assert!(dir.starts_with(&settings.portable_root));
        }
        assert!(
            !temp_dir.path().join("custom_assets").exists(),
            "custom_assets is obsolete; source assets save/load through presets subfolders"
        );
        assert!(
            !temp_dir.path().join("workflows").exists(),
            "workflow presets live in presets/workflows; root workflows is obsolete"
        );
    }
}
