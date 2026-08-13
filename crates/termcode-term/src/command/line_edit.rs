//! Line-oriented editing and the insert-entry variants modal keymaps bind to
//! `dd`, `yy`, `p`, `a`, `o` and friends.

use termcode_core::selection::Selection;
use termcode_core::transaction::Transaction;
use termcode_view::editor::{Editor, EditorMode};

use super::motion::cmd_line_first_nonblank;
use super::{
    CommandEntry, CommandRegistry, clamp_cursor_column, clamp_cursor_column_right, cmd_line_end,
    sync_cursor_from_selection, sync_selection_from_cursor,
};

pub(super) fn register_line_edit_commands(registry: &mut CommandRegistry) {
    registry.register(CommandEntry {
        id: "edit.delete_line",
        name: "Delete Line",
        handler: cmd_delete_line,
    });
    registry.register(CommandEntry {
        id: "edit.yank_line",
        name: "Yank Line",
        handler: cmd_yank_line,
    });
    registry.register(CommandEntry {
        id: "edit.paste_after",
        name: "Paste After Cursor",
        handler: cmd_paste_after,
    });
    registry.register(CommandEntry {
        id: "edit.paste_before",
        name: "Paste Before Cursor",
        handler: cmd_paste_before,
    });
    registry.register(CommandEntry {
        id: "edit.open_below",
        name: "Open Line Below",
        handler: cmd_open_below,
    });
    registry.register(CommandEntry {
        id: "edit.open_above",
        name: "Open Line Above",
        handler: cmd_open_above,
    });
    registry.register(CommandEntry {
        id: "mode.insert_after",
        name: "Insert After Cursor",
        handler: cmd_mode_insert_after,
    });
    registry.register(CommandEntry {
        id: "mode.insert_line_start",
        name: "Insert at Line Start",
        handler: cmd_mode_insert_line_start,
    });
    registry.register(CommandEntry {
        id: "mode.insert_line_end",
        name: "Insert at Line End",
        handler: cmd_mode_insert_line_end,
    });
}

/// Byte range covering `line` including its trailing line break.
///
/// The final line of a buffer has no trailing break, so deleting it would leave
/// a stray empty line behind; in that case the range is extended backwards over
/// the break that terminates the previous line instead.
fn line_delete_range(editor: &Editor, line: usize) -> Option<std::ops::Range<usize>> {
    let doc = editor.active_document()?;
    let rope = doc.buffer.text();
    let line_count = rope.len_lines();
    if line >= line_count {
        return None;
    }
    let mut start = rope.line_to_byte(line);
    let end = if line + 1 < line_count {
        rope.line_to_byte(line + 1)
    } else {
        rope.len_bytes()
    };
    if line + 1 >= line_count && line > 0 {
        let mut ci = rope.byte_to_char(start);
        if ci > 0 && rope.char(ci - 1) == '\n' {
            ci -= 1;
            if ci > 0 && rope.char(ci - 1) == '\r' {
                ci -= 1;
            }
        }
        start = rope.char_to_byte(ci);
    }
    if start >= end { None } else { Some(start..end) }
}

/// Text of `line` including its trailing line break, normalised to end with `\n`
/// so it round-trips through a line-wise paste.
fn line_text_with_break(editor: &Editor, line: usize) -> Option<String> {
    let doc = editor.active_document()?;
    if line >= doc.buffer.line_count() {
        return None;
    }
    let text: String = doc.buffer.line(line).into();
    if text.ends_with('\n') {
        Some(text)
    } else {
        Some(format!("{text}\n"))
    }
}

fn cmd_delete_line(editor: &mut Editor) -> anyhow::Result<()> {
    let line = match editor.active_view() {
        Some(v) => v.cursor.line,
        None => return Ok(()),
    };
    let range = match line_delete_range(editor, line) {
        Some(r) => r,
        None => return Ok(()),
    };
    let doc_len = editor
        .active_document()
        .ok_or_else(|| anyhow::anyhow!("No active document"))?
        .buffer
        .len_bytes();
    let start = range.start;
    let txn = Transaction::delete(range, doc_len).with_selection(Selection::point(start));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_yank_line(editor: &mut Editor) -> anyhow::Result<()> {
    let line = match editor.active_view() {
        Some(v) => v.cursor.line,
        None => return Ok(()),
    };
    let text = match line_text_with_break(editor, line) {
        Some(t) => t,
        None => return Ok(()),
    };
    let clipboard = editor
        .clipboard
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("Clipboard unavailable"))?;
    clipboard.set_text(&text)?;
    editor.status_message = Some("Yanked 1 line".to_string());
    Ok(())
}

/// Insert clipboard text relative to the cursor.
///
/// Text ending in a line break pastes line-wise (onto its own line, below or
/// above the current one, like Vim's `p`/`P`); anything else pastes inline.
fn paste_relative(editor: &mut Editor, after: bool) -> anyhow::Result<()> {
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
    let line_wise = text.ends_with('\n');

    let (insert_at, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        let doc_len = doc.buffer.len_bytes();
        if line_wise {
            let line = editor.active_view().map(|v| v.cursor.line).unwrap_or(0);
            let rope = doc.buffer.text();
            let target_line = if after { line + 1 } else { line };
            let at = if target_line < rope.len_lines() {
                rope.line_to_byte(target_line)
            } else {
                doc_len
            };
            (at, doc_len)
        } else {
            let head = doc.selection.primary().head;
            let at = if after {
                let rope = doc.buffer.text();
                let ci = rope.byte_to_char(head);
                // Never step over a line terminator. On an empty line the cursor
                // already sits on the break, and stepping into a `\r\n` pair
                // would split it.
                if ci < rope.len_chars() && !matches!(rope.char(ci), '\n' | '\r') {
                    rope.char_to_byte(ci + 1)
                } else {
                    head
                }
            } else {
                head
            };
            (at, doc_len)
        }
    };

    // A line-wise paste at the very end of a buffer with no trailing break needs
    // one inserted first, otherwise it would join onto the last line.
    let needs_leading_break = line_wise && insert_at == doc_len && doc_len > 0 && {
        let rope = editor.active_document().unwrap().buffer.text();
        rope.char(rope.len_chars() - 1) != '\n'
    };
    // Move the whole break to the front, `\r\n` included: stripping only the
    // `\n` would strand a bare `\r` at the end of a CRLF document.
    let payload = if needs_leading_break {
        let body = text
            .strip_suffix("\r\n")
            .or_else(|| text.strip_suffix('\n'))
            .unwrap_or(&text);
        format!("\n{body}")
    } else {
        text
    };

    let cursor_at = if line_wise {
        insert_at + usize::from(needs_leading_break)
    } else {
        insert_at + payload.len()
    };
    let txn = Transaction::insert(&payload, insert_at, doc_len)
        .with_selection(Selection::point(cursor_at));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_paste_after(editor: &mut Editor) -> anyhow::Result<()> {
    paste_relative(editor, true)
}

fn cmd_paste_before(editor: &mut Editor) -> anyhow::Result<()> {
    paste_relative(editor, false)
}

/// Open a blank line below or above the cursor line and enter Insert mode.
fn open_line(editor: &mut Editor, below: bool) -> anyhow::Result<()> {
    if editor.active_view().is_none() {
        return Ok(());
    }
    let (insert_at, doc_len, appended_at_eof) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        let line = editor.active_view().map(|v| v.cursor.line).unwrap_or(0);
        let rope = doc.buffer.text();
        let doc_len = doc.buffer.len_bytes();
        // The cursor line carries no trailing break only when it is the very
        // last line ropey reports. A newline-terminated buffer always has a
        // phantom empty line after it, so `line + 1` still indexes a line there.
        let unterminated = line + 1 >= rope.len_lines();
        let at = if below {
            if unterminated {
                doc_len
            } else {
                rope.line_to_byte(line + 1)
            }
        } else {
            rope.line_to_byte(line.min(rope.len_lines().saturating_sub(1)))
        };
        (at, doc_len, below && unterminated)
    };

    // Inserting the break at the start of a line makes that offset the start of
    // the new empty line. The exception is opening below a final line that has
    // no trailing break: there the break is appended, so the new line starts one
    // byte later. Testing `insert_at == doc_len` is not enough — that is also
    // true for the last real line of a newline-terminated buffer, where the
    // insert lands at the start of the phantom line.
    let cursor_at = if appended_at_eof {
        insert_at + 1
    } else {
        insert_at
    };
    let txn =
        Transaction::insert("\n", insert_at, doc_len).with_selection(Selection::point(cursor_at));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;

    editor.switch_mode(EditorMode::Insert);
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_open_below(editor: &mut Editor) -> anyhow::Result<()> {
    open_line(editor, true)
}

fn cmd_open_above(editor: &mut Editor) -> anyhow::Result<()> {
    open_line(editor, false)
}

fn cmd_mode_insert_after(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.active_view().is_none() {
        return Ok(());
    }
    editor.switch_mode(EditorMode::Insert);
    if let Some(view) = editor.active_view_mut() {
        view.cursor.column += 1;
    }
    clamp_cursor_column_right(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

fn cmd_mode_insert_line_start(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.active_view().is_none() {
        return Ok(());
    }
    editor.switch_mode(EditorMode::Insert);
    cmd_line_first_nonblank(editor)
}

fn cmd_mode_insert_line_end(editor: &mut Editor) -> anyhow::Result<()> {
    if editor.active_view().is_none() {
        return Ok(());
    }
    // Switch first: `cmd_line_end` allows one column further in Insert mode.
    editor.switch_mode(EditorMode::Insert);
    cmd_line_end(editor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::test_support::{cursor, editor_with, set_cursor, text};

    #[test]
    fn line_wise_paste_keeps_crlf_intact_at_eof() {
        let (mut editor, _f) = editor_with("pcrlf", "one");
        set_cursor(&mut editor, 0, 0);
        editor
            .clipboard
            .as_mut()
            .unwrap()
            .set_text("two\r\n")
            .unwrap();
        cmd_paste_after(&mut editor).unwrap();
        // The break moves to the front whole; no stray \r is left behind.
        assert_eq!(text(&editor), "one\ntwo");
    }

    #[test]
    fn delete_line_removes_middle_line_with_its_break() {
        let (mut editor, _f) = editor_with("dd_mid", "one\ntwo\nthree\n");
        set_cursor(&mut editor, 1, 0);
        cmd_delete_line(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\nthree\n");
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn delete_line_on_unterminated_last_line_eats_preceding_break() {
        let (mut editor, _f) = editor_with("dd_last", "one\ntwo");
        set_cursor(&mut editor, 1, 0);
        cmd_delete_line(&mut editor).unwrap();
        assert_eq!(text(&editor), "one");
    }

    #[test]
    fn open_below_puts_cursor_on_new_empty_line() {
        let (mut editor, _f) = editor_with("o_below", "one\ntwo\n");
        set_cursor(&mut editor, 0, 2);
        cmd_open_below(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\n\ntwo\n");
        assert_eq!(cursor(&editor), (1, 0));
        assert_eq!(editor.mode, EditorMode::Insert);
    }

    #[test]
    fn open_below_on_last_line_of_a_newline_terminated_file() {
        // The common case: `o` at the end of a normal file. The cursor must land
        // on the new blank line, not on the phantom line after it.
        let (mut editor, _f) = editor_with("o_below_last_real", "one\ntwo\n");
        set_cursor(&mut editor, 1, 2);
        cmd_open_below(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\ntwo\n\n");
        assert_eq!(cursor(&editor), (2, 0));
    }

    #[test]
    fn open_below_on_unterminated_last_line() {
        let (mut editor, _f) = editor_with("o_below_last", "one");
        set_cursor(&mut editor, 0, 1);
        cmd_open_below(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\n");
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn open_above_puts_cursor_on_new_empty_line() {
        let (mut editor, _f) = editor_with("o_above", "one\ntwo\n");
        set_cursor(&mut editor, 1, 2);
        cmd_open_above(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\n\ntwo\n");
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn yank_then_paste_after_duplicates_line_below() {
        let (mut editor, _f) = editor_with("yyp", "one\ntwo\n");
        set_cursor(&mut editor, 0, 1);
        cmd_yank_line(&mut editor).unwrap();
        cmd_paste_after(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\none\ntwo\n");
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn paste_before_inserts_above_current_line() {
        let (mut editor, _f) = editor_with("pbefore", "one\ntwo\n");
        set_cursor(&mut editor, 1, 0);
        editor
            .clipboard
            .as_mut()
            .unwrap()
            .set_text("mid\n")
            .unwrap();
        cmd_paste_before(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\nmid\ntwo\n");
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn line_wise_paste_at_unterminated_end_adds_break_first() {
        let (mut editor, _f) = editor_with("pend", "one");
        set_cursor(&mut editor, 0, 0);
        editor
            .clipboard
            .as_mut()
            .unwrap()
            .set_text("two\n")
            .unwrap();
        cmd_paste_after(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\ntwo");
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn charwise_paste_after_inserts_past_cursor() {
        let (mut editor, _f) = editor_with("pchar", "ac\n");
        set_cursor(&mut editor, 0, 0);
        editor.clipboard.as_mut().unwrap().set_text("b").unwrap();
        cmd_paste_after(&mut editor).unwrap();
        assert_eq!(text(&editor), "abc\n");
    }

    #[test]
    fn charwise_paste_on_an_empty_line_stays_on_that_line() {
        let (mut editor, _f) = editor_with("pchar_empty", "one\n\ntwo\n");
        set_cursor(&mut editor, 1, 0); // the empty line
        editor.clipboard.as_mut().unwrap().set_text("X").unwrap();
        cmd_paste_after(&mut editor).unwrap();
        assert_eq!(text(&editor), "one\nX\ntwo\n");
    }

    #[test]
    fn charwise_paste_does_not_split_a_crlf_pair() {
        let (mut editor, _f) = editor_with("pchar_crlf", "ab\r\ncd\r\n");
        set_cursor(&mut editor, 0, 2); // just past 'b', on the '\r'
        editor.clipboard.as_mut().unwrap().set_text("X").unwrap();
        cmd_paste_after(&mut editor).unwrap();
        assert_eq!(text(&editor), "abX\r\ncd\r\n");
    }

    #[test]
    fn insert_after_advances_one_column() {
        let (mut editor, _f) = editor_with("append", "abc\n");
        set_cursor(&mut editor, 0, 1);
        cmd_mode_insert_after(&mut editor).unwrap();
        assert_eq!(editor.mode, EditorMode::Insert);
        assert_eq!(cursor(&editor), (0, 2));
    }

    #[test]
    fn insert_line_end_allows_column_past_last_char() {
        let (mut editor, _f) = editor_with("insert_end", "abc\n");
        set_cursor(&mut editor, 0, 0);
        cmd_mode_insert_line_end(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 3));
    }

    #[test]
    fn insert_line_start_lands_on_first_nonblank() {
        let (mut editor, _f) = editor_with("insert_start", "  abc\n");
        set_cursor(&mut editor, 0, 5);
        cmd_mode_insert_line_start(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 2));
    }
}
