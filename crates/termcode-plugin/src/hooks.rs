use std::collections::HashMap;

use anyhow::Result;
use mlua::{Lua, RegistryKey};

use crate::types::HookContext;

/// All supported hook events that plugins can register for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HookEvent {
    OnOpen {
        path: Option<String>,
        filename: Option<String>,
        language: Option<String>,
    },
    OnSave {
        path: Option<String>,
        filename: Option<String>,
    },
    OnClose {
        path: Option<String>,
        filename: Option<String>,
    },
    OnModeChange {
        old_mode: String,
        new_mode: String,
    },
    OnCursorMove {
        line: usize,
        col: usize,
    },
    OnBufferChange {
        path: Option<String>,
        filename: Option<String>,
    },
    OnTabSwitch {
        path: Option<String>,
        filename: Option<String>,
    },
    OnReady,
}

impl HookEvent {
    /// Returns the hook name string used for registration (e.g., "on_save").
    pub fn name(&self) -> &'static str {
        match self {
            HookEvent::OnOpen { .. } => "on_open",
            HookEvent::OnSave { .. } => "on_save",
            HookEvent::OnClose { .. } => "on_close",
            HookEvent::OnModeChange { .. } => "on_mode_change",
            HookEvent::OnCursorMove { .. } => "on_cursor_move",
            HookEvent::OnBufferChange { .. } => "on_buffer_change",
            HookEvent::OnTabSwitch { .. } => "on_tab_switch",
            HookEvent::OnReady => "on_ready",
        }
    }

    /// All valid hook name strings for validation during registration.
    pub fn all_names() -> &'static [&'static str] {
        &[
            "on_open",
            "on_save",
            "on_close",
            "on_mode_change",
            "on_cursor_move",
            "on_buffer_change",
            "on_tab_switch",
            "on_ready",
        ]
    }

    /// Converts this event into a HookContext for passing to Lua callbacks.
    pub fn to_context(&self) -> HookContext {
        match self {
            HookEvent::OnOpen {
                path,
                filename,
                language,
            } => HookContext {
                path: path.clone(),
                filename: filename.clone(),
                language: language.clone(),
                ..HookContext::empty()
            },
            HookEvent::OnSave { path, filename }
            | HookEvent::OnClose { path, filename }
            | HookEvent::OnBufferChange { path, filename }
            | HookEvent::OnTabSwitch { path, filename } => HookContext {
                path: path.clone(),
                filename: filename.clone(),
                ..HookContext::empty()
            },
            HookEvent::OnModeChange { old_mode, new_mode } => HookContext {
                old_mode: Some(old_mode.clone()),
                new_mode: Some(new_mode.clone()),
                ..HookContext::empty()
            },
            HookEvent::OnCursorMove { line, col } => HookContext {
                line: Some(*line),
                col: Some(*col),
                ..HookContext::empty()
            },
            HookEvent::OnReady => HookContext::empty(),
        }
    }
}

/// Manages hook registrations and dispatching for all plugins.
pub struct HookManager {
    /// Maps hook name -> list of (plugin_name, lua_function_ref) in registration order.
    hooks: HashMap<String, Vec<(String, RegistryKey)>>,
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HookManager {
    pub fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register a hook callback for a plugin. Returns error if hook name is invalid.
    pub fn register_hook(
        &mut self,
        hook_name: &str,
        plugin_name: String,
        callback: RegistryKey,
    ) -> Result<()> {
        if !HookEvent::all_names().contains(&hook_name) {
            anyhow::bail!("unknown hook name: '{}'", hook_name);
        }
        self.hooks
            .entry(hook_name.to_string())
            .or_default()
            .push((plugin_name, callback));
        Ok(())
    }

    /// Dispatch a hook event to all registered listeners.
    ///
    /// Re-entrancy is guarded at the `PluginManager` level, not here.
    /// Individual hook errors are logged but do not stop dispatch to remaining listeners.
    pub fn dispatch_hook(&mut self, lua: &Lua, event: &HookEvent) -> Result<()> {
        let hook_name = event.name();

        let listeners = match self.hooks.get(hook_name) {
            Some(l) if !l.is_empty() => l,
            _ => return Ok(()),
        };

        let context = event.to_context();
        let context_table = context_to_lua_table(lua, &context)?;

        for (plugin_name, key) in listeners {
            let func: mlua::Function = match lua.registry_value(key) {
                Ok(f) => f,
                Err(e) => {
                    log::error!(
                        "Failed to retrieve hook '{}' callback for plugin '{}': {}",
                        hook_name,
                        plugin_name,
                        e
                    );
                    continue;
                }
            };

            if let Err(e) = func.call::<()>(context_table.clone()) {
                log::error!(
                    "Hook '{}' error in plugin '{}': {}",
                    hook_name,
                    plugin_name,
                    e
                );
            }
        }

        Ok(())
    }
}

/// Convert a HookContext into a Lua table.
fn context_to_lua_table(lua: &Lua, ctx: &HookContext) -> Result<mlua::Table> {
    let table = lua.create_table().map_err(|e| anyhow::anyhow!("{}", e))?;

    if let Some(ref path) = ctx.path {
        table
            .set("path", path.as_str())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(ref filename) = ctx.filename {
        table
            .set("filename", filename.as_str())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(ref language) = ctx.language {
        table
            .set("language", language.as_str())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(ref old_mode) = ctx.old_mode {
        table
            .set("old_mode", old_mode.as_str())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(ref new_mode) = ctx.new_mode {
        table
            .set("new_mode", new_mode.as_str())
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(line) = ctx.line {
        table
            .set("line", line)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }
    if let Some(col) = ctx.col {
        table
            .set("col", col)
            .map_err(|e| anyhow::anyhow!("{}", e))?;
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_lua() -> Lua {
        Lua::new()
    }

    #[test]
    fn test_hook_event_names() {
        assert_eq!(
            HookEvent::OnOpen {
                path: None,
                filename: None,
                language: None
            }
            .name(),
            "on_open"
        );
        assert_eq!(
            HookEvent::OnSave {
                path: None,
                filename: None
            }
            .name(),
            "on_save"
        );
        assert_eq!(
            HookEvent::OnClose {
                path: None,
                filename: None
            }
            .name(),
            "on_close"
        );
        assert_eq!(
            HookEvent::OnModeChange {
                old_mode: "Normal".into(),
                new_mode: "Insert".into()
            }
            .name(),
            "on_mode_change"
        );
        assert_eq!(
            HookEvent::OnCursorMove { line: 0, col: 0 }.name(),
            "on_cursor_move"
        );
        assert_eq!(
            HookEvent::OnBufferChange {
                path: None,
                filename: None
            }
            .name(),
            "on_buffer_change"
        );
        assert_eq!(
            HookEvent::OnTabSwitch {
                path: None,
                filename: None
            }
            .name(),
            "on_tab_switch"
        );
        assert_eq!(HookEvent::OnReady.name(), "on_ready");
    }

    #[test]
    fn test_all_names_complete() {
        let names = HookEvent::all_names();
        assert_eq!(names.len(), 8);
        assert!(names.contains(&"on_open"));
        assert!(names.contains(&"on_save"));
        assert!(names.contains(&"on_close"));
        assert!(names.contains(&"on_mode_change"));
        assert!(names.contains(&"on_cursor_move"));
        assert!(names.contains(&"on_buffer_change"));
        assert!(names.contains(&"on_tab_switch"));
        assert!(names.contains(&"on_ready"));
    }

    #[test]
    fn test_register_valid_hook() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        let func = lua
            .load("function(ctx) end")
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();

        let result = manager.register_hook("on_save", "test-plugin".into(), key);
        assert!(result.is_ok());
    }

    #[test]
    fn test_register_invalid_hook_name() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        let func = lua
            .load("function(ctx) end")
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();

        let result = manager.register_hook("on_invalid", "test-plugin".into(), key);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("unknown hook name")
        );
    }

    #[test]
    fn test_dispatch_calls_registered_hooks() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load(
            r#"
            _hook_called = false
            _hook_path = nil
        "#,
        )
        .exec()
        .unwrap();

        let func = lua
            .load(
                r#"
            function(ctx)
                _hook_called = true
                _hook_path = ctx.path
            end
        "#,
            )
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();

        manager
            .register_hook("on_save", "test-plugin".into(), key)
            .unwrap();

        let event = HookEvent::OnSave {
            path: Some("/tmp/test.rs".into()),
            filename: Some("test.rs".into()),
        };
        manager.dispatch_hook(&lua, &event).unwrap();

        let called: bool = lua.globals().get("_hook_called").unwrap();
        assert!(called);
        let path: String = lua.globals().get("_hook_path").unwrap();
        assert_eq!(path, "/tmp/test.rs");
    }

    #[test]
    fn test_dispatch_order_is_registration_order() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load("_order = {}").exec().unwrap();

        for name in &["plugin-a", "plugin-b", "plugin-c"] {
            let func = lua
                .load(&format!(
                    r#"function(ctx) table.insert(_order, "{}") end"#,
                    name
                ))
                .eval::<mlua::Function>()
                .unwrap();
            let key = lua.create_registry_value(func).unwrap();
            manager
                .register_hook("on_ready", (*name).to_string(), key)
                .unwrap();
        }

        manager.dispatch_hook(&lua, &HookEvent::OnReady).unwrap();

        let order: Vec<String> = lua
            .load("return _order")
            .eval::<mlua::Table>()
            .unwrap()
            .sequence_values::<String>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(order, vec!["plugin-a", "plugin-b", "plugin-c"]);
    }

    #[test]
    fn test_hook_error_does_not_stop_others() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load("_second_called = false").exec().unwrap();

        let bad_func = lua
            .load(r#"function(ctx) error("intentional error") end"#)
            .eval::<mlua::Function>()
            .unwrap();
        let bad_key = lua.create_registry_value(bad_func).unwrap();
        manager
            .register_hook("on_ready", "bad-plugin".into(), bad_key)
            .unwrap();

        let good_func = lua
            .load("function(ctx) _second_called = true end")
            .eval::<mlua::Function>()
            .unwrap();
        let good_key = lua.create_registry_value(good_func).unwrap();
        manager
            .register_hook("on_ready", "good-plugin".into(), good_key)
            .unwrap();

        manager.dispatch_hook(&lua, &HookEvent::OnReady).unwrap();

        let called: bool = lua.globals().get("_second_called").unwrap();
        assert!(called, "Second hook should still fire after first errors");
    }

    #[test]
    fn test_context_fields_on_open() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load("_ctx = {}").exec().unwrap();
        let func = lua
            .load("function(ctx) _ctx = ctx end")
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();
        manager
            .register_hook("on_open", "test".into(), key)
            .unwrap();

        let event = HookEvent::OnOpen {
            path: Some("/tmp/main.rs".into()),
            filename: Some("main.rs".into()),
            language: Some("rust".into()),
        };
        manager.dispatch_hook(&lua, &event).unwrap();

        let ctx: mlua::Table = lua.globals().get("_ctx").unwrap();
        assert_eq!(ctx.get::<String>("path").unwrap(), "/tmp/main.rs");
        assert_eq!(ctx.get::<String>("filename").unwrap(), "main.rs");
        assert_eq!(ctx.get::<String>("language").unwrap(), "rust");
    }

    #[test]
    fn test_context_fields_on_mode_change() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load("_ctx = {}").exec().unwrap();
        let func = lua
            .load("function(ctx) _ctx = ctx end")
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();
        manager
            .register_hook("on_mode_change", "test".into(), key)
            .unwrap();

        let event = HookEvent::OnModeChange {
            old_mode: "Normal".into(),
            new_mode: "Insert".into(),
        };
        manager.dispatch_hook(&lua, &event).unwrap();

        let ctx: mlua::Table = lua.globals().get("_ctx").unwrap();
        assert_eq!(ctx.get::<String>("old_mode").unwrap(), "Normal");
        assert_eq!(ctx.get::<String>("new_mode").unwrap(), "Insert");
    }

    #[test]
    fn test_context_fields_on_cursor_move() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load("_ctx = {}").exec().unwrap();
        let func = lua
            .load("function(ctx) _ctx = ctx end")
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();
        manager
            .register_hook("on_cursor_move", "test".into(), key)
            .unwrap();

        let event = HookEvent::OnCursorMove { line: 10, col: 5 };
        manager.dispatch_hook(&lua, &event).unwrap();

        let ctx: mlua::Table = lua.globals().get("_ctx").unwrap();
        assert_eq!(ctx.get::<usize>("line").unwrap(), 10);
        assert_eq!(ctx.get::<usize>("col").unwrap(), 5);
    }

    #[test]
    fn test_dispatch_no_listeners() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        let result = manager.dispatch_hook(&lua, &HookEvent::OnReady);
        assert!(result.is_ok());
    }

    #[test]
    fn test_context_nil_fields_for_unsaved() {
        let lua = create_test_lua();
        let mut manager = HookManager::new();

        lua.load("_has_path = true").exec().unwrap();
        let func = lua
            .load("function(ctx) _has_path = (ctx.path ~= nil) end")
            .eval::<mlua::Function>()
            .unwrap();
        let key = lua.create_registry_value(func).unwrap();
        manager
            .register_hook("on_save", "test".into(), key)
            .unwrap();

        let event = HookEvent::OnSave {
            path: None,
            filename: None,
        };
        manager.dispatch_hook(&lua, &event).unwrap();

        let has_path: bool = lua.globals().get("_has_path").unwrap();
        assert!(!has_path, "path should be nil for unsaved buffers");
    }
}
