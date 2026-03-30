use std::path::PathBuf;

/// Get the default configuration directory path.
/// Returns `~/.config/termcode` on macOS/Linux, `%APPDATA%\termcode` on Windows.
pub fn config_dir() -> PathBuf {
    dirs_or_default("termcode")
}

/// Get runtime directories to search for themes, plugins, and queries.
///
/// Returns directories in priority order (first match wins):
/// 1. `runtime/` next to the binary (portable / development)
/// 2. `~/.config/termcode/` (user config directory)
/// 3. `runtime/` in CWD (fallback)
pub fn runtime_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Next to the binary (portable install / cargo run during development)
    if let Ok(exe) = std::env::current_exe() {
        let exe_runtime = exe.parent().unwrap_or(&exe).join("runtime");
        if exe_runtime.exists() {
            dirs.push(exe_runtime);
        }
    }

    // 2. User config directory (~/.config/termcode/)
    let cfg = config_dir();
    if cfg.exists() {
        dirs.push(cfg);
    }

    // 3. CWD/runtime (fallback for development)
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
    if let Some(config) = dirs::config_dir() {
        config.join(app_name)
    } else {
        PathBuf::from(".").join(format!(".{app_name}"))
    }
}
