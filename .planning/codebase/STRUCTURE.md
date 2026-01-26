# Codebase Structure

**Analysis Date:** 2026-01-26

## Directory Layout

```
wasm-poke/
├── src/                          # Main Rust source
│   ├── lib.rs                    # Core library: parsing, analysis, inspection
│   ├── main.rs                   # CLI and TUI implementation
│   ├── model.rs                  # Data structures (FunctionInfo, WasmModuleInfo, etc.)
│   ├── parser.rs                 # WASM binary parser (imported functions, code, exports, names)
│   └── help.rs                   # Instruction reference documentation
│
├── tests/                        # Integration and unit tests
│   ├── search_test.rs            # Tests for function_matches() filtering
│   ├── source_map_test.rs        # Tests for DWARF source mapping
│   │
│   └── fixtures/                 # Test fixtures
│       ├── simple-wasm/          # Small WASM crate for testing (cdylib)
│       │   ├── Cargo.toml
│       │   └── src/lib.rs        # Minimal WASM exports for size testing
│       │
│       └── repro_crate/          # Multi-module fixture
│           ├── Cargo.toml
│           └── src/
│               ├── lib.rs
│               └── other.rs
│
├── Cargo.toml                    # Workspace and package configuration
├── Cargo.lock                    # Dependency lock file
├── README.md                     # User-facing documentation
└── LICENSE                       # MIT or Apache-2.0
```

## Directory Purposes

**src/**
- Purpose: All Rust source code for library and binary
- Contains: Parser, model, analysis, UI, utilities
- Key files: `lib.rs` (1609 lines), `main.rs` (2082 lines)

**tests/**
- Purpose: Integration and unit tests plus test fixtures
- Contains: Test code and minimal WASM crates for fixture generation
- Key files: `search_test.rs`, `source_map_test.rs`, fixture crates

**tests/fixtures/**
- Purpose: Fixture WASM crates compiled to wasm32-unknown-unknown
- Contains: Minimal `no_std` Rust code that compiles to deterministic WASM
- Usage: Tests include compiled `.wasm` bytes via `include_bytes!()` when feature `fixture-tests` is enabled

## Key File Locations

**Entry Points:**

- `src/main.rs`: Binary entry point
  - `fn main()`: CLI parsing via clap, mode routing (TUI vs JSON vs summary)
  - `fn run_tui()`: Interactive terminal UI using ratatui + crossterm
  - `fn run_non_interactive()`: JSON or summary text output
  - 2082 lines; handles all user interaction

**Core Library:**

- `src/lib.rs`: Main library (1609 lines)
  - Parsing: `parse_wasm()`, `parse_wasm_from_bytes()` (via re-export from parser.rs in some sections)
  - Filtering: `filter_functions()`, `function_matches()`, `sorted_by_size()`
  - Call graphs: `build_call_graph()`, `unique_cumulative_size()`
  - Source mapping: `init_dwarf_context()`, `map_instr_to_source()`, `map_instr_to_source_fast()`, `function_source_span()`
  - Disassembly: `disassemble_function_wat_bytes()`, `disassemble_function_wat_lines()`, `hexdump()`
  - Utilities: `function_body_bytes()`, helper types

**Models:**

- `src/model.rs`: Data structures (73 lines)
  - `FunctionInfo`: Single function metadata
  - `WasmModuleInfo`: Module aggregate
  - `CallGraph`: Call relationship graph

**Parsing (duplicated across files):**

- `src/parser.rs`: Standalone parser module (149 lines)
  - `parse_wasm()`, `parse_wasm_from_bytes()` with tests
  - Identical to lib.rs parsing; kept for modularity

**Utilities:**

- `src/help.rs`: WASM instruction documentation (199 lines)
  - `get_instruction_help()`: Returns brief description for each instruction mnemonic

**Configuration:**

- `Cargo.toml`: Package metadata, dependencies, workspace members
  - Workspace includes fixture crates
  - Feature gate: `fixture-tests` enables include_bytes! tests

## Naming Conventions

**Files:**
- Snake case: `src/main.rs`, `src/lib.rs`, `src/help.rs`
- Test files: `search_test.rs`, `source_map_test.rs`
- Fixtures in subdirectories: `tests/fixtures/simple-wasm/`, `tests/fixtures/repro_crate/`

**Modules:**
- `src/lib.rs`: Public module declarations via `pub mod help;`
- `src/main.rs`: All code in main; imports via `use wasm_poke::{...}`

**Functions:**
- Snake case throughout (Rust convention)
- Public API: `parse_wasm()`, `filter_functions()`, `function_matches()`, `build_call_graph()`, `map_instr_to_source()`
- Internal helpers: Prefix with underscore or module-local visibility

**Types:**
- PascalCase structs: `FunctionInfo`, `WasmModuleInfo`, `CallGraph`, `SourceLocation`, `SourceSpan`
- Generic and lifetime annotations where needed: `filter_functions<'a>()` returns `Vec<&'a FunctionInfo>`

**Variables:**
- Snake case: `module`, `wasm_bytes`, `func_index`, `body_offset`
- One-letter for loop counters: `i`, `n`
- Descriptive names for collections: `functions`, `export_map`, `name_map`, `graph`

## Where to Add New Code

**New Feature - Analysis Algorithm:**
- File: `src/lib.rs`
- Pattern: Add as public function taking `&WasmModuleInfo` and/or `&CallGraph` as input
- Example: `fn cumulative_size(...)` or `fn function_reachability(...)`
- Return: Result or value type matching existing conventions (use Option for nullable, Result for errors)

**New Feature - Source Mapping Enhancement:**
- File: `src/lib.rs` (section starting at line 473 "Inspect utilities: DWARF/source mapping")
- Pattern: Add function using cached `DWARF_CONTEXT` via `init_dwarf_context()`
- Example: `fn source_range_to_function()` or `fn all_instr_locations()`
- Cache: Reuse `DWARF_CONTEXT` static; do not recreate per call

**New Feature - CLI Command/Mode:**
- File: `src/main.rs`
- Pattern:
  1. Add CLI argument to `struct Cli` using clap derive macros
  2. Add conditional in `main()` to route to new mode function
  3. Implement mode function: `fn run_new_mode(cli: &Cli, module: &WasmModuleInfo) -> Result<()>`
- Example: Add `--inspect-control-flow` flag that calls new analysis function

**New Feature - UI Component (TUI):**
- File: `src/main.rs` (TUI implementation spans ~1200-2000 lines)
- Pattern: Add new widget or view following existing ratatui patterns
- Examples: Graph visualization tab, source code panel, call tree view
- State management: Add fields to local state in `run_tui()` for widget state

**New Test:**
- File:
  - Logic tests: `tests/search_test.rs` or new file `tests/analysis_test.rs`
  - Fixture-dependent: Requires compiled WASM; enable with `--features fixture-tests`
- Pattern: Use `make_func()` helper to construct FunctionInfo, or include fixture bytes
- Example: Test new filtering logic by calling `filter_functions()` with test data

**Shared Utilities:**
- File: `src/lib.rs` or new `src/util.rs`
- Pattern: Export via `pub use` if in separate module
- Example: `fn glob_match()`, `fn saturating_u32_sum()`

**New Fixture (for new tests):**
- Directory: `tests/fixtures/your_crate/`
- Files: Create Cargo.toml (member of workspace) and src/lib.rs (#![no_std] cdylib)
- Build: `cargo build -p your_crate --target wasm32-unknown-unknown`
- Copy: Move `.wasm` to `tests/fixtures/your_crate.wasm`
- Include: In test file: `include_bytes!("../fixtures/your_crate.wasm")`

## Special Directories

**target/**
- Purpose: Build artifacts (binaries, libraries, dependencies)
- Generated: Yes (by cargo)
- Committed: No (.gitignore excludes)

**tests/fixtures/**
- Purpose: Minimal WASM crates and compiled `.wasm` binaries for testing
- Generated: `.wasm` files generated by `cargo build --target wasm32-unknown-unknown`
- Committed: Source code committed; `.wasm` files copied locally for test runs with feature gate

**.planning/**
- Purpose: GSD planning documents (this structure document, architecture, concerns, etc.)
- Generated: Yes (by GSD tools)
- Committed: Yes

## Import Organization

**src/lib.rs** (top of file):
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
use addr2line::Context as Addr2LineContext;
use gimli::{self, ...};

pub mod help;
```

**Order:**
1. Standard library (std)
2. External crates (alphabetical)
3. Internal modules (pub mod)

**src/main.rs** imports:
```rust
use std::io;
use std::path::PathBuf;
use std::time::{...};

use anyhow::{...};
use clap::{...};
use crossterm::{...};
use ratatui::{...};

use wasm_poke::{...};
```

## Code Organization Notes

- **Duplicate parser**: `src/parser.rs` contains identical parsing logic to `src/lib.rs`. Parser logic is in both places; clarify in future refactor (consolidate or clarify purpose).
- **TUI monolith**: `src/main.rs` contains all UI code (not extracted to separate module yet). Candidates for extraction: event loop, widget rendering, state management.
- **No top-level mod.rs**: Crate uses mixed module style (lib.rs for library, main.rs for binary, parser.rs and help.rs as submodules).
- **Feature gate**: Fixture tests disabled by default (feature `fixture-tests`); prevents large `.wasm` binaries in default builds.

---

*Structure analysis: 2026-01-26*
