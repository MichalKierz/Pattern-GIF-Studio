use std::path::{Path, PathBuf};

pub fn pick_gif_output(default_path: PathBuf) -> Option<PathBuf> {
    let (default_dir, default_file_name) = split_default_gif_path(&default_path);
    rfd::FileDialog::new()
        .set_directory(default_dir)
        .add_filter("GIF image", &["gif"])
        .set_file_name(default_file_name)
        .save_file()
}

pub fn pick_json_file(default_dir: PathBuf, filter_label: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_directory(default_dir)
        .add_filter(filter_label, &["json"])
        .pick_file()
}

pub fn save_json_file(default_path: PathBuf, filter_label: &str) -> Option<PathBuf> {
    let (default_dir, default_file_name) = split_default_json_path(&default_path);
    rfd::FileDialog::new()
        .set_directory(default_dir)
        .add_filter(filter_label, &["json"])
        .set_file_name(default_file_name)
        .save_file()
}

fn split_default_gif_path(path: &Path) -> (PathBuf, String) {
    if path.extension().is_some() {
        let dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pattern-loop.gif")
            .to_owned();
        (dir, file_name)
    } else {
        (path.to_path_buf(), "pattern-loop.gif".to_owned())
    }
}

fn split_default_json_path(path: &Path) -> (PathBuf, String) {
    if path.extension().is_some() {
        let dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workflow.json")
            .to_owned();
        (dir, file_name)
    } else {
        (path.to_path_buf(), "workflow.json".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{split_default_gif_path, split_default_json_path};

    #[test]
    fn gif_dialog_default_uses_current_file_parent_and_name() {
        let (dir, file_name) = split_default_gif_path(std::path::Path::new(
            r"D:\Code\fractal-gif-studio\exports\custom.gif",
        ));

        assert_eq!(
            dir,
            std::path::PathBuf::from(r"D:\Code\fractal-gif-studio\exports")
        );
        assert_eq!(file_name, "custom.gif");
    }

    #[test]
    fn gif_dialog_default_uses_directory_with_standard_name() {
        let (dir, file_name) =
            split_default_gif_path(std::path::Path::new(r"D:\Code\fractal-gif-studio\exports"));

        assert_eq!(
            dir,
            std::path::PathBuf::from(r"D:\Code\fractal-gif-studio\exports")
        );
        assert_eq!(file_name, "pattern-loop.gif");
    }

    #[test]
    fn json_dialog_default_uses_current_file_parent_and_name() {
        let (dir, file_name) = split_default_json_path(std::path::Path::new(
            r"D:\Code\fractal-gif-studio\presets\workflows\scene.json",
        ));

        assert_eq!(
            dir,
            std::path::PathBuf::from(r"D:\Code\fractal-gif-studio\presets\workflows")
        );
        assert_eq!(file_name, "scene.json");
    }
}
