# Codebase Concerns

**Analysis Date:** 2026-01-26

## Tech Debt

**Global Memory Leak via Box::leak() in DWARF Initialization:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\lib.rs` line 534 uses `Box::leak()` to obtain `'static` lifetimes for DWARF section data. This intentionally leaks memory that is never freed.
- Files: `src/lib.rs` (lines 534, 492-564)
- Impact: Each WASM file analyzed will permanently allocate and leak memory for all DWARF debug sections (potentially megabytes per file). In long-running processes or batch analysis, this accumulates unbounded.
- Fix approach: Replace `OnceLock<Mutex<DwarfCtx>>` with lazy initialization that uses reference-counted or scoped lifetimes. Consider using `&'a` with explicit lifetime management, or storing owned `Vec<u8>` in the struct instead of borrowed slices. May require refactoring `Addr2LineContext` initialization.

**Unbounded Tree View Memory Allocation in Graph Mode:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\main.rs` lines 844-847 and 934-1034 document memory concerns. The `compute_tree_view_filtered()` function generates ALL possible rows in memory before filtering, then discards most of them.
- Files: `src/main.rs` (lines 835-1034, especially 934-1034)
- Impact: Large call graphs with filtering enabled can cause OOM. Comments at lines 845, 934 acknowledge this is "memory intensive." A call graph with thousands of nodes and deep nesting generates exponentially more intermediate rows.
- Fix approach: Implement lazy filtering that walks the tree once, emitting rows only if they or their descendants match the filter. Use a pre-pass to mark matchable subtrees, then walk once to yield only those. Avoid materializing all_rows.

**Unbounded Cache Growth in Inspect Mode:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\main.rs` lines 229, 239, 284-294 maintain three unbounded HashMaps and one Vec as caches: `source_span_cache`, `source_file_cache`, `inspect_cache`, `name_map`.
- Files: `src/main.rs` (lines 184-252, 411-448, 284-294)
- Impact: As users inspect many functions across a session, all WAT disassemblies, source file contents, and source span mappings accumulate in memory. No eviction policy exists. Inspecting hundreds of functions leaks memory.
- Fix approach: Implement a bounded LRU cache (e.g., max 20 cached functions) that evicts oldest entries. Or cache to disk with tempfile for large source files. Document expected memory usage.

**Duplicate Parser Code and DWARF Scanning:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\lib.rs` contains two near-identical parsing workflows: `parse_wasm_from_bytes()` (lines 91-206) and `build_call_graph()` (lines 377-429), each re-parsing the entire WASM module. Same for DWARF source mapping: `map_instr_to_source()` (lines 619-688) scans the entire module to find a single function's body, while `map_instr_to_source_fast()` (lines 567-616) uses pre-cached body ranges from FunctionInfo.
- Files: `src/lib.rs` (multiple functions), `src/main.rs` (lines 307-403 rebuilds indices on every filter change)
- Impact: Redundant parsing on every filter refresh reduces responsiveness. For large WASMs, filter latency becomes noticeable. The "fast" variant exists but is not consistently used.
- Fix approach: Cache the parsed module structure once. Unify parsing with a single entry point. Always use body ranges from FunctionInfo; remove the slow `map_instr_to_source()` function or deprecate it.

**DWARF Context Shared Across Multiple Module Instances:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\lib.rs` line 497 defines `DWARF_CONTEXT` as a static global `OnceLock`. This means analyzing multiple WASM files in sequence reuses the cached DWARF context from the first file.
- Files: `src/lib.rs` (line 497, functions 501-564, 567-616)
- Impact: If two different WASM files are analyzed in the same process, source mappings for the second file may be incorrect or missing because the DWARF context was initialized only from the first file. Affects tools/tests that process multiple WASMs.
- Fix approach: Tie the DWARF context lifecycle to the module being analyzed. Store DWARF context as part of the TUI App state or as a module-scoped cache, not global. Invalidate and reinitialize per new WASM file.

**Unhandled addr2line API Error Cases:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\lib.rs` lines 587-596, 764-803 use `find_frames()` and `find_location()` calls that may fail or return empty results. The code often falls through silently or tries a "next byte" probe (line 604), but error handling is incomplete.
- Files: `src/lib.rs` (lines 567-616, 696-838)
- Impact: Malformed or unexpected DWARF sections can cause source mapping to silently fail with no feedback. Users see blank source panes without knowing why.
- Fix approach: Log warnings when addr2line fails. Provide diagnostic feedback (e.g., "DWARF parsing failed for function X") to help users understand why source code is unavailable.

**Missing Input Validation on WASM Parsing:**
- Issue: `C:\Users\chw11\code\wasm-poke\src\lib.rs` line 105-167 parses the WASM with `Parser::new(0).parse_all()` but does not validate that the file is actually a valid WASM module until parsing. Errors bubble up, but malformed inputs may crash or produce undefined behavior.
- Files: `src/lib.rs` (lines 83-206)
- Impact: Passing a non-WASM file (e.g., a PNG) will fail with a cryptic parsing error. No user-friendly error message.
- Fix approach: Add early magic number check (`\0asm`), provide better error messages, add fuzz testing.

## Known Bugs

**Duplicate Code (lines 689-690 and 728-729) in main.rs:**
- Symptoms: `self.expanded.clear()` is called twice in succession (line 689-690, line 728-729).
- Files: `src/main.rs` (lines 689-690, 728-729)
- Trigger: Enter graph mode via 'g' key
- Impact: Benign redundancy; no functional bug. Second clear() is a no-op.
- Workaround: None needed; remove duplicate for clarity.

**Case Sensitivity Mismatch in Filter (main.rs vs lib.rs):**
- Symptoms: `C:\Users\chw11\code\wasm-poke\src\main.rs` lines 99-101 build glob patterns with `.case_insensitive(true)`, but the README (line 30) documents filtering as case-sensitive.
- Files: `src/main.rs` (lines 99-101, 326-328, 989-992), README (line 30)
- Trigger: Apply a filter like "ADD" (uppercase) to a function named "add_func" (lowercase)
- Impact: Users expect case-sensitive filtering but get case-insensitive behavior in practice. Conflicting documentation.
- Workaround: Set `.case_insensitive(false)` in GlobBuilder or update README.

**Source Span Filter Drops Unmatched Instructions (line 1509-1513):**
- Symptoms: In inspect mode, if an instruction maps to a source file not in the primary file list, the file list UI filters to show only matching files (line 1509-1513). If filtering results in an empty list, the code attempts to fall back but logic is unclear.
- Files: `src/main.rs` (lines 1508-1554)
- Trigger: View inspect mode for a function with inlined code from multiple files, then navigate to an instruction that maps to a file not in the original span list.
- Impact: Source file list may become empty or show wrong file, confusing users about available source files.
- Workaround: Press '[' or ']' to manually cycle files.

## Security Considerations

**No Path Traversal Protection in Source File Reading:**
- Risk: `C:\Users\chw11\code\wasm-poke\src\main.rs` line 442 reads arbitrary files from `span.file` paths without validation. If a WASM module embeds absolute paths (e.g., `/etc/passwd` in a DWARF stub), the tool attempts to read them.
- Files: `src/main.rs` (lines 439-446), `src/lib.rs` (lines 696-838)
- Current mitigation: std::fs::read_to_string() will fail gracefully if the file doesn't exist, so it won't crash. Errors are silently ignored.
- Recommendations: Validate source file paths (restrict to project directory or relative paths). Log path access attempts for debugging. Consider a sandbox or flag for unsafe path reading.

**Unsafe Box::leak() Without Bounds:**
- Risk: See tech debt section. Unbounded memory allocation from untrusted WASM input.
- Files: `src/lib.rs` (line 534)
- Current mitigation: Only reachable if WASM contains DWARF sections.
- Recommendations: Bounds-check allocated size before leak. Implement bounded cache.

**Mutable Global DWARF_CONTEXT in Mutex:**
- Risk: Global mutable state (line 497) could be accessed concurrently by multiple threads in a hypothetical future concurrent design.
- Files: `src/lib.rs` (line 497)
- Current mitigation: Mutex guard serializes access. But correctness still depends on single-module-per-lifetime invariant.
- Recommendations: Document thread-safety assumptions. Consider scoping DWARF context to per-module state.

## Performance Bottlenecks

**Graph Mode cumulative size calculation (line 1715):**
- Problem: Computing `unique_cumulative_size()` for every visible row in graph mode is expensive. Each call does a full DFS traversal of the call subgraph starting from the row's function.
- Files: `src/main.rs` (line 1715), `src/lib.rs` (lines 435-471)
- Cause: In a call graph with 1000 nodes and moderate branching, rendering 20 visible rows costs 20 DFS traversals, each possibly visiting hundreds of nodes.
- Improvement path: Cache cumulative sizes per root node. Use memoization or pre-compute for all reachable nodes from graph root once, rather than per-row.

**Filter Matching Compiled Matchers Per Term (lines 315-343 in main.rs):**
- Problem: `refresh_indices()` recompiles glob matchers for every filter term on every keystroke, even for multi-term filters. GlobBuilder is not free.
- Files: `src/main.rs` (lines 315-343)
- Cause: No caching of compiled matchers between filter changes.
- Improvement path: Cache the compiled matchers alongside the filter string. Only recompile if filter changes.

**Tree View Scroll Management (lines 1628-1634 in main.rs):**
- Problem: Tree view manually manages scroll offset. Rendering a windowed subset of rows (line 1637) still computes cumulative size for all visible rows (line 1715), which may exceed the viewport.
- Files: `src/main.rs` (lines 1627-1755)
- Cause: Virtualization is incomplete; data is windowed but all expensive computations still happen.
- Improvement path: Pre-compute cumulative sizes for the entire tree once, then just slice the cached results.

**DWARF addr2line probe loop (lines 668-677 in lib.rs):**
- Problem: If an instruction address is not found, the code probes the next 4096 bytes (loop at line 668-677). This is a brute-force fallback.
- Files: `src/lib.rs` (lines 668-677)
- Cause: DWARF line info may be sparse or misaligned.
- Improvement path: Use DWARF line info directly to find the nearest valid address, rather than blindly probing.

## Fragile Areas

**Graph Mode Tree Walking (lines 837-1034 in main.rs):**
- Files: `src/main.rs` (lines 837-1034)
- Why fragile: Complex state machine with expanded/collapsed nodes, path tracking, and window offset management. Multiple recursive tree walks (compute_tree_view, compute_tree_view_filtered, find_visible_child_row) with nearly identical logic. Easy to break or desync.
- Safe modification: Add comprehensive tests for tree navigation (expand, collapse, scroll, find_child). Test with cyclic call graphs. Unify the three tree-walking functions into a single generic implementation.
- Test coverage: No visible test coverage for graph mode tree logic in visible test files.

**DWARF Frame Iteration and Frame Inlining Logic (lines 586-596, 764-803 in lib.rs):**
- Files: `src/lib.rs` (lines 586-616, 696-838)
- Why fragile: Balances `find_frames()` (handles inlined functions) vs `find_location()` (simpler fallback). Logic for "taking the first frame" vs "counting all frames" is debated in comments (lines 796-800, 761-763). Behavior differs between two source mapping functions.
- Safe modification: Write tests mapping known DWARF sections and verify correct source locations. Document the inlining strategy (e.g., "always use top frame" vs "aggregate all frames"). Unify the two approaches.
- Test coverage: No visible integration tests with real DWARF sections.

**App State Caches and Invalidation (lines 184-252, 229-251 in main.rs):**
- Files: `src/main.rs` (lines 184-252)
- Why fragile: Multiple overlapping caches (source_span_cache, source_file_cache, inspect_cache, name_map) with unclear invalidation strategy. If selected function changes but caches are not cleared, stale data may display.
- Safe modification: Add invariant checks (e.g., assert that wat_lines are for the current function). Clear all caches when switching functions or modes. Test cache invalidation.
- Test coverage: No visible tests for cache correctness.

## Scaling Limits

**Call Graph Size (node count ~1000, branching factor ~5):**
- Current capacity: Graph mode handles moderate graphs (a few hundred nodes) interactively. Rendering 20 visible rows with cumulative size calculations remains responsive.
- Limit: ~10,000 nodes with moderate branching becomes sluggish. ~100,000 nodes will OOM or timeout.
- Scaling path: Pre-compute all cumulative sizes once at load time (takes seconds but done once). Implement proper virtualization (compute only visible rows). Use a more efficient graph representation (e.g., adjacency list with precomputed transitive closures).

**Module Size (code_size, number of functions):**
- Current capacity: Handles WASM modules with ~10,000 defined functions and total code size ~50 MB.
- Limit: Modules with >100,000 functions or >500 MB code size will be slow to parse and filter.
- Scaling path: Lazy-load function metadata. Index by name for faster filtering. Stream WASM sections instead of buffering all bytes.

**Source File Caching (file count, max file size):**
- Current capacity: Caches source files in memory. Reasonable for a few dozen files up to ~100 KB each.
- Limit: Hundreds of large source files (>1 MB each) will exhaust memory.
- Scaling path: Implement LRU eviction. Cache to disk (tempfile). Stream source file content from disk.

## Dependencies at Risk

**wasmparser 0.241.2:**
- Risk: WASM binary format evolves. If a new proposal (e.g., GC, multi-value) is added, parsing may fail or silently ignore new sections.
- Impact: Future WASM modules may not parse correctly.
- Migration plan: Monitor wasmparser releases. Add WASM suite tests (https://github.com/WebAssembly/testsuite) to ensure compatibility.

**addr2line 0.25.1 and gimli 0.32.3:**
- Risk: DWARF format is complex; bugs or missing features in addr2line/gimli may produce incorrect or missing source mappings.
- Impact: Source code pane displays wrong files/lines.
- Migration plan: Update to latest versions regularly. Add integration tests with real DWARF binaries.

**rustc-demangle 0.1:**
- Risk: Rust name mangling scheme evolves. Old demangler may fail to demangle names from new compilers.
- Impact: Names display as raw mangled strings instead of demangled.
- Migration plan: Periodically update rustc-demangle and test with latest Rust compiler output.

## Missing Critical Features

**No Export of Call Graph or Analysis Results:**
- Problem: TUI is interactive but no way to export analysis (call graph, cumulative sizes, source mappings) for external tools or reports.
- Blocks: Batch analysis, CI integration, team sharing of analysis results.
- Improvement: Add output formats (e.g., `--export-call-graph json/dot`, `--export-cumulative csv`).

**No Filtering Within Graph Mode:**
- Problem: In graph mode, filter is disabled and cleared (line 735). Users cannot drill down into a subgraph by name.
- Blocks: Finding calls to a specific function within a large call graph.
- Improvement: Allow filtering in graph mode (show only nodes matching pattern + their ancestors/descendants).

**No Search/Jump to Function:**
- Problem: Navigating to a specific function requires scrolling or filtering. No direct "jump to index X" command.
- Blocks: Cross-referencing (e.g., "go to function 42").
- Improvement: Add '/' binding (already used for filter) or new binding to jump by index or name.

**No Diff Mode:**
- Problem: Cannot compare two WASM modules to see which functions grew/shrunk.
- Blocks: Tracking code size changes across commits or versions.
- Improvement: Add `--diff previous.wasm` mode.

## Test Coverage Gaps

**Graph Mode Navigation and Tree Walking:**
- What's not tested: Graph mode expand/collapse, scrolling, cyclic graphs, find_visible_child_row.
- Files: `src/main.rs` (lines 837-1092)
- Risk: Easy to introduce regressions in tree traversal logic.
- Priority: High (complex state machine)

**DWARF and Source Mapping:**
- What's not tested: map_instr_to_source correctness with real DWARF sections. Behavior with missing/malformed DWARF. Inlined function frame handling.
- Files: `src/lib.rs` (lines 493-838)
- Risk: Silent failures in source mapping. Wrong line numbers displayed.
- Priority: High (user-visible feature)

**Filter Matching with Complex Patterns:**
- What's not tested: Multi-term filters ("foo bar"), wildcard interactions ("*foo*bar*"), edge cases (empty filter, non-ASCII characters).
- Files: `src/lib.rs` (lines 214-363), `src/main.rs` (lines 315-343)
- Risk: Filter behavior surprising users. Case sensitivity bug.
- Priority: Medium (user-facing but not crash-prone)

**TUI Cache Invalidation:**
- What's not tested: Cache correctness when switching functions, modes, filters. Stale data in inspect mode.
- Files: `src/main.rs` (lines 184-252, 411-448)
- Risk: Displaying wrong data (e.g., WAT for function A when function B is selected).
- Priority: Medium (data correctness)

**Error Handling in Malformed WASM:**
- What's not tested: Non-WASM input files. WASM with missing sections. Overflow in function count or size.
- Files: `src/lib.rs` (lines 83-206)
- Risk: Crashes or confusing error messages on bad input.
- Priority: Low (not a security risk if errors are handled)

---

*Concerns audit: 2026-01-26*
