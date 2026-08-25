use std::path::PathBuf;

/// Get the default configuration directory path.
/// Returns `$XDG_CONFIG_HOME/termcode` (default `~/.config/termcode`) on macOS/Linux,
/// `%APPDATA%\termcode` on Windows. This matches the layout install.sh creates.
pub fn config_dir() -> PathBuf {
    dirs_or_default("termcode")
}

/// Where `install.sh` puts the binary: `~/.local/bin/termcode`, on every
/// platform that script supports. `None` when the home directory is unknown.
///
/// Here rather than next to the update check because it is the same knowledge
/// as [`config_dir`]: what layout the installer creates. The updater compares
/// the running executable against it before handing an install back to the
/// script, so a binary from `cargo install`, a package manager or `target/`
/// is never replaced by one the user did not put there.
pub fn installed_binary_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".local").join("bin").join("termcode"))
}

/// Get runtime directories to search for themes, plugins, and queries.
///
/// Returns directories in priority order (first match wins):
/// 1. `runtime/` next to the binary (portable / development)
/// 2. `~/.config/termcode/runtime/` (installed via install.sh)
/// 3. `~/.config/termcode/` (user config directory, for user overrides)
/// 4. `runtime/` in CWD (fallback)
pub fn runtime_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Next to the binary (portable install / cargo run during development)
    if let Ok(exe) = std::env::current_exe() {
        let exe_runtime = exe.parent().unwrap_or(&exe).join("runtime");
        if exe_runtime.exists() {
            dirs.push(exe_runtime);
        }
    }

    // 2. Installed runtime under user config (~/.config/termcode/runtime/)
    let cfg_runtime = config_dir().join("runtime");
    if cfg_runtime.exists() {
        dirs.push(cfg_runtime);
    }

    // 3. User config directory itself (~/.config/termcode/) for user overrides
    let cfg = config_dir();
    if cfg.exists() {
        dirs.push(cfg);
    }

    // 4. CWD/runtime (fallback for development)
    let cwd_runtime = PathBuf::from("runtime");
    if cwd_runtime.exists() && !dirs.contains(&cwd_runtime) {
        dirs.push(cwd_runtime);
    }

    dirs
}

/// Get the primary runtime directory (first available).
/// Kept for backward compatibility with code that needs a single path.
pub fn runtime_dir() -> PathBuf {
    runtime_dirs()
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("runtime"))
}

fn dirs_or_default(app_name: &str) -> PathBuf {
    // On Unix (including macOS) follow the XDG convention that install.sh uses.
    // `dirs::config_dir()` would resolve to ~/Library/Application Support on macOS,
    // where the installed runtime and user config never live.
    #[cfg(unix)]
    {
        let xdg = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty());
        if let Some(dir) = unix_config_dir(xdg, dirs::home_dir(), app_name) {
            return dir;
        }
    }

    if let Some(config) = dirs::config_dir() {
        config.join(app_name)
    } else {
        PathBuf::from(".").join(format!(".{app_name}"))
    }
}

/// Resolve the XDG-style config directory: `$XDG_CONFIG_HOME/<app>` when set,
/// otherwise `~/.config/<app>`. Returns `None` when the home directory is unknown.
#[cfg(unix)]
fn unix_config_dir(
    xdg_config_home: Option<std::ffi::OsString>,
    home: Option<PathBuf>,
    app_name: &str,
) -> Option<PathBuf> {
    match xdg_config_home {
        Some(xdg) => Some(PathBuf::from(xdg).join(app_name)),
        None => home.map(|home| home.join(".config").join(app_name)),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn config_dir_uses_dot_config_by_default() {
        let dir = unix_config_dir(None, Some(PathBuf::from("/home/u")), "termcode");
        assert_eq!(dir, Some(PathBuf::from("/home/u/.config/termcode")));
    }

    #[test]
    fn config_dir_honors_xdg_config_home() {
        let dir = unix_config_dir(
            Some("/custom/cfg".into()),
            Some(PathBuf::from("/home/u")),
            "termcode",
        );
        assert_eq!(dir, Some(PathBuf::from("/custom/cfg/termcode")));
    }

    #[test]
    fn config_dir_without_home_is_none() {
        assert_eq!(unix_config_dir(None, None, "termcode"), None);
    }

    #[test]
    fn config_dir_is_not_application_support() {
        // Regression: on macOS `dirs::config_dir()` points at
        // ~/Library/Application Support, where install.sh never writes.
        let dir = config_dir();
        assert!(
            !dir.to_string_lossy().contains("Application Support"),
            "got {dir:?}"
        );
    }
}
