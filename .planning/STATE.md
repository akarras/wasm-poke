# Project State: wasm-poke egui rewrite

**Last Updated:** 2026-01-26

## Project Reference

**Core Value:** Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

**Current Focus:** Function List Panel (Phase 2)

**Key Constraint:** Desktop only - no web/WASM target. Centralized state to prevent sync bugs.

## Current Position

**Phase:** 2 of 6 (Function List Panel)
**Plan:** 1 of 3 complete
**Status:** In progress

**Progress:** [###.......] 25%

### Phase 2 Success Criteria

- [ ] User sees scrollable list of functions with names and sizes
- [ ] User can select functions (single click, Ctrl+click, Shift+click)
- [ ] User can filter functions by typing in search box
- [ ] List performs well with 1000+ functions (virtualized rendering)

### Active Requirements

| ID | Requirement | Status |
|----|-------------|--------|
| FUNC-01 | Function list with virtualized rendering | In Progress |
| FUNC-02 | Multi-select support (Ctrl/Shift click) | State Ready |
| FUNC-03 | Name filter search box | Pending |

## Performance Metrics

**Plans Completed:** 3
**Plans Total:** ~12 (across 6 phases)
**Verification Pass Rate:** 100%
**Phase 1 Duration:** 11 min (01-01: 5 min, 01-02: 6 min)
**Phase 2 Duration:** 3 min so far (02-01: 3 min)

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
- [ ] Execute Plan 02-02 (Function list with TableBuilder)
- [ ] Execute Plan 02-03 (Selection and filter integration)

## Session Continuity

### Last Session

**Date:** 2026-01-26
**Accomplished:** Completed Plan 02-01 (State extension for multi-select)
  - Added bytesize and egui_extras dependencies
  - Extended SelectionState with BTreeSet<u32> multi-select
  - Added focus_index and last_selected tracking
  - Added helper methods for selection manipulation
  - 6 unit tests pass
**Stopped At:** Plan 02-01 complete, ready for Plan 02-02

### Next Session

**Start With:** Execute Plan 02-02 (Function list with TableBuilder)
**Context Needed:** 02-01-SUMMARY.md, src/gui/state.rs, src/gui/app.rs

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
| src/main.rs | egui entry point |

---
*State initialized: 2026-01-26*
*Phase 1 completed: 2026-01-26*
*Plan 02-01 completed: 2026-01-26*
