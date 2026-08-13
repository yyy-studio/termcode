//! Every shipped keymap preset must parse, resolve to real commands, and avoid
//! sequences that shadow one another. A typo in a preset file is otherwise
//! easy to miss at runtime: unparsable keys, unknown commands and unknown
//! sections are only logged.

use std::collections::HashSet;
use std::path::PathBuf;

use termcode_config::keymap::{KeymapPreset, parse_key_sequence};
use termcode_term::command::{CommandRegistry, register_builtin_commands};

const PRESETS: &[&str] = &["vscode", "vim", "helix"];

fn keymaps_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("runtime/keymaps")
}

fn load(name: &str) -> KeymapPreset {
    let path = keymaps_dir().join(format!("{name}.toml"));
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    toml::from_str(&content).unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()))
}

fn sections(
    preset: &KeymapPreset,
) -> Vec<(&'static str, &std::collections::HashMap<String, String>)> {
    vec![
        ("global", &preset.global),
        ("mode.normal", &preset.modes.normal),
        ("mode.insert", &preset.modes.insert),
        ("mode.file_explorer", &preset.modes.file_explorer),
        ("mode.search", &preset.modes.search),
        ("mode.fuzzy_finder", &preset.modes.fuzzy_finder),
        ("mode.command_palette", &preset.modes.command_palette),
    ]
}

fn registry() -> CommandRegistry {
    let mut r = CommandRegistry::new();
    register_builtin_commands(&mut r);
    r
}

#[test]
fn presets_have_meta() {
    for name in PRESETS {
        let preset = load(name);
        assert_eq!(&preset.meta.name, name, "{name}: meta.name must match file");
        assert!(
            !preset.meta.description.is_empty(),
            "{name}: meta.description is empty"
        );
    }
}

/// Section names are matched by the parser, so a misspelled `[mode.nromal]`
/// would bind nothing at all. The loader reports those; no shipped preset may
/// have any.
#[test]
fn presets_have_no_unknown_sections() {
    for name in PRESETS {
        let warnings = load(name).warnings();
        assert!(warnings.is_empty(), "{name}: {warnings:?}");
    }
}

#[test]
fn every_key_string_parses() {
    for name in PRESETS {
        let preset = load(name);
        for (section, bindings) in sections(&preset) {
            for key in bindings.keys() {
                assert!(
                    parse_key_sequence(key).is_some(),
                    "{name} [{section}]: unparsable key sequence {key:?}"
                );
            }
        }
    }
}

#[test]
fn every_command_exists_in_the_registry() {
    let reg = registry();
    for name in PRESETS {
        let preset = load(name);
        for (section, bindings) in sections(&preset) {
            for cmd in bindings.values() {
                assert!(
                    reg.get_by_string(cmd).is_some(),
                    "{name} [{section}]: unknown command {cmd:?}"
                );
            }
        }
    }
}

/// A binding that is also the prefix of a longer one makes the longer one
/// unreachable, because an exact match fires immediately.
///
/// Resolution settles one table at a time (mode, then global), so a shadow only
/// exists *within* a table: a mode chord is safe from a same-prefix global
/// binding. But `[global]` bindings apply in every non-overlay mode, so each
/// mode table is checked together with `[global]` as well as on its own.
#[test]
fn no_binding_shadows_a_longer_sequence() {
    for name in PRESETS {
        let preset = load(name);
        let global: Vec<(&str, Vec<_>)> = preset
            .global
            .keys()
            .map(|k| ("global", parse_key_sequence(k).unwrap()))
            .collect();

        for (section, bindings) in sections(&preset) {
            let mut seqs: Vec<(&str, Vec<_>)> = bindings
                .keys()
                .map(|k| (section, parse_key_sequence(k).unwrap()))
                .collect();
            if section != "global" {
                seqs.extend(global.iter().cloned());
            }
            assert_no_shadowing(name, &seqs);
        }
    }
}

fn assert_no_shadowing(preset: &str, seqs: &[(&str, Vec<crossterm::event::KeyEvent>)]) {
    let exact: HashSet<&[crossterm::event::KeyEvent]> =
        seqs.iter().map(|(_, s)| s.as_slice()).collect();
    for (section, seq) in seqs {
        for cut in 1..seq.len() {
            assert!(
                !exact.contains(&seq[..cut]),
                "{preset} [{section}]: {seq:?} is unreachable because its \
                 {cut}-key prefix is bound on its own"
            );
        }
    }
}

/// Every preset must leave a way to save, quit and reach the command palette,
/// since a preset replaces the built-in keymap wholesale.
#[test]
fn presets_bind_the_essential_commands() {
    for name in PRESETS {
        let preset = load(name);
        let bound: HashSet<&str> = sections(&preset)
            .iter()
            .flat_map(|(_, b)| b.values().map(|v| v.as_str()))
            .collect();
        for essential in ["file.save", "fuzzy.open", "palette.open", "mode.normal"] {
            assert!(
                bound.contains(essential),
                "{name}: no binding for {essential}"
            );
        }
    }
}

/// Exercised through the same loader the app uses, but against an explicit
/// directory list. The `_in` seam exists so this test neither mutates the
/// process-global CWD (cargo runs tests in parallel threads) nor reads whatever
/// the developer happens to have in `~/.config/termcode/`.
#[test]
fn presets_are_discoverable_by_the_loader() {
    let dirs = vec![keymaps_dir()];

    let found = termcode_config::keymap::list_available_keymaps_in(&dirs);
    for name in PRESETS {
        assert!(
            found.iter().any(|n| n == name),
            "preset {name} not discovered; found {found:?}"
        );
        assert!(
            termcode_config::keymap::load_keymap_preset_in(&dirs, name).is_some(),
            "preset {name} failed to load"
        );
    }
}

/// A user file in `~/.config/termcode/keymaps/` has to be able to replace a
/// shipped preset by name, which only works if that directory outranks the
/// runtime ones.
#[test]
fn the_user_keymap_directory_outranks_the_runtime_ones() {
    let dirs = termcode_config::keymap::keymap_dirs();
    let user_dir = termcode_config::default::config_dir().join("keymaps");
    assert_eq!(
        dirs.first(),
        Some(&user_dir),
        "user keymap directory must be searched first; got {dirs:?}"
    );
}

/// A malformed file must not mask a working preset further down the list.
#[test]
fn a_broken_preset_does_not_hide_a_later_one() {
    let broken_dir =
        std::env::temp_dir().join("termcode-keymap-broken-a_broken_preset_does_not_hide");
    std::fs::create_dir_all(&broken_dir).unwrap();
    std::fs::write(broken_dir.join("vim.toml"), "this is not = valid toml [[[").unwrap();

    let dirs = vec![broken_dir.clone(), keymaps_dir()];
    let loaded = termcode_config::keymap::load_keymap_preset_in(&dirs, "vim");

    let _ = std::fs::remove_dir_all(&broken_dir);
    assert!(
        loaded.is_some_and(|p| p.meta.name == "vim"),
        "the shipped vim preset should still load past the broken file"
    );
}

// ---------------------------------------------------------------------------
// End-to-end resolution through the real preset files
// ---------------------------------------------------------------------------

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use termcode_term::input::{InputMapper, KeyResolution};
use termcode_view::editor::EditorMode;

fn mapper_for(name: &str) -> InputMapper {
    InputMapper::from_preset(&load(name), &registry())
}

fn press(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// Feed a sequence and return the final resolution, asserting every key before
/// the last one leaves the mapper pending.
fn type_seq(mapper: &mut InputMapper, mode: EditorMode, keys: &[KeyEvent]) -> KeyResolution {
    let (last, prefix) = keys.split_last().unwrap();
    for k in prefix {
        assert_eq!(
            mapper.resolve_key(mode, *k),
            KeyResolution::Pending,
            "expected {k:?} to leave a pending chord"
        );
    }
    mapper.resolve_key(mode, *last)
}

#[test]
fn vim_preset_resolves_its_chords() {
    let mut m = mapper_for("vim");
    let cases: &[(&[KeyEvent], &str)] = &[
        (&[press('g'), press('g')], "cursor.home"),
        (&[press('d'), press('d')], "edit.delete_line"),
        (&[press('y'), press('y')], "edit.yank_line"),
        (&[press('g'), press('d')], "goto.definition"),
        (&[press(']'), press('d')], "diagnostic.next"),
        (&[press(' '), press('f')], "fuzzy.open"),
        (&[ctrl('w'), press('q')], "tab.close"),
    ];
    for (keys, expected) in cases {
        assert_eq!(
            type_seq(&mut m, EditorMode::Normal, keys),
            KeyResolution::Match(expected),
            "vim: {keys:?}"
        );
    }
}

#[test]
fn vim_preset_keeps_single_key_bindings() {
    let mut m = mapper_for("vim");
    for (key, expected) in [
        (press('j'), "cursor.down"),
        (press('w'), "cursor.word_next"),
        (press('u'), "edit.undo"),
        (press('p'), "edit.paste_after"),
        (press('/'), "search.open"),
        (press('n'), "search.next"),
    ] {
        assert_eq!(
            m.resolve_key(EditorMode::Normal, key),
            KeyResolution::Match(expected)
        );
    }
}

/// The regression this whole preset mechanism exists for: in the built-in
/// keymap, Ctrl+W in Insert mode closes the tab. Vim users expect it to delete
/// a word, and mode bindings must win over the global table.
#[test]
fn vim_preset_insert_ctrl_w_does_not_close_the_tab() {
    let mut m = mapper_for("vim");
    assert_eq!(
        m.resolve_key(EditorMode::Insert, ctrl('w')),
        KeyResolution::Match("edit.delete_word_before")
    );
    // ...while Normal mode keeps it as the window/tab prefix.
    assert_eq!(
        m.resolve_key(EditorMode::Normal, ctrl('w')),
        KeyResolution::Pending
    );
}

#[test]
fn helix_preset_resolves_space_leader_and_prefixes() {
    let mut m = mapper_for("helix");
    let cases: &[(&[KeyEvent], &str)] = &[
        (&[press(' '), press('f')], "fuzzy.open"),
        (&[press(' '), press('k')], "lsp.hover"),
        (&[press(' '), press('e')], "view.toggle_sidebar"),
        (&[press('g'), press('d')], "goto.definition"),
        (&[press('g'), press('h')], "cursor.line_start"),
        (&[press('x'), press('d')], "edit.delete_line"),
        (&[press('['), press('d')], "diagnostic.prev"),
    ];
    for (keys, expected) in cases {
        assert_eq!(
            type_seq(&mut m, EditorMode::Normal, keys),
            KeyResolution::Match(expected),
            "helix: {keys:?}"
        );
    }
}

/// The VS Code preset is chord-free: every key must resolve on the first press.
#[test]
fn vscode_preset_has_no_pending_chords() {
    let preset = load("vscode");
    for (section, bindings) in sections(&preset) {
        for key in bindings.keys() {
            let seq = parse_key_sequence(key).unwrap();
            assert_eq!(
                seq.len(),
                1,
                "vscode [{section}]: {key:?} is a chord, but this preset is meant to be chord-free"
            );
        }
    }
}

#[test]
fn vscode_preset_routes_the_palette_away_from_ctrl_shift_p() {
    // Terminals cannot distinguish Ctrl+Shift+P from Ctrl+P, so the preset must
    // not rely on it; Ctrl+P has to stay on the file finder.
    let mut m = mapper_for("vscode");
    assert_eq!(
        m.resolve_key(EditorMode::Insert, ctrl('p')),
        KeyResolution::Match("fuzzy.open")
    );
    assert_eq!(
        m.resolve_key(
            EditorMode::Insert,
            KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)
        ),
        KeyResolution::Match("palette.open")
    );
}

/// A keymap with no modal layer must declare it, or the editor opens in Normal
/// mode where that keymap binds almost nothing.
#[test]
fn only_the_non_modal_preset_starts_in_insert() {
    assert!(
        load("vscode").meta.starts_in_insert(),
        "vscode preset must set initial_mode = \"insert\""
    );
    for name in ["vim", "helix"] {
        assert!(
            !load(name).meta.starts_in_insert(),
            "{name} is modal and must rest in Normal mode"
        );
    }
}
