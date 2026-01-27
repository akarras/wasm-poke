---
status: diagnosed
phase: 05-navigation-help
source: [05-01-SUMMARY.md, 05-02-SUMMARY.md]
started: 2026-01-27T03:00:00Z
updated: 2026-01-27T03:15:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Navigate to call target with Enter
expected: Navigate to a `call N` instruction, press Enter, inspector shows the called function
result: issue
reported: "This technically works, I think we should show the demangled name so we know what fn we're looking at."
severity: minor

### 2. Navigate back with Backspace
expected: After goto, press Backspace to return to previous function at exact cursor position
result: issue
reported: "it navigates back but it is at the top of the function"
severity: major

### 3. Multi-level navigation history
expected: Navigate A→B→C with Enter, then back C→B→A with Backspace (history stack works)
result: pass

### 4. Enter on non-call instruction does nothing
expected: Navigate to a non-call instruction (e.g., i32.add), press Enter, nothing happens
result: pass

### 5. Hover tooltip on WAT instruction
expected: Hover over any WAT instruction (e.g., i32.add), see explanatory tooltip
result: pass

### 6. Hover on comment shows no tooltip
expected: Hover over a comment line (;;), no tooltip appears
result: pass

### 7. Hover on unknown instruction shows fallback
expected: If an unknown instruction exists, hovering shows "Unknown WebAssembly instruction. See spec for details."
result: pass

## Summary

total: 7
passed: 5
issues: 2
pending: 0
skipped: 0

## Gaps

- truth: "Navigate to call target shows which function you're navigating to"
  status: failed
  reason: "User reported: This technically works, I think we should show the demangled name so we know what fn we're looking at."
  severity: minor
  test: 1
  root_cause: "No visual feedback when navigating to call target - user doesn't know which function they're about to jump to"
  artifacts:
    - path: "src/gui/panels/inspector.rs"
      issue: "GotoCall handling navigates silently without showing target function name"
  missing:
    - "Show demangled function name in tooltip or status when hovering/pressing Enter on call instruction"
  debug_session: ""

- truth: "Backspace returns to previous function at exact cursor position"
  status: failed
  reason: "User reported: it navigates back but it is at the top of the function"
  severity: major
  test: 2
  root_cause: "update_cache() in inspector.rs line 181 unconditionally resets instruction_cursor to 0 when function changes, overwriting the cursor position restored by navigate_back()"
  artifacts:
    - path: "src/gui/panels/inspector.rs"
      issue: "Line 181 resets cursor unconditionally on function change"
  missing:
    - "Remove unconditional cursor reset in update_cache() or make it conditional on navigation source"
  debug_session: ".planning/debug/back-navigation-cursor-position.md"
