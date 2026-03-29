use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use mlua::{Lua, RegistryKey};

use termcode_config::config::PluginConfig;
use termcode_view::editor::Editor;

use crate::api::{register_editor_api, register_log_api, with_scoped_api};
use crate::hooks::{HookEvent, HookManager};
use crate::sandbox::{create_lua_vm, create_plugin_require};
use crate::types::{DeferredAction, PluginInfo, PluginStatus, is_valid_name};

/// Metadata parsed from a plugin's `plugin.toml` manifest.
#[derive(Debug, serde::Deserialize)]
struct PluginManifest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
}

fn default_version() -> String {
    "0.1.0".to_string()
}

/// Central coordinator for the plugin system.
///
/// Owns the Lua VM, plugin metadata, registered commands, and hook manager.
/// Provides methods for plugin discovery, loading, command execution, and hook dispatch.
pub struct PluginManager {
    lua: Lua,
    plugins: Vec<PluginInfo>,
    commands: HashMap<String, (RegistryKey, String)>,
    hooks: HookManager,
    config: PluginConfig,
    is_dispatching: bool,
}

impl PluginManager {
    /// Creates a new PluginManager with a sandboxed Lua VM.
    pub fn new(config: PluginConfig) -> Result<Self> {
        let lua = create_lua_vm(&config)?;
        register_editor_api(&lua)?;
        register_log_api(&lua)?;

        Ok(Self {
            lua,
            plugins: Vec::new(),
            commands: HashMap::new(),
            hooks: HookManager::new(),
            config,
            is_dispatching: false,
        })
    }

    /// Scans plugin directories and loads all discovered plugins.
    ///
    /// Directories are scanned in order; within each directory, plugins are loaded
    /// alphabetically. If a plugin name duplicates one already loaded, the later
    /// one takes precedence (the earlier is marked as skipped).
    pub fn load_plugins(&mut self, dirs: &[PathBuf]) {
        let mut seen_names: HashMap<String, usize> = HashMap::new();

        for dir in dirs {
            if !dir.is_dir() {
                log::debug!("Plugin directory does not exist: {}", dir.display());
                continue;
            }

            let mut entries: Vec<PathBuf> = match std::fs::read_dir(dir) {
                Ok(rd) => rd
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect(),
                Err(e) => {
                    log::warn!("Failed to read plugin directory {}: {}", dir.display(), e);
                    continue;
                }
            };
            entries.sort();

            for plugin_path in entries {
                let info = match self.load_plugin(&plugin_path) {
                    Ok(info) => info,
                    Err(e) => {
                        let name = plugin_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        log::error!("Failed to load plugin '{}': {}", name, e);
                        PluginInfo {
                            name,
                            version: String::new(),
                            description: String::new(),
                            author: String::new(),
                            path: plugin_path,
                            status: PluginStatus::Failed(format!("{}", e)),
                        }
                    }
                };

                if let Some(&prev_idx) = seen_names.get(&info.name) {
                    log::info!(
                        "Plugin '{}' from {} overrides previous from {}",
                        info.name,
                        info.path.display(),
                        self.plugins[prev_idx].path.display()
                    );
                    self.plugins[prev_idx].status =
                        PluginStatus::Failed("overridden by later plugin".to_string());
                }

                let idx = self.plugins.len();
                seen_names.insert(info.name.clone(), idx);
                self.plugins.push(info);
            }
        }
    }

    /// Loads a single plugin from the given directory path.
    ///
    /// Parses `plugin.toml` (or derives defaults from directory name), validates
    /// the plugin name, checks per-plugin overrides, sets up the plugin namespace,
    /// and executes `init.lua`.
    fn load_plugin(&mut self, path: &Path) -> Result<PluginInfo> {
        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let manifest = self.parse_manifest(path, &dir_name)?;
        let plugin_name = manifest.name.unwrap_or_else(|| dir_name.clone());

        if !is_valid_name(&plugin_name) {
            anyhow::bail!(
                "invalid plugin name '{}': must match [a-z0-9_-]+",
                plugin_name
            );
        }

        if let Some(ovr) = self.config.overrides.get(&plugin_name) {
            if ovr.enabled == Some(false) {
                return Ok(PluginInfo {
                    name: plugin_name,
                    version: manifest.version,
                    description: manifest.description,
                    author: manifest.author,
                    path: path.to_path_buf(),
                    status: PluginStatus::Disabled,
                });
            }
        }

        let init_path = path.join("init.lua");
        if !init_path.exists() {
            anyhow::bail!("init.lua not found in {}", path.display());
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.execute_plugin_init(path, &plugin_name, &init_path)
        }));

        match result {
            Ok(Ok(())) => Ok(PluginInfo {
                name: plugin_name,
                version: manifest.version,
                description: manifest.description,
                author: manifest.author,
                path: path.to_path_buf(),
                status: PluginStatus::Loaded,
            }),
            Ok(Err(e)) => Ok(PluginInfo {
                name: plugin_name,
                version: manifest.version,
                description: manifest.description,
                author: manifest.author,
                path: path.to_path_buf(),
                status: PluginStatus::Failed(format!("{}", e)),
            }),
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                log::error!(
                    "Plugin '{}' panicked during init: {}",
                    plugin_name,
                    panic_msg
                );
                Ok(PluginInfo {
                    name: plugin_name,
                    version: manifest.version,
                    description: manifest.description,
                    author: manifest.author,
                    path: path.to_path_buf(),
                    status: PluginStatus::Failed(format!("panic: {}", panic_msg)),
                })
            }
        }
    }

    /// Parses `plugin.toml` or returns defaults derived from the directory name.
    fn parse_manifest(&self, path: &Path, dir_name: &str) -> Result<PluginManifest> {
        let toml_path = path.join("plugin.toml");
        if toml_path.exists() {
            let content = std::fs::read_to_string(&toml_path)?;
            let mut manifest: PluginManifest = toml::from_str(&content)?;
            if manifest.name.is_none() {
                manifest.name = Some(dir_name.to_string());
            }
            Ok(manifest)
        } else {
            Ok(PluginManifest {
                name: Some(dir_name.to_string()),
                version: default_version(),
                description: String::new(),
                author: String::new(),
            })
        }
    }

    /// Sets up the plugin namespace and executes init.lua.
    fn execute_plugin_init(
        &mut self,
        plugin_dir: &Path,
        plugin_name: &str,
        init_path: &Path,
    ) -> Result<()> {
        let lua = &self.lua;

        lua.globals()
            .set("_current_plugin_name", plugin_name.to_string())
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let plugin_table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
        plugin_table
            .set("name", plugin_name.to_string())
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let plugin_config_table = self.build_plugin_config_table(lua, plugin_name)?;
        plugin_table
            .set("config", plugin_config_table)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let require_fn = create_plugin_require(lua, plugin_dir)?;
        plugin_table
            .set("require", require_fn)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // plugin.register_command(name, description, callback)
        let registration_flag = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
        registration_flag
            .set("active", true)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let cmd_plugin_name = plugin_name.to_string();
        let reg_flag_clone = registration_flag.clone();
        let register_command_fn = lua
            .create_function(
                move |lua, (name, description, callback): (String, String, mlua::Function)| {
                    let active: bool = reg_flag_clone.get("active").unwrap_or(false);
                    if !active {
                        return Err(mlua::Error::RuntimeError(
                            "register_command can only be called during plugin init".to_string(),
                        ));
                    }
                    if !is_valid_name(&name) {
                        return Err(mlua::Error::RuntimeError(format!(
                            "invalid command name '{}': must match [a-z0-9_-]+",
                            name
                        )));
                    }
                    let full_id = format!("plugin.{}.{}", cmd_plugin_name, name);

                    let commands_table: mlua::Table = lua.globals().get("_plugin_commands")?;
                    let entry = lua.create_table()?;
                    entry.set("id", full_id.clone())?;
                    entry.set(
                        "description",
                        format!("[{}] {}", cmd_plugin_name, description),
                    )?;
                    let key = lua.create_registry_value(callback)?;
                    entry.set("registry_idx", lua.create_string(full_id.as_bytes())?)?;
                    lua.set_named_registry_value(
                        &full_id,
                        lua.registry_value::<mlua::Function>(&key)?,
                    )?;
                    lua.remove_registry_value(key)?;
                    let len: i64 = commands_table.len()?;
                    commands_table.set(len + 1, entry)?;
                    Ok(())
                },
            )
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        plugin_table
            .set("register_command", register_command_fn)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // plugin.on(hook_name, callback)
        let hook_plugin_name = plugin_name.to_string();
        let reg_flag_clone2 = registration_flag.clone();
        let on_fn = lua
            .create_function(
                move |lua, (hook_name, callback): (String, mlua::Function)| {
                    let active: bool = reg_flag_clone2.get("active").unwrap_or(false);
                    if !active {
                        return Err(mlua::Error::RuntimeError(
                            "plugin.on() can only be called during plugin init".to_string(),
                        ));
                    }

                    if !HookEvent::all_names().contains(&hook_name.as_str()) {
                        return Err(mlua::Error::RuntimeError(format!(
                            "unknown hook name: '{}'",
                            hook_name
                        )));
                    }

                    let hooks_table: mlua::Table = lua.globals().get("_plugin_hooks")?;
                    let entry = lua.create_table()?;
                    let hook_key = format!(
                        "_hook_{}_{}_{}",
                        hook_plugin_name,
                        hook_name,
                        hooks_table.len()?
                    );
                    entry.set("hook_name", hook_name)?;
                    entry.set("plugin_name", hook_plugin_name.clone())?;
                    entry.set("hook_key", hook_key.clone())?;
                    lua.set_named_registry_value(&hook_key, callback)?;
                    let len: i64 = hooks_table.len()?;
                    hooks_table.set(len + 1, entry)?;
                    Ok(())
                },
            )
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        plugin_table
            .set("on", on_fn)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        lua.globals()
            .set("plugin", plugin_table)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Temporary tables for collecting registrations during init
        let commands_table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
        lua.globals()
            .set("_plugin_commands", commands_table)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let hooks_table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
        lua.globals()
            .set("_plugin_hooks", hooks_table)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let source = std::fs::read_to_string(init_path)?;
        self.lua
            .load(&source)
            .set_name(format!("={}/init.lua", plugin_name))
            .exec()
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Disable registration after init completes
        registration_flag
            .set("active", false)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Collect registered commands
        let commands_table: mlua::Table = self
            .lua
            .globals()
            .get("_plugin_commands")
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        for pair in commands_table.pairs::<i64, mlua::Table>() {
            let (_, entry) = pair.map_err(|e| anyhow::anyhow!("{}", e))?;
            let full_id: String = entry.get("id").map_err(|e| anyhow::anyhow!("{}", e))?;
            let description: String = entry
                .get("description")
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let func: mlua::Function = self
                .lua
                .named_registry_value(&full_id)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let key = self
                .lua
                .create_registry_value(func)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            self.commands.insert(full_id, (key, description));
        }

        // Collect registered hooks
        let hooks_table: mlua::Table = self
            .lua
            .globals()
            .get("_plugin_hooks")
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        for pair in hooks_table.pairs::<i64, mlua::Table>() {
            let (_, entry) = pair.map_err(|e| anyhow::anyhow!("{}", e))?;
            let hook_name: String = entry
                .get("hook_name")
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let hook_plugin_name: String = entry
                .get("plugin_name")
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let hook_key: String = entry
                .get("hook_key")
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let func: mlua::Function = self
                .lua
                .named_registry_value(&hook_key)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let callback_key = self
                .lua
                .create_registry_value(func)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            self.hooks
                .register_hook(&hook_name, hook_plugin_name, callback_key)?;
        }

        // Clean up temporary globals
        self.lua
            .globals()
            .set("_plugin_commands", mlua::Value::Nil)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        self.lua
            .globals()
            .set("_plugin_hooks", mlua::Value::Nil)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        self.lua
            .globals()
            .set("plugin", mlua::Value::Nil)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(())
    }

    /// Builds a Lua table with per-plugin configuration overrides.
    fn build_plugin_config_table(&self, lua: &Lua, plugin_name: &str) -> Result<mlua::Table> {
        let table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;

        if let Some(ovr) = self.config.overrides.get(plugin_name) {
            for (key, value) in &ovr.config {
                let lua_val = toml_value_to_lua(lua, value)?;
                table
                    .set(key.as_str(), lua_val)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
        }

        Ok(table)
    }

    /// Executes a plugin command by its full ID (e.g., "plugin.my-plugin.cmd-name").
    ///
    /// Resets buffer_mutated and deferred_actions, sets scoped API, calls the
    /// stored Lua function, and returns execution results.
    pub fn execute_command(
        &mut self,
        name: &str,
        editor: &mut Editor,
    ) -> Result<(bool, Vec<DeferredAction>)> {
        let (key, _desc) = self
            .commands
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("unknown plugin command: {}", name))?;

        let func: mlua::Function = self
            .lua
            .registry_value(key)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let plugin_name = name
            .strip_prefix("plugin.")
            .and_then(|rest| rest.split('.').next())
            .unwrap_or("unknown");

        self.lua
            .globals()
            .set("_current_plugin_name", plugin_name.to_string())
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_api(editor, || {
                func.call::<()>(()).map_err(|e| anyhow::anyhow!("{}", e))
            })
        }));

        match result {
            Ok(Ok((_, mutated, actions))) => Ok((mutated, actions)),
            Ok(Err(e)) => {
                log::error!("Plugin command '{}' error: {}", name, e);
                Err(e)
            }
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                log::error!("Plugin command '{}' panicked: {}", name, panic_msg);
                Err(anyhow::anyhow!("plugin command panicked: {}", panic_msg))
            }
        }
    }

    /// Dispatches a hook event to all registered listeners.
    ///
    /// Sets scoped API for each listener, catches per-hook errors, and sets
    /// status bar warning on error.
    pub fn dispatch_hook(
        &mut self,
        hook: HookEvent,
        editor: &mut Editor,
    ) -> Result<(bool, Vec<DeferredAction>)> {
        if self.is_dispatching {
            log::warn!("Skipped re-entrant hook dispatch for '{}'", hook.name());
            return Ok((false, Vec::new()));
        }

        self.is_dispatching = true;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_scoped_api(editor, || {
                self.hooks
                    .dispatch_hook(&self.lua, &hook)
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })
        }));

        self.is_dispatching = false;

        match result {
            Ok(Ok((_, mutated, actions))) => Ok((mutated, actions)),
            Ok(Err(e)) => {
                log::error!("Hook '{}' dispatch error: {}", hook.name(), e);
                Err(e)
            }
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else {
                    "unknown panic".to_string()
                };
                log::error!("Hook '{}' panicked: {}", hook.name(), panic_msg);
                Err(anyhow::anyhow!("hook dispatch panicked: {}", panic_msg))
            }
        }
    }

    /// Returns a list of all registered plugin commands as (full_id, description) pairs.
    pub fn list_commands(&self) -> Vec<(String, String)> {
        self.commands
            .iter()
            .map(|(id, (_key, desc))| (id.clone(), desc.clone()))
            .collect()
    }

    /// Returns a list of all loaded plugin info.
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.clone()
    }

    /// Returns true if there are any registered commands matching the prefix.
    pub fn has_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }
}

/// Expands a tilde `~` prefix to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Converts a TOML Value to a Lua value.
fn toml_value_to_lua(lua: &Lua, value: &toml::Value) -> Result<mlua::Value> {
    match value {
        toml::Value::String(s) => lua
            .create_string(s.as_str())
            .map(mlua::Value::String)
            .map_err(|e| anyhow::anyhow!("{}", e)),
        toml::Value::Integer(i) => Ok(mlua::Value::Integer(*i)),
        toml::Value::Float(f) => Ok(mlua::Value::Number(*f)),
        toml::Value::Boolean(b) => Ok(mlua::Value::Boolean(*b)),
        toml::Value::Array(arr) => {
            let table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
            for (i, v) in arr.iter().enumerate() {
                let lua_val = toml_value_to_lua(lua, v)?;
                table
                    .set((i + 1) as i64, lua_val)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
            Ok(mlua::Value::Table(table))
        }
        toml::Value::Table(tbl) => {
            let table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;
            for (k, v) in tbl {
                let lua_val = toml_value_to_lua(lua, v)?;
                table
                    .set(k.as_str(), lua_val)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            }
            Ok(mlua::Value::Table(table))
        }
        toml::Value::Datetime(dt) => lua
            .create_string(dt.to_string().as_str())
            .map(mlua::Value::String)
            .map_err(|e| anyhow::anyhow!("{}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use termcode_config::config::PluginOverride;

    fn default_config() -> PluginConfig {
        PluginConfig {
            enabled: true,
            plugin_dirs: Vec::new(),
            instruction_limit: 1_000_000,
            memory_limit_mb: 10,
            overrides: HashMap::new(),
        }
    }

    fn create_test_editor() -> Editor {
        use termcode_core::config_types::EditorConfig;
        use termcode_syntax::language::LanguageRegistry;
        use termcode_theme::theme::Theme;

        let theme = Theme::default();
        let config = EditorConfig::default();
        let lang = LanguageRegistry::new();
        Editor::new(theme, config, lang, None)
    }

    fn create_plugin_dir(base: &Path, name: &str, init_lua: &str) -> PathBuf {
        let dir = base.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("init.lua"), init_lua).unwrap();
        dir
    }

    fn create_plugin_dir_with_toml(
        base: &Path,
        name: &str,
        toml_content: &str,
        init_lua: &str,
    ) -> PathBuf {
        let dir = base.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.toml"), toml_content).unwrap();
        fs::write(dir.join("init.lua"), init_lua).unwrap();
        dir
    }

    #[test]
    fn test_new_creates_manager() {
        let config = default_config();
        let manager = PluginManager::new(config).unwrap();
        assert!(manager.plugins.is_empty());
        assert!(manager.commands.is_empty());
    }

    #[test]
    fn test_load_plugins_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);
        assert!(manager.plugins.is_empty());
    }

    #[test]
    fn test_load_plugins_nonexistent_dir() {
        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[PathBuf::from("/nonexistent/path")]);
        assert!(manager.plugins.is_empty());
    }

    #[test]
    fn test_load_simple_plugin() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "hello",
            r#"
            -- simple plugin that does nothing
        "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert_eq!(manager.plugins[0].name, "hello");
        assert!(matches!(manager.plugins[0].status, PluginStatus::Loaded));
    }

    #[test]
    fn test_load_plugin_with_toml() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir_with_toml(
            tmp.path(),
            "my-plugin",
            r#"
            name = "my-plugin"
            version = "1.0.0"
            description = "A test plugin"
            author = "tester"
            "#,
            "-- init",
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert_eq!(manager.plugins[0].name, "my-plugin");
        assert_eq!(manager.plugins[0].version, "1.0.0");
        assert_eq!(manager.plugins[0].description, "A test plugin");
        assert_eq!(manager.plugins[0].author, "tester");
    }

    #[test]
    fn test_load_plugin_missing_init() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("no-init");
        fs::create_dir_all(&dir).unwrap();

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert!(matches!(manager.plugins[0].status, PluginStatus::Failed(_)));
    }

    #[test]
    fn test_load_plugin_syntax_error() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(tmp.path(), "bad-syntax", "this is not valid lua {{{}}}");

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert!(matches!(manager.plugins[0].status, PluginStatus::Failed(_)));
    }

    #[test]
    fn test_load_plugin_runtime_error() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(tmp.path(), "bad-runtime", r#"error("init failure")"#);

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert!(matches!(manager.plugins[0].status, PluginStatus::Failed(_)));
    }

    #[test]
    fn test_load_plugin_invalid_name() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir_with_toml(
            tmp.path(),
            "Bad_Plugin",
            r#"name = "Bad Plugin""#,
            "-- init",
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert!(matches!(manager.plugins[0].status, PluginStatus::Failed(_)));
    }

    #[test]
    fn test_load_plugin_disabled_override() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(tmp.path(), "disabled-plugin", "-- init");

        let mut overrides = HashMap::new();
        overrides.insert(
            "disabled-plugin".to_string(),
            PluginOverride {
                enabled: Some(false),
                config: HashMap::new(),
            },
        );

        let config = PluginConfig {
            enabled: true,
            overrides,
            ..default_config()
        };
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 1);
        assert!(matches!(manager.plugins[0].status, PluginStatus::Disabled));
    }

    #[test]
    fn test_plugin_command_registration() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "cmd-test",
            r#"
            plugin.register_command("greet", "Say hello", function()
                editor.set_status("Hello from plugin!")
            end)
            "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        assert!(manager.has_command("plugin.cmd-test.greet"));
        let cmds = manager.list_commands();
        assert_eq!(cmds.len(), 1);
    }

    #[test]
    fn test_plugin_command_execution() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "exec-test",
            r#"
            plugin.register_command("hello", "Set status", function()
                editor.set_status("executed!")
            end)
            "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let mut editor = create_test_editor();
        let (mutated, actions) = manager
            .execute_command("plugin.exec-test.hello", &mut editor)
            .unwrap();

        assert!(!mutated);
        assert!(actions.is_empty());
        assert_eq!(editor.status_message.as_deref(), Some("executed!"));
    }

    #[test]
    fn test_plugin_hook_registration_and_dispatch() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "hook-test",
            r#"
            plugin.on("on_ready", function(ctx)
                -- on_ready hook fires
            end)
            "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let mut editor = create_test_editor();
        let result = manager.dispatch_hook(HookEvent::OnReady, &mut editor);
        assert!(result.is_ok());
    }

    #[test]
    fn test_plugin_hook_with_context() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "hook-ctx",
            r#"
            plugin.on("on_save", function(ctx)
                if ctx.filename then
                    editor.set_status("saved: " .. ctx.filename)
                end
            end)
            "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let mut editor = create_test_editor();
        manager
            .dispatch_hook(
                HookEvent::OnSave {
                    path: Some("/tmp/test.rs".to_string()),
                    filename: Some("test.rs".to_string()),
                },
                &mut editor,
            )
            .unwrap();

        assert_eq!(editor.status_message.as_deref(), Some("saved: test.rs"));
    }

    #[test]
    fn test_duplicate_plugin_name_later_wins() {
        let tmp1 = tempfile::tempdir().unwrap();
        let tmp2 = tempfile::tempdir().unwrap();

        create_plugin_dir(
            tmp1.path(),
            "dup",
            r#"
            plugin.register_command("cmd", "First", function()
                editor.set_status("first")
            end)
            "#,
        );
        create_plugin_dir(
            tmp2.path(),
            "dup",
            r#"
            plugin.register_command("cmd", "Second", function()
                editor.set_status("second")
            end)
            "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp1.path().to_path_buf(), tmp2.path().to_path_buf()]);

        assert_eq!(manager.plugins.len(), 2);
        // First one should be overridden
        assert!(matches!(manager.plugins[0].status, PluginStatus::Failed(_)));

        let mut editor = create_test_editor();
        manager
            .execute_command("plugin.dup.cmd", &mut editor)
            .unwrap();
        assert_eq!(editor.status_message.as_deref(), Some("second"));
    }

    #[test]
    fn test_registration_outside_init_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "reg-after",
            r#"
            plugin.register_command("early", "Works during init", function()
                -- Try to register a command during execution (should fail)
                plugin.register_command("late", "Should fail", function() end)
            end)
            "#,
        );

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let mut editor = create_test_editor();
        let result = manager.execute_command("plugin.reg-after.early", &mut editor);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_command_error() {
        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();

        let mut editor = create_test_editor();
        let result = manager.execute_command("plugin.nonexistent.cmd", &mut editor);
        assert!(result.is_err());
    }

    #[test]
    fn test_list_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(tmp.path(), "alpha", "-- init");
        create_plugin_dir(tmp.path(), "beta", "-- init");

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let plugins = manager.list_plugins();
        assert_eq!(plugins.len(), 2);
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/some/path");
        assert!(!expanded.to_string_lossy().starts_with('~'));

        let no_tilde = expand_tilde("/absolute/path");
        assert_eq!(no_tilde, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_plugin_config_override() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(
            tmp.path(),
            "cfg-test",
            r#"
            local greeting = plugin.config.greeting or "default"
            plugin.register_command("greet", "Show greeting", function()
                editor.set_status(greeting)
            end)
            "#,
        );

        let mut overrides = HashMap::new();
        let mut plugin_cfg = HashMap::new();
        plugin_cfg.insert(
            "greeting".to_string(),
            toml::Value::String("custom hello".to_string()),
        );
        overrides.insert(
            "cfg-test".to_string(),
            PluginOverride {
                enabled: None,
                config: plugin_cfg,
            },
        );

        let config = PluginConfig {
            enabled: true,
            overrides,
            ..default_config()
        };
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let mut editor = create_test_editor();
        manager
            .execute_command("plugin.cfg-test.greet", &mut editor)
            .unwrap();
        assert_eq!(editor.status_message.as_deref(), Some("custom hello"));
    }

    #[test]
    fn test_alphabetical_load_order() {
        let tmp = tempfile::tempdir().unwrap();
        create_plugin_dir(tmp.path(), "charlie", "-- init");
        create_plugin_dir(tmp.path(), "alpha", "-- init");
        create_plugin_dir(tmp.path(), "bravo", "-- init");

        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.load_plugins(&[tmp.path().to_path_buf()]);

        let names: Vec<&str> = manager.plugins.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn test_reentrancy_guard() {
        let config = default_config();
        let mut manager = PluginManager::new(config).unwrap();
        manager.is_dispatching = true;

        let mut editor = create_test_editor();
        let (mutated, actions) = manager
            .dispatch_hook(HookEvent::OnReady, &mut editor)
            .unwrap();
        assert!(!mutated);
        assert!(actions.is_empty());

        manager.is_dispatching = false;
    }
}
