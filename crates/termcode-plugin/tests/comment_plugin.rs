//! Exercises the shipped `comment` plugin against a real editor, so the Lua in
//! `runtime/plugins/comment` is covered rather than a copy of it in a string.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use termcode_config::config::PluginConfig;
use termcode_core::config_types::EditorConfig;
use termcode_plugin::manager::PluginManager;
use termcode_syntax::language::LanguageRegistry;
use termcode_theme::theme::Theme;
use termcode_view::editor::Editor;

fn runtime_plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("runtime/plugins")
}

fn manager() -> PluginManager {
    let config = PluginConfig {
        enabled: true,
        plugin_dirs: Vec::new(),
        instruction_limit: 1_000_000,
        memory_limit_mb: 10,
        overrides: HashMap::new(),
    };
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[runtime_plugins_dir()]);
    manager
}

/// Opens `contents` under a name with the given extension and returns the
/// editor plus the temp dir keeping the file alive.
fn editor_with(name: &str, contents: &str) -> (Editor, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join(name);
    fs::write(&file, contents).unwrap();

    let mut editor = Editor::new(
        Theme::default(),
        EditorConfig::default(),
        LanguageRegistry::new(),
        None,
    );
    editor.open_file(&file).unwrap();
    (editor, tmp)
}

fn text_of(editor: &Editor) -> String {
    editor.active_document().unwrap().buffer.text().to_string()
}

fn toggle(manager: &mut PluginManager, editor: &mut Editor) {
    manager
        .execute_command("plugin.comment.toggle", editor)
        .unwrap();
}

#[test]
fn comments_and_uncomments_the_cursor_line() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.rs", "let x = 1;\nlet y = 2;\n");

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "// let x = 1;\nlet y = 2;\n");

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "let x = 1;\nlet y = 2;\n");
}

#[test]
fn markers_align_on_the_shallowest_line() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.rs", "    if a {\n        b();\n    }\n");

    // Select all three lines.
    select_lines(&mut editor, 0, 2);
    toggle(&mut pm, &mut editor);
    assert_eq!(
        text_of(&editor),
        "    // if a {\n    //     b();\n    // }\n"
    );

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "    if a {\n        b();\n    }\n");
}

#[test]
fn a_single_bare_line_makes_the_whole_block_comment() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.py", "# done\nnot done\n");

    select_lines(&mut editor, 0, 1);
    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "# # done\n# not done\n");
}

#[test]
fn blank_lines_are_left_alone() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.lua", "local a\n\nlocal b\n");

    select_lines(&mut editor, 0, 2);
    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "-- local a\n\n-- local b\n");
}

#[test]
fn block_syntax_wraps_each_line() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.html", "<p>one</p>\n<p>two</p>\n");

    select_lines(&mut editor, 0, 1);
    toggle(&mut pm, &mut editor);
    assert_eq!(
        text_of(&editor),
        "<!-- <p>one</p> -->\n<!-- <p>two</p> -->\n"
    );

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "<p>one</p>\n<p>two</p>\n");
}

#[test]
fn crlf_endings_survive_the_round_trip() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.rs", "let x = 1;\r\nlet y = 2;\r\n");

    select_lines(&mut editor, 0, 1);
    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "// let x = 1;\r\n// let y = 2;\r\n");

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "let x = 1;\r\nlet y = 2;\r\n");
}

#[test]
fn an_unknown_extension_is_refused_rather_than_guessed() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.unknownext", "some text\n");

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "some text\n");
    assert!(
        editor
            .status_message
            .as_deref()
            .unwrap()
            .contains("No comment syntax")
    );
}

#[test]
fn a_selection_ending_at_column_one_does_not_reach_that_line() {
    let mut pm = manager();
    let (mut editor, _tmp) = editor_with("a.rs", "one();\ntwo();\nthree();\n");

    // Lines 1-2 selected, head resting at the start of line 3.
    let doc = editor.active_document_mut().unwrap();
    let start = doc
        .buffer
        .pos_to_byte(&termcode_core::position::Position::new(0, 0));
    let end = doc
        .buffer
        .pos_to_byte(&termcode_core::position::Position::new(2, 0));
    doc.selection = termcode_core::selection::Selection::new(
        vec![termcode_core::selection::Range::new(start, end)],
        0,
    );

    toggle(&mut pm, &mut editor);
    assert_eq!(text_of(&editor), "// one();\n// two();\nthree();\n");
}

fn select_lines(editor: &mut Editor, first: usize, last: usize) {
    let doc = editor.active_document_mut().unwrap();
    let start = doc
        .buffer
        .pos_to_byte(&termcode_core::position::Position::new(first, 0));
    let line_len = doc.buffer.line(last).len_chars();
    let end = doc
        .buffer
        .pos_to_byte(&termcode_core::position::Position::new(last, line_len));
    doc.selection = termcode_core::selection::Selection::new(
        vec![termcode_core::selection::Range::new(start, end)],
        0,
    );
}
