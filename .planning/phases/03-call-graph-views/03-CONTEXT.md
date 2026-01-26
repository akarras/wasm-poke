# Phase 3: Call Graph Views - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Three tree views for exploring function call relationships:
1. **Call Tree** — What functions the selected function calls (downstream, recursive)
2. **Callers Tree** — What functions call the selected function (upstream)
3. **Size Tree** — Cumulative size impact through the call graph

Users can expand/collapse nodes, navigate with keyboard, filter, and sync selection with the function list. All three are separate dockable tabs.

</domain>

<decisions>
## Implementation Decisions

### Tree Visualization
- Click + keyboard for expand/collapse (click arrow to toggle, Enter/Space or arrow keys)
- Depth limit of ~5 levels, show "..." for deeper nodes
- Icons + indentation style (expand/collapse arrows, possibly function icon)
- Recursive/circular calls: show once with marker (↻ or "(recursive)"), don't re-expand

### Size Display
- Human readable format ("12.5 KB" with appropriate units)
- Always show percentage of total Wasm size ("12.5 KB (8.2%)")
- Color intensity for size visualization (larger = more intense background)
- Cumulative size = unique reachable bytes (bytes removed if function eliminated)

### Selection Behavior
- Bidirectional sync: tree selection updates function list, list updates tree
- Combined view for multi-select: show union of all selected functions' call trees
- Multi-root display: Claude's discretion on how to organize multiple root functions
- Click-to-navigate behavior: Claude's discretion (single vs double click)

### Panel Layout
- Three separate dockable tabs: "Call Tree", "Callers", "Size Tree"
- Each tree panel has its own filter/search box at top
- No selection state: show full call graph (all entry points and their trees)

### Claude's Discretion
- Multi-root display style when multiple functions selected
- Single vs double click for navigation
- Exact arrow/icon styling
- Filter matching behavior (substring, fuzzy, etc.)
- Keyboard shortcut specifics beyond j/k navigation

</decisions>

<specifics>
## Specific Ideas

- Color intensity for size should help users quickly spot "heavy" subtrees
- The "unique reachable" size metric answers "how much would I save by removing this?"
- Entry points in full call graph = functions not called by anything else (roots)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 03-call-graph-views*
*Context gathered: 2026-01-26*
