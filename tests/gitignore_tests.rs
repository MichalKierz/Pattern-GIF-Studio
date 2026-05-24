fn gitignore_entries() -> Vec<&'static str> {
    include_str!("../.gitignore")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

#[test]
fn gitignore_keeps_runtime_and_generated_paths_out_of_github() {
    let entries = gitignore_entries();

    for required in [
        "/target/",
        "/target_test/",
        "/exports/",
        "/app_data/",
        "/custom_assets/",
        "/build/portable/",
        "*.gif",
        "*.exe",
        "*.dll",
        "*.pdb",
    ] {
        assert!(
            entries.contains(&required),
            ".gitignore should contain {required}"
        );
    }
}

#[test]
fn gitignore_does_not_hide_source_tests_or_bundled_presets() {
    let entries = gitignore_entries();

    for forbidden in [
        "/tests/",
        "tests/",
        "/presets/",
        "presets/",
        "/build/",
        "build/",
        "*.json",
        "*.md",
        "*.ps1",
        "*.toml",
    ] {
        assert!(
            !entries.contains(&forbidden),
            ".gitignore should not hide source path or file type {forbidden}"
        );
    }
}

#[test]
fn portable_package_script_seeds_runtime_dirs_without_custom_assets_or_readme() {
    let script = include_str!("../build/package_windows_portable.ps1");

    assert!(
        script.contains("@(\"app_data\", \"exports\")"),
        "portable package should seed only the active runtime directories"
    );
    assert!(
        !script.contains("\"workflows\""),
        "portable package must not recreate obsolete root workflows directory"
    );
    assert!(
        !script.contains("custom_assets"),
        "portable package must not recreate obsolete custom_assets directories"
    );
    assert!(
        !script.contains("README"),
        "portable package must not copy README files into the final build"
    );
}
