# Phase 4: Three-Panel Inspector - Research

**Researched:** 2026-01-26
**Domain:** egui multi-panel synchronized code/hex viewer, DWARF source mapping
**Confidence:** HIGH

## Summary

This phase implements a three-panel inspector view for wasm-poke: hex bytes, WAT disassembly, and source code, all synchronized via cursor navigation. The codebase already has significant infrastructure for this phase:

1. **Disassembly exists**: `disassemble_function_wat_lines()` in `lib.rs` returns structured `WatLine` data with byte offsets
2. **DWARF mapping exists**: `map_instr_to_source_fast()` provides instruction-to-source mapping
3. **Hex dump exists**: `hexdump()` function and `function_body_bytes()` for raw byte access
4. **SelectionState exists**: Already has `instruction_cursor: usize` field ready for this phase

The implementation should leverage egui's native capabilities for synchronized scroll, custom painting for line highlighting, and use monospace fonts throughout for code display.

**Primary recommendation:** Build custom panels using egui's ScrollArea with synchronized offsets, LayoutJob for syntax highlighting, and Painter::rect_filled for line highlighting. Do not use external crates for code editing since this is read-only display.

## Standard Stack

The established libraries/tools for this domain:

### Core (Already in Project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| egui | 0.33.3 | GUI framework | Already used, provides all needed primitives |
| egui_extras | 0.33 | Table, syntax highlighting | Already in project, has `syntect` feature |
| wasmparser | 0.241.2 | WASM parsing/disassembly | Already used for disassembly |
| addr2line/gimli | 0.25.1/0.32.3 | DWARF source mapping | Already used for source location |

### Supporting (New Feature Flag)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| syntect (via egui_extras) | (feature flag) | Rust syntax highlighting | Enable `syntect` feature in egui_extras |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Custom highlighting | egui_code_editor crate | Adds dependency; we only need read-only display |
| TextEdit | Custom Label/LayoutJob | TextEdit is for editing; we need read-only |
| External hex viewer | Custom rendering | No egui hex viewer exists; custom is simple |

**Installation:**
```toml
# Update Cargo.toml - add syntect feature to egui_extras
egui_extras = { version = "0.33", features = ["syntect"] }
```

## Architecture Patterns

### Recommended Project Structure
```
src/gui/
├── panels/
│   ├── mod.rs            # Add InspectorPanel export
│   ├── inspector.rs      # NEW: Three-panel inspector panel
│   └── ...existing panels...
└── ...
```

### Pattern 1: Inspector Panel Structure
**What:** Single `InspectorPanel` struct managing three sub-panels
**When to use:** When building the inspector view
**Example:**
```rust
// Source: Matches existing panel patterns in codebase
pub struct InspectorPanel {
    // Cached disassembly for selected function
    cached_func_index: Option<u32>,
    cached_wat_lines: Vec<WatLine>,
    cached_hex_bytes: Vec<u8>,
    cached_source_lines: Vec<SourceLine>,

    // Scroll synchronization
    scroll_offset: f32,
}
```

### Pattern 2: Synchronized Scroll with Cursor-Driven Navigation
**What:** WAT panel cursor position drives highlighting in other panels
**When to use:** For all three panels
**Example:**
```rust
// Source: egui ScrollArea docs - https://docs.rs/egui/latest/egui/containers/scroll_area/struct.ScrollArea.html
fn show_wat_panel(&mut self, ui: &mut egui::Ui, selection: &mut SelectionState) {
    let row_height = 18.0;

    egui::ScrollArea::vertical()
        .id_salt("wat_scroll")
        .vertical_scroll_offset(self.scroll_offset)
        .show_rows(ui, row_height, self.cached_wat_lines.len(), |ui, row_range| {
            for i in row_range {
                let is_current = i == selection.instruction_cursor;
                let line = &self.cached_wat_lines[i];

                // Highlight current line
                if is_current {
                    let rect = ui.available_rect_before_wrap();
                    ui.painter().rect_filled(rect, 0.0, HIGHLIGHT_COLOR);
                }

                ui.label(egui::RichText::new(&line.text).monospace());
            }
        });
}
```

### Pattern 3: Line Highlighting with Painter
**What:** Paint background color behind the current instruction line
**When to use:** For visual cursor indication across panels
**Example:**
```rust
// Source: egui Painter docs - https://docs.rs/egui/latest/egui/struct.Painter.html
fn render_highlighted_line(ui: &mut egui::Ui, text: &str, is_highlighted: bool) {
    let response = ui.horizontal(|ui| {
        if is_highlighted {
            let rect = ui.available_rect_before_wrap();
            let expanded_rect = rect.expand2(egui::vec2(4.0, 2.0));
            ui.painter().rect_filled(expanded_rect, 0.0, HIGHLIGHT_COLOR);
        }
        ui.label(egui::RichText::new(text).monospace());
    });
}
```

### Pattern 4: LayoutJob for Syntax Highlighting
**What:** Use LayoutJob for multi-colored text (WAT mnemonics, Rust source)
**When to use:** For WAT panel and source panel
**Example:**
```rust
// Source: egui LayoutJob docs - https://docs.rs/egui/latest/egui/text/struct.LayoutJob.html
fn highlight_wat_line(line: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();

    // Parse instruction and highlight differently
    let parts: Vec<&str> = line.trim().splitn(2, ' ').collect();
    if let Some(mnemonic) = parts.first() {
        job.append(
            mnemonic,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::monospace(14.0),
                color: KEYWORD_COLOR,
                ..Default::default()
            },
        );
        if let Some(operands) = parts.get(1) {
            job.append(
                &format!(" {}", operands),
                0.0,
                egui::TextFormat {
                    font_id: egui::FontId::monospace(14.0),
                    color: OPERAND_COLOR,
                    ..Default::default()
                },
            );
        }
    }
    job
}
```

### Pattern 5: Source File Caching
**What:** Cache source file contents to avoid repeated filesystem reads
**When to use:** When displaying source panel
**Example:**
```rust
use std::collections::HashMap;

pub struct SourceCache {
    files: HashMap<String, Vec<String>>,
}

impl SourceCache {
    pub fn get_lines(&mut self, path: &str) -> Option<&Vec<String>> {
        if !self.files.contains_key(path) {
            if let Ok(content) = std::fs::read_to_string(path) {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                self.files.insert(path.to_string(), lines);
            }
        }
        self.files.get(path)
    }
}
```

### Anti-Patterns to Avoid
- **Re-disassembling on every frame:** Cache WAT lines when function selection changes, not on each render
- **Calling DWARF mapping per frame:** Precompute instruction-to-source mappings when function loads
- **Using TextEdit for read-only code:** Use Label/LayoutJob instead for better performance
- **Painting highlight after text:** Paint background BEFORE text to avoid covering it

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Syntax highlighting | Custom lexer/parser | egui_extras::syntax_highlighting | Supports Rust via syntect |
| Monospace text layout | Custom font loading | egui::RichText::monospace() | Built into egui |
| Virtual scrolling | Manual row culling | ScrollArea::show_rows() | Handles virtualization automatically |
| DWARF parsing | Custom DWARF parser | addr2line crate (already used) | Complex format, well-tested crate |
| WAT disassembly | Custom disassembler | wasmparser Operator iteration (already done) | Comprehensive instruction support |
| Hex display | Complex hex grid | Simple monospace Label rows | Hex is just formatted text |

**Key insight:** The codebase already has all the data extraction functions (`disassemble_function_wat_lines`, `function_body_bytes`, `map_instr_to_source_fast`). This phase is primarily about UI rendering and synchronization.

## Common Pitfalls

### Pitfall 1: Scroll Desync Between Panels
**What goes wrong:** Hex and source panels drift from WAT cursor position
**Why it happens:** Using independent scroll state or not updating on cursor change
**How to avoid:** WAT cursor position is the single source of truth; other panels scroll to match
**Warning signs:** User navigates with j/k and non-WAT panels don't follow

### Pitfall 2: Expensive Per-Frame Computation
**What goes wrong:** UI becomes sluggish when displaying large functions
**Why it happens:** Re-running disassembly, DWARF lookups, or file reads every frame
**How to avoid:** Cache all expensive computations; only update when function selection changes
**Warning signs:** Frame rate drops when inspector panel is visible

### Pitfall 3: Highlight Drawn Over Text
**What goes wrong:** Background highlight covers/obscures the text
**Why it happens:** Calling painter.rect_filled() after adding Label
**How to avoid:** Paint background FIRST, then add text widget
**Warning signs:** Highlighted line text becomes unreadable

### Pitfall 4: N:1 Mapping Confusion
**What goes wrong:** Multiple WAT instructions map to same source line, user is confused
**Why it happens:** Compiler optimizations, loop unrolling, inlining
**How to avoid:** Show instruction count badge when multiple WAT lines map to same source
**Warning signs:** Source line seems "stuck" when navigating WAT

### Pitfall 5: Missing DWARF Graceful Handling
**What goes wrong:** Crash or panic when WASM file lacks debug info
**Why it happens:** Not handling None returns from source mapping functions
**How to avoid:** Display "No source info available" message; still show hex and WAT
**Warning signs:** App crashes on release-built WASM files

### Pitfall 6: Source File Path Resolution
**What goes wrong:** Source file not found even though DWARF has path
**Why it happens:** DWARF paths are build-time paths, may not exist on user's machine
**How to avoid:** Check file existence, show helpful message with attempted path
**Warning signs:** Source panel shows "file not found" for all functions

## Code Examples

Verified patterns from official sources and codebase:

### Hex Panel Row Rendering
```rust
// Source: lib.rs hexdump pattern adapted for egui
fn render_hex_row(
    ui: &mut egui::Ui,
    bytes: &[u8],
    offset: usize,
    highlight_range: Option<std::ops::Range<usize>>,
) {
    ui.horizontal(|ui| {
        // Offset column
        ui.label(egui::RichText::new(format!("{:04x}:", offset)).monospace().weak());

        // Hex bytes
        for (i, byte) in bytes.iter().enumerate() {
            let byte_offset = offset + i;
            let is_highlighted = highlight_range
                .as_ref()
                .map(|r| r.contains(&byte_offset))
                .unwrap_or(false);

            let text = egui::RichText::new(format!("{:02x} ", byte)).monospace();
            if is_highlighted {
                ui.label(text.background_color(HIGHLIGHT_COLOR));
            } else {
                ui.label(text);
            }
        }
    });
}
```

### Source Panel with egui_extras Syntax Highlighting
```rust
// Source: egui_extras syntax_highlighting docs
fn render_source_panel(
    ui: &mut egui::Ui,
    source_lines: &[String],
    current_line: Option<u32>,
    theme: &egui_extras::syntax_highlighting::CodeTheme,
) {
    let code = source_lines.join("\n");

    // Use egui_extras syntax highlighting for Rust
    egui_extras::syntax_highlighting::code_view_ui(
        ui,
        theme,
        &code,
        "rs",
    );

    // Note: For line highlighting with selection, may need custom approach
    // combining ScrollArea::show_rows with per-line rendering
}
```

### Keyboard Navigation Handler
```rust
// Source: Matches existing handle_keyboard pattern in function_list.rs
fn handle_keyboard(
    &mut self,
    ctx: &egui::Context,
    selection: &mut SelectionState,
    wat_line_count: usize,
) -> bool {
    if wat_line_count == 0 {
        return false;
    }

    let current = selection.instruction_cursor;
    let new_cursor = ctx.input(|i| {
        if i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown) {
            Some((current + 1).min(wat_line_count - 1))
        } else if i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp) {
            Some(current.saturating_sub(1))
        } else if i.key_pressed(egui::Key::G) && !i.modifiers.shift {
            Some(0)
        } else if i.key_pressed(egui::Key::G) && i.modifiers.shift {
            Some(wat_line_count - 1)
        } else {
            None
        }
    });

    if let Some(new) = new_cursor {
        if new != current {
            selection.instruction_cursor = new;
            return true;  // Cursor changed, need to update scroll
        }
    }
    false
}
```

### Computing Instruction Byte Ranges for Hex Highlighting
```rust
// Source: WatLine already has offset field from lib.rs
fn compute_instruction_byte_range(
    wat_lines: &[WatLine],
    current_index: usize,
) -> Option<std::ops::Range<usize>> {
    let current = wat_lines.get(current_index)?;
    let next = wat_lines.get(current_index + 1);

    let start = current.offset;
    let end = next.map(|n| n.offset).unwrap_or(start + 4); // Estimate if last

    Some(start..end)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| egui ScrollArea scroll_to_me | scroll_offset() + show_rows() | egui 0.24+ | Better control for sync |
| Manual syntax highlighting | egui_extras::syntax_highlighting | egui_extras 0.26+ | Less code, better results |
| Custom font handling | RichText::monospace() | Always available | Simpler API |

**Deprecated/outdated:**
- egui `scroll_to_me()` can affect multiple ScrollAreas; use explicit offset for sync
- Custom LayoutJob for Rust highlighting: egui_extras syntect feature is better

## Open Questions

Things that couldn't be fully resolved:

1. **Click-to-navigate in Source Panel**
   - What we know: Context says clicking in any panel updates cursor
   - What's unclear: Mapping clicked source line back to WAT instruction is complex (1:N)
   - Recommendation: For source panel clicks, find first WAT instruction mapping to that line

2. **Instruction Count Badge Position**
   - What we know: Should show when multiple WAT instructions map to same source
   - What's unclear: Best visual placement (inline with source? separate indicator?)
   - Recommendation: Show count in parentheses after line number in source gutter

3. **Horizontal Scroll for Long Lines**
   - What we know: Some WAT lines and source lines may be very long
   - What's unclear: Should panels independently scroll horizontally?
   - Recommendation: Enable horizontal scroll per-panel, don't sync horizontal scroll

## Sources

### Primary (HIGH confidence)
- egui 0.33.3 documentation - ScrollArea, Painter, LayoutJob, RichText
- egui_extras 0.33.3 documentation - syntax_highlighting module
- Existing codebase: `lib.rs` (WatLine, disassembly, DWARF mapping), `gui/state.rs` (SelectionState)

### Secondary (MEDIUM confidence)
- [egui ScrollArea API](https://docs.rs/egui/latest/egui/containers/scroll_area/struct.ScrollArea.html) - scroll_offset, show_rows
- [egui LayoutJob API](https://docs.rs/egui/latest/egui/text/struct.LayoutJob.html) - multi-colored text
- [egui Painter API](https://docs.rs/egui/latest/egui/struct.Painter.html) - rect_filled for backgrounds
- [egui Visuals](https://docs.rs/egui/latest/egui/style/struct.Visuals.html) - code_bg_color, selection colors

### Tertiary (LOW confidence)
- GitHub discussions on synchronized scroll - various approaches mentioned
- egui_code_editor crate patterns - for line number rendering ideas

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in use or are feature flags
- Architecture: HIGH - patterns match existing codebase, well-documented egui APIs
- Pitfalls: HIGH - based on existing codebase bugs (TUI sync issues) and egui docs

**Research date:** 2026-01-26
**Valid until:** 60 days - egui 0.33 is stable; patterns are well-established
