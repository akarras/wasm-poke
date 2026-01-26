# Project Research Summary

**Project:** wasm-poke egui GUI milestone
**Domain:** WebAssembly analysis tool for Rust developers
**Researched:** 2026-01-26
**Confidence:** HIGH

## Executive Summary

wasm-poke is a WebAssembly binary analysis tool focused on Rust developers working with interpreted Wasm runtimes. The project has a working CLI/TUI version and needs a GUI rewrite using egui to provide both native desktop and web deployment. The recommended approach uses **egui 0.33 with eframe** for cross-platform UI, **glow** backend for smaller WASM binaries, and **egui_dock** for flexible panel layouts. The existing analysis infrastructure (wasmparser, gimli, addr2line) remains unchanged.

The key insight from research is that egui's **immediate-mode paradigm eliminates traditional state synchronization bugs** that currently plague the TUI version. Instead of maintaining separate state in multiple views, the architecture centers on a single source of truth (`WasmPokeApp`) with views that query state each frame. This matches production patterns from Rerun.io and directly addresses the multi-view cursor synchronization issues identified in the current codebase.

Critical risks include: (1) state architecture mistakes that recreate existing TUI sync bugs, (2) WASM memory limits with large binary files, (3) async file loading complexity on web targets, and (4) widget ID collisions in dynamic lists. These are all preventable through proper architecture established in Phase 1 before implementing features.

## Key Findings

### Recommended Stack

The egui ecosystem provides mature, battle-tested tools for cross-platform developer tools. The stack choice prioritizes small WASM binary size for web deployment while maintaining full feature parity with native builds.

**Core technologies:**
- **egui 0.33 + eframe**: Immediate-mode GUI framework with single codebase for native and web - active development, excellent documentation, production-proven at Rerun.io
- **glow rendering backend**: OpenGL-based renderer that produces binaries 1-2MB smaller than wgpu - critical for web deployment of analysis tools
- **Trunk 0.21**: Zero-config WASM bundler recommended by egui documentation - handles wasm-bindgen and wasm-opt automatically
- **egui_dock**: Panel layout system with tabs and docking - standard pattern for multi-view developer tools
- **rfd 0.15**: Cross-platform file dialogs that work on native and web - required for loading wasm binaries
- **poll-promise 0.3**: Async result polling for immediate-mode UI - needed for non-blocking file operations on web

**Critical version notes:**
- egui is transitioning to wgpu as default backend (April 2025+), but glow should be explicitly enabled for this use case
- Rust 1.81+ required for egui 0.33 features

### Expected Features

Based on analysis of competing tools (Twiggy, wasm-objdump, Bloaty, Compiler Explorer) and developer workflow patterns, the feature landscape divides into three categories.

**Must have (table stakes):**
- Function list with size sorting and metrics - core value proposition, users leave without this
- Filter/search by name with glob patterns - essential for 1000+ function modules
- Name demangling for Rust symbols - usability requirement, raw names are unreadable
- Keyboard navigation - power users expect vim-like navigation (j/k/g/G)
- WAT/disassembly view - fundamental to binary analysis
- Basic hex view - see raw bytes at instruction level
- Cross-platform support - developers use Windows/macOS/Linux
- Responsive UI with no freezing - multi-MB files are common

**Should have (competitive differentiators):**
- **Source-to-Wasm mapping**: Show which Rust code compiles to what instructions - answers "why is this function so big?" using existing DWARF infrastructure
- **Synchronized three-panel view**: Hex | Instructions | Source in lockstep like Compiler Explorer - clicking instruction highlights byte AND source line (major differentiator)
- **Static call graph visualization**: Tree view of function calls - understand code paths without running
- **Cumulative size analysis**: "If I remove this function, how much code goes away?" - leverages existing `unique_cumulative_size` data
- **Web deployment**: Drop a Wasm file in browser, analyze instantly - frictionless onboarding compared to CLI-only competitors

**Defer (v2+):**
- Diff mode (compare two binaries) - high value but high complexity, better to nail core features first
- Bookmarks/annotations - needs persistence layer, adds state complexity
- Real-time file watching - overengineered for manual analysis workflow

**Anti-features (explicitly avoid):**
- Runtime/dynamic tracing - changes tool category from static analyzer to profiler
- Full debugger functionality - out of scope, Chrome DevTools already exists
- Wasm editing/patching - read-only analysis is the focus

### Architecture Approach

The architecture follows egui's immediate-mode design philosophy with centralized state and query-on-demand views. This eliminates the state synchronization bugs present in the current TUI implementation.

**Major components:**
1. **WasmPokeApp**: Top-level state container owning all analysis data (WasmModuleInfo, CallGraph, wasm_bytes), selection state (selected_function, expanded_nodes, cursor positions), and UI configuration (dock_state, filter text, view modes). Implements `eframe::App` trait.
2. **AnalysisModel**: Immutable after file load. Contains parsed Wasm data, call graph, DWARF context. All views read from this without modification.
3. **SelectionState**: Mutable shared state for current selections and cursor positions. Views both read and write this - single source of truth prevents desyncs.
4. **View Functions**: Stateless rendering functions (function_list_view, call_tree_view, inspector_view) that receive `&mut WasmPokeApp` and query state each frame. No separate view structs with duplicate state.
5. **DockState**: Layout management via egui_dock - handles tabs, splits, and panel arrangement with persistence to localStorage.

**Key pattern**: Views derive display state from SelectionState each frame rather than caching positions. Example: hex offset, source line, and instruction cursor all derive from a single `instruction_cursor: usize` field. This fixes the multi-view sync bugs in the current TUI where `wat_cursor`, `wat_scroll`, and `source_scroll` can get out of sync during `goto` operations.

### Critical Pitfalls

The research identified pitfalls from three sources: egui ecosystem issues, WASM platform constraints, and problems in the existing TUI codebase.

1. **Scattered State Leading to Desync** — The current TUI has state distributed across `selected`, `tree_selected`, `wat_cursor`, `wat_scroll`, `source_scroll`, `wat_lines`, and multiple caches. When `goto` is triggered (main.rs lines 671-738), updating all these fields correctly is error-prone. **Mitigation**: Single source of truth (`SelectionState { function_index, instruction_cursor }`) with all display positions derived on each frame. Immediate mode guarantees views cannot desync if they derive from same state.

2. **Widget ID Collisions** — egui tracks widget state (scroll, collapse, drag) by ID. Dynamic lists without stable keys or using display names as IDs causes state corruption (scroll positions jump, clicks affect wrong items). **Mitigation**: Use function index (u32) for IDs, not display names. Push unique ID for list items: `ui.push_id(func.index, |ui| { ... })`. Use `id_salt` for stateful widgets in loops.

3. **WASM Memory Limits** — Browser tabs have ~2GB limit. Loading 100MB+ Wasm files, then caching disassembly for 10K functions, can exhaust memory and crash tabs. **Mitigation**: Lazy loading (only disassemble on inspect), LRU cache for recently viewed functions, size warning UI for >50MB files, memory budget monitoring on WASM target.

4. **Async File Operations Blocking UI** — Browser File API is async. Using blocking patterns (std::fs) doesn't compile for WASM. Naive async patterns (pollster) panic with "condvar wait not supported." **Mitigation**: Use poll-promise for async operations, abstract file loading behind trait with native (blocking) and WASM (async polling) implementations, show loading progress UI.

5. **Frame Delay Breaking Immediate Feedback** — In immediate mode, interaction is recorded in frame N but code sees it in frame N+1. For cursor navigation, this creates lag. **Mitigation**: Process input before rendering (check key presses at frame start, then render with updated cursor). Accept one-frame delay for most interactions (users won't notice). Use `request_repaint` for background updates.

## Implications for Roadmap

Based on combined research, the roadmap should follow architectural dependencies and risk mitigation priorities. The architecture research (ARCHITECTURE.md) provides an explicit build order recommendation that aligns with feature dependencies (FEATURES.md) and pitfall prevention (PITFALLS.md).

### Phase 1: Foundation & State Architecture
**Rationale:** Establish state model and platform abstractions BEFORE implementing any features. Prevents recreating TUI sync bugs. All subsequent phases depend on this foundation.
**Delivers:** App shell with egui_dock, empty tabs, single-source-of-truth SelectionState, platform abstraction layer (native vs WASM file loading).
**Addresses:** None yet - this is infrastructure.
**Avoids:** Pitfall 1 (scattered state), Pitfall 10 (platform code leaking into logic).
**Research needs:** Standard patterns - no additional research needed.

### Phase 2: Core List Views
**Rationale:** Function list is the entry point and primary navigation. Must work correctly before building dependent features like inspector or call graph. Tests state-sharing pattern across multiple views.
**Delivers:** Function list with filtering, size sorting, name demangling, keyboard navigation. Basic call tree view (read-only).
**Addresses:** Table stakes features - function list, filter/search, size metrics, demangling, keyboard nav.
**Avoids:** Pitfall 2 (ID collisions - use function index as stable ID), Pitfall 7 (virtualization - use TableBuilder from start), Pitfall 8 (filter state index mismatch - store selection as function ID not list position).
**Research needs:** Standard patterns - egui_extras TableBuilder is well-documented.

### Phase 3: Three-Panel Inspector
**Rationale:** This is the most complex feature and addresses the current TUI's main bug (cursor sync). Validates that centralized state model works for tightly coupled views. Differentiator feature that sets wasm-poke apart.
**Delivers:** Inspector tab with hex | WAT | source panes. Synchronized cursor navigation across all three views. Source-to-Wasm mapping via DWARF.
**Addresses:** Major differentiators - synchronized three-panel view, source-to-Wasm mapping.
**Avoids:** Pitfall 1 (desync - all three panes derive from single cursor), Pitfall 3 (frame delay - process input before rendering), Pitfall 9 (business logic in render - compute derived positions outside hot loop).
**Research needs:** Some research needed for DWARF line mapping edge cases, but core patterns are established in existing TUI code.

### Phase 4: Async File Loading & Web Deployment
**Rationale:** File loading is the entry point for the app, but complex enough to defer until core features work. Web deployment validates cross-platform architecture and unblocks frictionless onboarding.
**Delivers:** Async file loading with progress UI, drag-and-drop support, web deployment via Trunk, memory-conscious loading for large files.
**Addresses:** Table stakes - responsive UI, cross-platform support. Differentiator - web deployment.
**Avoids:** Pitfall 4 (WASM memory limits - lazy loading with LRU cache), Pitfall 5 (async blocking - poll-promise pattern), Pitfall 12 (missing request_repaint after background load).
**Research needs:** Some research needed for incremental file reading on WASM (Blob.stream API), but rfd and poll-promise patterns are documented.

### Phase 5: Advanced Features & Polish
**Rationale:** Once core features and web deployment work, add features that increase value but aren't blocking. Performance optimization based on profiling real usage.
**Delivers:** Size tree view with cumulative calculations, instruction-level help system, goto definition from call instructions, layout persistence, performance optimizations.
**Addresses:** Differentiators - cumulative size analysis, instruction help.
**Avoids:** Pitfall 6 (CPU spin - profile and use reactive mode correctly), Pitfall 11 (inconsistent styling - test both platforms), Pitfall 13 (scroll position reset - remember fractional position or top item).
**Research needs:** Standard patterns - no additional research.

### Phase Ordering Rationale

- **Phase 1 first**: State architecture mistakes are expensive to fix later. Immediate-mode patterns must be established before features.
- **Phase 2 before Phase 3**: Simple views (function list) validate state-sharing pattern before tackling complex synchronized views (inspector).
- **Phase 3 before Phase 4**: Get core features working on native before adding web complexity. WASM builds and async file loading add many moving parts.
- **Phase 5 last**: Polish depends on working infrastructure. Performance optimization needs profiling data from real usage.

**Dependency chain:**
```
Phase 1 (foundation)
    |
    +--> Phase 2 (list views) --> Phase 3 (inspector)
    |                                  |
    +--> Phase 4 (web deployment) <----+
                    |
                    +--> Phase 5 (polish)
```

### Research Flags

**Needs research during planning:**
- **Phase 3**: DWARF line mapping edge cases (source files with multiple functions, inlined code). Could use `/gsd:research-phase` for "DWARF source mapping patterns."
- **Phase 4**: Incremental WASM file reading on web (Blob.stream API, memory pressure detection). Could use `/gsd:research-phase` for "browser memory management for large files."

**Standard patterns (skip research-phase):**
- **Phase 1**: App shell, egui_dock setup - documented in eframe_template.
- **Phase 2**: TableBuilder virtualization, function list filtering - standard egui patterns.
- **Phase 5**: Performance profiling, reactive mode - egui docs cover this.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified via official docs (egui GitHub, eframe docs, Trunk docs), production examples (Rerun.io), eframe_template starter project. Version numbers confirmed from crates.io. |
| Features | HIGH | Verified against competitor analysis (Twiggy, wasm-objdump, Bloaty documented), UX patterns from established tools (Compiler Explorer, IDA, hex editors), developer workflow research. |
| Architecture | HIGH | Based on official egui architecture docs, Rerun.io production patterns (explicitly documented), community discussions on state management. Validated against current TUI codebase issues (main.rs analysis). |
| Pitfalls | HIGH | Sourced from GitHub issues (frame delay #1904, ID collisions #4940, WASM file upload #2091), community discussions (#7553 state management), and concrete bugs identified in current TUI code (lines 671-738 goto sync issues). |

**Overall confidence:** HIGH

The research synthesizes authoritative sources (official docs, production codebases, GitHub issues with maintainer responses) with project-specific analysis (current TUI bugs, existing infrastructure). All recommendations are backed by either documentation or production usage.

### Gaps to Address

Areas where research was inconclusive or needs validation during implementation:

- **WASM binary size with syntect**: FEATURES.md notes syntect adds 500KB-1MB to binaries. Need to measure actual size and consider lazy loading or keyword-based highlighting for web. *Handle during Phase 4 by building with/without syntect feature and comparing dist/ size.*

- **Large file handling on web**: PITFALLS.md warns about memory limits but streaming solutions (Blob.stream) need prototyping. *Handle during Phase 4 by testing with 100MB+ files and implementing progressive loading if needed.*

- **DWARF mapping for inlined functions**: Current TUI has this working but edge cases exist (multiple source files per function, inlined calls). *Handle during Phase 3 by reviewing addr2line API for inline context handling.*

- **Table virtualization performance**: egui_extras TableBuilder provides virtualization but needs profiling with 10K+ function modules to validate. *Handle during Phase 2 by testing with large production Wasm binaries.*

## Sources

### Primary (HIGH confidence)
- [egui GitHub](https://github.com/emilk/egui) - official releases, issue tracker, maintainer responses
- [eframe documentation](https://docs.rs/eframe/latest/eframe/) - application framework API
- [egui_extras docs](https://docs.rs/egui_extras/latest/egui_extras/) - TableBuilder, syntax highlighting
- [eframe_template](https://github.com/emilk/eframe_template) - starter project with Trunk setup
- [Trunk documentation](https://trunkrs.dev/) - WASM bundler configuration
- [egui_dock GitHub](https://github.com/Adanos020/egui_dock) - panel layout patterns
- [Rerun.io Architecture](https://github.com/rerun-io/rerun/blob/main/ARCHITECTURE.md) - production egui patterns
- [wasmparser crate](https://docs.rs/wasmparser/) - existing dependency, unchanged
- [gimli crate](https://docs.rs/gimli/) - DWARF parsing, existing dependency
- [addr2line crate](https://docs.rs/addr2line/) - source mapping, existing dependency

### Secondary (MEDIUM confidence)
- [Twiggy Documentation](https://rustwasm.github.io/twiggy/) - competitor feature comparison
- [WABT GitHub](https://github.com/WebAssembly/wabt) - wasm-objdump, wasm-decompile feature set
- [Bloaty](https://github.com/google/bloaty) - binary size analysis patterns
- [Compiler Explorer](https://godbolt.org/) - source-assembly synchronization UX patterns
- GitHub issues: #1904 (frame delay), #4940 (ID collisions), #2091 (WASM file upload), #7553 (state management)
- [IMGUI paradigm wiki](https://github.com/ocornut/imgui/wiki/About-the-IMGUI-paradigm) - immediate mode concepts

### Tertiary (LOW confidence)
- Community blog posts on egui performance optimization
- LogRocket article on cross-platform egui apps (patterns, not benchmarks)

---
*Research completed: 2026-01-26*
*Ready for roadmap: yes*
