//! Word motions and line-relative jumps that modal keymaps rely on.
//!
//! Word boundaries follow Vim's default rules, since the presets that bind
//! these commands (`w`, `b`, `e`) come with that expectation.

use termcode_core::selection::Selection;
use termcode_core::transaction::Transaction;
use termcode_view::editor::Editor;

use super::{
    CommandEntry, CommandRegistry, clamp_cursor_column, sync_cursor_from_selection,
    sync_selection_from_cursor,
};

pub(super) fn register_motion_commands(registry: &mut CommandRegistry) {
    registry.register(CommandEntry {
        id: "cursor.word_next",
        name: "Next Word",
        handler: cmd_word_next,
    });
    registry.register(CommandEntry {
        id: "cursor.word_prev",
        name: "Previous Word",
        handler: cmd_word_prev,
    });
    registry.register(CommandEntry {
        id: "cursor.word_end",
        name: "End of Word",
        handler: cmd_word_end,
    });
    registry.register(CommandEntry {
        id: "cursor.line_first_nonblank",
        name: "Go to First Non-Blank",
        handler: cmd_line_first_nonblank,
    });
    registry.register(CommandEntry {
        id: "edit.delete_word_before",
        name: "Delete Word Before Cursor",
        handler: cmd_delete_word_before,
    });
}

/// How a character participates in word motions. This follows Vim's default
/// split: a run of word characters and a run of punctuation are separate words,
/// and whitespace separates both.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

fn char_class(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Whitespace
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Punctuation
    }
}

/// Char index of the cursor in the active document.
fn cursor_char_index(editor: &Editor) -> Option<usize> {
    let doc = editor.active_document()?;
    let view = editor.active_view()?;
    let byte = doc.buffer.pos_to_byte(&view.cursor);
    Some(doc.buffer.text().byte_to_char(byte))
}

/// Move the cursor to a char index, clamping to the end of the buffer.
fn move_to_char_index(editor: &mut Editor, char_idx: usize) {
    let pos = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let rope = doc.buffer.text();
        let clamped = char_idx.min(rope.len_chars());
        doc.buffer.byte_to_pos(rope.char_to_byte(clamped))
    };
    let scroll_off = editor.config.scroll_off;
    if let Some(view) = editor.active_view_mut() {
        view.cursor = pos;
        view.ensure_cursor_visible(scroll_off);
    }
    sync_selection_from_cursor(editor);
}

fn cmd_word_next(editor: &mut Editor) -> anyhow::Result<()> {
    let target = {
        let idx = match cursor_char_index(editor) {
            Some(i) => i,
            None => return Ok(()),
        };
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return Ok(()),
        };
        let rope = doc.buffer.text();
        let len = rope.len_chars();
        let mut i = idx;
        if i < len {
            let start = char_class(rope.char(i));
            if start != CharClass::Whitespace {
                while i < len && char_class(rope.char(i)) == start {
                    i += 1;
                }
            }
            while i < len && char_class(rope.char(i)) == CharClass::Whitespace {
                i += 1;
            }
        }
        i
    };
    move_to_char_index(editor, target);
    Ok(())
}

/// Char index of the start of the word before `idx`: skip any whitespace
/// immediately behind the cursor, then walk back over the run it lands in.
fn prev_word_start(editor: &Editor, idx: usize) -> usize {
    let Some(doc) = editor.active_document() else {
        return idx;
    };
    let rope = doc.buffer.text();
    if idx == 0 {
        return 0;
    }
    let mut i = idx - 1;
    while i > 0 && char_class(rope.char(i)) == CharClass::Whitespace {
        i -= 1;
    }
    if char_class(rope.char(i)) == CharClass::Whitespace {
        return 0;
    }
    let cls = char_class(rope.char(i));
    while i > 0 && char_class(rope.char(i - 1)) == cls {
        i -= 1;
    }
    i
}

fn cmd_word_prev(editor: &mut Editor) -> anyhow::Result<()> {
    let Some(idx) = cursor_char_index(editor) else {
        return Ok(());
    };
    let target = prev_word_start(editor, idx);
    move_to_char_index(editor, target);
    Ok(())
}

/// Delete from the start of the previous word up to the cursor, as Vim's and
/// readline's Insert-mode `Ctrl+W` do.
fn cmd_delete_word_before(editor: &mut Editor) -> anyhow::Result<()> {
    let Some(idx) = cursor_char_index(editor) else {
        return Ok(());
    };
    let start_char = prev_word_start(editor, idx);
    if start_char >= idx {
        return Ok(());
    }
    let (from, to, doc_len) = {
        let doc = editor
            .active_document()
            .ok_or_else(|| anyhow::anyhow!("No active document"))?;
        let rope = doc.buffer.text();
        (
            rope.char_to_byte(start_char),
            rope.char_to_byte(idx),
            doc.buffer.len_bytes(),
        )
    };
    let txn = Transaction::delete(from..to, doc_len).with_selection(Selection::point(from));
    editor
        .active_document_mut()
        .unwrap()
        .apply_transaction(&txn)?;
    sync_cursor_from_selection(editor);
    Ok(())
}

fn cmd_word_end(editor: &mut Editor) -> anyhow::Result<()> {
    let target = {
        let idx = match cursor_char_index(editor) {
            Some(i) => i,
            None => return Ok(()),
        };
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return Ok(()),
        };
        let rope = doc.buffer.text();
        let len = rope.len_chars();
        if len == 0 {
            0
        } else {
            let mut i = idx + 1;
            while i < len && char_class(rope.char(i)) == CharClass::Whitespace {
                i += 1;
            }
            if i >= len {
                // No word after the cursor. Staying put beats landing on the
                // trailing newline, which is where `len - 1` points for the
                // usual newline-terminated buffer.
                idx
            } else {
                let cls = char_class(rope.char(i));
                while i + 1 < len && char_class(rope.char(i + 1)) == cls {
                    i += 1;
                }
                i
            }
        }
    };
    move_to_char_index(editor, target);
    Ok(())
}

pub(super) fn cmd_line_first_nonblank(editor: &mut Editor) -> anyhow::Result<()> {
    let col = {
        let doc = match editor.active_document() {
            Some(d) => d,
            None => return Ok(()),
        };
        let view = match editor.active_view() {
            Some(v) => v,
            None => return Ok(()),
        };
        if view.cursor.line >= doc.buffer.line_count() {
            return Ok(());
        }
        let line_text: String = doc.buffer.line(view.cursor.line).into();
        // A line with nothing but whitespace has no first non-blank, so the
        // cursor belongs at its start. Treating the line break as the target
        // would instead park the cursor past the last character, and would give
        // a different answer for the final line, which has no break at all.
        line_text
            .chars()
            .take_while(|c| *c != '\n' && *c != '\r')
            .position(|c| !c.is_whitespace())
            .unwrap_or(0)
    };
    if let Some(view) = editor.active_view_mut() {
        view.cursor.column = col;
    }
    clamp_cursor_column(editor);
    sync_selection_from_cursor(editor);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::test_support::{cursor, editor_with, set_cursor, text};

    #[test]
    fn word_next_skips_to_following_word() {
        let (mut editor, _f) = editor_with("wnext", "foo bar_baz  qux\n");
        set_cursor(&mut editor, 0, 0);
        cmd_word_next(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 4));
        cmd_word_next(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 13));
    }

    #[test]
    fn word_next_treats_punctuation_as_its_own_word() {
        let (mut editor, _f) = editor_with("wpunct", "foo.bar\n");
        set_cursor(&mut editor, 0, 0);
        cmd_word_next(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 3));
        cmd_word_next(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 4));
    }

    #[test]
    fn word_prev_returns_to_word_start() {
        let (mut editor, _f) = editor_with("wprev", "foo bar baz\n");
        set_cursor(&mut editor, 0, 9);
        cmd_word_prev(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 8));
        cmd_word_prev(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 4));
    }

    #[test]
    fn word_end_lands_on_last_char_of_word() {
        let (mut editor, _f) = editor_with("wend", "foo bar\n");
        set_cursor(&mut editor, 0, 0);
        cmd_word_end(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 2));
        cmd_word_end(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 6));
    }

    #[test]
    fn delete_word_before_removes_the_preceding_word() {
        let (mut editor, _f) = editor_with("dwb", "hello world\n");
        set_cursor(&mut editor, 0, 11);
        cmd_delete_word_before(&mut editor).unwrap();
        assert_eq!(text(&editor), "hello \n");
        assert_eq!(cursor(&editor), (0, 6));
    }

    #[test]
    fn delete_word_before_eats_trailing_whitespace_with_the_word() {
        let (mut editor, _f) = editor_with("dwb_ws", "hello world   \n");
        set_cursor(&mut editor, 0, 14);
        cmd_delete_word_before(&mut editor).unwrap();
        assert_eq!(text(&editor), "hello \n");
    }

    #[test]
    fn delete_word_before_is_a_noop_at_the_start_of_a_buffer() {
        let (mut editor, _f) = editor_with("dwb_start", "abc\n");
        set_cursor(&mut editor, 0, 0);
        cmd_delete_word_before(&mut editor).unwrap();
        assert_eq!(text(&editor), "abc\n");
    }

    #[test]
    fn word_end_stays_put_at_the_last_word() {
        let (mut editor, _f) = editor_with("wend_last", "foo bar\n");
        set_cursor(&mut editor, 0, 6); // on the 'r' of "bar"
        cmd_word_end(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 6));
    }

    #[test]
    fn first_nonblank_skips_indentation() {
        let (mut editor, _f) = editor_with("fnb", "    indented\n");
        set_cursor(&mut editor, 0, 10);
        cmd_line_first_nonblank(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (0, 4));
    }

    #[test]
    fn first_nonblank_on_a_whitespace_only_line_goes_to_the_start() {
        // Both spellings of "blank line" must agree: with a trailing break...
        let (mut editor, _f) = editor_with("fnb_ws", "one\n   \ntwo\n");
        set_cursor(&mut editor, 1, 2);
        cmd_line_first_nonblank(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (1, 0));
    }

    #[test]
    fn first_nonblank_on_an_unterminated_whitespace_line_goes_to_the_start() {
        // ...and without one.
        let (mut editor, _f) = editor_with("fnb_ws_end", "one\n   ");
        set_cursor(&mut editor, 1, 2);
        cmd_line_first_nonblank(&mut editor).unwrap();
        assert_eq!(cursor(&editor), (1, 0));
    }
}
