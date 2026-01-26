---
phase: 01-foundation
plan: 01
subsystem: ui
tags: [egui, eframe, egui_dock, gui, desktop]

# Dependency graph
requires: []
provides:
  - egui application shell with eframe
  - dockable 4-panel layout via egui_dock
  - centralized SelectionState architecture
  - WasmPokeApp struct with module/callgraph storage
affects:
  - 01-02 (file loading)
  - all future phases (UI foundation)

# Tech tracking
tech-stack:
  added: [eframe 0.33, egui_dock 0.18, rfd 0.17, log 0.4, env_logger 0.11]
  patterns: [centralized state, tab viewer pattern, dock layout]

key-files:
  created:
    - src/gui/mod.rs
    - src/gui/app.rs
    - src/gui/state.rs
    - src/gui/tabs.rs
  modified:
    - Cargo.toml
    - src/main.rs

key-decisions:
  - "Removed ratatui/crossterm TUI dependencies"
  - "SelectionState as single source of truth for all panels"
  - "TabViewer holds references to state, not mutable app reference (borrow checker)"

patterns-established:
  - "SelectionState: centralized selection prevents sync bugs across panels"
  - "WasmPokeTabViewer pattern: separate struct for TabViewer impl with borrowed state"

# Metrics
duration: 5min
completed: 2026-01-26
---

# Phase 01 Plan 01: egui Application Shell Summary

**egui application shell with eframe, 4-panel dockable layout via egui_dock, and SelectionState foundation**

## Performance

- **Duration:** 5 min
- **Started:** 2026-01-26T20:08:05Z
- **Completed:** 2026-01-26T20:12:57Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Replaced ratatui TUI with egui-based GUI application
- Created dockable 4-panel layout (Functions, Call Graph, Size Tree, Inspector)
- Established SelectionState pattern for centralized selection management
- Application launches with File menu (Open, Quit) and draggable panels

## Task Commits

Each task was committed atomically:

1. **Task 1: Add GUI dependencies to Cargo.toml** - `262cc3d` (feat)
2. **Task 2: Create GUI module structure with state and tabs** - `4955223` (feat)
3. **Task 3: Implement WasmPokeApp with eframe::App and dock layout** - `fe72640` (feat)

## Files Created/Modified

- `Cargo.toml` - Added eframe/egui_dock/rfd/log, removed ratatui/crossterm
- `src/main.rs` - Replaced TUI with egui entry point
- `src/gui/mod.rs` - GUI module exports
- `src/gui/app.rs` - WasmPokeApp and eframe::App implementation
- `src/gui/state.rs` - SelectionState centralized selection
- `src/gui/tabs.rs` - TabKind enum for panel types

## Decisions Made

1. **WasmPokeTabViewer pattern** - The TabViewer implementation holds references to borrowed state rather than &mut WasmPokeApp to satisfy borrow checker (DockArea needs &mut dock_state while TabViewer needs access to app state)

2. **Removed TUI code entirely** - Clean break from ratatui/crossterm; no backwards compatibility maintained since this is a rewrite

3. **SelectionState fields** - Using `Option<u32>` for selected_function (function index, not list position) to survive filter changes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

1. **Borrow checker with DockArea** - Initial code had `DockArea::new(&mut self.dock_state).show(ctx, &mut WasmPokeTabViewer { app: self })` which failed because both borrow `self` mutably. Resolved by having WasmPokeTabViewer hold only the specific borrowed references it needs (`module: Option<&'a WasmModuleInfo>`).

2. **Deprecated egui::menu::bar** - Replaced with `ui.horizontal()` + `ui.menu_button()` pattern.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Application shell is ready for file loading (Plan 02)
- WasmPokeApp has `module`, `call_graph`, `wasm_bytes`, `wasm_path` fields ready to populate
- SelectionState ready to track selection across panels
- Remaining unused field warnings will resolve as features are added

---
*Phase: 01-foundation*
*Completed: 2026-01-26*
