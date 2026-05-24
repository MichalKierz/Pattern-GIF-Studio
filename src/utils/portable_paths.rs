use std::path::{Path, PathBuf};

pub fn is_inside_root(path: &Path, root: &Path) -> bool {
    if path.is_relative() {
        return false;
    }
    lexical_normalize(path).starts_with(lexical_normalize(root))
}

pub fn portable_dir_or_default(path: PathBuf, root: &Path, default: &Path) -> PathBuf {
    if is_inside_root(&path, root) && !is_inside_target_profile(&path, root) {
        path
    } else {
        default.to_path_buf()
    }
}

pub fn portable_file_or_default(path: PathBuf, root: &Path, default: &Path) -> PathBuf {
    if is_inside_root(&path, root) && !is_inside_target_profile(&path, root) {
        return path;
    }

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| {
            default
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("pattern-loop.gif")
        });

    default
        .parent()
        .map(|parent| parent.join(file_name))
        .unwrap_or_else(|| default.to_path_buf())
}

fn is_inside_target_profile(path: &Path, root: &Path) -> bool {
    let normalized_path = lexical_normalize(path);
    let normalized_root = lexical_normalize(root);
    let Ok(relative) = normalized_path.strip_prefix(normalized_root) else {
        return false;
    };
    let mut components = relative.components();
    let Some(std::path::Component::Normal(first)) = components.next() else {
        return false;
    };
    let Some(std::path::Component::Normal(second)) = components.next() else {
        return false;
    };
    first == "target" && (second == "debug" || second == "release")
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{is_inside_root, portable_dir_or_default, portable_file_or_default};

    #[test]
    fn outside_directory_falls_back_to_portable_default() {
        let root = std::path::Path::new(r"D:\Portable\PatternGifStudio");
        let default = root.join("presets").join("patterns");
        let outside = std::path::PathBuf::from(r"C:\Users\me\Desktop");

        assert_eq!(portable_dir_or_default(outside, root, &default), default);
    }

    #[test]
    fn outside_file_keeps_name_but_moves_under_portable_default_parent() {
        let root = std::path::Path::new(r"D:\Portable\PatternGifStudio");
        let default = root.join("exports").join("pattern-loop.gif");
        let outside = std::path::PathBuf::from(r"C:\Users\me\Desktop\loop.gif");

        assert_eq!(
            portable_file_or_default(outside, root, &default),
            root.join("exports").join("loop.gif")
        );
    }

    #[test]
    fn relative_paths_are_not_treated_as_portable_state_paths() {
        let root = std::path::Path::new(r"D:\Portable\PatternGifStudio");

        assert!(!is_inside_root(
            std::path::Path::new("exports\\loop.gif"),
            root
        ));
    }

    #[test]
    fn path_traversal_inside_root_is_not_treated_as_portable() {
        let root = std::path::Path::new(r"D:\Portable\PatternGifStudio");
        let traversal = root
            .join("exports")
            .join("..")
            .join("..")
            .join("outside.gif");

        assert!(!is_inside_root(&traversal, root));
        assert_eq!(
            portable_file_or_default(traversal, root, &root.join("exports").join("loop.gif")),
            root.join("exports").join("outside.gif")
        );
    }

    #[test]
    fn stale_cargo_target_directories_fall_back_to_project_portable_dirs() {
        let root = std::path::Path::new(r"D:\Code\fractal-gif-studio");
        let default = root.join("presets").join("patterns");
        let stale_dir = root
            .join("target")
            .join("debug")
            .join("presets")
            .join("patterns");
        let stale_file = root
            .join("target")
            .join("debug")
            .join("exports")
            .join("loop.gif");

        assert_eq!(portable_dir_or_default(stale_dir, root, &default), default);
        assert_eq!(
            portable_file_or_default(stale_file, root, &root.join("exports").join("loop.gif")),
            root.join("exports").join("loop.gif")
        );
    }
}
