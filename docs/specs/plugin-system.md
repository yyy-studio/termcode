# Phase 5: Plugin System

Lua-based plugin system using `mlua` crate, implemented in the `termcode-plugin` crate (Layer 3). Plugins can register custom commands, respond to editor events (hooks), and read/modify editor state through a sandboxed Lua API.

## Code Reference Checklist

| Item                    | Result                                                                                                                                                                                                                                       |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Similar feature exists? | No. `termcode-plugin` crate exists but is empty (single comment `// Plugin system - Phase 5`). No plugin infrastructure anywhere.                                                                                                            |
| Reference pattern       | Command Pattern: `CommandRegistry` + `CommandEntry` + `CommandHandler = fn(&mut Editor) -> anyhow::Result<()>` in `termcode-term/src/command.rs`. Plugins must integrate with this system to register commands.                              |
| Reference pattern       | LSP Bridge pattern: `LspBridge` uses `mpsc` channels to bridge async tokio operations with the synchronous event loop. Plugin async operations (if any) should follow the same channel-based pattern.                                        |
| Reference pattern       | Config loading: `AppConfig::load()` reads TOML at startup. Plugin config should follow the same approach -- load once at startup, path-based discovery.                                                                                      |
| Technical constraints   | `termcode-view` is frontend-agnostic -- plugin API types that touch Editor must live in `termcode-plugin` (Layer 3) or be thin wrappers. `CommandHandler` is `fn` pointer, not closure -- Lua callbacks need a different dispatch mechanism. |

### Key files

| File                                   | Role                                                                 |
| -------------------------------------- | -------------------------------------------------------------------- |
| `crates/termcode-plugin/src/lib.rs`    | Empty -- all plugin code goes here                                   |
| `crates/termcode-plugin/Cargo.toml`    | Currently depends only on `termcode-view`                            |
| `crates/termcode-term/src/command.rs`  | `CommandRegistry`, `CommandEntry`, `CommandHandler` type definitions |
| `crates/termcode-term/src/app.rs`      | `App` struct, event loop, `handle_key()` dispatch                    |
| `crates/termcode-term/src/input.rs`    | `InputMapper` -- keybinding resolution                               |
| `crates/termcode-view/src/editor.rs`   | `Editor` struct -- single source of truth for model layer            |
| `crates/termcode-config/src/config.rs` | `AppConfig` -- where plugin config section will be added             |
| `runtime/plugins/`                     | Plugin discovery directory (does not exist yet, must be created)     |

---

## Architecture Overview

```
runtime/plugins/
  my-plugin/
    init.lua          <-- Plugin entry point
    plugin.toml       <-- Plugin metadata & config

termcode-plugin (Layer 3)
  PluginManager       <-- Owns mlua::Lua VM, loads/manages plugins
  PluginApi           <-- Lua-exposed API (editor.*, buffer.*, etc.)
  PluginHookManager   <-- Event hook dispatch (on_save, on_open, etc.)
  PluginCommand       <-- Bridge between Lua functions and CommandRegistry

termcode-term (Layer 3)
  App                 <-- Owns PluginManager, calls hooks at event points
```

### Data Flow

```
Plugin Load (startup):
  App::new() -> PluginManager::new(config) -> scan runtime/plugins/ + config.plugin_dirs
    -> For each plugin: Lua::load(init.lua) -> plugin registers commands/hooks

Command Execution (user triggers plugin command):
  KeyEvent -> InputMapper -> CommandId("plugin.example.my-cmd")
    -> App intercepts "plugin.*" commands
    -> PluginManager::execute_command("plugin.example.my-cmd", &mut Editor)
    -> Lua function called with PluginApi wrapper around Editor

Hook Dispatch (editor events):
  App detects event (file open, save, buffer change, etc.)
    -> PluginManager::dispatch_hook("on_save", context)
    -> All registered Lua hook functions called in registration order
```

---

## Functional Requirements

### FR-PLUGIN-001: Plugin Manager

- **Description**: Central coordinator that owns the Lua VM, discovers plugins, and manages their lifecycle. "Disable" means config-based exclusion at startup (`PluginStatus::Disabled`), not runtime toggling (runtime reload is out-of-scope for Phase 5).
- **Priority**: High
- **Status**: Draft
- **Code Reference**: New struct in `crates/termcode-plugin/src/manager.rs`
- **Details**:
  - `PluginManager` owns a single `mlua::Lua` instance created with `Lua::new_with()` specifying only safe standard libraries (see FR-PLUGIN-007 for exact configuration).
  - Scans plugin directories at startup for `init.lua` files. Plugins within each directory are loaded in **alphabetical order by directory name** (deterministic ordering for consistent hook execution order).
  - Plugin discovery paths are determined by: `runtime/plugins/` (always scanned, bundled plugins) plus paths from `plugin_config.plugin_dirs` (default: `["~/.config/termcode/plugins"]`). See FR-PLUGIN-008 for config details.
  - Each plugin runs in its own Lua table namespace to prevent global pollution.
  - `PluginManager` exposes:
    - `new(config: PluginConfig) -> Result<Self>` -- create Lua VM with config (sandbox limits, paths), register global API. Returns error if VM creation fails; `App` handles by logging warning and running without plugins.
    - `load_plugins(dirs: &[PathBuf]) -> Vec<PluginInfo>` -- discover and load all plugins
    - `load_plugin(path: &Path) -> Result<PluginInfo>` -- load a single plugin
    - `execute_command(&mut self, name: &str, editor: &mut Editor) -> Result<()>` -- run a plugin command (needs `&mut self` for `buffer_mutated` flag, deferred actions, `is_dispatching` guard)
    - `dispatch_hook(&mut self, hook: HookEvent, editor: &mut Editor) -> Result<()>` -- fire a hook to all listeners (needs `&mut self` for `is_dispatching` guard)
    - `list_plugins() -> Vec<PluginInfo>` -- list loaded plugins with status
    - `list_commands() -> Vec<(String, String)>` -- list plugin-registered commands (id, description)
  - **Acceptance Criteria**:
    - AC1: `PluginManager::new(config)` returns `Ok(Self)` with a sandboxed Lua VM; returns `Err` on VM creation failure (App logs warning and continues without plugins)
    - AC2: `load_plugins()` discovers all valid plugins in given directories and returns `PluginInfo` for each
    - AC3: Plugins with invalid `init.lua` are marked as `PluginStatus::Failed` (still listed in `list_plugins()`, not silently skipped); other plugins load normally
    - AC4: `list_plugins()` returns accurate status (`Loaded`, `Failed`, `Disabled`) for all discovered plugins
    - AC5: Directories are scanned in list order (`runtime/plugins/` first, then each path in `plugin_dirs` in config order). Within each directory, plugins are loaded in alphabetical order by directory name.
    - AC6: Duplicate plugin names across directories: later directory takes precedence, earlier plugin is omitted from `list_plugins()` entirely (not listed). A warning is logged.

### FR-PLUGIN-002: Plugin Metadata (plugin.toml)

- **Description**: Each plugin may include a `plugin.toml` manifest describing its name, version, description, and configuration. If absent, defaults are derived from the directory name.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: Parsed in `PluginManager::load_plugin()`
- **Details**:
  - Structure:

    ```toml
    [plugin]
    name = "example-plugin"
    version = "0.1.0"
    description = "An example plugin"
    author = "Author Name"
    # Minimum termcode version required (optional, reserved for future use -- not validated in Phase 5)
    # min_version = "0.1.0"

    [config]
    # Plugin-specific config, accessible from Lua as `plugin.config`
    some_option = true
    another_option = "value"
    ```

  - Parsed into `PluginInfo` struct:

    ```rust
    pub enum PluginStatus {
        Loaded,
        Failed(String),  // error message
        Disabled,
    }

    pub struct PluginInfo {
        pub name: String,
        pub version: String,
        pub description: String,
        pub author: String,
        pub path: PathBuf,
        pub status: PluginStatus,
    }
    ```

  - If `plugin.toml` is missing, plugin still loads with defaults derived from directory name.
  - **Plugin name validation**: Plugin names must match `[a-z0-9_-]+` (lowercase alphanumeric, hyphens, underscores). Dots are not allowed (they conflict with the `plugin.{name}.{id}` command ID format). Invalid names cause the plugin to be marked as `PluginStatus::Failed` with a warning log.
  - Command IDs registered via `register_command()` must also match `[a-z0-9_-]+`.
  - **Acceptance Criteria**:
    - AC1: `plugin.toml` with all fields parses into correct `PluginInfo`
    - AC2: Missing `plugin.toml` loads plugin with name derived from directory name
    - AC3: Invalid TOML syntax logs warning and marks plugin as `PluginStatus::Failed` (listed in `list_plugins()`)
    - AC4: `[config]` section values are accessible from Lua via `plugin.config`
    - AC5: Plugin name containing dots or invalid characters → `PluginStatus::Failed` with warning
    - AC6: Command ID containing dots or invalid characters → Lua error at registration time

### FR-PLUGIN-003: Plugin Entry Point (init.lua)

- **Description**: Each plugin's `init.lua` is executed once at load time. It registers commands, hooks, and sets up plugin state.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: Loaded by `PluginManager::load_plugin()`
- **Details**:
  - `init.lua` receives a `plugin` table with:
    - `plugin.name` -- plugin name from `plugin.toml`
    - `plugin.config` -- plugin-specific config table from `plugin.toml [config]`
    - `plugin.register_command(id, description, callback)` -- register a command (init-time only)
    - `plugin.on(hook_name, callback)` -- register a hook listener (init-time only)
    - `plugin.require(module_name)` -- per-plugin require with isolated module cache (see FR-PLUGIN-007)
  - `register_command()` and `on()` are only valid during `init.lua` execution. Calling them from within a command/hook callback raises a Lua error: `"registration only allowed during plugin initialization"`.
  - Example `init.lua`:

    ```lua
    local p = plugin

    p.register_command("hello", "Say Hello", function()
      editor.set_status("Hello from " .. p.name .. "!")
    end)

    p.on("on_save", function(ctx)
      editor.set_status("File saved: " .. ctx.path)
    end)
    ```

  - **Acceptance Criteria**:
    - AC1: `init.lua` is executed exactly once per plugin load
    - AC2: `plugin.name` and `plugin.config` are accessible from Lua and match `plugin.toml` values
    - AC3: Commands registered via `plugin.register_command()` appear in `PluginManager::list_commands()`
    - AC4: Hooks registered via `plugin.on()` are dispatched when the corresponding event fires
    - AC5: Calling `register_command()` or `on()` outside of `init.lua` execution (e.g., from a command/hook callback) raises a Lua error: `"registration only allowed during plugin initialization"`

### FR-PLUGIN-004: Editor API (Lua)

- **Description**: Lua-exposed API for reading and modifying editor state. Exposed as global `editor` table in the Lua VM.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `Editor` struct in `crates/termcode-view/src/editor.rs`
- **Details**:
  - The API provides safe, read/write access to editor state via scoped closures on a pre-created `editor` table (see Technical Design section). The `Editor` reference is captured temporarily during command/hook execution via `Lua::scope()`.
  - **Read-only methods**:
    - `editor.get_mode() -> string` -- current editor mode ("normal", "insert", etc.)
    - `editor.get_cursor() -> {line, col}` -- cursor position (1-based)
    - `editor.get_selection() -> {start={line,col}, end={line,col}}|nil` -- primary selection; returns `nil` when no explicit selection is active (cursor only, no range selected)
    - `editor.get_line(n) -> string` -- get line content (1-based), returned **without** trailing newline
    - `editor.get_line_count() -> number` -- total lines in active buffer
    - `editor.get_filename() -> string|nil` -- active document filename
    - `editor.get_filepath() -> string|nil` -- active document full path
    - `editor.get_status() -> string|nil` -- current status message
    - `editor.get_theme_name() -> string` -- active theme name
    - `editor.get_config(key) -> value|nil` -- read editor config value. `key` is a flat string matching `EditorConfig` field names (e.g., `"tab_size"`, `"line_numbers"`, `"mouse_enabled"`). Returns `nil` for unknown keys. Type mapping: Rust `bool` → Lua `boolean`, `usize/u64` → `number`, `String` → `string`, enum → `string` (variant name)
  - **Write methods**:
    - `editor.set_status(msg)` -- set status bar message
    - `editor.insert_text(text)` -- insert text at cursor position (creates Transaction)
    - `editor.delete_selection()` -- delete current selection
    - `editor.set_cursor(line, col)` -- move cursor (1-based)
    - `editor.set_selection(start_line, start_col, end_line, end_col)` -- set selection
    - `editor.open_file(path)` -- open a file in a new tab
    - `editor.execute_command(id)` -- execute a built-in command by ID
  - **Buffer methods** (operate on active document):
    - `editor.buffer_get_text() -> string` -- entire buffer content
    - `editor.buffer_get_range(start_line, start_col, end_line, end_col) -> string`
    - `editor.buffer_replace_range(start_line, start_col, end_line, end_col, text)` -- replace text range (creates Transaction)
  - **Buffer-mutating** write methods (`insert_text`, `delete_selection`, `buffer_replace_range`) create proper `Transaction` objects for undo/redo support and trigger LSP `didChange` notification. `PluginManager` tracks a `buffer_mutated: bool` flag during execution, reset before each command/hook call. After Lua scope ends, `App` checks this flag to send `didChange` if needed.
  - Non-buffer write methods (`set_status`, `set_cursor`, `set_selection`) do NOT create Transactions or trigger `didChange`.
  - **Deferred action methods** (`open_file`, `execute_command`): These methods require `App`-level access (LSP didOpen, clipboard, quit, etc.) that is not available inside `Lua::scope()`. They are implemented as **deferred actions**: the Lua API appends them to a `Vec<DeferredAction>` during execution, and `App` processes them after the Lua scope ends. This avoids re-entrancy into `App` methods during plugin execution.
    - `DeferredAction` enum: `OpenFile(PathBuf)`, `ExecuteCommand(String)`
    - `execute_command()` cannot execute other `plugin.*` commands (to prevent indirect re-entrancy). Attempting this returns a Lua error.
  - Line/column arguments use 1-based indexing (Lua convention). Internal conversion to 0-based.
  - **Range semantics**: All range parameters (`set_selection`, `buffer_get_range`, `buffer_replace_range`) use **inclusive start, exclusive end** (half-open interval). For example, `buffer_get_range(1, 1, 1, 5)` returns columns 1-4 of line 1 (4 characters). This matches common editor conventions and simplifies "cursor at end of range" semantics.
    - **Reversed range** (start > end): raises a Lua error `"invalid range: start must be before end"`.
    - **Empty range** (start == end): returns empty string for `buffer_get_range`, deletes nothing but inserts replacement text at the position for `buffer_replace_range`, zero-width selection for `set_selection`.
  - **Error behavior for invalid arguments**:
    - Out-of-range line/col (e.g., `get_line(0)`, `get_line(999999)`, `set_cursor(-1, -1)`): raise a Lua error with descriptive message (e.g., `"line out of range: 999999, max: 42"`)
    - `nil` arguments where non-nil expected: raise a Lua error (e.g., `"expected number, got nil"`)
    - Wrong type arguments: raise a Lua error via mlua's automatic type checking
    - `get_filename()` / `get_filepath()` when no document is open: return `nil` (not an error)
    - All other methods requiring an active document (`get_cursor`, `get_line`, `get_line_count`, `get_selection`, `insert_text`, `delete_selection`, `set_cursor`, `set_selection`, `buffer_*`) raise a Lua error: `"no active document"` when no document tab is active
  - **Acceptance Criteria**:
    - AC1: All read-only methods return correct values matching `Editor` state
    - AC2: Buffer-mutating write methods (`insert_text`, `delete_selection`, `buffer_replace_range`) create proper `Transaction` objects and apply them via `Document::apply_transaction()` (inverse computed before apply, then committed to History -- matching the existing pattern, no `termcode-view` changes needed). Verifiable via undo.
    - AC3: Out-of-range arguments raise Lua errors (not panics) with descriptive messages
    - AC4: `nil`/wrong-type arguments raise Lua errors via mlua type checking
    - AC5: Buffer-mutating write methods trigger LSP `didChange` notification
    - AC6: `open_file()` and `execute_command()` append to deferred action queue; actions execute in order after Lua scope ends
    - AC7: `execute_command("plugin.*")` raises a Lua error (prevents indirect re-entrancy)

### FR-PLUGIN-005: Hook System

- **Description**: Event-driven hook system allowing plugins to respond to editor lifecycle events.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: Hook dispatch points in `App::handle_key()`, `App::open_file()`, etc.
- **Details**:
  - Supported hooks:
    | Hook Name | Trigger Point | Context Fields |
    | ----------------- | ------------------------------------ | --------------------------------------- |
    | `on_open` | After file is opened | `{path, filename, language}` |
    | `on_save` | After file is saved | `{path, filename}` |
    | `on_close` | Before tab is closed | `{path, filename}` |
    | `on_mode_change` | After mode switches | `{old_mode, new_mode}` |
    | `on_cursor_move` | After cursor position changes | `{line, col}` |
    | `on_buffer_change`| After any buffer modification | `{path, filename}` |
    | `on_tab_switch` | After active tab changes | `{path, filename}` |
    | `on_ready` | After all plugins loaded, editor ready | `{}` |
  - Hook registration: `plugin.on("on_save", function(ctx) ... end)`
  - Multiple plugins can register for the same hook. Execution order is plugin load order.
  - Hook callbacks receive a context table with event-specific fields. For unsaved buffers (no file path), `path` and `filename` are `nil`. Hooks requiring a document context (`on_save`, `on_buffer_change`) are not fired for non-document tabs (e.g., image viewer). `on_tab_switch` fires for all tab types with `path = nil` for non-document tabs.
  - Hook errors are logged but do not crash the editor. Status bar shows warning.
  - `on_cursor_move` fires only when the cursor position (line, col) differs from the last dispatched position, checked once per render frame. It does NOT use time-based debouncing. This prevents performance degradation from plugins doing expensive work on repeated cursor movements.
  - Hooks are **not re-entrant**. If a hook callback triggers an action that would fire another hook, the inner hook call is **skipped** (not queued) and a warning is logged: `"Skipped re-entrant hook '{hook_name}' from plugin '{plugin_name}'"`.
  - **Acceptance Criteria**:
    - AC1: All 8 hook types fire at the correct trigger points with correct context fields (exception: hooks suppressed by the re-entrancy guard per AC5 are expected to not fire)
    - AC2: Multiple plugins can register for the same hook; execution follows load order
    - AC3: Hook errors are caught and displayed in status bar; editor does not crash
    - AC4: `on_cursor_move` does not fire when cursor position is unchanged between frames
    - AC5: Re-entrant hook calls are skipped with a warning log message

### FR-PLUGIN-006: Plugin Command Registration

- **Description**: Plugins register commands that integrate with the existing command system. Plugin commands are accessible via command palette and keybindings.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `CommandRegistry` in `crates/termcode-term/src/command.rs`
- **Details**:
  - Plugin commands use the ID format `plugin.{plugin_name}.{command_id}`.
    - Example: `plugin.example.hello`
  - Registration flow:
    1. Lua calls `plugin.register_command("hello", "Say Hello", callback)`
    2. `PluginManager` stores the Lua function reference keyed by full command ID
    3. A proxy `CommandEntry` is registered in `CommandRegistry` with a noop handler
    4. `App` intercepts `"plugin.*"` commands in **both** `handle_key()` (keybinding path) and the command palette execution path, delegating to `PluginManager::execute_command()`
  - This follows the same pattern used for `palette.open`, `goto.definition`, and other App-level commands. Both the keybinding and palette dispatch paths must check for `plugin.*` prefix before calling the noop handler.
  - Plugin commands appear in the command palette with format: `[Plugin Name] Command Description`
  - **Acceptance Criteria**:
    - AC1: Plugin commands are accessible from the command palette
    - AC2: Plugin commands can be bound to keys via `keybindings.toml`
    - AC3: Both keybinding path (`handle_key()`) and command palette path correctly intercept and delegate `"plugin.*"` commands to `PluginManager`
    - AC4: Plugin command execution errors are displayed in status bar (not panics)
  - Plugin commands can be bound to keys via `keybindings.toml`:
    ```toml
    [normal]
    "ctrl+shift+h" = "plugin.example.hello"
    ```

### FR-PLUGIN-007: Lua Sandbox

- **Description**: Lua VM runs in a sandboxed environment with restricted access to the host system.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `mlua::Lua` configuration
- **Details**:
  - Sandbox restrictions:
    - **No `os.execute`, `io.popen`** -- no arbitrary shell execution
    - **No `io` library** -- no direct file I/O (use `editor.open_file()` instead)
    - **No `debug` library** -- no VM introspection/manipulation
    - **No `loadfile`, `dofile`** -- plugins cannot load arbitrary Lua files outside their directory
    - **`require`** is not exposed as a global. Instead, each plugin receives a per-plugin `require` function injected into its namespace table during `load_plugin()`. This per-plugin `require` uses a custom searcher that resolves module paths relative to that plugin's directory only, and maintains a **per-plugin module cache** (separate `loaded` table per plugin, not the global `package.loaded`). This prevents module name collisions when two plugins both `require("utils")`. Path traversal (`..`) is rejected — `plugin.require("../escape")` raises a Lua error: `"invalid module path: directory traversal not allowed"`. This avoids ambiguity in a shared VM where a single global `require` cannot determine the calling plugin's directory.
  - Allowed standard libraries:
    - `string`, `table`, `math`, `utf8` -- safe data manipulation
    - `os.clock`, `os.time`, `os.date` -- time utilities only (no `os.execute`)
    - `pcall`, `xpcall`, `error`, `assert` -- error handling
    - `type`, `tostring`, `tonumber`, `select`, `pairs`, `ipairs`, `next` -- basics
  - Resource limits:
    - Instruction count limit per command/hook execution (configurable, default 1,000,000 instructions)
    - Memory limit per Lua VM (configurable, default 64 MB)
    - These prevent infinite loops and memory exhaustion from buggy plugins
  - **Exact mlua API**: Use `Lua::new_with(StdLib::BASE | StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8 | StdLib::OS, LuaOptions::default())`. Then selectively remove dangerous base globals (`loadfile`, `dofile`) and dangerous `os` functions (keep only `clock`, `time`, `date`). `StdLib::BASE` is required for `pcall`, `type`, `tostring`, `pairs`, etc. Use `Lua::set_hook()` with `HookTriggers::every_nth_instruction(N)` for instruction counting. Use `Lua::set_memory_limit(bytes)` for memory limits.
  - **Acceptance Criteria**:
    - AC1: `os.execute`, `io.popen`, `io.open` raise errors when called from Lua
    - AC2: `debug` library is not accessible
    - AC3: `loadfile`, `dofile` raise errors; `require` only resolves within plugin directory; paths containing `..` are rejected
    - AC4: Instruction limit triggers error after configured number of instructions (default 1M)
    - AC5: Memory limit triggers error when VM exceeds configured limit (default 64MB)
    - AC6: `string`, `table`, `math`, `utf8`, `os.clock`, `os.time`, `os.date` are accessible
    - AC7: `os` library is restricted to only `clock`, `time`, `date`; all other `os.*` functions (e.g., `os.execute`, `os.remove`, `os.rename`) raise errors

### FR-PLUGIN-008: Plugin Configuration

- **Description**: Application-level plugin configuration in `config.toml` and per-plugin configuration in `plugin.toml`.
- **Priority**: Medium
- **Status**: Draft
- **Code Reference**: `AppConfig` in `crates/termcode-config/src/config.rs`
- **Details**:
  - New `[plugins]` section in `config.toml`:

    ```toml
    [plugins]
    enabled = true                          # Global plugin enable/disable
    plugin_dirs = ["~/.config/termcode/plugins"]  # Additional plugin search paths
    instruction_limit = 1000000             # Max Lua instructions per execution
    memory_limit_mb = 64                    # Max Lua VM memory

    # Per-plugin overrides
    [plugins.overrides."example-plugin"]
    enabled = false                         # Disable specific plugin
    ```

  - Parsed into:

    ```rust
    #[derive(Debug, Deserialize)]
    #[serde(default)]
    pub struct PluginConfig {
        pub enabled: bool,
        pub plugin_dirs: Vec<String>,
        pub instruction_limit: u64,
        pub memory_limit_mb: u64,
        pub overrides: HashMap<String, PluginOverride>,
    }

    #[derive(Debug, Deserialize)]
    pub struct PluginOverride {
        pub enabled: Option<bool>,
    }
    ```

  - Added to `AppConfig`:

    ```rust
    pub struct AppConfig {
        // ... existing fields ...
        pub plugins: PluginConfig,
    }
    ```

  - **Config precedence**: Global `plugins.enabled = false` disables all plugins unconditionally, regardless of per-plugin overrides. When global is `true`, per-plugin `overrides.{name}.enabled` takes effect (default: `true` if not specified).
  - **Tilde expansion**: `plugin_dirs` paths undergo tilde expansion (`~` → home directory) during config loading, using `dirs::home_dir()` or equivalent.
  - **Acceptance Criteria**:
    - AC1: `[plugins]` section with all fields parses correctly into `PluginConfig`
    - AC2: Missing `[plugins]` section uses defaults (`enabled = true`, `plugin_dirs = ["~/.config/termcode/plugins"]`, 1M instructions, 64MB memory)
    - AC3: Global `enabled = false` prevents all plugin loading regardless of per-plugin overrides
    - AC4: Per-plugin `enabled = false` disables only that plugin when global is `true`
    - AC5: `plugin_dirs` paths with `~` are expanded to home directory

### FR-PLUGIN-009: Plugin Lifecycle

- **Description**: Defines the full lifecycle of plugin loading, execution, and teardown.
- **Priority**: Medium
- **Status**: Draft
- **Code Reference**: `App::new()` in `crates/termcode-term/src/app.rs`
- **Details**:
  - **Startup sequence** (in `App::new()` or `App::run()` init):
    1. Parse `PluginConfig` from `AppConfig`
    2. If `plugins.enabled == false`, skip all plugin loading
    3. Create `PluginManager::new(plugin_config)`
    4. Collect plugin directories: `runtime/plugins/` + `plugin_config.plugin_dirs`
    5. `PluginManager::load_plugins(dirs)` -- discover and load each plugin
    6. For each loaded plugin, register its commands in `CommandRegistry`
    7. Re-validate user keybindings (`InputMapper`) against the updated `CommandRegistry` so that plugin command IDs bound in `keybindings.toml` are recognized
    8. Dispatch `on_ready` hook
  - **Shutdown**: No explicit teardown needed. Lua VM is dropped with `PluginManager`.
  - **Error handling during load**:
    - If `init.lua` has syntax errors: log warning, mark plugin as `PluginStatus::Failed`, continue loading others
    - If `init.lua` runtime error: log warning, mark plugin as `PluginStatus::Failed`, continue
    - Never crash the editor due to plugin errors. Use `catch_unwind` at the `PluginManager` boundary if `mlua` can panic.
  - **Runtime reload is out-of-scope for Phase 5**. Plugins load only at startup. No unload/reload/hot-reload support. This makes `Box::leak` for `CommandId` safe (bounded by startup-only plugin loading).
  - **Acceptance Criteria**:
    - AC1: Startup sequence executes steps 1-8 in order when `plugins.enabled = true`
    - AC2: `plugins.enabled = false` skips all plugin loading (no Lua VM created)
    - AC3: Syntax errors in `init.lua` log warning and mark plugin as `Failed`; other plugins load
    - AC4: Runtime errors in `init.lua` log warning and mark plugin as `Failed`; other plugins load
    - AC5: `on_ready` hook fires after all plugins are loaded

### FR-PLUGIN-010: Crate Structure

- **Description**: Internal module structure of the `termcode-plugin` crate.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `crates/termcode-plugin/`
- **Details**:
  - Module structure:
    ```
    crates/termcode-plugin/src/
      lib.rs            -- pub mod declarations, re-exports
      manager.rs        -- PluginManager struct
      api.rs            -- Lua API registration (editor.*, buffer.*)
      hooks.rs          -- HookEvent enum, HookManager
      sandbox.rs        -- Lua VM creation with sandboxing
      types.rs          -- PluginInfo, HookContext, internal types
    ```
  - `Cargo.toml` dependencies:
    ```toml
    [dependencies]
    termcode-view = { workspace = true }
    termcode-core = { workspace = true }
    termcode-config = { workspace = true }
    mlua = { version = "0.10", features = ["lua54", "vendored", "serialize"] }
    serde = { workspace = true }
    toml = { workspace = true }
    anyhow = { workspace = true }
    log = { workspace = true }
    ```
  - `termcode-plugin` is Layer 3 (depends on core, config, view). It does NOT depend on `termcode-term`.
  - **Acceptance Criteria**:
    - AC1: All 6 modules compile with no errors
    - AC2: `cargo test -p termcode-plugin` passes
    - AC3: No cyclic dependencies introduced (termcode-plugin does NOT depend on termcode-term)
  - `termcode-term` depends on `termcode-plugin` and owns the `PluginManager` instance inside `App`.
  - The `PluginManager::execute_command()` and `dispatch_hook()` methods accept `&mut Editor` -- this is valid because `termcode-plugin` already depends on `termcode-view` which defines `Editor`.

### FR-PLUGIN-011: Logging API

- **Description**: Plugins can write to the editor's log system for debugging.
- **Priority**: Low
- **Status**: Draft
- **Code Reference**: `log` crate usage across codebase
- **Details**:
  - Lua global `log` table:
    - `log.info(msg)` -- log at info level
    - `log.warn(msg)` -- log at warn level
    - `log.error(msg)` -- log at error level
    - `log.debug(msg)` -- log at debug level
  - Log messages are prefixed with `[plugin:{plugin_name}]` for traceability.
  - Logs go through the standard `log` crate macros, visible when running with `RUST_LOG=debug`.
  - **Acceptance Criteria**:
    - AC1: `log.info("msg")` from Lua produces `[plugin:name] msg` in log output
    - AC2: All four log levels (info, warn, error, debug) work correctly
    - AC3: Non-string arguments are converted via `tostring()` before logging

---

## Technical Design

### Lua VM Ownership and Editor Access

The key design challenge is safely passing `&mut Editor` to Lua callbacks. The approach:

1. `PluginManager` owns the `Lua` VM permanently.
2. When a command or hook is executed, `App` calls `PluginManager::execute_command(name, &mut Editor)`.
3. Inside that method, use `Lua::scope()` to create scoped closures (via `scope.create_function_mut()`) that capture `&mut Editor`. These closures are set as the `editor.*` API methods for the duration of the scope.
4. The Lua callback is called within the scope. The `Editor` reference is only valid for the duration of the scope.
5. After the scope ends, the scoped closures are invalidated automatically.

```rust
// Pseudocode
impl PluginManager {
    pub fn execute_command(&mut self, name: &str, editor: &mut Editor) -> Result<()> {
        self.buffer_mutated = false;
        self.deferred_actions.clear();
        let lua = &self.lua;
        let callback = self.commands.get(name)?.clone();

        lua.scope(|scope| {
            // Create scoped API methods that capture &mut Editor
            let get_line = scope.create_function_mut(|_, n: usize| {
                // Access editor within scope lifetime
                let line = editor.active_document()
                    .ok_or_else(|| mlua::Error::runtime("no active document"))?
                    .buffer.line(n);
                Ok(line)
            })?;
            let editor_table = lua.globals().get::<mlua::Table>("editor")?;
            editor_table.set("get_line", get_line)?;
            // ... register other scoped methods similarly ...

            callback.call::<()>(())?;
            Ok(())
        })?;

        // App checks self.buffer_mutated and self.deferred_actions after this returns
        Ok(())
    }
}
```

The `editor` global table is pre-created at VM init with placeholder methods. During `Lua::scope()`, these are temporarily replaced with scoped closures that have access to `&mut Editor`. After the scope, the closures are invalidated. This approach avoids storing scoped `UserData` in globals (which is not allowed by mlua's lifetime system).

### Command Integration Pattern

Plugin commands follow the same noop-handler-with-interception pattern as `goto.definition`:

```rust
// In App, after plugins load:
for (cmd_id_string, description) in plugin_manager.list_commands() {
    // Leak the string to get &'static str for CommandId
    // Safe: plugins load only at startup (no runtime reload in Phase 5)
    let static_id: &'static str = Box::leak(cmd_id_string.into_boxed_str());
    let static_name: &'static str = Box::leak(description.into_boxed_str());
    command_registry.register(CommandEntry {
        id: static_id,
        name: static_name,
        handler: cmd_noop,
    });
}

// In App::handle_key(), before normal command dispatch:
if cmd_id.starts_with("plugin.") {
    if let Some(ref mut pm) = self.plugin_manager {
        if let Err(e) = pm.execute_command(cmd_id, &mut self.editor) {
            self.editor.status_message = Some(format!("Plugin error: {e}"));
        }
    }
    return;
}
```

### Hook Dispatch Integration

Hooks are fired from `App` at the appropriate points:

```rust
// After file save in App:
if is_save {
    self.lsp_notify_did_save();
    if let Some(ref mut pm) = self.plugin_manager {
        let _ = pm.dispatch_hook(HookEvent::OnSave { path, filename }, &mut self.editor);
    }
}
```

### Memory Layout

```
App (termcode-term)
 ├── editor: Editor (termcode-view)
 ├── command_registry: CommandRegistry
 ├── input_mapper: InputMapper
 ├── plugin_manager: Option<PluginManager> (termcode-plugin)
 │    ├── lua: mlua::Lua
 │    ├── plugins: Vec<PluginInfo>
 │    ├── commands: HashMap<String, mlua::RegistryKey>  // Lua function refs
 │    └── hooks: HashMap<HookEvent, Vec<mlua::RegistryKey>>
 ├── lsp_bridge: Option<LspBridge>
 └── ...
```

---

## Crate Boundary Summary

| Change                     | Crate             | Layer |
| -------------------------- | ----------------- | ----- |
| `PluginManager`            | `termcode-plugin` | 3     |
| Lua API (`editor.*`)       | `termcode-plugin` | 3     |
| `HookEvent` enum           | `termcode-plugin` | 3     |
| Sandbox configuration      | `termcode-plugin` | 3     |
| `PluginConfig`             | `termcode-config` | 1     |
| `App` plugin integration   | `termcode-term`   | 3     |
| Hook dispatch calls        | `termcode-term`   | 3     |
| Command proxy registration | `termcode-term`   | 3     |

### No changes needed in

- `termcode-core` (Layer 0)
- `termcode-theme` (Layer 0)
- `termcode-syntax` (Layer 1)
- `termcode-view` (Layer 2) -- `Editor` is accessed via existing public API, no new fields
- `termcode-lsp` (Layer 2)

---

## Edge Cases

1. **Plugin syntax error in init.lua**: Log warning with file path and Lua error message. Plugin marked as failed in `PluginInfo`. Other plugins continue loading normally.
2. **Plugin runtime error during command**: Catch error, display in status bar as `"Plugin error: {message}"`. Editor state is not corrupted because `Lua::scope()` ensures the Editor borrow ends cleanly.
3. **Plugin infinite loop**: Instruction count limit (default 1M) triggers `mlua::Error::RuntimeError`. Caught and displayed as timeout error in status bar.
4. **Plugin exceeds memory limit**: `mlua` memory limit triggers error. Plugin execution is aborted. **Known limitation**: since all plugins share a single Lua VM with a global memory limit, one plugin's excessive memory usage can affect others. Per-plugin VMs may be considered in a future phase if isolation is needed.
5. **Two plugins with the same name**: Since full IDs are `plugin.{plugin_name}.{command_id}`, command ID collision requires same plugin names. This is prevented by edge case 11 (earlier plugin is skipped entirely when duplicate names are found). Therefore, command overwrites do not occur in practice.
6. **Plugin tries to access editor outside of command/hook context**: Scoped closures are invalidated after `Lua::scope()` ends. API methods return error: `"editor not available outside command/hook context"`.
7. **Hook callback mutates editor state that triggers another hook**: Hooks are not re-entrant. A boolean flag (`is_dispatching`) prevents recursive hook dispatch. Inner hook calls are **skipped** with a warning log: `"Skipped re-entrant hook '{name}'"`. Queuing is not used to avoid complexity and unbounded growth. Specifically: if a hook callback calls `editor.insert_text()` or other write methods, the buffer mutation is applied, but `on_buffer_change` is **suppressed** (skipped) because `is_dispatching` is still `true`.
8. **Plugin directory does not exist**: Silently skip. Do not create directories automatically.
9. **Plugin modifies buffer without going through Transaction**: Not possible -- all write methods in the Lua API create proper Transactions internally.
10. **Large number of plugins**: Load sequentially. No parallel loading (single Lua VM). Target: plugin loading adds <100ms wall time for up to 50 plugins on a modern machine (init.lua execution only, no heavy computation). This is a soft guideline, not a hard requirement.
11. **Duplicate plugin names across directories**: If `runtime/plugins/foo/` and `~/.config/termcode/plugins/foo/` both exist, later directories in the scan order take precedence (user plugins override bundled plugins). The earlier one is skipped with a warning log.
12. **Deferred action failure**: If a deferred action fails (e.g., `OpenFile` with non-existent path, `ExecuteCommand` with invalid ID), the error is displayed in the status bar. Remaining deferred actions in the queue continue executing.

---

## Example Plugin

### Directory: `runtime/plugins/example/`

### `plugin.toml`

```toml
[plugin]
name = "example"
version = "0.1.0"
description = "Example plugin demonstrating the plugin API"
author = "termcode"
```

### `init.lua`

```lua
local p = plugin

-- Register a command that wraps the current selection in quotes
p.register_command("wrap_quotes", "Wrap Selection in Quotes", function()
  local sel = editor.get_selection()
  if sel then
    local text = editor.buffer_get_range(sel.start.line, sel.start.col, sel["end"].line, sel["end"].col)
    editor.buffer_replace_range(sel.start.line, sel.start.col, sel["end"].line, sel["end"].col, '"' .. text .. '"')
    editor.set_status("Wrapped selection in quotes")
  else
    editor.set_status("No selection")
  end
end)

-- Register a hook that shows a message when a file is saved
p.on("on_save", function(ctx)
  log.info("File saved: " .. ctx.path)
end)

-- Register a command that inserts the current date
p.register_command("insert_date", "Insert Current Date", function()
  local date = os.date("%Y-%m-%d")
  editor.insert_text(date)
end)
```

---

## Test Scenarios

### TS-1: Sandbox Restriction Verification

- Call `os.execute("ls")` from plugin → Lua error raised, no shell execution
- Call `io.open("/etc/passwd")` from plugin → Lua error raised, no file access
- Access `debug.getinfo` from plugin → Lua error (debug library unavailable)
- Call `loadfile("/tmp/malicious.lua")` from plugin → Lua error raised
- Call `plugin.require("socket")` (external module) from plugin → Lua error raised
- Call `plugin.require("mymodule")` where `mymodule.lua` exists in plugin dir → succeeds
- Two plugins both `plugin.require("utils")` with different `utils.lua` → each gets its own module (per-plugin cache isolation)
- Global `require` is not accessible → Lua error
- `plugin.require("../escape")` (path traversal) → Lua error `"invalid module path: directory traversal not allowed"`

### TS-2: Command Registration and Execution Round-trip

- Plugin registers command "hello" → `plugin.{name}.hello` appears in `list_commands()`
- User triggers command via palette → Lua callback executes, status bar updated
- User binds command in keybindings.toml → key press triggers plugin command
- Plugin command raises Lua error → status bar shows error, editor continues normally

### TS-3: Hook Dispatch with Context Validation

- Open file → `on_open` hook fires with correct `{path, filename, language}`
- Save file → `on_save` hook fires with correct `{path, filename}`
- Close tab → `on_close` hook fires with correct `{path, filename}` (nil for unsaved)
- Switch mode → `on_mode_change` fires with correct `{old_mode, new_mode}`
- Move cursor → `on_cursor_move` fires only when position changes between frames
- Buffer change → `on_buffer_change` fires with correct `{path, filename}`
- Switch tab → `on_tab_switch` fires with correct `{path, filename}` (nil for non-document tabs)
- All plugins loaded → `on_ready` fires with `{}`
- Hook that triggers another hook → inner hook is skipped, warning logged

### TS-4: Error Recovery

- Plugin with syntax error in `init.lua` → warning logged, plugin marked `Failed`, listed in `list_plugins()`, others load
- Plugin with runtime error in `init.lua` → warning logged, plugin marked `Failed`, listed in `list_plugins()`, others load
- Plugin command hits instruction limit: test with `while true do end` loop → Lua error raised within the instruction hook callback, error displayed in status bar, editor remains stable
- Plugin command exceeds memory limit: configure high instruction limit (100M) and low memory limit (1MB), test with `local t = {}; for i=1,math.huge do t[i] = string.rep("x", 1024) end` → Lua memory error raised (not instruction limit), error displayed, VM remains functional for other plugins

### TS-5: API Boundary Validation

- `editor.get_line(0)` (1-based, so 0 is invalid) → Lua error with descriptive message
- `editor.get_line(999999)` (beyond buffer) → Lua error with max line info
- `editor.set_cursor(-1, -1)` → Lua error
- `editor.get_filename()` with no open document → returns `nil`
- `editor.insert_text("hello")` → text inserted, undoable via Ctrl+Z
- `editor.buffer_replace_range(...)` → Transaction created, LSP didChange sent
- `editor.open_file("test.txt")` → deferred action appended, file opened after scope ends
- `editor.execute_command("plugin.foo.bar")` → Lua error (plugin.\* commands blocked)
- `editor.execute_command("file.save")` → deferred action appended, executes after scope
- `editor.get_line(1)` on a file with trailing newline → returns line content **without** trailing newline
- `editor.buffer_get_range(1, 5, 1, 1)` (reversed range) → Lua error `"invalid range: start must be before end"`
- `editor.buffer_get_range(1, 3, 1, 3)` (empty range) → returns empty string `""`
- `editor.buffer_replace_range(1, 3, 1, 3, "x")` (empty range replace) → inserts "x" at position (no-op delete, then insert)

### TS-6: Plugin Metadata and Configuration

- Valid `plugin.toml` with all fields → `PluginInfo` has correct name, version, description, author
- Missing `plugin.toml` → plugin loads with name derived from directory name
- Invalid TOML syntax → warning logged, plugin marked `Failed`, listed in `list_plugins()`
- `[config]` section values → accessible from Lua via `plugin.config.some_option`
- Global `plugins.enabled = false` in config.toml → no plugins loaded
- Per-plugin `enabled = false` override → only that plugin disabled
- Missing `[plugins]` section in config.toml → defaults applied (enabled=true, 1M instructions, 64MB memory)
- Plugin name `foo.bar` (contains dot) → `PluginStatus::Failed`, warning logged
- Plugin name `my_plugin-v2` (valid) → loads successfully
- Command ID `hello.world` (contains dot) registered via `register_command()` → Lua error at registration
- `plugin_dirs = ["~/my-plugins"]` → tilde expanded to home directory, plugins discovered correctly
- Non-existent plugin directory in `plugin_dirs` → silently skipped, no error

### TS-7: Logging API

- `log.info("test")` from plugin → `[plugin:name] test` appears at info level
- `log.warn("warning")` → appears at warn level
- `log.error("error")` → appears at error level
- `log.debug("debug")` → appears at debug level (visible with `RUST_LOG=debug`)
- `log.info(42)` (non-string) → converted via `tostring()`, logs "42"

### TS-8: Crate Structure Validation

- `cargo build -p termcode-plugin` compiles with no errors
- `cargo test -p termcode-plugin` passes all tests
- `cargo tree -p termcode-plugin` shows no dependency on `termcode-term` (no cyclic deps)
- All 6 modules (`lib.rs`, `manager.rs`, `api.rs`, `hooks.rs`, `sandbox.rs`, `types.rs`) exist and are reachable from `lib.rs`

### TS-9: Lifecycle and Status Verification

- `init.lua` is executed exactly once per plugin load (register a command, verify it appears once in `list_commands()`)
- `list_plugins()` returns all three statuses: `Loaded` for valid plugins, `Failed` for syntax/runtime errors, `Disabled` for config-disabled plugins
- Startup sequence: plugins load before `on_ready` fires (verify via a hook that checks `list_plugins()` returns loaded plugins)
- Multi-plugin hook order: register hooks from plugin A (loaded first) and B (loaded second) → A's hook fires before B's
- Hook error: plugin registers a hook that throws → error shown in status bar, other hooks still fire
- Duplicate plugin name across directories: `runtime/plugins/foo/` and user `plugins/foo/` → earlier (bundled) is skipped, only user version loads, warning logged
- Deferred action failure: `editor.open_file("nonexistent.txt")` → error in status bar, subsequent deferred actions still execute
- Editor access outside scope: store `editor` reference in Lua variable, call after init → Lua error "editor not available outside command/hook context"
- Registration outside init: call `plugin.register_command()` from within a command callback → Lua error "registration only allowed during plugin initialization"
- Performance (soft guideline, not a hard pass/fail): loading 50 trivial plugins completes in <100ms wall time. This is a monitoring metric, not a blocking test criterion.

---

## Assigned IDs

| ID            | Description                                        |
| ------------- | -------------------------------------------------- |
| FR-PLUGIN-001 | PluginManager -- Lua VM ownership and lifecycle    |
| FR-PLUGIN-002 | Plugin metadata manifest (plugin.toml)             |
| FR-PLUGIN-003 | Plugin entry point (init.lua) and registration API |
| FR-PLUGIN-004 | Editor API exposed to Lua                          |
| FR-PLUGIN-005 | Hook system for editor events                      |
| FR-PLUGIN-006 | Plugin command registration and dispatch           |
| FR-PLUGIN-007 | Lua sandbox and resource limits                    |
| FR-PLUGIN-008 | Plugin configuration in config.toml                |
| FR-PLUGIN-009 | Plugin lifecycle (load, execute, teardown)         |
| FR-PLUGIN-010 | Crate internal module structure                    |
| FR-PLUGIN-011 | Logging API for plugins                            |
