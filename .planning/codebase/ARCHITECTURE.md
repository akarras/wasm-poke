# Architecture

**Analysis Date:** 2026-01-26

## Pattern Overview

**Overall:** Modular library + CLI application with layered concerns (parsing, filtering, analysis, inspection, UI)

**Key Characteristics:**
- Parsing layer decouples WASM binary analysis from presentation
- Library and binary are co-located (single crate, no separation)
- Lazy initialization for expensive operations (DWARF context caching)
- Multiple presentation modes: interactive TUI, JSON output, summary text
- Stateless analysis functions enable flexible composition

## Layers

**Parser Layer:**
- Purpose: Extract structured information from WebAssembly binary format
- Location: `src/parser.rs` + portions of `src/lib.rs`
- Contains: `parse_wasm_from_bytes()`, section parsing logic for imports/exports/code/names
- Depends on: `wasmparser` crate, model types
- Used by: All higher layers (core to all analysis)

**Model Layer:**
- Purpose: Data structures representing parsed WASM modules and functions
- Location: `src/model.rs`
- Contains: `FunctionInfo`, `WasmModuleInfo`, `CallGraph`, `SourceLocation`, `SourceSpan`
- Depends on: serde for serialization
- Used by: All analysis and presentation layers

**Analysis Layer:**
- Purpose: Compute metrics, relationships, and mappings from structured data
- Location: `src/lib.rs` (main analysis functions)
- Contains:
  - Size/percentage computation: `WasmModuleInfo::percentage()`
  - Filtering & sorting: `filter_functions()`, `sorted_by_size()`
  - Call graphs: `build_call_graph()`, `unique_cumulative_size()`
  - Source mapping: `init_dwarf_context()`, `map_instr_to_source()`, `function_source_span()`
  - Disassembly: `disassemble_function_wat_bytes()`, `disassemble_function_wat_lines()`
- Depends on: Parser layer, model types, domain crates (rustc-demangle, addr2line, gimli, globset)
- Used by: Presentation layer (TUI and CLI)

**Presentation Layer:**
- Purpose: User-facing interaction and output formatting
- Location: `src/main.rs`
- Contains:
  - CLI argument parsing via clap
  - TUI implementation via ratatui/crossterm (interactive mode)
  - JSON output formatting
  - Summary text output
- Depends on: Analysis layer, model types
- Used by: End users (entry point)

**Utilities:**
- Purpose: Instruction help and formatting
- Location: `src/help.rs`
- Contains: WASM instruction documentation reference (`get_instruction_help()`)

## Data Flow

**File Input → Parsed Module:**
1. CLI receives `.wasm` file path
2. `parse_wasm()` reads file bytes
3. `parse_wasm_from_bytes()` parses with wasmparser
4. Returns `WasmModuleInfo` with all functions and metadata

**Parsed Module → Filtered Results:**
1. Filtering pattern received from user (CLI flag or TUI input)
2. `filter_functions()` applies glob-style matching (case-insensitive, `*` wildcards)
3. `sorted_by_size()` sorts filtered results by code_size descending
4. Returns `Vec<&FunctionInfo>` (references into original module)

**Function Details → Call Graph:**
1. `build_call_graph()` scans code section operators
2. Records direct `call` operators as edges
3. Records `call_indirect` presence separately
4. Returns `CallGraph` with edges HashMap and indirect call tracking

**Function Index → Source Location (Lazy):**
1. First call to mapping function triggers `init_dwarf_context()`
2. DWARF sections parsed and cached in static `DWARF_CONTEXT` (OnceLock)
3. Subsequent calls reuse cached context
4. `map_instr_to_source()` or `map_instr_to_source_fast()` performs address translation
5. Returns `Option<SourceLocation>` with file/line/column

**State Management:**
- `WasmModuleInfo`: Immutable after parsing; owned by caller
- `CallGraph`: Immutable, derived from bytes once at startup (in TUI mode)
- `DWARF_CONTEXT`: Global OnceLock with Mutex; thread-safe, lazily initialized per process
- TUI state: `ListState`, `TableState` in `main.rs` manage cursor position and selection

## Key Abstractions

**FunctionInfo:**
- Purpose: Represents a single defined function's metadata and size
- Examples: `src/model.rs` struct definition
- Pattern: Value type (Clone, Serialize/Deserialize); multiple name sources with preference order (demangled > raw > export > synthetic)

**WasmModuleInfo:**
- Purpose: Aggregate of all functions in a module plus totals
- Examples: `src/model.rs` struct definition
- Pattern: Container type; computes percentages on-demand via `percentage()` method

**CallGraph:**
- Purpose: Represents direct call relationships between functions
- Examples: `src/lib.rs` definition; built by `build_call_graph()`
- Pattern: Directed graph stored as HashMap<src, Vec<dst>>; separates direct calls from indirect calls

**SourceLocation / SourceSpan:**
- Purpose: Map WASM instruction offsets to source code locations (file/line/column)
- Examples: `src/lib.rs` struct definitions
- Pattern: Immutable value types; populated by DWARF/addr2line during inspection

## Entry Points

**Binary Entry:**
- Location: `src/main.rs` `main()` function
- Triggers: User runs `wasm-poke <file.wasm> [options]`
- Responsibilities:
  1. Parse CLI arguments via clap
  2. Load WASM file and parse module
  3. Build call graph (for TUI mode)
  4. Route to TUI or non-interactive mode
  5. Handle user input in TUI (keyboard events)

**Library Entry Points (for external consumers):**
- `parse_wasm()`: Parse file directly
- `parse_wasm_from_bytes()`: Parse in-memory bytes
- `function_matches()`: Single-function filtering (used in tests)
- `sorted_by_size()`: Get sorted function list
- `build_call_graph()`: Get function relationships

## Error Handling

**Strategy:** Result-based with anyhow context for user-friendly messages

**Patterns:**
- `parse_wasm()` wraps file I/O errors with context about file path
- `parse_wasm_from_bytes()` uses `anyhow::anyhow!()` for parsing errors (e.g., overflow)
- DWARF operations are fallible; `map_instr_to_source()` returns `Option` (graceful degradation)
- TUI error: If parsing fails, error message is printed and program exits
- JSON/summary modes: Errors prevent output; non-zero exit code

## Cross-Cutting Concerns

**Logging:** None; output only via stdout/TUI rendering

**Validation:**
- Index overflow checks: `checked_add()` for imported/defined function counts
- Byte range validation: `body_range.end <= wasm_bytes.len()` in `function_body_bytes()`
- Pattern validation: Invalid glob patterns silently return empty results (no error)

**Name Processing:**
- Demangling: Attempted via `rustc-demangle::try_demangle()` on raw names and exports
- Fallback chain: demangled > raw > first export > synthetic `func[index]`
- Case handling: Filtering is case-insensitive; names stored as-is

**Performance Considerations:**
- Lazy DWARF init: Expensive parsing deferred until first source mapping request
- Static caching: DWARF context cached per process lifetime
- Reference-based filtering: `Vec<&FunctionInfo>` avoids cloning
- Glob compilation once per filter call: Matcher reused for all functions

---

*Architecture analysis: 2026-01-26*
