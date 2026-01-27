---
phase: 04-inspector
plan: 01
completed: 2026-01-26

subsystem: inspector
tags: [wat, disassembly, keyboard-nav, panels]

dependency-graph:
  requires: [02-01, 02-02, 02-03]
  provides: [inspector-panel, wat-display, instruction-cursor]
  affects: [04-02, 04-03]

tech-stack:
  added: [syntect]
  patterns: [virtualized-scroll, cached-disassembly]

key-files:
  created:
    - src/gui/panels/inspector.rs
  modified:
    - Cargo.toml
    - src/gui/panels/mod.rs
    - src/gui/app.rs

decisions:
  - id: "cache-on-selection-change"
    choice: "Update WAT cache only when func_index changes"
    rationale: "Avoids redundant disassembly on every frame"
  - id: "reset-cursor-on-function-change"
    choice: "Reset instruction_cursor to 0 when function changes"
    rationale: "Prevents stale cursor position from previous function"
  - id: "click-to-position-cursor"
    choice: "Clicking a line sets instruction_cursor"
    rationale: "Intuitive mouse interaction complements keyboard nav"

metrics:
  duration: "4 min"
  tasks: 3
  files-changed: 4
  lines-added: ~256
---

# Phase 04 Plan 01: WAT Panel Foundation Summary

**One-liner:** InspectorPanel with cached WAT disassembly display and vim-style j/k/g/G keyboard navigation.

## What Was Built

Created the foundational InspectorPanel that displays WAT disassembly for the currently selected function:

1. **InspectorPanel struct** with caching for efficient rendering:
   - `cached_func_index` - tracks which function is cached
   - `cached_wat_lines` - Vec<WatLine> from disassemble_function_wat_lines
   - `cached_hex_bytes` - function body bytes for future hex view

2. **WAT display** with virtualized scrolling:
   - Uses ScrollArea with show_rows for performance
   - Monospace font for instruction text
   - Visual highlight on current line using selection.bg_fill

3. **Keyboard navigation** following function_list pattern:
   - j/ArrowDown - move cursor down
   - k/ArrowUp - move cursor up
   - g/Home - jump to top
   - G (shift+g)/End - jump to bottom
   - Click to set cursor position

4. **Integration with app.rs**:
   - InspectorPanel field in WasmPokeApp
   - wasm_bytes passed to WasmPokeTabViewer
   - TabKind::Inspector match arm calls inspector_panel.show()

## Commit Log

| Hash | Type | Description |
|------|------|-------------|
| d26b142 | chore | Enable syntect feature for egui_extras |
| 79ec732 | feat | Create InspectorPanel with WAT display and keyboard navigation |
| 7e24433 | feat | Wire InspectorPanel into application |

## Verification

- [x] cargo check passes
- [x] cargo build succeeds
- [x] InspectorPanel created with 234 lines (> 150 min requirement)
- [x] InspectorPanel exports from panels/mod.rs
- [x] TabKind::Inspector calls inspector_panel.show()
- [x] Uses disassemble_function_wat_lines from wasm_poke

## Deviations from Plan

None - plan executed exactly as written.

## Files Changed

```
Cargo.toml                        (+1 syntect feature)
src/gui/panels/mod.rs             (+2 lines - inspector module and export)
src/gui/panels/inspector.rs       (+234 lines - new file)
src/gui/app.rs                    (+19 lines - panel wiring)
```

## Next Phase Readiness

Ready for 04-02 (Hex Panel):
- InspectorPanel caches hex bytes
- instruction_cursor tracks position for sync
- Pattern established for multi-panel inspector layout
