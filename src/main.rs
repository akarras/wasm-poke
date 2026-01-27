//! wasm-poke: Interactive WebAssembly function size explorer.
//!
//! This is the main entry point for both the GUI application and headless CLI modes.
//!
//! Usage:
//! - `wasm-poke` - Launch the interactive GUI
//! - `wasm-poke <file.wasm>` - Launch GUI with file pre-loaded
//! - `wasm-poke --json <file.wasm>` - Output JSON to stdout
//! - `wasm-poke --summary <file.wasm>` - Output text summary to stdout
//! - `wasm-poke --json -o output.json <file.wasm>` - Output JSON to file

use std::io::{Read, Write};
use std::process::ExitCode;

use clap::Parser;

mod cli;
mod gui;
mod output;

use cli::{Cli, OutputMode};
use gui::WasmPokeApp;

fn main() -> ExitCode {
    // Parse CLI arguments FIRST (before any GUI initialization)
    let cli = Cli::parse();

    // Dispatch based on output mode
    match cli.output_mode() {
        OutputMode::Gui => run_gui(cli.file),
        OutputMode::Json => run_json_output(&cli),
        OutputMode::Summary => run_summary_output(&cli),
    }
}

/// Launch the interactive GUI.
fn run_gui(file: Option<String>) -> ExitCode {
    use eframe::egui;

    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let result = eframe::run_native(
        "wasm-poke",
        native_options,
        Box::new(move |cc| {
            let mut app = WasmPokeApp::new(cc);
            // Auto-load file if provided on command line
            if let Some(path) = file {
                app.load_wasm_from_path(&path);
            }
            Ok(Box::new(app))
        }),
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(1)
        }
    }
}

/// Run headless JSON output mode.
fn run_json_output(cli: &Cli) -> ExitCode {
    // Require a file argument for headless mode
    let file_arg = match &cli.file {
        Some(f) => f.as_str(),
        None => {
            eprintln!("Error: --json requires a file argument");
            eprintln!("Usage: wasm-poke --json <file.wasm>");
            eprintln!("       wasm-poke --json - < input.wasm  (read from stdin)");
            return ExitCode::from(2);
        }
    };

    // Load wasm bytes
    let wasm_bytes = match load_wasm_bytes(file_arg) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Error loading wasm: {}", e);
            return ExitCode::from(1);
        }
    };

    // Parse module info
    let module = match wasm_poke::parse_wasm_from_bytes(&wasm_bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error parsing wasm: {}", e);
            return ExitCode::from(1);
        }
    };

    // Build call graph
    let call_graph = match wasm_poke::build_call_graph(&wasm_bytes) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Error building call graph: {}", e);
            return ExitCode::from(1);
        }
    };

    // Get output writer
    let mut writer: Box<dyn Write> = match get_output_writer(&cli.output) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error opening output file: {}", e);
            return ExitCode::from(1);
        }
    };

    // Output JSON
    if let Err(e) = output::output_json(&module, &call_graph, &wasm_bytes, &mut writer) {
        eprintln!("Error writing JSON: {}", e);
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

/// Run headless summary output mode.
fn run_summary_output(cli: &Cli) -> ExitCode {
    // Require a file argument for headless mode
    let file_arg = match &cli.file {
        Some(f) => f.as_str(),
        None => {
            eprintln!("Error: --summary requires a file argument");
            eprintln!("Usage: wasm-poke --summary <file.wasm>");
            eprintln!("       wasm-poke --summary - < input.wasm  (read from stdin)");
            return ExitCode::from(2);
        }
    };

    // Load wasm bytes
    let wasm_bytes = match load_wasm_bytes(file_arg) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Error loading wasm: {}", e);
            return ExitCode::from(1);
        }
    };

    // Parse module info
    let module = match wasm_poke::parse_wasm_from_bytes(&wasm_bytes) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error parsing wasm: {}", e);
            return ExitCode::from(1);
        }
    };

    // Get output writer
    let mut writer: Box<dyn Write> = match get_output_writer(&cli.output) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error opening output file: {}", e);
            return ExitCode::from(1);
        }
    };

    // Output summary
    if let Err(e) = output::output_summary(&module, &mut writer) {
        eprintln!("Error writing summary: {}", e);
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

/// Load wasm bytes from a file path or stdin (if "-").
fn load_wasm_bytes(file_arg: &str) -> std::io::Result<Vec<u8>> {
    if file_arg == "-" {
        // Read from stdin
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        Ok(buffer)
    } else {
        // Read from file
        std::fs::read(file_arg)
    }
}

/// Get an output writer for the given path, or stdout if None.
fn get_output_writer(output_path: &Option<String>) -> std::io::Result<Box<dyn Write>> {
    match output_path {
        None => Ok(Box::new(std::io::stdout())),
        Some(path) => {
            let file = std::fs::File::create(path)?;
            Ok(Box::new(file))
        }
    }
}
