use std::collections::HashMap;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use crossterm::cursor::Show;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use termcode_config::config::AppConfig;
use termcode_lsp::types::LspResponse;
use termcode_syntax::language::LanguageRegistry;
use termcode_theme::loader::parse_theme;
use termcode_theme::theme::Theme;
use termcode_view::editor::{Editor, EditorMode};
use termcode_view::file_explorer::{FileNodeKind, NewEntryKind};

use termcode_view::palette::{PaletteItem, PaletteMode};

use crate::command::{
    CommandRegistry, insert_char, register_builtin_commands, rerun_search,
    sync_cursor_from_selection,
};
use crate::event::{AppEvent, EventHandler};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use termcode_plugin::{DeferredAction, HookEvent, PluginManager};
use termcode_view::image::ImageId;

use crate::input::{InputMapper, KeyResolution};
use crate::layout;
use crate::lsp_bridge::LspBridge;
use crate::mouse::{self, MouseAction};
use crate::render;

mod settings;

/// Palette id for "no preset": the built-in keymap. Not a filename, so it
/// cannot clash with a preset discovered on disk.
const BUILTIN_KEYMAP: &str = "(built-in)";

pub struct App {
    editor: Editor,
    event_handler: EventHandler,
    command_registry: CommandRegistry,
    input_mapper: InputMapper,
    should_quit: bool,
    lsp_bridge: Option<LspBridge>,
    lsp_event_rx: mpsc::UnboundedReceiver<AppEvent>,
    /// Trigger characters per language, cached from server capabilities.
    lsp_trigger_chars: HashMap<String, Vec<String>>,
    /// Last known terminal size, updated each frame for accurate mouse layout.
    terminal_size: (u16, u16),
    /// Whether mouse capture was enabled at startup (for clean teardown).
    mouse_enabled: bool,
    image_picker: Option<Picker>,
    pub image_cache: HashMap<ImageId, Mutex<StatefulProtocol>>,
    plugin_manager: Option<PluginManager>,
    /// User keybinding overrides, kept so a live keymap switch can re-apply them.
    kb_config: termcode_config::keymap::KeybindingConfig,
    /// The config file settings changes are written back to: whichever one was
    /// actually loaded, so a project-local config is not silently bypassed.
    config_path: PathBuf,
    /// Where keybinding overrides are written. Always the user config
    /// directory: keymap presets ship read-only.
    keybindings_path: PathBuf,
    /// Everything loaded at startup, kept so the settings screen can show the
    /// values that live nowhere else (plugins, LSP servers, chord timeout).
    app_config: AppConfig,
    /// Name of the theme in use, for the settings screen and for knowing what
    /// to write to the config file.
    theme_name: String,
    /// Name of the keymap in use, or [`BUILTIN_KEYMAP`].
    keymap_name: String,
    /// Keys pressed so far while the settings screen is capturing a rebinding.
    settings_capture: Vec<KeyEvent>,
    /// Directory the editor was opened on. The session is keyed on it rather
    /// than on the explorer's root, which the `..` row can walk elsewhere.
    session_root: PathBuf,
    /// How long a partially typed chord stays pending before it is abandoned.
    chord_timeout: Duration,
    /// When the current pending chord started, for the timeout above.
    chord_started: Option<Instant>,
}

impl App {
    pub fn new(root: Option<PathBuf>) -> Self {
        let mut config_path = termcode_config::default::config_dir().join("config.toml");
        let mut app_config = AppConfig::load(&config_path);

        // Also try project-local config/config.toml
        let project_config = PathBuf::from("config/config.toml");
        if project_config.exists() {
            app_config = AppConfig::load(&project_config);
            config_path = project_config;
        }

        let mut app = Self::with_config(root, app_config);
        // Settings are saved back to the file they came from.
        app.config_path = config_path;
        app
    }

    pub fn with_config(root: Option<PathBuf>, app_config: AppConfig) -> Self {
        // The settings screen reports and rewrites what was loaded, so keep a
        // copy before the pieces below are moved out of it.
        let startup_config = app_config.clone();
        let (theme, startup_theme_status) = match load_theme_by_name(&app_config.theme) {
            Ok(theme) => (theme, None),
            Err(e) => {
                log::warn!(
                    "Failed to load theme '{}': {}. Falling back to built-in one-dark",
                    app_config.theme,
                    e
                );
                (
                    load_default_theme(),
                    Some(format!(
                        "Failed to load theme '{}': {e} (using one-dark)",
                        app_config.theme
                    )),
                )
            }
        };
        let config = app_config.editor.clone();
        let mut lang_registry = LanguageRegistry::with_builtins();
        for dir in termcode_config::default::runtime_dirs() {
            lang_registry.load_queries(&dir);
        }
        let mut editor = Editor::new(theme, config, lang_registry, root);
        // Captured before anything can move the explorer's root, so the session
        // is written back to the directory the editor was opened on.
        let session_root = editor.file_explorer.root.clone();
        editor.file_tree_style = app_config.ui.file_tree_style;
        editor.file_explorer.respect_gitignore = app_config.ui.file_tree_style.respect_gitignore;
        if !app_config.ui.file_tree_style.respect_gitignore {
            // Reload file tree without gitignore filtering
            let _ = editor.file_explorer.refresh();
        }
        editor.file_explorer.width = app_config.ui.sidebar_width;
        editor.file_explorer.visible = app_config.ui.sidebar_visible;
        // Startup problems accumulate: a user with both a bad theme and a bad
        // keymap preset should hear about both, not just the last one.
        let mut startup_warnings: Vec<String> = Vec::new();
        if let Some(msg) = startup_theme_status {
            startup_warnings.push(msg);
        }
        editor.clipboard = Some(Box::new(crate::clipboard::ArboardClipboard::new()));

        let mut command_registry = CommandRegistry::new();
        register_builtin_commands(&mut command_registry);

        let keybindings_path = termcode_config::default::config_dir().join("keybindings.toml");
        let kb_config = termcode_config::keymap::load_keybindings(&keybindings_path);

        // Read the preset once: the mapper is rebuilt after plugins register
        // their commands, and re-reading would repeat any parse warning.
        let preset = match app_config
            .keymap
            .preset
            .as_deref()
            .filter(|n| !n.is_empty())
        {
            Some(name) => match termcode_config::keymap::load_keymap_preset(name) {
                Some(preset) => {
                    // The loader logs these too, but a half-bound keymap is
                    // confusing enough that the user should not have to go
                    // looking in a log file for the reason.
                    startup_warnings.extend(
                        preset
                            .warnings()
                            .into_iter()
                            .map(|w| format!("Keymap '{name}': {w}")),
                    );
                    Some(preset)
                }
                None => {
                    log::warn!("Keymap preset '{name}' not found; using the built-in keymap");
                    startup_warnings
                        .push(format!("Keymap preset '{name}' not found (using default)"));
                    None
                }
            },
            None => None,
        };
        // A keymap with no modal layer has to say so, or the editor would open
        // in Normal mode where that keymap binds almost nothing.
        if preset.as_ref().is_some_and(|p| p.meta.starts_in_insert()) {
            editor.default_mode = EditorMode::Insert;
        }
        editor.switch_to_default_mode();
        let mut input_mapper = build_input_mapper(preset.as_ref(), &command_registry, &kb_config);
        if !startup_warnings.is_empty() {
            editor.status_message = Some(startup_warnings.join("  |  "));
        }

        let (lsp_event_tx, lsp_event_rx) = mpsc::unbounded_channel();

        let lsp_bridge = if app_config.lsp.is_empty() {
            None
        } else {
            Some(LspBridge::new(app_config.lsp, lsp_event_tx))
        };

        let image_picker = Picker::from_query_stdio().ok();

        let plugin_manager = if app_config.plugins.enabled {
            match PluginManager::new(app_config.plugins.clone()) {
                Ok(mut pm) => {
                    let mut plugin_dirs: Vec<PathBuf> = termcode_config::default::runtime_dirs()
                        .iter()
                        .map(|d| d.join("plugins"))
                        .filter(|d| d.exists())
                        .collect();
                    for dir_str in &app_config.plugins.plugin_dirs {
                        plugin_dirs.push(termcode_plugin::expand_tilde(dir_str));
                    }
                    pm.load_plugins(&plugin_dirs);

                    for (cmd_id, cmd_desc) in pm.list_commands() {
                        let leaked_id: &'static str = Box::leak(cmd_id.into_boxed_str());
                        let leaked_name: &'static str = Box::leak(cmd_desc.into_boxed_str());
                        command_registry.register(crate::command::CommandEntry {
                            id: leaked_id,
                            name: leaked_name,
                            handler: crate::command::cmd_noop,
                        });
                    }

                    // Rebuild now that plugin commands exist, so a keymap can
                    // bind them.
                    input_mapper =
                        build_input_mapper(preset.as_ref(), &command_registry, &kb_config);

                    Some(pm)
                }
                Err(e) => {
                    log::error!("Failed to initialize plugin system: {}", e);
                    editor.status_message = Some(format!("Plugin error: {}", e));
                    None
                }
            }
        } else {
            None
        };

        let mouse_enabled = editor.config.mouse_enabled;
        Self {
            editor,
            event_handler: EventHandler::new(50),
            command_registry,
            input_mapper,
            should_quit: false,
            lsp_bridge,
            lsp_event_rx,
            lsp_trigger_chars: HashMap::new(),
            terminal_size: (80, 24),
            mouse_enabled,
            image_picker,
            image_cache: HashMap::new(),
            plugin_manager,
            kb_config,
            config_path: termcode_config::default::config_dir().join("config.toml"),
            keybindings_path,
            theme_name: startup_config.theme.clone(),
            // An empty preset is how the settings screen records "no preset":
            // the key has to be present, since a missing one is the default.
            keymap_name: startup_config
                .keymap
                .preset
                .clone()
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| BUILTIN_KEYMAP.to_string()),
            app_config: startup_config,
            settings_capture: Vec::new(),
            session_root,
            chord_timeout: Duration::from_millis(app_config.keymap.chord_timeout_ms),
            chord_started: None,
        }
    }

    pub fn show_sidebar(&mut self) {
        self.editor.file_explorer.visible = true;
        self.editor.switch_mode(EditorMode::FileExplorer);
    }

    pub fn focus_sidebar_if_visible(&mut self) {
        if self.editor.file_explorer.visible {
            self.editor.switch_mode(EditorMode::FileExplorer);
        }
    }

    pub fn open_file(&mut self, path: &Path) -> anyhow::Result<()> {
        if is_image_extension(path) {
            self.open_image_file(path)?;
            return Ok(());
        }
        let (doc_id, _view_id) = self.editor.open_file(path)?;
        self.lsp_notify_did_open(doc_id);

        let doc = self.editor.documents.get(&doc_id);
        let hook_path = doc
            .and_then(|d| d.path.as_ref())
            .map(|p| p.display().to_string());
        let hook_filename = doc
            .and_then(|d| d.path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());
        let hook_language = doc
            .and_then(|d| d.language_id.as_ref())
            .map(|l| l.as_ref().to_string());
        self.dispatch_plugin_hook(HookEvent::OnOpen {
            path: hook_path,
            filename: hook_filename,
            language: hook_language,
        });

        Ok(())
    }

    fn open_image_file(&mut self, path: &Path) -> anyhow::Result<()> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_ascii_lowercase();
        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();
        let (dimensions, decoded) = if let Some(picker) = &mut self.image_picker {
            match image::open(path) {
                Ok(dyn_image) => {
                    let dims = Some((dyn_image.width(), dyn_image.height()));
                    let protocol = picker.new_resize_protocol(dyn_image);
                    (dims, Some(protocol))
                }
                Err(e) => {
                    self.editor.status_message = Some(format!("Failed to decode image: {e}"));
                    (None, None)
                }
            }
        } else {
            (None, None)
        };
        let image_id = self.editor.open_image(path, ext, file_size, dimensions);
        if let Some(protocol) = decoded {
            self.image_cache.insert(image_id, Mutex::new(protocol));
        }
        Ok(())
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut terminal = setup_terminal(self.editor.config.mouse_enabled)?;

        // Settle the resting mode now that startup files are open: a non-modal
        // keymap wants Insert, which needs a view to exist first. Callers open
        // files between `new()` and `run()`, so this cannot happen any earlier.
        if self.editor.mode == EditorMode::Normal {
            self.editor.switch_to_default_mode();
        }

        let app_result = (|| -> anyhow::Result<()> {
            terminal.draw(|frame| {
                render::render(frame, &self.editor, &self.image_cache, &self.input_mapper)
            })?;

            self.dispatch_plugin_hook(HookEvent::OnReady);

            loop {
                while let Ok(lsp_event) = self.lsp_event_rx.try_recv() {
                    self.update(lsp_event);
                }

                let prev_mode = self.editor.mode;
                let prev_active_tab = self.editor.tabs.active;
                let prev_cursor = self
                    .editor
                    .active_view()
                    .map(|v| (v.cursor.line, v.cursor.column));

                let event = self.event_handler.next()?;
                self.update(event);

                if self.should_quit {
                    break;
                }

                self.dispatch_state_diff_hooks(prev_mode, prev_active_tab, prev_cursor);

                {
                    let size = terminal.size()?;
                    self.terminal_size = (size.width, size.height);
                    let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
                    let app_layout = layout::compute_layout(
                        area,
                        self.editor.file_explorer.visible,
                        self.editor.file_explorer.width,
                        self.editor.theme.ui.pane_focus_style,
                        self.editor.theme.ui.panel_borders,
                    );
                    if let Some(view) = self.editor.active_view_mut() {
                        view.area_height = app_layout.editor_area.height;
                        view.area_width = app_layout.editor_area.width;
                    }
                    if let Some(sidebar) = app_layout.sidebar {
                        self.editor.file_explorer.viewport_height = sidebar.height as usize;
                    }

                    // Must match max_height values in FuzzyFinderWidget / CommandPaletteWidget
                    const FUZZY_MAX_HEIGHT: u16 = 20;
                    const PALETTE_MAX_HEIGHT: u16 = 15;
                    const OVERLAY_CHROME: usize = 3; // top border + input + bottom border

                    let fuzzy_height = FUZZY_MAX_HEIGHT.min(app_layout.editor_area.height) as usize;
                    self.editor.fuzzy_finder.visible_height =
                        fuzzy_height.saturating_sub(OVERLAY_CHROME);
                    let palette_height =
                        PALETTE_MAX_HEIGHT.min(app_layout.editor_area.height) as usize;
                    self.editor.command_palette.visible_height =
                        palette_height.saturating_sub(OVERLAY_CHROME);

                    // Must match the row budget in SettingsWidget: the settings
                    // screen fills the editor area, minus its border and the
                    // hint line at the bottom.
                    self.editor.settings.visible_height = (app_layout.editor_area.height as usize)
                        .saturating_sub(crate::ui::settings::CHROME_ROWS);
                    if let Some(picker) = &mut self.editor.settings.picker {
                        let rows = crate::ui::settings::picker_visible_rows(
                            app_layout.editor_area.height,
                            picker.options.len(),
                        );
                        picker.set_visible_height(rows);
                    }
                }

                self.editor.sync_tab_modified();
                terminal.draw(|frame| {
                    render::render(frame, &self.editor, &self.image_cache, &self.input_mapper)
                })?;
            }

            Ok(())
        })();

        if let Some(ref bridge) = self.lsp_bridge {
            bridge.shutdown();
        }
        self.save_session();

        let restore_result = restore_terminal(&mut terminal, self.mouse_enabled);

        match (app_result, restore_result) {
            (Err(app_err), Err(restore_err)) => Err(anyhow::anyhow!(
                "{app_err}; additionally failed to restore terminal: {restore_err}"
            )),
            (Err(app_err), Ok(())) => Err(app_err),
            (Ok(()), Err(restore_err)) => Err(restore_err),
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    pub fn restore_session(&mut self) {
        if let Some(session) = crate::session::load_session(&self.session_root) {
            for file in &session.files {
                if let Err(e) = self.open_file(&file.path) {
                    log::warn!("Session restore failed for {}: {e}", file.path.display());
                    continue;
                }
                if let Some(view) = self.editor.active_view_mut() {
                    view.cursor.line = file.cursor_line;
                    view.cursor.column = file.cursor_column;
                }
            }
            if session.active_tab < self.editor.tabs.tabs.len() {
                self.editor.tabs.set_active(session.active_tab);
                self.editor.sync_active_view_to_tab();
            }
        }
    }

    fn save_session(&self) {
        let root = self.session_root.clone();
        let files: Vec<crate::session::SessionFile> = self
            .editor
            .tabs
            .tabs
            .iter()
            .filter_map(|tab| {
                let doc_id = match &tab.content {
                    termcode_view::image::TabContent::Document(id) => *id,
                    termcode_view::image::TabContent::Image(_) => return None,
                };
                let doc = self.editor.documents.get(&doc_id)?;
                let path = doc.path.clone()?;
                let view = self.editor.find_view_by_doc_id(doc_id)?;
                let view = self.editor.views.get(&view)?;
                Some(crate::session::SessionFile {
                    path,
                    cursor_line: view.cursor.line,
                    cursor_column: view.cursor.column,
                })
            })
            .collect();

        if files.is_empty() {
            if let Err(e) = crate::session::clear_session(&root) {
                log::warn!("Failed to clear session: {e}");
            }
            return;
        }

        let session = crate::session::Session {
            root,
            files,
            active_tab: self.editor.tabs.active,
        };
        if let Err(e) = crate::session::save_session(&session) {
            log::warn!("Failed to save session: {e}");
        }
    }

    fn update(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Mouse(mouse_event) => self.handle_mouse(mouse_event),
            AppEvent::Resize(_, _) => {
                // Re-render happens automatically
            }
            AppEvent::Tick => self.expire_pending_chord(),
            AppEvent::Lsp(response) => self.handle_lsp_response(response),
        }
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) {
        // The confirm dialog is not skipped here: `mouse::handle_mouse` keeps
        // it modal and answers clicks on its buttons.
        //
        // A click can move the cursor or change the mode, so a chord started
        // before it would complete against the wrong target.
        self.abandon_pending_chord();
        // A click can also take the user out of the settings screen, and a
        // half-typed rebinding must not still be waiting when they come back.
        if self.editor.mode == EditorMode::Settings {
            self.editor.settings.cancel_capture();
            self.settings_capture.clear();
        }
        let (w, h) = self.terminal_size;
        let area = ratatui::layout::Rect::new(0, 0, w, h);
        let app_layout = layout::compute_layout(
            area,
            self.editor.file_explorer.visible,
            self.editor.file_explorer.width,
            self.editor.theme.ui.pane_focus_style,
            self.editor.theme.ui.panel_borders,
        );

        match mouse::handle_mouse(&mut self.editor, event, &app_layout) {
            MouseAction::None => {}
            MouseAction::OpenExplorerItem(_index) => {
                self.handle_explorer_enter();
            }
            MouseAction::ToggleExplorerExpand(index) => {
                if let Err(e) = self.editor.file_explorer.toggle_expand(index) {
                    self.editor.status_message = Some(format!("Error: {e}"));
                }
                self.editor
                    .file_explorer
                    .compute_scroll_left(&self.editor.file_tree_style);
            }
            MouseAction::SwitchTab(index) => {
                self.editor.tabs.set_active(index);
                self.editor.sync_active_view_to_tab();
            }
            MouseAction::OpenSettings => self.open_settings(),
            MouseAction::Quit => self.handle_quit(),
            MouseAction::ConfirmSelected => self.execute_confirm_action(),
            MouseAction::ExplorerCommand(command) => self.dispatch_explorer_command(command),
            MouseAction::SidebarResized(width) => self.persist_sidebar_width(width),
        }
    }

    fn handle_lsp_response(&mut self, response: LspResponse) {
        match response {
            LspResponse::Diagnostics { uri, diagnostics } => {
                let path = parse_file_uri(&uri);
                if let Some(path) = path {
                    for doc in self.editor.documents.values_mut() {
                        if doc.path.as_ref() == Some(&path) {
                            doc.diagnostics = diagnostics;
                            break;
                        }
                    }
                }
            }
            LspResponse::Completion { items } => {
                self.editor.completion.items = items
                    .into_iter()
                    .map(|i| termcode_view::editor::CompletionItem {
                        label: i.label,
                        detail: i.detail,
                        insert_text: i.insert_text,
                    })
                    .collect();
                self.editor.completion.selected = 0;
                self.editor.completion.visible = !self.editor.completion.items.is_empty();
            }
            LspResponse::Hover { contents } => {
                if !contents.is_empty() {
                    if let Some(view) = self.editor.active_view() {
                        self.editor.hover.position = termcode_core::position::Position::new(
                            view.cursor.line,
                            view.cursor.column,
                        );
                    }
                    self.editor.hover.content = contents;
                    self.editor.hover.visible = true;
                }
            }
            LspResponse::Definition { uri, position } => {
                let path = parse_file_uri(&uri);
                if let Some(path) = path {
                    let is_current = self
                        .editor
                        .active_document()
                        .and_then(|d| d.path.as_ref())
                        .is_some_and(|p| *p == path);

                    if !is_current {
                        if let Err(e) = self.open_file(&path) {
                            self.editor.status_message = Some(format!("Error: {e}"));
                            return;
                        }
                    }
                    if let Some(view) = self.editor.active_view_mut() {
                        view.cursor = position;
                    }
                }
            }
            LspResponse::ServerStarted {
                language,
                trigger_characters,
            } => {
                if !trigger_characters.is_empty() {
                    self.lsp_trigger_chars
                        .insert(language.clone(), trigger_characters);
                }
                self.editor.status_message = Some(format!("LSP: {language} server started"));
            }
            LspResponse::ServerError { language, error } => {
                self.editor.status_message = Some(format!("LSP error ({language}): {error}"));
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Confirm dialog intercept: consume ALL keys while dialog is active
        if self.editor.confirm_dialog.is_some() {
            // A chord left pending here would be flushed into the buffer by the
            // timeout while the dialog is up, or resume against the next key
            // after the dialog closes.
            self.abandon_pending_chord();
            self.handle_confirm_key(key);
            return;
        }

        // Always-available escape hatch. A keymap can also bind `app.quit`, but
        // must never be able to leave the editor with no way out.
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('q') {
            self.abandon_pending_chord();
            self.handle_quit();
            return;
        }

        self.editor.hover.visible = false;

        if self.editor.help_visible {
            // Any key closes help popup
            self.editor.help_visible = false;
            return;
        }

        // A new entry being named owns every key while its row is open --
        // otherwise a name could not contain a letter the explorer binds.
        if self.editor.mode == EditorMode::FileExplorer
            && self.editor.file_explorer.new_entry.is_some()
        {
            self.abandon_pending_chord();
            self.handle_new_entry_key(key);
            return;
        }

        if self.editor.mode != EditorMode::Insert && self.editor.completion.visible {
            self.editor.completion.visible = false;
        }

        match self.editor.mode {
            EditorMode::Search => {
                self.handle_search_key(key);
                return;
            }
            EditorMode::FuzzyFinder => {
                self.handle_fuzzy_finder_key(key);
                return;
            }
            EditorMode::CommandPalette => {
                self.handle_command_palette_key(key);
                return;
            }
            EditorMode::Settings => {
                self.handle_settings_key(key);
                return;
            }
            _ => {}
        }

        if self.handle_completion_popup_key(key) {
            return;
        }

        if self.run_bound_command(key) {
            return;
        }

        if self.editor.mode == EditorMode::Insert {
            self.handle_insert_key(key);
        }
    }

    /// Let the completion popup have the keys it owns while it is open.
    /// Returns whether the key was consumed.
    ///
    /// The popup binds Up/Down/Enter/Tab/Esc, which the Insert-mode keymap also
    /// binds to cursor motion and newline; while it is up, the popup wins.
    fn handle_completion_popup_key(&mut self, key: KeyEvent) -> bool {
        if self.editor.mode != EditorMode::Insert || !self.editor.completion.visible {
            return false;
        }
        // The popup can appear asynchronously mid-chord. A key it consumes never
        // reaches the mapper, so the chord cannot continue across it: end it
        // here and keep what was typed.
        if matches!(
            key.code,
            KeyCode::Down | KeyCode::Up | KeyCode::Enter | KeyCode::Tab | KeyCode::Esc
        ) {
            self.recover_pending_chord();
        }
        let item_count = self.editor.completion.items.len();
        match key.code {
            KeyCode::Down => {
                if item_count > 0 {
                    self.editor.completion.selected =
                        (self.editor.completion.selected + 1) % item_count;
                }
                true
            }
            KeyCode::Up => {
                if item_count > 0 {
                    self.editor.completion.selected =
                        (self.editor.completion.selected + item_count - 1) % item_count;
                }
                true
            }
            KeyCode::Enter | KeyCode::Tab => {
                self.accept_completion();
                true
            }
            KeyCode::Esc => {
                self.editor.completion.visible = false;
                true
            }
            // Any other key is ordinary input: dismiss the popup and let the
            // keymap have it.
            _ => {
                self.editor.completion.visible = false;
                false
            }
        }
    }

    /// Resolve `key` through the keymap and run whatever it maps to. Returns
    /// whether the key was accounted for; `false` means nothing is bound to it
    /// and the caller may treat it as input.
    fn run_bound_command(&mut self, key: KeyEvent) -> bool {
        // Runs at most twice: a dead chord in Insert mode types its prefix and
        // then re-resolves the key that ended it, which cannot itself find a
        // pending chord because the buffer was just cleared.
        loop {
            let abandoned: Vec<KeyEvent> = self.input_mapper.pending().to_vec();
            match self.feed_key(self.editor.mode, key) {
                KeyResolution::Pending => return true,
                KeyResolution::Match(cmd_id) => {
                    if let Some(explorer_cmd) = cmd_id.strip_prefix("explorer.") {
                        self.dispatch_explorer_command(explorer_cmd);
                    } else {
                        self.dispatch_command(cmd_id);
                    }
                    return true;
                }
                KeyResolution::NoMatch if !abandoned.is_empty() => {
                    // The chord died. While typing, its keys were real input and
                    // must reappear as text -- an insert-mode chord like `j k`
                    // otherwise eats every stray `j`. Elsewhere they are
                    // discarded, and the terminating key with them.
                    if self.editor.mode != EditorMode::Insert {
                        return true;
                    }
                    let chars: Vec<char> = abandoned.iter().filter_map(Self::typed_char).collect();
                    if let Some(c) = Self::typed_char(&key) {
                        // Both are text: type them together so one keystroke
                        // still produces one `didChange`.
                        let mut all = chars;
                        all.push(c);
                        self.insert_typed_text(&all);
                        return true;
                    }
                    // The terminator carries no text (Esc, Enter, Backspace...).
                    // The mapper only ever saw it as the tail of a dead
                    // sequence, so give it a fresh chance to resolve -- `Esc`
                    // must still leave Insert mode.
                    self.insert_typed_text(&chars);
                    continue;
                }
                KeyResolution::NoMatch => return false,
            }
        }
    }

    /// Handle a key that Insert mode binds to nothing: the completion trigger,
    /// indentation, or plain typing.
    fn handle_insert_key(&mut self, key: KeyEvent) {
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char(' ') {
            self.trigger_completion();
            return;
        }

        if key.code == KeyCode::Tab && key.modifiers.is_empty() {
            // Indentation is not typed text: it must not offer completions the
            // way pressing space or a letter does.
            let indent = self.indent_chars();
            self.insert_text(&indent);
            return;
        }

        if let Some(c) = Self::typed_char(&key) {
            self.insert_typed_text(&[c]);
        }
    }

    /// Feed a key to the mapper, keeping the chord timeout in sync with the
    /// result.
    fn feed_key(&mut self, mode: EditorMode, key: KeyEvent) -> KeyResolution {
        let resolution = self.input_mapper.resolve_key(mode, key);
        // Restart the clock on every key that advances the chord, so
        // `chord_timeout_ms` is the gap a user gets between keys rather than a
        // budget for the whole sequence -- a three-key chord would otherwise
        // leave almost no time for its last key.
        self.chord_started = match resolution {
            KeyResolution::Pending => Some(Instant::now()),
            _ => None,
        };
        resolution
    }

    /// Feed a key to the mapper on behalf of an overlay, putting the keys of a
    /// chord that dies back into the overlay's input.
    ///
    /// Every overlay needs this: keys typed there are query text first and
    /// commands second, so a sequence that turns out to be bound to nothing has
    /// to reappear in the query rather than vanish.
    fn feed_overlay_key(&mut self, mode: EditorMode, key: KeyEvent) -> KeyResolution {
        let abandoned: Vec<KeyEvent> = self.input_mapper.pending().to_vec();
        let resolution = self.feed_key(mode, key);
        if resolution == KeyResolution::NoMatch {
            self.flush_chord_into_overlay(&abandoned);
        }
        resolution
    }

    /// Forget a chord in progress without typing its keys.
    fn abandon_pending_chord(&mut self) {
        if self.input_mapper.clear_pending() {
            self.chord_started = None;
        }
    }

    /// Give up on a chord, putting its keys back as text wherever they were
    /// typed. Use this when something else claims the keyboard mid-chord; use
    /// [`App::abandon_pending_chord`] when the keys should simply be dropped.
    fn recover_pending_chord(&mut self) {
        let abandoned: Vec<KeyEvent> = self.input_mapper.pending().to_vec();
        if abandoned.is_empty() {
            return;
        }
        self.input_mapper.clear_pending();
        self.chord_started = None;
        match self.editor.mode {
            EditorMode::Insert => self.flush_chord_as_text(&abandoned),
            EditorMode::Search | EditorMode::FuzzyFinder | EditorMode::CommandPalette => {
                self.flush_chord_into_overlay(&abandoned)
            }
            _ => {}
        }
    }

    /// Drop a chord the user started but never finished.
    fn expire_pending_chord(&mut self) {
        // A dialog owns the screen; flushing chord keys into the buffer behind
        // it would modify a document the user is being asked about.
        if self.editor.confirm_dialog.is_some() {
            return;
        }
        let Some(started) = self.chord_started else {
            return;
        };
        if started.elapsed() < self.chord_timeout {
            return;
        }
        self.recover_pending_chord();
    }

    /// Insert a character into whichever overlay input is focused.
    ///
    /// Overlays own their keys, so this is also how a dead chord's prefix gets
    /// back into the query it was typed into.
    fn overlay_insert_char(&mut self, c: char) {
        let Some((text, cursor)) = self.overlay_input() else {
            return;
        };
        let byte_idx = char_to_byte_index(text, *cursor);
        text.insert(byte_idx, c);
        *cursor += 1;
        self.overlay_requery();
    }

    /// Delete the character before the cursor of the focused overlay input.
    fn overlay_backspace(&mut self) {
        let Some((text, cursor)) = self.overlay_input() else {
            return;
        };
        if *cursor == 0 {
            return;
        }
        let byte_idx = char_to_byte_index(text, *cursor - 1);
        text.remove(byte_idx);
        *cursor -= 1;
        self.overlay_requery();
    }

    /// The text input the focused overlay is editing, with its cursor. `None`
    /// outside the overlay modes, where there is no input to edit.
    fn overlay_input(&mut self) -> Option<(&mut String, &mut usize)> {
        match self.editor.mode {
            EditorMode::Search if self.editor.search.replace_focused => Some((
                &mut self.editor.search.replace_text,
                &mut self.editor.search.replace_cursor_pos,
            )),
            EditorMode::Search => Some((
                &mut self.editor.search.query,
                &mut self.editor.search.cursor_pos,
            )),
            EditorMode::FuzzyFinder => Some((
                &mut self.editor.fuzzy_finder.query,
                &mut self.editor.fuzzy_finder.cursor_pos,
            )),
            EditorMode::CommandPalette => Some((
                &mut self.editor.command_palette.query,
                &mut self.editor.command_palette.cursor_pos,
            )),
            _ => None,
        }
    }

    /// Re-apply the overlay's filter after its input changed.
    fn overlay_requery(&mut self) {
        // Editing the replace field changes no matches, so it needs no re-filter.
        match self.editor.mode {
            EditorMode::Search if !self.editor.search.replace_focused => {
                rerun_search(&mut self.editor)
            }
            EditorMode::FuzzyFinder => self.editor.fuzzy_finder.update_filter(),
            EditorMode::CommandPalette => self.editor.command_palette.update_filter(),
            _ => {}
        }
    }

    /// Put a dead chord's keys back into the overlay input they were typed into.
    fn flush_chord_into_overlay(&mut self, keys: &[KeyEvent]) {
        for c in keys.iter().filter_map(Self::typed_char) {
            self.overlay_insert_char(c);
        }
    }

    /// Whether a key event carries text a user meant to type.
    fn typed_char(key: &KeyEvent) -> Option<char> {
        let KeyCode::Char(c) = key.code else {
            return None;
        };
        (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT).then_some(c)
    }

    /// The characters one Tab press inserts.
    fn indent_chars(&self) -> Vec<char> {
        if self.editor.config.insert_spaces {
            vec![' '; self.editor.config.tab_size]
        } else {
            vec!['\t']
        }
    }

    /// Insert characters into the active document and tell everyone who needs
    /// to know. Returns the last character that made it in.
    ///
    /// The single path for text entry: notifications go out once for the whole
    /// batch, so a recovered chord still produces one `didChange`, exactly as a
    /// single keystroke does.
    fn insert_text(&mut self, chars: &[char]) -> Option<char> {
        let mut inserted = None;
        for &c in chars {
            if let Err(e) = insert_char(&mut self.editor, c) {
                self.editor.status_message = Some(format!("Error: {e}"));
                break;
            }
            inserted = Some(c);
        }
        inserted?;
        self.lsp_notify_did_change();
        let (path, filename) = self.active_doc_path_info();
        self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
        inserted
    }

    /// Insert text the user typed, offering completions on the last character
    /// the way a live keystroke should.
    fn insert_typed_text(&mut self, chars: &[char]) {
        if let Some(last) = self.insert_text(chars) {
            self.maybe_trigger_completion(last);
        }
    }

    /// Type out the keys of a chord that never completed.
    ///
    /// Only printable keys can be recovered; a modifier combo held as a chord
    /// prefix has no text to insert and is simply dropped.
    fn flush_chord_as_text(&mut self, keys: &[KeyEvent]) {
        let chars: Vec<char> = keys.iter().filter_map(Self::typed_char).collect();
        self.insert_typed_text(&chars);
    }

    /// Run a file-explorer command. These live in `App` because they operate on
    /// the explorer tree rather than on a document.
    fn dispatch_explorer_command(&mut self, command: &str) {
        match command {
            "down" => {
                let style = self.editor.file_tree_style;
                self.editor.file_explorer.move_selection(1, &style);
            }
            "up" => {
                let style = self.editor.file_tree_style;
                self.editor.file_explorer.move_selection(-1, &style);
            }
            "enter" => self.handle_explorer_enter(),
            "expand" => self.handle_explorer_expand(),
            "collapse" => self.handle_explorer_collapse(),
            "refresh" => {
                let selected = self.editor.file_explorer.selected;
                if let Err(e) = self.editor.file_explorer.refresh_node(selected) {
                    self.editor.status_message = Some(format!("Refresh failed: {e}"));
                }
            }
            "refresh_all" => {
                if let Err(e) = self.editor.file_explorer.refresh() {
                    self.editor.status_message = Some(format!("Refresh failed: {e}"));
                }
            }
            "new_file" => self.begin_new_entry(NewEntryKind::File),
            "new_folder" => self.begin_new_entry(NewEntryKind::Directory),
            "copy_path" => self.copy_selected_path(),
            other => log::warn!("Unknown explorer command: explorer.{other}"),
        }
    }

    /// Open the inline row a new file or directory is named in.
    ///
    /// The explorer takes focus first: the row is fed by `handle_new_entry_key`,
    /// which only runs in `FileExplorer` mode, so a button click from the editor
    /// would otherwise leave a row nothing types into.
    fn begin_new_entry(&mut self, kind: NewEntryKind) {
        self.editor.switch_mode(EditorMode::FileExplorer);
        self.editor.file_explorer.begin_new_entry(kind);
        self.editor.status_message = Some(format!(
            "{}: type a name, Enter to create, Esc to cancel",
            kind.prompt()
        ));
    }

    /// Feed one key to the inline new-entry row.
    fn handle_new_entry_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.editor.file_explorer.cancel_new_entry();
                self.editor.status_message = Some("Cancelled".to_string());
                return;
            }
            KeyCode::Enter => {
                self.commit_new_entry();
                return;
            }
            _ => {}
        }

        let Some(input) = self.editor.file_explorer.new_entry.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Backspace => input.backspace(),
            KeyCode::Delete => input.delete(),
            KeyCode::Left => input.move_left(),
            KeyCode::Right => input.move_right(),
            KeyCode::Home => input.move_home(),
            KeyCode::End => input.move_end(),
            _ => {
                if let Some(c) = Self::typed_char(&key) {
                    input.insert_char(c);
                }
            }
        }
    }

    /// Create the entry the inline row names. A new file is opened straight
    /// away, since naming one is how a user asks to start writing it.
    fn commit_new_entry(&mut self) {
        let kind = self
            .editor
            .file_explorer
            .new_entry
            .as_ref()
            .map(|input| input.kind);
        match self.editor.file_explorer.commit_new_entry() {
            Ok(path) => {
                self.editor.status_message = Some(format!("Created: {}", path.display()));
                if kind == Some(NewEntryKind::File) {
                    self.open_file_from_overlay(&path);
                    self.editor.switch_to_default_mode();
                }
            }
            // The row stays open on failure, with the name still in it.
            Err(e) => self.editor.status_message = Some(format!("{e}")),
        }
    }

    /// Put the selected entry's absolute path on the system clipboard.
    fn copy_selected_path(&mut self) {
        let Some(path) = self
            .editor
            .file_explorer
            .selected_path()
            .map(Path::to_path_buf)
        else {
            self.editor.status_message = Some("Nothing selected".to_string());
            return;
        };
        // Joined with the working directory rather than canonicalised: a path
        // to paste elsewhere should not have symlinks resolved out of it, nor
        // carry Windows' verbatim `\\?\` prefix.
        let absolute = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(&path))
                .unwrap_or(path)
        };
        let text = absolute.display().to_string();

        let Some(clipboard) = self.editor.clipboard.as_mut() else {
            self.editor.status_message = Some("Clipboard unavailable".to_string());
            return;
        };
        self.editor.status_message = Some(match clipboard.set_text(&text) {
            Ok(()) => format!("Copied path: {text}"),
            Err(e) => format!("Clipboard error: {e}"),
        });
    }

    fn dispatch_plugin_hook(&mut self, hook: HookEvent) {
        if let Some(pm) = &mut self.plugin_manager {
            match pm.dispatch_hook(hook, &mut self.editor) {
                Ok((buffer_mutated, deferred_actions)) => {
                    if buffer_mutated {
                        self.lsp_notify_did_change();
                        sync_cursor_from_selection(&mut self.editor);
                    }
                    self.process_deferred_actions(deferred_actions);
                }
                Err(e) => {
                    log::error!("Hook dispatch error: {}", e);
                }
            }
        }
    }

    fn dispatch_state_diff_hooks(
        &mut self,
        prev_mode: EditorMode,
        prev_active_tab: usize,
        prev_cursor: Option<(usize, usize)>,
    ) {
        if self.plugin_manager.is_none() {
            return;
        }

        if self.editor.mode != prev_mode {
            self.dispatch_plugin_hook(HookEvent::OnModeChange {
                old_mode: format!("{:?}", prev_mode),
                new_mode: format!("{:?}", self.editor.mode),
            });
        }

        if self.editor.tabs.active != prev_active_tab {
            let (path, filename) = self.active_doc_path_info();
            self.dispatch_plugin_hook(HookEvent::OnTabSwitch { path, filename });
        }

        let cur_cursor = self
            .editor
            .active_view()
            .map(|v| (v.cursor.line, v.cursor.column));
        if cur_cursor != prev_cursor {
            if let Some((line, col)) = cur_cursor {
                self.dispatch_plugin_hook(HookEvent::OnCursorMove { line, col });
            }
        }
    }

    fn active_doc_path_info(&self) -> (Option<String>, Option<String>) {
        if let Some(doc) = self.editor.active_document() {
            let path = doc.path.as_ref().map(|p| p.display().to_string());
            let filename = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string());
            (path, filename)
        } else {
            (None, None)
        }
    }

    fn process_deferred_actions(&mut self, actions: Vec<DeferredAction>) {
        for action in actions {
            match action {
                DeferredAction::OpenFile(path) => {
                    if let Err(e) = self.open_file(&path) {
                        self.editor.status_message =
                            Some(format!("Plugin deferred open error: {e}"));
                    }
                }
                DeferredAction::ExecuteCommand(cmd_id) => {
                    self.dispatch_command(&cmd_id);
                }
            }
        }
    }

    fn dispatch_command(&mut self, cmd_id: &str) {
        // Commands the registry holds only as noops because their behaviour
        // needs `App`; see `register_app_level_commands`. They must be caught
        // here, before the registry would run the noop.
        let app_level: Option<fn(&mut Self)> = match cmd_id {
            "app.quit" => Some(Self::handle_quit),
            "tab.close" => Some(Self::handle_close_tab_with_confirm),
            "palette.open" => Some(Self::open_command_palette),
            "theme.list" => Some(Self::open_theme_palette),
            "keymap.list" => Some(Self::open_keymap_palette),
            "settings.open" => Some(Self::open_settings),
            "goto.definition" => Some(Self::request_definition),
            "lsp.hover" => Some(Self::request_hover),
            "lsp.trigger_completion" => Some(Self::trigger_completion),
            _ => None,
        };
        if let Some(run) = app_level {
            run(self);
            return;
        }

        if cmd_id.starts_with("plugin.") {
            if let Some(pm) = &mut self.plugin_manager {
                match pm.execute_command(cmd_id, &mut self.editor) {
                    Ok((buffer_mutated, deferred_actions)) => {
                        if buffer_mutated {
                            self.lsp_notify_did_change();
                            sync_cursor_from_selection(&mut self.editor);
                            let (path, filename) = self.active_doc_path_info();
                            self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
                        }
                        self.process_deferred_actions(deferred_actions);
                    }
                    Err(e) => {
                        self.editor.status_message = Some(format!("Plugin error: {e}"));
                    }
                }
            }
            return;
        }

        let is_save = cmd_id == "file.save";
        let is_mutation = is_document_mutation(cmd_id);
        let result = self
            .command_registry
            .execute_by_str(cmd_id, &mut self.editor);
        if let Err(e) = result {
            self.editor.status_message = Some(format!("Error: {e}"));
        } else if is_save {
            self.lsp_notify_did_save();
            let (path, filename) = self.active_doc_path_info();
            self.dispatch_plugin_hook(HookEvent::OnSave { path, filename });
        } else if is_mutation {
            self.lsp_notify_did_change();
            let (path, filename) = self.active_doc_path_info();
            self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match self.feed_overlay_key(EditorMode::Search, key) {
            KeyResolution::Pending => return,
            KeyResolution::Match(cmd_id) => {
                // When replace field is focused, Enter replaces current match
                if cmd_id == "search.next" && self.editor.search.replace_focused {
                    self.dispatch_command("search.replace_current");
                    return;
                }
                self.dispatch_command(cmd_id);
                return;
            }
            KeyResolution::NoMatch => {}
        }

        if key.code == KeyCode::Tab && self.editor.search.replace_mode {
            self.editor.search.replace_focused = !self.editor.search.replace_focused;
            return;
        }

        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('h') {
            self.editor.search.replace_mode = !self.editor.search.replace_mode;
            if !self.editor.search.replace_mode {
                self.editor.search.replace_focused = false;
            }
            return;
        }

        if key.modifiers == KeyModifiers::CONTROL | KeyModifiers::ALT && key.code == KeyCode::Enter
        {
            self.dispatch_command("search.replace_all");
            return;
        }

        // Which of the two fields is edited is settled by `replace_focused`,
        // inside the overlay helpers.
        if key.code == KeyCode::Backspace {
            self.overlay_backspace();
            return;
        }

        if let Some(c) = Self::typed_char(&key) {
            self.overlay_insert_char(c);
        }
    }

    fn handle_fuzzy_finder_key(&mut self, key: KeyEvent) {
        match self.feed_overlay_key(EditorMode::FuzzyFinder, key) {
            KeyResolution::Pending => return,
            KeyResolution::Match(cmd_id) => {
                match cmd_id {
                    "fuzzy.close" => self.editor.switch_to_default_mode(),
                    "fuzzy.up" => self.editor.fuzzy_finder.move_selection(-1),
                    "fuzzy.down" => self.editor.fuzzy_finder.move_selection(1),
                    // Anything else a keymap binds here still runs, with the
                    // LSP/plugin notifications `dispatch_command` applies.
                    other => self.dispatch_command(other),
                }
                return;
            }
            KeyResolution::NoMatch => {}
        }

        if key.code == KeyCode::Enter {
            if let Some(path) = self.editor.fuzzy_finder.selected_path() {
                let full_path = self.editor.file_explorer.root.join(path);
                self.open_file_from_overlay(&full_path);
            }
            self.editor.switch_to_default_mode();
            return;
        }

        if key.code == KeyCode::Backspace {
            self.overlay_backspace();
            return;
        }

        if let Some(c) = Self::typed_char(&key) {
            self.overlay_insert_char(c);
        }
    }

    fn handle_command_palette_key(&mut self, key: KeyEvent) {
        match self.feed_overlay_key(EditorMode::CommandPalette, key) {
            KeyResolution::Pending => return,
            KeyResolution::Match(cmd_id) => {
                match cmd_id {
                    "palette.close" => {
                        self.editor.command_palette.mode = PaletteMode::Commands;
                        self.editor.switch_to_default_mode();
                    }
                    "palette.up" => self.editor.command_palette.move_selection(-1),
                    "palette.down" => self.editor.command_palette.move_selection(1),
                    // Anything else a keymap binds here still runs, with the
                    // LSP/plugin notifications `dispatch_command` applies.
                    other => self.dispatch_command(other),
                }
                return;
            }
            KeyResolution::NoMatch => {}
        }

        if key.code == KeyCode::Enter {
            match self.editor.command_palette.mode {
                PaletteMode::Commands => {
                    let cmd_id = self
                        .editor
                        .command_palette
                        .selected_command()
                        .map(|c| c.id.clone());
                    self.editor.switch_to_default_mode();
                    if let Some(id) = cmd_id {
                        self.dispatch_command(&id);
                    }
                }
                PaletteMode::Themes => {
                    let theme_name = self
                        .editor
                        .command_palette
                        .selected_command()
                        .map(|c| c.id.clone());
                    self.editor.command_palette.mode = PaletteMode::Commands;
                    self.editor.switch_to_default_mode();
                    if let Some(name) = theme_name {
                        self.apply_theme(&name);
                    }
                }
                PaletteMode::Keymaps => {
                    let keymap_name = self
                        .editor
                        .command_palette
                        .selected_command()
                        .map(|c| c.id.clone());
                    self.editor.command_palette.mode = PaletteMode::Commands;
                    self.editor.switch_to_default_mode();
                    if let Some(name) = keymap_name {
                        self.apply_keymap(&name);
                    }
                }
            }
            return;
        }

        if key.code == KeyCode::Backspace {
            self.overlay_backspace();
            return;
        }

        if let Some(c) = Self::typed_char(&key) {
            self.overlay_insert_char(c);
        }
    }

    fn open_command_palette(&mut self) {
        let commands: Vec<PaletteItem> = self
            .command_registry
            .list_commands()
            .into_iter()
            .map(|(id, name)| PaletteItem {
                id: id.to_string(),
                name: name.to_string(),
            })
            .collect();
        self.editor.command_palette.query.clear();
        self.editor.command_palette.cursor_pos = 0;
        self.editor.command_palette.mode = PaletteMode::Commands;
        self.editor.command_palette.load_commands(commands);
        self.editor.switch_mode(EditorMode::CommandPalette);
    }

    fn open_theme_palette(&mut self) {
        let themes: Vec<PaletteItem> = list_available_themes()
            .into_iter()
            .map(|name| PaletteItem {
                id: name.clone(),
                name,
            })
            .collect();
        self.editor.command_palette.query.clear();
        self.editor.command_palette.cursor_pos = 0;
        self.editor.command_palette.mode = PaletteMode::Themes;
        self.editor.command_palette.load_commands(themes);
        self.editor.switch_mode(EditorMode::CommandPalette);
    }

    fn open_keymap_palette(&mut self) {
        // `BUILTIN_KEYMAP` is not a file, so it can never collide with a preset
        // name discovered on disk.
        let mut items = vec![PaletteItem {
            id: BUILTIN_KEYMAP.to_string(),
            name: "Built-in (hybrid)".to_string(),
        }];
        items.extend(
            termcode_config::keymap::list_available_keymaps()
                .into_iter()
                .map(|name| PaletteItem {
                    id: name.clone(),
                    name,
                }),
        );
        self.editor.command_palette.query.clear();
        self.editor.command_palette.cursor_pos = 0;
        self.editor.command_palette.mode = PaletteMode::Keymaps;
        self.editor.command_palette.load_commands(items);
        self.editor.switch_mode(EditorMode::CommandPalette);
    }

    /// Switch keymaps for this session. Selecting one from the command palette
    /// does not rewrite `config.toml`; the settings screen saves it separately.
    fn apply_keymap(&mut self, name: &str) {
        let preset = if name == BUILTIN_KEYMAP {
            None
        } else {
            match termcode_config::keymap::load_keymap_preset(name) {
                Some(preset) => Some(preset),
                None => {
                    self.editor.status_message = Some(format!("Keymap '{name}' could not be read"));
                    return;
                }
            }
        };
        self.input_mapper =
            build_input_mapper(preset.as_ref(), &self.command_registry, &self.kb_config);
        self.chord_started = None;
        self.keymap_name = name.to_string();
        self.editor.default_mode = match preset.as_ref() {
            Some(p) if p.meta.starts_in_insert() => EditorMode::Insert,
            _ => EditorMode::Normal,
        };
        // Returning to the resting mode would close the settings screen the
        // keymap was picked from, before the user has finished with it.
        if self.editor.mode != EditorMode::Settings {
            self.editor.switch_to_default_mode();
        }
        let warnings = preset.as_ref().map(|p| p.warnings()).unwrap_or_default();
        self.editor.status_message = Some(if warnings.is_empty() {
            format!("Keymap: {name}")
        } else {
            format!("Keymap: {name}  |  {}", warnings.join("  |  "))
        });
    }

    /// Rebuild the input mapper from the keymap in use plus the current
    /// overrides. Used after a rebinding, which changes `kb_config` in place.
    fn rebuild_input_mapper(&mut self) {
        let preset = (self.keymap_name != BUILTIN_KEYMAP)
            .then(|| termcode_config::keymap::load_keymap_preset(&self.keymap_name))
            .flatten();
        self.input_mapper =
            build_input_mapper(preset.as_ref(), &self.command_registry, &self.kb_config);
        self.chord_started = None;
    }

    fn apply_theme(&mut self, name: &str) {
        let theme_file = format!("{name}.toml");
        let theme_path = termcode_config::default::runtime_dirs()
            .iter()
            .map(|d| d.join("themes").join(&theme_file))
            .find(|p| p.exists());

        let Some(path) = theme_path else {
            self.editor.status_message = Some(format!("Theme not found: {name}"));
            return;
        };

        match termcode_theme::loader::load_theme(&path) {
            Ok(theme) => {
                self.editor.switch_theme(theme);
                self.theme_name = name.to_string();
                self.editor.status_message = Some(format!("Theme: {name}"));
            }
            Err(e) => {
                self.editor.status_message = Some(format!("Failed to load theme: {e}"));
            }
        }
    }

    fn open_file_from_overlay(&mut self, path: &Path) {
        use termcode_view::image::TabContent;

        if is_image_extension(path) {
            let existing_image = self.editor.tabs.tabs.iter().find_map(|t| {
                if let TabContent::Image(image_id) = &t.content {
                    let entry = self.editor.images.get(image_id)?;
                    if entry.path == path {
                        return Some(*image_id);
                    }
                }
                None
            });

            if let Some(image_id) = existing_image {
                if let Some(idx) = self.editor.tabs.find_by_image_id(image_id) {
                    self.editor.tabs.set_active(idx);
                }
                self.editor.sync_active_view_to_tab();
            } else if let Err(e) = self.open_image_file(path) {
                self.editor.status_message = Some(format!("Error: {e}"));
            }
            return;
        }

        let existing_doc = self.editor.tabs.tabs.iter().find_map(|t| match &t.content {
            TabContent::Document(doc_id) => {
                let doc = self.editor.documents.get(doc_id)?;
                if doc.path.as_ref() == Some(&path.to_path_buf()) {
                    Some(*doc_id)
                } else {
                    None
                }
            }
            TabContent::Image(_) => None,
        });

        if let Some(doc_id) = existing_doc {
            if let Some(idx) = self.editor.tabs.find_by_doc_id(doc_id) {
                self.editor.tabs.set_active(idx);
            }
            self.editor.sync_active_view_to_tab();
        } else if let Err(e) = self.open_file(path) {
            self.editor.status_message = Some(format!("Error: {e}"));
        }
    }

    fn handle_close_tab(&mut self) {
        use termcode_view::image::TabContent;
        if let Some(tab) = self.editor.tabs.active_tab() {
            match tab.content {
                TabContent::Document(doc_id) => {
                    let doc = self.editor.documents.get(&doc_id);
                    let close_path = doc
                        .and_then(|d| d.path.as_ref())
                        .map(|p| p.display().to_string());
                    let close_filename = doc
                        .and_then(|d| d.path.as_ref())
                        .and_then(|p| p.file_name())
                        .map(|n| n.to_string_lossy().to_string());
                    self.dispatch_plugin_hook(HookEvent::OnClose {
                        path: close_path,
                        filename: close_filename,
                    });

                    self.lsp_notify_did_close(doc_id);
                    self.editor.close_document(doc_id);
                }
                TabContent::Image(image_id) => {
                    self.image_cache.remove(&image_id);
                    self.editor.close_image(image_id);
                }
            }
        }
        self.after_tab_close();
    }

    /// Move focus after a tab is closed: to the remaining active tab, or to the
    /// file tree when no tabs are left.
    fn after_tab_close(&mut self) {
        if self.editor.tabs.tabs.is_empty() {
            self.editor.active_view = None;
            self.show_sidebar();
        } else {
            self.editor.sync_active_view_to_tab();
        }
    }

    fn handle_quit(&mut self) {
        let modified_count = self
            .editor
            .documents
            .values()
            .filter(|doc| doc.is_modified())
            .count();

        if modified_count == 0 {
            self.should_quit = true;
        } else {
            use termcode_view::confirm::{ConfirmAction, ConfirmDialog};
            let message = format!("You have {modified_count} unsaved file(s).");
            let buttons = vec![
                "Save All & Quit".to_string(),
                "Quit Without Saving".to_string(),
                "Cancel".to_string(),
            ];
            self.editor.confirm_dialog =
                Some(ConfirmDialog::new(ConfirmAction::QuitAll, message, buttons));
        }
    }

    fn handle_close_tab_with_confirm(&mut self) {
        use termcode_view::confirm::{ConfirmAction, ConfirmDialog};
        use termcode_view::image::TabContent;

        if let Some(tab) = self.editor.tabs.active_tab() {
            match tab.content {
                TabContent::Document(doc_id) => {
                    let doc = self.editor.documents.get(&doc_id);
                    let is_modified = doc.is_some_and(|d| d.is_modified());
                    if is_modified {
                        let name = doc
                            .map(|d| d.display_name().to_string())
                            .unwrap_or_else(|| "Untitled".to_string());
                        let message = format!("'{name}' has unsaved changes.");
                        let buttons = vec![
                            "Save & Close".to_string(),
                            "Close Without Saving".to_string(),
                            "Cancel".to_string(),
                        ];
                        self.editor.confirm_dialog = Some(ConfirmDialog::new(
                            ConfirmAction::CloseTab(doc_id),
                            message,
                            buttons,
                        ));
                    } else {
                        self.handle_close_tab();
                    }
                }
                TabContent::Image(_) => {
                    self.handle_close_tab();
                }
            }
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::BackTab => {
                if let Some(ref mut dialog) = self.editor.confirm_dialog {
                    dialog.select_prev();
                }
            }
            KeyCode::Right | KeyCode::Tab => {
                if let Some(ref mut dialog) = self.editor.confirm_dialog {
                    dialog.select_next();
                }
            }
            KeyCode::Esc => {
                self.editor.confirm_dialog = None;
            }
            KeyCode::Enter => {
                self.execute_confirm_action();
            }
            _ => {}
        }
    }

    fn execute_confirm_action(&mut self) {
        use termcode_view::confirm::ConfirmAction;

        let dialog = match self.editor.confirm_dialog.take() {
            Some(d) => d,
            None => return,
        };
        let button = dialog.selected_button;

        match dialog.action {
            ConfirmAction::CloseTab(doc_id) => {
                // Button 0: Save + Close, Button 1: Discard, Button 2: Cancel
                match button {
                    0 => {
                        if !self.editor.documents.contains_key(&doc_id) {
                            return;
                        }
                        match self.editor.save_document(doc_id) {
                            Ok(()) => {
                                self.lsp_notify_did_save_doc(doc_id);
                                let (path, filename) = self.doc_path_info(doc_id);
                                self.dispatch_plugin_hook(HookEvent::OnSave { path, filename });
                                self.close_tab_for_doc(doc_id);
                            }
                            Err(e) => {
                                self.editor.status_message = Some(format!("Save failed: {e}"));
                            }
                        }
                    }
                    1 if self.editor.documents.contains_key(&doc_id) => {
                        self.close_tab_for_doc(doc_id);
                    }
                    _ => {
                        // Cancel -- dialog already dismissed via .take()
                    }
                }
            }
            ConfirmAction::QuitAll => {
                // Button 0: Save all + Quit, Button 1: Discard + Quit, Button 2: Cancel
                match button {
                    0 => {
                        let modified_ids: Vec<_> = self
                            .editor
                            .documents
                            .iter()
                            .filter(|(_, doc)| doc.is_modified())
                            .map(|(id, _)| *id)
                            .collect();
                        for doc_id in modified_ids {
                            match self.editor.save_document(doc_id) {
                                Ok(()) => {
                                    self.lsp_notify_did_save_doc(doc_id);
                                    let (path, filename) = self.doc_path_info(doc_id);
                                    self.dispatch_plugin_hook(HookEvent::OnSave { path, filename });
                                }
                                Err(e) => {
                                    self.editor.status_message = Some(format!("Save failed: {e}"));
                                    return;
                                }
                            }
                        }
                        self.should_quit = true;
                    }
                    1 => {
                        self.should_quit = true;
                    }
                    _ => {
                        // Cancel
                    }
                }
            }
        }
    }

    fn close_tab_for_doc(&mut self, doc_id: termcode_view::document::DocumentId) {
        let doc = self.editor.documents.get(&doc_id);
        let close_path = doc
            .and_then(|d| d.path.as_ref())
            .map(|p| p.display().to_string());
        let close_filename = doc
            .and_then(|d| d.path.as_ref())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());
        self.dispatch_plugin_hook(HookEvent::OnClose {
            path: close_path,
            filename: close_filename,
        });
        self.lsp_notify_did_close(doc_id);
        self.editor.close_document(doc_id);

        self.after_tab_close();
    }

    fn doc_path_info(
        &self,
        doc_id: termcode_view::document::DocumentId,
    ) -> (Option<String>, Option<String>) {
        if let Some(doc) = self.editor.documents.get(&doc_id) {
            let path = doc.path.as_ref().map(|p| p.display().to_string());
            let filename = doc
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string());
            (path, filename)
        } else {
            (None, None)
        }
    }

    fn lsp_notify_did_save_doc(&self, doc_id: termcode_view::document::DocumentId) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let doc = match self.editor.documents.get(&doc_id) {
            Some(d) => d,
            None => return,
        };
        let language = match &doc.language_id {
            Some(id) => id.as_ref().to_string(),
            None => return,
        };
        let path = match &doc.path {
            Some(p) => p.clone(),
            None => return,
        };
        let uri = make_file_uri(&path);
        bridge.notify_did_save(&language, &uri);
    }

    fn handle_explorer_enter(&mut self) {
        let selected = self.editor.file_explorer.selected;
        if selected >= self.editor.file_explorer.tree.len() {
            return;
        }

        let kind = self.editor.file_explorer.tree[selected].kind;
        match kind {
            // Enter *enters*: the directory becomes the root, where the arrow
            // keys expand it inside the tree it is already part of.
            FileNodeKind::Directory => self.navigate_explorer_into(selected),
            FileNodeKind::File | FileNodeKind::Symlink => {
                let path = self.editor.file_explorer.tree[selected].path.clone();
                self.open_file_from_overlay(&path);
                self.editor.switch_to_default_mode();
            }
        }
    }

    fn handle_explorer_expand(&mut self) {
        let selected = self.editor.file_explorer.selected;
        if selected >= self.editor.file_explorer.tree.len() {
            return;
        }
        if self.editor.file_explorer.tree[selected].is_parent {
            self.navigate_explorer_to_parent();
            return;
        }
        let node = &self.editor.file_explorer.tree[selected];
        if node.kind == FileNodeKind::Directory && !node.expanded {
            if let Err(e) = self.editor.file_explorer.toggle_expand(selected) {
                self.editor.status_message = Some(format!("Error: {e}"));
            }
            self.editor
                .file_explorer
                .compute_scroll_left(&self.editor.file_tree_style);
        }
    }

    fn handle_explorer_collapse(&mut self) {
        let selected = self.editor.file_explorer.selected;
        if selected >= self.editor.file_explorer.tree.len() {
            return;
        }
        let node = &self.editor.file_explorer.tree[selected];
        if node.kind == FileNodeKind::Directory && node.expanded {
            if let Err(e) = self.editor.file_explorer.toggle_expand(selected) {
                self.editor.status_message = Some(format!("Error: {e}"));
            }
            self.editor
                .file_explorer
                .compute_scroll_left(&self.editor.file_tree_style);
        } else {
            let current_depth = node.depth;
            if current_depth > 0 {
                for i in (0..selected).rev() {
                    if self.editor.file_explorer.tree[i].depth < current_depth
                        && !self.editor.file_explorer.tree[i].is_parent
                    {
                        self.editor.file_explorer.selected = i;
                        let vh = self.editor.file_explorer.viewport_height;
                        self.editor.file_explorer.ensure_visible(vh);
                        break;
                    }
                }
            }
        }
    }

    /// Re-root the tree one level up, from the `..` row.
    fn navigate_explorer_to_parent(&mut self) {
        if let Err(e) = self.editor.file_explorer.navigate_to_parent() {
            self.editor.status_message = Some(format!("Error: {e}"));
            return;
        }
        self.editor
            .file_explorer
            .compute_scroll_left(&self.editor.file_tree_style);
    }

    /// Re-root the tree at the directory on row `index`.
    fn navigate_explorer_into(&mut self, index: usize) {
        if let Err(e) = self.editor.file_explorer.navigate_into(index) {
            self.editor.status_message = Some(format!("Error: {e}"));
            return;
        }
        self.editor
            .file_explorer
            .compute_scroll_left(&self.editor.file_tree_style);
    }

    // --- LSP lifecycle helpers ---

    fn lsp_notify_did_open(&self, doc_id: termcode_view::document::DocumentId) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let doc = match self.editor.documents.get(&doc_id) {
            Some(d) => d,
            None => return,
        };
        let language = match &doc.language_id {
            Some(id) => id.as_ref().to_string(),
            None => return,
        };
        if !bridge.has_server(&language) {
            return;
        }
        let path = match &doc.path {
            Some(p) => p.clone(),
            None => return,
        };
        let uri = make_file_uri(&path);
        let root_uri = make_file_uri(&self.editor.file_explorer.root);
        let text = doc.buffer.text().to_string();
        let version = doc.version;

        let did_open = crate::lsp_bridge::DidOpenParams {
            uri,
            language_id: language.clone(),
            version,
            text,
        };

        // If the server is already running, send didOpen directly.
        // Otherwise, queue didOpen inside start_server so it fires after initialization.
        if bridge.has_running_client(&language) {
            bridge.notify_did_open(
                &did_open.language_id,
                &did_open.uri,
                &did_open.language_id,
                did_open.version,
                &did_open.text,
            );
        } else {
            bridge.start_server_with_did_open(&language, &root_uri, Some(did_open));
        }
    }

    fn lsp_notify_did_change(&self) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let doc = match self.editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let language = match &doc.language_id {
            Some(id) => id.as_ref().to_string(),
            None => return,
        };
        let path = match &doc.path {
            Some(p) => p.clone(),
            None => return,
        };
        let uri = make_file_uri(&path);
        let text = doc.buffer.text().to_string();
        bridge.notify_did_change(&language, &uri, doc.version, &text);
    }

    fn lsp_notify_did_save(&self) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let doc = match self.editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let language = match &doc.language_id {
            Some(id) => id.as_ref().to_string(),
            None => return,
        };
        let path = match &doc.path {
            Some(p) => p.clone(),
            None => return,
        };
        let uri = make_file_uri(&path);
        bridge.notify_did_save(&language, &uri);
    }

    fn lsp_notify_did_close(&self, doc_id: termcode_view::document::DocumentId) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let doc = match self.editor.documents.get(&doc_id) {
            Some(d) => d,
            None => return,
        };
        let language = match &doc.language_id {
            Some(id) => id.as_ref().to_string(),
            None => return,
        };
        let path = match &doc.path {
            Some(p) => p.clone(),
            None => return,
        };
        let uri = make_file_uri(&path);
        bridge.notify_did_close(&language, &uri);
    }

    // --- Completion helpers ---

    fn trigger_completion(&mut self) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let (language, uri, position) = match self.get_cursor_lsp_context() {
            Some(ctx) => ctx,
            None => return,
        };
        if let Some(view) = self.editor.active_view() {
            self.editor.completion.trigger_position =
                termcode_core::position::Position::new(view.cursor.line, view.cursor.column);
        }
        bridge.request_completion(&language, &uri, position);
    }

    fn maybe_trigger_completion(&mut self, ch: char) {
        if self.lsp_bridge.is_none() {
            return;
        }
        let ch_str = ch.to_string();
        let language = self
            .editor
            .active_document()
            .and_then(|d| d.language_id.as_ref())
            .map(|id| id.as_ref().to_string());
        let should_trigger = if let Some(ref lang) = language {
            if let Some(triggers) = self.lsp_trigger_chars.get(lang) {
                triggers.iter().any(|t| t == &ch_str)
            } else {
                // Fallback defaults until server capabilities arrive.
                matches!(ch, '.' | ':')
            }
        } else {
            false
        };
        if should_trigger {
            self.trigger_completion();
        }
    }

    fn accept_completion(&mut self) {
        let selected = self.editor.completion.selected;
        let insert_text = match self.editor.completion.items.get(selected) {
            Some(item) => item.insert_text.clone(),
            None => return,
        };
        self.editor.completion.visible = false;

        let trigger_pos = self.editor.completion.trigger_position;
        let doc = match self.editor.active_document() {
            Some(d) => d,
            None => return,
        };
        let view = match self.editor.active_view() {
            Some(v) => v,
            None => return,
        };

        if trigger_pos.line != view.cursor.line {
            return;
        }

        let line_idx = view.cursor.line;
        if line_idx >= doc.buffer.line_count() {
            return;
        }

        let line_byte_start = doc.buffer.text().line_to_byte(line_idx);
        let rope_line = doc.buffer.line(line_idx);
        let line_text: String = rope_line.chars().collect();
        let line_text = line_text.trim_end_matches('\n').trim_end_matches('\r');

        let trigger_byte = line_text
            .char_indices()
            .nth(trigger_pos.column)
            .map(|(i, _)| i)
            .unwrap_or(line_text.len());
        let cursor_byte = line_text
            .char_indices()
            .nth(view.cursor.column)
            .map(|(i, _)| i)
            .unwrap_or(line_text.len());

        let from = line_byte_start + trigger_byte;
        let to = line_byte_start + cursor_byte;
        let doc_len = doc.buffer.len_bytes();

        let txn = termcode_core::transaction::Transaction::replace(from..to, &insert_text, doc_len);
        if let Err(e) = self
            .editor
            .active_document_mut()
            .unwrap()
            .apply_transaction(&txn)
        {
            self.editor.status_message = Some(format!("Error: {e}"));
            return;
        }

        if let Some(view) = self.editor.active_view_mut() {
            view.cursor.column = trigger_pos.column + insert_text.chars().count();
        }
        crate::command::sync_selection_from_cursor(&mut self.editor);
        self.lsp_notify_did_change();
        let (path, filename) = self.active_doc_path_info();
        self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
    }

    fn request_hover(&mut self) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let (language, uri, position) = match self.get_cursor_lsp_context() {
            Some(ctx) => ctx,
            None => return,
        };
        bridge.request_hover(&language, &uri, position);
    }

    fn request_definition(&mut self) {
        let bridge = match &self.lsp_bridge {
            Some(b) => b,
            None => return,
        };
        let (language, uri, position) = match self.get_cursor_lsp_context() {
            Some(ctx) => ctx,
            None => return,
        };
        bridge.request_definition(&language, &uri, position);
    }

    fn get_cursor_lsp_context(
        &self,
    ) -> Option<(String, String, termcode_core::position::Position)> {
        let doc = self.editor.active_document()?;
        let view = self.editor.active_view()?;
        let language = doc.language_id.as_ref()?.as_ref().to_string();
        let path = doc.path.as_ref()?;
        let uri = make_file_uri(path);
        let position = termcode_core::position::Position::new(view.cursor.line, view.cursor.column);
        Some((language, uri, position))
    }
}

fn is_image_extension(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_ascii_lowercase(),
        None => return false,
    };
    matches!(
        ext.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "tiff" | "tif"
    )
}

/// Returns true for command IDs that mutate the document content.
fn is_document_mutation(cmd_id: &str) -> bool {
    matches!(
        cmd_id,
        "edit.backspace"
            | "edit.delete_char"
            | "edit.newline"
            | "edit.undo"
            | "edit.redo"
            | "edit.delete_line"
            | "edit.delete_word_before"
            | "edit.paste_after"
            | "edit.paste_before"
            | "edit.open_below"
            | "edit.open_above"
            | "clipboard.cut"
            | "clipboard.paste"
            | "search.replace_current"
            | "search.replace_all"
    )
}

/// Build the input mapper for a keymap preset (or the built-in default) and
/// layer the user's `keybindings.toml` overrides on top.
fn build_input_mapper(
    preset: Option<&termcode_config::keymap::KeymapPreset>,
    registry: &CommandRegistry,
    kb_config: &termcode_config::keymap::KeybindingConfig,
) -> InputMapper {
    let mut mapper = match preset {
        Some(preset) => InputMapper::from_preset(preset, registry),
        None => InputMapper::new(),
    };
    mapper.apply_overrides(kb_config, registry);
    mapper
}

/// Construct a file:// URI string with percent-encoding.
fn make_file_uri(path: &Path) -> String {
    termcode_lsp::types::path_to_uri_string(path)
}

/// Parse a file:// URI string back to a PathBuf with percent-decoding.
fn parse_file_uri(uri: &str) -> Option<PathBuf> {
    termcode_lsp::types::uri_str_to_path(uri)
}

/// Convert a char index to a byte index in a string.
/// Returns the string's byte length if char_pos is at or past the end.
fn char_to_byte_index(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

fn load_default_theme() -> Theme {
    let theme_toml = include_str!("../../../runtime/themes/one-dark.toml");
    parse_theme(theme_toml).unwrap_or_default()
}

fn load_theme_by_name(name: &str) -> Result<Theme, termcode_theme::loader::ThemeError> {
    let theme_file = format!("{name}.toml");
    for dir in termcode_config::default::runtime_dirs() {
        let path = dir.join("themes").join(&theme_file);
        if path.exists() {
            return termcode_theme::loader::load_theme(&path);
        }
    }
    // Fallback: try runtime_dir (even if it doesn't exist, to get a proper error)
    let fallback = termcode_config::default::runtime_dir()
        .join("themes")
        .join(&theme_file);
    termcode_theme::loader::load_theme(&fallback)
}

fn list_available_themes() -> Vec<String> {
    let mut themes = Vec::new();
    for dir in termcode_config::default::runtime_dirs() {
        let themes_dir = dir.join("themes");
        if let Ok(entries) = std::fs::read_dir(&themes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !themes.contains(&stem.to_string()) {
                            themes.push(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    themes.sort();
    themes
}

/// Restore the terminal from a panic handler and then run the original hook.
///
/// The release profile uses `panic = "abort"`, so no unwinding (and therefore no
/// `Drop`) runs on panic. Without this hook the terminal is left in raw mode with
/// the alternate screen and mouse capture still active, which floods the shell
/// with mouse escape sequences and makes it unusable.
fn install_panic_hook() {
    static HOOK_INSTALLED: Once = Once::new();
    HOOK_INSTALLED.call_once(|| {
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            if MOUSE_CAPTURE_ACTIVE.load(Ordering::Relaxed) {
                let _ = execute!(stdout, DisableMouseCapture);
            }
            let _ = execute!(stdout, LeaveAlternateScreen, Show);
            let _ = disable_raw_mode();
            original_hook(info);
        }));
    });
}

/// Whether mouse capture is currently active. Read by the panic hook, which
/// cannot see the config of whichever `setup_terminal` call is live.
static MOUSE_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

fn setup_terminal(mouse_enabled: bool) -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    install_panic_hook();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let mut entered_alt_screen = false;
    let mut mouse_captured = false;

    let setup_result = (|| -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
        execute!(stdout, EnterAlternateScreen)?;
        entered_alt_screen = true;
        if mouse_enabled {
            execute!(stdout, EnableMouseCapture)?;
            mouse_captured = true;
            MOUSE_CAPTURE_ACTIVE.store(true, Ordering::Relaxed);
        }
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(terminal)
    })();

    if let Err(e) = setup_result {
        let mut cleanup_stdout = io::stdout();
        if mouse_enabled && mouse_captured {
            let _ = execute!(cleanup_stdout, DisableMouseCapture);
            MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        }
        if entered_alt_screen {
            let _ = execute!(cleanup_stdout, LeaveAlternateScreen);
        }
        let _ = disable_raw_mode();
        return Err(e);
    }

    setup_result
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mouse_enabled: bool,
) -> anyhow::Result<()> {
    disable_raw_mode()?;
    if mouse_enabled {
        MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Relaxed);
        execute!(
            terminal.backend_mut(),
            DisableMouseCapture,
            LeaveAlternateScreen
        )?;
    } else {
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    }
    terminal.show_cursor()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use termcode_config::config::AppConfig;

    struct TestFile(PathBuf);

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// An app whose Insert mode carries a `j k` chord, mirroring the binding
    /// `config/keybindings.toml` advertises.
    fn app_with_insert_chord(name: &str) -> (App, TestFile) {
        let path = std::env::temp_dir().join(format!("termcode-app-test-{name}.txt"));
        std::fs::write(&path, "abc\n").unwrap();

        // The built-in keymap: this is about recovering a dead Insert-mode
        // chord, which needs a Normal mode to fall back to.
        let mut config = AppConfig::default();
        config.keymap.preset = None;
        let mut app = App::with_config(None, config);
        app.open_file(&path).unwrap();

        let overrides: termcode_config::keymap::KeybindingConfig = toml::from_str(
            r#"
[mode.insert]
"j k" = "mode.normal"
"#,
        )
        .unwrap();
        app.input_mapper
            .apply_overrides(&overrides, &app.command_registry);
        app.editor.switch_mode(EditorMode::Insert);
        (app, TestFile(path))
    }

    fn press(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn code(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            app.handle_key(press(c));
        }
    }

    fn text(app: &App) -> String {
        app.editor
            .active_document()
            .unwrap()
            .buffer
            .text()
            .to_string()
    }

    /// An app rooted in an empty scratch directory, focused on the explorer.
    fn app_on_empty_dir(name: &str) -> (App, TestDir) {
        let dir = std::env::temp_dir().join(format!("termcode-app-explorer-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_config(Some(dir.clone()), AppConfig::default());
        app.editor.switch_mode(EditorMode::FileExplorer);
        (app, TestDir(dir))
    }

    struct TestDir(PathBuf);

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn naming_a_new_file_creates_it_and_opens_it() {
        let (mut app, dir) = app_on_empty_dir("new-file");
        app.dispatch_explorer_command("new_file");

        // Every letter is part of the name -- `n` and `y` are explorer
        // bindings, and must not run while the row is open.
        type_str(&mut app, "notes.ny");
        assert_eq!(
            app.editor.file_explorer.new_entry.as_ref().unwrap().name,
            "notes.ny"
        );

        app.handle_key(code(KeyCode::Enter));
        let created = dir.0.join("notes.ny");
        assert!(created.is_file(), "the file should exist on disk");
        assert!(app.editor.file_explorer.new_entry.is_none());
        assert_eq!(
            app.editor.active_document().and_then(|d| d.path.clone()),
            Some(created),
            "a file is opened as soon as it is named"
        );
    }

    #[test]
    fn naming_a_new_folder_creates_it_and_leaves_the_explorer_focused() {
        let (mut app, dir) = app_on_empty_dir("new-folder");
        app.dispatch_explorer_command("new_folder");
        type_str(&mut app, "src");
        app.handle_key(code(KeyCode::Enter));

        assert!(dir.0.join("src").is_dir());
        assert_eq!(app.editor.mode, EditorMode::FileExplorer);
        assert!(app.editor.tabs.tabs.is_empty(), "a folder opens no tab");
    }

    #[test]
    fn esc_drops_the_name_without_creating_anything() {
        let (mut app, dir) = app_on_empty_dir("cancel");
        app.dispatch_explorer_command("new_file");
        type_str(&mut app, "gone.rs");
        app.handle_key(code(KeyCode::Esc));

        assert!(app.editor.file_explorer.new_entry.is_none());
        assert!(!dir.0.join("gone.rs").exists());
        // Esc closed the row, not the explorer.
        assert_eq!(app.editor.mode, EditorMode::FileExplorer);
    }

    #[test]
    fn leaving_the_explorer_drops_a_half_typed_name() {
        let (mut app, _dir) = app_on_empty_dir("leave");
        app.dispatch_explorer_command("new_file");
        type_str(&mut app, "half");
        app.editor.switch_mode(EditorMode::Normal);
        assert!(app.editor.file_explorer.new_entry.is_none());
    }

    #[test]
    fn copying_a_path_needs_something_selected() {
        let (mut app, _dir) = app_on_empty_dir("copy-empty");
        app.dispatch_explorer_command("copy_path");
        assert_eq!(
            app.editor.status_message.as_deref(),
            Some("Nothing selected")
        );
    }

    #[test]
    fn copying_a_path_puts_an_absolute_path_on_the_clipboard() {
        let (mut app, dir) = app_on_empty_dir("copy");
        app.editor.clipboard = Some(Box::new(crate::clipboard::MockClipboard::new()));
        app.dispatch_explorer_command("new_folder");
        type_str(&mut app, "src");
        app.handle_key(code(KeyCode::Enter));

        app.dispatch_explorer_command("copy_path");
        let copied = app.editor.clipboard.as_mut().unwrap().get_text();
        let expected = dir.0.join("src");
        assert_eq!(
            copied.as_deref(),
            Some(expected.display().to_string().as_str())
        );
        assert!(expected.is_absolute());
    }

    #[test]
    fn clicking_cancel_in_the_quit_dialog_keeps_the_editor_open() {
        let (mut app, _f) = app_with_insert_chord("quit-dialog");
        app.terminal_size = (80, 24);
        // A modified document is what puts the dialog up in the first place.
        app.handle_key(press('x'));
        app.handle_quit();
        let dialog = app
            .editor
            .confirm_dialog
            .as_ref()
            .expect("unsaved work must be confirmed");
        let cancel = dialog.buttons.len() - 1;
        let placed =
            crate::ui::confirm_dialog::layout(dialog, ratatui::layout::Rect::new(0, 0, 80, 24))
                .unwrap();
        let (x, _) = placed.buttons[cancel];
        let y = placed.button_y;

        let click = crossterm::event::MouseEvent {
            kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        };
        // Cancel does not start focused, so the first click only moves to it.
        app.handle_mouse(click);
        assert!(app.editor.confirm_dialog.is_some(), "still deciding");
        assert_eq!(
            app.editor.confirm_dialog.as_ref().unwrap().selected_button,
            cancel
        );

        app.handle_mouse(click);
        assert!(app.editor.confirm_dialog.is_none(), "the dialog closed");
        assert!(!app.should_quit, "Cancel must not quit");
    }

    #[test]
    fn insert_chord_completes_when_both_keys_arrive() {
        let (mut app, _f) = app_with_insert_chord("chord_ok");
        app.handle_key(press('j'));
        app.handle_key(press('k'));
        assert_eq!(app.editor.mode, EditorMode::Normal);
        assert_eq!(text(&app), "abc\n");
    }

    #[test]
    fn insert_dead_chord_types_its_prefix_and_the_terminating_char() {
        let (mut app, _f) = app_with_insert_chord("chord_dead");
        app.handle_key(press('j'));
        app.handle_key(press('x'));
        assert_eq!(text(&app), "jxabc\n");
        assert_eq!(app.editor.mode, EditorMode::Insert);
    }

    /// The terminating key carries no text, so it must still get to run its own
    /// binding rather than being consumed with the dead chord.
    #[test]
    fn insert_dead_chord_still_honours_esc() {
        let (mut app, _f) = app_with_insert_chord("chord_esc");
        app.handle_key(press('j'));
        app.handle_key(code(KeyCode::Esc));
        assert_eq!(text(&app), "jabc\n");
        assert_eq!(app.editor.mode, EditorMode::Normal);
    }

    #[test]
    fn insert_dead_chord_still_honours_enter() {
        let (mut app, _f) = app_with_insert_chord("chord_enter");
        app.handle_key(press('j'));
        app.handle_key(code(KeyCode::Enter));
        assert_eq!(text(&app), "j\nabc\n");
        assert_eq!(app.editor.mode, EditorMode::Insert);
    }

    #[test]
    fn insert_dead_chord_still_honours_backspace() {
        let (mut app, _f) = app_with_insert_chord("chord_bs");
        app.handle_key(press('j'));
        app.handle_key(code(KeyCode::Backspace));
        // `j` is typed, then Backspace removes it again.
        assert_eq!(text(&app), "abc\n");
        assert_eq!(app.editor.mode, EditorMode::Insert);
    }

    #[test]
    fn typing_and_backspace_edit_the_focused_overlay_input() {
        let (mut app, _f) = app_with_insert_chord("overlay_query");
        app.editor.switch_mode(EditorMode::FuzzyFinder);
        type_str(&mut app, "abc");
        app.handle_key(code(KeyCode::Backspace));
        assert_eq!(app.editor.fuzzy_finder.query, "ab");
        assert_eq!(app.editor.fuzzy_finder.cursor_pos, 2);
    }

    /// The search overlay has two inputs. Both typing and deleting must follow
    /// the focused one, and leave the other alone.
    #[test]
    fn search_backspace_follows_the_focused_field() {
        let (mut app, _f) = app_with_insert_chord("overlay_replace");
        app.editor.switch_mode(EditorMode::Search);
        type_str(&mut app, "ab");

        app.editor.search.replace_mode = true;
        app.editor.search.replace_focused = true;
        type_str(&mut app, "xy");
        app.handle_key(code(KeyCode::Backspace));

        assert_eq!(app.editor.search.replace_text, "x");
        assert_eq!(app.editor.search.query, "ab");
    }

    /// A chord in progress must not survive a dialog or a quit prompt.
    #[test]
    fn confirm_dialog_abandons_a_pending_chord() {
        let (mut app, _f) = app_with_insert_chord("chord_dialog");
        app.handle_key(press('j'));
        assert!(app.input_mapper.has_pending());

        app.editor.confirm_dialog = Some(termcode_view::confirm::ConfirmDialog::new(
            termcode_view::confirm::ConfirmAction::CloseTab(termcode_view::document::DocumentId(0)),
            "Save?".to_string(),
            vec!["Save".to_string(), "Discard".to_string()],
        ));
        app.handle_key(code(KeyCode::Esc));

        assert!(!app.input_mapper.has_pending());
        assert_eq!(text(&app), "abc\n");
    }
}
