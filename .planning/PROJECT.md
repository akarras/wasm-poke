# wasm-poke

## What This Is

A WebAssembly analysis tool for Rust developers who need to understand what their code compiles to. Focused on performance analysis for interpreted Wasm runtimes where instruction count directly impacts performance. Provides function-level size analysis, call graphs, and source mapping via DWARF debug information.

## Core Value

Developers can see exactly how their Rust code translates to Wasm instructions, with source mapping, so they can make informed performance decisions.

## Requirements

### Validated

- ✓ Parse Wasm binary files and extract module structure — existing
- ✓ Extract function metadata (names, sizes, indices, imports/exports) — existing
- ✓ Filter functions by glob pattern (case-insensitive) — existing
- ✓ Sort functions by code size — existing
- ✓ Build static call graphs from code section — existing
- ✓ Map instructions to source locations via DWARF — existing
- ✓ Demangle Rust symbol names — existing
- ✓ JSON output mode — existing
- ✓ Summary text output — existing

### Active

- [ ] egui-based UI replacing ratatui TUI
- [ ] Native build target (Windows, macOS, Linux)
- [ ] Web build target (Wasm frontend)
- [ ] Function list view with filtering
- [ ] Call tree view (entry to exit, static analysis)
- [ ] Size summary tree (bytes rolling up through call graph)
- [ ] Three-panel inspection view (hex bytes | Wasm instructions | source code)
- [ ] Synchronized panel navigation (selecting instruction highlights corresponding hex/source)
- [ ] Goto navigation between functions from call instructions
- [ ] Stable state management (no desync between views)

### Out of Scope

- Expanded instruction rendering and explanations — deferred to v2
- Dynamic tracing / runtime profiling — not needed for static analysis use case
- Support for non-Rust source mapping — Rust→Wasm is the primary use case

## Context

The existing codebase has solid parsing and analysis infrastructure built on wasmparser, gimli, and addr2line. The current TUI (ratatui/crossterm) works but has stability issues with state synchronization:
- Source mapping doesn't always align with highlighted function
- Filtering causes mismatches between view and data
- Goto sometimes breaks hex view state

The rewrite to egui provides an opportunity to fix these issues by rebuilding the UI layer with proper state management, while preserving the proven analysis logic.

Target users are Rust developers working with interpreted Wasm runtimes (no JIT), where instruction count is the key performance metric.

## Constraints

- **Tech stack**: egui for UI — enables native + web from single codebase
- **Compatibility**: Must support existing Wasm files with DWARF debug info
- **Architecture**: Preserve existing parser/analysis layer in src/lib.rs, src/parser.rs, src/model.rs
- **Distribution**: Web version should work without installation (drop-in Wasm file)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| egui over other GUI frameworks | Supports native + Wasm builds, Rust-native, immediate mode fits inspection UI | — Pending |
| Rewrite UI rather than fix TUI bugs | Cleaner to rebuild with proper state management than patch existing issues | — Pending |
| Static call graph only | Dynamic tracing adds complexity; static analysis sufficient for size optimization | — Pending |

---
*Last updated: 2026-01-26 after initialization*
