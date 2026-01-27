# Phase 6: Output Modes - Research

**Researched:** 2026-01-26
**Domain:** Rust CLI argument parsing, JSON serialization, Unix CLI patterns
**Confidence:** HIGH

## Summary

This phase adds CLI output modes (`--json`, `--summary`) to wasm-poke, allowing headless operation for scripting and CI pipelines. The implementation leverages libraries already present in the codebase (clap, serde, serde_json) plus standard library TTY detection. No new dependencies are required for core functionality; indicatif can optionally be added for progress bars.

The architecture requires restructuring main.rs to parse arguments before deciding whether to launch the GUI or produce CLI output. The existing analysis pipeline in lib.rs (parse_wasm_from_bytes, build_call_graph) provides all data needed for output generation.

**Primary recommendation:** Add clap derive arguments to main.rs with output mode enum. When `--json` or `--summary` is specified, skip GUI launch entirely and write to stdout (or file via `-o`).

## Standard Stack

The established libraries/tools for this domain:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4.5 | CLI argument parsing | Already in Cargo.toml, derive API is idiomatic |
| serde | 1.0 | Serialization traits | Already used for model types |
| serde_json | 1.0 | JSON output | Already in Cargo.toml |
| std::io::IsTerminal | stable | TTY detection | Standard library since Rust 1.70, no crate needed |
| std::process::ExitCode | stable | Exit codes | Standard library, works with Termination trait |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| indicatif | 0.17 | Progress bars | For large file progress indicator (>1MB files) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| indicatif | std::io::Write to stderr | Simpler but no progress bar animation |
| clap | lexopt | Less features but smaller binary |
| serde_json | Manual JSON | No serialization, full control (but more code) |

**Installation (if adding indicatif):**
```bash
cargo add indicatif@0.17
```

## Architecture Patterns

### Recommended Project Structure
```
src/
    main.rs          # CLI parsing, mode dispatch (GUI vs output modes)
    lib.rs           # Analysis pipeline (unchanged)
    model.rs         # Data types with Serialize derive (already done)
    output.rs        # NEW: JSON/summary output generation
    cli.rs           # NEW: Clap argument definitions
```

### Pattern 1: Mode Dispatch in main()

**What:** Parse CLI arguments first, then branch to either GUI or output mode
**When to use:** Always - this is the core pattern for this phase

**Example:**
```rust
// Source: https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html
use clap::Parser;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "wasm-poke", version, about)]
struct Cli {
    /// Input .wasm file (or - for stdin)
    file: Option<String>,

    /// Output JSON to stdout
    #[arg(short = 'j', long, group = "output_mode")]
    json: bool,

    /// Output summary to stdout
    #[arg(short = 's', long, group = "output_mode")]
    summary: bool,

    /// Write output to file instead of stdout
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Suppress warnings and progress
    #[arg(short = 'q', long)]
    quiet: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.json || cli.summary {
        // Headless output mode
        match run_output_mode(&cli) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(1)
            }
        }
    } else {
        // GUI mode (original behavior)
        run_gui()
    }
}
```

### Pattern 2: Mutually Exclusive Output Modes

**What:** Use clap ArgGroup to ensure only one output mode is active
**When to use:** When `--json` and `--summary` should conflict

**Example:**
```rust
// Source: https://docs.rs/clap/latest/clap/struct.ArgGroup.html
#[derive(Parser)]
struct Cli {
    #[arg(short = 'j', long, group = "output_mode")]
    json: bool,

    #[arg(short = 's', long, group = "output_mode")]
    summary: bool,
}
// Passing both --json and --summary causes clap error automatically
```

### Pattern 3: JSON Output Structure

**What:** Serde-serializable struct matching user's requested schema
**When to use:** For `--json` output

**Example:**
```rust
// Based on CONTEXT.md decisions
use serde::Serialize;

#[derive(Serialize)]
struct JsonOutput {
    functions: Vec<FunctionOutput>,
    call_graph: CallGraphOutput,
    summary: SummaryOutput,
}

#[derive(Serialize)]
struct FunctionOutput {
    index: u32,
    name: String,           // demangled
    raw_name: Option<String>, // mangled
    code_size: u32,
    percentage: f64,
    exports: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<SourceInfo>,
}

#[derive(Serialize)]
struct CallGraphOutput {
    edges: Vec<[u32; 2]>,  // [[src, dst], ...]
    indirect: Vec<u32>,     // function indices with call_indirect
}

#[derive(Serialize)]
struct SummaryOutput {
    total_code_size: u64,
    function_count: u32,
    imported_functions: u32,
    max_call_depth: u32,
}

// Serialize with pretty printing for human readability
let output = serde_json::to_string_pretty(&json_output)?;
println!("{output}");
```

### Pattern 4: Stdin Reading for Binary Data

**What:** Read wasm bytes from stdin when file is "-"
**When to use:** For pipe support: `cat foo.wasm | wasm-poke --json`

**Example:**
```rust
// Source: https://doc.rust-lang.org/std/io/trait.Read.html
use std::io::{self, Read};

fn load_wasm_bytes(file_arg: &str) -> io::Result<Vec<u8>> {
    if file_arg == "-" {
        let mut buf = Vec::new();
        io::stdin().read_to_end(&mut buf)?;
        Ok(buf)
    } else {
        std::fs::read(file_arg)
    }
}
```

### Pattern 5: TTY Detection for Progress

**What:** Only show progress bar when stderr is a terminal
**When to use:** For progress indicator on large files

**Example:**
```rust
// Source: https://doc.rust-lang.org/beta/std/io/trait.IsTerminal.html
use std::io::{self, IsTerminal};

fn should_show_progress(quiet: bool) -> bool {
    !quiet && io::stderr().is_terminal()
}
```

### Pattern 6: Unix-style Output Separation

**What:** Data to stdout, diagnostics to stderr
**When to use:** Always for CLI output modes

**Example:**
```rust
// Progress/warnings to stderr
eprintln!("warning: large file, this may take a moment...");

// Data to stdout (or file via -o)
println!("{json_output}");
```

### Anti-Patterns to Avoid
- **Mixing data and diagnostics:** Never print warnings/progress to stdout; use stderr
- **Blocking on stdin without TTY check:** Stdin hangs if not piped and user didn't intend stdin
- **Pretty-printing JSON in pipes:** Default to compact JSON, pretty-print only for TTY or explicit flag

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CLI argument parsing | Manual argv parsing | clap derive | Short/long flags, help text, conflicts, validation |
| JSON serialization | Format strings | serde_json | Escaping, nested structures, type safety |
| TTY detection | libc isatty | std::io::IsTerminal | Cross-platform, stable since 1.70 |
| Exit codes | process::exit() | ExitCode return | Cleaner, destructors run properly |
| Progress bars | print!("\r...") | indicatif | Cursor handling, rate limiting, spinner |

**Key insight:** All the hard work (argument conflict detection, JSON escaping, terminal handling) is already solved by standard and well-tested libraries.

## Common Pitfalls

### Pitfall 1: GUI dependencies loaded in headless mode
**What goes wrong:** eframe/egui initialization fails without display
**Why it happens:** Importing gui module unconditionally pulls in display deps
**How to avoid:** Check output mode BEFORE importing gui; use conditional compilation or runtime gating
**Warning signs:** "Failed to initialize display" errors in CI

### Pitfall 2: Stdin blocking unexpectedly
**What goes wrong:** CLI hangs waiting for stdin when user meant to pass a file
**Why it happens:** `-` for stdin isn't obvious; forgetting filename causes hang
**How to avoid:** Require explicit `-` for stdin; show error if no file and stdin isn't piped
**Warning signs:** Program hangs with no output

### Pitfall 3: Exit code 0 on error
**What goes wrong:** Shell scripts don't detect failure
**Why it happens:** Forgetting to return non-zero exit code
**How to avoid:** Use ExitCode::from(1) for all error paths; test with `$?`
**Warning signs:** `wasm-poke --json bad.file && echo "success"` prints success

### Pitfall 4: Progress bar interferes with JSON output
**What goes wrong:** Progress bar output mixed with JSON
**Why it happens:** Both writing to same stream, or progress bar not using stderr
**How to avoid:** indicatif defaults to stderr; verify with explicit ProgressDrawTarget::stderr()
**Warning signs:** JSON parsing fails due to embedded escape sequences

### Pitfall 5: Large file memory issues with stdin
**What goes wrong:** OOM when piping very large wasm files
**Why it happens:** read_to_end loads entire file into memory
**How to avoid:** This is inherent to wasm parsing (need full file); document size limits if needed
**Warning signs:** Memory usage spikes, process killed

## Code Examples

Verified patterns from official sources:

### Complete CLI Definition
```rust
// Source: https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html
use clap::Parser;

#[derive(Parser)]
#[command(
    name = "wasm-poke",
    version,
    about = "WebAssembly function size explorer",
    long_about = "Analyze WebAssembly modules to understand function sizes and call graphs.\n\n\
                  Run without flags to open the GUI. Use --json or --summary for headless output."
)]
struct Cli {
    /// Input .wasm file (use - for stdin)
    #[arg(value_name = "FILE")]
    file: Option<String>,

    /// Output analysis as JSON
    #[arg(short = 'j', long, group = "output_mode")]
    json: bool,

    /// Output human-readable summary
    #[arg(short = 's', long, group = "output_mode")]
    summary: bool,

    /// Write output to file instead of stdout
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<String>,

    /// Suppress progress and warnings
    #[arg(short = 'q', long)]
    quiet: bool,
}
```

### JSON Output Generation
```rust
// Source: https://docs.rs/serde_json/latest/serde_json/fn.to_string_pretty.html
use serde::Serialize;
use serde_json;

fn output_json(
    module: &WasmModuleInfo,
    call_graph: &CallGraph,
    writer: &mut impl std::io::Write,
) -> anyhow::Result<()> {
    let output = build_json_output(module, call_graph);
    let json = serde_json::to_string_pretty(&output)?;
    writeln!(writer, "{json}")?;
    Ok(())
}
```

### Summary Output Generation
```rust
fn output_summary(
    module: &WasmModuleInfo,
    call_graph: &CallGraph,
    writer: &mut impl std::io::Write,
) -> anyhow::Result<()> {
    writeln!(writer, "WebAssembly Module Summary")?;
    writeln!(writer, "==========================")?;
    writeln!(writer)?;
    writeln!(writer, "Functions: {} defined, {} imported",
             module.defined_functions, module.imported_functions)?;
    writeln!(writer, "Total code size: {} bytes", module.total_code_size)?;
    writeln!(writer)?;
    writeln!(writer, "Top 20 functions by size:")?;

    let sorted = wasm_poke::sorted_by_size(module, None);
    for (i, func) in sorted.iter().take(20).enumerate() {
        let pct = module.percentage(func);
        writeln!(writer, "  {:2}. {} - {} bytes ({:.1}%)",
                 i + 1, func.best_name(), func.code_size, pct)?;
    }

    Ok(())
}
```

### Progress Bar for Large Files
```rust
// Source: https://docs.rs/indicatif/latest/indicatif/struct.ProgressBar.html
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal;

fn maybe_progress(file_size: u64, quiet: bool) -> Option<ProgressBar> {
    const LARGE_FILE_THRESHOLD: u64 = 1_000_000; // 1MB

    if quiet || !std::io::stderr().is_terminal() || file_size < LARGE_FILE_THRESHOLD {
        return None;
    }

    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .unwrap());
    pb.set_message("Analyzing...");
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    Some(pb)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| atty crate for TTY | std::io::IsTerminal | Rust 1.70 (2023) | No external dependency needed |
| process::exit() | ExitCode return | Rust 1.61 (2022) | Cleaner shutdown, destructors run |
| clap builder API | clap derive API | clap 3.0 (2021) | Less boilerplate, type-safe |

**Deprecated/outdated:**
- atty crate: Unmaintained, use std::io::IsTerminal
- clap 2.x: Still works but missing derive features

## Open Questions

Things that couldn't be fully resolved:

1. **Call depth calculation**
   - What we know: Need max depth for summary stats
   - What's unclear: Should it handle cycles (recursive functions)?
   - Recommendation: Use BFS with visited set, report "infinite" or cap at module function count for cycles

2. **Source mapping in JSON**
   - What we know: DWARF info available via existing lib.rs functions
   - What's unclear: Performance impact of mapping all functions for large modules
   - Recommendation: Include source info only for functions that have DWARF data; document as optional

3. **Pretty vs compact JSON**
   - What we know: User decisions say JSON output should be structured
   - What's unclear: Whether to default to pretty or compact
   - Recommendation: Use pretty-print (to_string_pretty) for human-readable default; add `--compact` flag later if needed

## Sources

### Primary (HIGH confidence)
- [clap derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html) - CLI argument parsing patterns
- [clap ArgGroup docs](https://docs.rs/clap/latest/clap/struct.ArgGroup.html) - Mutually exclusive flags
- [std::io::IsTerminal](https://doc.rust-lang.org/beta/std/io/trait.IsTerminal.html) - TTY detection
- [std::process::ExitCode](https://doc.rust-lang.org/stable/std/process/struct.ExitCode.html) - Exit code handling
- [serde_json to_string_pretty](https://docs.rs/serde_json/latest/serde_json/fn.to_string_pretty.html) - JSON formatting
- [std::io::Read](https://doc.rust-lang.org/std/io/trait.Read.html) - Stdin reading

### Secondary (MEDIUM confidence)
- [indicatif ProgressBar](https://docs.rs/indicatif/latest/indicatif/struct.ProgressBar.html) - Progress bars to stderr
- [Rain's Rust CLI recommendations](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html) - CLI patterns
- [Command Line Apps in Rust book](https://rust-cli.github.io/book/in-depth/exit-code.html) - Exit codes

### Tertiary (LOW confidence)
- WebSearch results for community patterns (verified against official docs above)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in Cargo.toml or std
- Architecture: HIGH - Clear patterns from clap docs and existing codebase
- Pitfalls: MEDIUM - Based on common patterns and Rust CLI book

**Research date:** 2026-01-26
**Valid until:** 2026-02-26 (30 days - stable libraries)
