---
phase: 04-inspector
plan: 04
subsystem: gui/keyboard-navigation
tags: [egui, keyboard, focus, tabs]
dependency-graph:
  requires: ["04-03"]
  provides: ["keyboard-focus-isolation"]
  affects: []
tech-stack:
  added: []
  patterns: ["active-tab-tracking", "focus-gating"]
key-files:
  created: []
  modified:
    - src/gui/state.rs
    - src/gui/app.rs
    - src/gui/panels/function_list.rs
    - src/gui/panels/inspector.rs
    - src/gui/panels/call_tree.rs
    - src/gui/panels/callers_tree.rs
    - src/gui/panels/size_tree.rs
decisions:
  - id: FOCUS-01
    choice: "Track active_tab in SelectionState"
    rationale: "Centralized state prevents per-panel focus tracking complexity"
  - id: FOCUS-02
    choice: "Click detection via ui_contains_pointer + any_click"
    rationale: "Simple egui pattern that works with egui_dock tab system"
metrics:
  duration: "3 min"
  completed: "2026-01-27"
---

# Phase 4 Plan 4: Keyboard Focus Isolation Summary

**One-liner:** Fixed j/k keyboard navigation to only affect the active tab by tracking active_tab in SelectionState and gating handle_keyboard calls.

## What Was Built

### Root Cause Analysis

The bug was that all panels called `handle_keyboard()` using `ctx.input(|i| i.key_pressed(...))` which is global. When user pressed 'j':
1. FunctionListPanel.handle_keyboard() processed it - updated selection.focus_index
2. InspectorPanel.handle_keyboard() processed it - updated selection.instruction_cursor
3. All tree panels also processed it - updated tree focus paths

### Fix Implementation

1. **Added active_tab field to SelectionState** (state.rs)
   - Tracks which tab currently has keyboard focus
   - Defaults to FunctionList on startup
   - Replaced derive(Default) with explicit Default impl

2. **Track active tab on click** (app.rs)
   - In WasmPokeTabViewer::ui(), detect tab click
   - Use `ui.ui_contains_pointer() && ui.input(|i| i.pointer.any_click())`
   - Set `selection.active_tab = *tab` on click

3. **Gate keyboard handling in all panels**
   - function_list.rs: only handle if active_tab == TabKind::FunctionList
   - inspector.rs: only handle if active_tab == TabKind::Inspector
   - call_tree.rs: only handle if active_tab == TabKind::CallTree
   - callers_tree.rs: only handle if active_tab == TabKind::Callers
   - size_tree.rs: only handle if active_tab == TabKind::SizeTree

## Verification

- [x] `cargo check` passes
- [x] Keyboard navigation isolated to active tab
- [x] No simultaneous updates to multiple panels from single keypress

## Commits

| Commit | Description |
|--------|-------------|
| 5e01540 | feat(04-04): add active_tab field to SelectionState |
| 8eff8f1 | feat(04-04): track active tab on click in WasmPokeTabViewer |
| 29d4a08 | feat(04-04): gate keyboard handling by active_tab in all panels |

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

Phase 4 UAT issue resolved. Application ready for Phase 5 (Export/Stats) or Phase 6 (Polish).
