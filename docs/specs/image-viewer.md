# Image File Viewer

Display image files (.png, .jpg, .jpeg, .gif, .bmp, .webp, .ico, .tiff, .avif) inline within the terminal using `ratatui-image`, with automatic terminal graphics protocol detection and Halfblock fallback.

## Code Reference Checklist

| Item                    | Result                                                                                                                                                                                                   |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Similar feature exists? | No. Only text documents can be opened. `Editor::open_file()` always creates a `Document` (text buffer) and `View`.                                                                                       |
| Reference pattern       | `Tab` currently holds `doc_id: DocumentId`. The `open_file()` flow: `Editor::open_file()` -> create Document + View -> `TabManager::add(label, doc_id)`. Render dispatches based on active doc/view.     |
| Technical constraints   | `termcode-view` is frontend-agnostic (no ratatui, no image crate). New types (`ImageId`, `ImageEntry`, `TabContent`) must be pure data. `ratatui-image` dependency belongs in `termcode-term` (Layer 3). |

### Key files

| File                                   | Role                                          |
| -------------------------------------- | --------------------------------------------- |
| `crates/termcode-view/src/tab.rs`      | `Tab` struct (currently `doc_id: DocumentId`) |
| `crates/termcode-view/src/editor.rs`   | `Editor` struct, `open_file()` method         |
| `crates/termcode-view/src/document.rs` | `DocumentId` type definition                  |
| `crates/termcode-term/src/app.rs`      | `App::open_file()`, file open routing         |
| `crates/termcode-term/src/render.rs`   | Main render orchestration                     |
| `crates/termcode-term/src/ui/`         | All widget implementations                    |
| `crates/termcode-term/Cargo.toml`      | Dependencies for the terminal layer           |

---

## Functional Requirements

### FR-IMG-001: Image File Detection

- **Description**: When the user opens a file (via file explorer, fuzzy finder, or CLI argument), detect whether it is an image file by extension. Supported extensions: `.png`, `.jpg`, `.jpeg`, `.gif`, `.bmp`, `.webp`, `.ico`, `.tiff`, `.tif`, `.avif`.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `App::open_file()` in `crates/termcode-term/src/app.rs:99`, `App::open_file_from_overlay()` in `crates/termcode-term/src/app.rs:762`
- **Details**:
  - Extension check is case-insensitive.
  - Detection happens at the `App` level (termcode-term), before calling `Editor::open_file()`.
  - If detected as an image, route to `Editor::open_image()` instead of `Editor::open_file()`.

### FR-IMG-002: TabContent Enum

- **Description**: Introduce a `TabContent` enum in `termcode-view` to distinguish between document tabs and image tabs.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `Tab` struct in `crates/termcode-view/src/tab.rs:4`
- **Details**:
  - Replace `Tab.doc_id: DocumentId` with `Tab.content: TabContent`.
  - `TabContent` enum:
    ```
    enum TabContent {
        Document(DocumentId),
        Image(ImageId),
    }
    ```
  - All existing code that accesses `tab.doc_id` must be updated to pattern-match on `tab.content`.
  - `TabManager::find_by_doc_id()` continues to work by matching `TabContent::Document(id)`.
  - Add `TabManager::find_by_image_id()` for image tab lookup.
  - `Tab.modified` is always `false` for image tabs.

### FR-IMG-003: ImageId and ImageEntry Types

- **Description**: Define `ImageId` and `ImageEntry` in `termcode-view` as frontend-agnostic metadata types.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: Analogous to `DocumentId` in `crates/termcode-view/src/document.rs:13`
- **Details**:
  - `ImageId(pub usize)` -- unique identifier, analogous to `DocumentId`.
  - `ImageEntry`:
    ```
    struct ImageEntry {
        pub id: ImageId,
        pub path: PathBuf,
        pub format: String,       // e.g., "png", "jpeg"
        pub file_size: u64,       // in bytes
    }
    ```
  - No `image` crate dependency. Only stores file metadata.
  - Actual image decoding and rendering handled in `termcode-term`.

### FR-IMG-004: Editor Image State

- **Description**: Add image storage and management to the `Editor` struct.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `Editor` struct in `crates/termcode-view/src/editor.rs:56`
- **Details**:
  - Add `images: HashMap<ImageId, ImageEntry>` field to `Editor`.
  - Add `next_image_id: usize` counter.
  - Add `open_image(&mut self, path: &Path) -> anyhow::Result<ImageId>` method:
    - Allocates new `ImageId`.
    - Reads file metadata (size from `fs::metadata()`).
    - Determines format from extension.
    - Inserts `ImageEntry` into `images` map.
    - Creates tab with `TabContent::Image(image_id)`.
    - Returns `ImageId`.
  - Add `active_image(&self) -> Option<&ImageEntry>`:
    - Returns the `ImageEntry` if the active tab is an image tab.
  - Add `close_image(&mut self, image_id: ImageId)`:
    - Removes from `images` map.
    - Removes associated tab via `TabManager::remove_by_image_id()`.

### FR-IMG-005: Image Rendering with ratatui-image

- **Description**: Create `ImageViewWidget` in `termcode-term` that uses the `ratatui-image` crate to render images in the terminal.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: Widget pattern in `crates/termcode-term/src/ui/editor_view.rs`
- **Details**:
  - New file: `crates/termcode-term/src/ui/image_view.rs`
  - `ImageViewWidget` implements `ratatui::widgets::Widget`.
  - Uses `ratatui_image::picker::Picker` with `Picker::from_query_stdio()` for automatic terminal protocol detection (Sixel, Kitty, iTerm2).
  - Falls back to Halfblock if no graphics protocol is available.
  - The `Picker` should be created once during `App` initialization and stored in `App`, not created per-frame.
  - Image decoding (`image` crate `DynamicImage`) is done once when the image is first opened, cached in `App` (not in `Editor`, since `image` crate is a term-layer dependency).
  - Widget receives a reference to the protocol-specific image object and renders it.
  - Image is resized/fitted to the available `editor_area` while preserving aspect ratio.
  - Display the file name and dimensions in the status bar area.

### FR-IMG-006: Render Dispatch

- **Description**: Modify `render.rs` to branch between editor view and image view based on active tab content type.
- **Priority**: High
- **Status**: Draft
- **Code Reference**: `render()` in `crates/termcode-term/src/render.rs:22`
- **Details**:
  - In the `render()` function, after rendering the tab bar, check active tab content:
    - `TabContent::Document(_)` -> render `EditorViewWidget` (existing behavior).
    - `TabContent::Image(_)` -> render `ImageViewWidget`.
  - Overlays (search, fuzzy finder, command palette) still render on top of image view.
  - Completion and hover widgets are skipped for image tabs.

### FR-IMG-007: Read-Only Behavior

- **Description**: Image tabs are read-only. Editing commands are naturally skipped since there is no active `Document` or `View`.
- **Priority**: Medium
- **Status**: Draft
- **Code Reference**: Command dispatch in `crates/termcode-term/src/app.rs`
- **Details**:
  - When the active tab is an image, `editor.active_document()` and `editor.active_view()` return `None`.
  - Commands that operate on `&mut Editor` and access `active_document()` will early-return or no-op.
  - `EditorMode` remains `Normal` for image tabs. `Insert` mode entry is blocked.
  - Navigation commands (tab switching, sidebar toggle) continue to work.
  - Close tab (`Ctrl+W`) works -- calls `Editor::close_image()`.
  - Status bar shows image info (filename, dimensions, format) instead of cursor position.

### FR-IMG-008: Duplicate Open Prevention

- **Description**: If the same image file is already open in a tab, switch to that tab instead of opening a duplicate.
- **Priority**: Medium
- **Status**: Draft
- **Code Reference**: `open_file_from_overlay()` in `crates/termcode-term/src/app.rs:762` (existing pattern for documents)
- **Details**:
  - Before opening a new image, check `Editor.images` for an entry with the same path.
  - If found, call `TabManager::find_by_image_id()` and activate that tab.
  - Same pattern already used for documents in `open_file_from_overlay()`.

---

## Technical Design

### New File: `crates/termcode-view/src/image.rs`

```rust
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImageId(pub usize);

#[derive(Debug, Clone)]
pub struct ImageEntry {
    pub id: ImageId,
    pub path: PathBuf,
    pub format: String,
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub enum TabContent {
    Document(crate::document::DocumentId),
    Image(ImageId),
}
```

### Modified File: `crates/termcode-view/src/tab.rs`

```rust
use crate::image::{ImageId, TabContent};

#[derive(Debug, Clone)]
pub struct Tab {
    pub label: String,
    pub content: TabContent,  // was: doc_id: DocumentId
    pub modified: bool,
}
```

Key method changes on `TabManager`:

- `add()` -> `add_document()` and `add_image()` (or generic `add(label, content)`).
- `find_by_doc_id()` -> pattern-match on `TabContent::Document`.
- Add `find_by_image_id()`.
- `remove_by_doc_id()` -> match variant.

### Modified File: `crates/termcode-view/src/editor.rs`

New fields:

```rust
pub images: HashMap<ImageId, ImageEntry>,
next_image_id: usize,
```

New methods: `open_image()`, `active_image()`, `close_image()`, `next_image_id()`.

`active_document()` / `active_view()` must handle the case where active tab is an image (return `None`).

### New File: `crates/termcode-term/src/ui/image_view.rs`

- `ImageViewWidget` struct holding a reference to the decoded image protocol object.
- Implements `ratatui::widgets::Widget`.
- Uses `ratatui_image::StatefulImage` or `ratatui_image::Image` depending on protocol.

### Image Cache in App

```rust
// In App struct:
image_picker: Option<ratatui_image::picker::Picker>,
image_cache: HashMap<ImageId, Box<dyn ratatui_image::protocol::Protocol>>,
```

- `image_picker` initialized once in `App::new()` via `Picker::from_query_stdio()`.
- When opening an image, decode with `image::open()`, then `picker.new_protocol()` to get the protocol-specific image, store in cache.
- On tab close, remove from cache.

### Modified File: `crates/termcode-term/Cargo.toml`

New dependencies:

```toml
ratatui-image = { version = "...", features = ["crossterm"] }
image = { version = "...", default-features = false, features = ["png", "jpeg", "gif", "bmp", "webp", "ico", "tiff", "avif"] }
```

---

## Affected Code Paths

Every site that accesses `tab.doc_id` must be updated to use `tab.content`. Key locations:

| File         | Usage of `tab.doc_id`                                      |
| ------------ | ---------------------------------------------------------- |
| `tab.rs`     | `find_by_doc_id()`, `remove_by_doc_id()`                   |
| `editor.rs`  | `open_file()`, `sync_tab_modified()`, `active_*()` methods |
| `app.rs`     | `open_file_from_overlay()`, tab close logic                |
| `render.rs`  | Active document lookup for editor widget                   |
| `command.rs` | Commands accessing active document                         |
| `mouse.rs`   | Tab bar click handling                                     |

---

## Constraints

- `termcode-view` must NOT depend on `ratatui`, `image`, or `ratatui-image`.
- `Document` and `Buffer` structs are not modified.
- Existing text editing functionality must be unaffected.
- Image decoding and protocol objects live in `termcode-term` only.
- The `TabContent` enum is the only structural change to `termcode-view`'s tab model.

---

## Edge Cases

1. **Corrupted/unreadable image file**: Display error message in the editor area instead of crashing. Status bar shows "Error: could not decode image".
2. **Very large image files**: Consider a size limit (configurable). Images over the limit show a warning with file size info.
3. **Terminal resize**: Image must be re-fitted to new dimensions on resize events.
4. **Terminal without graphics support**: Halfblock fallback renders a low-resolution preview. This is handled automatically by `ratatui-image`'s `Picker`.
5. **Switching tabs**: Switching from image tab to document tab (and vice versa) must correctly update `active_view` state. When switching to an image tab, `active_view` should be set to `None`.
6. **Binary files misidentified**: Only match by extension, not by content sniffing. A `.png` file that is actually text will fail at decode and show an error.
