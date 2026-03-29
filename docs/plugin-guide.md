# Plugin Guide

This guide covers how to create and configure plugins for termcode.

## Plugin Directory Structure

Plugins are discovered from plugin directories. Each plugin lives in its own subdirectory:

```
runtime/plugins/
  my-plugin/
    plugin.toml    # Optional metadata manifest
    init.lua       # Required entry point
    helpers.lua    # Optional modules (loaded via plugin.require)
```

Default plugin directories:

- `runtime/plugins/` (bundled with termcode)
- Additional directories configured via `config.toml`

## plugin.toml Format

The `plugin.toml` manifest is optional. If missing, defaults are derived from the directory name.

```toml
name = "my-plugin"           # Plugin name (must match [a-z0-9_-]+)
version = "1.0.0"            # Version string (default: "0.1.0")
description = "My plugin"    # Description shown in plugin list
author = "Your Name"         # Author name
```

If `name` is omitted, the directory name is used.

## init.lua Entry Point

The `init.lua` file is executed once during startup. Use it to register commands and hooks.

```lua
-- Register a command (available in command palette)
plugin.register_command("greet", "Say hello", function()
    editor.set_status("Hello from " .. plugin.name .. "!")
end)

-- Register a hook
plugin.on("on_save", function(ctx)
    log.info("Saved: " .. (ctx.filename or "untitled"))
end)

-- Load a local module
local utils = plugin.require("utils")
```

**Registration is only allowed during init.** Calling `plugin.register_command()` or `plugin.on()` inside a command callback will error.

## Lua API Reference

### editor (Read-Only)

| Method           | Signature                                                 | Description                                                                                                       |
| ---------------- | --------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `get_mode`       | `() -> string`                                            | Current editor mode: `"normal"`, `"insert"`, `"file_explorer"`, `"search"`, `"fuzzy_finder"`, `"command_palette"` |
| `get_cursor`     | `() -> {line, col}`                                       | Cursor position (1-based)                                                                                         |
| `get_selection`  | `() -> {start_line, start_col, end_line, end_col} or nil` | Primary selection range (1-based), nil if point selection                                                         |
| `get_line`       | `(line: integer) -> string`                               | Line content at given line number (1-based, no trailing newline)                                                  |
| `get_line_count` | `() -> integer`                                           | Total number of lines in the buffer                                                                               |
| `get_filename`   | `() -> string or nil`                                     | Filename of active document, nil if unsaved                                                                       |
| `get_filepath`   | `() -> string or nil`                                     | Full file path of active document                                                                                 |
| `get_status`     | `() -> string or nil`                                     | Current status bar message                                                                                        |
| `get_theme_name` | `() -> string`                                            | Name of the active theme                                                                                          |
| `get_config`     | `() -> {tab_size, scroll_off}`                            | Editor configuration values                                                                                       |

### editor (Write)

| Method          | Signature                                        | Description                       |
| --------------- | ------------------------------------------------ | --------------------------------- |
| `set_status`    | `(msg: string)`                                  | Set the status bar message        |
| `set_cursor`    | `(line: integer, col: integer)`                  | Move cursor to position (1-based) |
| `set_selection` | `(anchor_line, anchor_col, head_line, head_col)` | Set selection range (1-based)     |

### editor (Buffer Mutation)

| Method                 | Signature                                                  | Description                                                 |
| ---------------------- | ---------------------------------------------------------- | ----------------------------------------------------------- |
| `insert_text`          | `(text: string)`                                           | Insert text at current cursor position                      |
| `delete_selection`     | `()`                                                       | Delete the primary selection range                          |
| `buffer_get_text`      | `() -> string`                                             | Get entire buffer content                                   |
| `buffer_get_range`     | `(start_line, start_col, end_line, end_col) -> string`     | Get text in range (1-based, inclusive start, exclusive end) |
| `buffer_replace_range` | `(start_line, start_col, end_line, end_col, text: string)` | Replace text in range                                       |

### editor (Deferred Actions)

These actions are queued and processed after the Lua callback returns:

| Method            | Signature              | Description                                      |
| ----------------- | ---------------------- | ------------------------------------------------ |
| `open_file`       | `(path: string)`       | Open a file in a new tab                         |
| `execute_command` | `(command_id: string)` | Execute a built-in command (e.g., `"file.save"`) |

Calling `execute_command` with a `plugin.*` command ID is rejected to prevent recursion.

### log

| Method      | Signature | Description        |
| ----------- | --------- | ------------------ |
| `log.info`  | `(msg)`   | Log at INFO level  |
| `log.warn`  | `(msg)`   | Log at WARN level  |
| `log.error` | `(msg)`   | Log at ERROR level |
| `log.debug` | `(msg)`   | Log at DEBUG level |

All log methods accept any value type (non-strings are converted via `tostring()`). Log messages are prefixed with `[plugin:name]`.

### plugin (Init-Time Only)

| Method                    | Signature                       | Description                                 |
| ------------------------- | ------------------------------- | ------------------------------------------- |
| `plugin.name`             | `string`                        | The plugin's name                           |
| `plugin.config`           | `table`                         | Per-plugin configuration from `config.toml` |
| `plugin.register_command` | `(name, description, callback)` | Register a command                          |
| `plugin.on`               | `(hook_name, callback)`         | Register a hook listener                    |
| `plugin.require`          | `(module_name) -> value`        | Load a Lua module from the plugin directory |

## Hook Names and Context Fields

| Hook Name          | When Fired                   | Context Fields                 |
| ------------------ | ---------------------------- | ------------------------------ |
| `on_open`          | After a file is opened       | `path`, `filename`, `language` |
| `on_save`          | After a file is saved        | `path`, `filename`             |
| `on_close`         | Before a tab is closed       | `path`, `filename`             |
| `on_mode_change`   | When editor mode changes     | `old_mode`, `new_mode`         |
| `on_cursor_move`   | When cursor position changes | `line`, `col`                  |
| `on_buffer_change` | After buffer content changes | `path`, `filename`             |
| `on_tab_switch`    | When active tab changes      | `path`, `filename`             |
| `on_ready`         | After all plugins are loaded | (none)                         |

Context fields are `nil` when not applicable (e.g., unsaved buffers have no `path`).

## config.toml [plugins] Section

```toml
[plugins]
enabled = true                              # Enable the plugin system (default: false)
plugin_dirs = ["~/my-plugins"]              # Additional plugin directories
instruction_limit = 1000000                 # Lua instruction limit per execution (default: 1,000,000)
memory_limit = 10485760                     # Lua memory limit in bytes (default: 10 MB)

[plugins.overrides.my-plugin]
enabled = false                             # Disable a specific plugin

[plugins.overrides.my-plugin.config]
greeting = "Hello"                          # Per-plugin config (accessible via plugin.config.greeting)
```

## Sandbox Restrictions

Plugins run in a sandboxed Lua 5.4 environment with the following restrictions:

**Allowed libraries:** base globals, string, table, math, utf8

**Allowed os functions:** `os.clock`, `os.time`, `os.date`

**Blocked:**

- `io` library (file I/O)
- `debug` library
- `os.execute`, `os.remove`, `os.rename`, `os.exit`
- `loadfile`, `dofile`
- Global `require` (use `plugin.require` instead)
- Path traversal in `plugin.require` (`..` in module names)

**Resource limits:**

- Instruction count limit prevents infinite loops
- Memory limit prevents excessive allocation
- Both configurable via `config.toml`

## Example Plugin

See `runtime/plugins/example/` for a complete working example with:

- `wrap-quotes` command: wraps the current selection in double quotes
- `insert-date` command: inserts the current date at cursor position
- `on_save` hook: logs saved filename
- `on_ready` hook: logs when the plugin is ready
