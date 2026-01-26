---
phase: 02-function-list
plan: 02
subsystem: ui
tags: [egui, egui_extras, TableBuilder, virtualization, bytesize]

# Dependency graph
requires:
  - phase: 02-01
    provides: SelectionState with multi-select and focus_index
  - phase: 01-01
    provides: WasmPokeApp with TabViewer pattern and DockArea
provides:
  - FunctionListPanel with virtualized table rendering
  - Sortable columns (Name, Size, Calls)
  - Live filter input with match count
  - Single-click row selection integrated with SelectionState
affects: [02-03, 03-call-graph, 04-inspector]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Panel struct with show() method taking ui, module, call_graph, selection"
    - "Cache invalidation via dirty flag for filter/sort changes"
    - "Incoming call count via CallGraph edge iteration"

key-files:
  created:
    - src/gui/panels/mod.rs
    - src/gui/panels/function_list.rs
  modified:
    - src/gui/mod.rs
    - src/gui/app.rs

key-decisions:
  - "Use cached_indices for filtered/sorted view instead of cloning functions"
  - "Count incoming calls by iterating all CallGraph edges (simple, sufficient for now)"
  - "Use ByteSize::to_string_as(false) for IEC units (KiB, MiB)"

patterns-established:
  - "Panel pattern: struct with state + show() method receiving shared refs"
  - "TableBuilder pattern: header() for clickable sort, body.rows() for virtualized content"

# Metrics
duration: 6min
completed: 2026-01-26
---

# Phase 2 Plan 2: Function List Panel Summary

**FunctionListPanel with virtualized TableBuilder rendering, clickable sort headers, live filter, and SelectionState integration**

## Performance

- **Duration:** 6 min
- **Started:** 2026-01-26T00:00:00Z
- **Completed:** 2026-01-26T00:06:00Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Created panels module structure under src/gui/panels/
- Implemented FunctionListPanel with 3-column virtualized table (Name, Size, Calls)
- Integrated filter input with match count display ("N of M functions")
- Clickable column headers with sort direction indicators
- Single-click selection updates SelectionState.select_single()

## Task Commits

Each task was committed atomically:

1. **Task 1: Create panels module structure** - `aba6668` (feat)
2. **Task 2: Implement FunctionListPanel** - `443a4e2` (feat)
3. **Task 3: Integrate FunctionListPanel into TabViewer** - `02ed1c2` (feat)

## Files Created/Modified

- `src/gui/panels/mod.rs` - Panel module exports (created)
- `src/gui/panels/function_list.rs` - FunctionListPanel with TableBuilder (created, 295 lines)
- `src/gui/mod.rs` - Added panels module declaration
- `src/gui/app.rs` - Added function_list_panel field and TabViewer integration

## Decisions Made

- **Cached indices pattern:** Store Vec<usize> indices into functions array instead of cloning FunctionInfo structs - more memory efficient for large modules
- **Incoming call count:** Iterate all CallGraph edges to count calls to each function - O(total edges) per function but simple and correct
- **ByteSize API:** Used to_string_as(false) for IEC units since ByteSize 1.3 doesn't have display().iec_short()

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed ByteSize API usage**
- **Found during:** Task 2 (FunctionListPanel implementation)
- **Issue:** Plan specified `ByteSize::b(size).display().iec_short().to_string()` but ByteSize 1.3 doesn't have display() method
- **Fix:** Used `ByteSize::b(size).to_string_as(false)` which produces IEC units
- **Files modified:** src/gui/panels/function_list.rs
- **Verification:** Cargo check passes, sizes display correctly
- **Committed in:** 443a4e2 (Task 2 commit)

**2. [Rule 3 - Blocking] Added missing egui import**
- **Found during:** Task 2 (FunctionListPanel implementation)
- **Issue:** egui types not in scope, needed `use eframe::egui;`
- **Fix:** Added import at top of function_list.rs
- **Files modified:** src/gui/panels/function_list.rs
- **Verification:** Cargo check passes
- **Committed in:** 443a4e2 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both were API/import issues that blocked compilation. No scope change.

## Issues Encountered

None - once blocking issues were auto-fixed, plan executed as written.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- FunctionListPanel renders and responds to clicks
- SelectionState tracks selected function
- Ready for Plan 02-03: Multi-select (Ctrl+click, Shift+click) and keyboard navigation

---
*Phase: 02-function-list*
*Plan: 02*
*Completed: 2026-01-26*
