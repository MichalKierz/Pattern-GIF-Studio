#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = embed_windows_icon() {
        println!("cargo:warning=Windows icon resource was not embedded: {error}");
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn embed_windows_icon() -> std::io::Result<()> {
    use std::{env, fs, path::PathBuf, process::Command};

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir.join("assets").join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon_path.display());

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let rc_path = out_dir.join("pattern-gif-studio.rc");
    let res_path = out_dir.join("pattern-gif-studio.res");
    let escaped_icon_path = icon_path.display().to_string().replace('\\', "\\\\");
    fs::write(&rc_path, format!("1 ICON \"{escaped_icon_path}\"\n"))?;

    let rc = find_rc_exe().unwrap_or_else(|| PathBuf::from("rc.exe"));
    let status = Command::new(&rc)
        .args(["/nologo", "/fo"])
        .arg(&res_path)
        .arg(&rc_path)
        .status()?;

    if status.success() {
        println!(
            "cargo:rustc-link-arg-bin=pattern-gif-studio={}",
            res_path.display()
        );
        Ok(())
    } else {
        Err(std::io::Error::other(format!(
            "{} exited with status {status}",
            rc.display()
        )))
    }
}

#[cfg(target_os = "windows")]
fn find_rc_exe() -> Option<std::path::PathBuf> {
    use std::{env, fs, path::PathBuf};

    if env::var_os("PATH")
        .and_then(|_| which_on_path("rc.exe"))
        .is_some()
    {
        return which_on_path("rc.exe");
    }

    let kits_root = env::var_os("ProgramFiles(x86)")
        .map(PathBuf::from)
        .map(|path| path.join("Windows Kits").join("10").join("bin"))?;
    let mut versions = fs::read_dir(kits_root)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    versions.sort();
    versions.reverse();

    for version in versions {
        for arch in ["x64", "x86"] {
            let candidate = version.join(arch).join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn which_on_path(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|path| path.join(name))
            .find(|candidate| candidate.is_file())
    })
}
