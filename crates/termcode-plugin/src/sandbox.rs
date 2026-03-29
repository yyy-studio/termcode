use std::path::{Path, PathBuf};

use mlua::{HookTriggers, Lua, LuaOptions, StdLib};

use termcode_config::config::PluginConfig;

/// Converts an mlua::Error into an anyhow::Error by formatting its Display output.
fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{}", e)
}

/// Creates a sandboxed Lua VM with restricted standard libraries and resource limits.
///
/// The VM includes: base globals, string, table, math, utf8, and a restricted os table.
/// Dangerous globals (loadfile, dofile, require) are removed after creation.
pub fn create_lua_vm(config: &PluginConfig) -> anyhow::Result<Lua> {
    // Note: Base library globals (print, type, tostring, pairs, etc.) are always
    // loaded by mlua's Lua::new_with (via luaopen_base). No StdLib flag needed.
    let libs = StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8 | StdLib::OS;
    let lua = Lua::new_with(libs, LuaOptions::default()).map_err(lua_err)?;

    remove_dangerous_globals(&lua)?;
    restrict_os_table(&lua)?;
    set_resource_limits(&lua, config)?;

    Ok(lua)
}

/// Removes globals that could escape the sandbox: loadfile, dofile, require.
fn remove_dangerous_globals(lua: &Lua) -> anyhow::Result<()> {
    let globals = lua.globals();
    globals.set("loadfile", mlua::Value::Nil).map_err(lua_err)?;
    globals.set("dofile", mlua::Value::Nil).map_err(lua_err)?;
    globals.set("require", mlua::Value::Nil).map_err(lua_err)?;
    Ok(())
}

/// Restricts the `os` table to only safe functions: clock, time, date.
fn restrict_os_table(lua: &Lua) -> anyhow::Result<()> {
    lua.load(
        r#"
        local safe_os = {
            clock = os.clock,
            time = os.time,
            date = os.date,
        }
        os = safe_os
        "#,
    )
    .exec()
    .map_err(lua_err)?;
    Ok(())
}

/// Sets instruction count limit and memory limit on the Lua VM.
fn set_resource_limits(lua: &Lua, config: &PluginConfig) -> anyhow::Result<()> {
    let instruction_limit = config.instruction_limit;
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(instruction_limit),
        move |_lua, _debug| {
            Err(mlua::Error::RuntimeError(
                "instruction limit exceeded".to_string(),
            ))
        },
    );

    lua.set_memory_limit(config.memory_limit_bytes())
        .map_err(lua_err)?;

    Ok(())
}

/// Creates a per-plugin `require` function that only resolves modules relative to
/// the plugin's own directory. Rejects paths containing `..` to prevent traversal.
pub fn create_plugin_require(lua: &Lua, plugin_dir: &Path) -> anyhow::Result<mlua::Function> {
    let plugin_dir = plugin_dir.to_path_buf();
    let require_fn = lua
        .create_function(move |lua, module_name: String| {
            validate_module_path(&module_name)?;
            let module_path = resolve_module_path(&plugin_dir, &module_name);

            let source = std::fs::read_to_string(&module_path).map_err(|e| {
                mlua::Error::RuntimeError(format!("cannot find module '{}': {}", module_name, e))
            })?;

            lua.load(&source)
                .set_name(format!("={}", module_name))
                .eval::<mlua::Value>()
        })
        .map_err(lua_err)?;

    Ok(require_fn)
}

/// Validates that a module path does not contain `..` or absolute path components.
fn validate_module_path(module_name: &str) -> std::result::Result<(), mlua::Error> {
    if module_name.contains("..") {
        return Err(mlua::Error::RuntimeError(
            "module path traversal ('..') is not allowed".to_string(),
        ));
    }
    if module_name.starts_with('/') || module_name.starts_with('\\') {
        return Err(mlua::Error::RuntimeError(
            "absolute module paths are not allowed".to_string(),
        ));
    }
    Ok(())
}

/// Resolves a Lua module name (using dots as separators) to a file path
/// relative to the plugin directory.
fn resolve_module_path(plugin_dir: &Path, module_name: &str) -> PathBuf {
    let relative = module_name.replace('.', std::path::MAIN_SEPARATOR_STR);
    plugin_dir.join(format!("{}.lua", relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> PluginConfig {
        PluginConfig {
            enabled: true,
            plugin_dirs: Vec::new(),
            instruction_limit: 1_000_000,
            memory_limit_mb: 10,
            overrides: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_create_lua_vm_basic() {
        let config = default_config();
        let lua = create_lua_vm(&config).expect("should create VM");

        let result: String = lua.load("return type(print)").eval().unwrap();
        assert_eq!(result, "function");

        let result: String = lua.load("return type(string.len)").eval().unwrap();
        assert_eq!(result, "function");

        let result: String = lua.load("return type(table.insert)").eval().unwrap();
        assert_eq!(result, "function");

        let result: String = lua.load("return type(math.floor)").eval().unwrap();
        assert_eq!(result, "function");
    }

    #[test]
    fn test_dangerous_globals_removed() {
        let config = default_config();
        let lua = create_lua_vm(&config).expect("should create VM");

        let result: String = lua.load("return type(loadfile)").eval().unwrap();
        assert_eq!(result, "nil");

        let result: String = lua.load("return type(dofile)").eval().unwrap();
        assert_eq!(result, "nil");

        let result: String = lua.load("return type(require)").eval().unwrap();
        assert_eq!(result, "nil");
    }

    #[test]
    fn test_os_restricted() {
        let config = default_config();
        let lua = create_lua_vm(&config).expect("should create VM");

        let result: String = lua.load("return type(os.clock)").eval().unwrap();
        assert_eq!(result, "function");

        let result: String = lua.load("return type(os.time)").eval().unwrap();
        assert_eq!(result, "function");

        let result: String = lua.load("return type(os.date)").eval().unwrap();
        assert_eq!(result, "function");

        let result: String = lua.load("return type(os.execute)").eval().unwrap();
        assert_eq!(result, "nil");

        let result: String = lua.load("return type(os.remove)").eval().unwrap();
        assert_eq!(result, "nil");

        let result: String = lua.load("return type(os.rename)").eval().unwrap();
        assert_eq!(result, "nil");

        let result: String = lua.load("return type(os.exit)").eval().unwrap();
        assert_eq!(result, "nil");
    }

    #[test]
    fn test_io_not_available() {
        let config = default_config();
        let lua = create_lua_vm(&config).expect("should create VM");

        let result: String = lua.load("return type(io)").eval().unwrap();
        assert_eq!(result, "nil");
    }

    #[test]
    fn test_debug_not_available() {
        let config = default_config();
        let lua = create_lua_vm(&config).expect("should create VM");

        let result: String = lua.load("return type(debug)").eval().unwrap();
        assert_eq!(result, "nil");
    }

    #[test]
    fn test_instruction_limit() {
        let config = PluginConfig {
            instruction_limit: 100,
            ..default_config()
        };
        let lua = create_lua_vm(&config).expect("should create VM");

        let result = lua.load("while true do end").exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("instruction limit exceeded"),
            "unexpected error: {}",
            err_msg
        );
    }

    #[test]
    fn test_memory_limit() {
        let config = PluginConfig {
            memory_limit_mb: 1,
            ..default_config()
        };
        let lua = create_lua_vm(&config).expect("should create VM");

        let result = lua
            .load(
                r#"
            local t = {}
            for i = 1, 1000000 do
                t[i] = string.rep("x", 1000)
            end
        "#,
            )
            .exec();
        assert!(result.is_err(), "should fail with memory limit exceeded");
    }

    #[test]
    fn test_require_path_traversal_rejected() {
        let config = default_config();
        let lua = create_lua_vm(&config).expect("should create VM");

        let plugin_dir = std::env::temp_dir().join("test_plugin_require");
        let require_fn = create_plugin_require(&lua, &plugin_dir).expect("should create require");
        lua.globals().set("require", require_fn).unwrap();

        let result = lua.load(r#"require("../escape")"#).exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("path traversal"),
            "unexpected error: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_module_path_rejects_traversal() {
        assert!(validate_module_path("../escape").is_err());
        assert!(validate_module_path("foo/../bar").is_err());
        assert!(validate_module_path("/absolute/path").is_err());
        assert!(validate_module_path("valid.module").is_ok());
        assert!(validate_module_path("simple").is_ok());
    }

    #[test]
    fn test_resolve_module_path() {
        let dir = Path::new("/plugins/my-plugin");
        assert_eq!(
            resolve_module_path(dir, "utils"),
            PathBuf::from("/plugins/my-plugin/utils.lua")
        );
        assert_eq!(
            resolve_module_path(dir, "lib.helpers"),
            PathBuf::from(format!(
                "/plugins/my-plugin/lib{}helpers.lua",
                std::path::MAIN_SEPARATOR
            ))
        );
    }
}
