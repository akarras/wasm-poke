---
status: complete
phase: 04-inspector
source: [04-01-SUMMARY.md, 04-02-SUMMARY.md, 04-03-SUMMARY.md]
started: 2026-01-26T12:00:00Z
updated: 2026-01-26T12:30:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Inspector Tab Shows WAT Disassembly
expected: Select a function from the function list. Click the Inspector tab. You should see WAT disassembly text displayed in monospace font.
result: pass

### 2. Current Instruction Is Highlighted
expected: In the WAT panel, one instruction line should be visually highlighted (different background color) indicating the current cursor position.
result: pass

### 3. Keyboard Navigation Works (j/k)
expected: With the Inspector tab focused, press j to move cursor down one instruction, press k to move cursor up. The highlight should follow the cursor.
result: issue
reported: "fail- it doesn't recognize which pane is focused. it always navigates on the functions panel."
severity: major

### 4. Jump Navigation Works (g/G)
expected: Press g to jump to the first instruction (top). Press Shift+G to jump to the last instruction (bottom). Highlight moves accordingly.
result: issue
reported: "Same issue- it does look like shift+G does actually go to the bottom instruction, so it's likely all panes are processing it, but the fact it updated the functions pane means it usually is also resetting focus"
severity: major

### 5. Three-Panel Layout Visible
expected: The Inspector should show three columns side-by-side: Hex bytes (left, ~20%), WAT instructions (center, ~45%), and Source code (right, ~35%).
result: pass

### 6. Hex Panel Shows Bytes
expected: The left hex panel should show hex bytes with offset addresses (like 0000:, 0008:, etc.) and 8 bytes per row.
result: pass

### 7. Hex Panel Highlights Current Instruction Bytes
expected: As you navigate instructions with j/k, the hex bytes corresponding to the current instruction should be highlighted with a background color.
result: issue
reported: "same issue as the wat- we move focus on the functions pane. I can't tell if it's working on the hex because the other sections are also updating."
severity: major

### 8. Source Panel Shows Code or Graceful Message
expected: If DWARF debug info is present, the right panel shows source code with line numbers. If no debug info, it shows "No source info available" message.
result: pass

### 9. Source Panel Highlights Current Line
expected: When DWARF info is present, the source line corresponding to the current WAT instruction should be highlighted.
result: skipped
reason: Test file (simple_wasm.wasm) lacks sufficient DWARF debug info to verify

### 10. Click WAT Line to Select
expected: Click on any WAT instruction line. The cursor should jump to that line, and all panels (hex, source) should update their highlights accordingly.
result: pass

### 11. Click Source Line to Navigate
expected: Click on a source code line. The WAT cursor should jump to the first instruction that maps to that source line, and highlights update in all panels.
result: skipped
reason: Test file lacks DWARF debug info to verify source click navigation

### 12. Synchronized Scrolling
expected: Navigate with j/k rapidly to move many lines. All three panels should scroll together to keep the current item visible.
result: skipped
reason: Blocked by keyboard focus issue - cannot test j/k navigation in inspector

### 13. Function Change Resets Cursor
expected: Select a different function from the function list. The Inspector should update to show that function's code, with cursor reset to the first instruction (top).
result: pass

## Summary

total: 13
passed: 7
issues: 3
pending: 0
skipped: 3

## Gaps

- truth: "Keyboard navigation (j/k) works in Inspector panel when focused"
  status: failed
  reason: "User reported: fail- it doesn't recognize which pane is focused. it always navigates on the functions panel."
  severity: major
  test: 3
  root_cause: "All panels call handle_keyboard() using ctx.input() which is global - every panel processes every keypress regardless of active tab. Need to track active tab and only process keyboard in that panel."
  artifacts:
    - path: "src/gui/panels/inspector.rs"
      issue: "handle_keyboard() uses ctx.input() without checking if panel is active"
    - path: "src/gui/panels/function_list.rs"
      issue: "handle_keyboard() uses ctx.input() without checking if panel is active"
  missing:
    - "Track active tab in SelectionState or WasmPokeApp"
    - "Pass active_tab to each panel's show() method"
    - "Only process keyboard events if this panel's tab matches active_tab"

- truth: "Jump navigation (g/G) works in Inspector panel when focused"
  status: failed
  reason: "User reported: Same issue- all panes are processing keyboard events, function pane updates reset focus"
  severity: major
  test: 4
  root_cause: "Same as test 3 - global keyboard handling"
  artifacts: []
  missing: []

- truth: "Hex panel byte highlighting updates with j/k navigation"
  status: failed
  reason: "User reported: Cannot verify due to keyboard focus issue - other sections also updating"
  severity: major
  test: 7
  root_cause: "Same as test 3 - blocked by keyboard focus issue"
  artifacts: []
  missing: []
