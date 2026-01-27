# Phase 4: Three-Panel Inspector - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Synchronized view of hex bytes, WAT instructions, and source code for the selected function. Users see exactly how their Rust code maps to Wasm instructions. Cursor navigation in WAT panel drives synchronized highlighting across all three panels. Goto navigation and instruction help are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Panel Layout
- Three-panel arrangement: Hex (narrow left) | WAT | Source (side by side)
- Hex panel is narrow and resizable on the left edge
- WAT and Source panels share remaining space side-by-side
- All panels are resizable via egui_dock

### Cursor Visualization
- Full-line background highlight on current instruction in WAT panel
- Auto-scroll to keep highlighted line visible (minimal scroll, not centered)
- Matching highlight colors across all three panels for visual connection

### Source Display
- Show full source file with scroll, not context window
- Line numbers always visible in gutter
- Rust syntax highlighting for source code
- When DWARF debug info missing: empty panel with "No source info available" message

### Sync Feedback
- Hex panel highlights byte range for current instruction with background color
- Clicking in any panel (hex, WAT, or source) updates cursor position
- When multiple WAT instructions map to same source line: show instruction count badge/indicator
- Matching highlight colors across panels show the mapping relationship

### Claude's Discretion
- Exact highlight colors (work with egui theme)
- Hex panel default width
- Whether to show full function bytes or context window in hex panel
- Syntax highlighting implementation approach

</decisions>

<specifics>
## Specific Ideas

- WAT panel is the primary navigation driver (j/k keys navigate instruction-by-instruction)
- This is the critical feature that differentiates wasm-poke — must work flawlessly
- Cursor sync must be immediate and never desync (addressing main TUI bug)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 04-inspector*
*Context gathered: 2026-01-26*
