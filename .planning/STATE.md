# Project State: wasm-poke egui rewrite

**Last Updated:** 2026-01-26

## Project Reference

**Core Value:** Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

**Current Focus:** Phase 2 Complete - Ready for Phase 3 (Tree Panels)

**Key Constraint:** Desktop only - no web/WASM target. Centralized state to prevent sync bugs.

## Current Position

**Phase:** 2 of 6 (Function List Panel) - COMPLETE
**Plan:** 3 of 3 complete
**Status:** Phase complete

**Progress:** [#####.....] 42%

### Phase 2 Success Criteria

- [x] User sees scrollable list of functions with names and sizes
- [x] User can select functions (single click, Ctrl+click, Shift+click)
- [x] User can filter functions by typing in search box
- [x] List performs well with 1000+ functions (virtualized rendering)

### Active Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FUNC-01 | Function list with virtualized rendering | Complete |
| FUNC-02 | Multi-select support (Ctrl/Shift click) | Complete |
| FUNC-03 | Name filter search box | Complete |
| FUNC-04 | Vim-style keyboard navigation | Complete |

## Performance Metrics

**Plans Completed:** 5
**Plans Total:** ~12 (across 6 phases)
**Verification Pass Rate:** 100%
**Phase 1 Duration:** 11 min (01-01: 5 min, 01-02: 6 min)
**Phase 2 Duration:** 13 min (02-01: 3 min, 02-02: 6 min, 02-03: 4 min)

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
- [ ] Run `/gsd:plan-phase 3` to create Phase 3 execution plan (Tree Panels)

## Session Continuity

### Last Session

**Date:** 2026-01-26
**Accomplished:** Completed Plan 02-03 (Keyboard navigation and multi-select)
  - Vim-style keyboard navigation: j/k/g/G/arrows for row navigation
  - Half-page scrolling: Ctrl+d (down), Ctrl+u (up)
  - Multi-select: Ctrl+click toggles, Shift+click extends range
  - Automatic scroll-to-row on keyboard navigation
  - Filter focus check to disable vim keys while typing
**Stopped At:** Phase 2 complete, ready for Phase 3

### Next Session

**Start With:** Run `/gsd:plan-phase 3` for Tree Panels phase
**Context Needed:** ROADMAP.md, 02-CONTEXT.md patterns, CallGraph structure

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
| src/main.rs | egui entry point |

---
*State initialized: 2026-01-26*
*Phase 1 completed: 2026-01-26*
*Plan 02-01 completed: 2026-01-26*
*Plan 02-02 completed: 2026-01-26*
*Plan 02-03 completed: 2026-01-26*
*Phase 2 completed: 2026-01-26*
