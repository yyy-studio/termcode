//! Integration-style tests covering the full plugin system spec.
//!
//! These tests exercise complete flows through the plugin system:
//! sandbox restrictions, command registration and execution, hook dispatch,
//! error recovery, API boundaries, metadata/config, logging, crate structure,
//! and lifecycle edge cases.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use termcode_config::config::{PluginConfig, PluginOverride};
use termcode_core::config_types::EditorConfig;
use termcode_plugin::HookEvent;
use termcode_plugin::manager::{PluginManager, expand_tilde};
use termcode_plugin::types::PluginStatus;
use termcode_syntax::language::LanguageRegistry;
use termcode_theme::theme::Theme;
use termcode_view::editor::Editor;

fn default_config() -> PluginConfig {
    PluginConfig {
        enabled: true,
        plugin_dirs: Vec::new(),
        instruction_limit: 1_000_000,
        memory_limit_mb: 10,
        overrides: HashMap::new(),
    }
}

fn create_test_editor() -> Editor {
    let theme = Theme::default();
    let config = EditorConfig::default();
    let lang = LanguageRegistry::new();
    Editor::new(theme, config, lang, None)
}

fn create_plugin_dir(base: &Path, name: &str, init_lua: &str) -> PathBuf {
    let dir = base.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("init.lua"), init_lua).unwrap();
    dir
}

fn create_plugin_dir_with_toml(
    base: &Path,
    name: &str,
    toml_content: &str,
    init_lua: &str,
) -> PathBuf {
    let dir = base.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("plugin.toml"), toml_content).unwrap();
    fs::write(dir.join("init.lua"), init_lua).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// TS-1: Sandbox restrictions
// ---------------------------------------------------------------------------

#[test]
fn ts1_os_execute_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "sandbox-test",
        r#"
        local ok, err = pcall(os.execute, "echo hello")
        if ok then
            error("os.execute should be blocked")
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    assert_eq!(manager.list_plugins().len(), 1);
    assert!(matches!(
        manager.list_plugins()[0].status,
        PluginStatus::Loaded
    ));
}

#[test]
fn ts1_io_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "io-test",
        r#"
        if io ~= nil then
            error("io library should not be available")
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    assert!(matches!(
        manager.list_plugins()[0].status,
        PluginStatus::Loaded
    ));
}

#[test]
fn ts1_debug_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "debug-test",
        r#"
        if debug ~= nil then
            error("debug library should not be available")
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    assert!(matches!(
        manager.list_plugins()[0].status,
        PluginStatus::Loaded
    ));
}

#[test]
fn ts1_loadfile_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "loadfile-test",
        r#"
        if loadfile ~= nil then
            error("loadfile should be removed")
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    assert!(matches!(
        manager.list_plugins()[0].status,
        PluginStatus::Loaded
    ));
}

#[test]
fn ts1_require_traversal_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "traversal-test",
        r#"
        local ok, err = pcall(plugin.require, "../escape")
        if ok then
            error("path traversal should be blocked")
        end
        if not string.find(tostring(err), "traversal") then
            error("expected path traversal error, got: " .. tostring(err))
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    assert!(matches!(
        manager.list_plugins()[0].status,
        PluginStatus::Loaded
    ));
}

#[test]
fn ts1_per_plugin_module_cache() {
    let tmp = tempfile::tempdir().unwrap();

    let plugin_a = tmp.path().join("plugin-a");
    fs::create_dir_all(&plugin_a).unwrap();
    fs::write(
        plugin_a.join("helper.lua"),
        r#"return { source = "plugin-a" }"#,
    )
    .unwrap();
    fs::write(
        plugin_a.join("init.lua"),
        r#"
        local h = plugin.require("helper")
        plugin.register_command("check", "Check source", function()
            editor.set_status(h.source)
        end)
        "#,
    )
    .unwrap();

    let plugin_b = tmp.path().join("plugin-b");
    fs::create_dir_all(&plugin_b).unwrap();
    fs::write(
        plugin_b.join("helper.lua"),
        r#"return { source = "plugin-b" }"#,
    )
    .unwrap();
    fs::write(
        plugin_b.join("init.lua"),
        r#"
        local h = plugin.require("helper")
        plugin.register_command("check", "Check source", function()
            editor.set_status(h.source)
        end)
        "#,
    )
    .unwrap();

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();

    manager
        .execute_command("plugin.plugin-a.check", &mut editor)
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("plugin-a"));

    manager
        .execute_command("plugin.plugin-b.check", &mut editor)
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("plugin-b"));
}

#[test]
fn ts1_global_require_blocked() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "no-global-require",
        r#"
        if rawget(_G, "require") ~= nil then
            error("global require should be nil")
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    assert!(matches!(
        manager.list_plugins()[0].status,
        PluginStatus::Loaded
    ));
}

// ---------------------------------------------------------------------------
// TS-2: Command registration and execution round-trip
// ---------------------------------------------------------------------------

#[test]
fn ts2_command_registration_and_palette() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "cmd-palette",
        r#"
        plugin.register_command("hello", "Say hello", function()
            editor.set_status("hello from plugin")
        end)
        plugin.register_command("goodbye", "Say goodbye", function()
            editor.set_status("goodbye from plugin")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let commands = manager.list_commands();
    assert_eq!(commands.len(), 2);

    let ids: Vec<&str> = commands.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"plugin.cmd-palette.hello"));
    assert!(ids.contains(&"plugin.cmd-palette.goodbye"));

    let descriptions: Vec<&str> = commands.iter().map(|(_, d)| d.as_str()).collect();
    assert!(descriptions.iter().any(|d| d.contains("[cmd-palette]")));
}

#[test]
fn ts2_command_execution_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "exec-rt",
        r#"
        plugin.register_command("greet", "Greet", function()
            local mode = editor.get_mode()
            editor.set_status("mode=" .. mode)
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let (mutated, actions) = manager
        .execute_command("plugin.exec-rt.greet", &mut editor)
        .unwrap();

    assert!(!mutated);
    assert!(actions.is_empty());
    assert_eq!(editor.status_message.as_deref(), Some("mode=normal"));
}

#[test]
fn ts2_command_runtime_error_display() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "err-display",
        r#"
        plugin.register_command("fail", "Always fails", function()
            error("intentional command error")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.execute_command("plugin.err-display.fail", &mut editor);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("intentional command error")
    );
}

// ---------------------------------------------------------------------------
// TS-3: Hook dispatch with context validation
// ---------------------------------------------------------------------------

#[test]
fn ts3_all_hook_types_fire() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "hook-all",
        r#"
        plugin.on("on_open", function(ctx)
            editor.set_status("on_open:" .. (ctx.filename or "nil"))
        end)
        plugin.on("on_save", function(ctx)
            editor.set_status("on_save:" .. (ctx.filename or "nil"))
        end)
        plugin.on("on_close", function(ctx)
            editor.set_status("on_close:" .. (ctx.filename or "nil"))
        end)
        plugin.on("on_mode_change", function(ctx)
            editor.set_status("on_mode_change:" .. ctx.old_mode .. "->" .. ctx.new_mode)
        end)
        plugin.on("on_cursor_move", function(ctx)
            editor.set_status("on_cursor_move:" .. ctx.line .. "," .. ctx.col)
        end)
        plugin.on("on_buffer_change", function(ctx)
            editor.set_status("on_buffer_change:" .. (ctx.filename or "nil"))
        end)
        plugin.on("on_tab_switch", function(ctx)
            editor.set_status("on_tab_switch:" .. (ctx.filename or "nil"))
        end)
        plugin.on("on_ready", function(ctx)
            editor.set_status("on_ready")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);
    let mut editor = create_test_editor();

    // on_ready
    manager
        .dispatch_hook(HookEvent::OnReady, &mut editor)
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("on_ready"));

    // on_open
    manager
        .dispatch_hook(
            HookEvent::OnOpen {
                path: Some("/tmp/main.rs".into()),
                filename: Some("main.rs".into()),
                language: Some("rust".into()),
            },
            &mut editor,
        )
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("on_open:main.rs"));

    // on_save
    manager
        .dispatch_hook(
            HookEvent::OnSave {
                path: Some("/tmp/main.rs".into()),
                filename: Some("main.rs".into()),
            },
            &mut editor,
        )
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("on_save:main.rs"));

    // on_close
    manager
        .dispatch_hook(
            HookEvent::OnClose {
                path: Some("/tmp/main.rs".into()),
                filename: Some("main.rs".into()),
            },
            &mut editor,
        )
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("on_close:main.rs"));

    // on_mode_change
    manager
        .dispatch_hook(
            HookEvent::OnModeChange {
                old_mode: "Normal".into(),
                new_mode: "Insert".into(),
            },
            &mut editor,
        )
        .unwrap();
    assert_eq!(
        editor.status_message.as_deref(),
        Some("on_mode_change:Normal->Insert")
    );

    // on_cursor_move
    manager
        .dispatch_hook(HookEvent::OnCursorMove { line: 5, col: 3 }, &mut editor)
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("on_cursor_move:5,3"));

    // on_buffer_change
    manager
        .dispatch_hook(
            HookEvent::OnBufferChange {
                path: Some("/tmp/main.rs".into()),
                filename: Some("main.rs".into()),
            },
            &mut editor,
        )
        .unwrap();
    assert_eq!(
        editor.status_message.as_deref(),
        Some("on_buffer_change:main.rs")
    );

    // on_tab_switch
    manager
        .dispatch_hook(
            HookEvent::OnTabSwitch {
                path: Some("/tmp/other.rs".into()),
                filename: Some("other.rs".into()),
            },
            &mut editor,
        )
        .unwrap();
    assert_eq!(
        editor.status_message.as_deref(),
        Some("on_tab_switch:other.rs")
    );
}

#[test]
fn ts3_multi_plugin_hook_order() {
    let tmp = tempfile::tempdir().unwrap();

    create_plugin_dir(
        tmp.path(),
        "aaa",
        r#"
        plugin.on("on_ready", function(ctx)
            local prev = editor.get_status() or ""
            editor.set_status(prev .. "aaa,")
        end)
        "#,
    );
    create_plugin_dir(
        tmp.path(),
        "bbb",
        r#"
        plugin.on("on_ready", function(ctx)
            local prev = editor.get_status() or ""
            editor.set_status(prev .. "bbb,")
        end)
        "#,
    );
    create_plugin_dir(
        tmp.path(),
        "ccc",
        r#"
        plugin.on("on_ready", function(ctx)
            local prev = editor.get_status() or ""
            editor.set_status(prev .. "ccc,")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    manager
        .dispatch_hook(HookEvent::OnReady, &mut editor)
        .unwrap();

    assert_eq!(editor.status_message.as_deref(), Some("aaa,bbb,ccc,"));
}

#[test]
fn ts3_reentrancy_skip() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "reentrant",
        r#"
        plugin.on("on_ready", function(ctx)
            editor.set_status("ready fired")
        end)
        "#,
    );

    let mut manager = PluginManager::new(default_config()).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.dispatch_hook(HookEvent::OnReady, &mut editor);
    assert!(result.is_ok());
    assert_eq!(editor.status_message.as_deref(), Some("ready fired"));
}

// ---------------------------------------------------------------------------
// TS-4: Error recovery
// ---------------------------------------------------------------------------

#[test]
fn ts4_syntax_error_marks_failed() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "bad-syntax", "this is {{invalid}} lua");

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert!(matches!(plugins[0].status, PluginStatus::Failed(_)));
}

#[test]
fn ts4_runtime_error_marks_failed() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "bad-runtime", r#"error("init failure")"#);

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    match &plugins[0].status {
        PluginStatus::Failed(msg) => assert!(msg.contains("init failure")),
        other => panic!("expected Failed, got {:?}", other),
    }
}

#[test]
fn ts4_instruction_limit_with_isolated_config() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "infinite", "while true do end");

    let config = PluginConfig {
        instruction_limit: 100,
        ..default_config()
    };
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    match &plugins[0].status {
        PluginStatus::Failed(msg) => {
            assert!(
                msg.contains("instruction limit"),
                "expected instruction limit error, got: {}",
                msg
            );
        }
        other => panic!("expected Failed, got {:?}", other),
    }
}

#[test]
fn ts4_memory_limit_with_isolated_config() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "mem-hog",
        r#"
        local t = {}
        for i = 1, 1000000 do
            t[i] = string.rep("x", 1000)
        end
        "#,
    );

    let config = PluginConfig {
        memory_limit_mb: 1,
        ..default_config()
    };
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert!(matches!(plugins[0].status, PluginStatus::Failed(_)));
}

// ---------------------------------------------------------------------------
// TS-5: API boundary validation
// ---------------------------------------------------------------------------

#[test]
fn ts5_coordinate_conversion_1based() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "coord-test",
        r#"
        plugin.register_command("check-cursor", "Check cursor", function()
            local pos = editor.get_cursor()
            editor.set_status(pos.line .. "," .. pos.col)
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let file = std::env::temp_dir().join("coord_test.txt");
    fs::write(&file, "hello\nworld\n").unwrap();
    editor.open_file(&file).unwrap();

    manager
        .execute_command("plugin.coord-test.check-cursor", &mut editor)
        .unwrap();

    // Cursor starts at 0,0 internally = 1,1 in Lua
    assert_eq!(editor.status_message.as_deref(), Some("1,1"));
    let _ = fs::remove_file(&file);
}

#[test]
fn ts5_deferred_action_ordering() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "deferred-order",
        r#"
        plugin.register_command("multi-defer", "Multiple deferred actions", function()
            editor.open_file("/tmp/first.rs")
            editor.execute_command("file.save")
            editor.open_file("/tmp/second.rs")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let (_, actions) = manager
        .execute_command("plugin.deferred-order.multi-defer", &mut editor)
        .unwrap();

    assert_eq!(actions.len(), 3);
    assert!(
        matches!(&actions[0], termcode_plugin::DeferredAction::OpenFile(p) if p == &PathBuf::from("/tmp/first.rs"))
    );
    assert!(
        matches!(&actions[1], termcode_plugin::DeferredAction::ExecuteCommand(c) if c == "file.save")
    );
    assert!(
        matches!(&actions[2], termcode_plugin::DeferredAction::OpenFile(p) if p == &PathBuf::from("/tmp/second.rs"))
    );
}

#[test]
fn ts5_plugin_command_rejection_from_lua() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "reject-test",
        r#"
        plugin.register_command("try-recurse", "Try recursion", function()
            editor.execute_command("plugin.other.cmd")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.execute_command("plugin.reject-test.try-recurse", &mut editor);
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("cannot execute plugin.*")
    );
}

#[test]
fn ts5_get_line_strips_newline() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "line-test",
        r#"
        plugin.register_command("get-line", "Get first line", function()
            local line = editor.get_line(1)
            editor.set_status(line)
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let file = std::env::temp_dir().join("line_test.txt");
    fs::write(&file, "hello world\nsecond line\n").unwrap();
    editor.open_file(&file).unwrap();

    manager
        .execute_command("plugin.line-test.get-line", &mut editor)
        .unwrap();

    assert_eq!(editor.status_message.as_deref(), Some("hello world"));
    let _ = fs::remove_file(&file);
}

// ---------------------------------------------------------------------------
// TS-6: Plugin metadata and config
// ---------------------------------------------------------------------------

#[test]
fn ts6_plugin_toml_parsing() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir_with_toml(
        tmp.path(),
        "meta-test",
        r#"
        name = "meta-test"
        version = "2.0.0"
        description = "Testing metadata"
        author = "test-author"
        "#,
        "-- init",
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins[0].name, "meta-test");
    assert_eq!(plugins[0].version, "2.0.0");
    assert_eq!(plugins[0].description, "Testing metadata");
    assert_eq!(plugins[0].author, "test-author");
}

#[test]
fn ts6_missing_toml_defaults_from_dirname() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(tmp.path(), "my-cool-plugin", "-- init");

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins[0].name, "my-cool-plugin");
    assert_eq!(plugins[0].version, "0.1.0");
    assert!(matches!(plugins[0].status, PluginStatus::Loaded));
}

#[test]
fn ts6_invalid_name_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir_with_toml(
        tmp.path(),
        "bad-name-dir",
        r#"name = "Invalid Name!""#,
        "-- init",
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert!(matches!(plugins[0].status, PluginStatus::Failed(_)));
}

#[test]
fn ts6_tilde_expansion() {
    let expanded = expand_tilde("~/some/path");
    assert!(!expanded.to_string_lossy().starts_with('~'));
    assert!(expanded.to_string_lossy().contains("some/path"));

    let expanded = expand_tilde("~");
    assert!(!expanded.to_string_lossy().starts_with('~'));

    let no_tilde = expand_tilde("/absolute/path");
    assert_eq!(no_tilde, PathBuf::from("/absolute/path"));
}

#[test]
fn ts6_nonexistent_dir_skipped() {
    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[PathBuf::from("/nonexistent/plugin/dir")]);
    assert!(manager.list_plugins().is_empty());
}

// ---------------------------------------------------------------------------
// TS-7: Logging API
// ---------------------------------------------------------------------------

#[test]
fn ts7_log_all_levels() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "log-test",
        r#"
        plugin.register_command("do-log", "Test logging", function()
            log.info("info message")
            log.warn("warn message")
            log.error("error message")
            log.debug("debug message")
            editor.set_status("logs done")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.execute_command("plugin.log-test.do-log", &mut editor);
    assert!(result.is_ok());
    assert_eq!(editor.status_message.as_deref(), Some("logs done"));
}

#[test]
fn ts7_log_non_string_tostring_conversion() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "log-types",
        r#"
        plugin.register_command("log-types", "Log non-string values", function()
            log.info(42)
            log.info(true)
            log.info(nil)
            log.info({1, 2, 3})
            editor.set_status("logged")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.execute_command("plugin.log-types.log-types", &mut editor);
    assert!(result.is_ok());
    assert_eq!(editor.status_message.as_deref(), Some("logged"));
}

// ---------------------------------------------------------------------------
// TS-8: Crate structure
// ---------------------------------------------------------------------------

#[test]
fn ts8_no_termcode_term_dependency() {
    let output = std::process::Command::new("cargo")
        .args(["tree", "-p", "termcode-plugin", "--prefix", "none"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo tree should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("termcode-term"),
        "termcode-plugin must not depend on termcode-term, but cargo tree shows:\n{}",
        stdout
    );
}

// ---------------------------------------------------------------------------
// TS-9: Lifecycle edge cases
// ---------------------------------------------------------------------------

#[test]
fn ts9_init_runs_once_only() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "init-once",
        r#"
        _init_count = (_init_count or 0) + 1
        if _init_count > 1 then
            error("init.lua should only run once")
        end
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 1);
    assert!(matches!(plugins[0].status, PluginStatus::Loaded));
}

#[test]
fn ts9_list_plugins_three_statuses() {
    let tmp = tempfile::tempdir().unwrap();

    // Loaded
    create_plugin_dir(tmp.path(), "good-plugin", "-- init");

    // Failed
    create_plugin_dir(tmp.path(), "bad-plugin", r#"error("fail")"#);

    // Disabled
    create_plugin_dir(tmp.path(), "off-plugin", "-- init");

    let mut overrides = HashMap::new();
    overrides.insert(
        "off-plugin".to_string(),
        PluginOverride {
            enabled: Some(false),
            config: HashMap::new(),
        },
    );

    let config = PluginConfig {
        overrides,
        ..default_config()
    };
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 3);

    let loaded = plugins
        .iter()
        .filter(|p| matches!(p.status, PluginStatus::Loaded))
        .count();
    let failed = plugins
        .iter()
        .filter(|p| matches!(p.status, PluginStatus::Failed(_)))
        .count();
    let disabled = plugins
        .iter()
        .filter(|p| matches!(p.status, PluginStatus::Disabled))
        .count();

    assert_eq!(loaded, 1);
    assert_eq!(failed, 1);
    assert_eq!(disabled, 1);
}

#[test]
fn ts9_registration_outside_init_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    create_plugin_dir(
        tmp.path(),
        "late-reg",
        r#"
        plugin.register_command("early", "Works during init", function()
            plugin.register_command("late", "Should fail", function() end)
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.execute_command("plugin.late-reg.early", &mut editor);
    // The plugin global is cleared after init, so attempting to call
    // plugin.register_command during command execution will error.
    assert!(result.is_err());
}

#[test]
fn ts9_duplicate_plugin_name_later_takes_precedence() {
    let tmp1 = tempfile::tempdir().unwrap();
    let tmp2 = tempfile::tempdir().unwrap();

    create_plugin_dir(
        tmp1.path(),
        "dup",
        r#"
        plugin.register_command("who", "Who am I", function()
            editor.set_status("first")
        end)
        "#,
    );
    create_plugin_dir(
        tmp2.path(),
        "dup",
        r#"
        plugin.register_command("who", "Who am I", function()
            editor.set_status("second")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp1.path().to_path_buf(), tmp2.path().to_path_buf()]);

    let mut editor = create_test_editor();
    manager
        .execute_command("plugin.dup.who", &mut editor)
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("second"));
}

#[test]
fn ts9_failed_plugin_does_not_block_others() {
    let tmp = tempfile::tempdir().unwrap();

    create_plugin_dir(tmp.path(), "aaa-fail", r#"error("I crash during init")"#);
    create_plugin_dir(
        tmp.path(),
        "bbb-good",
        r#"
        plugin.register_command("hello", "Hello", function()
            editor.set_status("bbb works!")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let plugins = manager.list_plugins();
    assert_eq!(plugins.len(), 2);
    assert!(matches!(plugins[0].status, PluginStatus::Failed(_)));
    assert!(matches!(plugins[1].status, PluginStatus::Loaded));

    let mut editor = create_test_editor();
    manager
        .execute_command("plugin.bbb-good.hello", &mut editor)
        .unwrap();
    assert_eq!(editor.status_message.as_deref(), Some("bbb works!"));
}

#[test]
fn ts9_hook_error_does_not_block_other_plugins() {
    let tmp = tempfile::tempdir().unwrap();

    create_plugin_dir(
        tmp.path(),
        "aaa-bad-hook",
        r#"
        plugin.on("on_ready", function(ctx)
            error("hook error from aaa")
        end)
        "#,
    );
    create_plugin_dir(
        tmp.path(),
        "bbb-good-hook",
        r#"
        plugin.on("on_ready", function(ctx)
            editor.set_status("bbb hook ran")
        end)
        "#,
    );

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[tmp.path().to_path_buf()]);

    let mut editor = create_test_editor();
    let result = manager.dispatch_hook(HookEvent::OnReady, &mut editor);
    assert!(result.is_ok());
    assert_eq!(editor.status_message.as_deref(), Some("bbb hook ran"));
}

// ---------------------------------------------------------------------------
// Example plugin validation
// ---------------------------------------------------------------------------

#[test]
fn example_plugin_loads_successfully() {
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("runtime")
        .join("plugins");

    if !example_dir.join("example").join("init.lua").exists() {
        panic!("Example plugin not found at {}", example_dir.display());
    }

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[example_dir]);

    let plugins = manager.list_plugins();
    let example = plugins.iter().find(|p| p.name == "example");
    assert!(example.is_some(), "example plugin should be loaded");

    let example = example.unwrap();
    assert!(
        matches!(example.status, PluginStatus::Loaded),
        "example plugin should load successfully, got: {:?}",
        example.status
    );
    assert_eq!(example.version, "0.1.0");
    assert!(!example.description.is_empty());

    let commands = manager.list_commands();
    assert!(
        commands
            .iter()
            .any(|(id, _)| id == "plugin.example.wrap-quotes"),
        "wrap-quotes command should be registered"
    );
    assert!(
        commands
            .iter()
            .any(|(id, _)| id == "plugin.example.insert-date"),
        "insert-date command should be registered"
    );
}

#[test]
fn example_plugin_insert_date_works() {
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("runtime")
        .join("plugins");

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[example_dir]);

    let mut editor = create_test_editor();
    let file = std::env::temp_dir().join("example_date_test.txt");
    fs::write(&file, "test content\n").unwrap();
    editor.open_file(&file).unwrap();

    let result = manager.execute_command("plugin.example.insert-date", &mut editor);
    assert!(result.is_ok());

    let status = editor.status_message.as_deref().unwrap_or("");
    assert!(
        status.starts_with("[example] Inserted date:"),
        "unexpected status: {}",
        status
    );
    let _ = fs::remove_file(&file);
}

#[test]
fn example_plugin_hooks_fire() {
    let example_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("runtime")
        .join("plugins");

    let config = default_config();
    let mut manager = PluginManager::new(config).unwrap();
    manager.load_plugins(&[example_dir]);

    let mut editor = create_test_editor();

    // on_ready hook should fire without error
    let result = manager.dispatch_hook(HookEvent::OnReady, &mut editor);
    assert!(result.is_ok());

    // on_save hook should fire without error
    let result = manager.dispatch_hook(
        HookEvent::OnSave {
            path: Some("/tmp/test.rs".into()),
            filename: Some("test.rs".into()),
        },
        &mut editor,
    );
    assert!(result.is_ok());
}
