# Specification Change History

## [2026-03-30] Unsaved Changes Confirmation Dialog specification

### Added

- [FR-CONFIRM-001] ConfirmDialog state struct in Editor
- [FR-CONFIRM-002] Ctrl+W close modified file with confirmation
- [FR-CONFIRM-003] Ctrl+Q quit with unsaved files confirmation
- [FR-CONFIRM-004] Dialog button actions (save/discard/cancel)
- [FR-CONFIRM-005] Keyboard navigation within dialog
- [FR-CONFIRM-006] Dialog widget rendering (centered overlay popup)

**Specification file**: `docs/specs/confirm-dialog.md`

## [2026-03-29] Phase 5: Plugin System specification

### Added

- [FR-PLUGIN-001] PluginManager -- Lua VM ownership and lifecycle
- [FR-PLUGIN-002] Plugin metadata manifest (plugin.toml)
- [FR-PLUGIN-003] Plugin entry point (init.lua) and registration API
- [FR-PLUGIN-004] Editor API exposed to Lua (read/write methods)
- [FR-PLUGIN-005] Hook system for editor events (8 hooks)
- [FR-PLUGIN-006] Plugin command registration and dispatch (integrates with CommandRegistry)
- [FR-PLUGIN-007] Lua sandbox and resource limits
- [FR-PLUGIN-008] Plugin configuration in config.toml
- [FR-PLUGIN-009] Plugin lifecycle (load, execute, teardown)
- [FR-PLUGIN-010] Crate internal module structure (6 modules)
- [FR-PLUGIN-011] Logging API for plugins

**Specification file**: `docs/specs/plugin-system.md`
