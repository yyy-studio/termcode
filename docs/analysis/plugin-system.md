# Plugin System Analysis Report

**Analysis Date**: 2026-03-29
**Analysis Target**: Phase 5 Plugin System - Lua-based plugin system using mlua
**Related Specs**: FR-PLUGIN-001 through FR-PLUGIN-011

## 1. Related Code Exploration Results

### Similar Features

| File | Feature | Reference Reason |
| --- | --- | --- |
| `crates/termcode-term/src/command.rs` | CommandRegistry + CommandEntry | Plugin commands must integrate with this exact pattern (noop handler + App interception) |
| `crates/termcode-term/src/app.rs` (lines 508-536) | `handle_key()` App-level interception | `goto.definition`, `lsp.hover`, `palette.open` use the noop-then-intercept pattern; plugin commands follow the same approach |
| `crates/termcode-term/src/app.rs` (lines 696-723) | Command palette execution path | Second dispatch path that must also intercept `plugin.*` commands |
| `crates/termcode-term/src/lsp_bridge.rs` | LspBridge async pattern | Channel-based bridge between async and sync; plugin system is sync but follows similar ownership model |
| `crates/termcode-config/src/config.rs` | AppConfig with serde defaults | `PluginConfig` must follow this exact pattern (`#[serde(default)]` on struct) |
| `crates/termcode-term/src/input.rs` (lines 154-210) | `apply_overrides()` keybinding validation | Must re-validate after plugin commands are registered so `plugin.*` IDs are recognized |

### Reusable Code

| File | Target | Usage |
| --- | --- | --- |
| `crates/termcode-term/src/command.rs:274` | `cmd_noop()` | Reuse as proxy handler for plugin commands |
| `crates/termcode-term/src/command.rs:697-713` | `insert_char()` pattern | Reference for creating Transactions from plugin API `insert_text()` |
| `crates/termcode-term/src/command.rs:629-644` | `sync_cursor_from_selection()` | Must call after plugin buffer mutations |
| `crates/termcode-config/src/default.rs:9-19` | `runtime_dir()` | Reuse for locating `runtime/plugins/` directory |
| `crates/termcode-config/src/default.rs:4-6` | `config_dir()` | Reuse for default `~/.config/termcode/plugins` path |
| `crates/termcode-view/src/document.rs:83-95` | `Document::apply_transaction()` | Plugin buffer mutations must go through this exact method |

### Related Type Definitions

| File | Type | Description |
| --- | --- | --- |
| `crates/termcode-term/src/command.rs:11-12` | `CommandId = &'static str`, `CommandHandler = fn(&mut Editor) -> Result<()>` | Plugin commands need `Box::leak` for dynamic `CommandId` strings |
| `crates/termcode-term/src/command.rs:14-18` | `CommandEntry { id, name, handler }` | Plugin proxy entries use this struct with `cmd_noop` handler |
| `crates/termcode-view/src/editor.rs:22-29` | `EditorMode` enum | Needed for `editor.get_mode()` Lua API |
| `crates/termcode-view/src/editor.rs:57-79` | `Editor` struct | Primary API surface for Lua; all fields are `pub` |
| `crates/termcode-view/src/document.rs:13` | `DocumentId(pub usize)` | Needed for document lookup |
| `crates/termcode-core/src/config_types.rs:7-16` | `EditorConfig` | Needed for `editor.get_config()` Lua API |
| `crates/termcode-core/src/transaction.rs` | `Transaction`, `ChangeSet`, `Operation` | Plugin buffer mutations create Transactions |
| `crates/termcode-core/src/selection.rs` | `Selection` | Plugin cursor/selection operations |
| `crates/termcode-core/src/position.rs` | `Position { line, column }` | Coordinate system (0-based internally, 1-based in Lua) |
| `crates/termcode-view/src/image.rs` | `TabContent` enum | Needed to determine if active tab is document vs image (hooks skip non-document tabs) |

### Code Flow (Command Execution - Reference Pattern)

```
[KeyEvent] --> [InputMapper.resolve(mode, key)] --> CommandId
  --> [App::handle_key()] intercepts special commands
    --> "palette.open" -> App::open_command_palette()
    --> "goto.definition" -> App::request_definition()
    --> "lsp.hover" -> App::request_hover()
    --> other -> CommandRegistry.execute(id, &mut editor)
```

| Step | File | Function | Description |
| --- | --- | --- | --- |
| Key Input | `app.rs:381` | `handle_key()` | Receives KeyEvent |
| Global resolve | `input.rs:129` | `resolve_global()` | Check global keybindings first |
| App intercept | `app.rs:426-439` | global match block | Intercepts `palette.open`, `file.save` etc. |
| Mode resolve | `input.rs:137` | `resolve()` | Check mode-specific keybindings |
| App intercept | `app.rs:508-524` | mode match block | Intercepts `goto.definition`, `lsp.hover` etc. |
| Execute | `command.rs:35` | `CommandRegistry::execute()` | Calls `(entry.handler)(editor)` |
| LSP notify | `app.rs:527-535` | post-execution | Sends `didChange`/`didSave` if mutation/save |

### Code Flow (Command Palette Execution - Second Dispatch Path)

```
[Palette Enter] --> [selected_command()] --> id String
  --> App::handle_command_palette_key() intercepts special IDs
    --> "theme.list" -> open_theme_palette()
    --> other -> registry.get_by_string(&id).handler(editor)
```

| Step | File | Function | Description |
| --- | --- | --- | --- |
| Enter key | `app.rs:696` | `handle_command_palette_key()` | Enter pressed in palette |
| Get selection | `app.rs:699-703` | `selected_command()` | Gets `PaletteItem { id, name }` |
| Intercept | `app.rs:706-708` | `if id == "theme.list"` | Special case handling |
| Lookup | `app.rs:712` | `get_by_string(&id)` | Finds entry, gets handler fn ptr |
| Execute | `app.rs:714` | `handler(&mut self.editor)` | Calls handler directly (not through registry.execute) |

**Critical**: This path calls the handler fn pointer directly. For plugin commands, this will call `cmd_noop` (does nothing). Must add `plugin.*` interception here too.

## 2. Impact Scope Analysis

### Directly Affected Files

| File | Change Type | Risk |
| --- | --- | --- |
| `crates/termcode-plugin/src/lib.rs` | **New** (all 6 modules) | Low - greenfield |
| `crates/termcode-plugin/Cargo.toml` | Modify (add dependencies) | Low |
| `crates/termcode-config/src/config.rs` | Modify (add `PluginConfig` to `AppConfig`) | Medium - must not break existing config parsing |
| `crates/termcode-term/src/app.rs` | Modify (plugin integration in `new()`, `handle_key()`, palette, hooks) | High - core event loop |
| `crates/termcode-term/Cargo.toml` | Modify (add `termcode-plugin` dependency) | Low |
| `runtime/plugins/example/init.lua` | **New** | Low |
| `runtime/plugins/example/plugin.toml` | **New** | Low |
| `Cargo.toml` (workspace) | Possibly modify (add mlua to workspace deps) | Low |

### Indirectly Affected Files

| File | Impact Reason |
| --- | --- |
| `crates/termcode-term/src/command.rs` | Need to export `cmd_noop` (currently `fn`, not `pub fn`) |
| `crates/termcode-term/src/input.rs` | `apply_overrides()` must be called again after plugin commands registered |
| `crates/termcode-config/Cargo.toml` | May need `dirs` crate (already present) for tilde expansion |

### Test Impact

| Test File | Status |
| --- | --- |
| `crates/termcode-plugin/` (all) | **New** - comprehensive unit tests needed |
| `crates/termcode-config/` | Needs tests for `PluginConfig` parsing/defaults |
| Existing tests | Should remain passing (no behavioral changes to existing code) |

## 3. Architecture Analysis

### Current Structure

```
[App (termcode-term)]
  |-- editor: Editor (termcode-view)
  |-- command_registry: CommandRegistry
  |-- input_mapper: InputMapper
  |-- lsp_bridge: Option<LspBridge>
  |-- (NEW) plugin_manager: Option<PluginManager>
```

### Extension Points

- **`App` struct**: Add `plugin_manager: Option<PluginManager>` field
- **`App::with_config()`**: Initialize PluginManager after CommandRegistry setup, register plugin commands, re-validate keybindings
- **`App::handle_key()`**: Add `plugin.*` prefix check before normal command dispatch (two locations: global resolve block at line ~425, mode resolve block at line ~507)
- **`App::handle_command_palette_key()`**: Add `plugin.*` prefix check at line ~706 before calling handler directly
- **`App::run()` event loop**: Add hook dispatch calls at appropriate points (save, close tab, mode change, cursor move, buffer change, tab switch)
- **`AppConfig`**: Add `plugins: PluginConfig` field with `#[serde(default)]`

### Constraints

1. **`CommandHandler` is `fn` pointer, not closure**: Plugin Lua callbacks cannot be wrapped as `fn` pointers. Must use the noop-handler-with-interception pattern (already proven for `goto.definition`, `lsp.hover`).

2. **`CommandId` is `&'static str`**: Plugin command IDs are runtime strings. Must use `Box::leak()` to get `&'static str`. Safe because plugins load only at startup (no runtime reload in Phase 5).

3. **`termcode-plugin` (Layer 3) cannot depend on `termcode-term` (Layer 3)**: No circular dependencies. `PluginManager` accepts `&mut Editor` as parameter, not `&mut App`. All App-level operations (LSP notifications, clipboard, file opening) use deferred actions.

4. **`Lua::scope()` lifetime**: Editor reference only valid during scope. Scoped closures are set on pre-created `editor` table, invalidated after scope ends.

5. **`cmd_noop` must be exported**: Currently private in `command.rs`. Need `pub fn cmd_noop`.

6. **Palette dispatch path**: At line 712-714, the palette calls `handler()` directly (not through `registry.execute()`). This bypasses any future middleware. Must add explicit `plugin.*` check.

7. **Hook re-entrancy**: `dispatch_hook()` sets `is_dispatching = true`. If a hook triggers a buffer mutation, `on_buffer_change` is suppressed. After `dispatch_hook()` returns, App checks `buffer_mutated` flag for LSP `didChange`.

### Cross-cutting Concerns

| Concern | Pattern | Files |
| --- | --- | --- |
| Error Handling | `anyhow::Result` throughout; status bar for user-facing errors | `app.rs`, `command.rs` |
| Logging | `log` crate (`log::warn!`, `log::info!`) | All crates |
| Configuration | TOML + serde with `#[serde(default)]` + `AppConfig::load()` | `config.rs` |
| LSP Notifications | `lsp_notify_did_change()` after mutations | `app.rs:1010-1029` |
| Transaction Pattern | inverse before apply, commit to history, version++ | `document.rs:83-95` |

### Component Interfaces

| From | To | Contract |
| --- | --- | --- |
| App | PluginManager | `execute_command(name: &str, editor: &mut Editor) -> Result<()>` |
| App | PluginManager | `dispatch_hook(hook: HookEvent, editor: &mut Editor) -> Result<()>` |
| App | PluginManager | `list_commands() -> Vec<(String, String)>` (for CommandRegistry proxy registration) |
| App | PluginManager | `list_plugins() -> Vec<PluginInfo>` |
| PluginManager | Editor | `&mut Editor` passed into `Lua::scope()` closures |
| PluginManager | Lua VM | `mlua::Lua` owned by PluginManager |
| Lua callbacks | Editor API | Scoped closures on `editor` global table |
| PluginManager | App | `buffer_mutated: bool` and `deferred_actions: Vec<DeferredAction>` (checked by App after execution) |

## 4. Technical Considerations

### Performance
- Plugin loading is startup-only; target <100ms for 50 plugins
- `on_cursor_move` fires once per frame only when position changes (not per keystroke)
- Instruction limit (default 1M) prevents runaway plugins
- Memory limit (default 64MB) prevents memory exhaustion
- Single Lua VM shared across all plugins (no per-plugin VM overhead)

### Security
- Sandbox: no `io`, `debug`, `os.execute`, `loadfile`, `dofile`
- Per-plugin `require` with directory isolation and `..` path rejection
- Resource limits (instructions, memory)
- No direct file I/O; only `editor.open_file()` deferred action

### Migration
- DB schema change required: No
- Data migration required: No
- Config file change: Additive only (`[plugins]` section with serde defaults)
- No breaking changes to existing behavior

### mlua Crate

The spec calls for `mlua = { version = "0.10", features = ["lua54", "vendored", "serialize"] }`. Key API points:
- `Lua::new_with(StdLib, LuaOptions)` for selective stdlib loading
- `Lua::scope()` for safely passing `&mut Editor` to callbacks
- `Lua::set_hook()` with `HookTriggers::every_nth_instruction()` for instruction limits
- `Lua::set_memory_limit()` for memory limits
- `Lua::create_registry_value()` / `RegistryKey` for storing Lua function references

## 5. Recommendations

### Recommended Approach

1. **Start with termcode-config**: Add `PluginConfig` struct to `config.rs` with serde defaults
2. **Build termcode-plugin crate**: Implement modules in order: `types.rs` -> `sandbox.rs` -> `hooks.rs` -> `api.rs` -> `manager.rs` -> `lib.rs`
3. **Integrate into termcode-term**: Add `PluginManager` to `App`, plugin command registration, handle_key interception, palette interception, hook dispatch calls
4. **Create example plugin**: `runtime/plugins/example/` with `plugin.toml` and `init.lua`
5. **Tests**: Unit tests in termcode-plugin, integration-style tests for sandbox and API

### Patterns to Avoid

- **Do NOT store `Editor` reference in Lua VM permanently** -- must use `Lua::scope()` for borrow safety
- **Do NOT make plugin commands closures** -- `CommandHandler` is `fn` pointer only; use noop + intercept
- **Do NOT add termcode-term dependency to termcode-plugin** -- violates layer boundaries
- **Do NOT use global Lua `require`** -- must be per-plugin with isolated module cache
- **Do NOT fire hooks recursively** -- use `is_dispatching` guard

### Recommended Reusable Code

| Code | Location | Purpose |
| --- | --- | --- |
| `cmd_noop` | `command.rs:274` | Proxy handler for plugin commands (needs `pub`) |
| `sync_cursor_from_selection` | `command.rs:629` | Call after plugin buffer mutations |
| `runtime_dir()` | `config/default.rs:9` | Locate `runtime/plugins/` |
| `config_dir()` | `config/default.rs:4` | Default `~/.config/termcode/plugins` path |
| `Transaction::insert/delete/replace` | `core/transaction.rs` | Plugin buffer mutation API |
| `Document::apply_transaction` | `view/document.rs:83` | Apply plugin-created transactions |

## 6. Technical Debt

### Identified Issues

| Area | Issue | Recommendation | Priority |
| --- | --- | --- | --- |
| `command.rs:274` | `cmd_noop` is private | Make `pub` for plugin proxy registration | High (blocking) |
| `app.rs:712-714` | Palette executes handler directly, bypassing any registry middleware | Add `plugin.*` check; consider unifying both dispatch paths in future | Medium |
| `app.rs:425-439` | Global keybinding dispatch and mode keybinding dispatch are separate code paths with duplicated interception logic | Consider extracting common dispatch function | Low |
| `command.rs` | `CommandEntry.name` is `&'static str` | Plugin names need `Box::leak` too; consistent with `CommandId` approach | Medium (already in spec) |

### Improvement Opportunities

- The two command dispatch paths (keybinding and palette) could be unified into a single `dispatch_command()` method on App, which would simplify plugin interception
- Consider extracting `is_document_mutation()` into a more extensible pattern (e.g., command metadata) for plugin commands that mutate buffers

## 7. Design Recommendation

**design_recommendation**: none

**Rationale**: The spec (plugin-system.md) is extremely detailed with complete architecture, data flow, API surface, pseudocode, and acceptance criteria. All design decisions are already made: crate structure, Lua VM ownership pattern (`Lua::scope()`), command integration (noop+intercept), hook system, sandbox configuration, deferred actions, and config schema. No additional architecture or UX design is needed. This can proceed directly to implementation planning.

## 8. Essential Files (Must Read)

Files that MUST be read before proceeding to yyy-plan:

| File | Reason | Priority |
| --- | --- | --- |
| `docs/specs/plugin-system.md` | Complete spec with all FRs, technical design, edge cases, test scenarios | Required |
| `crates/termcode-term/src/app.rs` | App struct, event loop, `handle_key()`, palette dispatch, hook integration points, LSP notification pattern | Required |
| `crates/termcode-term/src/command.rs` | CommandRegistry, CommandEntry, CommandHandler types, `cmd_noop`, Transaction patterns | Required |
| `crates/termcode-config/src/config.rs` | AppConfig structure, serde pattern for adding PluginConfig | Required |
| `crates/termcode-view/src/editor.rs` | Editor struct (full API surface for Lua bindings), EditorMode enum | Required |
| `crates/termcode-view/src/document.rs` | Document struct, `apply_transaction()` pattern (inverse + apply + history) | Required |
| `crates/termcode-plugin/Cargo.toml` | Current dependencies (only termcode-view), needs expansion | Recommended |
| `crates/termcode-term/src/input.rs` | InputMapper, `apply_overrides()` for re-validation after plugin load | Recommended |
| `crates/termcode-core/src/config_types.rs` | EditorConfig fields for `editor.get_config()` Lua API | Recommended |
| `crates/termcode-core/src/transaction.rs` | Transaction constructors (insert, delete, replace) for plugin buffer ops | Recommended |
| `crates/termcode-config/src/default.rs` | `runtime_dir()` and `config_dir()` for plugin discovery paths | Recommended |
| `Cargo.toml` (workspace) | Workspace dependency declarations for adding mlua | Recommended |

## 9. Handoff to yyy-plan

### Notes

- The spec is comprehensive; implementation can follow it closely with minimal ambiguity
- `cmd_noop` in `command.rs` must be made `pub` before plugin proxy registration works
- Two separate dispatch paths in App (keybinding at line ~507 and palette at line ~706) both need `plugin.*` interception
- After plugin commands are registered in CommandRegistry, `InputMapper::apply_overrides()` must be re-called for user keybindings to recognize `plugin.*` command IDs
- `buffer_mutated` flag and `deferred_actions` vector are owned by PluginManager and checked by App after each `execute_command()` / `dispatch_hook()` call
- Hook dispatch points must be identified in App: save (after `lsp_notify_did_save`), close tab (in `handle_close_tab`), mode change (after `switch_mode`), tab switch (after `sync_active_view_to_tab`), buffer change (after `lsp_notify_did_change`), cursor move (once per frame in event loop), file open (after `open_file`), ready (after all plugins loaded)

### Recommended Phase Structure

1. **Phase 1: Foundation** - `PluginConfig` in termcode-config, types/sandbox/hooks modules in termcode-plugin, Cargo.toml updates
2. **Phase 2: Core Plugin Engine** - `PluginManager`, plugin discovery/loading, Lua API registration (`api.rs`)
3. **Phase 3: Command Integration** - Plugin command registration in CommandRegistry, App interception in both dispatch paths, keybinding re-validation
4. **Phase 4: Hook System** - Hook dispatch from App at all trigger points, re-entrancy guard, `on_cursor_move` frame-based dedup
5. **Phase 5: Deferred Actions & LSP Bridge** - `DeferredAction` processing, `buffer_mutated` flag for LSP `didChange`, `cmd_noop` pub export
6. **Phase 6: Example Plugin & Tests** - Example plugin in runtime/plugins/example/, comprehensive unit tests
