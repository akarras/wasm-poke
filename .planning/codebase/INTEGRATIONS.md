# External Integrations

**Analysis Date:** 2026-01-26

## APIs & External Services

**None Detected**

This is a standalone CLI tool with no external API integrations, network requests, or third-party service dependencies. All analysis is performed locally on provided WebAssembly files.

## Data Storage

**Databases:**
- None - application is stateless

**File Storage:**
- Local filesystem only
- Input: reads `.wasm` files from user-specified paths
- Output: JSON/plaintext to stdout or TUI rendering to terminal
- No file writes or persistence layer

**Caching:**
- In-memory caching only (during runtime):
  - Source span cache (DWARF mappings per function)
  - Source file content cache (loaded source files in inspect mode)
  - WAT disassembly cache (function bytecode disassembly)
  - Function name map cache (demangled/export names)
- Single-session lifetime; all caches cleared on exit

**Cache Implementation:** `HashMap`-based (standard library)
- Files: `src/main.rs` (lines 229-240: `App` struct caches)
- No persistent cache between runs

## Authentication & Identity

**Auth Provider:**
- None - application requires no authentication
- No user management or login system
- No identity verification or authorization checks

## Monitoring & Observability

**Error Tracking:**
- None - no external error reporting service

**Logs:**
- All output to stdout/stderr only
- No log files written
- TUI renders directly to terminal (crossterm backend)
- Available output modes:
  - Interactive TUI (ratatui-rendered)
  - JSON output (`--json` flag) to stdout
  - Summary plaintext (`--no-ui` flag) to stdout

**Error Handling:**
- All errors use `anyhow::Result<T>` for context chaining
- Errors printed to stderr and exit with status code
- No error telemetry or remote reporting

## CI/CD & Deployment

**Hosting:**
- None - distributed as standalone binary
- Users build locally: `cargo build --release`
- No cloud infrastructure or deployment pipeline

**CI Pipeline:**
- None detected in this repo
- Tests run locally via `cargo test` with optional feature flag
- No GitHub Actions, GitLab CI, or similar configurations present

**Build Artifacts:**
- Single executable binary (platform-dependent)
- No dependencies bundled; Rust static linking handles all deps
- Portable across systems with same architecture/OS

## Webhooks & Callbacks

**Incoming:** None

**Outgoing:** None

This tool is entirely event-driven by user input (keyboard/mouse in TUI, command-line arguments in CLI mode). No network communication or external callbacks.

## DWARF/Source Mapping Integration

**Debug Information Source:**
- Embedded DWARF sections within input WASM files
- Read via `object::File::parse()` and parsed by `gimli`
- Address-to-source mapping via `addr2line::Context`

**Implementation Details:**
- Global cache: `src/lib.rs` lines 497 (DWARF_CONTEXT: `OnceLock<Mutex<DwarfCtx>>`)
- Initialization: `init_dwarf_context()` (lines 501-564)
- Functions: `map_instr_to_source()`, `map_instr_to_source_fast()`, `function_source_span()`
- No external DWARF sources; all data embedded in input WASM

**Sections Read:**
```
.debug_abbrev, .debug_info, .debug_line, .debug_str,
.debug_ranges, .debug_rnglists, .debug_str_offsets,
.debug_addr, .debug_aranges, .debug_line_str,
.debug_loclists, .debug_loc, .debug_types
```

## Dependencies at Risk

**None Identified**

All dependencies are from official Rust crate ecosystem (`crates.io`), actively maintained:
- `wasmparser` - maintained by Bytecode Alliance
- `ratatui` - actively developed open-source project
- `crossterm` - stable and widely used
- `addr2line` + `gimli` - stable DWARF parsing
- `serde` - core Rust serialization, very stable

No abandoned, forked, or experimental dependencies detected.

---

*Integration audit: 2026-01-26*
