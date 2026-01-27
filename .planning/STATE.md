# Project State: wasm-poke egui rewrite

**Last Updated:** 2026-01-27

## Project Reference

**Core Value:** Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

**Current Focus:** Phase 5 Complete - Navigation & Help

**Key Constraint:** Desktop only - no web/WASM target. Centralized state to prevent sync bugs.

## Current Position

**Phase:** 5 of 6 (Navigation & Help)
**Plan:** 3 of 3 complete (including gap closure)
**Status:** Phase complete, verified

**Progress:** [########░░] 83%

### Phase 5 Success Criteria

- [x] User can press Enter on a call instruction and navigate to the called function
- [x] User can return to previous function after goto (navigation history)
- [x] User can see help text explaining the current Wasm instruction (hover tooltip)
- [x] Help text covers all standard Wasm instructions (130+ with fallback)

### Active Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| INSP-03 | Goto navigation from call instructions | Complete |
| INSP-04 | Instruction explanations (help tooltips) | Complete |

## Performance Metrics

**Plans Completed:** 17
**Plans Total:** ~19 (across 6 phases)
**Verification Pass Rate:** 100%
**Phase 1 Duration:** 11 min (01-01: 5 min, 01-02: 6 min)
**Phase 2 Duration:** 13 min (02-01: 3 min, 02-02: 6 min, 02-03: 4 min)
**Phase 3 Duration:** 26 min (03-01: 5 min, 03-02: 5 min, 03-03: 8 min, 03-04: 8 min)
**Phase 4 Duration:** 14 min (04-01: 4 min, 04-02: 2 min, 04-03: 5 min, 04-04: 3 min)
**Phase 5 Duration:** 10 min (05-01: 5 min, 05-02: 2 min, 05-03: 3 min) - gap closure

## Accumulated Context

### Key Decisions

| Decision | Rationale | Phase |
|----------|-----------|-------|
| Desktop only | DWARF source mapping requires filesystem access; simplifies architecture | Pre-Phase 1 |
| egui + eframe | Native + future web possible from single codebase | Pre-Phase 1 |
| Centralized SelectionState | Prevents TUI sync bugs (wat_cursor, source_scroll desync) | Pre-Phase 1 |
| WasmPokeTabViewer pattern | TabViewer holds borrowed refs, not &mut app, to satisfy borrow checker | 01-01 |
| Removed TUI entirely | Clean break from ratatui/crossterm for egui rewrite | 01-01 |
| Separate dialog from parsing | load_wasm_file shows dialog; load_wasm_from_path handles parsing/state | 01-02 |
| Log errors instead of dialogs | Parse failures logged via log::error, no UI interruption | 01-02 |
| Reset SelectionState on load | Prevents stale selections when loading new file | 01-02 |
| BTreeSet for multi-select | Deterministic iteration order for predictable selection display | 02-01 |
| Separate focus from selection | focus_index allows keyboard navigation preview before confirming | 02-01 |
| last_selected for inspector | Inspector shows one function even with multi-select | 02-01 |
| Cached indices for filter/sort | Store Vec<usize> into functions instead of cloning - memory efficient | 02-02 |
| Incoming calls via edge iteration | Count calls by iterating CallGraph edges - simple and correct | 02-02 |
| Pass ctx to show() | Enables keyboard input handling outside UI closure | 02-03 |
| Filter focus disables vim keys | Prevents j/k interference while typing in search filter | 02-03 |
| Closure-based click handling | Captures row context and modifiers cleanly | 02-03 |
| CollapsingState over CollapsingHeader | Finer control over expand/collapse with custom headers | 03-01 |
| Backtrack visited set after children | Prevents false positives when same function called via different paths | 03-01 |
| Reverse graph computed once on load | O(E) precomputation enables O(1) caller lookup per function | 03-02 |
| Logarithmic color scale for size | Linear scale makes small differences invisible; log scale differentiates better | 03-03 |
| Warm orange for size visualization | Distinct from call tree, size = "weight" = warm colors | 03-03 |
| Focus path as Vec<(u32, usize)> | Unique node identification for keyboard navigation | 03-04 |
| handle_keyboard pattern | Consistent tree navigation across all panels | 03-04 |
| subtree_contains_match | Recursive filter with ancestor visibility | 03-04 |
| Cache on selection change | Update WAT cache only when func_index changes; avoids redundant disassembly | 04-01 |
| Reset cursor on function change | Reset instruction_cursor to 0 when function changes; prevents stale position | 04-01 |
| Click to position cursor | Clicking a line sets instruction_cursor; intuitive mouse interaction | 04-01 |
| Primary source file by frequency | Count DWARF mappings to determine dominant source file | 04-02 |
| Cache source files in HashMap | Avoid repeated filesystem reads for same source file | 04-02 |
| Instruction byte range from offsets | Use WatLine.offset differences to determine byte ranges for highlighting | 04-02 |
| Hex panel display-only | Click-to-navigate from bytes is complex (reverse-map offset to instruction) and low-value | 04-03 |
| N:1 indicator as asterisk | Simple gutter asterisk (*) when highlighted line maps to multiple WAT instructions | 04-03 |
| Use vertical_scroll_offset | egui 0.33 doesn't have scroll_to_row; use (row * ROW_HEIGHT) calculation | 04-03 |
| Track active_tab in SelectionState | Centralized state for keyboard focus isolation | 04-04 |
| Click detection for tab activation | ui_contains_pointer + any_click works with egui_dock | 04-04 |
| Fallback help for unknown instructions | Returns generic help instead of None for unknown Wasm instructions | 05-02 |
| Skip comments/syntax for tooltips | Comments (;;) and syntax markers ((, )) don't get tooltips | 05-02 |
| on_hover_text for tooltips | egui handles tooltip positioning automatically | 05-02 |
| KeyAction enum for navigation | Cleaner than sentinel values for keyboard command dispatch | 05-01 |
| Cap navigation history at 50 | Prevents unbounded memory growth; drops oldest when full | 05-01 |
| Clear history on clear_selection only | History persists across manual selections for usability | 05-01 |
| navigated_back flag for cursor restore | Set flag before navigate_back(), check in update_cache() to skip cursor=0 | 05-03 |
| Call target tooltip over generic help | Check for call target first, show function name instead of generic "call" help | 05-03 |

### Technical Debt

None yet.

### Blockers

None.

### TODOs

- [x] Run `/gsd:plan-phase 1` to create Phase 1 execution plan
- [x] Execute Plan 01-01 (egui app shell)
- [x] Execute Plan 01-02 (File loading)
- [x] Run `/gsd:plan-phase 2` to create Phase 2 execution plan (Function List)
- [x] Execute Plan 02-01 (State extension for multi-select)
- [x] Execute Plan 02-02 (Function list with TableBuilder)
- [x] Execute Plan 02-03 (Keyboard navigation and multi-select)
- [x] Run `/gsd:plan-phase 3` to create Phase 3 execution plan (Tree Panels)
- [x] Execute Plan 03-01 (Call Tree panel)
- [x] Execute Plan 03-02 (Callers Tree panel)
- [x] Execute Plan 03-03 (Size Tree panel)
- [x] Execute Plan 03-04 (Keyboard nav + filter for trees)
- [x] Run `/gsd:plan-phase 4` to create Phase 4 execution plan (Inspector Panel)
- [x] Execute Plan 04-01 (WAT Panel foundation)
- [x] Execute Plan 04-02 (Hex Panel and Source Panel)
- [x] Execute Plan 04-03 (Synchronized scrolling and click navigation)
- [x] Execute Plan 04-04 (Keyboard focus isolation fix)
- [x] Run `/gsd:plan-phase 5` to create Phase 5 execution plan (Navigation & Help)
- [x] Execute Plan 05-01 (Inspector navigation - Enter/Backspace)
- [x] Execute Plan 05-02 (Instruction help tooltips)
- [x] Execute Plan 05-03 (UAT gap closure - cursor restore + call tooltip)
- [ ] Run `/gsd:plan-phase 6` to create Phase 6 execution plan (Output Modes)

## Session Continuity

### Last Session

**Date:** 2026-01-27
**Accomplished:** Completed Phase 5 UAT gap closure (Plan 05-03)
  - Fixed cursor position restoration on back navigation
  - Added call target function name tooltips
  - All UAT gaps now closed
**Stopped At:** Phase 5 fully complete with gaps closed

### Next Session

**Start With:** Plan Phase 6 (Output Modes) - JSON and summary output
**Context Needed:** Review Phase 6 requirements in ROADMAP.md

### Important Files

| File | Purpose |
|------|---------|
| .planning/PROJECT.md | Core value, constraints |
| .planning/REQUIREMENTS.md | v1 requirements with IDs |
| .planning/ROADMAP.md | Phase structure and success criteria |
| .planning/research/SUMMARY.md | Architecture recommendations |
| src/lib.rs | Analysis entry point (preserve) |
| src/gui/mod.rs | GUI module exports |
| src/gui/app.rs | WasmPokeApp and eframe::App impl |
| src/gui/state.rs | SelectionState with multi-select support |
| src/gui/tabs.rs | TabKind enum for panel types |
| src/gui/panels/mod.rs | Panel module exports |
| src/gui/panels/function_list.rs | FunctionListPanel with keyboard nav + multi-select |
| src/gui/panels/call_tree.rs | CallTreePanel with keyboard nav + filter |
| src/gui/panels/callers_tree.rs | CallersTreePanel with keyboard nav + filter |
| src/gui/panels/size_tree.rs | SizeTreePanel with keyboard nav + filter |
| src/gui/panels/inspector.rs | InspectorPanel with synchronized three-panel view |
| src/main.rs | egui entry point |

---
*State initialized: 2026-01-26*
*Phase 1 completed: 2026-01-26*
*Plan 02-01 completed: 2026-01-26*
*Plan 02-02 completed: 2026-01-26*
*Plan 02-03 completed: 2026-01-26*
*Phase 2 completed: 2026-01-26*
*Plan 03-01 completed: 2026-01-26*
*Plan 03-02 completed: 2026-01-26*
*Plan 03-03 completed: 2026-01-26*
*Plan 03-04 completed: 2026-01-26*
*Phase 3 completed: 2026-01-26*
*Plan 04-01 completed: 2026-01-26*
*Plan 04-02 completed: 2026-01-27*
*Plan 04-03 completed: 2026-01-27*
*Plan 04-04 completed: 2026-01-27*
*Phase 4 completed: 2026-01-27*
*Plan 05-01 completed: 2026-01-27*
*Plan 05-02 completed: 2026-01-27*
*Plan 05-03 completed: 2026-01-27*
*Phase 5 completed: 2026-01-27*
