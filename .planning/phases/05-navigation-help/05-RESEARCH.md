# Phase 5: Inspector Navigation & Help - Research

**Researched:** 2026-01-26
**Domain:** egui keyboard navigation, WebAssembly instruction help, navigation history
**Confidence:** HIGH

## Summary

This phase adds two features to the Inspector panel: (1) goto navigation from call instructions to target functions with back-navigation support, and (2) instruction help text displayed on hover or in a panel.

The codebase already has a comprehensive `help.rs` module with 100+ instruction explanations covering all core WebAssembly 1.0 instructions. The existing WatLine structure captures instruction text that can be parsed to extract the mnemonic and any target function index. The navigation pattern follows a simple stack-based history.

**Primary recommendation:** Add navigation history to SelectionState, detect Enter key on call instructions to push current function and navigate to target, add Backspace/Escape to pop history. Display help text via egui's `on_hover_text` on each WAT instruction line.

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| egui | 0.33 | UI framework | Already in use, provides `on_hover_text`, `key_pressed` |
| wasmparser | 0.241.2 | Wasm parsing | Already extracts call targets via `Operator::Call { function_index }` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| (none new) | - | - | All functionality achievable with existing deps |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Parsing WatLine.text for call target | Store target in WatLine struct | Struct change is cleaner but more invasive; text parsing is simpler for Phase 5 |
| Hover tooltip | Dedicated help panel | Tooltip is standard UX, help panel requires layout changes |

**Installation:**
```bash
# No new dependencies required
```

## Architecture Patterns

### Recommended Additions to State

```rust
// In SelectionState:
pub struct SelectionState {
    // ... existing fields ...

    /// Navigation history stack for goto/back navigation.
    /// Each entry is (function_index, instruction_cursor) to restore full position.
    pub navigation_history: Vec<(u32, usize)>,
}
```

### Pattern 1: Navigation History Stack

**What:** Push (func_index, instruction_cursor) before navigating to a new function, pop to go back.

**When to use:** When user presses Enter on a call instruction.

**Example:**
```rust
// Source: standard IDE navigation pattern
fn navigate_to_function(&mut self, target_index: u32) {
    // Push current position before navigating
    if let Some(current) = self.selection.last_selected {
        self.selection.navigation_history.push((current, self.selection.instruction_cursor));
    }
    // Navigate to target
    self.selection.select_single(target_index);
    self.selection.instruction_cursor = 0;
}

fn navigate_back(&mut self) -> bool {
    if let Some((func_index, cursor)) = self.selection.navigation_history.pop() {
        self.selection.select_single(func_index);
        self.selection.instruction_cursor = cursor;
        true
    } else {
        false
    }
}
```

### Pattern 2: Extract Call Target from WAT Line

**What:** Parse "call 123" text to extract target function index.

**When to use:** When checking if current instruction is navigable.

**Example:**
```rust
// Source: existing WatLine.text format in lib.rs
fn extract_call_target(wat_text: &str) -> Option<u32> {
    let trimmed = wat_text.trim();
    if trimmed.starts_with("call ") && !trimmed.starts_with("call_indirect") {
        // Format is "call 123" - extract the number
        trimmed.strip_prefix("call ")?.trim().parse().ok()
    } else {
        None
    }
}
```

### Pattern 3: Hover Help Text

**What:** Show instruction explanation on hover using egui's `on_hover_text`.

**When to use:** When rendering WAT instruction rows.

**Example:**
```rust
// Source: egui docs.rs/egui/latest/egui/struct.Response.html
fn show_wat_row(ui: &mut egui::Ui, line: &WatLine) {
    let response = ui.label(&line.text);

    // Extract mnemonic from instruction text
    if let Some(help) = extract_mnemonic(&line.text)
        .and_then(|m| wasm_poke::help::get_instruction_help(m))
    {
        response.on_hover_text(help);
    }
}

fn extract_mnemonic(text: &str) -> Option<&str> {
    // "  i32.add" -> "i32.add"
    // "  call 123" -> "call"
    let trimmed = text.trim();
    if trimmed.starts_with(";;") || trimmed.starts_with("(") || trimmed == ")" {
        return None; // Skip comments and syntax
    }
    trimmed.split_whitespace().next()
}
```

### Anti-Patterns to Avoid
- **Storing full function data in history:** Only store indices; function data lives in module and persists
- **Navigating to imported functions:** Imported functions have no body; check before navigating
- **Forgetting to reset cursor on navigation:** Always set instruction_cursor = 0 when changing functions (already done in update_cache)

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Instruction help text | Manual documentation | Existing `help.rs` module | Already has 100+ instructions documented |
| Tooltip display | Custom popup system | `response.on_hover_text()` | egui handles positioning, timing, overflow |
| Key detection | Raw event parsing | `ctx.input(\|i\| i.key_pressed(Key::Enter))` | Standard egui pattern, handles modifiers |

**Key insight:** The existing codebase already has the hard parts (help.rs, WatLine parsing, keyboard handling patterns). This phase is mostly wiring.

## Common Pitfalls

### Pitfall 1: Navigating to Non-Existent Functions

**What goes wrong:** User presses Enter on `call 999` but function 999 is an import with no body.

**Why it happens:** Wasm function indices include imports before defined functions.

**How to avoid:** Check if target index exists in `module.functions` before navigating.

**Warning signs:** Error on navigation, empty inspector panel.

### Pitfall 2: Infinite History Growth

**What goes wrong:** History stack grows without bound as user navigates.

**Why it happens:** No limit on history size.

**How to avoid:** Cap history size (e.g., 50 entries) and drop oldest when full.

**Warning signs:** Memory growth during long sessions.

### Pitfall 3: Tooltip Obscures Content

**What goes wrong:** Help tooltip covers adjacent instructions.

**Why it happens:** Long help text with small panel width.

**How to avoid:** Use `on_hover_text` (positions at cursor) not `show_tooltip` (may overlap). Keep help text concise.

**Warning signs:** User can't see nearby instructions while hovering.

### Pitfall 4: Lost Position on Back Navigation

**What goes wrong:** User navigates back but cursor is at wrong instruction.

**Why it happens:** Only storing function index, not instruction cursor.

**How to avoid:** Store tuple (func_index, instruction_cursor) in history.

**Warning signs:** User loses context when navigating back.

## Code Examples

Verified patterns from official sources and existing codebase:

### Detecting Enter Key Press
```rust
// Source: existing inspector.rs handle_keyboard pattern
fn handle_keyboard_navigation(
    &self,
    ctx: &egui::Context,
    selection: &mut SelectionState,
    module: &WasmModuleInfo,
) -> NavigationAction {
    // Only process if this tab is active
    if selection.active_tab != TabKind::Inspector {
        return NavigationAction::None;
    }

    // Check for Enter key on call instruction
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        if let Some(target) = self.get_current_call_target(selection.instruction_cursor) {
            // Verify target exists
            if module.functions.iter().any(|f| f.index == target) {
                return NavigationAction::GotoFunction(target);
            }
        }
    }

    // Check for Backspace to go back
    if ctx.input(|i| i.key_pressed(egui::Key::Backspace)) {
        return NavigationAction::GoBack;
    }

    NavigationAction::None
}

enum NavigationAction {
    None,
    GotoFunction(u32),
    GoBack,
}
```

### Checking for Call Instruction
```rust
// Source: WatLine format from lib.rs disassemble_function_wat_lines
fn get_current_call_target(&self, cursor: usize) -> Option<u32> {
    let line = self.cached_wat_lines.get(cursor)?;
    let trimmed = line.text.trim();

    // "call 123" but not "call_indirect"
    if trimmed.starts_with("call ") && !trimmed.starts_with("call_indirect") {
        trimmed
            .strip_prefix("call ")?
            .trim()
            .parse()
            .ok()
    } else {
        None
    }
}
```

### Adding Hover Help
```rust
// Source: egui Response::on_hover_text
// Must be called on the Response from adding the widget
let (rect, response) = ui.allocate_exact_size(
    egui::vec2(ui.available_width(), ROW_HEIGHT),
    egui::Sense::click(),
);

// ... draw the instruction text ...

// Add hover help based on mnemonic
if let Some(help_text) = self.get_instruction_help(&line.text) {
    response.on_hover_text(help_text);
}

fn get_instruction_help(&self, wat_text: &str) -> Option<&'static str> {
    let trimmed = wat_text.trim();
    // Skip comments and syntax markers
    if trimmed.starts_with(";;") || trimmed.starts_with("(") || trimmed == ")" {
        return None;
    }
    // Extract first word as mnemonic
    let mnemonic = trimmed.split_whitespace().next()?;
    wasm_poke::help::get_instruction_help(mnemonic)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No navigation | Stack-based goto/back | This phase | Enables exploring call chains without losing context |
| No help | Hover tooltips | This phase | Users understand unfamiliar Wasm instructions |

**Deprecated/outdated:**
- None - this is new functionality

## Existing Assets

The codebase already has key components:

### help.rs Module (100+ instructions covered)
```rust
// Already exists at src/help.rs
pub fn get_instruction_help(mnemonic: &str) -> Option<&'static str>
```

Covers:
- Control flow: unreachable, nop, block, loop, if, else, end, br, br_if, br_table, return, call, call_indirect, drop, select
- Variables: local.get/set/tee, global.get/set
- Memory: all load/store variants, memory.size, memory.grow
- Constants: i32/i64/f32/f64.const
- Comparisons: all eq/ne/lt/gt/le/ge variants
- Numeric: all add/sub/mul/div/rem variants, bitwise ops, float ops
- Conversions: all wrap/extend/trunc/convert/demote/promote/reinterpret variants

### Missing from help.rs
After comparing with wasmparser Operator enum, these may need additions:
- SIMD instructions (v128.*, i8x16.*, etc.) - can be added with generic help
- Atomic instructions (memory.atomic.*, etc.) - can be added with generic help
- Reference type instructions (ref.null, ref.is_null, etc.)
- Table instructions (table.get, table.set, etc.)
- Bulk memory (memory.copy, memory.fill, etc.)
- Exception handling (try, catch, throw, etc.)

**Recommendation:** Add catch-all for unknown instructions: "WebAssembly instruction. See spec for details."

### WatLine Structure
```rust
// Already exists in lib.rs
pub struct WatLine {
    pub text: String,       // "  call 123" - includes indentation
    pub offset: usize,      // byte offset for hex highlighting
    pub indent: usize,      // nesting level
    pub src: Option<SourceLocation>, // DWARF mapping
}
```

Text format is consistent:
- `"  call 123"` - call with target index
- `"  i32.add"` - operation without argument
- `"  i32.const 42"` - operation with value
- `"  ;; comment"` - comments
- `"(func"`, `")"` - syntax markers

## Open Questions

Things that couldn't be fully resolved:

1. **Should help show for comments/syntax markers?**
   - What we know: Comments (;; ...) and syntax ((func, )) are not instructions
   - What's unclear: Should they show help or be silently skipped?
   - Recommendation: Skip silently - no help for non-instructions

2. **Cap on navigation history?**
   - What we know: Infinite history could leak memory
   - What's unclear: What's a reasonable limit?
   - Recommendation: 50 entries is typical for IDE navigation stacks

3. **Keyboard binding for "back"?**
   - What we know: Backspace is intuitive but may conflict with text input
   - What's unclear: Inspector has no text input, so probably safe
   - Recommendation: Use Backspace (or Alt+Left if needed)

## Sources

### Primary (HIGH confidence)
- Existing codebase: src/help.rs - 100+ instruction definitions
- Existing codebase: src/lib.rs - WatLine structure and disassembly format
- Existing codebase: src/gui/panels/inspector.rs - keyboard handling pattern
- [egui Response docs](https://docs.rs/egui/latest/egui/struct.Response.html) - on_hover_text API

### Secondary (MEDIUM confidence)
- [wasmparser Operator enum](https://docs.rs/wasmparser/latest/wasmparser/enum.Operator.html) - instruction list
- [MDN WebAssembly Reference](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference) - instruction categories

### Tertiary (LOW confidence)
- Browser navigation history pattern (general UX knowledge)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - no new dependencies, existing egui patterns
- Architecture: HIGH - simple stack pattern, existing keyboard handling
- Pitfalls: HIGH - common patterns with clear mitigations
- Help text: HIGH - existing help.rs module covers 100+ instructions

**Research date:** 2026-01-26
**Valid until:** 2026-03-26 (stable, no fast-moving dependencies)
