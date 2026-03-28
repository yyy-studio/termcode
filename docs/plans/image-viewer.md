# Image Viewer Implementation Plan

**Created**: 2026-03-29
**Specification**: docs/specs/image-viewer.md
**Analysis Report**: N/A (specification includes detailed code references and technical design)
**Status**: Complete

## 1. Requirements Summary

### Functional Requirements

- [FR-IMG-001] Image file detection by extension (case-insensitive) at the App level
- [FR-IMG-002] `TabContent` enum to distinguish document tabs from image tabs
- [FR-IMG-003] `ImageId` and `ImageEntry` frontend-agnostic metadata types in termcode-view
- [FR-IMG-004] Editor image state management (open, close, active image)
- [FR-IMG-005] Image rendering with `ratatui-image` crate (auto protocol detection, Halfblock fallback)
- [FR-IMG-006] Render dispatch branching between editor view and image view
- [FR-IMG-007] Read-only behavior for image tabs (no editing commands)
- [FR-IMG-008] Duplicate open prevention (switch to existing tab)

### Dependencies (New Crate Dependencies)

- `ratatui-image` with `crossterm` feature in `termcode-term`
- `image` crate (decode only, minimal features) in `termcode-term`

## 2. Specification Reference

### Applied Recommendations

- `TabContent` enum replaces `Tab.doc_id` field -- single structural change to termcode-view's tab model
- `ImageId` / `ImageEntry` are pure data types in termcode-view (no image crate dependency)
- Image decoding and `ratatui-image` protocol objects live exclusively in termcode-term (Layer 3)
- `Picker` created once during `App::new()`, cached for reuse
- Decoded images cached in `App` as `HashMap<ImageId, Box<dyn Protocol>>`

### Reusable Code

| Code                       | Location                                     | Purpose                                   |
| -------------------------- | -------------------------------------------- | ----------------------------------------- |
| `DocumentId` pattern       | `crates/termcode-view/src/document.rs:13`    | Model for `ImageId` (newtype usize)       |
| `TabManager` methods       | `crates/termcode-view/src/tab.rs`            | Extend with `TabContent` variant matching |
| `Editor::open_file()`      | `crates/termcode-view/src/editor.rs:121`     | Model for `open_image()` flow             |
| `EditorViewWidget`         | `crates/termcode-term/src/ui/editor_view.rs` | Model for `ImageViewWidget` structure     |
| `open_file_from_overlay()` | `crates/termcode-term/src/app.rs:762`        | Model for duplicate-open prevention       |

### Constraints

- `termcode-view` must NOT depend on `ratatui`, `image`, or `ratatui-image`
- `Document` and `Buffer` structs are not modified
- Existing text editing functionality must be unaffected
- The `TabContent` enum is the only structural change to termcode-view's tab model

## 3. Impact Analysis (Dry-Run)

### Overall Risk: Medium

### Files to Create

| File                                        | Risk | Description                                 |
| ------------------------------------------- | ---- | ------------------------------------------- |
| `crates/termcode-view/src/image.rs`         | Low  | `ImageId`, `ImageEntry`, `TabContent` types |
| `crates/termcode-term/src/ui/image_view.rs` | Low  | `ImageViewWidget` using `ratatui-image`     |

### Files to Modify

| File                                        | Risk   | Description                                                                                                                                                                                                                                            |
| ------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/termcode-view/src/lib.rs`           | Low    | Add `pub mod image;`                                                                                                                                                                                                                                   |
| `crates/termcode-view/src/tab.rs`           | High   | Replace `doc_id: DocumentId` with `content: TabContent`, update all methods                                                                                                                                                                            |
| `crates/termcode-view/src/editor.rs`        | High   | Add `images` map, `open_image()`, `active_image()`, `close_image()`; update `sync_tab_modified()` to handle `TabContent`                                                                                                                               |
| `crates/termcode-term/Cargo.toml`           | Medium | Add `ratatui-image` and `image` dependencies                                                                                                                                                                                                           |
| `Cargo.toml` (workspace)                    | Medium | Add workspace dependency entries for `ratatui-image` and `image`                                                                                                                                                                                       |
| `crates/termcode-term/src/app.rs`           | High   | Add image detection in `open_file()`, `open_file_from_overlay()`, `handle_close_tab()`, `sync_active_view_to_tab()`, `save_session()`; add `image_picker` and `image_cache` fields; update all `tab.doc_id` accesses to pattern-match on `tab.content` |
| `crates/termcode-term/src/render.rs`        | Medium | Branch rendering between `EditorViewWidget` and `ImageViewWidget` based on active tab content                                                                                                                                                          |
| `crates/termcode-term/src/command.rs`       | Medium | Update `cmd_tab_next()`, `cmd_tab_prev()` to handle `TabContent::Image` (set `active_view = None`)                                                                                                                                                     |
| `crates/termcode-term/src/mouse.rs`         | Medium | Guard `doc_id` access via `active_view()` (already returns Option, but tab click switching needs `TabContent` match)                                                                                                                                   |
| `crates/termcode-term/src/ui/mod.rs`        | Low    | Add `pub mod image_view;`                                                                                                                                                                                                                              |
| `crates/termcode-term/src/ui/status_bar.rs` | Low    | Display image info (filename, format, file size) when no doc/view is active but an image tab is                                                                                                                                                        |

### Files to Delete

| File | Risk | Description |
| ---- | ---- | ----------- |
| None | -    | -           |

### Destructive Operations

- None. This is purely additive with one refactor (`Tab.doc_id` -> `Tab.content`).

### Rollback Plan

- Full rollback via branch deletion: `git checkout main && git branch -D feature/image-viewer`
- No database or persistent state changes
- The `Tab.doc_id` -> `Tab.content` refactor is the main risk; if compilation breaks, all call sites are identified in the grep results above (19 occurrences across 6 files)

## 4. Implementation Order

### Phase 1: Data Types and Tab Refactor (termcode-view)

**Goal**: Introduce `ImageId`, `ImageEntry`, `TabContent` types and refactor `Tab` to use `TabContent`.
**Risk**: High (modifying core data structure used across entire codebase)
**Status**: Complete

- [x] Task 1.1: Create `crates/termcode-view/src/image.rs` with `ImageId`, `ImageEntry`, `TabContent` enum
- [x] Task 1.2: Add `pub mod image;` to `crates/termcode-view/src/lib.rs`
- [x] Task 1.3: Refactor `Tab` struct: replace `doc_id: DocumentId` with `content: TabContent`
- [x] Task 1.4: Update `TabManager::add()` to accept `TabContent` (or split into `add_document()` / `add_image()`)
- [x] Task 1.5: Update `TabManager::find_by_doc_id()` to pattern-match on `TabContent::Document`
- [x] Task 1.6: Add `TabManager::find_by_image_id()` method
- [x] Task 1.7: Update `TabManager::remove_by_doc_id()` to match variant; add `remove_by_image_id()`
- [x] Task 1.8: Update `Editor::open_file()` to use `TabContent::Document(doc_id)` when adding tab
- [x] Task 1.9: Update `Editor::sync_tab_modified()` to pattern-match tab content (skip image tabs)
- [x] Task 1.10: Update `Editor::close_document()` to match on `TabContent::Document`
- [x] Task 1.11: Add `images: HashMap<ImageId, ImageEntry>`, `next_image_id: usize` to `Editor`
- [x] Task 1.12: Implement `Editor::open_image()`, `Editor::active_image()`, `Editor::close_image()`
- [x] Task 1.13: Verify `cargo build -p termcode-view` compiles

**Checkpoint**: `cargo build -p termcode-view` succeeds.

### Phase 2: Fix All Downstream Compilation (termcode-term)

**Goal**: Update all `tab.doc_id` usages in termcode-term to use `tab.content` pattern matching. No new functionality yet -- just restore compilation.
**Risk**: High (touching app.rs, command.rs, mouse.rs -- core event handling)
**Status**: Complete

- [x] Task 2.1: Update `App::save_session()` -- pattern-match `TabContent::Document` (skip image tabs)
- [x] Task 2.2: Update `App::open_file_from_overlay()` -- pattern-match tab content for duplicate check
- [x] Task 2.3: Update `App::handle_close_tab()` -- branch on `TabContent::Document` vs `TabContent::Image`
- [x] Task 2.4: Update `App::sync_active_view_to_tab()` -- for `TabContent::Image`, set `active_view = None`
- [x] Task 2.5: Update `cmd_tab_next()` / `cmd_tab_prev()` in command.rs -- handle `TabContent::Image` by setting `active_view = None`
- [x] Task 2.6: Update mouse.rs tab-click handling for `TabContent` matching
- [x] Task 2.7: Verify `cargo build --workspace` compiles with zero new warnings

**Checkpoint**: `cargo build --workspace` succeeds. All existing tests pass. No behavioral changes yet.

### Phase 3: Image Detection and Opening

**Goal**: Detect image files by extension and route to `Editor::open_image()`.
**Risk**: Medium
**Status**: Complete

- [x] Task 3.1: Add helper function `is_image_extension(path: &Path) -> bool` in app.rs (or a utility module)
- [x] Task 3.2: Update `App::open_file()` to check extension and call `editor.open_image()` for images
- [x] Task 3.3: Update `App::open_file_from_overlay()` to detect and handle image files (including duplicate open prevention by checking `editor.images` paths)
- [x] Task 3.4: Update `handle_explorer_enter()` to route image files correctly
- [x] Task 3.5: Verify opening an image file creates a tab with correct label and `TabContent::Image`

**Checkpoint**: Image files can be opened and appear as tabs. Switching to them shows blank editor area (no rendering yet).

### Phase 4: Image Rendering

**Goal**: Add `ratatui-image` dependency and create `ImageViewWidget` for rendering.
**Risk**: Medium (new external dependencies)
**Status**: Complete

- [x] Task 4.1: Add `ratatui-image` and `image` to workspace `Cargo.toml` dependencies
- [x] Task 4.2: Add `ratatui-image` and `image` to `crates/termcode-term/Cargo.toml`
- [x] Task 4.3: Add `image_picker: Option<Picker>` and `image_cache: HashMap<ImageId, Mutex<Box<dyn StatefulProtocol>>>` to `App` struct
- [x] Task 4.4: Initialize `Picker::from_query_stdio()` in `App::new()` (wrapped in Option for fallback)
- [x] Task 4.5: Decode and cache image when opening (in `App::open_file()` after `editor.open_image()`)
- [x] Task 4.6: Remove cached image when closing image tab
- [x] Task 4.7: Create `crates/termcode-term/src/ui/image_view.rs` -- `ImageViewWidget` using `ratatui_image::StatefulImage`
- [x] Task 4.8: Add `pub mod image_view;` to `crates/termcode-term/src/ui/mod.rs`
- [x] Task 4.9: Update `render.rs` to branch rendering: if active tab is `TabContent::Image`, render `ImageViewWidget` instead of `EditorViewWidget`
- [x] Task 4.10: Skip completion/hover widget rendering when active tab is an image
- [x] Task 4.11: Update `StatusBarWidget` to show image info (filename, format, file size) when image tab is active

**Checkpoint**: Opening a PNG/JPEG file displays the image in the editor area. Status bar shows image metadata.

### Phase 5: Edge Cases and Polish

**Goal**: Handle errors, large files, resize, and ensure robust read-only behavior.
**Risk**: Low
**Status**: Complete

- [x] Task 5.1: Handle corrupted/unreadable image files -- display error message in editor area
- [x] Task 5.2: Handle terminal resize -- re-fit image to new dimensions
- [x] Task 5.3: Block `Insert` mode entry when active tab is an image
- [x] Task 5.4: Ensure `Ctrl+W` (close tab) works correctly for image tabs
- [x] Task 5.5: Ensure `Ctrl+S` (save) is a no-op for image tabs
- [x] Task 5.6: Test tab switching between document and image tabs (active_view transitions)
- [x] Task 5.7: Test duplicate open prevention for image files
- [x] Task 5.8: Run `cargo clippy --workspace` -- fix all warnings

**Checkpoint**: All edge cases handled. Full test suite passes. Clippy clean.

## 5. Quality Gate

- [ ] Build success: `cargo build --workspace`
- [ ] Tests pass: `cargo test --workspace`
- [ ] Lint pass: `cargo clippy --workspace` (0 warnings)
- [ ] Format check: `cargo fmt --check`
- [ ] Manual test: open a PNG file from file explorer -- image renders
- [ ] Manual test: open same image again -- switches to existing tab
- [ ] Manual test: switch between document tab and image tab -- no crash
- [ ] Manual test: close image tab with Ctrl+W -- tab removed, cache cleaned
- [ ] Manual test: open corrupted file with image extension -- error shown

## 6. Notes

### Key Risk: Tab.doc_id Refactor

The `Tab.doc_id` -> `Tab.content: TabContent` change is the highest-risk part of this feature. It touches 19 call sites across 6 files. Phase 1 and Phase 2 are deliberately separated to make this refactor atomic and verifiable before adding any new functionality.

### Architecture Boundary Compliance

- `ImageId`, `ImageEntry`, `TabContent` are defined in `termcode-view` (Layer 2) -- they are pure data types with no terminal/image dependencies
- `ratatui-image`, `image` crate, `Picker`, decoded image cache all live in `termcode-term` (Layer 3)
- `Editor` stores only metadata (`ImageEntry`); actual pixel data is in `App.image_cache`

### active_view Behavior for Image Tabs

When an image tab is active, `editor.active_view` must be `None`. This is because there is no `View` (viewport into a document) for images. All code that calls `editor.active_view()` or `editor.active_document()` already handles the `None` case via `Option`, but each call site should be verified during implementation.

### ratatui-image Version Compatibility

Verify compatibility between `ratatui-image` and `ratatui = "0.29"` (the workspace version). The `ratatui-image` crate version must match the ratatui version. Check crates.io at implementation time.

### Patterns to Avoid

- Do NOT add `image` or `ratatui-image` dependencies to `termcode-view`
- Do NOT store decoded `DynamicImage` or protocol objects in `Editor` -- they belong in `App`
- Do NOT create a `View` for image tabs -- use `active_view = None`

## 7. Implementation Notes

### Phase 1 (2026-03-29)

- Created: 1 file (`crates/termcode-view/src/image.rs`)
- Modified: 3 files (`lib.rs`, `tab.rs`, `editor.rs`)
- Risk: High (core Tab refactor)
- Notes: `Tab.doc_id` replaced with `Tab.content: TabContent`. All TabManager methods updated. Editor gains `images` map and image lifecycle methods.

### Phase 2 (2026-03-29)

- Modified: 3 files (`app.rs`, `command.rs`, `mouse.rs`)
- Risk: High (event handling)
- Notes: All `tab.doc_id` accesses in termcode-term converted to `tab.content` pattern matching. Test code updated.

### Phase 3 (2026-03-29)

- Modified: 1 file (`app.rs`)
- Risk: Medium
- Notes: Added `is_image_extension()` detection, `open_image_file()` method, and duplicate image open prevention in `open_file_from_overlay()`.

### Phase 4 (2026-03-29)

- Created: 1 file (`crates/termcode-term/src/ui/image_view.rs`)
- Modified: 6 files (workspace Cargo.toml, term Cargo.toml, `app.rs`, `render.rs`, `ui/mod.rs`, `status_bar.rs`)
- Risk: Medium (new deps)
- Notes: Added `ratatui-image` 8.x and `image` 0.25.x. `Picker` initialized once, images decoded and cached as `StatefulProtocol` in `Mutex`. Render branches on `TabContent::Image`. Status bar shows format and file size.

### Phase 5 (2026-03-29)

- Modified: 2 files (`command.rs`, `app.rs`)
- Risk: Low
- Notes: Insert mode blocked for image tabs. Save is no-op. Clippy/fmt clean. All 113 tests pass.

---

Last Updated: 2026-03-29
Status: Complete
