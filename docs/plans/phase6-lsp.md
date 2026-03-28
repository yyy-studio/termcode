# Phase 6: LSP Integration Implementation Plan

**Created**: 2026-03-27
**Analysis Report**: N/A (no formal analysis report; planning based on architecture blueprint, codebase review, and team lead specifications)
**Status**: Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-P6-01] LSP transport: JSON-RPC 2.0 framing over stdin/stdout with Content-Length headers
- [FR-P6-02] LSP client: manage a single language server process lifecycle (start, initialize, shutdown)
- [FR-P6-03] LSP registry: map language names to running LspClient instances; start servers on demand
- [FR-P6-04] Initialize handshake: send `initialize` request with workspace root, receive server capabilities
- [FR-P6-05] `textDocument/didOpen` notification on file open
- [FR-P6-06] `textDocument/didChange` notification on edits (full-document sync, debounced at 100ms)
- [FR-P6-07] `textDocument/didSave` notification on file save
- [FR-P6-08] `textDocument/publishDiagnostics` notification handling (server -> client)
- [FR-P6-09] Diagnostic display: inline error/warning squiggles, gutter severity icons, diagnostic count in status bar
- [FR-P6-10] `textDocument/completion` request and autocomplete popup UI
- [FR-P6-11] Completion popup: overlay near cursor, scrollable, accepts with Enter/Tab, dismisses with Esc
- [FR-P6-12] Completion trigger: on trigger characters (from server capabilities) or manually via Ctrl+Space
- [FR-P6-13] `textDocument/hover` request and hover popup UI
- [FR-P6-14] Hover popup: overlay near cursor showing type info / documentation
- [FR-P6-15] `textDocument/definition` request and jump-to-definition
- [FR-P6-16] Go-to-definition command: `goto.definition` (Ctrl+D or F12 in normal mode)

**Deferred to future phase**:

- Git integration (gix crate, branch in status bar, file status icons)
- Plugin system (termcode-plugin crate)
- Additional tree-sitter languages
- textDocument/references, textDocument/rename, workspace/symbol
- Incremental sync (TextDocumentSyncKind::Incremental) -- start with Full sync
- Multiple language servers per document (e.g., ESLint + TypeScript)

### Architecture Constraints (from docs/architecture/termcode.md)

- `termcode-lsp` depends on `termcode-core` + `termcode-config` only (NO dependency on view/document)
- `LspClient` API uses primitive types (uri strings, text strings, positions) -- not Document
- `LspClient` is fully async: uses `tokio::process::Command` to spawn servers, `async fn` for all I/O
- `LspRegistry` lives in `App` (termcode-term), NOT in `Editor` (termcode-view) -- avoids circular dependency
- `Diagnostic` type is defined in `termcode-core` -- usable by both view and lsp without cross-dependency
- `LspServerConfig` is defined in `termcode-config` -- lsp crate depends on config to receive it
- `LanguageId` is `Arc<str>` in termcode-syntax; LSP registry uses `String` keys (matched via string equality)
- Async: use tokio for server process I/O; bridge to synchronous event loop via channel
- TEA pattern: LSP responses arrive as `AppEvent::Lsp(LspResponse)` and are processed in the update cycle

## 2. Analysis Report Reference

### Reference Documents

- Architecture Blueprint: `docs/architecture/termcode.md`
- Project Plan: `/Users/hankyung/.claude/plans/cosmic-prancing-whisper.md`
- Phase 4 Plan (pattern reference): `docs/plans/phase4-search.md`

### Applied Recommendations (from architecture)

- Architecture specifies `client.rs`, `transport.rs`, `registry.rs`, `types.rs` under `termcode-lsp/src/`
- `LspClient::start(config)` takes `LspServerConfig` from termcode-config
- `LspClient` uses `tokio::process::Command` for non-blocking process spawn and I/O (architecture lines 1080-1093)
- `LspRegistry` keyed by language String, not LanguageId (Arc<str>) -- avoids lsp->syntax dependency
- Diagnostic display uses existing `Document.diagnostics: Vec<Diagnostic>` field (already in place)
- Error handling: LSP connection failure logs and continues without LSP (graceful degradation)
- Performance: debounce `didChange` notifications at 100ms per architecture spec

### Reusable Code

| Code                               | Location                                   | Purpose                                                             |
| ---------------------------------- | ------------------------------------------ | ------------------------------------------------------------------- |
| `Diagnostic` struct                | `crates/termcode-core/src/diagnostic.rs`   | Diagnostic data type with range, severity, message, source          |
| `DiagnosticSeverity` enum          | `crates/termcode-core/src/diagnostic.rs`   | Error/Warning/Info/Hint severity levels                             |
| `Position` struct                  | `crates/termcode-core/src/position.rs`     | Line/column position type                                           |
| `Document.diagnostics` field       | `crates/termcode-view/src/document.rs:22`  | Already-declared Vec<Diagnostic> on Document                        |
| `Document.language_id` field       | `crates/termcode-view/src/document.rs:19`  | LanguageId for matching to LSP server config                        |
| `Document.path` field              | `crates/termcode-view/src/document.rs:19`  | File path for LSP URI construction                                  |
| `EditorMode` enum                  | `crates/termcode-view/src/editor.rs:17`    | Mode-based input dispatch (add Completion variant or handle inline) |
| `AppEvent` enum                    | `crates/termcode-term/src/event.rs`        | Event type (add Lsp variant)                                        |
| `UiColors.error/warning/info/hint` | `crates/termcode-theme/src/theme.rs:23-26` | Theme colors for diagnostic severity rendering                      |
| `overlay.rs` utilities             | `crates/termcode-term/src/ui/overlay.rs`   | Shared overlay rendering (frame, input line, list items)            |
| `CommandRegistry`                  | `crates/termcode-term/src/command.rs`      | Register goto.definition, lsp.hover, lsp.completion commands        |
| `InputMapper`                      | `crates/termcode-term/src/input.rs`        | Add keybindings for LSP commands                                    |

### Constraints

- The current event loop is synchronous (crossterm polling with tick rate). LSP requires async I/O. Must introduce a channel-based bridge: tokio runtime runs in background thread, sends AppEvent::Lsp to the main loop.
- `AppConfig` in termcode-config currently has no `lsp` field -- must add `LspServerConfig` struct and `lsp: Vec<LspServerConfig>` to AppConfig.
- `lsp-types` crate (0.97) is referenced in architecture but NOT yet in workspace `Cargo.toml` -- must add it.
- The current `App` does not use tokio -- must add tokio dependency to termcode-term and set up a runtime.
- `EditorMode` changes may be needed: completion can work as an inline overlay (like search highlights) without a dedicated mode, or use a new mode. Plan uses inline state approach on Editor to avoid mode explosion.

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                         | Risk   | Description                                                                                                                                                                                                                                             |
| -------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/termcode-lsp/src/transport.rs`       | Low    | Async JSON-RPC message framing (Content-Length header read/write) over tokio AsyncRead/AsyncWrite                                                                                                                                                       |
| `crates/termcode-lsp/src/client.rs`          | Low    | Async LspClient: tokio::process spawn, async request/response/notification                                                                                                                                                                              |
| `crates/termcode-lsp/src/registry.rs`        | Low    | LspRegistry: map language -> running LspClient                                                                                                                                                                                                          |
| `crates/termcode-lsp/src/types.rs`           | Low    | Type conversions between lsp-types and termcode-core types                                                                                                                                                                                              |
| `crates/termcode-term/src/ui/completion.rs`  | Low    | Autocomplete popup widget                                                                                                                                                                                                                               |
| `crates/termcode-term/src/ui/hover.rs`       | Low    | Hover info popup widget                                                                                                                                                                                                                                 |
| `crates/termcode-term/src/ui/diagnostics.rs` | Low    | Diagnostic gutter icons and inline markers rendering helpers                                                                                                                                                                                            |
| `crates/termcode-term/src/lsp_bridge.rs`     | Medium | Bridge between async LspRegistry and sync event loop. **Deadlock risk**: must never hold a lock while awaiting a channel send; must ensure mpsc sender cannot block if receiver is not draining. Use `try_send` or unbounded channel where appropriate. |

### Files to Modify

| File                                         | Risk   | Description                                                                    |
| -------------------------------------------- | ------ | ------------------------------------------------------------------------------ |
| `Cargo.toml` (workspace root)                | Medium | Add `lsp-types = "0.97"` to workspace dependencies                             |
| `crates/termcode-lsp/Cargo.toml`             | Medium | Add dependencies: lsp-types, tokio, serde_json, serde, log, thiserror          |
| `crates/termcode-lsp/src/lib.rs`             | Medium | Export client, transport, registry, types modules                              |
| `crates/termcode-term/Cargo.toml`            | Medium | Add dependencies: termcode-lsp, tokio, serde_json                              |
| `crates/termcode-term/src/lib.rs`            | Low    | Add `pub mod lsp_bridge;`                                                      |
| `crates/termcode-term/src/event.rs`          | Medium | Add `AppEvent::Lsp(LspResponse)` variant and LspResponse type                  |
| `crates/termcode-term/src/app.rs`            | High   | Add LspRegistry, tokio runtime, LSP lifecycle management, dual-poll event loop |
| `crates/termcode-term/src/command.rs`        | Medium | Add LSP commands: goto.definition, lsp.hover, lsp.trigger_completion           |
| `crates/termcode-term/src/input.rs`          | Medium | Add keybindings for LSP commands (Ctrl+Space, F12/Ctrl+D, Ctrl+K Ctrl+I)       |
| `crates/termcode-term/src/render.rs`         | Medium | Render completion/hover popups, pass diagnostic info to editor view            |
| `crates/termcode-term/src/ui/mod.rs`         | Low    | Add completion, hover, diagnostics modules                                     |
| `crates/termcode-term/src/ui/editor_view.rs` | Medium | Add diagnostic rendering (gutter icons, inline markers/underlines)             |
| `crates/termcode-term/src/ui/status_bar.rs`  | Low    | Add diagnostic counts (errors/warnings) to status bar                          |
| `crates/termcode-view/src/editor.rs`         | Medium | Add CompletionState and HoverState fields to Editor                            |
| `crates/termcode-config/src/config.rs`       | Medium | Add `LspServerConfig` struct and `lsp: Vec<LspServerConfig>` to AppConfig      |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/phase6-lsp`
- No database changes; no external system modifications
- All changes are additive (new files, new fields) with no breaking changes to existing APIs

## 4. Implementation Order

### Phase 6.1: Foundation -- Async Transport and Client

**Goal**: Implement the async JSON-RPC transport layer and LspClient that can start a language server via `tokio::process::Command`, perform the initialize handshake, and shut down cleanly. All I/O is non-blocking and async.
**Risk**: Low
**Status**: Complete

- [x] Task 1.1: Add `lsp-types = "0.97"` to workspace `Cargo.toml` `[workspace.dependencies]`
- [x] Task 1.2: Update `crates/termcode-lsp/Cargo.toml` -- add dependencies: `lsp-types`, `tokio`, `serde_json`, `serde`, `log`, `thiserror`
- [x] Task 1.3: Add `LspServerConfig` struct to `crates/termcode-config/src/config.rs` with fields: `language: String`, `command: String`, `args: Vec<String>`. Add `lsp: Vec<LspServerConfig>` to `AppConfig` (with `#[serde(default)]`)
- [x] Task 1.4: Implement `crates/termcode-lsp/src/transport.rs`:
  - `LspTransport` struct wrapping `tokio::io::BufReader<tokio::process::ChildStdout>` (read) and `tokio::io::BufWriter<tokio::process::ChildStdin>` (write)
  - `async fn read_message(&mut self) -> Result<serde_json::Value>`: async read `Content-Length: N\r\n\r\n` header via `AsyncBufReadExt::read_line`, async read N bytes via `AsyncReadExt::read_exact`, parse JSON
  - `async fn write_message(&mut self, msg: &serde_json::Value) -> Result<()>`: serialize JSON, prepend Content-Length header, `AsyncWriteExt::write_all` + `flush().await`
  - Unit tests with `tokio::io::duplex` (in-memory async byte streams) to verify framing round-trip
- [x] Task 1.5: Implement `crates/termcode-lsp/src/types.rs`:
  - `fn position_to_lsp(pos: &Position) -> lsp_types::Position` (line/column conversion)
  - `fn lsp_to_position(pos: &lsp_types::Position) -> Position`
  - `fn diagnostic_from_lsp(diag: &lsp_types::Diagnostic) -> Diagnostic` (map severity, range, message, source)
  - `fn path_to_uri(path: &Path) -> lsp_types::Url`
  - `fn uri_to_path(uri: &lsp_types::Url) -> Option<PathBuf>`
- [x] Task 1.6: Implement `crates/termcode-lsp/src/client.rs`:
  - `LspClient` struct: `process: tokio::process::Child`, `transport: LspTransport`, `next_request_id: i64`, `server_capabilities: Option<ServerCapabilities>`, `pending_requests: HashMap<i64, tokio::sync::oneshot::Sender<serde_json::Value>>`, `notification_tx: mpsc::UnboundedSender<serde_json::Value>` (for forwarding server-initiated notifications)
  - `async fn start(config: &LspServerConfig) -> Result<Self>`: spawn process via `tokio::process::Command::new(&config.command).args(&config.args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn()`. Split stdin/stdout into LspTransport.
  - `async fn initialize(&mut self, root_uri: &str) -> Result<InitializeResult>`: send initialize request, await response via oneshot, send `initialized` notification
  - `async fn shutdown(&mut self) -> Result<()>`: send shutdown request, await response, send exit notification, `self.process.wait().await`
  - `async fn send_request<R: lsp_types::request::Request>(&mut self, params: R::Params) -> Result<R::Result>`: assign next id, create `oneshot::channel`, insert sender into `pending_requests`, write JSON-RPC request message via transport, await oneshot receiver for response, deserialize `R::Result`
  - `async fn send_notification<N: lsp_types::notification::Notification>(&mut self, params: N::Params) -> Result<()>`: write JSON-RPC notification (no id) via transport
  - `async fn notify_did_open(&mut self, uri: &str, language_id: &str, version: i32, text: &str) -> Result<()>`: send `textDocument/didOpen` notification
  - `async fn notify_did_change(&mut self, uri: &str, version: i32, text: &str) -> Result<()>`: send `textDocument/didChange` notification (full document text)
  - `async fn notify_did_save(&mut self, uri: &str) -> Result<()>`: send `textDocument/didSave` notification
  - `async fn notify_did_close(&mut self, uri: &str) -> Result<()>`: send `textDocument/didClose` notification (prevents server-side memory leaks)
  - Spawn a background reader task (`tokio::spawn`) that continuously reads messages from the transport reader half. For each message: if it has an `id` field matching a pending request, resolve the corresponding oneshot sender. If it is a notification (no id), forward via `notification_tx` to the bridge layer for dispatch.
- [x] Task 1.7: Update `crates/termcode-lsp/src/lib.rs` to export all modules
- [x] Task 1.8: Write integration tests for transport (use `tokio::io::duplex` for mock async streams; `#[tokio::test]`)

**Quality Gate**:

- `cargo build -p termcode-lsp` compiles
- `cargo clippy -p termcode-lsp` -- 0 warnings
- Unit tests for async transport message framing pass

### Phase 6.2: Registry and Event Bridge

**Goal**: Implement `LspRegistry` to manage multiple language servers, and the async-to-sync bridge that connects LspRegistry (tokio) to the synchronous App event loop.
**Risk**: Medium
**Status**: Complete

- [x] Task 2.1: Implement `crates/termcode-lsp/src/registry.rs`:
  - `LspRegistry` struct: `clients: HashMap<String, LspClient>`, `configs: HashMap<String, LspServerConfig>`
  - `LspRegistry::new(configs: Vec<LspServerConfig>) -> Self`: store configs indexed by language
  - `async fn start_for_language(&mut self, language: &str, root_uri: &str) -> Result<()>`: if not running, start LspClient from config and initialize
  - `fn get(&self, language: &str) -> Option<&LspClient>`
  - `fn get_mut(&mut self, language: &str) -> Option<&mut LspClient>`
  - `async fn shutdown_all(&mut self)`: shut down all running clients
  - `fn has_server(&self, language: &str) -> bool`
- [x] Task 2.2: Define `LspResponse` enum for async event delivery:
  - `Diagnostics { uri: String, diagnostics: Vec<Diagnostic> }`
  - `Completion { items: Vec<CompletionItem> }` (using a simplified CompletionItem defined in types.rs)
  - `Hover { contents: String }`
  - `Definition { uri: String, position: Position }`
  - `ServerStarted { language: String }`
  - `ServerError { language: String, error: String }`
- [x] Task 2.3: Update `crates/termcode-term/src/event.rs`: add `Lsp(LspResponse)` variant to `AppEvent`
- [x] Task 2.4: Update `crates/termcode-term/Cargo.toml`: add `termcode-lsp`, `tokio`, `serde_json` dependencies
- [x] Task 2.5: Implement `crates/termcode-term/src/lsp_bridge.rs`:
  - `LspBridge` struct: holds `tokio::runtime::Runtime`, `LspRegistry` (behind `Arc<tokio::sync::Mutex<_>>`), `mpsc::UnboundedSender<AppEvent>`
  - `LspBridge::new(configs: Vec<LspServerConfig>, event_tx: mpsc::UnboundedSender<AppEvent>) -> Self`: create tokio runtime, init registry
  - `LspBridge::start_server(&self, language: &str, root_uri: &str)`: `runtime.spawn` an async task that acquires the registry lock, calls `start_for_language`, sends `ServerStarted` or `ServerError` via `event_tx`
  - `LspBridge::notify_did_open(...)`, `notify_did_change(...)`, `notify_did_save(...)`, `notify_did_close(...)`: `runtime.spawn` async tasks that acquire registry lock and call the corresponding client method
  - `LspBridge::request_completion(...)`, `request_hover(...)`, `request_definition(...)`: `runtime.spawn` async tasks that acquire registry lock, call the client method, send `LspResponse` results via `event_tx`
  - Debounce didChange notifications: use `tokio::time::sleep(Duration::from_millis(100))` with a `tokio::sync::Notify` or `AbortHandle` to cancel pending debounce on new change
  - Subscribe to each LspClient's `notification_tx` receiver: spawn a forwarding task that converts server notifications (e.g., publishDiagnostics) into `AppEvent::Lsp(LspResponse::Diagnostics)` and sends via `event_tx`
  - **Deadlock avoidance rules**:
    - Use `mpsc::UnboundedSender` (not bounded) for `event_tx` so sends never block, even if the main thread is busy rendering
    - Never hold the `tokio::sync::Mutex` on `LspRegistry` across an `.await` on channel send -- acquire lock, do I/O, drop lock, then send result
    - If bounded channels are ever needed (e.g., for backpressure), use `try_send` and log/drop on failure rather than blocking
- [x] Task 2.6: Update `crates/termcode-term/src/lib.rs`: add `pub mod lsp_bridge;`

**Quality Gate**:

- `cargo build -p termcode-term` compiles with new dependencies
- `cargo clippy -p termcode-term` -- 0 warnings
- LspBridge can be constructed and does not panic

### Phase 6.3: App Integration -- Event Loop Refactor, Lifecycle, and Notifications

**Goal**: Refactor `App::run()` to support dual event sources (crossterm + LSP channel), wire LspBridge into App, start language servers on file open, and send didOpen/didChange/didSave/didClose notifications.
**Risk**: High
**Status**: Complete

- [x] Task 3.0: Refactor `App::run()` event loop for dual-source polling:
  - The current loop uses `crossterm::event::poll(tick_rate)` which blocks the thread for up to `tick_rate` duration, preventing LSP events from being processed promptly.
  - **New dual-polling approach**: Replace the single `EventHandler::next()` call with a select-style loop:
    1. Store `lsp_event_rx: mpsc::UnboundedReceiver<AppEvent>` on App (from LspBridge).
    2. In each iteration of the event loop, first drain all pending LSP events from `lsp_event_rx` via `try_recv()` in a loop (non-blocking), processing each `AppEvent::Lsp` immediately.
    3. Then call `crossterm::event::poll(tick_rate)` as before for terminal input.
    4. This ensures LSP responses (diagnostics, completion results) are processed on every frame without waiting for user input.
  - Alternative considered: convert the entire event loop to `tokio::select!` over crossterm async events + mpsc receiver. Rejected because crossterm's async feature adds complexity and the try_recv drain approach is simpler and sufficient given the 50ms tick rate.
  - Update `App::update()` to handle the new `AppEvent::Lsp(response)` variant by dispatching to a new `handle_lsp_response(&mut self, response: LspResponse)` method.
- [x] Task 3.1: Modify `App::new()` in `crates/termcode-term/src/app.rs`:
  - Accept or load `AppConfig` (load `LspServerConfig` list from config or use defaults)
  - Create `mpsc::unbounded_channel` for async event delivery
  - Construct `LspBridge` with configs and sender
  - Store `lsp_bridge: Option<LspBridge>` and `lsp_event_rx: mpsc::UnboundedReceiver<AppEvent>` on App
- [x] Task 3.2: Implement `App::handle_lsp_response()`:
  - On `LspResponse::Diagnostics { uri, diagnostics }`: find document by URI path, replace `doc.diagnostics` with the new diagnostics list
  - On `LspResponse::Completion { items }`: populate `editor.completion.items`, set `visible = true`
  - On `LspResponse::Hover { contents }`: populate `editor.hover.content`, set `visible = true`
  - On `LspResponse::Definition { uri, position }`: if same file, move cursor; if different file, `open_file` and navigate
  - On `LspResponse::ServerStarted { language }`: set status message "LSP: {language} server started"
  - On `LspResponse::ServerError { language, error }`: set status message "LSP error ({language}): {error}"
- [x] Task 3.3: Hook LSP notifications into editor lifecycle:
  - After `editor.open_file(path)`: if document has a `language_id` matching an LSP config, call `lsp_bridge.start_server()` then `lsp_bridge.notify_did_open(uri, language_id, version=1, text)`
  - After `insert_char` / edit commands: call `lsp_bridge.notify_did_change(uri, version, full_text)` (debounced)
  - After `file.save` command: call `lsp_bridge.notify_did_save(uri)`
  - After `close_document`: call `lsp_bridge.notify_did_close(uri)` (sends textDocument/didClose to prevent server memory leaks)
- [x] Task 3.4: Add document version tracking to `Document`:
  - Add `pub version: i32` field to Document (increment on each edit for LSP versioning)
  - Increment in `apply_transaction()`
- [x] Task 3.5: Add completion state to `Editor`:
  - Add `CompletionState` struct to `crates/termcode-view/src/editor.rs` (or new file `crates/termcode-view/src/completion.rs`):
    - `visible: bool`
    - `items: Vec<CompletionItem>` (simplified: label, detail, insert_text)
    - `selected: usize`
    - `trigger_position: Position` (where completion was triggered)
  - Add `pub completion: CompletionState` to Editor
- [x] Task 3.6: Add hover state to `Editor`:
  - `HoverState` struct: `visible: bool`, `content: String`, `position: Position` (where to anchor popup)
  - Add `pub hover: HoverState` to Editor

**Quality Gate**:

- `cargo build` (full workspace) compiles
- `cargo clippy` -- 0 warnings
- App launches without crash (even if no LSP servers configured)
- Status message shows "Language server started" or "No LSP server for <lang>" on file open
- LSP events are processed without blocking terminal input

### Phase 6.4: Diagnostics Display

**Goal**: Render diagnostics from LSP in the editor -- gutter icons per severity, inline underlines, and error/warning count in the status bar.
**Risk**: Medium
**Status**: Complete

- [x] Task 4.1: Implement diagnostic rendering in `crates/termcode-term/src/ui/editor_view.rs`:
  - In the line rendering loop, after syntax highlighting:
    - Check `doc.diagnostics` for any diagnostic whose range overlaps the current line
    - Render gutter icon: `E` (error color) / `W` (warning color) / `I` (info color) / `H` (hint color) in the gutter margin (1 char before line numbers, or using the existing separator column)
    - For inline display: underline characters in the diagnostic range using the appropriate severity color
  - Use `theme.ui.error`, `theme.ui.warning`, `theme.ui.info`, `theme.ui.hint` colors
- [x] Task 4.2: Update `crates/termcode-term/src/ui/status_bar.rs`:
  - After mode indicator, show diagnostic summary: e.g., `E:2 W:5` using error/warning colors
  - Count diagnostics from active document's diagnostics vector
  - Only show counts > 0
- [x] Task 4.3: Add diagnostic navigation commands:
  - `diagnostic.next`: jump cursor to next diagnostic location
  - `diagnostic.prev`: jump cursor to previous diagnostic location
  - Register in `CommandRegistry` and add keybindings (`]d` / `[d` in normal mode, matching Helix/Vim convention)

**Quality Gate**:

- Diagnostics render correctly in gutter and inline
- Status bar shows error/warning counts
- `diagnostic.next`/`diagnostic.prev` navigate between diagnostics
- `cargo test` passes

### Phase 6.5: Autocomplete

**Goal**: Trigger completion requests, display completion popup, and insert selected completion items.
**Risk**: Medium
**Status**: Complete

- [x] Task 5.1: Implement completion trigger logic in `crates/termcode-term/src/app.rs`:
  - After character insertion in Insert mode, check if char is a trigger character (from server capabilities) or if Ctrl+Space was pressed
  - Call `lsp_bridge.request_completion(uri, position)` to request completions
  - On `LspResponse::Completion`: populate `editor.completion.items`, set `visible = true`, set `trigger_position`
- [x] Task 5.2: Implement completion popup widget `crates/termcode-term/src/ui/completion.rs`:
  - Render as a floating popup anchored below/above the cursor position
  - Show list of completion labels with the selected item highlighted
  - Show detail/documentation for selected item if available (secondary text)
  - Popup dimensions: max 10 items visible, width fits longest label (capped at 40 chars)
  - Use overlay utilities from `overlay.rs` for background/border rendering
- [x] Task 5.3: Handle completion input in `App::handle_key()`:
  - When `editor.completion.visible`:
    - Up/Down (or Ctrl+P/Ctrl+N): move selection
    - Enter/Tab: accept selected completion (insert text at trigger position)
    - Esc: dismiss popup
    - Any other character: re-trigger completion with new prefix (or dismiss if no match)
  - Apply completion: construct a `Transaction` that replaces the prefix (from trigger_position to cursor) with the completion's `insert_text`
- [x] Task 5.4: Render completion popup in `crates/termcode-term/src/render.rs`:
  - After rendering editor view, if `editor.completion.visible`, render completion widget on top
- [x] Task 5.5: Dismiss completion on mode change or cursor movement away from trigger position

**Quality Gate**:

- Completion popup appears on Ctrl+Space
- Completion popup appears on trigger characters (e.g., `.` for Rust)
- Arrow keys navigate the list
- Enter/Tab inserts the selected completion
- Esc dismisses
- `cargo clippy` -- 0 warnings

### Phase 6.6: Hover and Go-to-Definition

**Goal**: Show hover information popup and implement jump-to-definition navigation.
**Risk**: Low
**Status**: Complete

- [x] Task 6.1: Implement hover request and display:
  - Add `lsp.hover` command: requests hover at current cursor position
  - On `LspResponse::Hover`: populate `editor.hover` with content and position, set `visible = true`
  - Keybinding: `K` in normal mode (like Vim), or `Ctrl+K Ctrl+I` (like VS Code)
  - Dismiss hover on any key press or cursor movement
- [x] Task 6.2: Implement hover popup widget `crates/termcode-term/src/ui/hover.rs`:
  - Render as floating popup anchored above cursor position
  - Display hover content (strip markdown formatting for plain text display)
  - Dimensions: width fits content (capped at 60 chars), height wraps text (capped at 10 lines)
  - Use overlay utilities for border rendering
- [x] Task 6.3: Render hover popup in `crates/termcode-term/src/render.rs`:
  - After rendering editor view and completion popup, if `editor.hover.visible`, render hover widget
- [x] Task 6.4: Implement go-to-definition:
  - Add `goto.definition` command: requests definition at current cursor position
  - On `LspResponse::Definition`: if same file, move cursor to position; if different file, open file and navigate
  - Keybinding: `gd` in normal mode (Vim style), or `F12` (VS Code style)
  - Register in `CommandRegistry` and `InputMapper`
- [x] Task 6.5: Add `lsp.trigger_completion` command registered in `CommandRegistry`:
  - Keybinding: `Ctrl+Space` (global in insert mode)
  - Manually triggers completion request

**Quality Gate**:

- Hover popup shows type information on `K` or `Ctrl+K Ctrl+I`
- Go-to-definition opens correct file at correct position
- `cargo build && cargo clippy` -- 0 warnings
- `cargo test` passes

### Phase 6.7: Polish and Testing

**Goal**: End-to-end testing, error handling hardening, cleanup.
**Risk**: Low
**Status**: Complete

- [x] Task 7.1: Error handling hardening:
  - LSP server crash: detect process exit, remove from registry, show status message, continue without LSP
  - LSP server not found: log warning, show status message "Language server '<cmd>' not found", continue
  - Timeout on LSP requests: 5s timeout, cancel and show status message
  - Invalid JSON-RPC responses: log and skip
- [x] Task 7.2: Add LSP-related integration tests:
  - Test transport framing with mock async streams (`tokio::io::duplex`)
  - Test type conversions (position, diagnostic, URI)
  - Test LspRegistry start/shutdown lifecycle
  - Test completion state management
- [x] Task 7.3: Documentation:
  - Update CLAUDE.md if needed with new module descriptions
  - Add `[[lsp]]` configuration examples to default config
- [x] Task 7.4: Final cleanup:
  - Ensure all new modules follow existing code patterns
  - Remove any debug logging
  - Verify `cargo fmt` compliance

**Quality Gate**:

- `cargo build` succeeds
- `cargo clippy` -- 0 warnings
- `cargo fmt --check` passes
- All tests pass (existing + new)
- App launches and works correctly with and without LSP servers configured
- Graceful degradation when LSP server is not available

## 5. Quality Gate

- [x] Build success: `cargo build`
- [x] Tests pass: `cargo test`
- [x] Lint pass: `cargo clippy -- -D warnings`
- [x] Format: `cargo fmt --check`
- [ ] Manual test: open a Rust file with rust-analyzer installed, verify completion, hover, diagnostics, go-to-definition

## 6. Notes

### Key Design Decisions

1. **Full document sync (not incremental)**: Start with `TextDocumentSyncKind::Full` for didChange notifications. This sends the entire document text on every change. Simpler to implement; incremental sync can be added later as an optimization.

2. **Async bridge pattern**: The main event loop remains synchronous (crossterm polling). A background tokio runtime manages LSP server I/O. Communication is via `mpsc::unbounded_channel<AppEvent>`. The main loop drains LSP events via `try_recv()` on each frame iteration, avoiding the need to convert the entire app to async.

3. **Dual-source event loop**: Rather than converting to a fully async event loop with `tokio::select!`, we use a simple drain-then-poll approach: drain all pending LSP events via `try_recv()`, then poll crossterm with the existing tick rate. This is simpler and sufficient given the 50ms tick cadence.

4. **Completion without dedicated EditorMode**: Rather than adding `EditorMode::Completion`, completion state is tracked as `CompletionState` on Editor and handled inline during Insert mode key processing. This avoids mode explosion and matches how VS Code handles completion (it's an overlay, not a mode).

5. **Hover as ephemeral popup**: Hover state is simple `visible: bool` + `content: String`. Any keypress or cursor movement dismisses it. No need for a dedicated mode.

6. **LspServerConfig location**: Defined in `termcode-config` per architecture spec. The lsp crate depends on config (not the other way around). This keeps the dependency graph clean.

7. **Document version tracking**: LSP requires monotonically increasing version numbers for `VersionedTextDocumentIdentifier`. Adding `version: i32` to `Document` is the simplest approach.

8. **didClose is mandatory**: LSP servers track open documents and allocate resources for them. Without `textDocument/didClose`, servers leak memory for every closed buffer. LspClient must expose `notify_did_close` and App must call it on document close.

### Patterns to Avoid

- Do NOT add `termcode-lsp` as a dependency of `termcode-view` -- this would create a circular dependency
- Do NOT store `LspClient` references in `Document` -- the LSP layer is owned by `App`
- Do NOT block the main event loop on LSP requests -- all requests must be async
- Do NOT use `lsp_types` types in `termcode-view` -- convert to core types at the bridge boundary
- Do NOT hold the LspRegistry mutex across `.await` on channel sends -- this risks deadlock
- Do NOT use synchronous `std::process::Command` or blocking I/O in LspClient -- all I/O must go through tokio

## 7. Implementation Notes

### Phase 6.1 (2026-03-27)

- Created: 3 files (transport.rs, types.rs, client.rs)
- Modified: 4 files (workspace Cargo.toml, lsp Cargo.toml, lsp lib.rs, config.rs)
- Risk: Low
- Notes: lsp-types 0.97 uses `Uri` (not `Url`), backed by `fluent_uri`. Added `parse_uri` helper for convenience.

### Phase 6.2 (2026-03-27)

- Created: 2 files (registry.rs, lsp_bridge.rs)
- Modified: 5 files (lsp lib.rs, types.rs, term Cargo.toml, event.rs, term lib.rs)
- Risk: Medium
- Notes: LspBridge uses Arc<tokio::sync::Mutex> for registry access. Debounce implemented with AbortHandle.

### Phase 6.3 (2026-03-27)

- Created: 0 files
- Modified: 3 files (app.rs, editor.rs, document.rs)
- Risk: High
- Notes: Drain-then-poll pattern for dual-source event loop. CompletionState/HoverState defined in termcode-view to avoid lsp dependency.

### Phase 6.4 (2026-03-27)

- Created: 0 files
- Modified: 4 files (editor_view.rs, status_bar.rs, command.rs, input.rs)
- Risk: Medium
- Notes: Diagnostic gutter icons in separator column. `]`/`[` keys for navigation (no chord support yet).

### Phase 6.5 (2026-03-27)

- Created: 1 file (completion.rs widget)
- Modified: 3 files (app.rs, render.rs, ui/mod.rs)
- Risk: Medium
- Notes: Completion popup anchored near cursor. Accept via Transaction::replace. Trigger on common chars (`.`, `:`, etc.) and Ctrl+Space.

### Phase 6.6 (2026-03-27)

- Created: 1 file (hover.rs widget)
- Modified: 2 files (app.rs, ui/mod.rs)
- Risk: Low
- Notes: Hover via Shift+K in normal mode. Go-to-definition via F12. Both use App-level key handling since they need LspBridge.

### Phase 6.7 (2026-03-27)

- Created: 0 files
- Modified: 3 files (client.rs timeout, registry.rs tests, config.toml examples)
- Risk: Low
- Notes: 5s request timeout. 14 new LSP tests. All quality gates passed.

---

Last Updated: 2026-03-27
Status: Complete (Phase 6.1-6.7)

### Potential Issues

- **Message interleaving**: While waiting for a response, the server may send notifications (e.g., publishDiagnostics). The background reader task dispatches responses to oneshot channels and notifications to the bridge layer, keeping them separated.
- **Process cleanup**: If the app crashes, spawned language server processes may become orphaned. Solution: use `kill_on_drop(true)` on `tokio::process::Command`.
- **Large completion lists**: Some servers return hundreds of completion items. Solution: cap displayed items and implement virtual scrolling in the popup.
- **Deadlock in LspBridge**: The bridge holds an `Arc<tokio::sync::Mutex<LspRegistry>>`. If a task holds this lock and tries to send on a bounded channel while the receiver (main thread) is also trying to acquire the lock, deadlock occurs. Mitigated by: (a) using unbounded sender so sends never block, (b) never holding the mutex across channel sends.

## 7. Implementation Notes

(To be filled by the implementing agent while working)

---

Last Updated: 2026-03-27
Status: Pending
