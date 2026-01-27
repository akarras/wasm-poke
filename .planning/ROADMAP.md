# Roadmap: wasm-poke egui rewrite

**Created:** 2026-01-26
**Depth:** comprehensive
**Phases:** 6
**Coverage:** 17/17 v1 requirements mapped

## Overview

This roadmap transforms wasm-poke from a ratatui TUI to an egui desktop application. The phase structure follows architectural dependencies: foundation first, then list views that validate the state model, then complex synchronized views (call graph, inspector), and finally output modes. Each phase delivers a complete, verifiable capability. Desktop only (no web/WASM target).

## Phases

### Phase 1: Foundation & State Architecture

**Goal:** Establish egui app shell with centralized state architecture that prevents sync bugs

**Dependencies:** None (starting phase)

**Requirements:**
- FOUND-01: egui app shell with eframe and egui_dock for panel layout
- FOUND-02: Centralized state architecture (single source of truth for selections)

**Success Criteria:**
1. User can launch the application and see a window with dockable panel layout
2. User can load a Wasm file via native file dialog
3. Application displays parsed function count after loading (proves analysis pipeline connected)
4. Panel layout can be rearranged by dragging (egui_dock working)

**Plans:** 2 plans

Plans:
- [x] 01-01-PLAN.md - Create egui app shell with dockable panel layout
- [x] 01-02-PLAN.md - Implement file loading and display function count

**Notes:**
- This phase establishes the SelectionState pattern that all subsequent phases depend on
- No feature work until state architecture is validated
- Reuse existing parser.rs and model.rs without modification

---

### Phase 2: Function List View

**Goal:** Users can browse and filter functions by name and size

**Dependencies:** Phase 1 (app shell, state architecture)

**Requirements:**
- LIST-01: Function list view with size sorting (descending by code size)
- LIST-02: Filter/search by glob pattern (case-insensitive)
- LIST-03: Name demangling for Rust symbols
- LIST-04: Keyboard navigation (j/k/g/G vim-style)

**Success Criteria:**
1. User can see all functions sorted by code size (largest first)
2. User can type a filter pattern and see only matching functions
3. User can read demangled Rust function names (not raw mangled symbols)
4. User can navigate the list with j/k keys and jump to top/bottom with g/G
5. Selecting a function updates the global selection state (visible in other panels when they exist)

**Plans:** 3 plans

Plans:
- [x] 02-01-PLAN.md - Extend SelectionState for multi-select and add bytesize dependency
- [x] 02-02-PLAN.md - Implement FunctionListPanel with table, sorting, and filtering
- [x] 02-03-PLAN.md - Add keyboard navigation and multi-select click handling

**Notes:**
- Use egui_extras TableBuilder for virtualization (handles 10K+ functions)
- Store selection as function index, not list position (prevents filter desync)
- Use function index as widget ID, not display name (prevents ID collisions)

---

### Phase 3: Call Graph Views

**Goal:** Users can explore function call relationships and understand cumulative size impact

**Dependencies:** Phase 2 (function list provides selection)

**Requirements:**
- CALL-01: Call tree view showing function call relationships (entry to exit)
- CALL-02: Size summary tree with cumulative bytes through call graph
- CALL-03: Keyboard navigation for call tree (expand/collapse, navigate)
- CALL-04: Filter/search in call tree view

**Success Criteria:**
1. User can see which functions the selected function calls (and what those call, recursively)
2. User can see cumulative size impact for each subtree (bytes that would be removed if function eliminated)
3. User can expand/collapse tree nodes with keyboard (Enter or arrow keys)
4. User can navigate tree with j/k keys
5. User can filter the call tree to find specific functions

**Plans:** 4 plans

Plans:
- [x] 03-01-PLAN.md - Create CallTreePanel with tree rendering and expand/collapse
- [x] 03-02-PLAN.md - Create CallersTreePanel with reverse graph
- [x] 03-03-PLAN.md - Create SizeTreePanel with cumulative size and color intensity
- [x] 03-04-PLAN.md - Add keyboard navigation and filter to all tree panels

**Notes:**
- Leverage existing CallGraph and unique_cumulative_size from model.rs
- Tree view is separate tab/panel from function list
- Selection in call tree updates global SelectionState (syncs with function list)

---

### Phase 4: Three-Panel Inspector

**Goal:** Users can see hex bytes, WAT instructions, and source code in synchronized panels

**Dependencies:** Phase 2 (function selection), Phase 3 (navigation patterns)

**Requirements:**
- INSP-01: Three-panel inspection view (hex bytes | WAT instructions | source code)
- INSP-02: Synchronized cursor navigation across all three panels
- INSP-05: Keyboard navigation with WAT panel as primary driver

**Success Criteria:**
1. User can see the selected function displayed as three synchronized columns: hex bytes, WAT disassembly, source code
2. User can move cursor in WAT panel and see corresponding hex bytes and source line highlighted automatically
3. User can use j/k to navigate instruction-by-instruction in WAT panel
4. Cursor sync is immediate and never desyncs (addresses main TUI bug)
5. Source panel shows "no source info" gracefully when DWARF mapping unavailable

**Plans:** 3 plans

Plans:
- [x] 04-01-PLAN.md - Create InspectorPanel with WAT display and keyboard navigation
- [x] 04-02-PLAN.md - Add Hex panel and Source panel with highlighting
- [x] 04-03-PLAN.md - Implement synchronized scrolling and click navigation

**Notes:**
- All three panels derive display position from single instruction_cursor in SelectionState
- This is the critical feature that differentiates wasm-poke and must work flawlessly
- Reuse existing DWARF/addr2line infrastructure from lib.rs

---

### Phase 5: Inspector Navigation & Help

**Goal:** Users can navigate between functions and understand individual instructions

**Dependencies:** Phase 4 (inspector panels exist)

**Requirements:**
- INSP-03: Goto navigation from call instructions to target function
- INSP-04: Instruction explanations (help text for each Wasm instruction)

**Success Criteria:**
1. User can press Enter on a call instruction and navigate to the called function
2. User can return to previous function after goto (navigation history)
3. User can see help text explaining the current Wasm instruction (hover or panel)
4. Help text covers all standard Wasm instructions (not just common ones)

**Plans:** 3 plans

Plans:
- [x] 05-01-PLAN.md - Add goto navigation with history stack
- [x] 05-02-PLAN.md - Add instruction help tooltips on hover
- [x] 05-03-PLAN.md - Gap closure: cursor restore and call target tooltips

**Notes:**
- Goto navigation pushes to a stack so user can "go back"
- Instruction help can be a tooltip or dedicated help panel
- Instruction descriptions can be static data (no runtime lookup needed)

---

### Phase 6: Output Modes

**Goal:** Users can use wasm-poke in scripts and CI without the GUI

**Dependencies:** Phase 1 (analysis pipeline), minimal UI dependency

**Requirements:**
- OUT-01: JSON output mode for scripting
- OUT-02: Summary text output without GUI

**Success Criteria:**
1. User can run `wasm-poke --json <file.wasm>` and get structured JSON output
2. User can run `wasm-poke --summary <file.wasm>` and get text summary without launching GUI
3. JSON output includes function list, sizes, and call graph data
4. CLI flags work without requiring X11/display (headless operation)

**Plans:** 2 plans

Plans:
- [x] 06-01-PLAN.md - Create CLI argument parsing and output generation modules
- [x] 06-02-PLAN.md - Integrate mode dispatch and test CLI output modes

**Notes:**
- Uses clap (already in Cargo.toml) for CLI argument parsing
- serde_json (already in Cargo.toml) for JSON serialization
- Mode dispatch in main.rs: check output flags BEFORE importing GUI modules
- Ensures wasm-poke remains useful for CI/scripting workflows

---

## Progress

| Phase | Status | Requirements | Completion |
|-------|--------|--------------|------------|
| 1 - Foundation | Complete | FOUND-01, FOUND-02 | 100% |
| 2 - Function List | Complete | LIST-01, LIST-02, LIST-03, LIST-04 | 100% |
| 3 - Call Graph | Complete | CALL-01, CALL-02, CALL-03, CALL-04 | 100% |
| 4 - Inspector | Complete | INSP-01, INSP-02, INSP-05 | 100% |
| 5 - Navigation & Help | Complete | INSP-03, INSP-04 | 100% |
| 6 - Output Modes | Complete | OUT-01, OUT-02 | 100% |

**Overall:** 6/6 phases complete (100%)

---
*Roadmap created: 2026-01-26*
*Last updated: 2026-01-27*
*Phase 2 complete: 2026-01-26*
*Phase 3 complete: 2026-01-26*
*Phase 4 complete: 2026-01-26*
*Phase 5 complete: 2026-01-27*
*Phase 6 complete: 2026-01-27*
*ALL PHASES COMPLETE: 2026-01-27*
