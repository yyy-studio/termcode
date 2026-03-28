use std::path::PathBuf;

/// Get the default configuration directory path.
pub fn config_dir() -> PathBuf {
    dirs_or_default("termcode")
}

/// Get the default runtime directory (themes, queries, plugins).
pub fn runtime_dir() -> PathBuf {
    // First check if runtime/ exists relative to the binary
    if let Ok(exe) = std::env::current_exe() {
        let exe_runtime = exe.parent().unwrap_or(&exe).join("runtime");
        if exe_runtime.exists() {
            return exe_runtime;
        }
    }
    // Fall back to CWD/runtime
    PathBuf::from("runtime")
}

fn dirs_or_default(app_name: &str) -> PathBuf {
    if let Some(config) = dirs::config_dir() {
        config.join(app_name)
    } else {
        PathBuf::from(".").join(format!(".{app_name}"))
    }
}
