use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};

pub fn save_asset<T: Serialize>(dir: &Path, name: &str, asset: &T) -> Result<PathBuf> {
    fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let path = dir.join(asset_file_name(name));
    let json = serde_json::to_string_pretty(asset).context("failed to serialize asset")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub fn save_asset_to_path<T: Serialize>(path: &Path, asset: &T) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(asset).context("failed to serialize asset")?;
    fs::write(path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path.to_path_buf())
}

pub fn load_asset<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("invalid asset {}", path.display()))
}

fn asset_file_name(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "source-asset.json".to_owned()
    } else {
        format!("{slug}.json")
    }
}

#[cfg(test)]
mod tests {
    use super::asset_file_name;

    #[test]
    fn asset_file_names_are_stable_ascii_json() {
        assert_eq!(asset_file_name("My Fractal 01!"), "my-fractal-01.json");
        assert_eq!(asset_file_name("###"), "source-asset.json");
    }
}
