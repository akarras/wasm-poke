# wasm-poke

Interactive WebAssembly function size explorer with Rust demangling, filtering, and a TUI list. Think “twiggy but interactive” focused on functions and easy filtering.

Highlights
- Parses a .wasm and shows each defined function’s body size (bytes) and its percentage of total code.
- Rust name demangling (raw vs demangled toggle).
- Interactive filter with `*` wildcards (case-sensitive).
- TUI list with navigation, plus non-interactive JSON/summary modes for scripting/CI.

Controls (TUI)
- q: quit
- /: enter filter input (use `*` wildcards). Enter to apply. Esc to cancel. Ctrl+u to clear line.
- c: clear filter
- r: toggle raw names vs best name (demangled → raw → export)
- ↑/↓/PgUp/PgDn/Home/End: navigation

CLI
- Interactive (default): wasm-poke path/to/app.wasm
- Non-interactive:
  - wasm-poke path/to/app.wasm --no-ui [--filter <pat>] [--top N]
  - wasm-poke path/to/app.wasm --json [--filter <pat>] [--raw-names]

Filtering semantics
- Exact match with `*` wildcards
  - "add" matches only "add"
  - "add*" matches "add", "adder", "add42"
  - "*add" matches "add", "my_add"
  - "*add*" matches any string containing "add"
- Case-sensitive

Install/build
- Prereqs:
  - Rust stable
  - Optional (for tests): wasm target
    - rustup target add wasm32-unknown-unknown
- Build:
  - cargo build
- Run:
  - cargo run -- path/to/app.wasm
  - cargo run -- path/to/app.wasm --no-ui --filter "*alloc*"
  - cargo run -- path/to/app.wasm --json > report.json

Workspace layout
- Main crate: this package (binary + library).
- Fixture crate: fixtures/simple-wasm (cdylib compiled to wasm); used to generate a small .wasm for tests/examples.

Unit tests (feature-gated include_bytes, copy wasm locally)
By default, fixture-backed tests are disabled. To run them, enable the `fixture-tests` feature and copy the built fixture wasm into `tests/fixtures/simple_wasm.wasm`. This keeps the artifact out of release builds.

Step 1: build the fixture to wasm
- Build the fixture with the wasm32 target:
  - rustup target add wasm32-unknown-unknown
  - cargo build -p simple-wasm --target wasm32-unknown-unknown
- The produced wasm is usually at:
  - target/wasm32-unknown-unknown/debug/simple_wasm.wasm
  - If you’ve set CARGO_TARGET_DIR, use that folder instead.

Step 2: copy the wasm into the repository (so tests can include_bytes it)

- Create the folder if needed: `mkdir -p tests/fixtures` (PowerShell: `New-Item -ItemType Directory -Force tests\\fixtures`)
- Copy: `cp target/wasm32-unknown-unknown/debug/simple_wasm.wasm tests/fixtures/simple_wasm.wasm` (PowerShell: `Copy-Item target\\wasm32-unknown-unknown\\debug\\simple_wasm.wasm tests\\fixtures\\simple_wasm.wasm -Force`)
Step 3: run the tests with the feature enabled

- `cargo test --features fixture-tests`







Notes
- To run fixture-backed tests, ensure `tests/fixtures/simple_wasm.wasm` exists and run `cargo test --features fixture-tests`.
- The library API you can use in your own tests/tools:
  - parse_wasm(path) -> WasmModuleInfo
  - parse_wasm_from_bytes(bytes) -> WasmModuleInfo
  - function_matches(FunctionInfo, pattern) -> bool
  - sorted_by_size(&WasmModuleInfo, Option<&str>) -> Vec<&FunctionInfo>

Output fields (JSON mode)
- index: global function index (imports first, then defined)
- size: body size in bytes (locals + instructions)
- percent: share of total code bytes
- raw_name: from name section if present
- demangled_name: if demangling succeeded
- export_names: any export names pointing to the function
- display_name: best display choice (demangled/raw/export)

Limitations
- Only defined functions (with bodies) have sizes; imported functions do not.
- If the module lacks a name section, only indices and export names are available.
- Filtering is case-sensitive and only supports `*` as a wildcard.

Troubleshooting
- No functions show up: the module might have no code section, or your filter is too restrictive. Press c to clear the filter.
- Names look weird: toggle demangling with r, and remember many release builds have stripped names.
- Windows terminal issues: ensure your terminal supports ANSI; PowerShell/CMD with recent Windows should be fine.

License
- MIT or Apache-2.0 (choose your preference if you add a LICENSE file).
