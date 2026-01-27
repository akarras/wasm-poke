---
phase: 06-output-modes
plan: 01
subsystem: cli
tags: [clap, serde, json, output]

# Dependency graph
requires:
  - phase: 05-navigation
    provides: WasmModuleInfo, CallGraph, function_source_span from lib.rs
provides:
  - CLI argument parsing with clap derive (Cli, OutputMode)
  - JSON output generation (JsonOutput, output_json)
  - Summary output generation (output_summary)
affects: [06-02, headless-mode, ci-integration]

# Tech tracking
tech-stack:
  added: []
  patterns: [clap derive for CLI, serde for JSON serialization]

key-files:
  created:
    - src/cli.rs
    - src/output.rs
  modified:
    - src/main.rs

key-decisions:
  - "Clap group for mutual exclusion of --json and --summary"
  - "OutputMode enum for dispatch clarity"
  - "Top 20 functions in summary output"

patterns-established:
  - "CLI module separate from main dispatch logic"
  - "Output functions take generic Write trait for testability"

# Metrics
duration: 4min
completed: 2026-01-26
---

# Phase 06 Plan 01: CLI and Output Modules Summary

**Clap-based CLI argument parsing with --json and --summary output modes, ready for headless dispatch integration**

## Performance

- **Duration:** 4 min
- **Started:** 2026-01-26T22:01:00Z
- **Completed:** 2026-01-26T22:05:00Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments
- Created CLI argument parsing module with clap derive (Cli struct, OutputMode enum)
- Created output generation module with JSON and summary formatters
- Wired both modules into main.rs crate structure
- Added 15 unit tests for CLI parsing and output generation

## Task Commits

Each task was committed atomically:

1. **Task 1: Create CLI argument parsing module** - `c71d83e` (feat)
2. **Task 2: Create output generation module** - `7e9e940` (feat)
3. **Task 3: Wire modules into crate structure** - `0ab6d77` (chore)

## Files Created/Modified
- `src/cli.rs` - CLI argument parsing with Cli struct, OutputMode enum, clap derive
- `src/output.rs` - JSON and summary output generators with JsonOutput, output_json, output_summary
- `src/main.rs` - Added mod cli; and mod output; declarations

## Decisions Made
- Used clap's `group` attribute to make --json and --summary mutually exclusive
- OutputMode enum provides clear dispatch pattern for Plan 02
- Summary shows top 20 functions by size with percentage
- Output functions use `impl Write` for testability with Vec<u8> in tests
- Source span included in JSON if DWARF info available

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- CLI and output modules ready for integration in Plan 02
- Plan 02 will add mode dispatch in main.rs to route based on OutputMode
- Modules compile with unused warnings (expected until dispatch integration)

---
*Phase: 06-output-modes*
*Completed: 2026-01-26*
