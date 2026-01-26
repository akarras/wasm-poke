# Phase 2: Function List View - Research

**Researched:** 2026-01-26
**Domain:** egui virtualized tables, keyboard input, selection handling
**Confidence:** HIGH

## Summary

Phase 2 implements a function list view with sorting, filtering, and vim-style keyboard navigation. The standard approach uses `egui_extras::TableBuilder` for virtualized rendering (handles 10K+ functions efficiently), the existing `globset` crate for pattern matching (already in codebase), and custom key handling via `ctx.input()` for vim bindings.

The existing codebase already has:
- `FunctionInfo` struct with `best_name()`, `code_size`, and call graph data
- `filter_functions()` and `function_matches()` using globset for case-insensitive glob matching
- `sorted_by_size()` for descending size sorting
- `SelectionState` with `selected_function: Option<u32>` (using function index, not list position)

**Primary recommendation:** Use `egui_extras::TableBuilder` with `body.rows()` for virtualization, `TableRow::select()` for highlighting, and custom keyboard handling in the update loop before rendering the table.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| egui_extras | 0.33.3 | TableBuilder for virtualized tables | Official egui companion, handles 10K+ rows |
| globset | 0.4 (existing) | Glob pattern matching | Already used in lib.rs for filtering |
| bytesize | 1.3+ | Human-readable size formatting | 3M+ downloads/month, simple API |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none needed) | - | - | - |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| bytesize | humansize, size | bytesize is most popular, simplest API |
| egui_extras::Table | Custom ScrollArea + rows | Table handles virtualization automatically |
| egui-selectable-table | egui_extras | External crate adds complexity, egui_extras now has selection built-in |

**Installation:**
```bash
cargo add egui_extras@0.33 bytesize
```

Note: egui_extras 0.33 matches eframe 0.33 already in Cargo.toml.

## Architecture Patterns

### Recommended Project Structure
```
src/
├── gui/
│   ├── app.rs              # WasmPokeApp (existing)
│   ├── state.rs            # SelectionState (existing, needs multi-select extension)
│   ├── tabs.rs             # TabKind enum (existing)
│   └── panels/
│       └── function_list.rs  # NEW: FunctionListPanel implementation
└── lib.rs                   # filter_functions, sorted_by_size (existing)
```

### Pattern 1: FunctionListPanel as Separate Module
**What:** Extract function list rendering into dedicated module with its own state
**When to use:** When panel complexity exceeds simple match arm in TabViewer
**Example:**
```rust
// src/gui/panels/function_list.rs
pub struct FunctionListPanel {
    filter_text: String,
    sort_column: SortColumn,
    sort_ascending: bool,
    // Cached filtered/sorted indices for performance
    cached_indices: Vec<usize>,
    cache_dirty: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SortColumn {
    Name,
    Size,
    Calls,
}

impl FunctionListPanel {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        module: &WasmModuleInfo,
        call_graph: Option<&CallGraph>,
        selection: &mut SelectionState,
    ) {
        // Filter input
        // Table with header
        // Keyboard handling
    }
}
```

### Pattern 2: Centralized Selection with Multi-Select
**What:** Extend SelectionState to support multi-select with last-selected tracking
**When to use:** Required by CONTEXT.md decisions (multi-select, last selected drives inspector)
**Example:**
```rust
// src/gui/state.rs
use std::collections::BTreeSet;

#[derive(Default)]
pub struct SelectionState {
    /// Selected function indices (BTreeSet for ordered iteration)
    pub selected_functions: BTreeSet<u32>,
    /// Most recently selected function (drives inspector in Phase 4)
    pub last_selected: Option<u32>,
    /// Focus position for keyboard navigation (may differ from selection)
    pub focus_index: Option<u32>,
    // ... existing fields
}

impl SelectionState {
    pub fn select_single(&mut self, index: u32) {
        self.selected_functions.clear();
        self.selected_functions.insert(index);
        self.last_selected = Some(index);
        self.focus_index = Some(index);
    }

    pub fn toggle_select(&mut self, index: u32) {
        if self.selected_functions.contains(&index) {
            self.selected_functions.remove(&index);
        } else {
            self.selected_functions.insert(index);
            self.last_selected = Some(index);
        }
        self.focus_index = Some(index);
    }

    pub fn extend_select(&mut self, from: u32, to: u32) {
        // Shift+click range selection
        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        for i in start..=end {
            self.selected_functions.insert(i);
        }
        self.last_selected = Some(to);
        self.focus_index = Some(to);
    }
}
```

### Pattern 3: TableBuilder with Row Selection
**What:** Use egui_extras::TableBuilder with sense() and select() for interactive rows
**When to use:** All list views with selection
**Example:**
```rust
use egui_extras::{TableBuilder, Column};

TableBuilder::new(ui)
    .striped(true)
    .resizable(true)
    .sense(egui::Sense::click())  // Enable row interaction
    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
    .column(Column::auto().at_least(200.0).clip(true))  // Name
    .column(Column::auto().at_least(80.0))               // Size
    .column(Column::auto().at_least(60.0))               // Calls
    .header(20.0, |mut header| {
        header.col(|ui| {
            if ui.selectable_label(sort_col == SortColumn::Name, "Name").clicked() {
                // Toggle sort
            }
        });
        // ... more columns
    })
    .body(|body| {
        body.rows(ROW_HEIGHT, filtered_indices.len(), |mut row| {
            let idx = row.index();
            let func_idx = filtered_indices[idx];
            let func = &module.functions[func_idx];
            let is_selected = selection.selected_functions.contains(&func.index);

            row.set_selected(is_selected);  // Highlight entire row

            row.col(|ui| {
                let name = func.best_name();
                ui.add(egui::Label::new(&name).truncate())
                    .on_hover_text(&name);  // Full name in tooltip
            });
            row.col(|ui| {
                ui.label(format_size(func.code_size));
            });
            row.col(|ui| {
                ui.label(format!("{}", call_count));
            });
        });
    });
```

### Pattern 4: Vim-Style Keyboard Navigation
**What:** Handle j/k/g/G/Ctrl+d/u keys before rendering, update focus/selection
**When to use:** Required by CONTEXT.md decisions
**Example:**
```rust
fn handle_keyboard(
    ctx: &egui::Context,
    selection: &mut SelectionState,
    filtered_count: usize,
    visible_rows: usize,
) {
    if filtered_count == 0 {
        return;
    }

    let current = selection.focus_index.unwrap_or(0) as usize;
    let mut new_focus = current;
    let shift = ctx.input(|i| i.modifiers.shift);

    ctx.input(|i| {
        // j or ArrowDown: move down
        if i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown) {
            new_focus = (current + 1).min(filtered_count - 1);
        }
        // k or ArrowUp: move up
        if i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp) {
            new_focus = current.saturating_sub(1);
        }
        // gg or Home: jump to top
        if i.key_pressed(egui::Key::Home) {
            new_focus = 0;
        }
        // G or End: jump to bottom
        if i.key_pressed(egui::Key::G) && !i.modifiers.shift || i.key_pressed(egui::Key::End) {
            new_focus = filtered_count - 1;
        }
        // Ctrl+d: half-page down
        if i.key_pressed(egui::Key::D) && i.modifiers.ctrl {
            new_focus = (current + visible_rows / 2).min(filtered_count - 1);
        }
        // Ctrl+u: half-page up
        if i.key_pressed(egui::Key::U) && i.modifiers.ctrl {
            new_focus = current.saturating_sub(visible_rows / 2);
        }
        // H: top of visible screen
        // M: middle of visible screen
        // L: bottom of visible screen
        // (These require tracking scroll offset - implement in Phase 2 refinement)
    });

    if new_focus != current {
        let func_index = /* map new_focus to function index via filtered_indices */;
        if shift {
            selection.extend_select(selection.last_selected.unwrap_or(0), func_index);
        } else {
            selection.select_single(func_index);
        }
    }
}
```

### Pattern 5: Scroll to Keep Selection Visible
**What:** Use TableBuilder::scroll_to_row() to keep focused row in view
**When to use:** After any navigation that changes focus
**Example:**
```rust
// Before calling .body(), scroll to focused row
let scroll_to = if selection_changed {
    selection.focus_index.and_then(|func_idx| {
        filtered_indices.iter().position(|&i| module.functions[i].index == func_idx)
    })
} else {
    None
};

let mut table = TableBuilder::new(ui)
    .striped(true)
    // ... other settings
;

if let Some(row) = scroll_to {
    table = table.scroll_to_row(row, Some(egui::Align::Center));
}

table.body(|body| { /* ... */ });
```

### Anti-Patterns to Avoid
- **Storing list position instead of function index:** Filter changes would desync selection
- **Re-sorting/filtering every frame:** Cache filtered indices, invalidate on filter/sort change
- **Using widget name as egui ID:** Function names may collide; use function index
- **Blocking keyboard input during text edit:** Check if filter TextEdit has focus before handling vim keys

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Virtualized table rendering | Custom ScrollArea with manual row clipping | egui_extras::TableBuilder with body.rows() | Handles viewport calculation, row recycling |
| Human-readable sizes | format!("{:.1} KB", bytes as f64 / 1024.0) | bytesize crate | Edge cases (0 bytes, petabytes), consistent formatting |
| Glob pattern matching | Regex or custom wildcard parsing | globset (already in codebase) | Battle-tested, case-insensitive, efficient |
| Name demangling | Custom Rust symbol parsing | rustc-demangle (already in codebase) | Handles all Rust mangling versions |

**Key insight:** The codebase already has filter_functions() and sorted_by_size() in lib.rs - reuse them rather than reimplementing.

## Common Pitfalls

### Pitfall 1: Table Row Selection Not Working on Labels
**What goes wrong:** Rows only selectable when clicking empty space, not on text labels
**Why it happens:** Labels absorb clicks by default in egui pre-0.29; fixed in later versions but still can occur
**How to avoid:** Use `TableBuilder::sense(Sense::click())` and check `row.response().clicked()` for entire row
**Warning signs:** Selection works inconsistently depending on where you click in the row

### Pitfall 2: Keyboard Input Consumed by TextEdit
**What goes wrong:** Vim keys (j/k/g) type into filter input instead of navigating list
**Why it happens:** TextEdit captures keyboard focus and consumes key events
**How to avoid:** Check `!ui.memory(|m| m.has_focus(filter_response.id))` before handling vim keys
**Warning signs:** Pressing 'j' adds 'j' to filter text instead of moving down

### Pitfall 3: Filter Invalidates Selection Position
**What goes wrong:** Selected row appears at wrong position or disappears after filtering
**Why it happens:** Storing selection as list position (e.g., row 5) instead of function index
**How to avoid:** Store `selected_function: Option<u32>` as function index, map to display position dynamically
**Warning signs:** Selection jumps unexpectedly when typing in filter

### Pitfall 4: Scroll Position Resets on Filter Change
**What goes wrong:** Table scrolls to top every time filter text changes
**Why it happens:** TableBuilder scroll state is per-id; filter change may create new table instance
**How to avoid:** Use stable `id_salt()` for table, explicitly manage scroll with `scroll_to_row()` after filter
**Warning signs:** User loses place in list when refining filter

### Pitfall 5: Performance Degrades with Large Functions Lists
**What goes wrong:** UI becomes sluggish with 10K+ functions
**Why it happens:** Re-filtering and re-sorting every frame, or using `body.row()` instead of `body.rows()`
**How to avoid:** Cache filtered/sorted indices, invalidate only on input change; use `body.rows()` for virtualization
**Warning signs:** Frame rate drops when scrolling or typing

### Pitfall 6: TableBuilder max_scroll_height Clips Content
**What goes wrong:** Table doesn't fill available space, has unexpected cutoff at ~800px
**Why it happens:** Default `max_scroll_height` is 800.0 in egui_extras
**How to avoid:** Call `.max_scroll_height(f32::INFINITY)` or calculate based on available height
**Warning signs:** Table has scrollbar even when container is tall enough to show all rows

## Code Examples

Verified patterns from official sources:

### TableBuilder with Virtualized Rows
```rust
// Source: https://docs.rs/egui_extras/latest/egui_extras/struct.TableBuilder.html
use egui_extras::{TableBuilder, Column};

TableBuilder::new(ui)
    .striped(true)
    .resizable(true)
    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
    .column(Column::auto().at_least(100.0).clip(true).resizable(true))
    .column(Column::auto().at_least(60.0))
    .column(Column::remainder())
    .max_scroll_height(f32::INFINITY)  // Fill available space
    .header(20.0, |mut header| {
        header.col(|ui| { ui.strong("Name"); });
        header.col(|ui| { ui.strong("Size"); });
        header.col(|ui| { ui.strong("Calls"); });
    })
    .body(|body| {
        let row_height = 18.0;
        body.rows(row_height, num_rows, |mut row| {
            let row_index = row.index();
            row.col(|ui| { /* ... */ });
            row.col(|ui| { /* ... */ });
            row.col(|ui| { /* ... */ });
        });
    });
```

### Keyboard Input Detection
```rust
// Source: https://docs.rs/egui/latest/egui/struct.Context.html
if ctx.input(|i| i.key_pressed(egui::Key::J)) {
    // Move selection down
}

if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
    // Half-page down
}

// Consume key to prevent further handling
ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::J));
```

### ByteSize Formatting
```rust
// Source: https://lib.rs/crates/bytesize
use bytesize::ByteSize;

fn format_size(bytes: u32) -> String {
    ByteSize::b(bytes as u64).display().iec_short().to_string()
    // "1.5K", "256B", "2.3M"
}
```

### Label with Truncation and Tooltip
```rust
// Source: https://docs.rs/egui/latest/egui/widgets/struct.Label.html
let name = func.best_name();
let response = ui.add(egui::Label::new(&name).truncate());
response.on_hover_text(&name);  // Show full name on hover
```

### Row Selection and Highlighting
```rust
// Source: https://github.com/emilk/egui/pull/3347
TableBuilder::new(ui)
    .sense(egui::Sense::click())
    .body(|body| {
        body.rows(row_height, count, |mut row| {
            let idx = row.index();
            let is_selected = selection.contains(&idx);
            row.set_selected(is_selected);  // Apply highlight to all cells

            row.col(|ui| { /* ... */ });

            let response = row.response();
            if response.clicked() {
                // Handle click - update selection state
            }
        });
    });
```

### Focus Check Before Keyboard Handling
```rust
// Avoid handling vim keys when TextEdit has focus
let filter_response = ui.add(egui::TextEdit::singleline(&mut self.filter_text));
let filter_focused = filter_response.has_focus();

// Only handle navigation if filter not focused
if !filter_focused {
    if ctx.input(|i| i.key_pressed(egui::Key::J)) {
        // Navigate down
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Individual row.row() calls | body.rows() virtualization | egui_extras 0.20+ | Required for 10K+ rows |
| Manual click detection per column | TableBuilder::sense() + row.response() | egui 0.28+ | Simpler row interaction |
| Custom row backgrounds | TableRow::set_selected() | egui 0.28+ | Built-in selection highlighting |
| Label::wrap(false) | Label::truncate() | egui 0.28+ | Cleaner text clipping API |

**Deprecated/outdated:**
- `Style::wrap` field: Use `Style::wrap_mode` instead
- Passing bool to `Label::wrap()`: Now takes no arguments, use `Label::truncate()` for clipping

## Open Questions

Things that couldn't be fully resolved:

1. **H/M/L (screen position) navigation**
   - What we know: Requires tracking visible row range from scroll state
   - What's unclear: How to get visible row indices from TableBuilder scroll position
   - Recommendation: Implement j/k/g/G first, add H/M/L as enhancement if feasible

2. **Scroll animation smoothness**
   - What we know: TableBuilder::scroll_to_row() works, animate_scrolling() exists
   - What's unclear: Default animation behavior and customization options
   - Recommendation: Use defaults first, tune if animation feels wrong

3. **Call count computation**
   - What we know: CallGraph has `edges` mapping src -> Vec<dst>
   - What's unclear: Whether to count incoming calls (callers) or outgoing calls (callees)
   - Recommendation: Count incoming calls (more useful for "what calls this function")

## Sources

### Primary (HIGH confidence)
- [egui_extras TableBuilder docs](https://docs.rs/egui_extras/latest/egui_extras/struct.TableBuilder.html) - API reference
- [egui_extras TableBody docs](https://docs.rs/egui_extras/latest/egui_extras/struct.TableBody.html) - Virtualization with rows()
- [egui_extras Column docs](https://docs.rs/egui_extras/0.33.3/egui_extras/struct.Column.html) - Column sizing API
- [egui InputState docs](https://docs.rs/egui/latest/egui/struct.InputState.html) - Keyboard input handling
- [egui Key enum docs](https://docs.rs/egui/latest/egui/enum.Key.html) - All available key codes
- [bytesize crate](https://lib.rs/crates/bytesize) - Human-readable size formatting

### Secondary (MEDIUM confidence)
- [Table row selection PR #3347](https://github.com/emilk/egui/pull/3347) - Implementation of set_selected() and sense()
- [egui Label truncation](https://docs.rs/egui/latest/egui/widgets/struct.Label.html) - TextWrapMode::Truncate
- [egui scroll_to_rect](https://docs.rs/egui/latest/egui/containers/scroll_area/struct.ScrollArea.html) - Programmatic scrolling

### Tertiary (LOW confidence)
- WebSearch results on vim navigation patterns - No egui-specific examples found

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - egui_extras is official, API verified via docs.rs
- Architecture: HIGH - Patterns based on existing codebase structure and egui idioms
- Pitfalls: HIGH - Based on documented issues and known egui behavior

**Research date:** 2026-01-26
**Valid until:** 2026-02-26 (egui ecosystem is stable, 30-day validity)
