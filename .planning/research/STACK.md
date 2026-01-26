# Technology Stack: egui GUI for wasm-poke

**Project:** wasm-poke egui GUI milestone
**Researched:** 2026-01-26
**Mode:** Stack research for adding egui GUI (native + web) to existing CLI tool

## Executive Summary

The recommended stack uses **egui 0.33.x** with **eframe** for dual native/web targeting. Use the **glow** backend for smaller WASM binary sizes (wgpu is becoming the default but adds binary bloat). Trunk handles web builds with zero-config. The existing Rust analysis code (wasmparser, gimli, addr2line) remains unchanged.

## Recommended Stack

### Core GUI Framework

| Technology | Version | Purpose | Confidence |
|------------|---------|---------|------------|
| egui | 0.33.3 | Immediate-mode GUI library | HIGH |
| eframe | 0.33.3 | Application framework for native + web | HIGH |
| egui_extras | 0.33.3 | Tables, syntax highlighting, extended widgets | HIGH |

**Why egui/eframe:**
- Single codebase compiles to both native desktop and WebAssembly
- Immediate-mode paradigm fits data-inspection UIs well
- Active development, strong community, well-documented
- The project author (Emil Ernerfeldt) works at Rerun.io building production egui apps

**Source:** [egui GitHub releases](https://github.com/emilk/egui/releases), [eframe docs](https://docs.rs/eframe/latest/eframe/)

### Rendering Backend

| Technology | Version | Purpose | Confidence |
|------------|---------|---------|------------|
| glow | 0.16.x | OpenGL rendering backend | HIGH |

**Why glow over wgpu:**
- Smaller WASM binary size (critical for web deployment)
- Faster compile times, fewer dependencies
- wgpu is becoming the default in egui, but adds ~1-2MB to WASM binaries
- For a data inspection tool, we don't need wgpu's advanced 3D capabilities

**Note:** As of April 2025, egui is transitioning to wgpu as the default backend. For now, explicitly opt into glow for this project's use case.

**Source:** [egui wgpu transition discussion](https://github.com/emilk/egui/issues/5889)

### Web Build Toolchain

| Technology | Version | Purpose | Confidence |
|------------|---------|---------|------------|
| trunk | 0.21.x | WASM bundler and dev server | HIGH |
| wasm-bindgen | (auto) | JS interop, managed by trunk | HIGH |
| wasm-opt | (auto) | Binary optimization, managed by trunk | HIGH |

**Why Trunk:**
- Zero-config setup for egui/eframe projects
- Automatically manages wasm-bindgen and wasm-opt
- Hot reloading during development
- Recommended by egui documentation

**Source:** [Trunk docs](https://trunkrs.dev/), [eframe_template](https://github.com/emilk/eframe_template)

### File Dialogs

| Technology | Version | Purpose | Confidence |
|------------|---------|---------|------------|
| rfd | 0.15.x | Native file dialogs (native + web) | MEDIUM |

**Why rfd:**
- Cross-platform file picker that works on native and WASM
- Async API required for WASM (browser file API is async)
- On web, uses browser's native file picker

**Caveat:** On WASM, file dialogs are async-only. Use with `wasm_bindgen_futures::spawn_local` or poll-promise pattern.

**Source:** [rfd GitHub](https://github.com/PolyMeilex/rfd)

### Async Handling

| Technology | Version | Purpose | Confidence |
|------------|---------|---------|------------|
| poll-promise | 0.3.x | Polling async results in immediate-mode UI | MEDIUM |

**Why poll-promise:**
- Designed for immediate-mode GUIs where you can't block on futures
- Works on both native (with tokio) and WASM (with wasm-bindgen-futures)
- Recommended by egui documentation for async patterns

**Source:** [poll-promise GitHub](https://github.com/EmbarkStudios/poll-promise)

### Supporting Libraries (Web-specific)

| Library | Version | Purpose | When Used |
|---------|---------|---------|-----------|
| wasm-bindgen-futures | 0.4.x | Async runtime for WASM | Web target only |
| web-sys | 0.3.x | Browser API bindings | Web target only |
| console_error_panic_hook | 0.1.x | Better panic messages in browser console | Web target only |

### Existing Dependencies (Unchanged)

| Library | Version | Purpose |
|---------|---------|---------|
| wasmparser | 0.241.2 | Parse WebAssembly binary format |
| gimli | 0.32.3 | DWARF debug info parsing |
| addr2line | 0.25.1 | DWARF to source location mapping |
| object | 0.37.3 | Object file parsing |
| rustc-demangle | 0.1.x | Demangle Rust symbols |
| serde | 1.0.x | Serialization |

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| GUI Framework | egui/eframe | iced | iced has less mature WASM support |
| GUI Framework | egui/eframe | dioxus | More complex, not immediate-mode |
| Backend | glow | wgpu | Larger WASM binaries, overkill for 2D UI |
| Web bundler | trunk | wasm-pack | Trunk is simpler, better egui integration |
| Async | poll-promise | tokio | tokio adds significant binary size on WASM |

## Cargo.toml Structure

### Recommended Dependencies

```toml
[package]
name = "wasm-poke"
version = "0.2.0"
edition = "2021"
rust-version = "1.81"

[features]
default = ["gui"]
gui = ["dep:eframe", "dep:egui_extras", "dep:rfd", "dep:poll-promise"]

[dependencies]
# Core analysis (existing)
wasmparser = "0.241.2"
rustc-demangle = "0.1"
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
addr2line = "0.25.1"
object = { version = "0.37.3", features = ["read", "wasm"] }
gimli = { version = "0.32.3", features = ["read"] }
globset = "0.4"

# CLI (keep for headless mode)
clap = { version = "4.5", features = ["derive"] }

# GUI dependencies
eframe = { version = "0.33", optional = true, default-features = false, features = [
    "accesskit",
    "default_fonts",
    "glow",
    "persistence",
] }
egui_extras = { version = "0.33", optional = true, features = ["syntect"] }
rfd = { version = "0.15", optional = true }
poll-promise = { version = "0.3", optional = true }
log = "0.4"

# Native-only dependencies
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
env_logger = "0.11"

# Web-only dependencies
[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-futures = "0.4"
web-sys = "0.3"
console_error_panic_hook = "0.1"
```

### Platform-Specific Features

```toml
# Linux/BSD native
[target.'cfg(target_os = "linux")'.dependencies]
eframe = { version = "0.33", optional = true, features = ["wayland", "x11"] }
```

## Project Structure Recommendation

```
wasm-poke/
  src/
    lib.rs           # Core analysis (existing, unchanged)
    main.rs          # Entry point (detect GUI vs CLI mode)
    cli/
      mod.rs         # CLI interface (existing ratatui code or simplified)
    gui/
      mod.rs         # GUI module root
      app.rs         # eframe::App implementation
      views/
        mod.rs
        function_list.rs    # Function table view
        disassembly.rs      # WAT/hex view
        source_view.rs      # Source code pane
        call_graph.rs       # Call graph tree
      widgets/
        mod.rs
        search_bar.rs
        hex_viewer.rs
  web/
    index.html       # Web entry point
  Trunk.toml         # Trunk build configuration
```

## Build Commands

### Native

```bash
# Development
cargo run --features gui

# Release
cargo run --release --features gui
```

### Web (WASM)

```bash
# Install trunk (once)
cargo install --locked trunk

# Development server with hot reload
trunk serve

# Production build
trunk build --release
```

## Web Build Configuration

### Trunk.toml

```toml
[build]
target = "index.html"
dist = "dist"
filehash = false

[watch]
ignore = ["./target"]

[[hooks]]
stage = "post_build"
command = "sh"
command_arguments = ["-c", "echo 'Build complete!'"]
```

### index.html

```html
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>wasm-poke</title>
    <link data-trunk rel="rust" data-wasm-opt="2" />
    <link data-trunk rel="css" href="style.css" />
</head>
<body>
    <canvas id="the_canvas_id"></canvas>
</body>
</html>
```

## Critical Implementation Notes

### 1. Conditional Compilation Pattern

```rust
// In main.rs
fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        // Check for --no-gui flag or if stdin is not a tty
        if should_use_cli() {
            run_cli();
        } else {
            run_gui_native();
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        run_gui_web();
    }
}
```

### 2. File Loading on Web

On WASM, file system access works differently:
- Use `rfd::AsyncFileDialog` with `wasm_bindgen_futures::spawn_local`
- Files are loaded into memory via browser's File API
- No direct file path access - store file contents in app state

### 3. Syntax Highlighting Strategy

egui_extras provides `syntax_highlighting` module with syntect support:

```rust
use egui_extras::syntax_highlighting::{highlight, CodeTheme};

let theme = CodeTheme::from_memory(ctx, &style);
let layout_job = highlight(ctx, &theme, code, "wat"); // or "rust"
ui.label(layout_job);
```

**Note:** syntect adds ~500KB-1MB to WASM binary. Consider lazy loading or using simpler keyword-based highlighting for web.

### 4. Table Performance

For large function lists (1000s of functions), use egui_extras TableBuilder with virtualization:

```rust
TableBuilder::new(ui)
    .striped(true)
    .resizable(true)
    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
    .column(Column::auto().at_least(40.0).resizable(true))
    // ... more columns
    .body(|body| {
        body.rows(row_height, num_rows, |mut row| {
            // Only renders visible rows
        });
    });
```

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| egui/eframe core | HIGH | Verified via official docs and releases |
| glow vs wgpu | HIGH | Verified via GitHub discussions |
| Trunk setup | HIGH | Verified via eframe_template |
| rfd for files | MEDIUM | Works but async patterns need care on WASM |
| poll-promise | MEDIUM | Standard approach but alternatives exist |
| Binary size | MEDIUM | syntect bloat is documented, need to test |

## Open Questions for Implementation

1. **Syntect binary size:** Need to measure actual WASM size with/without syntect feature
2. **File drop support:** egui supports drag-and-drop but needs testing for this use case
3. **Large file handling:** May need streaming/chunked parsing for very large WASM files on web
4. **State persistence:** eframe supports localStorage persistence on web - useful for settings

## Sources

- [egui GitHub](https://github.com/emilk/egui)
- [eframe documentation](https://docs.rs/eframe/latest/eframe/)
- [egui_extras docs](https://docs.rs/egui_extras/latest/egui_extras/)
- [eframe_template](https://github.com/emilk/eframe_template)
- [Trunk bundler](https://trunkrs.dev/)
- [rfd crate](https://github.com/PolyMeilex/rfd)
- [poll-promise](https://github.com/EmbarkStudios/poll-promise)
- [egui wgpu transition](https://github.com/emilk/egui/issues/5889)
