---
status: complete
phase: 02-function-list
source: [02-01-SUMMARY.md, 02-02-SUMMARY.md, 02-03-SUMMARY.md]
started: 2026-01-26T22:45:00Z
updated: 2026-01-26T22:45:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Function List Display
expected: After loading a .wasm file, the Functions tab shows a table with three columns: Name, Size, and Calls. Functions are listed with human-readable sizes (e.g., "1.2 KiB" instead of raw bytes).
result: pass

### 2. Default Sort Order
expected: Functions are sorted by Size column in descending order (largest first) by default.
result: pass

### 3. Column Sort Toggle
expected: Clicking a column header sorts by that column. Clicking again toggles between ascending and descending. Active column shows ^ or v indicator.
result: pass

### 4. Filter Input
expected: Typing in the filter box narrows the list to matching functions. Match count updates (e.g., "25 of 100 functions"). Clearing filter restores full list.
result: pass

### 5. Demangled Names
expected: Rust function names are shown demangled (readable like `core::fmt::write`) not raw mangled (like `_ZN4core3fmt5write17hXXXX`).
result: pass

### 6. Single Click Selection
expected: Clicking a function row highlights it. Only one row is selected.
result: pass

### 7. Ctrl+Click Multi-Select
expected: Holding Ctrl and clicking adds/removes individual functions from selection. Multiple rows can be highlighted.
result: pass

### 8. Shift+Click Range Select
expected: Clicking one row, then Shift+clicking another, selects all rows between them (inclusive).
result: pass

### 9. Keyboard Navigation j/k
expected: Pressing j moves selection down one row. Pressing k moves selection up one row.
result: pass

### 10. Keyboard Navigation g/G
expected: Pressing g jumps to the first function. Pressing G (shift+g) jumps to the last function.
result: issue
reported: "shift+G selects all of the rows in the middle, it seems to treat it like shift + clicking the bottom row"
severity: major

### 11. Keyboard Half-Page Scroll
expected: Ctrl+d scrolls down roughly half a page. Ctrl+u scrolls up roughly half a page.
result: pass

### 12. Arrow Key Navigation
expected: Arrow Up and Arrow Down work the same as k and j respectively.
result: pass

### 13. Scroll Follows Selection
expected: When using keyboard navigation, the selected row stays visible (table scrolls to keep it in view).
result: pass

### 14. Filter Doesn't Interfere with Typing
expected: While typing in the filter box, vim keys (j/k/g/G) are inserted as text, not interpreted as navigation commands.
result: pass

## Summary

total: 14
passed: 13
issues: 1
pending: 0
skipped: 0

## Gaps

- truth: "Pressing G (shift+g) jumps to the last function without selecting intermediate rows"
  status: failed
  reason: "User reported: shift+G selects all of the rows in the middle, it seems to treat it like shift + clicking the bottom row"
  severity: major
  test: 10
  root_cause: ""
  artifacts: []
  missing: []
  debug_session: ""
