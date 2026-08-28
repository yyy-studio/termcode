//! The command registry and every handler that needs nothing beyond an
//! [`Editor`].
//!
//! Two groups live in their own files because they grew large enough to bury
//! the rest: [`motion`] (word motions) and [`line_edit`] (line-oriented
//! editing). Commands whose behaviour needs `App` are registered here as noops
//! and intercepted before dispatch; see [`register_app_level_commands`].

use std::collections::HashMap;

use termcode_core::config_types::LineNumberStyle;
use termcode_core::selection::Selection;
use termcode_core::transaction::Transaction;
use termcode_view::editor::{Editor, EditorMode};

use crate::display_width::TabStops;
use crate::ui::editor_view::line_number_width_styled;

mod line_edit;
mod motion;
#[cfg(test)]
mod test_support;

pub type CommandId = &'static str;
pub type CommandHandler = fn(&mut Editor) -> anyhow::Result<()>;

pub struct CommandEntry {
    pub id: CommandId,
    pub name: &'static str,
    pub handler: CommandHandler,
}

pub struct CommandRegistry {
    commands: HashMap<CommandId, CommandEntry>,
    /// Commands that exist so keymaps can bind them, but that make no sense as
    /// command-palette entries (overlay navigation, file-explorer motions).
    hidden: std::collections::HashSet<CommandId>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            hidden: std::collections::HashSet::new(),
        }
    }

    pub fn register(&mut self, entry: CommandEntry) {
        self.commands.insert(entry.id, entry);
    }

    /// Register a command that is bindable but omitted from the command palette.
    pub fn register_hidden(&mut self, entry: CommandEntry) {
        self.hidden.insert(entry.id);
        self.commands.insert(entry.id, entry);
    }

    pub fn execute(&self, id: CommandId, editor: &mut Editor) -> anyhow::Result<()> {
        let entry = self
            .commands
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command: {id}"))?;
        (entry.handler)(editor)
    }

    pub fn get(&self, id: CommandId) -> Option<&CommandEntry> {
        self.commands.get(id)
    }

    /// Execute a command by a non-static string ID (e.g., from plugin deferred actions).
    pub fn execute_by_str(&self, id: &str, editor: &mut Editor) -> anyhow::Result<()> {
        let entry = self
            .commands
            .values()
            .find(|e| e.id == id)
            .ok_or_else(|| anyhow::anyhow!("Unknown command: {id}"))?;
        (entry.handler)(editor)
    }

    pub fn get_by_string(&self, id: &str) -> Option<&CommandEntry> {
        self.commands.values().find(|e| e.id == id)
    }

    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        let mut cmds: Vec<(&str, &str)> = self
            .commands
            .values()
            .filter(|e| !self.hidden.contains(e.id))
            .map(|e| (e.id, e.name))
            .collect();
        cmds.sort_by_key(|(_, name)| *name);
        cmds
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub fn register_builtin_commands(registry: &mut CommandRegistry) {
    registry.register(CommandEntry {
        id: "file.save",
        name: "Save File",
        handler: cmd_file_save,
    });
    registry.register(CommandEntry {
        id: "edit.delete_char",
        name: "Delete Character",
        handler: cmd_delete_char,
    });
    registry.register(CommandEntry {
        id: "edit.backspace",
        name: "Backspace",
        handler: cmd_backspace,
    });
    registry.register(CommandEntry {
        id: "edit.newline",
        name: "Insert Newline",
        handler: cmd_newline,
    });
    registry.register(CommandEntry {
        id: "edit.undo",
        name: "Undo",
        handler: cmd_undo,
    });
    registry.register(CommandEntry {
        id: "edit.redo",
        name: "Redo",
        handler: cmd_redo,
    });
    registry.register(CommandEntry {
        id: "cursor.up",
        name: "Cursor Up",
        handler: cmd_cursor_up,
    });
    registry.register(CommandEntry {
        id: "cursor.down",
        name: "Cursor Down",
        handler: cmd_cursor_down,
    });
    registry.register(CommandEntry {
        id: "cursor.left",
        name: "Cursor Left",
        handler: cmd_cursor_left,
    });
    registry.register(CommandEntry {
        id: "cursor.right",
        name: "Cursor Right",
        handler: cmd_cursor_right,
    });
    registry.register(CommandEntry {
        id: "cursor.page_up",
        name: "Page Up",
        handler: cmd_page_up,
    });
    registry.register(CommandEntry {
        id: "cursor.page_down",
        name: "Page Down",
        handler: cmd_page_down,
    });
    registry.register(CommandEntry {
        id: "cursor.line_start",
        name: "Go to Line Start",
        handler: cmd_line_start,
    });
    registry.register(CommandEntry {
        id: "cursor.line_end",
        name: "Go to Line End",
        handler: cmd_line_end,
    });
    registry.register(CommandEntry {
        id: "cursor.home",
        name: "Go to Beginning",
        handler: cmd_home,
    });
    registry.register(CommandEntry {
        id: "cursor.end",
        name: "Go to End",
        handler: cmd_end,
    });
    registry.register(CommandEntry {
        id: "mode.insert",
        name: "Enter Edit Mode",
        handler: cmd_mode_insert,
    });
    registry.register(CommandEntry {
        id: "mode.normal",
        name: "Enter Normal Mode",
        handler: cmd_mode_normal,
    });
    registry.register(CommandEntry {
        id: "tab.next",
        name: "Next Tab",
        handler: cmd_tab_next,
    });
    registry.register(CommandEntry {
        id: "tab.prev",
        name: "Previous Tab",
        handler: cmd_tab_prev,
    });
    registry.register(CommandEntry {
        id: "view.toggle_sidebar",
        name: "Toggle Sidebar",
        handler: cmd_toggle_sidebar,
    });

    registry.register(CommandEntry {
        id: "search.open",
        name: "Find",
        handler: cmd_search_open,
    });
    registry.register(CommandEntry {
        id: "search.open_replace",
        name: "Find and Replace",
        handler: cmd_search_open_replace,
    });
    registry.register(CommandEntry {
        id: "search.next",
        name: "Find Next",
        handler: cmd_search_next,
    });
    registry.register(CommandEntry {
        id: "search.prev",
        name: "Find Previous",
        handler: cmd_search_prev,
    });
    registry.register(CommandEntry {
        id: "search.replace_current",
        name: "Replace",
        handler: cmd_search_replace_current,
    });
    registry.register(CommandEntry {
        id: "search.replace_all",
        name: "Replace All",
        handler: cmd_search_replace_all,
    });
    registry.register(CommandEntry {
        id: "search.close",
        name: "Close Search",
        handler: cmd_search_close,
    });

    registry.register(CommandEntry {
        id: "fuzzy.open",
        name: "Open File",
        handler: cmd_fuzzy_open,
    });
    registry.register(CommandEntry {
        id: "fuzzy.close",
        name: "Close Finder",
        handler: cmd_fuzzy_close,
    });

    registry.register(CommandEntry {
        id: "palette.open",
        name: "Command Palette",
        handler: cmd_palette_open,
    });
    registry.register(CommandEntry {
        id: "palette.close",
        name: "Close Palette",
        handler: cmd_palette_close,
    });
    registry.register(CommandEntry {
        id: "diagnostic.next",
        name: "Next Diagnostic",
        handler: cmd_diagnostic_next,
    });
    registry.register(CommandEntry {
        id: "diagnostic.prev",
        name: "Previous Diagnostic",
        handler: cmd_diagnostic_prev,
    });

    registry.register(CommandEntry {
        id: "goto.definition",
        name: "Go to Definition",
        handler: cmd_noop,
    });
    registry.register(CommandEntry {
        id: "lsp.hover",
        name: "Show Hover Info",
        handler: cmd_noop,
    });
    registry.register(CommandEntry {
        id: "lsp.trigger_completion",
        name: "Trigger Completion",
        handler: cmd_noop,
    });

    registry.register(CommandEntry {
        id: "clipboard.copy",
        name: "Copy to Clipboard",
        handler: cmd_clipboard_copy,
    });
    registry.register(CommandEntry {
        id: "clipboard.cut",
        name: "Cut to Clipboard",
        handler: cmd_clipboard_cut,
    });
    registry.register(CommandEntry {
        id: "clipboard.paste",
        name: "Paste from Clipboard",
        handler: cmd_clipboard_paste,
    });

    registry.register(CommandEntry {
        id: "theme.list",
        name: "Select Theme",
        handler: cmd_noop,
    });

    registry.register(CommandEntry {
        id: "keymap.list",
        name: "Select Keymap",
        handler: cmd_noop,
    });

    registry.register(CommandEntry {
        id: "settings.open",
        name: "Open Settings",
        handler: cmd_noop,
    });

    registry.register(CommandEntry {
        id: "line_numbers.toggle",
        name: "Toggle Line Numbers",
        handler: cmd_line_numbers_toggle,
    });

    registry.register(CommandEntry {
        id: "help.toggle",
        name: "Toggle Help",
        handler: cmd_help_toggle,
    });

    motion::register_motion_commands(registry);
    line_edit::register_line_edit_commands(registry);
    register_app_level_commands(registry);
}

/// Commands whose behaviour lives in `App` rather than in a handler here.
///
/// They are registered as noops purely so keymap presets and user overrides can
/// bind them: `InputMapper` validates every binding against this registry, and
/// an unregistered id is silently dropped. `App::handle_key` / `dispatch_command`
/// intercept these ids before the noop handler would ever run.
fn register_app_level_commands(registry: &mut CommandRegistry) {
    const HIDDEN: &[(&str, &str)] = &[
        ("explorer.down", "Explorer: Down"),
        ("explorer.up", "Explorer: Up"),
        ("explorer.enter", "Explorer: Open"),
        ("explorer.expand", "Explorer: Expand"),
        ("explorer.collapse", "Explorer: Collapse"),
        ("explorer.refresh", "Explorer: Refresh Node"),
        ("explorer.refresh_all", "Explorer: Refresh All"),
        ("explorer.new_file", "Explorer: New File"),
        ("explorer.new_folder", "Explorer: New Folder"),
        ("explorer.copy_path", "Explorer: Copy Path"),
        ("fuzzy.up", "Finder: Previous Item"),
        ("fuzzy.down", "Finder: Next Item"),
        ("palette.up", "Palette: Previous Item"),
        ("palette.down", "Palette: Next Item"),
        ("settings.up", "Settings: Previous Item"),
        ("settings.down", "Settings: Next Item"),
        ("settings.page_up", "Settings: Page Up"),
        ("settings.page_down", "Settings: Page Down"),
        ("settings.focus_out", "Settings: Back to Categories"),
        ("settings.focus_in", "Settings: Into Settings"),
        ("settings.activate", "Settings: Open / Change Value"),
        ("settings.toggle_focus", "Settings: Switch Pane"),
        ("settings.close", "Settings: Close"),
    ];
    for (id, name) in HIDDEN {
        registry.register_hidden(CommandEntry {
            id,
            name,
            handler: cmd_noop,
        });
    }

    registry.register(CommandEntry {
        id: "tab.close",
        name: "Close Tab",
        handler: cmd_noop,
    });
    registry.register(CommandEntry {
        id: "app.quit",
        name: "Quit",
        handler: cmd_noop,
    });
}

pub fn cmd_noop(_editor: &mut Editor) -> anyhow::Result<()> {
    Ok(())
}

fn cmd_help_toggle(editor: &mut Editor) -> anyhow::Result<()> {
    editor.help_visible = !editor.help_visible;
    Ok(())
}

fn cmd_line_numbers_toggle(editor: &mut Editor) -> anyhow::Result<()> {
    editor.config.line_numbers = match editor.config.line_numbers {
        LineNumberStyle::Absolute => LineNumberStyle::Relative,
        LineNumberStyle::Relative => LineNumberStyle::RelativeAbsolute,
        LineNumberStyle::RelativeAbsolute => LineNumberStyle::None,
        LineNumberStyle::None => LineNumberStyle::Absolute,
    };
    let label = match editor.config.line_numbers {
        LineNumberStyle::Absolute => "Absolute",
        LineNumberStyle::Relative => "Relative",
        LineNumberStyle::RelativeAbsolute => "Relative + Absolute",
        LineNumberStyle::None => "Hidden",
    };
    editor.status_message = Some(format!("Line numbers: {label}"));
    Ok(())
}

fn cmd_clipboard_copy(editor: &mut Editor) -> anyhow::Result<()> {
    let doc = editor
        .active_document()
        .ok_or_else(|| anyhow::anyhow!("No active document"))?;
    let primary = doc.selection.primary();
    if primary.is_empty() {
        editor.status_message = Some("No selection to copy".to_string());
        return Ok(());
    }
    let from = primary.from();
    let to = primary.to();
    let text: String = doc.buffer.text().byte_slice(from..to).chars().collect();
    let char_count = text.chars().count();
    let clipboard = editor
        .clipboard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("Clipboard unavailable"))?;
    clipboard.set_text(&text)?;
    editor.status_message = Some(format!("Copied {char_count} characters"));
    Ok(())
}

fn cmd_clipboard_cut(editor: &mut Editor) -> anyhow::Result<()> {
    let (from, to, text, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        let primary = doc.selection.primary();
        if primary.is_empty() {
            editor.status_message = Some("No selection to cut".to_string());
            return Ok(());
        }
        let from = primary.from();
        let to = primary.to();
        let text: String = doc.buffer.text().byte_slice(from..to).chars().collect();
        (from, to, text, doc.buffer.len_bytes())
    };
    let clipboard = editor
        .clipboard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("Clipboard unavailable"))?;
    clipboard.set_text(&text)?;
    let txn = Transaction::delete(from..to, doc_len).with_selection(Selection::point(from));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    editor.status_message = Some("Cut to clipboard".to_string());
    Ok(())
}

fn cmd_clipboard_paste(editor: &mut Editor) -> anyhow::Result<()> {
    let text = {
        let clipboard = editor
            .clipboard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Clipboard unavailable"))?;
        clipboard
            .get_text()
            .ok_or_else(|| anyhow::anyhow!("Nothing to paste"))?
    };
    if text.is_empty() {
        return Ok(());
    }
    let (byte_pos, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        (doc.selection.primary().head, doc.buffer.len_bytes())
    };
    let new_pos = byte_pos + text.len();
    let txn =
        Transaction::insert(&text, byte_pos, doc_len).with_selection(Selection::point(new_pos));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_file_save(editor: &mut Editor) -> anyhow::Result<()> {
    let doc_id = match editor.active_view().map(|v| v.doc_id) {
        Some(id) => id,
        None => return Ok(()),
    };
    editor.save_document(doc_id)
}

fn cmd_delete_char(editor: &mut Editor) -> anyhow::Result<()> {
    let (byte_pos, doc_len, char_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        let byte_pos = doc.selection.primary().head;
        let doc_len = doc.buffer.len_bytes();
        if byte_pos >= doc_len {
            return Ok(());
        }
        let char_idx = doc.buffer.text().byte_to_char(byte_pos);
        let next_char_byte = doc.buffer.text().char_to_byte(char_idx + 1);
        let char_len = next_char_byte - byte_pos;
        (byte_pos, doc_len, char_len)
    };
    let txn = Transaction::delete(byte_pos..byte_pos + char_len, doc_len);
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_backspace(editor: &mut Editor) -> anyhow::Result<()> {
    let (prev_byte, cursor_byte, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        let cursor_byte = doc.selection.primary().head;
        if cursor_byte == 0 {
            return Ok(());
        }
        let char_idx = doc.buffer.text().byte_to_char(cursor_byte);
        let prev_byte = doc.buffer.text().char_to_byte(char_idx - 1);
        let doc_len = doc.buffer.len_bytes();
        (prev_byte, cursor_byte, doc_len)
    };
    let txn = Transaction::delete(prev_byte..cursor_byte, doc_len)
        .with_selection(Selection::point(prev_byte));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_newline(editor: &mut Editor) -> anyhow::Result<()> {
    let (byte_pos, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        (doc.selection.primary().head, doc.buffer.len_bytes())
    };
    let new_pos = byte_pos + 1; // '\n' is 1 byte
    let txn =
        Transaction::insert("\n", byte_pos, doc_len).with_selection(Selection::point(new_pos));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_undo(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(doc) = editor.active_document_mut() {
        doc.undo()?;
    }
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_redo(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(doc) = editor.active_document_mut() {
        doc.redo()?;
    }
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_cursor_up(editor: &mut Editor) -> anyhow::Result<()> {
    let scroll_off = editor.config.scroll_off;
    if let Some(view) = editor.active_view_mut() {
        if view.cursor.line > 0 {
            view.cursor.line -= 1;
            view.ensure_cursor_visible(scroll_off);
        }
    }
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_cursor_down(editor: &mut Editor) -> anyhow::Result<()> {
    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    let scroll_off = editor.config.scroll_off;
    if let Some(view) = editor.active_view_mut() {
        if view.cursor.line + 1 < line_count {
            view.cursor.line += 1;
            view.ensure_cursor_visible(scroll_off);
        }
    }
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_cursor_left(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(view) = editor.active_view_mut() {
        view.cursor.column = view.cursor.column.saturating_sub(1);
    }
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_cursor_right(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(view) = editor.active_view_mut() {
        view.cursor.column += 1;
    }
    clamp_cursor_column_right(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_page_up(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(view) = editor.active_view_mut() {
        let page = view.area_height as usize;
        view.cursor.line = view.cursor.line.saturating_sub(page);
        view.scroll_up(page);
    }
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_page_down(editor: &mut Editor) -> anyhow::Result<()> {
    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    if let Some(view) = editor.active_view_mut() {
        let page = view.area_height as usize;
        view.cursor.line = (view.cursor.line + page).min(line_count.saturating_sub(1));
        view.scroll_down(page, line_count);
    }
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_line_start(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(view) = editor.active_view_mut() {
        view.cursor.column = 0;
    }
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_line_end(editor: &mut Editor) -> anyhow::Result<()> {
    let max_col = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return Ok(()),
        };
        let view = match editor.active_view() {
            Some(v) => v,
            None => return Ok(()),
        };
        let line = view.cursor.line;
        if line >= doc.buffer.line_count() {
            return Ok(());
        }
        let line_text: String = doc.buffer.line(line).into();
        let len = line_text
            .trim_end_matches(&['\n', '\r'][..])
            .chars()
            .count();
        if editor.mode == EditorMode::Insert {
            len
        } else {
            len.saturating_sub(1)
        }
    };
    if let Some(view) = editor.active_view_mut() {
        view.cursor.column = max_col;
    }
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_home(editor: &mut Editor) -> anyhow::Result<()> {
    if let Some(view) = editor.active_view_mut() {
        view.cursor.line = 0;
        view.cursor.column = 0;
        view.scroll.top_line = 0;
    }
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_end(editor: &mut Editor) -> anyhow::Result<()> {
    let line_count = editor
        .active_document()
        .map(|d| d.buffer.line_count())
        .unwrap_or(0);
    let scroll_off = editor.config.scroll_off;
    if let Some(view) = editor.active_view_mut() {
        view.cursor.line = line_count.saturating_sub(1);
        view.ensure_cursor_visible(scroll_off);
    }
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_mode_insert(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.active_view().is_none() {
        return Ok(());
    }
    editor.switch_mode(EditorMode::Insert);
    Ok(())
}

fn cmd_mode_normal(editor: &mut Editor) -> anyhow::Result<()> {
    // Resolves to Insert under a non-modal keymap, where "leave the overlay /
    // stop editing" should still land the user in the buffer.
    editor.switch_to_default_mode();
    Ok(())
}

fn cmd_tab_next(editor: &mut Editor) -> anyhow::Result<()> {
    editor.tabs.next();
    editor.sync_active_view_to_tab();
    Ok(())
}

fn cmd_tab_prev(editor: &mut Editor) -> anyhow::Result<()> {
    editor.tabs.prev();
    editor.sync_active_view_to_tab();
    Ok(())
}

fn cmd_toggle_sidebar(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.file_explorer.visible {
        if editor.mode == EditorMode::FileExplorer {
            editor.toggle_sidebar();
            editor.switch_to_default_mode();
        } else {
            editor.switch_mode(EditorMode::FileExplorer);
        }
    } else {
        editor.toggle_sidebar();
        editor.switch_mode(EditorMode::FileExplorer);
    }
    Ok(())
}

/// Sync view cursor from document selection (after editing operations).
pub fn sync_cursor_from_selection(editor: &mut Editor) {
    let cursor_pos = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let head = doc.selection.primary().head;
        doc.buffer.byte_to_pos(head)
    };
    let scroll_off = editor.config.scroll_off;
    if let Some(view) = editor.active_view_mut() {
        view.cursor = cursor_pos;
        view.ensure_cursor_visible(scroll_off);
    }
    ensure_h_scroll(editor);
}

/// Adjust horizontal scroll so the cursor column is visible.
///
/// `pub(crate)` for the settings screen: changing `tab_size` or `line_numbers`
/// moves every display column on the cursor's line, and `left_col` was computed
/// against the old ones. That path calls this rather than writing the
/// arithmetic out again -- a second copy of "which column is the cursor drawn
/// in" is the defect this whole area exists to remove.
pub(crate) fn ensure_h_scroll(editor: &mut Editor) {
    let (cursor_display_col, code_width) = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let view = match editor.active_view() {
            Some(v) => v,
            None => return,
        };
        let line = view.cursor.line;
        if line >= doc.buffer.line_count() {
            return;
        }
        let line_text: String = doc.buffer.line(line).into();
        let line_text_trimmed = line_text.trim_end_matches(&['\n', '\r'][..]);
        // The same stops the renderer paints with: correcting `left_col`
        // against a column the widget does not draw the cursor at is what put
        // `End` on a tab-indented line in the wrong place.
        let display_col = TabStops::from_config(&editor.config)
            .col_at_char(line_text_trimmed, view.cursor.column);
        let gutter_width =
            line_number_width_styled(doc.buffer.line_count(), editor.config.line_numbers);
        let code_w = (view.area_width).saturating_sub(gutter_width + 1) as usize;
        (display_col, code_w)
    };
    if let Some(view) = editor.active_view_mut() {
        view.ensure_cursor_visible_h(cursor_display_col, code_width);
    }
}

/// Sync document selection from view cursor (after cursor movement).
pub fn sync_selection_from_cursor(editor: &mut Editor) {
    let byte_pos = {
        let view = match editor.active_view() {
            Some(v) => v,
            None => return,
        };
        let doc = match editor.documents.get(&view.doc_id) {
            Some(d) => d,
            None => return,
        };
        doc.buffer.pos_to_byte(&view.cursor)
    };
    let doc_id = editor.active_view().map(|v| v.doc_id);
    if let Some(doc_id) = doc_id {
        if let Some(doc) = editor.documents.get_mut(&doc_id) {
            doc.selection = Selection::point(byte_pos);
        }
    }
    ensure_h_scroll(editor);
}

/// Insert a character at the cursor position (called from App for Insert mode).
pub fn insert_char(editor: &mut Editor, ch: char) -> anyhow::Result<()> {
    let (byte_pos, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        (doc.selection.primary().head, doc.buffer.len_bytes())
    };
    let s = ch.to_string();
    let new_pos = byte_pos + s.len();
    let txn = Transaction::insert(&s, byte_pos, doc_len).with_selection(Selection::point(new_pos));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    Ok(())
}

/// Clamp cursor column to not exceed line length.
/// In Normal mode, cursor sits ON a character so max is `len - 1`.
/// In Insert mode, cursor can be after the last character so max is `len`.
fn clamp_cursor_column(editor: &mut Editor) {
    let max_col = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let view = match editor.active_view() {
            Some(v) => v,
            None => return,
        };
        let line = view.cursor.line;
        if line >= doc.buffer.line_count() {
            return;
        }
        let line_text: String = doc.buffer.line(line).into();
        let len = line_text
            .trim_end_matches(&['\n', '\r'][..])
            .chars()
            .count();
        if editor.mode == EditorMode::Insert {
            len
        } else {
            len.saturating_sub(1)
        }
    };
    if let Some(view) = editor.active_view_mut() {
        if view.cursor.column > max_col {
            view.cursor.column = max_col;
        }
    }
}

fn cmd_search_open(editor: &mut Editor) -> anyhow::Result<()> {
    editor.search.replace_mode = false;
    editor.search.replace_focused = false;
    editor.search.cursor_pos = editor.search.query.chars().count();
    editor.switch_mode(EditorMode::Search);
    Ok(())
}

fn cmd_search_open_replace(editor: &mut Editor) -> anyhow::Result<()> {
    editor.search.replace_mode = true;
    editor.search.replace_focused = false;
    editor.search.cursor_pos = editor.search.query.chars().count();
    editor.search.replace_cursor_pos = editor.search.replace_text.chars().count();
    editor.switch_mode(EditorMode::Search);
    Ok(())
}

fn cmd_search_next(editor: &mut Editor) -> anyhow::Result<()> {
    editor.search.next_match();
    scroll_to_current_match(editor);
    Ok(())
}

fn cmd_search_prev(editor: &mut Editor) -> anyhow::Result<()> {
    editor.search.prev_match();
    scroll_to_current_match(editor);
    Ok(())
}

fn cmd_search_replace_current(editor: &mut Editor) -> anyhow::Result<()> {
    let m = match editor.search.current() {
        Some(m) => (m.start, m.end),
        None => return Ok(()),
    };
    let replace_text = editor.search.replace_text.clone();
    let doc_len = match editor.active_document() {
        Some(doc) => doc.buffer.len_bytes(),
        None => return Ok(()),
    };

    let txn = Transaction::replace(m.0..m.1, &replace_text, doc_len);
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;

    rerun_search(editor);
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_search_replace_all(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.search.matches.is_empty() {
        return Ok(());
    }
    let replace_text = editor.search.replace_text.clone();

    // Apply replacements in reverse order to preserve byte offsets
    let matches: Vec<(usize, usize)> = editor
        .search
        .matches
        .iter()
        .rev()
        .map(|m| (m.start, m.end))
        .collect();

    for (start, end) in matches {
        let doc_len = match editor.active_document() {
            Some(doc) => doc.buffer.len_bytes(),
            None => return Ok(()),
        };
        let txn = Transaction::replace(start..end, &replace_text, doc_len);
        editor
            .active_document_mut()
            .unwrap()
            .apply_transaction(&txn)?;
    }

    rerun_search(editor);
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_search_close(editor: &mut Editor) -> anyhow::Result<()> {
    editor.search.matches.clear();
    editor.search.current_match = None;
    editor.switch_to_default_mode();
    Ok(())
}

fn cmd_fuzzy_open(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.fuzzy_finder.all_files.is_empty() {
        let root = editor.file_explorer.root.clone();
        let respect_gitignore = editor.file_tree_style.respect_gitignore;
        let show_hidden = editor.file_tree_style.show_hidden_files;
        editor
            .fuzzy_finder
            .load_files(&root, respect_gitignore, show_hidden);
    }
    editor.fuzzy_finder.query.clear();
    editor.fuzzy_finder.cursor_pos = 0;
    editor.fuzzy_finder.update_filter();
    editor.switch_mode(EditorMode::FuzzyFinder);
    Ok(())
}

fn cmd_fuzzy_close(editor: &mut Editor) -> anyhow::Result<()> {
    editor.switch_to_default_mode();
    Ok(())
}

/// `App::dispatch_command` always intercepts `palette.open` and calls
/// `App::open_command_palette`, which is the only place that can populate the
/// command list. This handler exists so the id is registered and bindable; it
/// must stay a noop rather than opening an empty palette.
fn cmd_palette_open(_editor: &mut Editor) -> anyhow::Result<()> {
    Ok(())
}

fn cmd_palette_close(editor: &mut Editor) -> anyhow::Result<()> {
    editor.switch_to_default_mode();
    Ok(())
}

/// Re-run search after text changes (replace operations).
pub fn rerun_search(editor: &mut Editor) {
    if editor.search.query.is_empty() {
        editor.search.matches.clear();
        editor.search.current_match = None;
        return;
    }
    let text = match editor.active_document() {
        Some(doc) => doc.buffer.text().to_string(),
        None => return,
    };
    editor.search.find_matches(&text);
}

/// Scroll editor view to current search match.
fn scroll_to_current_match(editor: &mut Editor) {
    let byte_pos = match editor.search.current() {
        Some(m) => m.start,
        None => return,
    };
    let pos = match editor.active_document() {
        Some(doc) => doc.buffer.byte_to_pos(byte_pos),
        None => return,
    };
    let scroll_off = editor.config.scroll_off;
    if let Some(view) = editor.active_view_mut() {
        view.cursor = pos;
        view.ensure_cursor_visible(scroll_off);
    }
    sync_selection_from_cursor(editor);
}

fn cmd_diagnostic_next(editor: &mut Editor) -> anyhow::Result<()> {
    let (cursor_line, cursor_col) = {
        let view = editor
            .active_view()
            .ok_or_else(|| anyhow::anyhow!("No active view"))?;
        (view.cursor.line, view.cursor.column)
    };
    let doc = editor
        .active_document()
        .ok_or_else(|| anyhow::anyhow!("No active document"))?;

    let next = doc
        .diagnostics
        .iter()
        .filter(|d| {
            d.range.0.line > cursor_line
                || (d.range.0.line == cursor_line && d.range.0.column > cursor_col)
        })
        .min_by_key(|d| (d.range.0.line, d.range.0.column));

    let target = next.or_else(|| {
        doc.diagnostics
            .iter()
            .min_by_key(|d| (d.range.0.line, d.range.0.column))
    });

    if let Some(diag) = target {
        let line = diag.range.0.line;
        let col = diag.range.0.column;
        let msg = diag.message.clone();
        if let Some(view) = editor.active_view_mut() {
            view.cursor.line = line;
            view.cursor.column = col;
        }
        editor.status_message = Some(msg);
    } else {
        editor.status_message = Some("No diagnostics".to_string());
    }
    Ok(())
}

fn cmd_diagnostic_prev(editor: &mut Editor) -> anyhow::Result<()> {
    let (cursor_line, cursor_col) = {
        let view = editor
            .active_view()
            .ok_or_else(|| anyhow::anyhow!("No active view"))?;
        (view.cursor.line, view.cursor.column)
    };
    let doc = editor
        .active_document()
        .ok_or_else(|| anyhow::anyhow!("No active document"))?;

    let prev = doc
        .diagnostics
        .iter()
        .filter(|d| {
            d.range.0.line < cursor_line
                || (d.range.0.line == cursor_line && d.range.0.column < cursor_col)
        })
        .max_by_key(|d| (d.range.0.line, d.range.0.column));

    let target = prev.or_else(|| {
        doc.diagnostics
            .iter()
            .max_by_key(|d| (d.range.0.line, d.range.0.column))
    });

    if let Some(diag) = target {
        let line = diag.range.0.line;
        let col = diag.range.0.column;
        let msg = diag.message.clone();
        if let Some(view) = editor.active_view_mut() {
            view.cursor.line = line;
            view.cursor.column = col;
        }
        editor.status_message = Some(msg);
    } else {
        editor.status_message = Some("No diagnostics".to_string());
    }
    Ok(())
}

/// Clamp cursor column for rightward movement (allow up to line length).
fn clamp_cursor_column_right(editor: &mut Editor) {
    let max_col = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let view = match editor.active_view() {
            Some(v) => v,
            None => return,
        };
        let line = view.cursor.line;
        if line >= doc.buffer.line_count() {
            return;
        }
        let line_text: String = doc.buffer.line(line).into();
        line_text
            .trim_end_matches(&['\n', '\r'][..])
            .chars()
            .count()
    };
    if let Some(view) = editor.active_view_mut() {
        if view.cursor.column > max_col {
            view.cursor.column = max_col;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deeply tab-indented line: 13 tabs of indent and 40 characters of code,
    /// the shape from the defect report.
    const INDENTED: &str = "\t\t\t\t\t\t\t\t\t\t\t\t\txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";

    fn editor_with_the_indented_line(name: &str, tab_size: usize) -> (Editor, std::path::PathBuf) {
        use termcode_core::config_types::EditorConfig;
        use termcode_syntax::language::LanguageRegistry;
        use termcode_theme::theme::Theme;

        let dir = std::env::temp_dir().join("termcode-hscroll-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}-{tab_size}.txt"));
        std::fs::write(&path, format!("{INDENTED}\n")).unwrap();

        let config = EditorConfig {
            tab_size,
            ..EditorConfig::default()
        };
        let mut editor = Editor::new(Theme::default(), config, LanguageRegistry::new(), None);
        editor.open_file(&path).unwrap();
        let view = editor.active_view_mut().unwrap();
        view.area_width = 59;
        view.area_height = 20;
        (editor, path)
    }

    #[test]
    fn end_on_a_tab_indented_line_scrolls_to_where_the_cursor_is_drawn() {
        // FR-TAB-007. `left_col` is corrected against the cursor's display
        // column, so it has to be the column the *renderer* puts the cursor at.
        // Before the fix the indent measured 0 columns, so `End` on this line
        // left `left_col` 52 columns short at `tab_size = 4` and 104 short at 8
        // -- the cursor was past the right edge of a viewport that thought it
        // had arrived.
        use ratatui::widgets::Widget;

        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_the_indented_line("end-of-line", tab_size);
            let tabs = crate::display_width::TabStops::from_config(&editor.config);
            let last = INDENTED.chars().count();
            editor.active_view_mut().unwrap().cursor.column = last;
            ensure_h_scroll(&mut editor);

            let code_width = 59usize - 4;
            let view = editor.active_view().unwrap();
            let display_col = tabs.col_at_char(INDENTED, last);
            assert!(
                (view.scroll.left_col..view.scroll.left_col + code_width).contains(&display_col),
                "tab_size={tab_size}: cursor column {display_col} is outside \
                 [{}, {})",
                view.scroll.left_col,
                view.scroll.left_col + code_width
            );

            // And the frame agrees: the reversed cell is inside the code area.
            let area = ratatui::layout::Rect::new(20, 2, 59, 20);
            let mut buf = ratatui::buffer::Buffer::empty(area);
            let doc = editor.active_document().unwrap();
            crate::ui::editor_view::EditorViewWidget::new(
                doc,
                view,
                &editor.theme,
                editor.mode,
                None,
                &editor.config,
                true,
            )
            .render(area, &mut buf);

            let reversed = (area.x..area.x + area.width)
                .find(|x| {
                    buf[(*x, area.y)]
                        .style()
                        .add_modifier
                        .contains(ratatui::style::Modifier::REVERSED)
                })
                .unwrap_or_else(|| panic!("tab_size={tab_size}: the cursor was not drawn at all"));
            assert!(
                reversed >= area.x + 4 && reversed < area.x + area.width,
                "tab_size={tab_size}: the cursor was drawn at column {reversed}, outside the code area"
            );
        }
    }

    #[test]
    fn the_cursor_at_the_start_of_a_tab_indented_line_needs_no_scroll() {
        for tab_size in [4usize, 8, 2] {
            let (mut editor, _p) = editor_with_the_indented_line("start-of-line", tab_size);
            editor.active_view_mut().unwrap().cursor.column = 0;
            ensure_h_scroll(&mut editor);
            assert_eq!(
                editor.active_view().unwrap().scroll.left_col,
                0,
                "tab_size={tab_size}"
            );
        }
    }

    #[test]
    fn hidden_commands_are_excluded_from_the_palette() {
        let mut registry = CommandRegistry::new();
        register_builtin_commands(&mut registry);
        let ids: Vec<&str> = registry
            .list_commands()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert!(!ids.contains(&"explorer.down"));
        assert!(!ids.contains(&"palette.up"));
        // Still bindable even though hidden.
        assert!(registry.get_by_string("explorer.down").is_some());
        // Visible commands are unaffected.
        assert!(ids.contains(&"file.save"));
        assert!(ids.contains(&"tab.close"));
    }
}
