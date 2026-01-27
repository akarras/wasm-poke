# Phase 6: Output Modes - Context

**Gathered:** 2026-01-26
**Status:** Ready for planning

<domain>
## Phase Boundary

CLI output modes for headless/scripted usage. Users can run wasm-poke with `--json` or `--summary` flags to get structured output without launching the GUI. Enables CI pipeline integration and scripting workflows.

</domain>

<decisions>
## Implementation Decisions

### JSON structure
- Object with sections: `{"functions": [...], "call_graph": {...}, "summary": {...}}`
- Full graph edges in call_graph: `"edges": [[0, 1], [0, 3], ...]` - complete graph structure
- Include source mapping when DWARF info available (file/line for functions)
- Both name formats: `"name": "demangled"`, `"raw_name": "mangled"` - maximum compatibility

### Summary format
- Include both size stats (total, top functions, breakdown) and structural stats (function count, call depth, imports/exports)
- Show top 20 functions by default
- Include percentage of total size: `"my_func: 1234 bytes (12.3%)"`

### Flag design
- Long and short flags: `--json`/`-j`, `--summary`/`-s`
- Output modes are auto-headless: `--json` or `--summary` skips GUI entirely
- Optional output file: `-o`/`--output` to write to file instead of stdout
- Support stdin: `cat foo.wasm | wasm-poke --json` for piping

### Verbosity levels
- Quiet mode: `-q`/`--quiet` suppresses warnings/info
- Progress indicator for large files (>1MB), auto-detect TTY
- Progress/warnings to stderr, data to stdout (proper Unix style)
- Standard Unix exit codes: 0 = success, 1 = error, 2 = usage error

### Claude's Discretion
- Exact JSON field names (following Rust/serde conventions)
- Summary table vs plain text formatting (pick what looks clean)
- Progress bar library choice
- Threshold for "large file" progress indicator

</decisions>

<specifics>
## Specific Ideas

- Should feel like standard Unix CLI tools (pg_dump, jq output style)
- Works in CI pipelines without X11/display
- Exit codes enable shell scripting: `wasm-poke --json foo.wasm && echo "success"`

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-output-modes*
*Context gathered: 2026-01-26*
