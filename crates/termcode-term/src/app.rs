use std::collections::HashMap;
use std::io::{self, Stdout};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crossterm::event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use termcode_config::config::AppConfig;
use termcode_core::config_types::EditorConfig;
use termcode_lsp::types::LspResponse;
use termcode_syntax::language::LanguageRegistry;
use termcode_theme::loader::parse_theme;
use termcode_theme::theme::Theme;
use termcode_view::editor::{Editor, EditorMode};
use termcode_view::file_explorer::FileNodeKind;

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

use crate::input::InputMapper;
use crate::layout;
use crate::lsp_bridge::LspBridge;
use crate::mouse::{self, MouseAction};
use crate::render;

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
}

impl App {
    pub fn new(root: Option<PathBuf>) -> Self {
        let config_path = termcode_config::default::config_dir().join("config.toml");
        let mut app_config = AppConfig::load(&config_path);

        // Also try project-local config/config.toml
        let project_config = PathBuf::from("config/config.toml");
        if project_config.exists() {
            app_config = AppConfig::load(&project_config);
        }

        Self::with_config(root, app_config)
    }

    pub fn with_config(root: Option<PathBuf>, app_config: AppConfig) -> Self {
        let theme = load_default_theme();
        let config = EditorConfig::default();
        let lang_registry = LanguageRegistry::with_builtins();
        let mut editor = Editor::new(theme, config, lang_registry, root);
        editor.file_tree_style = app_config.ui.file_tree_style;
        editor.clipboard = Some(Box::new(crate::clipboard::ArboardClipboard::new()));

        let mut command_registry = CommandRegistry::new();
        register_builtin_commands(&mut command_registry);

        let mut input_mapper = InputMapper::new();
        let keybindings_path = termcode_config::default::config_dir().join("keybindings.toml");
        let kb_config = termcode_config::keymap::load_keybindings(&keybindings_path);
        input_mapper.apply_overrides(&kb_config, &command_registry);

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
                    let mut plugin_dirs =
                        vec![termcode_config::default::runtime_dir().join("plugins")];
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

                    input_mapper.apply_overrides(&kb_config, &command_registry);

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
        }
    }

    pub fn show_sidebar(&mut self) {
        self.editor.file_explorer.visible = true;
        self.editor.switch_mode(EditorMode::FileExplorer);
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
        let image_id = self.editor.open_image(path, ext, file_size);

        if let Some(picker) = &mut self.image_picker {
            match image::open(path) {
                Ok(dyn_image) => {
                    let protocol = picker.new_resize_protocol(dyn_image);
                    self.image_cache.insert(image_id, Mutex::new(protocol));
                }
                Err(e) => {
                    self.editor.status_message = Some(format!("Failed to decode image: {e}"));
                }
            }
        }
        Ok(())
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        let mut terminal = setup_terminal(self.editor.config.mouse_enabled)?;

        terminal.draw(|frame| render::render(frame, &self.editor, &self.image_cache))?;

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
                let palette_height = PALETTE_MAX_HEIGHT.min(app_layout.editor_area.height) as usize;
                self.editor.command_palette.visible_height =
                    palette_height.saturating_sub(OVERLAY_CHROME);
            }

            self.editor.sync_tab_modified();
            terminal.draw(|frame| render::render(frame, &self.editor, &self.image_cache))?;
        }

        if let Some(ref bridge) = self.lsp_bridge {
            bridge.shutdown();
        }

        self.save_session();

        restore_terminal(&mut terminal, self.mouse_enabled)?;
        Ok(())
    }

    pub fn restore_session(&mut self) {
        let root = self.editor.file_explorer.root.clone();
        if let Some(session) = crate::session::load_session(&root) {
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
                self.sync_active_view_to_tab();
            }
        }
    }

    fn save_session(&self) {
        let root = self.editor.file_explorer.root.clone();
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
            AppEvent::Tick => {}
            AppEvent::Lsp(response) => self.handle_lsp_response(response),
        }
    }

    fn handle_mouse(&mut self, event: crossterm::event::MouseEvent) {
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
            MouseAction::SwitchTab(index) => {
                self.editor.tabs.set_active(index);
                self.sync_active_view_to_tab();
            }
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
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }

        // Ctrl+C: copy selection to clipboard.
        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
            if let Err(e) = self
                .command_registry
                .execute("clipboard.copy", &mut self.editor)
            {
                self.editor.status_message = Some(format!("Error: {e}"));
            }
            return;
        }

        self.editor.hover.visible = false;

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
            _ => {}
        }

        if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('w') {
            self.handle_close_tab();
            return;
        }

        if let Some(cmd_id) = self.input_mapper.resolve_global(key) {
            self.dispatch_command(cmd_id);
            return;
        }

        if self.editor.mode == EditorMode::Insert {
            if self.editor.completion.visible {
                match key.code {
                    KeyCode::Down => {
                        let len = self.editor.completion.items.len();
                        if len > 0 {
                            self.editor.completion.selected =
                                (self.editor.completion.selected + 1) % len;
                        }
                        return;
                    }
                    KeyCode::Up => {
                        let len = self.editor.completion.items.len();
                        if len > 0 {
                            self.editor.completion.selected =
                                (self.editor.completion.selected + len - 1) % len;
                        }
                        return;
                    }
                    KeyCode::Enter | KeyCode::Tab => {
                        self.accept_completion();
                        return;
                    }
                    KeyCode::Esc => {
                        self.editor.completion.visible = false;
                        return;
                    }
                    _ => {
                        self.editor.completion.visible = false;
                    }
                }
            }

            if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char(' ') {
                self.trigger_completion();
                return;
            }

            if let KeyCode::Char(c) = key.code {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    if let Err(e) = insert_char(&mut self.editor, c) {
                        self.editor.status_message = Some(format!("Error: {e}"));
                    }
                    self.lsp_notify_did_change();
                    let (path, filename) = self.active_doc_path_info();
                    self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
                    self.maybe_trigger_completion(c);
                    return;
                }
            }
        }

        if self.editor.mode == EditorMode::FileExplorer {
            if let Some(cmd_id) = self.input_mapper.resolve(EditorMode::FileExplorer, key) {
                match cmd_id {
                    "explorer.down" => self.editor.file_explorer.move_selection(1),
                    "explorer.up" => self.editor.file_explorer.move_selection(-1),
                    "explorer.enter" => self.handle_explorer_enter(),
                    "explorer.expand" => self.handle_explorer_expand(),
                    "explorer.collapse" => self.handle_explorer_collapse(),
                    "mode.normal" => self.editor.switch_mode(EditorMode::Normal),
                    _ => {}
                }
            }
            return;
        }

        if let Some(cmd_id) = self.input_mapper.resolve(self.editor.mode, key) {
            self.dispatch_command(cmd_id);
        }
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
        if cmd_id == "palette.open" {
            self.open_command_palette();
            return;
        }
        if cmd_id == "goto.definition" {
            self.request_definition();
            return;
        }
        if cmd_id == "lsp.hover" {
            self.request_hover();
            return;
        }
        if cmd_id == "lsp.trigger_completion" {
            self.trigger_completion();
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
        if let Some(cmd_id) = self.input_mapper.resolve(EditorMode::Search, key) {
            // When replace field is focused, Enter replaces current match
            if cmd_id == "search.next" && self.editor.search.replace_focused {
                if let Err(e) = self
                    .command_registry
                    .execute("search.replace_current", &mut self.editor)
                {
                    self.editor.status_message = Some(format!("Error: {e}"));
                } else {
                    self.lsp_notify_did_change();
                    let (path, filename) = self.active_doc_path_info();
                    self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
                }
                return;
            }
            if let Err(e) = self.command_registry.execute(cmd_id, &mut self.editor) {
                self.editor.status_message = Some(format!("Error: {e}"));
            }
            return;
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
            if let Err(e) = self
                .command_registry
                .execute("search.replace_all", &mut self.editor)
            {
                self.editor.status_message = Some(format!("Error: {e}"));
            } else {
                self.lsp_notify_did_change();
                let (path, filename) = self.active_doc_path_info();
                self.dispatch_plugin_hook(HookEvent::OnBufferChange { path, filename });
            }
            return;
        }

        if self.editor.search.replace_focused {
            if key.code == KeyCode::Backspace {
                if self.editor.search.replace_cursor_pos > 0 {
                    let byte_idx = char_to_byte_index(
                        &self.editor.search.replace_text,
                        self.editor.search.replace_cursor_pos - 1,
                    );
                    self.editor.search.replace_text.remove(byte_idx);
                    self.editor.search.replace_cursor_pos -= 1;
                }
                return;
            }

            if let KeyCode::Char(c) = key.code {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    let byte_idx = char_to_byte_index(
                        &self.editor.search.replace_text,
                        self.editor.search.replace_cursor_pos,
                    );
                    self.editor.search.replace_text.insert(byte_idx, c);
                    self.editor.search.replace_cursor_pos += 1;
                }
            }
        } else {
            if key.code == KeyCode::Backspace {
                if self.editor.search.cursor_pos > 0 {
                    let byte_idx = char_to_byte_index(
                        &self.editor.search.query,
                        self.editor.search.cursor_pos - 1,
                    );
                    self.editor.search.query.remove(byte_idx);
                    self.editor.search.cursor_pos -= 1;
                    rerun_search(&mut self.editor);
                }
                return;
            }

            if let KeyCode::Char(c) = key.code {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    let byte_idx = char_to_byte_index(
                        &self.editor.search.query,
                        self.editor.search.cursor_pos,
                    );
                    self.editor.search.query.insert(byte_idx, c);
                    self.editor.search.cursor_pos += 1;
                    rerun_search(&mut self.editor);
                }
            }
        }
    }

    fn handle_fuzzy_finder_key(&mut self, key: KeyEvent) {
        if let Some(cmd_id) = self.input_mapper.resolve(EditorMode::FuzzyFinder, key) {
            match cmd_id {
                "fuzzy.close" => self.editor.switch_mode(EditorMode::Normal),
                "fuzzy.up" => self.editor.fuzzy_finder.move_selection(-1),
                "fuzzy.down" => self.editor.fuzzy_finder.move_selection(1),
                _ => {}
            }
            return;
        }

        if key.code == KeyCode::Enter {
            if let Some(path) = self.editor.fuzzy_finder.selected_path() {
                let full_path = self.editor.file_explorer.root.join(path);
                self.open_file_from_overlay(&full_path);
            }
            self.editor.switch_mode(EditorMode::Normal);
            return;
        }

        if key.code == KeyCode::Backspace {
            if self.editor.fuzzy_finder.cursor_pos > 0 {
                let byte_idx = char_to_byte_index(
                    &self.editor.fuzzy_finder.query,
                    self.editor.fuzzy_finder.cursor_pos - 1,
                );
                self.editor.fuzzy_finder.query.remove(byte_idx);
                self.editor.fuzzy_finder.cursor_pos -= 1;
                self.editor.fuzzy_finder.update_filter();
            }
            return;
        }

        if let KeyCode::Char(c) = key.code {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                let byte_idx = char_to_byte_index(
                    &self.editor.fuzzy_finder.query,
                    self.editor.fuzzy_finder.cursor_pos,
                );
                self.editor.fuzzy_finder.query.insert(byte_idx, c);
                self.editor.fuzzy_finder.cursor_pos += 1;
                self.editor.fuzzy_finder.update_filter();
            }
        }
    }

    fn handle_command_palette_key(&mut self, key: KeyEvent) {
        if let Some(cmd_id) = self.input_mapper.resolve(EditorMode::CommandPalette, key) {
            match cmd_id {
                "palette.close" => {
                    self.editor.command_palette.mode = PaletteMode::Commands;
                    self.editor.switch_mode(EditorMode::Normal);
                }
                "palette.up" => self.editor.command_palette.move_selection(-1),
                "palette.down" => self.editor.command_palette.move_selection(1),
                _ => {}
            }
            return;
        }

        if key.code == KeyCode::Enter {
            match self.editor.command_palette.mode {
                PaletteMode::Commands => {
                    let cmd_id = self
                        .editor
                        .command_palette
                        .selected_command()
                        .map(|c| c.id.clone());
                    self.editor.switch_mode(EditorMode::Normal);
                    if let Some(id) = cmd_id {
                        if id == "theme.list" {
                            self.open_theme_palette();
                            return;
                        }
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
                    self.editor.switch_mode(EditorMode::Normal);
                    if let Some(name) = theme_name {
                        self.apply_theme(&name);
                    }
                }
            }
            return;
        }

        if key.code == KeyCode::Backspace {
            if self.editor.command_palette.cursor_pos > 0 {
                let byte_idx = char_to_byte_index(
                    &self.editor.command_palette.query,
                    self.editor.command_palette.cursor_pos - 1,
                );
                self.editor.command_palette.query.remove(byte_idx);
                self.editor.command_palette.cursor_pos -= 1;
                self.editor.command_palette.update_filter();
            }
            return;
        }

        if let KeyCode::Char(c) = key.code {
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                let byte_idx = char_to_byte_index(
                    &self.editor.command_palette.query,
                    self.editor.command_palette.cursor_pos,
                );
                self.editor.command_palette.query.insert(byte_idx, c);
                self.editor.command_palette.cursor_pos += 1;
                self.editor.command_palette.update_filter();
            }
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

    fn apply_theme(&mut self, name: &str) {
        let theme_path = termcode_config::default::runtime_dir()
            .join("themes")
            .join(format!("{name}.toml"));
        match termcode_theme::loader::load_theme(&theme_path) {
            Ok(theme) => {
                self.editor.switch_theme(theme);
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
                self.editor.active_view = None;
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
            if let Some(view_id) = self.editor.find_view_by_doc_id(doc_id) {
                self.editor.active_view = Some(view_id);
            }
        } else {
            match self.editor.open_file(path) {
                Ok((doc_id, _)) => self.lsp_notify_did_open(doc_id),
                Err(e) => self.editor.status_message = Some(format!("Error: {e}")),
            }
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
        if self.editor.tabs.tabs.is_empty() {
            self.editor.active_view = None;
        } else {
            self.sync_active_view_to_tab();
        }
    }

    fn sync_active_view_to_tab(&mut self) {
        use termcode_view::image::TabContent;
        if let Some(tab) = self.editor.tabs.active_tab() {
            match tab.content {
                TabContent::Document(doc_id) => {
                    if let Some(view_id) = self.editor.find_view_by_doc_id(doc_id) {
                        self.editor.active_view = Some(view_id);
                    }
                }
                TabContent::Image(_) => {
                    self.editor.active_view = None;
                }
            }
        }
    }

    fn handle_explorer_enter(&mut self) {
        let selected = self.editor.file_explorer.selected;
        if selected >= self.editor.file_explorer.tree.len() {
            return;
        }

        let kind = self.editor.file_explorer.tree[selected].kind;
        match kind {
            FileNodeKind::Directory => {
                if let Err(e) = self.editor.file_explorer.toggle_expand(selected) {
                    self.editor.status_message = Some(format!("Error: {e}"));
                }
            }
            FileNodeKind::File | FileNodeKind::Symlink => {
                let path = self.editor.file_explorer.tree[selected].path.clone();
                self.open_file_from_overlay(&path);
                self.editor.switch_mode(EditorMode::Normal);
            }
        }
    }

    fn handle_explorer_expand(&mut self) {
        let selected = self.editor.file_explorer.selected;
        if selected >= self.editor.file_explorer.tree.len() {
            return;
        }
        let node = &self.editor.file_explorer.tree[selected];
        if node.kind == FileNodeKind::Directory && !node.expanded {
            if let Err(e) = self.editor.file_explorer.toggle_expand(selected) {
                self.editor.status_message = Some(format!("Error: {e}"));
            }
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
        } else {
            let current_depth = node.depth;
            if current_depth > 0 {
                for i in (0..selected).rev() {
                    if self.editor.file_explorer.tree[i].depth < current_depth {
                        self.editor.file_explorer.selected = i;
                        let vh = self.editor.file_explorer.viewport_height;
                        self.editor.file_explorer.ensure_visible(vh);
                        break;
                    }
                }
            }
        }
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
            | "search.replace_current"
            | "search.replace_all"
    )
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

fn list_available_themes() -> Vec<String> {
    let themes_dir = termcode_config::default::runtime_dir().join("themes");
    let mut themes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&themes_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    themes.push(stem.to_string());
                }
            }
        }
    }
    themes.sort();
    themes
}

fn setup_terminal(mouse_enabled: bool) -> anyhow::Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if mouse_enabled {
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    } else {
        execute!(stdout, EnterAlternateScreen)?;
    }
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mouse_enabled: bool,
) -> anyhow::Result<()> {
    disable_raw_mode()?;
    if mouse_enabled {
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
