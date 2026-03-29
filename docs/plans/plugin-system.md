# Plugin System Implementation Plan

**Created**: 2026-03-29
**Analysis Report**: docs/analysis/plugin-system.md
**Specification**: docs/specs/plugin-system.md
**Status**: Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-PLUGIN-001] Plugin Manager -- central coordinator, Lua VM ownership, plugin discovery/loading
- [FR-PLUGIN-002] Plugin Metadata (plugin.toml) -- manifest parsing, defaults from directory name
- [FR-PLUGIN-003] Plugin Entry Point (init.lua) -- execution, command/hook registration
- [FR-PLUGIN-004] Editor API (Lua) -- read/write editor state via scoped closures
- [FR-PLUGIN-005] Hook System -- 8 event hooks with re-entrancy guard
- [FR-PLUGIN-006] Plugin Command Registration -- noop+intercept pattern, palette integration
- [FR-PLUGIN-007] Lua Sandbox -- restricted stdlib, resource limits, per-plugin require
- [FR-PLUGIN-008] Plugin Configuration -- PluginConfig in config.toml, per-plugin overrides
- [FR-PLUGIN-009] Plugin Lifecycle -- startup sequence, error handling, no runtime reload
- [FR-PLUGIN-010] Crate Structure -- 6 modules in termcode-plugin
- [FR-PLUGIN-011] Logging API -- log.info/warn/error/debug from Lua

### Database

- None (no DB in this project)

### API

- None (no REST API; this is a TUI application)

### UI

- Plugin commands visible in command palette with `[Plugin Name] Description` format
- Plugin errors displayed in status bar

## 2. Analysis Report Reference

### Reference Documents

- Analysis Report: `docs/analysis/plugin-system.md`
- Specification: `docs/specs/plugin-system.md`

### Applied Recommendations

- Start with termcode-config (PluginConfig), then build termcode-plugin modules in dependency order (types -> sandbox -> hooks -> api -> manager -> lib), then integrate into termcode-term
- Use noop-handler-with-interception pattern for plugin commands (same as goto.definition, lsp.hover)
- Use `Box::leak()` for dynamic CommandId strings (safe because plugins load only at startup)
- Use `Lua::scope()` for safely passing `&mut Editor` to Lua callbacks
- Use `is_dispatching` guard for hook re-entrancy prevention

### Reusable Code

| Code                          | Location                                  | Purpose                                         |
| ----------------------------- | ----------------------------------------- | ----------------------------------------------- |
| `cmd_noop`                    | `crates/termcode-term/src/command.rs:274` | Proxy handler for plugin commands (needs `pub`) |
| `sync_cursor_from_selection`  | `crates/termcode-term/src/command.rs:629` | Call after plugin buffer mutations              |
| `runtime_dir()`               | `crates/termcode-config/src/default.rs:9` | Locate `runtime/plugins/`                       |
| `config_dir()`                | `crates/termcode-config/src/default.rs:4` | Default `~/.config/termcode/plugins` path       |
| `Document::apply_transaction` | `crates/termcode-view/src/document.rs:83` | Apply plugin-created transactions               |
| `Transaction` constructors    | `crates/termcode-core/src/transaction.rs` | Plugin buffer mutation API                      |

### Constraints

- `CommandHandler` is `fn` pointer, not closure -- Lua callbacks cannot be wrapped as fn pointers; must use noop+intercept
- `CommandId` is `&'static str` -- runtime plugin command IDs require `Box::leak()`
- `termcode-plugin` (Layer 3) cannot depend on `termcode-term` (Layer 3) -- no circular deps
- `Lua::scope()` lifetime -- Editor reference only valid during scope
- Two separate dispatch paths in App (keybinding line ~507 and palette line ~706) both need `plugin.*` interception
- `cmd_noop` is currently private -- must be made `pub`
- Hook re-entrancy must be prevented with `is_dispatching` flag

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                    | Risk | Description                                                 |
| --------------------------------------- | ---- | ----------------------------------------------------------- |
| `crates/termcode-plugin/src/types.rs`   | Low  | PluginInfo, PluginStatus, HookContext, DeferredAction types |
| `crates/termcode-plugin/src/sandbox.rs` | Low  | Lua VM creation with sandboxing, resource limits            |
| `crates/termcode-plugin/src/hooks.rs`   | Low  | HookEvent enum, HookManager with registration and dispatch  |
| `crates/termcode-plugin/src/api.rs`     | Low  | Lua API registration (editor._, log._) with scoped closures |
| `crates/termcode-plugin/src/manager.rs` | Low  | PluginManager struct, plugin discovery/loading/execution    |
| `runtime/plugins/example/plugin.toml`   | Low  | Example plugin metadata                                     |
| `runtime/plugins/example/init.lua`      | Low  | Example plugin entry point                                  |

### Files to Modify

| File                                   | Risk   | Description                                                                                                                                                                                                                                             |
| -------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/termcode-plugin/Cargo.toml`    | Low    | Add mlua, termcode-core, termcode-config, serde, toml, anyhow, log deps                                                                                                                                                                                 |
| `crates/termcode-plugin/src/lib.rs`    | Low    | Module declarations, re-exports                                                                                                                                                                                                                         |
| `Cargo.toml` (workspace)               | Low    | Add mlua to workspace dependencies                                                                                                                                                                                                                      |
| `crates/termcode-config/src/config.rs` | Medium | Add PluginConfig, PluginOverride structs; add `plugins` field to AppConfig                                                                                                                                                                              |
| `crates/termcode-term/Cargo.toml`      | Low    | Add termcode-plugin dependency                                                                                                                                                                                                                          |
| `crates/termcode-term/src/command.rs`  | Medium | Make `cmd_noop` pub                                                                                                                                                                                                                                     |
| `crates/termcode-term/src/app.rs`      | High   | Add plugin*manager field, init in with_config(), plugin.* interception in handle*key() (2 locations), plugin.* interception in handle_command_palette_key(), hook dispatch at 8 trigger points, deferred action processing, buffer_mutated -> didChange |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/plugin-system`
- No DB migrations or breaking API changes
- All changes are additive (new crate modules, new config section with serde defaults)

## 4. Implementation Order

### Phase 1: Configuration and Types

**Goal**: Establish PluginConfig in termcode-config and type definitions in termcode-plugin
**Risk**: Low
**Status**: Complete

- [x] Task 1.1: Add `PluginConfig` and `PluginOverride` structs to `crates/termcode-config/src/config.rs` with `#[serde(default)]`. Add `plugins: PluginConfig` field to `AppConfig` with `#[serde(default)]`. Ensure existing config parsing is unaffected.
- [x] Task 1.2: Add `mlua` to workspace dependencies in root `Cargo.toml`: `mlua = { version = "0.10", features = ["lua54", "vendored", "serialize"] }`
- [x] Task 1.3: Update `crates/termcode-plugin/Cargo.toml` with all required dependencies (termcode-view, termcode-core, termcode-config, mlua, serde, toml, anyhow, log)
- [x] Task 1.4: Create `crates/termcode-plugin/src/types.rs` -- define `PluginInfo`, `PluginStatus`, `DeferredAction`, `HookContext` structs and the plugin name validation regex `[a-z0-9_-]+`
- [x] Task 1.5: Update `crates/termcode-plugin/src/lib.rs` with module declarations
- [x] Task 1.6: Verify build: `cargo build -p termcode-config -p termcode-plugin`

### Phase 2: Lua Sandbox

**Goal**: Create sandboxed Lua VM with restricted stdlib and resource limits
**Risk**: Low
**Status**: Complete

- [x] Task 2.1: Create `crates/termcode-plugin/src/sandbox.rs` -- implement `create_lua_vm(config: &PluginConfig) -> Result<Lua>` using `Lua::new_with(StdLib::BASE | StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8 | StdLib::OS, LuaOptions::default())`
- [x] Task 2.2: Remove dangerous globals (`loadfile`, `dofile`, `require`) and restrict `os` table to only `clock`, `time`, `date`. Global `require` is replaced by per-plugin `require` in Task 2.4.
- [x] Task 2.3: Set instruction limit via `Lua::set_hook()` with `HookTriggers::every_nth_instruction(N)` and memory limit via `Lua::set_memory_limit(bytes)`
- [x] Task 2.4: Implement per-plugin `require` function with directory-isolated module search and `..` path rejection
- [x] Task 2.5: Write unit tests for sandbox restrictions (os.execute, io.open, debug, loadfile, dofile, require path traversal, instruction limit, memory limit)
- [x] Task 2.6: Verify build: `cargo test -p termcode-plugin`

### Phase 3: Hook System

**Goal**: Implement HookEvent enum and hook registration/dispatch infrastructure
**Risk**: Low
**Status**: Complete

- [x] Task 3.1: Create `crates/termcode-plugin/src/hooks.rs` -- define `HookEvent` enum with all 8 variants (OnOpen, OnSave, OnClose, OnModeChange, OnCursorMove, OnBufferChange, OnTabSwitch, OnReady)
- [x] Task 3.2: Implement hook storage: `HashMap<String, Vec<(String, RegistryKey)>>` mapping hook name to list of (plugin_name, lua_function_ref) pairs
- [x] Task 3.3: Implement `register_hook()` and `dispatch_hook()` methods with `is_dispatching` re-entrancy guard
- [x] Task 3.4: Implement hook context table creation for each event type (path, filename, language, old_mode, new_mode, line, col)
- [x] Task 3.5: Write unit tests for hook registration, dispatch order, re-entrancy guard, error handling
- [x] Task 3.6: Verify build: `cargo test -p termcode-plugin`

### Phase 4: Editor Lua API

**Goal**: Implement the editor._ and log._ Lua API with scoped closures
**Risk**: Medium
**Status**: Complete

- [x] Task 4.1: Create `crates/termcode-plugin/src/api.rs` -- implement `register_editor_api(lua: &Lua)` to create the pre-populated `editor` global table with placeholder methods
- [x] Task 4.2: Implement read-only API methods as scoped closures: `get_mode`, `get_cursor`, `get_selection`, `get_line`, `get_line_count`, `get_filename`, `get_filepath`, `get_status`, `get_theme_name`, `get_config`
- [x] Task 4.3: Implement write API methods as scoped closures: `set_status`, `set_cursor`, `set_selection`
- [x] Task 4.4: Implement buffer-mutating API methods as scoped closures: `insert_text`, `delete_selection`, `buffer_get_text`, `buffer_get_range`, `buffer_replace_range` -- each creates proper Transaction, calls `Document::apply_transaction()`, sets `buffer_mutated` flag
- [x] Task 4.5: Implement deferred action methods: `open_file`, `execute_command` -- append to `Vec<DeferredAction>`, reject `plugin.*` command IDs
- [x] Task 4.6: Implement `log.*` global table: `log.info`, `log.warn`, `log.error`, `log.debug` with `[plugin:{name}]` prefix. Mechanism: set a `_current_plugin_name` Lua global before each plugin's command/hook execution; log functions read this global for the prefix.
- [x] Task 4.7: Implement 1-based to 0-based coordinate conversion, range validation (inclusive start, exclusive end), error handling for out-of-range, nil, wrong-type arguments, "no active document" guard
- [x] Task 4.8: Implement `set_scoped_api(lua, scope, editor, buffer_mutated, deferred_actions)` function that creates scoped closures on the pre-existing `editor` table
- [x] Task 4.9: Write unit tests for API methods (coordinate conversion, range validation, error cases, Transaction creation)
- [x] Task 4.10: Verify build: `cargo test -p termcode-plugin`

### Phase 5: Plugin Manager

**Goal**: Implement PluginManager with plugin discovery, loading, command execution, and hook dispatch
**Risk**: Medium
**Status**: Complete

- [x] Task 5.1: Create `crates/termcode-plugin/src/manager.rs` -- implement `PluginManager` struct with fields: `lua: Lua`, `plugins: Vec<PluginInfo>`, `commands: HashMap<String, RegistryKey>`, `hooks` storage, `buffer_mutated: bool`, `deferred_actions: Vec<DeferredAction>`, `is_dispatching: bool`, `last_cursor_pos: Option<(usize, usize)>`
- [x] Task 5.2: Implement `new(config: PluginConfig) -> Result<Self>` -- create sandboxed VM, register global API tables, set resource limits
- [x] Task 5.3: Implement `load_plugins(dirs: &[PathBuf]) -> Vec<PluginInfo>` -- scan directories in order, alphabetical within each directory, handle duplicates (later takes precedence)
- [x] Task 5.4: Implement `load_plugin(path: &Path) -> Result<PluginInfo>` -- parse plugin.toml (or derive defaults from dir name), validate plugin name, set up per-plugin namespace table with `plugin.name`, `plugin.config`, `plugin.register_command()`, `plugin.on()`, `plugin.require()`, execute init.lua, disable registration after init. Wrap with `std::panic::catch_unwind()` -- on panic: mark plugin as `PluginStatus::Failed(panic_msg)`, log error, continue loading remaining plugins.
- [x] Task 5.5: Implement `execute_command(name: &str, editor: &mut Editor) -> Result<()>` -- reset `buffer_mutated` and `deferred_actions`, use `Lua::scope()` to set scoped API, call stored Lua function, handle errors. Wrap with `std::panic::catch_unwind()` -- on panic: convert to `anyhow::Error`, display in status bar, skip this plugin for future dispatch.
- [x] Task 5.6: Implement `dispatch_hook(hook: HookEvent, editor: &mut Editor) -> Result<()>` -- check `is_dispatching` guard, set scoped API, call all registered hooks with context table, catch per-hook errors, set status bar warning on error. Wrap with `std::panic::catch_unwind()` -- on panic: log error, continue dispatching to remaining plugins' hooks.
- [x] Task 5.7: Implement `list_commands() -> Vec<(String, String)>` and `list_plugins() -> Vec<PluginInfo>`
- [x] Task 5.8: Implement tilde expansion for `plugin_dirs` paths and per-plugin override checking (`PluginConfig.overrides`)
- [x] Task 5.9: Write unit tests for plugin discovery, loading, command execution, hook dispatch, error handling
- [x] Task 5.10: Update `crates/termcode-plugin/src/lib.rs` with all re-exports
- [x] Task 5.11: Verify build: `cargo test -p termcode-plugin`

### Phase 6: App Integration

**Goal**: Integrate PluginManager into App with command interception, hook dispatch, and deferred actions
**Risk**: High
**Status**: Complete

- [x] Task 6.1: Make `cmd_noop` public in `crates/termcode-term/src/command.rs` (change `fn cmd_noop` to `pub fn cmd_noop`)
- [x] Task 6.2: Add `termcode-plugin` dependency to `crates/termcode-term/Cargo.toml`
- [x] Task 6.3: Add `plugin_manager: Option<PluginManager>` field to `App` struct in `crates/termcode-term/src/app.rs`
- [x] Task 6.4: Initialize PluginManager in `App::with_config()`: check `plugins.enabled`, create PluginManager, collect plugin dirs (runtime_dir/plugins + config plugin_dirs), call `load_plugins()`, register proxy commands in CommandRegistry using `cmd_noop`, re-call `input_mapper.apply_overrides()` for keybinding re-validation
- [x] Task 6.5: Extract a unified `dispatch_command(&mut self, cmd_id: &str)` method on App that handles: (1) `plugin.*` interception → `plugin_manager.execute_command()`, (2) App-level intercepts (palette.open, goto.definition, lsp.hover, etc.), (3) normal `CommandRegistry::execute()`. After execution: process deferred actions, check `buffer_mutated` for `lsp_notify_did_change()`.
- [x] Task 6.6: Refactor `handle_key()` global resolve block (after line ~425) to use `dispatch_command()`
- [x] Task 6.7: Refactor `handle_key()` mode resolve block (after line ~507) to use `dispatch_command()`
- [x] Task 6.8: Refactor `handle_command_palette_key()` (after line ~706) to use `dispatch_command()`
- [x] Task 6.9: Implement `process_deferred_actions()` helper on App: **drain-once policy** -- drain the deferred actions list, process each action, do NOT re-enter if processing produces new deferred actions (log warning if new actions generated). Handle `OpenFile` (call `self.open_file()`), handle `ExecuteCommand` (route through `dispatch_command()` for proper App-level interception), display errors in status bar.
- [x] Task 6.10a: Add state-snapshot plumbing to event loop in `App::run()` -- capture `editor.mode`, active tab ID, and cursor position **before** event processing each iteration. State-diff comparison occurs **after** all explicit callsite hooks (6.10b) have completed within the same iteration, to avoid double-firing hooks triggered by hook side-effects. Dispatch state-diff hooks:
  - `on_mode_change`: if mode changed, dispatch with `{old_mode, new_mode}`
  - `on_tab_switch`: if active tab ID changed, dispatch with `{path, filename}`
  - `on_cursor_move`: if cursor position changed, dispatch with `{line, col}`
- [x] Task 6.10b: Add explicit callsite hook dispatch at 5 trigger points:
  - `on_open`: after `self.open_file()` succeeds
  - `on_save`: after `self.lsp_notify_did_save()`
  - `on_close`: before tab close in `handle_close_tab()` (document tabs only)
  - `on_buffer_change`: after `self.lsp_notify_did_change()` and after plugin `buffer_mutated` handling
  - `on_ready`: at end of plugin initialization
- [x] Task 6.11: After plugin execution/hook dispatch, check `buffer_mutated` flag and: (a) call `lsp_notify_did_change()`, (b) call `sync_cursor_from_selection()` on the active document to sync cursor/view state. This is the App-layer responsibility since `sync_cursor_from_selection` lives in `termcode-term`.
- [x] Task 6.12: Verify full build: `cargo build --workspace`
- [x] Task 6.13: Verify all existing tests pass: `cargo test --workspace`
- [x] Task 6.13a: Verify dispatch_command() refactor preserves existing behavior -- manual test that palette.open, goto.definition, lsp.hover, file.save still work correctly through the unified dispatch path
- [x] Task 6.14: Verify lint: `cargo clippy --workspace`

### Phase 7: Example Plugin and Final Tests

**Goal**: Create example plugin and comprehensive tests
**Risk**: Low
**Status**: Complete

- [x] Task 7.1: Create `runtime/plugins/example/plugin.toml` with metadata (name, version, description, author)
- [x] Task 7.2: Create `runtime/plugins/example/init.lua` with example commands (wrap_quotes, insert_date) and example hooks (on_save logging)
- [x] Task 7.3: Write integration-style tests in termcode-plugin covering all spec test scenarios:
  - TS-1: Sandbox restrictions (os.execute, io, debug, loadfile, require traversal, per-plugin module cache, global require blocked)
  - TS-2: Command registration and execution round-trip (palette, keybinding, error display)
  - TS-3: Hook dispatch with context validation (all 8 hooks, multi-plugin order, re-entrancy skip)
  - TS-4: Error recovery (syntax error → Failed, runtime error → Failed, instruction limit, memory limit with isolated config)
  - TS-5: API boundary validation (coordinate conversion, range validation, reversed/empty range, get_line no newline, deferred action ordering, plugin.\* rejection)
  - TS-6: Plugin metadata and config (plugin.toml parsing, missing toml, invalid name, tilde expansion, non-existent dir)
  - TS-7: Logging API (info/warn/error/debug levels, non-string tostring conversion)
  - TS-8: Crate structure (`cargo tree -p termcode-plugin` no termcode-term dependency)
  - TS-9: Lifecycle edge cases (init-once, list_plugins 3 statuses, editor-outside-scope error, registration-outside-init error, deferred-action failure continuation, duplicate plugin name skip, panic containment -- panicking plugin does not prevent other plugins from executing)
- [x] Task 7.4: Test with manual run: `cargo run -- .` -- verify plugin loads, commands appear in palette, hooks fire
- [x] Task 7.5: Create `docs/plugin-guide.md` -- user-facing plugin documentation covering: plugin directory structure, plugin.toml format, Lua API reference (editor._ and log._ methods with signatures), hook names and context fields, config.toml [plugins] section options, sandbox restrictions
- [x] Task 7.6: Final verification: `cargo build --workspace && cargo test --workspace && cargo clippy --workspace && cargo fmt --check`

## 5. Quality Gate

- [ ] Build success: `cargo build --workspace`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Lint pass: `cargo clippy --workspace` (0 warnings)
- [ ] Format check: `cargo fmt --check`
- [ ] Example plugin loads without errors
- [ ] Plugin commands appear in command palette
- [ ] Plugin hooks fire at correct trigger points
- [ ] Sandbox restrictions enforced (os.execute, io, debug blocked)
- [ ] Existing functionality unaffected (config parsing, keybindings, LSP)

## 6. Notes

### Implementation Considerations

- The `mlua` crate with `vendored` feature compiles Lua 5.4 from source, adding build time. This is a one-time cost.
- `Box::leak()` for CommandId/name strings is acceptable because plugins only load at startup. If runtime reload is added in a future phase, this must be revisited.
- The `editor` global table is pre-created at VM init with placeholders. During `Lua::scope()`, methods are temporarily replaced with scoped closures. This is the recommended mlua pattern for safely passing mutable references.
- `on_cursor_move` uses position-change detection (not time-based debouncing) to prevent performance issues. The `last_cursor_pos` is stored on PluginManager and compared once per render frame.

### Patterns to Avoid (from Analysis Report)

- Do NOT store `Editor` reference in Lua VM permanently -- must use `Lua::scope()` for borrow safety
- Do NOT make plugin commands closures -- `CommandHandler` is `fn` pointer only; use noop + intercept
- Do NOT add termcode-term dependency to termcode-plugin -- violates layer boundaries
- Do NOT use global Lua `require` -- must be per-plugin with isolated module cache
- Do NOT fire hooks recursively -- use `is_dispatching` guard

### Technical Debt Addressed

- `cmd_noop` visibility change from private to public (Phase 6, Task 6.1)
- Palette dispatch path needs plugin interception alongside existing `theme.list` check (Phase 6, Task 6.7)

### Potential Issues

- `mlua` `set_hook` for instruction counting may have performance overhead on hot paths (hooks firing frequently). Mitigation: `on_cursor_move` is already frame-deduplicated, and instruction limits only apply during active Lua execution.
- Build time increase from `mlua` vendored Lua compilation. Mitigation: one-time compile, cached by cargo.
- Single shared Lua VM means one plugin's memory usage affects others. Noted as known limitation in spec; per-plugin VMs may be considered in a future phase.

## 7. Implementation Notes

### Phase 1 (2026-03-29)

- Created: 1 file (`crates/termcode-plugin/src/types.rs`)
- Modified: 4 files (`Cargo.toml`, `crates/termcode-config/src/config.rs`, `crates/termcode-plugin/Cargo.toml`, `crates/termcode-plugin/src/lib.rs`)
- Risk: Low
- Notes: Used manual byte-matching instead of regex for name validation to avoid adding regex dependency. PluginConfig defaults to `enabled: false` for safety. All existing tests pass.

### Phase 2 (2026-03-29)

- Created: 1 file (`crates/termcode-plugin/src/sandbox.rs`)
- Modified: 1 file (`crates/termcode-plugin/src/lib.rs`)
- Risk: Low
- Notes: Used `map_err(lua_err)` pattern to convert `mlua::Error` to `anyhow::Error` since mlua is compiled without the `send` feature (Error doesn't implement Send+Sync). StdLib has no BASE constant -- base globals are loaded automatically by `new_with`. All 12 tests pass including sandbox restrictions, instruction limit, memory limit, and path traversal rejection.

### Phase 3 (2026-03-29)

- Created: 1 file (`crates/termcode-plugin/src/hooks.rs`)
- Modified: 2 files (`crates/termcode-plugin/src/lib.rs`, `docs/plans/plugin-system.md`)
- Risk: Low
- Notes: HookEvent uses rich enum variants carrying event-specific data, with `name()` and `to_context()` conversion methods. HookManager stores callbacks as `RegistryKey` refs in registration order per hook name. mlua 0.10 Table has no lifetime parameter. All 25 tests pass (13 new hook tests).

### Phase 4 (2026-03-29)

- Created: 1 file (`crates/termcode-plugin/src/api.rs`)
- Modified: 2 files (`crates/termcode-plugin/src/lib.rs`, `crates/termcode-plugin/Cargo.toml`)
- Risk: Medium
- Notes: Used thread-local storage with raw pointers instead of `Lua::scope()` to avoid lifetime issues with `create_function` requiring `'static`. The `with_scoped_api()` function sets the editor pointer before Lua execution and clears it immediately after, ensuring safety through single-threaded synchronous execution. Methods error with a clear message when called outside scope. Added `termcode-syntax` and `termcode-theme` as dev-dependencies for test helpers. All 44 tests pass (14 new API tests).

### Phase 5 (2026-03-29)

- Created: 1 file (`crates/termcode-plugin/src/manager.rs`)
- Modified: 4 files (`crates/termcode-plugin/src/lib.rs`, `crates/termcode-plugin/Cargo.toml`, `crates/termcode-plugin/src/hooks.rs`, `crates/termcode-config/src/config.rs`)
- Risk: Medium
- Notes: PluginManager uses named registry values to pass Lua function references between init-time callbacks and host-side collection, avoiding the need for `create_any_userdata` on `RegistryKey`. Command registration uses `plugin.register_command(name, desc, fn)` with a registration flag that disables after init. Hook registration similarly uses `plugin.on(hook_name, fn)`. Plugin commands return `(bool, Vec<DeferredAction>)` for the App layer to process buffer mutations and deferred actions. Fixed pre-existing clippy warnings (HookManager Default impl, PluginOverride derive). Added `dirs` and `tempfile` dependencies. All 67 tests pass (23 new manager tests).

### Phase 6 (2026-03-29)

- Created: 0 files
- Modified: 3 files (`crates/termcode-term/src/app.rs`, `crates/termcode-term/Cargo.toml`, `crates/termcode-term/src/command.rs`)
- Risk: High
- Notes: Extracted unified `dispatch_command()` method replacing 3 duplicated dispatch patterns (global resolve, mode resolve, palette execute). Used `get_by_string()` instead of `execute()` to avoid `&'static str` lifetime requirement for dynamically-constructed command IDs. Plugin commands intercepted via `starts_with("plugin.")` before registry fallback. State-diff hooks (mode change, tab switch, cursor move) fire after event processing in the main loop. Explicit callsite hooks (on_open, on_save, on_close, on_buffer_change, on_ready) fire at 8 trigger points. All 180 existing tests pass, zero clippy warnings.

### Phase 7 (2026-03-29)

- Created: 4 files (`runtime/plugins/example/plugin.toml`, `runtime/plugins/example/init.lua`, `crates/termcode-plugin/tests/integration.rs`, `docs/plugin-guide.md`)
- Modified: 1 file (`docs/plans/plugin-system.md`)
- Risk: Low
- Notes: 38 integration tests covering all 9 spec test scenarios (TS-1 through TS-9). Example plugin demonstrates wrap-quotes command, insert-date command, on_save hook, and on_ready hook. Plugin guide covers directory structure, plugin.toml format, full Lua API reference, hook names/context fields, config.toml options, and sandbox restrictions. cargo fmt fixed pre-existing import ordering issues. All 218 workspace tests pass, zero clippy warnings.

---

Last Updated: 2026-03-29
Status: Complete (Phase 7/7 complete)
