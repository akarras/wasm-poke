# Architecture Patterns for egui-based Wasm Analysis Tool

**Project:** wasm-poke GUI Migration (TUI to egui)
**Researched:** 2026-01-26
**Confidence:** HIGH (based on official docs, production examples, and ecosystem patterns)

## Executive Summary

The egui ecosystem provides well-established patterns for building complex multi-view developer tools. The key insight is that **immediate mode eliminates traditional state synchronization bugs** - the challenge shifts from "keeping views in sync" to "organizing your data model for efficient querying."

For wasm-poke, the recommended architecture is:
1. **Centralized Application State** - Single `App` struct owning all data and selection state
2. **Query-on-Demand Views** - Panels query state each frame rather than maintaining local caches
3. **Shared Mutable References** - Pass `&mut self` to view functions, no Arc/Mutex needed
4. **egui_dock for Layout** - Provides docking, tabs, and flexible panel arrangement

This mirrors the production-proven pattern used by Rerun.io (also built with egui) and aligns with egui's design philosophy.

---

## Recommended Architecture

### High-Level Component Diagram

```
+-----------------------------------------------------------------------+
|                           WasmPokeApp                                  |
|  +------------------+  +------------------+  +-------------------+      |
|  | Analysis Model   |  | Selection State  |  | UI Configuration  |     |
|  |------------------|  |------------------|  |-------------------|     |
|  | WasmModuleInfo   |  | selected_func    |  | dock_state        |     |
|  | CallGraph        |  | selected_instr   |  | view_modes        |     |
|  | wasm_bytes       |  | expanded_nodes   |  | filter_text       |     |
|  | DWARF context    |  | inspect_cursor   |  | raw_names_mode    |     |
|  +------------------+  +------------------+  +-------------------+      |
+-----------------------------------------------------------------------+
                                |
                                v
+-----------------------------------------------------------------------+
|                        View Layer (Panels)                             |
|  +-------------------+  +-------------------+  +-------------------+    |
|  | FunctionListView  |  | CallTreeView      |  | InspectorView     |   |
|  | reads: functions  |  | reads: call_graph |  | reads: wat_lines  |   |
|  | writes: selection |  | writes: selection |  | reads: source     |   |
|  +-------------------+  | writes: expanded  |  | writes: cursor    |   |
|                         +-------------------+  +-------------------+    |
+-----------------------------------------------------------------------+
```

### Component Boundaries

| Component | Responsibility | Communicates With |
|-----------|---------------|-------------------|
| `WasmPokeApp` | Top-level state container, frame orchestration | egui Context, all views |
| `AnalysisModel` | Parsed wasm data (read-only after load) | All views (read-only) |
| `SelectionState` | Current selections, cursor positions | All views (read/write) |
| `FunctionListView` | Render function table, handle selection | SelectionState |
| `CallTreeView` | Render expandable call graph | SelectionState, AnalysisModel |
| `InspectorView` | Three-panel hex/wat/source display | SelectionState, AnalysisModel |
| `DockState` | Tab/panel layout management | egui_dock framework |

---

## State Management Patterns

### Pattern 1: Centralized State with View Functions

**Recommendation: Use this pattern.** It matches egui's design and eliminates sync bugs.

```rust
struct WasmPokeApp {
    // Analysis data (immutable after load)
    module: WasmModuleInfo,
    call_graph: CallGraph,
    wasm_bytes: Vec<u8>,

    // Selection state (mutable, shared across views)
    selected_function: Option<u32>,
    expanded_nodes: HashSet<Vec<(u32, usize)>>,
    inspect_cursor: usize,

    // UI state
    dock_state: DockState<TabKind>,
    filter: String,
    raw_names: bool,
}

impl eframe::App for WasmPokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // All views receive &mut self - they share state naturally
        DockArea::new(&mut self.dock_state)
            .show(ctx, &mut WasmPokeTabViewer { app: self });
    }
}
```

**Why this works:**
- egui redraws every frame, so all views see the same state
- Selection changes in one panel immediately visible to others (next frame)
- No callbacks, no channels, no synchronization primitives needed
- Explicit data flow: views read from `self`, write to `self`

### Pattern 2: Selection Flows Downstream

Selection state should flow **one direction**: from user interaction to shared state to all views.

```rust
// In FunctionListView
if ui.selectable_label(is_selected, &func_name).clicked() {
    app.selected_function = Some(func.index);
    app.inspect_cursor = 0;  // Reset cursor on new selection
}

// In InspectorView (same frame or next)
if let Some(func_idx) = app.selected_function {
    let wat_lines = disassemble_function_wat_lines(&app.wasm_bytes, func_idx);
    // Render inspector for selected function
}
```

### Pattern 3: Derived State Computation

Compute derived data on-demand rather than caching. Cache only when profiling shows need.

```rust
// GOOD: Compute when needed
fn get_display_name(&self, func: &FunctionInfo) -> String {
    if self.raw_names {
        func.raw_name.clone().unwrap_or_else(|| format!("func[{}]", func.index))
    } else {
        func.best_name()
    }
}

// GOOD: Cache expensive operations lazily
fn get_wat_lines(&mut self, func_index: u32) -> &[WatLine] {
    self.wat_cache.entry(func_index).or_insert_with(|| {
        disassemble_function_wat_lines(&self.wasm_bytes, func_index)
            .unwrap_or_default()
    })
}
```

---

## View Synchronization

### How It Works in Immediate Mode

Traditional retained-mode GUIs require explicit synchronization:
```
User clicks row -> Event handler -> Update state -> Notify listeners -> Redraw
```

egui eliminates this complexity:
```
User clicks row -> Modify state -> Next frame -> All views read same state
```

**Key insight from Rerun.io architecture:** They explicitly chose immediate mode to avoid "state synchronization bugs" that plagued their earlier designs.

### Synchronization Points in wasm-poke

| User Action | State Changed | Views Affected |
|-------------|--------------|----------------|
| Select function in list | `selected_function` | CallTree highlights, Inspector shows function |
| Expand node in tree | `expanded_nodes` | Tree re-renders with children visible |
| Move cursor in WAT | `inspect_cursor` | Hex highlights byte, Source highlights line |
| Change filter | `filter` | FunctionList and CallTree filter results |
| Toggle raw names | `raw_names` | All name displays update |

### Multi-View Cursor Synchronization (Existing TUI Bug Fix)

The current TUI has sync bugs in the three-panel inspector. The fix in egui:

```rust
struct InspectorState {
    cursor: usize,           // Current instruction index
    // Derived on each frame:
    // - hex_offset = wat_lines[cursor].offset
    // - source_line = wat_lines[cursor].src.line
}

fn render_inspector(&mut self, ui: &mut Ui) {
    let cursor = self.inspector_state.cursor;

    // All three panes derive from same cursor
    let wat_lines = self.get_wat_lines(self.selected_function.unwrap());
    let current_line = &wat_lines[cursor.min(wat_lines.len().saturating_sub(1))];

    // Hex pane: scroll to current_line.offset
    // WAT pane: highlight cursor row
    // Source pane: highlight current_line.src.line
}
```

**Why this fixes sync bugs:** Single source of truth (`cursor`) with derived positions computed fresh each frame.

---

## Layout Architecture with egui_dock

### Recommended Approach

Use `egui_dock` for flexible panel layout with tabs.

```rust
enum TabKind {
    FunctionList,
    CallTree,
    SizeTree,
    Inspector,
}

struct WasmPokeTabViewer<'a> {
    app: &'a mut WasmPokeApp,
}

impl TabViewer for WasmPokeTabViewer<'_> {
    type Tab = TabKind;

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        match tab {
            TabKind::FunctionList => "Functions".into(),
            TabKind::CallTree => "Call Graph".into(),
            TabKind::SizeTree => "Size Tree".into(),
            TabKind::Inspector => "Inspector".into(),
        }
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab {
            TabKind::FunctionList => function_list_view(ui, self.app),
            TabKind::CallTree => call_tree_view(ui, self.app),
            TabKind::SizeTree => size_tree_view(ui, self.app),
            TabKind::Inspector => inspector_view(ui, self.app),
        }
    }
}
```

### Default Layout

```rust
fn default_dock_state() -> DockState<TabKind> {
    let mut state = DockState::new(vec![TabKind::FunctionList]);

    // Split: left panel (list/tree), right panel (inspector)
    let [left, right] = state.main_surface_mut()
        .split_right(NodeIndex::root(), 0.6, vec![TabKind::Inspector]);

    // Add tabs to left panel
    state.main_surface_mut()
        .set_focused_node_and_surface((SurfaceIndex::main(), left));
    state.push_to_focused_leaf(TabKind::CallTree);
    state.push_to_focused_leaf(TabKind::SizeTree);

    state
}
```

---

## Patterns to Follow

### Pattern: View Functions Over Methods

Organize views as standalone functions receiving app state, not methods on separate structs.

```rust
// RECOMMENDED: View functions
fn function_list_view(ui: &mut Ui, app: &mut WasmPokeApp) {
    // Direct access to all app state
    for (i, func) in app.module.functions.iter().enumerate() {
        let selected = app.selected_function == Some(func.index);
        if ui.selectable_label(selected, app.get_display_name(func)).clicked() {
            app.selected_function = Some(func.index);
        }
    }
}

// AVOID: Separate view structs with their own state
struct FunctionListView {
    selected: Option<u32>,  // Duplicate state = sync bugs
}
```

### Pattern: ID-Based Widget State

egui persists widget state (scroll position, collapse state) using IDs. Use explicit IDs for dynamic content.

```rust
fn call_tree_view(ui: &mut Ui, app: &mut WasmPokeApp) {
    let root = app.graph_root.unwrap_or(0);

    // Unique ID per tree root ensures scroll/collapse state is preserved
    ui.push_id(format!("call_tree_{}", root), |ui| {
        render_tree_node(ui, app, root, 0);
    });
}
```

### Pattern: Lazy Data Loading

Don't load everything upfront. Load on-demand with caching.

```rust
impl WasmPokeApp {
    fn ensure_wat_lines(&mut self, func_index: u32) {
        if !self.wat_cache.contains_key(&func_index) {
            let lines = disassemble_function_wat_lines(&self.wasm_bytes, func_index)
                .unwrap_or_default();
            self.wat_cache.insert(func_index, lines);
        }
    }

    fn ensure_source_file(&mut self, path: &str) {
        if !self.source_cache.contains_key(path) {
            if let Ok(content) = std::fs::read_to_string(path) {
                self.source_cache.insert(path.to_string(), content);
            }
        }
    }
}
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Separate State Per View

**What:** Each view maintains its own copy of selection/display state.
**Why bad:** State gets out of sync, duplicate storage, complex synchronization.
**Instead:** Single state struct passed to all views.

### Anti-Pattern 2: Arc<Mutex<State>> for UI Threading

**What:** Using concurrent primitives for state shared between panels.
**Why bad:** egui is single-threaded; this adds complexity without benefit.
**Instead:** Simple `&mut self` references in immediate mode.

### Anti-Pattern 3: Callback-Based View Updates

**What:** Using channels or callbacks to notify views of changes.
**Why bad:** Unnecessary complexity; immediate mode handles this automatically.
**Instead:** Modify state directly, let next frame reflect changes.

### Anti-Pattern 4: Deep Nesting of UI Components

**What:** Complex hierarchies of UI component structs.
**Why bad:** Hard to pass state through, obscures data flow.
**Instead:** Flat view functions that all receive `&mut WasmPokeApp`.

---

## Build Order Recommendations

Based on dependencies and complexity, suggested implementation order:

### Phase 1: Foundation
1. **App shell with egui_dock** - Basic window, empty tabs
2. **Load wasm and display stats** - Verify data model works
3. **Simple function list** - Table with selection

### Phase 2: Core Views
4. **Function list with filtering** - Port filter logic from TUI
5. **Basic inspector (WAT only)** - Single-pane disassembly view
6. **Call tree view** - Expandable tree with existing data structures

### Phase 3: Three-Panel Inspector
7. **Hex pane** - Synchronized with WAT cursor
8. **Source pane** - DWARF mapping, file loading
9. **Cursor synchronization** - All three panes track same position

### Phase 4: Polish
10. **Size tree view** - Cumulative size calculations
11. **Keyboard navigation** - Port TUI keybindings
12. **Layout persistence** - Save/restore dock configuration

**Rationale:**
- Phase 1 establishes the framework before content
- Phase 2 proves the state-sharing pattern works
- Phase 3 tackles the hardest sync problem (existing TUI bug)
- Phase 4 adds features that depend on working infrastructure

---

## Migration Path from Current TUI

### What to Keep

The existing codebase has valuable components to preserve:

| TUI Component | egui Equivalent | Notes |
|--------------|-----------------|-------|
| `App` struct state | `WasmPokeApp` struct | Same fields, different rendering |
| `WasmModuleInfo`, `CallGraph` | Unchanged | Analysis layer stays the same |
| `refresh_indices()` | Unchanged | Filter logic reusable |
| `compute_tree_view()` | Refactor for egui tree | Virtualization may differ |
| `disassemble_function_wat_lines()` | Unchanged | Pure data transformation |
| DWARF/source mapping | Unchanged | Pure data transformation |

### What to Replace

| TUI Component | egui Replacement |
|--------------|------------------|
| `ratatui::Terminal` | `eframe::run_native()` |
| `draw_ui()`, `draw_table()` | egui panel/widget calls |
| Key event handling | egui input handling or `ui.input()` |
| `TableState`, `ListState` | egui_extras Table or egui_dock |
| Manual scroll management | egui handles this automatically |

### State Field Migration

```rust
// Current TUI App struct -> egui WasmPokeApp

// Keep as-is:
wasm_path: String,
module: WasmModuleInfo,
wasm_bytes: Vec<u8>,
call_graph: CallGraph,
name_map: HashMap<u32, String>,
source_span_cache: HashMap<u32, Vec<SourceSpan>>,
source_file_cache: HashMap<String, String>,
inspect_cache: HashMap<u32, Vec<WatLine>>,

// Adapt:
indices: Vec<usize>,           // Keep for filtered/sorted view
selected: usize,               // -> selected_function: Option<u32>
filter: String,                // Keep
graph_root: Option<u32>,       // Keep
expanded: HashSet<...>,        // Keep
tree_selected: usize,          // -> incorporated into selection model

// New:
dock_state: DockState<TabKind>,  // egui_dock layout
```

---

## Scalability Considerations

| Concern | At 100 functions | At 10K functions | At 1M functions |
|---------|------------------|------------------|-----------------|
| Function list | Direct render | Virtual scroll | Virtual scroll + filtering |
| Call tree | Full expand ok | Lazy expand | Virtualized tree |
| WAT lines | Full in memory | Per-function cache | Streaming/pagination |
| Source files | Load all | LRU cache | Load on demand |

**egui_extras provides virtual scrolling** via `Table` and `TableBuilder`. For very large function counts, use this instead of rendering all rows.

---

## Sources

### Official Documentation
- [egui Documentation](https://docs.rs/egui/latest/egui/)
- [eframe Application Framework](https://deepwiki.com/membrane-io/egui/5-eframe-application-framework)
- [egui Context and UI System](https://deepwiki.com/membrane-io/egui/3.1-context-and-ui)

### egui_dock
- [egui_dock Documentation](https://docs.rs/egui_dock/latest/egui_dock/)
- [egui_dock GitHub](https://github.com/Adanos020/egui_dock)

### Production Examples
- [Rerun.io Architecture](https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md) - Large-scale egui application
- [egui-arbor](https://github.com/kyjohnso/egui-arbor) - Tree widget patterns

### Community Patterns
- [Sharing State Between Views Discussion](https://github.com/emilk/egui/discussions/276)
- [egui Layout System](https://deepwiki.com/emilk/egui/4.3-layout-system)
- [Multi-Page Applications Discussion](https://github.com/emilk/egui/discussions/5302)
