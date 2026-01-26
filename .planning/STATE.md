# Project State: wasm-poke egui rewrite

**Last Updated:** 2026-01-26

## Project Reference

**Core Value:** Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

**Current Focus:** Phase 3 Complete - All Call Graph Views Implemented

**Key Constraint:** Desktop only - no web/WASM target. Centralized state to prevent sync bugs.

## Current Position

**Phase:** 3 of 6 (Call Graph Views)
**Plan:** 3 of 3 complete
**Status:** Phase complete

**Progress:** [########..] 75%

### Phase 3 Success Criteria

- [x] User can see Call Tree tab displaying downstream calls for selected function
- [x] Tree expand/collapse works
- [x] Recursive calls handled gracefully (no infinite loops, shows marker)
- [x] Selection syncs bidirectionally between function list and call tree
- [x] User can see Callers Tree tab (upstream calls)
- [x] User can see Size Tree tab (cumulative size impact)

### Active Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| TREE-01 | Call Tree with expand/collapse | Complete |
| TREE-02 | Cycle detection with marker | Complete |
| TREE-03 | Depth limit of 5 levels | Complete |
| TREE-04 | Callers Tree (upstream) | Complete |
| TREE-05 | Size Tree (cumulative) | Complete |

## Performance Metrics

**Plans Completed:** 9
**Plans Total:** ~12 (across 6 phases)
**Verification Pass Rate:** 100%
**Phase 1 Duration:** 11 min (01-01: 5 min, 01-02: 6 min)
**Phase 2 Duration:** 13 min (02-01: 3 min, 02-02: 6 min, 02-03: 4 min)
**Phase 3 Duration:** 18 min (03-01: 5 min, 03-02: 5 min, 03-03: 8 min)

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
- [ ] Run `/gsd:plan-phase 4` to create Phase 4 execution plan (Inspector Panel)

## Session Continuity

### Last Session

**Date:** 2026-01-26
**Accomplished:** Completed Plan 03-03 (Size Tree panel)
  - SizeTreePanel with cumulative size using unique_cumulative_size
  - Size format: "name - X.X KiB (Y.Y%)"
  - Logarithmic color-coded background for visual differentiation
  - Same tree rendering pattern as CallTreePanel
  - Wired into app and tab viewer
**Stopped At:** Phase 3 complete, ready for Phase 4

### Next Session

**Start With:** Run `/gsd:plan-phase 4` to plan Inspector Panel
**Context Needed:** WAT disassembly from lib.rs, source mapping functions

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
| src/gui/panels/call_tree.rs | CallTreePanel with tree rendering |
| src/gui/panels/callers_tree.rs | CallersTreePanel with upstream calls |
| src/gui/panels/size_tree.rs | SizeTreePanel with cumulative size |
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
*Phase 3 completed: 2026-01-26*
