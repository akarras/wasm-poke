# Domain Pitfalls

**Domain:** egui-based WebAssembly analysis tool (native + web)
**Researched:** 2026-01-26
**Confidence:** HIGH (verified against official docs, GitHub issues, project-specific analysis)

## Critical Pitfalls

Mistakes that cause rewrites or major issues.

### Pitfall 1: Scattered State Leading to Desync (Current TUI Problem)

**What goes wrong:** State for synchronized views (function selection, cursor position, scroll offsets, filter state) is stored in multiple places without a clear single source of truth. When one view updates state, other views may not reflect the change, or worse, they read stale cached data.

**Why it happens:** In the current TUI (`main.rs` lines 184-252), state is distributed across:
- `selected` / `tree_selected` (selection indices)
- `wat_cursor` / `wat_scroll` / `source_scroll` (view-specific cursors)
- `wat_lines` (cached disassembly)
- `source_span_cache` / `source_file_cache` (cached mappings)
- `inspect_cache` (cached WAT lines per function)
- `manual_source_file` (user override)

When `goto` is triggered (`main.rs` lines 671-738), it updates `graph_root`, `expanded`, `tree_selected`, `wat_scroll`, `source_scroll`, `wat_cursor`, and `wat_lines` - but the hex view offset derives from `wat_cursor` at render time. If any of these updates are missed or happen in the wrong order, views desync.

**Consequences:**
- Source mapping shows wrong file/line for selected function
- Hex view highlights wrong byte range after goto
- Filter changes cause selection to point to wrong function index
- Users lose trust in the tool's accuracy

**Prevention:**
1. **Single source of truth:** Define one authoritative state struct:
   ```rust
   struct SelectionState {
       function_index: u32,
       instruction_cursor: usize,  // within current function
   }
   ```
2. **Derived state is always derived:** Hex offset, source location, scroll positions are computed from `SelectionState`, never stored separately
3. **State transitions are atomic:** When changing function, update `SelectionState` once; all views re-derive
4. **Use egui's single-frame guarantee:** In immediate mode, if you derive state from the source of truth each frame, you cannot desync

**Detection:**
- Warning sign: Multiple fields that represent "which instruction am I looking at"
- Warning sign: Code that updates scroll position independently from selection
- Warning sign: Caches keyed by function index that outlive function changes

**Phase to address:** Phase 1 (Core UI shell) - establish state model before any views exist

---

### Pitfall 2: ID Collisions Causing Widget State Corruption

**What goes wrong:** egui tracks widget state (drag position, scroll offset, collapse state) by ID. If two widgets share an ID in the same frame, egui may apply state from one widget to another, causing unpredictable behavior.

**Why it happens:** egui generates IDs automatically for most widgets, but:
- Dynamically generated lists without stable keys
- Multiple identical widgets in loops
- Copy-pasted widget code without changing identifiers
- Using function names as IDs when names can repeat (demangled names aren't unique)

**Consequences:**
- Scroll positions jump unexpectedly
- Clicking one item affects another
- Collapse/expand state applies to wrong sections
- "Double use of widget ID" errors appear on screen

**Prevention:**
1. **Use stable, unique IDs for stateful widgets:**
   ```rust
   // Bad: ID derived from potentially duplicate name
   ui.collapsing(function.best_name(), |ui| { ... });

   // Good: ID derived from unique function index
   let id = ui.make_persistent_id(function.index);
   egui::CollapsingHeader::new(function.best_name())
       .id_salt(id)
       .show(ui, |ui| { ... });
   ```
2. **Push unique ID for list items:**
   ```rust
   for (i, func) in functions.iter().enumerate() {
       ui.push_id(func.index, |ui| {
           // All widgets inside get unique IDs
       });
   }
   ```
3. **Avoid using display names as IDs** - use function index (u32) which is guaranteed unique

**Detection:**
- On-screen "Double use of widget ID" errors
- Scroll positions that "snap" when clicking elsewhere
- Interaction affecting wrong widgets

**Phase to address:** Phase 2 (List views) - when implementing function list with potentially duplicate names

---

### Pitfall 3: Frame Delay Breaking Immediate Feedback

**What goes wrong:** In immediate mode, a widget cannot both check for interaction AND react to it in the same frame. The interaction is recorded in frame N, but your code only sees it in frame N+1. This creates 1-2 frame lag that feels unresponsive for cursor navigation.

**Why it happens:** egui processes input at the start of the frame, then you render widgets. A click on a button is detected *after* the button was already drawn. Your response happens next frame.

**Consequences:**
- Selection highlight lags behind cursor movement
- Hover states feel sluggish
- Fast keyboard navigation (holding j/k) feels imprecise
- Source line highlight appears to "chase" the cursor

**Prevention:**
1. **Accept the one-frame delay for most interactions** - users won't notice
2. **For critical hover highlighting, use `Context::read_response`:**
   ```rust
   // Check if this widget WILL be interacted with
   let id = ui.next_auto_id();
   let will_hover = ui.ctx().read_response(id)
       .map(|r| r.hovered())
       .unwrap_or(false);

   // Style based on predicted interaction
   let style = if will_hover { highlight_style } else { normal_style };
   ui.label(RichText::new(text).style(style));
   ```
3. **For keyboard navigation, process input before rendering:**
   ```rust
   // Process navigation keys first
   if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
       self.cursor += 1;
   }
   // Then render with updated cursor
   ```
4. **Use `request_discard` sparingly** for first-frame layout jitter (costs extra render pass)

**Detection:**
- Selection highlight visibly lagging behind rapid key presses
- Hover effects that feel "sticky"

**Phase to address:** Phase 3 (Inspection panel) - when implementing synchronized cursor movement

---

### Pitfall 4: WASM Memory Limits with Large Wasm Files

**What goes wrong:** Browser tabs have a ~2GB memory limit. Loading a large Wasm file (100MB+) into memory, then duplicating it for parsing, caching disassembly, caching source mappings, and caching hex views can exhaust available memory.

**Why it happens:** The current implementation loads the entire wasm file into `wasm_bytes: Vec<u8>`, then:
- Parser creates its own data structures
- Call graph is built in memory
- Each inspected function caches `Vec<WatLine>`
- Source spans are cached per function
- Source file contents are cached

For a 50MB Wasm file with 10,000 functions, memory can easily reach 500MB-1GB.

**Consequences:**
- Browser tab crashes on large files
- Mobile browsers fail immediately
- Desktop native version works, web version fails
- No warning before crash

**Prevention:**
1. **Lazy loading:** Don't disassemble all functions upfront; only disassemble on inspect
2. **LRU cache for disassembly:** Keep only N recently viewed functions in cache
3. **Streaming file access on web:** Use `Blob.stream()` for incremental reading
4. **Memory budget monitoring:**
   ```rust
   #[cfg(target_arch = "wasm32")]
   fn check_memory_pressure() -> bool {
       // Check if approaching limits
   }
   ```
5. **Size warning UI:** Warn users when loading files > 50MB on web

**Detection:**
- Tab crashes with no error message
- Performance degradation as more functions are inspected
- Memory usage visible in browser dev tools climbing continuously

**Phase to address:** Phase 4 (File loading) - implement memory-conscious loading strategy

---

### Pitfall 5: Async File Operations Blocking UI on WASM

**What goes wrong:** File access APIs in browsers are async. Using blocking patterns (like Rust's `std::fs`) doesn't compile for WASM, and naive async patterns (like pollster) panic on WASM with "condvar wait not supported."

**Why it happens:** Native file access is synchronous. Browser file access (File API, drag-and-drop) is async. The same Rust code cannot handle both without abstraction.

**Consequences:**
- App freezes while loading files
- WASM builds fail to compile or panic at runtime
- Users think app is broken when loading large files

**Prevention:**
1. **Use `poll_promise` for async operations:**
   ```rust
   #[cfg(target_arch = "wasm32")]
   {
       let promise = poll_promise::Promise::spawn_local(async {
           // Async file loading
       });
       // Poll in update() without blocking
   }
   ```
2. **Abstract file loading behind trait:**
   ```rust
   trait FileLoader {
       fn load(&self, ctx: &Context) -> Option<LoadingState>;
   }
   // Native: blocking impl
   // WASM: async polling impl
   ```
3. **Show loading progress UI** while file is being read
4. **Use eframe's built-in file handling** for drag-and-drop

**Detection:**
- Compilation errors mentioning `std::fs` on WASM
- Runtime panic: "condvar wait not supported"
- UI freezes during file operations

**Phase to address:** Phase 4 (File loading) - design platform-agnostic file handling

---

## Moderate Pitfalls

Mistakes that cause delays or technical debt.

### Pitfall 6: CPU Spin in Continuous Rendering Mode

**What goes wrong:** egui can run in "continuous mode" (redraw every frame at 60Hz) or "reactive mode" (redraw only on input/animation). Using continuous mode when not needed wastes CPU and drains laptop batteries.

**Why it happens:** Calling `ctx.request_repaint()` every frame, or having always-animating elements, keeps the app redrawing even when idle.

**Prevention:**
1. **Default to reactive mode** - egui handles this automatically
2. **Only request repaint when needed:**
   ```rust
   if self.has_pending_animation() {
       ctx.request_repaint();
   }
   ```
3. **Use `request_repaint_after` for periodic updates:**
   ```rust
   // Update every second for elapsed time display
   ctx.request_repaint_after(Duration::from_secs(1));
   ```
4. **Check focus state:**
   ```rust
   if !ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
       // Minimized/unfocused - reduce repaint frequency
       ctx.request_repaint_after(Duration::from_millis(100));
   }
   ```

**Phase to address:** Phase 5 (Performance) - profile and optimize rendering

---

### Pitfall 7: Virtualization Failures with Large Function Lists

**What goes wrong:** Rendering 10,000+ table rows in egui causes frame drops and memory issues. Without virtualization, each row creates widgets even when not visible.

**Why it happens:** The naive approach iterates all items and creates widgets for each:
```rust
// Bad: creates 10,000 labels
for func in &module.functions {
    ui.label(&func.name);
}
```

**Prevention:**
1. **Use `TableBuilder` with `rows()` or `heterogeneous_rows()`:**
   ```rust
   TableBuilder::new(ui)
       .column(Column::auto())
       .body(|body| {
           body.rows(row_height, functions.len(), |mut row| {
               let func = &functions[row.index()];
               row.col(|ui| ui.label(&func.name));
           });
       });
   ```
2. **Use `ScrollArea::show_rows()` for lists:**
   ```rust
   let row_height = 20.0;
   ScrollArea::vertical().show_rows(
       ui,
       row_height,
       functions.len(),
       |ui, row_range| {
           for i in row_range {
               ui.label(&functions[i].name);
           }
       }
   );
   ```
3. **Be aware of f32 precision limits:** Above 2M rows, scrolling gets jittery due to f32 precision

**Phase to address:** Phase 2 (List views) - implement from the start with virtualization

---

### Pitfall 8: Filter State Causing Index Mismatch

**What goes wrong:** Storing selection as an index into a filtered list (`indices[selected]`), then changing the filter, causes the selection to point to a different function.

**Why it happens (from current TUI):**
```rust
// Current approach - indices is a filtered/sorted view
self.indices = all.iter().filter(...).collect();
self.selected = self.selected.min(self.indices.len() - 1);
// selected=5 now points to different function!
```

**Prevention:**
1. **Store selection as function ID, not index:**
   ```rust
   struct SelectionState {
       selected_function: Option<u32>,  // function index, not list position
   }
   ```
2. **After filter change, find position of selected function:**
   ```rust
   fn list_position(&self, function_index: u32) -> Option<usize> {
       self.filtered_indices.iter().position(|&i| i == function_index)
   }
   ```
3. **If selected function is filtered out, either:**
   - Clear selection
   - Keep selection but grey it out in detail view
   - Auto-select nearest visible function

**Phase to address:** Phase 2 (List views) - design selection model correctly from start

---

### Pitfall 9: Mixing Business Logic with Render Logic

**What goes wrong:** Performing expensive operations (parsing, searching, file I/O) inside the `update()` function blocks the UI thread and causes frame drops.

**Why it happens:** It's convenient to do work where you need the results:
```rust
fn update(&mut self, ctx: &egui::Context, ...) {
    // Bad: blocks rendering
    let results = expensive_search(&self.data, &self.query);
    ui.label(format!("{} results", results.len()));
}
```

**Prevention:**
1. **Move expensive work to background threads:**
   ```rust
   fn update(&mut self, ctx: &egui::Context, ...) {
       // Check for results, don't compute
       if let Some(results) = self.search_results.take() {
           self.cached_results = results;
       }
       ui.label(format!("{} results", self.cached_results.len()));
   }
   ```
2. **Use channels for thread communication:**
   ```rust
   if let Ok(result) = self.result_receiver.try_recv() {
       self.data = result;
   }
   ```
3. **Show loading indicators** for operations > 100ms

**Phase to address:** Phase 4 (File loading) - design async loading from start

---

### Pitfall 10: Platform-Specific Code Leaking Into Core Logic

**What goes wrong:** #[cfg(target_arch)] scattered throughout makes code hard to test and maintain. Platform differences become implicit and easy to miss.

**Prevention:**
1. **Create platform abstraction layer:**
   ```rust
   // platform.rs
   pub trait Platform {
       fn load_file(&self, path: &str) -> impl Future<Output = Result<Vec<u8>>>;
       fn save_file(&self, path: &str, data: &[u8]) -> impl Future<Output = Result<()>>;
       fn open_url(&self, url: &str);
   }
   ```
2. **Implement per platform:**
   ```rust
   #[cfg(not(target_arch = "wasm32"))]
   mod native;
   #[cfg(target_arch = "wasm32")]
   mod web;
   ```
3. **Keep core logic platform-agnostic:**
   ```rust
   struct App<P: Platform> {
       platform: P,
       // No cfg attributes in App
   }
   ```

**Phase to address:** Phase 1 (Core UI shell) - establish platform abstraction

---

## Minor Pitfalls

Mistakes that cause annoyance but are fixable.

### Pitfall 11: Inconsistent Styling Between Native and Web

**What goes wrong:** Fonts, sizes, and colors render differently on native vs web due to different DPI scaling and browser CSS defaults.

**Prevention:**
- Use egui's built-in scaling (`ctx.set_pixels_per_point()`)
- Test on both platforms regularly
- Use relative sizing (percentage, em) not absolute pixels

**Phase to address:** Phase 5 (Performance and polish)

---

### Pitfall 12: Forgetting to Call request_repaint for Background Updates

**What goes wrong:** After receiving data from a background thread, the UI doesn't update until the user moves the mouse because egui is in reactive mode.

**Prevention:**
```rust
if let Ok(data) = self.receiver.try_recv() {
    self.data = data;
    ctx.request_repaint();  // Don't forget!
}
```

**Phase to address:** Phase 4 (File loading)

---

### Pitfall 13: Scroll Position Reset on List Update

**What goes wrong:** When the function list is re-filtered or re-sorted, scroll position jumps to top.

**Prevention:**
- Remember scroll position as fraction of list (not absolute pixels)
- Or remember top visible item and restore to same item

**Phase to address:** Phase 2 (List views)

---

## Phase-Specific Warnings

| Phase | Topic | Likely Pitfall | Mitigation |
|-------|-------|----------------|------------|
| 1 | Core shell | State architecture | Define single source of truth immediately |
| 2 | List views | ID collisions, virtualization | Use function index for IDs, use TableBuilder |
| 3 | Inspection | Frame delay, cursor sync | Process input before render, derive positions |
| 4 | File loading | WASM async, memory limits | poll_promise, LRU caches, streaming |
| 5 | Polish | CPU usage, platform differences | Reactive mode, thorough cross-platform testing |

## Sources

### Official/Authoritative
- [egui Context documentation](https://docs.rs/egui/latest/egui/struct.Context.html)
- [egui ID documentation](https://docs.rs/egui/latest/egui/struct.Id.html)
- [eframe documentation](https://docs.rs/eframe/latest/eframe/)
- [egui_extras TableBuilder](https://docs.rs/egui_extras/latest/egui_extras/struct.TableBuilder.html)

### GitHub Issues and Discussions
- [UI State management discussion](https://github.com/emilk/egui/discussions/7553)
- [Frame delay, input lag issue](https://github.com/emilk/egui/issues/1904)
- [Table frame lag issue](https://github.com/emilk/egui/issues/1874)
- [ScrollArea jitter with large rows](https://github.com/emilk/egui/issues/1391)
- [Double use of widget ID](https://github.com/emilk/egui/issues/4940)
- [WASM file upload handling](https://github.com/emilk/egui/issues/2091)
- [Drag and drop incremental reading](https://github.com/emilk/egui/issues/4654)
- [High CPU usage when focused](https://github.com/emilk/egui/issues/2008)

### Community Resources
- [IMGUI paradigm wiki](https://github.com/ocornut/imgui/wiki/About-the-IMGUI-paradigm)
- [Building cross-platform apps with egui - LogRocket](https://blog.logrocket.com/building-cross-platform-gui-apps-rust-using-egui/)
- [Best practices for large scrollable areas](https://github.com/emilk/egui/discussions/2443)

### Project-Specific Analysis
- Current TUI source: `C:\Users\chw11\code\wasm-poke\src\main.rs` (lines 184-252 for App struct, 671-738 for goto logic)
