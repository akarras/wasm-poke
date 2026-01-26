# Requirements: wasm-poke

**Defined:** 2026-01-26
**Core Value:** Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

## v1 Requirements

Requirements for the egui rewrite. Each maps to roadmap phases.

### Foundation

- [ ] **FOUND-01**: egui app shell with eframe and egui_dock for panel layout
- [ ] **FOUND-02**: Centralized state architecture (single source of truth for selections)

### List Views

- [ ] **LIST-01**: Function list view with size sorting (descending by code size)
- [ ] **LIST-02**: Filter/search by glob pattern (case-insensitive)
- [ ] **LIST-03**: Name demangling for Rust symbols
- [ ] **LIST-04**: Keyboard navigation (j/k/g/G vim-style)

### Call Graph

- [ ] **CALL-01**: Call tree view showing function call relationships (entry to exit)
- [ ] **CALL-02**: Size summary tree with cumulative bytes through call graph
- [ ] **CALL-03**: Keyboard navigation for call tree (expand/collapse, navigate)
- [ ] **CALL-04**: Filter/search in call tree view

### Inspector

- [ ] **INSP-01**: Three-panel inspection view (hex bytes | WAT instructions | source code)
- [ ] **INSP-02**: Synchronized cursor navigation across all three panels
- [ ] **INSP-03**: Goto navigation from call instructions to target function
- [ ] **INSP-04**: Instruction explanations (help text for each Wasm instruction)
- [ ] **INSP-05**: Keyboard navigation with WAT panel as primary driver

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

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Web/WASM build | DWARF source mapping requires filesystem access |
| Real-time file watching | Overengineered for manual analysis workflow |
| Runtime/dynamic tracing | Changes tool category from static analyzer to profiler |
| Full debugger functionality | Chrome DevTools already exists for Wasm debugging |
| Wasm editing/patching | Read-only analysis is the focus |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| FOUND-01 | TBD | Pending |
| FOUND-02 | TBD | Pending |
| LIST-01 | TBD | Pending |
| LIST-02 | TBD | Pending |
| LIST-03 | TBD | Pending |
| LIST-04 | TBD | Pending |
| CALL-01 | TBD | Pending |
| CALL-02 | TBD | Pending |
| CALL-03 | TBD | Pending |
| CALL-04 | TBD | Pending |
| INSP-01 | TBD | Pending |
| INSP-02 | TBD | Pending |
| INSP-03 | TBD | Pending |
| INSP-04 | TBD | Pending |
| INSP-05 | TBD | Pending |
| OUT-01 | TBD | Pending |
| OUT-02 | TBD | Pending |

**Coverage:**
- v1 requirements: 17 total
- Mapped to phases: 0
- Unmapped: 17 ⚠️

---
*Requirements defined: 2026-01-26*
*Last updated: 2026-01-26 after initial definition*
