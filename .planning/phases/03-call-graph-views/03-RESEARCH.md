# Phase 3: Call Graph Views - Research

**Researched:** 2026-01-26
**Domain:** egui tree widgets, call graph traversal, size visualization
**Confidence:** HIGH

## Summary

Phase 3 implements three tree views for exploring function call relationships: Call Tree (downstream calls), Callers Tree (upstream calls), and Size Tree (cumulative size impact). The codebase already has the core infrastructure: `CallGraph` with edges, `unique_cumulative_size()` function, and `SelectionState` with `expanded_nodes` for tree state.

The recommended approach is to use egui's built-in `CollapsingState` for tree rendering rather than adding a third-party dependency. This keeps the dependency tree lean, matches the existing codebase style (see `FunctionListPanel`), and provides enough flexibility for custom keyboard navigation and color-coded backgrounds.

**Primary recommendation:** Build tree panels using `CollapsingState::show_header()` with custom selection handling, leveraging the existing `CallGraph` infrastructure and adding a computed reverse graph for the Callers view.

## Standard Stack

The established libraries/tools for this domain:

### Core (Already in Project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| egui | 0.33 | Immediate mode GUI | Already used, provides CollapsingState |
| egui_dock | 0.18 | Tab/panel layout | Already used for dockable panels |
| bytesize | 1.3 | Human-readable size formatting | Already used in FunctionListPanel |

### No New Dependencies Needed
| Instead of | Could Use | Why Not |
|------------|-----------|---------|
| egui_ltreeview (0.6.1) | CollapsingState | Existing egui API sufficient, fewer deps |
| egui-arbor | CollapsingState | Overkill for our use case |
| petgraph | HashMap<u32, Vec<u32>> | Already have CallGraph structure |

**Rationale:** The codebase already has all the building blocks. `CollapsingState` provides expand/collapse with animation, `show_header()` enables custom content, and `show_toggle_button()` with custom icons handles the expand arrows. No new dependencies required.

## Architecture Patterns

### Recommended Project Structure
```
src/gui/panels/
├── mod.rs              # pub use statements
├── function_list.rs    # Existing (Phase 2)
├── call_tree.rs        # NEW: Call Tree panel
├── callers_tree.rs     # NEW: Callers Tree panel
└── size_tree.rs        # NEW: Size Tree panel
```

### Pattern 1: Computed Reverse Graph (Callers)

**What:** Build reverse edges on-demand from existing `CallGraph.edges`
**When to use:** When user needs Callers view, computed once after file load
**Example:**
```rust
/// Build reverse call graph: callee -> [callers]
pub fn build_reverse_graph(graph: &CallGraph) -> HashMap<u32, Vec<u32>> {
    let mut reverse: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&caller, callees) in &graph.edges {
        for &callee in callees {
            reverse.entry(callee).or_default().push(caller);
        }
    }
    reverse
}
```
Time complexity: O(V+E) single pass. Store result in `WasmPokeApp`.

### Pattern 2: Tree Node Path Identification

**What:** Identify tree nodes by path from root for expand state tracking
**When to use:** Track which nodes are expanded across renders
**Example:**
```rust
// Already in SelectionState:
pub expanded_nodes: HashSet<Vec<(u32, usize)>>,

// Path represents: [(function_index, position_in_parent), ...]
// Example: [(5, 0), (12, 2)] means:
//   - Root is function 5
//   - Child at position 2 is function 12
fn node_path_key(ancestors: &[(u32, usize)], func_index: u32, position: usize) -> Vec<(u32, usize)> {
    let mut path = ancestors.to_vec();
    path.push((func_index, position));
    path
}
```

### Pattern 3: Recursive Tree Rendering with Depth Limit

**What:** Render tree recursively with cycle detection and depth cutoff
**When to use:** All three tree views
**Example:**
```rust
const MAX_DEPTH: usize = 5;

fn render_tree_node(
    ui: &mut egui::Ui,
    func_index: u32,
    depth: usize,
    path: &[(u32, usize)],
    visited: &mut HashSet<u32>,  // Cycle detection
    expanded: &mut HashSet<Vec<(u32, usize)>>,
    // ... other params
) {
    // Cycle detection
    if visited.contains(&func_index) {
        ui.label(format!("{} (recursive)", name));
        return;
    }

    // Depth limit
    if depth >= MAX_DEPTH {
        ui.label("...");
        return;
    }

    visited.insert(func_index);
    // ... render node with CollapsingState
    visited.remove(&func_index);  // Backtrack for sibling branches
}
```

### Pattern 4: CollapsingState with Custom Header

**What:** Use egui's CollapsingState for expand/collapse with custom clickable header
**When to use:** Each tree node row
**Example:**
```rust
// Source: egui docs - CollapsingState::show_header
let id = ui.make_persistent_id(("tree_node", path));
let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, false);

// Custom toggle icon (arrow)
let openness = state.openness(ui.ctx());
let response = state.show_toggle_button(ui, |ui, openness, response| {
    // Custom arrow: > when closed, v when open
    paint_tree_arrow(ui, openness, response);
});

// Selectable header content (separate from toggle)
let header_response = ui.horizontal(|ui| {
    // Color background based on size
    let bg_color = size_to_color(cumulative_size, total_size);
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, bg_color);

    // Function name + size
    ui.label(&func_name);
    ui.label(format_size(cumulative_size));
});

// Handle selection on header click (not toggle)
if header_response.response.clicked() {
    selection.select_single(func_index);
}

// Show children if expanded
if state.is_open() {
    state.show_body_indented(&header_response.response, ui, |ui| {
        for (pos, &child) in children.iter().enumerate() {
            render_tree_node(ui, child, depth + 1, &new_path, ...);
        }
    });
}
```

### Pattern 5: Size-Based Color Intensity

**What:** Map cumulative size to background color intensity
**When to use:** Size Tree view, optionally Call Tree
**Example:**
```rust
fn size_to_background_color(size: u64, total: u64, base_color: Color32) -> Color32 {
    let ratio = (size as f64 / total as f64).min(1.0);
    // Logarithmic scale works better for size visualization
    let intensity = (ratio.ln_1p() / 1.0_f64.ln_1p()).max(0.0).min(1.0);

    // Interpolate alpha from 0.05 (faint) to 0.4 (strong)
    let alpha = egui::lerp(0.05..=0.4, intensity as f32);
    Color32::from_rgba_unmultiplied(
        base_color.r(),
        base_color.g(),
        base_color.b(),
        (alpha * 255.0) as u8
    )
}
```

### Anti-Patterns to Avoid

- **Re-computing reverse graph every frame:** Compute once on file load, store in app state
- **Deep recursion without limits:** Always enforce MAX_DEPTH and cycle detection
- **Storing full node state in SelectionState:** Use path-based keys, not tree structure
- **Manual tree drawing without CollapsingState:** Loses animation and consistent UX

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Expand/collapse animation | Manual animation code | `CollapsingState.openness()` | Handles tweening automatically |
| Human-readable sizes | String formatting | `ByteSize::b(n).to_string_as(false)` | Already in project, handles all units |
| Percentage formatting | Manual division | `module.percentage(&func)` | Existing `WasmModuleInfo` method |
| Tree toggle icons | Custom drawing | `CollapsingState::show_toggle_button()` | Consistent with egui style |
| Cumulative size calc | Graph traversal | `unique_cumulative_size()` | Already implemented in lib.rs |

**Key insight:** The lib.rs already has `unique_cumulative_size()` that computes exactly what the Size Tree needs - no new graph algorithms required.

## Common Pitfalls

### Pitfall 1: Infinite Recursion from Cycles

**What goes wrong:** Recursive/mutually recursive functions cause stack overflow
**Why it happens:** Call graphs can have cycles (A calls B, B calls A)
**How to avoid:**
- Track `visited: HashSet<u32>` during traversal
- Show cycle indicator ("(recursive)" marker) instead of re-expanding
- Backtrack visited set after processing children for sibling branches
**Warning signs:** Stack overflow on certain function selections

### Pitfall 2: Performance with Large Call Graphs

**What goes wrong:** UI becomes sluggish with thousands of nodes
**Why it happens:** Rendering all nodes every frame
**How to avoid:**
- Rely on CollapsingState's built-in culling (collapsed = not rendered)
- Depth limit of ~5 levels prevents explosion
- Consider virtualizing if needed (but unlikely with depth limit)
**Warning signs:** Frame rate drops when expanding large trees

### Pitfall 3: Selection State Desync

**What goes wrong:** Tree selection doesn't match function list, or vice versa
**Why it happens:** Duplicated selection state between panels
**How to avoid:**
- Use ONLY `SelectionState` from `gui/state.rs` (already exists)
- Never store selection in panel struct
- Bidirectional sync happens through shared state
**Warning signs:** Clicking tree doesn't highlight in list

### Pitfall 4: Expand State Lost on Filter

**What goes wrong:** Filtering clears all expand states
**Why it happens:** Using function index alone as expand key
**How to avoid:**
- Use path-based keys: `Vec<(func_index, position)>`
- Path is stable across filters since it's relative to root
**Warning signs:** Changing filter collapses everything

### Pitfall 5: Wrong Direction for Callers vs Call Tree

**What goes wrong:** Callers tree shows callees, or vice versa
**Why it happens:** Confusing graph edge direction
**How to avoid:**
- **Call Tree:** Follow `graph.edges[selected]` (who does selected call?)
- **Callers Tree:** Follow `reverse_graph[selected]` (who calls selected?)
- Clear naming: `callees` vs `callers`
**Warning signs:** Tree contents seem backwards

## Code Examples

Verified patterns from existing codebase and official docs:

### Existing CallGraph Usage (from lib.rs)
```rust
// Source: src/lib.rs line 365-372
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallGraph {
    pub edges: HashMap<u32, Vec<u32>>,      // caller -> [callees]
    pub has_indirect: HashMap<u32, bool>,   // has call_indirect?
}
```

### Existing Cumulative Size (from lib.rs)
```rust
// Source: src/lib.rs line 431-471
pub fn unique_cumulative_size(
    root: u32,
    module: &WasmModuleInfo,
    graph: &CallGraph,
) -> (u64, usize) {
    // Returns (total_bytes, unique_node_count)
    // Uses DFS with visited set to avoid double-counting
}
```

### Existing Selection State (from gui/state.rs)
```rust
// Source: src/gui/state.rs line 27-47
pub struct SelectionState {
    pub selected_functions: BTreeSet<u32>,
    pub last_selected: Option<u32>,
    pub focus_index: Option<u32>,
    pub instruction_cursor: usize,
    pub expanded_nodes: HashSet<Vec<(u32, usize)>>,  // Ready for trees!
}
```

### Size Formatting (from function_list.rs)
```rust
// Source: src/gui/panels/function_list.rs line 449
let size_str = ByteSize::b(func.code_size as u64).to_string_as(false);
// Produces: "1.2 KiB", "456 B", "2.3 MiB"
```

### Keyboard Navigation Pattern (from function_list.rs)
```rust
// Source: src/gui/panels/function_list.rs line 134-235
// Key handling pattern to follow:
fn handle_keyboard(
    &mut self,
    ctx: &egui::Context,
    selection: &mut SelectionState,
    // ...
) -> Option<usize> {
    // Check if filter input has focus (disable vim keys while typing)
    if self.filter_focused { return None; }

    // Get current position, check modifiers
    let (shift, ctrl) = ctx.input(|i| (i.modifiers.shift, i.modifiers.ctrl));

    // Handle j/k/arrows for navigation
    ctx.input(|i| {
        if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) { ... }
        if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) { ... }
    });
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| egui TreeView demo (nested CollapsingHeader) | CollapsingState + custom header | egui 0.20+ | Selectable tree rows |
| Manual expand/collapse state | CollapsingState persistence | Always | Automatic state persistence |

**Deprecated/outdated:**
- `CollapsingHeader` alone: Cannot make header clickable without toggle
- `egui_ltreeview` before 0.6: API changed significantly

## Open Questions

Things that couldn't be fully resolved:

1. **Multi-select in trees (union view)**
   - What we know: Context says "show union of all selected functions' call trees"
   - What's unclear: Exact UI for multiple roots - list them vertically? Merge overlapping branches?
   - Recommendation: Start with vertical list of roots, each as separate tree. Can refine later.

2. **Keyboard expand/collapse specifics**
   - What we know: "Enter/Space or arrow keys" for expand/collapse
   - What's unclear: Does Right arrow expand? Does Left collapse? What about when at leaf?
   - Recommendation: Follow standard tree conventions - Right=expand/enter, Left=collapse/parent

3. **Filter search behavior**
   - What we know: Each tree has filter box at top
   - What's unclear: Does filter match node names? Show matching subtrees? Highlight matches?
   - Recommendation: Start with simple name filtering (show nodes that match, expand parents), add highlighting

## Sources

### Primary (HIGH confidence)
- Codebase: `src/lib.rs` - CallGraph, unique_cumulative_size
- Codebase: `src/gui/state.rs` - SelectionState with expanded_nodes
- Codebase: `src/gui/panels/function_list.rs` - Keyboard navigation pattern
- [egui CollapsingState docs](https://docs.rs/egui/latest/egui/containers/collapsing_header/struct.CollapsingState.html) - API reference

### Secondary (MEDIUM confidence)
- [egui issue #417](https://github.com/emilk/egui/issues/417) - Selectable tree lines approach
- [egui_ltreeview docs](https://docs.rs/egui_ltreeview) - Alternative API reference
- [Graph transpose algorithm](https://www.geeksforgeeks.org/dsa/transpose-graph/) - Reverse graph construction

### Tertiary (LOW confidence)
- WebSearch results for egui tree patterns - Community approaches vary

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Already in codebase, no new deps needed
- Architecture: HIGH - Patterns based on existing code and official docs
- Pitfalls: HIGH - Based on common graph traversal issues and egui patterns

**Research date:** 2026-01-26
**Valid until:** 90 days (egui stable, existing code won't change)
