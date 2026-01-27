# Phase 04 Plan 02: Hex Panel and Source Panel Summary

**One-liner:** Three-panel inspector with hex bytes (left), WAT (center), and source (right), all showing synchronized highlights for current instruction.

## What Was Built

Extended InspectorPanel to display three synchronized panels side-by-side:

1. **Source file caching infrastructure:**
   - `cached_source_mappings` - Vec of SourceLocation for each WAT line
   - `source_file_cache` - HashMap caching loaded source files
   - `cached_source_path` - Primary source file (by frequency)
   - `cached_source_lines` - Lines of current source file
   - `cached_current_source_line` - Line number for current instruction
   - `current_source_line()` helper method

2. **Three-panel layout:**
   - Hex panel (~20% width) with 8 bytes per row
   - WAT panel (~45% width) with instruction disassembly
   - Source panel (remaining width) with line numbers

3. **Hex panel features:**
   - Offset gutter showing hex addresses (e.g., `0000:`)
   - Bytes displayed as two-digit hex values
   - Current instruction bytes highlighted with selection color
   - `instruction_byte_range()` calculates byte range from WAT offsets

4. **Source panel features:**
   - Filename header (just filename, not full path)
   - Line number gutter (4-digit, right-aligned)
   - Current source line highlighted
   - "No source info available" when DWARF info missing

5. **Panel synchronization:**
   - All panels sync on `instruction_cursor` position
   - Keyboard navigation (j/k/g/G) updates all panels
   - Click on WAT line updates hex and source highlights

## Commit Log

| Hash | Type | Description |
|------|------|-------------|
| 8143623 | feat | Add source file caching to InspectorPanel |
| 2cc0b08 | feat | Implement three-panel layout with Hex and Source panels |

## Verification

- [x] cargo check passes
- [x] cargo build succeeds
- [x] Three-panel layout renders with correct proportions
- [x] show_hex_panel function exists
- [x] Hex panel shows bytes with offset gutter
- [x] Source panel shows line numbers
- [x] Current instruction bytes highlighted in hex panel
- [x] Current source line highlighted in source panel
- [x] Missing DWARF shows graceful message
- [x] Uses function_body_bytes from wasm_poke
- [x] Uses map_instr_to_source_fast from wasm_poke

## Deviations from Plan

None - plan executed exactly as written.

## Files Changed

```
src/gui/panels/inspector.rs    (+280 lines, -7 lines)
```

## Key Implementation Details

### Source Mapping Strategy
- Uses `map_instr_to_source_fast()` for each WAT line during cache update
- Determines primary source file by counting occurrences in mappings
- Caches source file content to avoid repeated filesystem reads

### Hex Byte Range Calculation
- Uses `WatLine.offset` to determine instruction start
- End is next instruction's offset, or +4 bytes for last instruction
- Validates range is within cached bytes before highlighting

### Panel Layout
- Uses `ui.horizontal()` with `allocate_ui_with_layout()`
- Fixed proportions: 20% hex, 45% WAT, ~35% source
- Separators between panels for visual clarity

## Next Phase Readiness

Ready for 04-03 (if needed for additional inspector features):
- Three-panel infrastructure complete
- Source mapping cached per function
- Highlighting synchronized across panels

## Performance Notes

- Source mappings computed once on function change (not per frame)
- Source file cached in HashMap to avoid repeated reads
- ScrollArea with show_rows for virtualized rendering in all panels
