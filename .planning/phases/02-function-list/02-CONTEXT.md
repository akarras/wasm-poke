# Phase 2: Function List View - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Users can browse and filter functions by name and size in a sortable, navigable list. The list displays functions from the loaded Wasm file with sorting, filtering, and vim-style keyboard navigation. Inspector integration (showing function details) is Phase 4.

</domain>

<decisions>
## Implementation Decisions

### List Display
- Columns: Function name, byte size, call count (3 columns)
- Size format: Human-readable (1.2 KB, 45 B) — no raw bytes shown
- Long names: Truncate in list, full name in tooltip on hover
- Sorting: Clickable column headers for name/size/calls — default sort by size descending

### Filter Behavior
- Position: Filter input always visible above the list
- Timing: Live filtering as user types (no debounce, no Enter required)
- Empty state: "No functions match 'xyz'" message centered in list area
- Match count: Display "42 matches" near the filter input

### Selection Feedback
- Highlight style: Background color + left border accent (both)
- Selection mode: Multi-select supported (Shift/Ctrl click)
- Inspector target: Most recently selected function drives inspector (Phase 4)
- Focus vs selected: Distinct visual states — focus ring separate from selection highlight

### Keyboard Feel
- Vim bindings: Full set — j/k, g/G/gg, Ctrl+d/u (half-page), H/M/L (screen positions)
- Arrow keys: Also work for navigation (accessible to non-vim users)
- Scroll behavior: Keep selected row centered in view
- Enter key: Opens selected function in inspector (Phase 4 integration point)

### Claude's Discretion
- Exact colors for selection/focus states
- Column width proportions
- Tooltip delay and styling
- Scroll animation smoothness

</decisions>

<specifics>
## Specific Ideas

- Full vim navigation set suggests power-user focus — should feel snappy and responsive
- Multi-select + "last selected drives inspector" allows comparison workflows while keeping inspector simple
- Centered scroll keeps context visible during rapid navigation

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 02-function-list*
*Context gathered: 2026-01-26*
