---
phase: 05-navigation-help
plan: 02
subsystem: ui
tags: [egui, tooltips, hover, help, wasm-instructions]

# Dependency graph
requires:
  - phase: 04-inspector-panel
    provides: WAT disassembly display with row rendering
provides:
  - Instruction help tooltips on hover in WAT panel
  - Extended instruction coverage (sign-extension, saturating truncation, reference types, table ops, bulk memory)
  - Fallback help text for unknown instructions
affects: [05-03-polish]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "extract_mnemonic helper to parse WAT text for help lookup"
    - "on_hover_text for egui tooltip display"

key-files:
  created: []
  modified:
    - src/help.rs
    - src/gui/panels/inspector.rs

key-decisions:
  - "Fallback to generic help message instead of None for unknown instructions"
  - "Skip comments (;;) and syntax markers ((, )) for tooltips"
  - "Use egui on_hover_text for automatic tooltip positioning"

patterns-established:
  - "extract_mnemonic: parse first word of trimmed WAT text, skip comments/syntax"

# Metrics
duration: 2min
completed: 2026-01-27
---

# Phase 05 Plan 02: Instruction Help Tooltips Summary

**WAT instruction hover tooltips with 130+ instruction explanations and fallback for unknown instructions**

## Performance

- **Duration:** 2 min
- **Started:** 2026-01-27T02:46:07Z
- **Completed:** 2026-01-27T02:47:58Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added hover tooltips to WAT panel showing instruction explanations
- Extended help.rs with 35+ additional instruction descriptions (sign-extension, saturating truncation, reference types, table operations, bulk memory)
- Added fallback "Unknown WebAssembly instruction" message for instructions not in database
- Comments (;;) and syntax markers ((func, )) do not trigger tooltips

## Task Commits

Each task was committed atomically:

1. **Task 1: Add fallback and additional instructions to help.rs** - `2957199` (feat)
2. **Task 2: Add hover tooltips to WAT instruction display** - `8bc3075` (feat)

## Files Created/Modified
- `src/help.rs` - Added sign-extension, saturating truncation, reference types, table ops, bulk memory instructions; changed fallback from None to generic help message
- `src/gui/panels/inspector.rs` - Added get_instruction_help import, extract_mnemonic helper, on_hover_text call in show_wat_panel

## Decisions Made
- Changed fallback from `None` to `Some("Unknown WebAssembly instruction. See spec for details.")` - ensures all instructions get at least some help text
- Skip comments starting with `;;` - these are informational, not instructions
- Skip syntax markers like `(func`, `(param`, `)` - these are structure, not instructions
- Keep `end` as a valid instruction with tooltip - it's a real Wasm control flow instruction

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Hover tooltips functional for all WAT instructions
- Ready for Plan 05-03 (keyboard help panel) or polish phase
- All success criteria met:
  - Hovering over instructions shows explanatory tooltips
  - 130+ WebAssembly instructions have help text
  - Comments and syntax do not trigger tooltips
  - Unknown instructions show generic fallback help

---
*Phase: 05-navigation-help*
*Completed: 2026-01-27*
