---
phase: 06-output-modes
plan: 02
subsystem: cli-integration
tags: [cli, main, dispatch, headless, exit-codes]
requires: [06-01]
provides: [cli-mode-dispatch, headless-operation, stdin-support]
affects: []
tech-stack:
  added: []
  patterns: [mode-dispatch, exit-code-convention]
key-files:
  created: []
  modified: [src/main.rs, src/gui/app.rs]
decisions:
  - id: cli-dispatch
    choice: match-on-output-mode
    reason: Clear enum-based dispatch separates GUI and headless paths
  - id: exit-codes
    choice: unix-convention
    reason: 0=success, 1=error, 2=usage matches standard CLI tools
  - id: auto-load
    choice: gui-accepts-file-arg
    reason: Running `wasm-poke file.wasm` auto-loads file in GUI mode
metrics:
  duration: 4 min
  completed: 2026-01-26
---

# Phase 6 Plan 02: Mode Dispatch Integration Summary

**One-liner:** CLI mode dispatch in main.rs routes to GUI, JSON, or summary output with proper exit codes.

## What Changed

### Task 1: Implement mode dispatch in main.rs (f39a788)

Modified `src/main.rs`:
- Changed `main()` return type from `eframe::Result<()>` to `ExitCode`
- Added mode dispatch based on `cli.output_mode()`:
  - `OutputMode::Gui` -> `run_gui(file)`
  - `OutputMode::Json` -> `run_json_output(&cli)`
  - `OutputMode::Summary` -> `run_summary_output(&cli)`
- Implemented `run_gui()` with auto-load if file provided on command line
- Implemented `run_json_output()` with full wasm parsing and call graph
- Implemented `run_summary_output()` for human-readable output
- Added `load_wasm_bytes()` helper supporting file or stdin ("-")
- Added `get_output_writer()` helper supporting stdout or file output
- Exit codes: 0 (success), 1 (error), 2 (usage error)

Modified `src/gui/app.rs`:
- Made `load_wasm_from_path()` public for CLI file loading
- Changed signature to `pub fn load_wasm_from_path(&mut self, path: impl AsRef<std::path::Path>)`

### Task 2: Test CLI output modes (verification only)

Verified all CLI modes work correctly:
- `--json <file>` produces valid JSON with functions, call_graph, summary
- `--summary <file>` produces formatted text with stats and top 20 functions
- `-o <path>` writes output to file instead of stdout
- stdin reading works with `cat file.wasm | wasm-poke --json -`
- Exit code 1 for errors (file not found, parse error)
- Exit code 2 for usage errors (missing required file)
- Mutual exclusion enforced (--json --summary rejected)
- GUI still launches when no output flags specified

## Verification Results

| Check | Result |
|-------|--------|
| `wasm-poke --help` | Shows all flags with descriptions |
| `wasm-poke --json <file.wasm>` | Valid JSON output |
| `wasm-poke --summary <file.wasm>` | Formatted summary output |
| `wasm-poke --json -o out.json <file.wasm>` | JSON written to file |
| `wasm-poke --json nonexistent.wasm` | Exit code 1 |
| `wasm-poke --json` (no file) | Exit code 2 with usage hint |
| `wasm-poke --json --summary` | Clap rejects with error |

## Decisions Made

| Decision | Rationale |
|----------|-----------|
| Mode dispatch via match | Clean separation between GUI and headless code paths |
| Exit code convention | 0/1/2 follows Unix CLI standards |
| Auto-load in GUI mode | `wasm-poke file.wasm` opens GUI with file loaded |
| Public load_wasm_from_path | Enables CLI to trigger file loading |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Made load_wasm_from_path public**
- **Found during:** Task 1
- **Issue:** Method was private, CLI couldn't call it
- **Fix:** Changed visibility to `pub` and generalized path parameter
- **Files modified:** src/gui/app.rs
- **Commit:** f39a788

## Phase 6 Completion

This plan completes Phase 6 (Output Modes). All success criteria met:

- [x] OUT-01: JSON output mode works with structured output
- [x] OUT-02: Summary text output works without GUI
- [x] Headless operation: no X11/display required for --json or --summary
- [x] Exit codes follow Unix convention (0 success, 1 error, 2 usage)
- [x] GUI mode still functions when no output flags specified

## Files Changed

```
src/main.rs           - Mode dispatch implementation (173 lines added)
src/gui/app.rs        - Public load_wasm_from_path (2 lines changed)
```

## Commits

- f39a788: feat(06-02): implement CLI mode dispatch in main.rs
