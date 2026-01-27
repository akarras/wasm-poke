# Requirements: wasm-poke

**Defined:** 2026-01-26
**Core Value:** Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

## v1 Requirements

Requirements for the egui rewrite. Each maps to roadmap phases.

### Foundation

- [x] **FOUND-01**: egui app shell with eframe and egui_dock for panel layout
- [x] **FOUND-02**: Centralized state architecture (single source of truth for selections)

### List Views

- [x] **LIST-01**: Function list view with size sorting (descending by code size)
- [x] **LIST-02**: Filter/search by glob pattern (case-insensitive)
- [x] **LIST-03**: Name demangling for Rust symbols
- [x] **LIST-04**: Keyboard navigation (j/k/g/G vim-style)

### Call Graph

- [x] **CALL-01**: Call tree view showing function call relationships (entry to exit)
- [x] **CALL-02**: Size summary tree with cumulative bytes through call graph
- [x] **CALL-03**: Keyboard navigation for call tree (expand/collapse, navigate)
- [x] **CALL-04**: Filter/search in call tree view

### Inspector

- [x] **INSP-01**: Three-panel inspection view (hex bytes | WAT instructions | source code)
- [x] **INSP-02**: Synchronized cursor navigation across all three panels
- [ ] **INSP-03**: Goto navigation from call instructions to target function
- [ ] **INSP-04**: Instruction explanations (help text for each Wasm instruction)
- [x] **INSP-05**: Keyboard navigation with WAT panel as primary driver

### Output

- [ ] **OUT-01**: JSON output mode for scripting
- [ ] **OUT-02**: Summary text output without GUI

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Comparison

- **COMP-01**: Diff mode to compare two Wasm binaries
- **COMP-02**: Highlight size changes between versions

### Persistence

- **PERS-01**: Bookmarks for functions of interest
- **PERS-02**: Annotations on functions/instructions
- **PERS-03**: Layout persistence across sessions

### Web Deployment

- **WEB-01**: Web build target (Wasm frontend) - deferred due to DWARF filesystem requirements

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Web/WASM build (v1) | DWARF source mapping requires filesystem access |
| Real-time file watching | Overengineered for manual analysis workflow |
| Runtime/dynamic tracing | Changes tool category from static analyzer to profiler |
| Full debugger functionality | Chrome DevTools already exists for Wasm debugging |
| Wasm editing/patching | Read-only analysis is the focus |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | Phase 1 | Complete |
| FOUND-02 | Phase 1 | Complete |
| LIST-01 | Phase 2 | Complete |
| LIST-02 | Phase 2 | Complete |
| LIST-03 | Phase 2 | Complete |
| LIST-04 | Phase 2 | Complete |
| CALL-01 | Phase 3 | Complete |
| CALL-02 | Phase 3 | Complete |
| CALL-03 | Phase 3 | Complete |
| CALL-04 | Phase 3 | Complete |
| INSP-01 | Phase 4 | Complete |
| INSP-02 | Phase 4 | Complete |
| INSP-03 | Phase 5 | Pending |
| INSP-04 | Phase 5 | Pending |
| INSP-05 | Phase 4 | Complete |
| OUT-01 | Phase 6 | Pending |
| OUT-02 | Phase 6 | Pending |

**Coverage:**
- v1 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0

---
*Requirements defined: 2026-01-26*
*Last updated: 2026-01-26 after Phase 4 completion*
