# Technology Stack

**Analysis Date:** 2026-01-26

## Languages

**Primary:**
- Rust 1.92.0 (stable) - Binary CLI application and library for WebAssembly analysis

## Runtime

**Environment:**
- Rust compiler (rustc 1.92.0)
- Cargo 1.92.0 (package manager and build system)

**Package Manager:**
- Cargo
- Lockfile: `Cargo.lock` (present and version-locked)
- Edition: 2021 (Rust edition 2021)

## Frameworks

**Core:**
- wasmparser 0.241.2 - WebAssembly binary parsing and operator reading
- ratatui 0.29.0 - Terminal UI framework for interactive list/table rendering
- crossterm 0.29.0 - Cross-platform terminal manipulation (raw mode, events)

**Parsing & Analysis:**
- object 0.37.3 - Binary object file parsing (WASM sections, debug info)
- gimli 0.32.3 - DWARF debug information reading
- addr2line 0.25.1 - Source location mapping from DWARF
- rustc-demangle 0.1 - Rust symbol demangling

**Utilities:**
- serde 1.0 + serde_json 1.0 - Serialization for JSON output
- clap 4.5 (with derive feature) - CLI argument parsing
- globset 0.4 - Glob pattern matching for function filtering
- anyhow 1.0 - Error handling and context

## Configuration

**Build Profiles:**
- Default profile: `debug` (standard development)
- `release` profile: `opt-level = 3`, `codegen-units = 1` (optimized for performance)
- `release-debug` profile: inherits from release with `debug = true` (includes debug symbols in optimized build)

**Features:**
- `default`: no default features
- `fixture-tests`: conditional feature for enabling fixture-based tests (requires `tests/fixtures/simple_wasm.wasm`)

**Workspace:**
- Members: `tests/fixtures/*` (includes `simple-wasm` and `repro_crate` crates)
- Structure: monorepo with test fixtures as workspace members

## Platform Requirements

**Development:**
- Rust stable toolchain (stable channel)
- Optional: `wasm32-unknown-unknown` target (for building test fixtures)
  - Installed via: `rustup target add wasm32-unknown-unknown`

**Binary:**
- Linux, macOS, Windows (all tier-1 Rust platforms)
- Terminal with ANSI support (for TUI rendering)
- No external runtime dependencies beyond standard system libraries

**Production:**
- Standalone binary (statically linked, no runtime dependencies)
- Minimum system resources: negligible (primarily CPU-bound for parsing)

## Key Dependencies

**Critical (Direct):**
- `wasmparser` - Core functionality: parsing WASM modules
- `ratatui` - TUI rendering (list, table, layout, styling)
- `crossterm` - Terminal control (raw mode, events, cursor management)
- `serde_json` - JSON serialization for `--json` output mode
- `clap` - CLI interface

**Analysis & Debugging:**
- `addr2line` - Maps bytecode instructions to source files/lines (via DWARF)
- `gimli` - DWARF debug info parsing
- `object` - Object file (ELF/WASM) section extraction
- `rustc-demangle` - Demangles Rust symbol names for display

**Supporting:**
- `globset` - Efficient glob pattern matching for `--filter`
- `anyhow` - Error chaining and context

## Build System

**Build Tool:** Cargo (Rust's official package manager)

**Build Commands:**
```bash
cargo build           # Debug build
cargo build --release # Release build (opt-level=3)
cargo test [--features fixture-tests]  # Run tests
```

**Binary Output:**
- Debug: `target/debug/wasm-poke`
- Release: `target/release/wasm-poke`

**Special Profiles:**
- `release-debug`: Optimized but with debug symbols
  - Built via: `cargo build --profile release-debug`
  - Output: `target/release-debug/wasm-poke`

## Compilation Targets

**Primary Target:** `x86_64-unknown-linux-gnu` (or equivalent for host OS)

**Test Fixture Target:** `wasm32-unknown-unknown`
- Used to compile fixture crates to WebAssembly for testing
- Fixture crates: `simple-wasm`, `repro_crate`
- Output: `target/wasm32-unknown-unknown/[profile]/[crate_name].wasm`

## Environment & Versioning

**Rust Edition:** 2021 (latest stable)

**Dependency Pinning:** All dependencies pinned in `Cargo.lock` (commit-time snapshot)

**No Configuration Files:**
- No `.env` or environment variable requirements for compilation
- No config files (`.config.*`, `.nvmrc`, etc.) detected
- Build configuration entirely in `Cargo.toml`

---

*Stack analysis: 2026-01-26*
