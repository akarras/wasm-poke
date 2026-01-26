# Coding Conventions

**Analysis Date:** 2026-01-26

## Naming Patterns

**Files:**
- Lowercase snake_case: `src/lib.rs`, `src/main.rs`, `src/parser.rs`, `src/help.rs`
- No abbreviations in filenames
- Grouped by functional domain (all source in `src/`, all tests in `tests/`)

**Functions:**
- Lowercase snake_case: `parse_wasm()`, `build_call_graph()`, `filter_functions()`, `run_ui_loop()`
- Verbs for actions: `parse_`, `build_`, `filter_`, `draw_`, `run_`, `map_`, `disassemble_`
- Descriptive and explicit: `disassemble_function_wat_bytes()` not `disasm()`
- Public library functions documented with doc comments explaining purpose and return values

**Variables:**
- Lowercase snake_case for locals: `wasm_bytes`, `import_funcs`, `body_sizes`, `file_stats`
- Use descriptive names: `current_inspect_function`, `tree_selected` not `func`, `sel`
- Abbreviations only where context makes meaning clear: `idx` (index), `i` (loop counter), `v` (vector)
- Hungarian notation NOT used; rely on type system and context

**Types/Structs:**
- PascalCase for all struct names: `FunctionInfo`, `WasmModuleInfo`, `CallGraph`, `WatLine`, `TreeRow`
- Descriptive names reflecting domain: `SourceLocation`, `SourceSpan`, `DwarfCtx`, `JsonOutput`
- Derive attributes (Debug, Clone, Serialize, Deserialize) explicitly listed above struct definition

**Constants and Statics:**
- UPPERCASE_SNAKE_CASE: `DWARF_CONTEXT` (global static)
- Docstring explaining purpose

## Code Style

**Formatting:**
- Default Rust style (4-space indentation, no tabs)
- No explicit rustfmt.toml or .prettierrc found; uses Rust defaults
- Line length: observed to be flexible, some lines >100 chars common in match arms

**Linting:**
- No .clippy.toml or explicit clippy config found
- Likely uses default clippy rules during development
- Code shows careful overflow checking with `.checked_add()` and `.saturating_add()` for safety

## Import Organization

**Order (from observed files):**
1. Standard library imports (std::*)
2. External crate imports (wasmparser, anyhow, serde, etc.)
3. Internal module imports (crate::*, pub mod)
4. Conditional/cfg imports last (#[cfg(test)])

**Example from `src/lib.rs`:**
```rust
use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use globset::GlobBuilder;
use object::{Object, ObjectSection};
use rustc_demangle::try_demangle;
use serde::{Deserialize, Serialize};
use wasmparser::{...};

pub mod help;
```

**Module Re-exports:**
- `pub mod help;` in lib.rs makes submodule public
- No wildcard imports (*) in observed code; always explicit imports

## Error Handling

**Primary Pattern: anyhow Result type**
- All fallible functions return `Result<T>` or `Result<()>`
- `anyhow!()` macro for creating errors with context
- `.with_context()` for adding contextual information to errors

**Examples from `src/lib.rs` and `src/main.rs`:**
```rust
// Creating errors with context
pub fn parse_wasm<P: AsRef<Path>>(path: P) -> Result<WasmModuleInfo> {
    let data = std::fs::read(&path)
        .with_context(|| format!("Failed to read file {}", path.as_ref().display()))?;
    parse_wasm_from_bytes(&data)
}

// Overflow checks
imported_funcs = imported_funcs
    .checked_add(1)
    .ok_or_else(|| anyhow!("imported function count overflow"))?;

// Option handling in match
match sub? {
    Name::Function(fnames) => {
        for naming in fnames {
            let naming = naming?;
            // process
        }
    }
    _ => {}
}
```

**Fallback patterns:**
- `.unwrap_or_default()` for default values
- `.unwrap_or()` with explicit fallback
- Early returns with `?` operator preferred over nested matches
- No panics expected in normal operation; errors propagated with `?`

## Logging

**Framework:** Built-in Rust `println!()` only

**Patterns:**
- Debug output via println! in tests: `println!("Offset {}: {} : {}", offset, loc.file, loc.line);`
- No structured logging or log crate dependency
- TUI application uses ratatui for status display instead of logging
- Non-interactive mode (`--no-ui`) prints to stdout

## Comments

**When to Comment:**
- Non-obvious algorithm logic (e.g., temp storage allocation, address translation)
- Why something is done (not what), especially for safety/overflow checks
- Domain-specific explanations (DWARF context, call graph computation)

**Example patterns from `src/lib.rs`:**
```rust
// temp storage while walking the module
let mut body_sizes: Vec<(...,)> = Vec::new();

// We use a forward-only parser over the payloads.
for payload in Parser::new(0).parse_all(bytes) {

// Cached DWARF/addr2line context to avoid reparsing per mapping call
pub struct DwarfCtx {

// Map global index -> size for defined functions
let mut size_map: Map<u32, u32> = Map::with_capacity(module.functions.len());
```

**Avoided:**
- Trivial comments restating code: `let x = 5; // set x to 5`
- Over-documenting self-explanatory code blocks
- Line-end comments except for clarity on complex expressions

## Documentation Comments

**Doc Comments (///):**
- Used extensively on public functions and types
- Full English sentences with explanation of purpose, parameters, and behavior
- Examples in `src/lib.rs`:
```rust
/// Information about a single (defined) function in the module.
///
/// Note: Only defined functions have bodies and thus sizes. Imported functions
/// are not listed here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {

/// Returns the "best" available display name for this function.
/// Prefers demangled name, then raw name, then first export, finally `func[index]`.
pub fn best_name(&self) -> String {

/// Build a direct call graph by scanning operators in each defined function body.
/// - Only direct `call` operators contribute edges
/// - `call_indirect` is recorded via `has_indirect` but no edges are added
pub fn build_call_graph(bytes: &[u8]) -> Result<CallGraph> {
```

## Function Design

**Size:**
- Generally 20-200 lines per function
- Longer functions allowed for complex state machines (e.g., `run_ui_loop` 1600+ lines in main.rs but organized with clear phases)
- Parser and disassembler functions include exhaustive match expressions (long but necessary)

**Parameters:**
- Prefer explicit parameters over global state
- Use borrowing (&) for large data structures (modules, graphs)
- Generic parameters for flexibility: `<P: AsRef<Path>>` for path handling

**Return Values:**
- `Result<T>` for fallible operations
- `Option<T>` for queries that may not find results
- `Vec<T>` for collections that may be empty
- Direct values (no boxing unless required)

**Example from `src/lib.rs`:**
```rust
pub fn filter_functions<'a>(funcs: &'a [FunctionInfo], pattern: &str) -> Vec<&'a FunctionInfo> {
    // Returns borrowed references to avoid cloning function info

pub fn sorted_by_size<'a>(
    module: &'a WasmModuleInfo,
    pattern: Option<&str>,
) -> Vec<&'a FunctionInfo> {
    // Optional pattern parameter; returns owned vec of refs with correct lifetime
```

## Module Design

**Exports:**
- Explicitly declare `pub fn` and `pub struct` for public API
- No re-exports of all items; only what's needed externally
- `pub mod help;` exposes submodule

**Barrel Files:**
- Not used; each module (lib.rs, main.rs, parser.rs) is imported directly
- `src/lib.rs` is the root crate library exposing parser functions and types
- `src/main.rs` stands alone as CLI binary with no re-exports

## Struct Field Design

**Public fields:**
- All struct fields are public by convention
- No getters/setters; direct access
- Example from `src/model.rs`:
```rust
pub struct FunctionInfo {
    pub index: u32,
    pub code_size: u32,
    pub body_range: Option<Range<usize>>,
    pub export_names: Vec<String>,
    pub raw_name: Option<String>,
    pub demangled_name: Option<String>,
}
```

**Type Safety:**
- Use `Option<T>` for optional fields, not null pointers
- Use strong types (u32, u64, usize) for indices and sizes
- Avoid magic numbers; use typed structs

## Lifetime Parameters

**Usage:**
- Explicit lifetimes on borrowed data: `<'a>` for function references in collections
- Lifetime elision used where applicable
- Example from `src/lib.rs`:
```rust
pub fn filter_functions<'a>(funcs: &'a [FunctionInfo], pattern: &str) -> Vec<&'a FunctionInfo>
    // References in return vec must live as long as input slice

pub fn map_instr_to_source_fast(
    module: &WasmModuleInfo,
    wasm_bytes: &[u8],
    func_index: u32,
    body_offset: usize,
) -> Option<SourceLocation>
    // Owned return value; no lifetime needed
```

## Match Expression Style

**Patterns:**
- Exhaustive matching preferred; fallback `_ => {}` when not all variants needed
- Guard clauses used for filtering
- Nested matches acceptable for clarity
- Example from `src/lib.rs`:
```rust
match payload {
    Payload::ImportSection(s) => { /* handle */ }
    Payload::ExportSection(s) => { /* handle */ }
    Payload::CodeSectionEntry(body) => { /* handle */ }
    Payload::CustomSection(cs) => { /* handle */ }
    _ => {}
}
```

---

*Convention analysis: 2026-01-26*
