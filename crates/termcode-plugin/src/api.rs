use std::cell::RefCell;
use std::path::PathBuf;

use mlua::Lua;
use termcode_core::position::Position;
use termcode_core::selection::{Range, Selection};
use termcode_core::transaction::Transaction;
use termcode_view::editor::{Editor, EditorMode};

use crate::types::DeferredAction;

/// Converts an mlua::Error into an anyhow::Error.
fn lua_err(e: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{}", e)
}

/// Converts an EditorMode to its string representation for Lua.
fn mode_to_string(mode: &EditorMode) -> &'static str {
    match mode {
        EditorMode::Normal => "normal",
        EditorMode::Insert => "insert",
        EditorMode::FileExplorer => "file_explorer",
        EditorMode::Search => "search",
        EditorMode::FuzzyFinder => "fuzzy_finder",
        EditorMode::CommandPalette => "command_palette",
    }
}

// Thread-local storage for the editor pointer during scoped API calls.
// SAFETY: This pointer is only valid during `with_scoped_api` execution.
// The pointer is set before Lua code runs and cleared immediately after.
thread_local! {
    static EDITOR_PTR: RefCell<Option<*mut Editor>> = const { RefCell::new(None) };
    static BUFFER_MUTATED: RefCell<bool> = const { RefCell::new(false) };
    static DEFERRED_ACTIONS: RefCell<Vec<DeferredAction>> = const { RefCell::new(Vec::new()) };
}

/// Safely access the editor through the thread-local pointer.
/// Returns an error if called outside of a scoped API context.
fn with_editor<F, R>(f: F) -> mlua::Result<R>
where
    F: FnOnce(&Editor) -> mlua::Result<R>,
{
    EDITOR_PTR.with(|ptr| {
        let ptr = ptr.borrow();
        let ptr = ptr.ok_or_else(|| {
            mlua::Error::RuntimeError(
                "editor API can only be called during command or hook execution".to_string(),
            )
        })?;
        // SAFETY: The pointer is valid during the scope of `with_scoped_api`.
        let editor = unsafe { &*ptr };
        f(editor)
    })
}

/// Safely access the editor mutably through the thread-local pointer.
fn with_editor_mut<F, R>(f: F) -> mlua::Result<R>
where
    F: FnOnce(&mut Editor) -> mlua::Result<R>,
{
    EDITOR_PTR.with(|ptr| {
        let ptr = ptr.borrow();
        let ptr = ptr.ok_or_else(|| {
            mlua::Error::RuntimeError(
                "editor API can only be called during command or hook execution".to_string(),
            )
        })?;
        // SAFETY: The pointer is valid during the scope of `with_scoped_api`.
        let editor = unsafe { &mut *ptr };
        f(editor)
    })
}

fn set_buffer_mutated() {
    BUFFER_MUTATED.with(|bm| *bm.borrow_mut() = true);
}

fn push_deferred_action(action: DeferredAction) {
    DEFERRED_ACTIONS.with(|da| da.borrow_mut().push(action));
}

/// Execute a function with the editor API active.
///
/// Sets the thread-local editor pointer, calls `f`, then clears the pointer.
/// Returns the `buffer_mutated` flag and any deferred actions collected.
///
/// SAFETY: The caller must ensure `editor` outlives the call to `f`.
/// This is guaranteed because `f` runs synchronously within the same scope.
pub fn with_scoped_api<F, R>(
    editor: &mut Editor,
    f: F,
) -> anyhow::Result<(R, bool, Vec<DeferredAction>)>
where
    F: FnOnce() -> anyhow::Result<R>,
{
    EDITOR_PTR.with(|ptr| *ptr.borrow_mut() = Some(editor as *mut Editor));
    BUFFER_MUTATED.with(|bm| *bm.borrow_mut() = false);
    DEFERRED_ACTIONS.with(|da| da.borrow_mut().clear());

    let result = f();

    EDITOR_PTR.with(|ptr| *ptr.borrow_mut() = None);
    let mutated = BUFFER_MUTATED.with(|bm| *bm.borrow());
    let actions = DEFERRED_ACTIONS.with(|da| std::mem::take(&mut *da.borrow_mut()));

    let value = result?;
    Ok((value, mutated, actions))
}

/// Registers the `editor` and `log` global tables with real API methods.
///
/// The methods use thread-local storage to access the editor. They will
/// return an error if called outside of a `with_scoped_api` context.
pub fn register_editor_api(lua: &Lua) -> anyhow::Result<()> {
    let editor_table = lua.create_table().map_err(lua_err)?;

    register_read_api(lua, &editor_table)?;
    register_write_api(lua, &editor_table)?;
    register_buffer_api(lua, &editor_table)?;
    register_deferred_api(lua, &editor_table)?;

    lua.globals().set("editor", editor_table).map_err(lua_err)?;

    Ok(())
}

/// Registers the `log` global table with info/warn/error/debug methods.
///
/// Each method reads `_current_plugin_name` from globals for the `[plugin:name]` prefix.
pub fn register_log_api(lua: &Lua) -> anyhow::Result<()> {
    let log_table = lua.create_table().map_err(lua_err)?;

    for (method_name, level) in &[
        ("info", log::Level::Info),
        ("warn", log::Level::Warn),
        ("error", log::Level::Error),
        ("debug", log::Level::Debug),
    ] {
        let level = *level;
        let func = lua
            .create_function(move |lua, msg: mlua::Value| {
                let msg_str = value_to_string(lua, &msg)?;
                let plugin_name: String = lua
                    .globals()
                    .get::<mlua::Value>("_current_plugin_name")
                    .ok()
                    .and_then(|v| match v {
                        mlua::Value::String(s) => s.to_str().ok().map(|s| s.to_string()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "unknown".to_string());

                log::log!(level, "[plugin:{}] {}", plugin_name, msg_str);
                Ok(())
            })
            .map_err(lua_err)?;
        log_table.set(*method_name, func).map_err(lua_err)?;
    }

    lua.globals().set("log", log_table).map_err(lua_err)?;

    Ok(())
}

/// Converts a Lua value to a string using Lua's tostring() for non-string types.
fn value_to_string(lua: &Lua, value: &mlua::Value) -> mlua::Result<String> {
    match value {
        mlua::Value::String(s) => Ok(s
            .to_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|_| "<invalid utf8>".to_string())),
        mlua::Value::Nil => Ok("nil".to_string()),
        mlua::Value::Boolean(b) => Ok(b.to_string()),
        mlua::Value::Integer(i) => Ok(i.to_string()),
        mlua::Value::Number(n) => Ok(n.to_string()),
        other => {
            let tostring: mlua::Function = lua.globals().get("tostring")?;
            let result: String = tostring.call(other.clone())?;
            Ok(result)
        }
    }
}

/// Convert 1-based Lua line/col to 0-based internal Position.
/// Returns error if values are less than 1.
fn lua_to_position(line: i64, col: i64) -> mlua::Result<Position> {
    if line < 1 {
        return Err(mlua::Error::RuntimeError(format!(
            "line must be >= 1, got {}",
            line
        )));
    }
    if col < 1 {
        return Err(mlua::Error::RuntimeError(format!(
            "col must be >= 1, got {}",
            col
        )));
    }
    Ok(Position::new((line - 1) as usize, (col - 1) as usize))
}

/// Convert 0-based internal Position to 1-based Lua line/col table.
fn position_to_lua(lua: &Lua, pos: &Position) -> mlua::Result<mlua::Table> {
    let table = lua.create_table()?;
    table.set("line", (pos.line + 1) as i64)?;
    table.set("col", (pos.column + 1) as i64)?;
    Ok(table)
}

/// Validate and convert 1-based Lua range (inclusive start, exclusive end)
/// to 0-based byte range. Returns (start_byte, end_byte).
fn validate_range(
    editor: &Editor,
    start_line: i64,
    start_col: i64,
    end_line: i64,
    end_col: i64,
) -> mlua::Result<(usize, usize)> {
    let start_pos = lua_to_position(start_line, start_col)?;
    let end_pos = lua_to_position(end_line, end_col)?;

    let doc = editor
        .active_document()
        .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;

    let start_byte = doc.buffer.pos_to_byte(&start_pos);
    let end_byte = doc.buffer.pos_to_byte(&end_pos);

    if start_byte > end_byte {
        return Err(mlua::Error::RuntimeError(format!(
            "invalid range: start ({},{}) is after end ({},{})",
            start_line, start_col, end_line, end_col
        )));
    }

    let doc_len = doc.buffer.len_bytes();
    if end_byte > doc_len {
        return Err(mlua::Error::RuntimeError(format!(
            "range end ({},{}) is beyond document end",
            end_line, end_col
        )));
    }

    Ok((start_byte, end_byte))
}

/// Read-only API methods
fn register_read_api(lua: &Lua, table: &mlua::Table) -> anyhow::Result<()> {
    // editor.get_mode() -> string
    let func = lua
        .create_function(|_lua, ()| with_editor(|ed| Ok(mode_to_string(&ed.mode).to_string())))
        .map_err(lua_err)?;
    table.set("get_mode", func).map_err(lua_err)?;

    // editor.get_cursor() -> {line, col} (1-based)
    let func = lua
        .create_function(|lua, ()| {
            with_editor(|ed| {
                let view = ed
                    .active_view()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active view".to_string()))?;
                position_to_lua(lua, &view.cursor)
            })
        })
        .map_err(lua_err)?;
    table.set("get_cursor", func).map_err(lua_err)?;

    // editor.get_selection() -> {start={line,col}, end={line,col}} | nil (1-based)
    // Returns nil when no range is selected (anchor == head).
    let func = lua
        .create_function(|lua, ()| {
            with_editor(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                let primary = doc.selection.primary();

                // No range selected when anchor equals head
                if primary.anchor == primary.head {
                    return Ok(mlua::Value::Nil);
                }

                let anchor_pos = doc.buffer.byte_to_pos(primary.anchor);
                let head_pos = doc.buffer.byte_to_pos(primary.head);

                // Normalize: start is always before end
                let (start_pos, end_pos) = if primary.anchor <= primary.head {
                    (anchor_pos, head_pos)
                } else {
                    (head_pos, anchor_pos)
                };

                let result = lua.create_table()?;
                result.set("start", position_to_lua(lua, &start_pos)?)?;
                result.set("end", position_to_lua(lua, &end_pos)?)?;
                Ok(mlua::Value::Table(result))
            })
        })
        .map_err(lua_err)?;
    table.set("get_selection", func).map_err(lua_err)?;

    // editor.get_line(line_number) -> string (1-based, no trailing newline)
    let func = lua
        .create_function(|_lua, line_num: i64| {
            with_editor(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                if line_num < 1 {
                    return Err(mlua::Error::RuntimeError(format!(
                        "line must be >= 1, got {}",
                        line_num
                    )));
                }
                let line_idx = (line_num - 1) as usize;
                let line_count = doc.buffer.line_count();
                if line_idx >= line_count {
                    return Err(mlua::Error::RuntimeError(format!(
                        "line {} is beyond document end (document has {} lines)",
                        line_num, line_count
                    )));
                }
                let line_slice = doc.buffer.line(line_idx);
                let line_str: String = line_slice.into();
                Ok(line_str.trim_end_matches(&['\n', '\r'][..]).to_string())
            })
        })
        .map_err(lua_err)?;
    table.set("get_line", func).map_err(lua_err)?;

    // editor.get_line_count() -> integer
    let func = lua
        .create_function(|_lua, ()| {
            with_editor(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                Ok(doc.buffer.line_count() as i64)
            })
        })
        .map_err(lua_err)?;
    table.set("get_line_count", func).map_err(lua_err)?;

    // editor.get_filename() -> string or nil
    let func = lua
        .create_function(|lua, ()| {
            with_editor(|ed| {
                let doc = match ed.active_document() {
                    Some(d) => d,
                    None => return Ok(mlua::Value::Nil),
                };
                match doc.path.as_ref().and_then(|p| p.file_name()) {
                    Some(name) => Ok(mlua::Value::String(
                        lua.create_string(name.to_string_lossy().as_ref())?,
                    )),
                    None => Ok(mlua::Value::Nil),
                }
            })
        })
        .map_err(lua_err)?;
    table.set("get_filename", func).map_err(lua_err)?;

    // editor.get_filepath() -> string or nil
    let func = lua
        .create_function(|lua, ()| {
            with_editor(|ed| {
                let doc = match ed.active_document() {
                    Some(d) => d,
                    None => return Ok(mlua::Value::Nil),
                };
                match &doc.path {
                    Some(path) => Ok(mlua::Value::String(
                        lua.create_string(path.to_string_lossy().as_ref())?,
                    )),
                    None => Ok(mlua::Value::Nil),
                }
            })
        })
        .map_err(lua_err)?;
    table.set("get_filepath", func).map_err(lua_err)?;

    // editor.get_status() -> string or nil
    let func = lua
        .create_function(|lua, ()| {
            with_editor(|ed| match &ed.status_message {
                Some(msg) => Ok(mlua::Value::String(lua.create_string(msg.as_str())?)),
                None => Ok(mlua::Value::Nil),
            })
        })
        .map_err(lua_err)?;
    table.set("get_status", func).map_err(lua_err)?;

    // editor.get_theme_name() -> string
    let func = lua
        .create_function(|_lua, ()| with_editor(|ed| Ok(ed.theme.name.clone())))
        .map_err(lua_err)?;
    table.set("get_theme_name", func).map_err(lua_err)?;

    // editor.get_config() -> table with editor config values
    let func = lua
        .create_function(|lua, ()| {
            with_editor(|ed| {
                let config = lua.create_table()?;
                config.set("tab_size", ed.config.tab_size as i64)?;
                config.set("scroll_off", ed.config.scroll_off as i64)?;
                Ok(config)
            })
        })
        .map_err(lua_err)?;
    table.set("get_config", func).map_err(lua_err)?;

    Ok(())
}

/// Write API methods: set_status, set_cursor, set_selection
fn register_write_api(lua: &Lua, table: &mlua::Table) -> anyhow::Result<()> {
    // editor.set_status(msg: string)
    let func = lua
        .create_function(|_lua, msg: String| {
            with_editor_mut(|ed| {
                ed.status_message = Some(msg);
                Ok(())
            })
        })
        .map_err(lua_err)?;
    table.set("set_status", func).map_err(lua_err)?;

    // editor.set_cursor(line, col) -- 1-based
    let func = lua
        .create_function(|_lua, (line, col): (i64, i64)| {
            let pos = lua_to_position(line, col)?;
            with_editor_mut(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                let line_count = doc.buffer.line_count();
                if pos.line >= line_count {
                    return Err(mlua::Error::RuntimeError(format!(
                        "line {} is beyond document end (document has {} lines)",
                        line, line_count
                    )));
                }
                let byte_pos = doc.buffer.pos_to_byte(&pos);

                let doc_mut = ed
                    .active_document_mut()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                doc_mut.selection = Selection::point(byte_pos);

                let view = ed
                    .active_view_mut()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active view".to_string()))?;
                view.cursor = pos;

                Ok(())
            })
        })
        .map_err(lua_err)?;
    table.set("set_cursor", func).map_err(lua_err)?;

    // editor.set_selection(anchor_line, anchor_col, head_line, head_col) -- 1-based
    let func = lua
        .create_function(
            |_lua, (a_line, a_col, h_line, h_col): (i64, i64, i64, i64)| {
                let anchor_pos = lua_to_position(a_line, a_col)?;
                let head_pos = lua_to_position(h_line, h_col)?;

                with_editor_mut(|ed| {
                    let doc = ed.active_document().ok_or_else(|| {
                        mlua::Error::RuntimeError("no active document".to_string())
                    })?;
                    let anchor_byte = doc.buffer.pos_to_byte(&anchor_pos);
                    let head_byte = doc.buffer.pos_to_byte(&head_pos);

                    let doc_mut = ed.active_document_mut().ok_or_else(|| {
                        mlua::Error::RuntimeError("no active document".to_string())
                    })?;
                    doc_mut.selection = Selection::new(vec![Range::new(anchor_byte, head_byte)], 0);

                    let view = ed
                        .active_view_mut()
                        .ok_or_else(|| mlua::Error::RuntimeError("no active view".to_string()))?;
                    view.cursor = head_pos;

                    Ok(())
                })
            },
        )
        .map_err(lua_err)?;
    table.set("set_selection", func).map_err(lua_err)?;

    Ok(())
}

/// Buffer-mutating API methods: insert_text, delete_selection, buffer_get_text,
/// buffer_get_range, buffer_replace_range
fn register_buffer_api(lua: &Lua, table: &mlua::Table) -> anyhow::Result<()> {
    // editor.insert_text(text: string) -- inserts at current cursor position
    let func = lua
        .create_function(|_lua, text: String| {
            with_editor_mut(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                let cursor_byte = doc.selection.primary().head;
                let doc_len = doc.buffer.len_bytes();

                let txn = Transaction::insert(&text, cursor_byte, doc_len);
                let doc_mut = ed
                    .active_document_mut()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                doc_mut
                    .apply_transaction(&txn)
                    .map_err(|e| mlua::Error::RuntimeError(format!("{}", e)))?;

                set_buffer_mutated();
                Ok(())
            })
        })
        .map_err(lua_err)?;
    table.set("insert_text", func).map_err(lua_err)?;

    // editor.delete_selection() -- deletes the primary selection range
    let func = lua
        .create_function(|_lua, ()| {
            with_editor_mut(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                let primary = doc.selection.primary();
                if primary.is_empty() {
                    return Ok(());
                }
                let from = primary.from();
                let to = primary.to();
                let doc_len = doc.buffer.len_bytes();

                let txn = Transaction::delete(from..to, doc_len);
                let doc_mut = ed
                    .active_document_mut()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                doc_mut
                    .apply_transaction(&txn)
                    .map_err(|e| mlua::Error::RuntimeError(format!("{}", e)))?;

                set_buffer_mutated();
                Ok(())
            })
        })
        .map_err(lua_err)?;
    table.set("delete_selection", func).map_err(lua_err)?;

    // editor.buffer_get_text() -> string (entire buffer content)
    let func = lua
        .create_function(|_lua, ()| {
            with_editor(|ed| {
                let doc = ed
                    .active_document()
                    .ok_or_else(|| mlua::Error::RuntimeError("no active document".to_string()))?;
                let text: String = doc.buffer.text().into();
                Ok(text)
            })
        })
        .map_err(lua_err)?;
    table.set("buffer_get_text", func).map_err(lua_err)?;

    // editor.buffer_get_range(start_line, start_col, end_line, end_col) -> string
    // 1-based, inclusive start, exclusive end
    let func = lua
        .create_function(
            |_lua, (s_line, s_col, e_line, e_col): (i64, i64, i64, i64)| {
                with_editor(|ed| {
                    let (start_byte, end_byte) = validate_range(ed, s_line, s_col, e_line, e_col)?;
                    let doc = ed.active_document().ok_or_else(|| {
                        mlua::Error::RuntimeError("no active document".to_string())
                    })?;

                    let rope = doc.buffer.text();
                    let start_char = rope.byte_to_char(start_byte);
                    let end_char = rope.byte_to_char(end_byte);
                    let slice: String = rope.slice(start_char..end_char).into();
                    Ok(slice)
                })
            },
        )
        .map_err(lua_err)?;
    table.set("buffer_get_range", func).map_err(lua_err)?;

    // editor.buffer_replace_range(start_line, start_col, end_line, end_col, text)
    // 1-based, inclusive start, exclusive end
    let func = lua
        .create_function(
            |_lua, (s_line, s_col, e_line, e_col, text): (i64, i64, i64, i64, String)| {
                with_editor_mut(|ed| {
                    let (start_byte, end_byte) = validate_range(ed, s_line, s_col, e_line, e_col)?;

                    let doc = ed.active_document().ok_or_else(|| {
                        mlua::Error::RuntimeError("no active document".to_string())
                    })?;
                    let doc_len = doc.buffer.len_bytes();

                    let txn = if start_byte == end_byte {
                        Transaction::insert(&text, start_byte, doc_len)
                    } else {
                        Transaction::replace(start_byte..end_byte, &text, doc_len)
                    };

                    let doc_mut = ed.active_document_mut().ok_or_else(|| {
                        mlua::Error::RuntimeError("no active document".to_string())
                    })?;
                    doc_mut
                        .apply_transaction(&txn)
                        .map_err(|e| mlua::Error::RuntimeError(format!("{}", e)))?;

                    set_buffer_mutated();
                    Ok(())
                })
            },
        )
        .map_err(lua_err)?;
    table.set("buffer_replace_range", func).map_err(lua_err)?;

    Ok(())
}

/// Deferred action methods: open_file, execute_command
fn register_deferred_api(lua: &Lua, table: &mlua::Table) -> anyhow::Result<()> {
    // editor.open_file(path: string)
    let func = lua
        .create_function(|_lua, path: String| {
            push_deferred_action(DeferredAction::OpenFile(PathBuf::from(path)));
            Ok(())
        })
        .map_err(lua_err)?;
    table.set("open_file", func).map_err(lua_err)?;

    // editor.execute_command(command_id: string)
    let func = lua
        .create_function(|_lua, cmd_id: String| {
            if cmd_id.starts_with("plugin.") {
                return Err(mlua::Error::RuntimeError(
                    "cannot execute plugin.* commands from within a plugin (prevents recursion)"
                        .to_string(),
                ));
            }
            push_deferred_action(DeferredAction::ExecuteCommand(cmd_id));
            Ok(())
        })
        .map_err(lua_err)?;
    table.set("execute_command", func).map_err(lua_err)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_lua() -> Lua {
        Lua::new()
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

    #[test]
    fn test_register_editor_api_creates_table() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let editor_table: mlua::Table = lua.globals().get("editor").unwrap();
        let _get_mode: mlua::Function = editor_table.get("get_mode").unwrap();
    }

    #[test]
    fn test_api_error_outside_scope() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let result = lua.load("return editor.get_mode()").exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("can only be called during command or hook execution"));
    }

    #[test]
    fn test_register_log_api() {
        let lua = create_test_lua();
        register_log_api(&lua).unwrap();

        let log_table: mlua::Table = lua.globals().get("log").unwrap();
        assert!(log_table.get::<mlua::Function>("info").is_ok());
        assert!(log_table.get::<mlua::Function>("warn").is_ok());
        assert!(log_table.get::<mlua::Function>("error").is_ok());
        assert!(log_table.get::<mlua::Function>("debug").is_ok());
    }

    #[test]
    fn test_log_with_plugin_name() {
        let lua = create_test_lua();
        register_log_api(&lua).unwrap();

        lua.globals()
            .set("_current_plugin_name", "test-plugin")
            .unwrap();

        lua.load(r#"log.info("hello")"#).exec().unwrap();
        lua.load(r#"log.warn(42)"#).exec().unwrap();
        lua.load(r#"log.error(true)"#).exec().unwrap();
        lua.load(r#"log.debug(nil)"#).exec().unwrap();
    }

    #[test]
    fn test_lua_to_position_valid() {
        let pos = lua_to_position(1, 1).unwrap();
        assert_eq!(pos, Position::new(0, 0));

        let pos = lua_to_position(10, 5).unwrap();
        assert_eq!(pos, Position::new(9, 4));
    }

    #[test]
    fn test_lua_to_position_invalid() {
        assert!(lua_to_position(0, 1).is_err());
        assert!(lua_to_position(1, 0).is_err());
        assert!(lua_to_position(-1, 1).is_err());
    }

    #[test]
    fn test_position_to_lua_conversion() {
        let lua = create_test_lua();
        let pos = Position::new(0, 0);
        let table = position_to_lua(&lua, &pos).unwrap();
        assert_eq!(table.get::<i64>("line").unwrap(), 1);
        assert_eq!(table.get::<i64>("col").unwrap(), 1);

        let pos = Position::new(9, 4);
        let table = position_to_lua(&lua, &pos).unwrap();
        assert_eq!(table.get::<i64>("line").unwrap(), 10);
        assert_eq!(table.get::<i64>("col").unwrap(), 5);
    }

    #[test]
    fn test_mode_to_string() {
        assert_eq!(mode_to_string(&EditorMode::Normal), "normal");
        assert_eq!(mode_to_string(&EditorMode::Insert), "insert");
        assert_eq!(mode_to_string(&EditorMode::FileExplorer), "file_explorer");
        assert_eq!(mode_to_string(&EditorMode::Search), "search");
        assert_eq!(mode_to_string(&EditorMode::FuzzyFinder), "fuzzy_finder");
        assert_eq!(
            mode_to_string(&EditorMode::CommandPalette),
            "command_palette"
        );
    }

    #[test]
    fn test_scoped_api_get_mode() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let (mode, _, _) = with_scoped_api(&mut editor, || {
            let mode: String = lua
                .load("return editor.get_mode()")
                .eval()
                .map_err(lua_err)?;
            Ok(mode)
        })
        .unwrap();
        assert_eq!(mode, "normal");
    }

    #[test]
    fn test_scoped_api_set_status() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        with_scoped_api(&mut editor, || {
            lua.load(r#"editor.set_status("hello from lua")"#)
                .exec()
                .map_err(lua_err)?;
            Ok(())
        })
        .unwrap();

        assert_eq!(editor.status_message.as_deref(), Some("hello from lua"));
    }

    #[test]
    fn test_scoped_api_get_theme_name() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let (name, _, _) = with_scoped_api(&mut editor, || {
            let name: String = lua
                .load("return editor.get_theme_name()")
                .eval()
                .map_err(lua_err)?;
            Ok(name)
        })
        .unwrap();
        assert!(!name.is_empty());
    }

    #[test]
    fn test_deferred_open_file() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let (_, _, actions) = with_scoped_api(&mut editor, || {
            lua.load(r#"editor.open_file("/tmp/test.rs")"#)
                .exec()
                .map_err(lua_err)?;
            Ok(())
        })
        .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            DeferredAction::OpenFile(p) => assert_eq!(p, &PathBuf::from("/tmp/test.rs")),
            _ => panic!("expected OpenFile action"),
        }
    }

    #[test]
    fn test_deferred_execute_command() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let (_, _, actions) = with_scoped_api(&mut editor, || {
            lua.load(r#"editor.execute_command("file.save")"#)
                .exec()
                .map_err(lua_err)?;
            Ok(())
        })
        .unwrap();

        assert_eq!(actions.len(), 1);
        match &actions[0] {
            DeferredAction::ExecuteCommand(cmd) => assert_eq!(cmd, "file.save"),
            _ => panic!("expected ExecuteCommand action"),
        }
    }

    #[test]
    fn test_deferred_execute_plugin_command_rejected() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let result = with_scoped_api(&mut editor, || {
            lua.load(r#"editor.execute_command("plugin.my-plugin.foo")"#)
                .exec()
                .map_err(lua_err)?;
            Ok(())
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_no_active_document_guard() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let result = with_scoped_api(&mut editor, || {
            lua.load("return editor.get_line(1)")
                .exec()
                .map_err(lua_err)?;
            Ok(())
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_scope_clears_pointer() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        with_scoped_api(&mut editor, || Ok(())).unwrap();

        let result = lua.load("return editor.get_mode()").exec();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("can only be called during command or hook execution"));
    }

    #[test]
    fn test_buffer_mutated_flag() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();

        // Open a document so we have an active buffer
        let tmp = std::env::temp_dir().join("test_buffer_mutated.txt");
        std::fs::write(&tmp, "hello world\n").unwrap();
        editor.open_file(&tmp).unwrap();

        let (_, mutated, _) = with_scoped_api(&mut editor, || {
            lua.load(r#"editor.insert_text("!")"#)
                .exec()
                .map_err(lua_err)?;
            Ok(())
        })
        .unwrap();

        assert!(mutated);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_get_line_no_newline() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();

        let tmp = std::env::temp_dir().join("test_get_line.txt");
        std::fs::write(&tmp, "first line\nsecond line\n").unwrap();
        editor.open_file(&tmp).unwrap();

        let (line, _, _) = with_scoped_api(&mut editor, || {
            let line: String = lua
                .load("return editor.get_line(1)")
                .eval()
                .map_err(lua_err)?;
            Ok(line)
        })
        .unwrap();

        assert_eq!(line, "first line");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_get_config() {
        let lua = create_test_lua();
        register_editor_api(&lua).unwrap();

        let mut editor = create_test_editor();
        let (tab_size, _, _) = with_scoped_api(&mut editor, || {
            let val: i64 = lua
                .load("return editor.get_config().tab_size")
                .eval()
                .map_err(lua_err)?;
            Ok(val)
        })
        .unwrap();

        assert!(tab_size > 0);
    }
}
