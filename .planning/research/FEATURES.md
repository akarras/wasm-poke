# Feature Landscape

**Domain:** WebAssembly analysis tool for Rust developers (egui-based)
**Researched:** 2026-01-26
**Confidence:** HIGH (verified against existing tools and standards)

## Table Stakes

Features users expect from a Wasm analyzer. Missing these = product feels incomplete or unusable.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Function list with size sorting** | Core use case for size analysis; users need to see what's big | Low | Already exists in wasm-poke |
| **Filter/search by name** | Essential for navigating large modules (1000+ functions common) | Low | Already exists with glob support |
| **Size metrics (bytes, percentage)** | Primary data users need; why else use a size analyzer? | Low | Already exists |
| **Name demangling (Rust)** | Raw Rust names are unreadable (e.g., `_ZN3foo3bar17h...`); tool is useless without this | Low | Already exists via rustc-demangle |
| **Keyboard navigation** | Power users expect vim-like navigation (j/k/g/G); mouse-only is slow | Low | Plan for egui version |
| **Export/save results** | Users need to share findings, integrate into CI/reports | Medium | JSON output exists; consider CSV |
| **Responsive UI / no freezing** | Users load multi-MB Wasm files; UI must stay responsive | Medium | Async loading, progress indicators |
| **Cross-platform support** | Developers use Windows/macOS/Linux; must work on all | Medium | egui + eframe handles this |
| **Basic hex view** | Seeing raw bytes is fundamental to binary analysis | Medium | Exists in current TUI |
| **WAT/disassembly view** | Users need to see actual Wasm instructions, not just sizes | Medium | Exists in current TUI |

## Differentiators

Features that set wasm-poke apart. Not expected, but add significant value.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Source-to-Wasm mapping** | "Why is this function so big?" - Answer by showing which Rust code compiles to what instructions | High | Partially exists via DWARF; needs polish |
| **Synchronized three-panel view** | Hex | Instructions | Source in lockstep like Compiler Explorer; clicking instruction highlights byte AND source line | High | Major differentiator; few tools do this well |
| **Static call graph visualization** | See which functions call what; understand code paths without running | Medium | Call graph exists; needs tree view in egui |
| **Cumulative size analysis** | "If I remove this function, how much total code goes away?" (Twiggy's key feature) | Medium | `unique_cumulative_size` exists |
| **Instruction-level help** | Hover/press ? on `i32.add` and see explanation; teaching tool | Low | Help system exists in help.rs |
| **Web deployment (no install)** | Drop a Wasm file in browser, analyze instantly; frictionless onboarding | High | egui compiles to Wasm; feasible |
| **Interpreted Wasm optimization focus** | Tailored advice for non-JIT runtimes where instruction count matters | Medium | Unique positioning |
| **Name highlighting in calls** | `call 42` annotated with `;; my_function` so users don't manually look up indices | Low | Exists in current TUI |
| **Multi-term filter** | `alloc vec` finds functions matching both terms (AND logic) | Low | Exists in current TUI |
| **Goto definition from call instruction** | Select `call 42`, press g, jump to function 42's definition | Medium | Exists as 'g' in inspect mode |
| **Diff mode** | Compare two Wasm binaries, show what changed/grew/shrank | High | Twiggy has this; high value for iteration |
| **Bookmarks/annotations** | Mark interesting functions, add notes for later | Medium | Persistence needed |

## Anti-Features

Features to explicitly NOT build. Common mistakes in this domain or scope creep.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Runtime/dynamic tracing** | Adds massive complexity; changes tool category from static analyzer to profiler/debugger | Stay focused on static analysis; recommend complementary runtime tools |
| **Full debugger (breakpoints, stepping)** | Out of scope; debuggers already exist (Chrome DevTools, wasmtime debug) | Link to existing debuggers from docs |
| **Support for all source languages** | Rust→Wasm is the focus; supporting C++/Go/Zig dilutes UX for primary users | Note in docs that DWARF-based mapping may work for other languages but is not tested |
| **Wasm editing/patching** | Different tool category (hex editor); muddles the read-only analysis focus | Read-only by design; recommend wabt for transforms |
| **JIT/AOT compilation analysis** | Interpreted Wasm runtimes are the use case; JIT codegen is a different problem | Focus messaging on instruction count, not native perf |
| **Pretty decompilation output** | wasm-decompile already exists in wabt; no need to reinvent | Show WAT format; link to wasm-decompile for C-like view |
| **Real-time file watching** | Overengineered for a manual analysis workflow; complicates state management | Manual refresh/reload button |
| **Plugin system** | Premature; focus on core features before extensibility | Hardcode features; revisit if/when user demand exists |
| **Collaborative/cloud features** | Scope creep; local tool is fine | Local-first; export/share via JSON/screenshots |
| **Source code editing** | This is an analyzer, not an IDE; no code completion, no save-to-file | Display source read-only with navigation only |

## Feature Dependencies

```
Function list (base)
    |
    +-- Filter/search
    +-- Size sorting
    +-- Name demangling
    |
    +-- Call graph --> Cumulative size analysis
    |                      |
    |                      +-- Diff mode (needs two modules)
    |
    +-- Inspect mode
            |
            +-- Hex view (requires body_bytes extraction)
            +-- WAT view (requires disassembly)
            +-- Source view (requires DWARF parsing)
            |       |
            |       +-- Synchronized navigation
            |               |
            |               +-- Instruction help
            |               +-- Goto definition
            |
            +-- Source file list (multiple files per function)

Web deployment (independent, but requires all features to work in Wasm target)
```

## MVP Recommendation

For the egui rewrite MVP, prioritize:

### Must Have (Phase 1)
1. **Function list with filtering** - Core use case; users leave without this
2. **Size metrics display** - Primary value proposition
3. **Keyboard navigation** - Power users expect this
4. **Name demangling** - Usability requirement for Rust code

### Should Have (Phase 2)
5. **Call graph tree view** - Key differentiator; already have data
6. **Cumulative size** - Leverages existing infrastructure
7. **Three-panel inspection** - Major differentiator

### Nice to Have (Phase 3+)
8. **Web deployment** - Frictionless onboarding
9. **Diff mode** - Iterative optimization workflow
10. **Bookmarks** - Power user feature

## Defer to Post-MVP

- **Diff mode**: High value but high complexity; better to nail core features first
- **Bookmarks/annotations**: Needs persistence layer; adds state complexity
- **Web deployment**: Test native thoroughly first; Wasm build adds CI/testing burden

## Competitor Landscape

| Tool | Strengths | Weaknesses | Opportunity |
|------|-----------|------------|-------------|
| [Twiggy](https://github.com/rustwasm/twiggy) | Mature, CLI-focused, good dominators/diff | No GUI, no source mapping, archived/maintenance mode | Better UX, source mapping, GUI |
| [wasm-objdump](https://github.com/WebAssembly/wabt) | Authoritative, part of wabt | Text output only, no filtering, no size focus | Interactive exploration |
| [wasm-decompile](https://github.com/WebAssembly/wabt) | C-like readable output | Separate tool, no Rust specifics | Integration as export option |
| [Bloaty](https://github.com/google/bloaty) | Excellent ELF/Mach-O support | Wasm support limited, no Rust demangling | Wasm-native focus |
| [Compiler Explorer](https://godbolt.org/) | Amazing source-asm mapping | Compile-time only, not for pre-built binaries | Post-compilation analysis |

## UX Patterns from Related Tools

### From Hex Editors (HxD, ImHex)
- Side-by-side hex + ASCII view
- Goto offset dialog
- Selection highlighting across views
- Infinite undo/redo (not needed for read-only)

### From Disassemblers (IDA, Ghidra)
- Cross-reference navigation (xrefs)
- Graph view for call relationships
- Name annotations on call instructions
- Search by name, address, or pattern

### From Performance Profilers
- Flame graphs for hierarchical size (like icicle charts)
- Sorting by different metrics (self size, cumulative size)
- Filtering/focusing on specific subtrees

### From Compiler Explorer
- Source-assembly color coding (matching lines)
- Multiple views synchronized by selection
- Hover for details (instruction help)

## Sources

- [Twiggy Documentation](https://rustwasm.github.io/twiggy/index.html)
- [WABT GitHub](https://github.com/WebAssembly/wabt)
- [wasm-decompile Documentation](https://v8.dev/blog/wasm-decompile)
- [Bloaty McBloatface](https://github.com/google/bloaty)
- [Compiler Explorer](https://godbolt.org/)
- [Hex Editors Overview (Wikipedia)](https://en.wikipedia.org/wiki/Hex_editor)
- [ImHex](https://github.com/WerWolv/ImHex)
- [IDA Pro vs Ghidra Comparison](https://hackmag.com/security/nsa-ghidra)
- [Flame Graphs (Brendan Gregg)](https://www.brendangregg.com/flamegraphs.html)
- [DWARF for WebAssembly](https://yurydelendik.github.io/webassembly-dwarf/)
- [WebAssembly Tool Conventions - Debugging](https://github.com/WebAssembly/tool-conventions/blob/main/Debugging.md)
- [Kano Model - Table Stakes Features](https://uxbooth.com/articles/discovering-table-stakes-delighters/)

---

*Feature research completed: 2026-01-26*
