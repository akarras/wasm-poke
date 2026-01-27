//! CLI argument parsing for headless operation.
//!
//! Provides `Cli` struct for parsing command-line arguments using clap derive,
//! with support for `--json` and `--summary` output modes.

use clap::Parser;

/// Output mode determined by CLI flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// No output flags - launch the GUI (default).
    #[default]
    Gui,
    /// `--json` specified - output structured JSON.
    Json,
    /// `--summary` specified - output human-readable summary.
    Summary,
}

/// WebAssembly function size explorer.
///
/// wasm-poke helps you understand where code size comes from in your
/// WebAssembly binaries. Use it interactively with the GUI, or in
/// headless mode with --json or --summary for CI/scripting.
#[derive(Parser, Debug)]
#[command(
    name = "wasm-poke",
    version,
    about = "WebAssembly function size explorer",
    long_about = "wasm-poke helps you understand where code size comes from in your \
WebAssembly binaries.\n\n\
Without output flags: Launches the interactive GUI.\n\
With --json or --summary: Runs in headless mode, outputs to stdout (or file with -o)."
)]
pub struct Cli {
    /// Input .wasm file to analyze.
    ///
    /// If not provided, the GUI will open without a file loaded.
    /// Use "-" to read from stdin (headless mode only).
    #[arg(value_name = "FILE")]
    pub file: Option<String>,

    /// Output structured JSON with function info and call graph.
    ///
    /// Mutually exclusive with --summary.
    #[arg(short = 'j', long, group = "output_mode")]
    pub json: bool,

    /// Output human-readable summary with function list.
    ///
    /// Mutually exclusive with --json.
    #[arg(short = 's', long, group = "output_mode")]
    pub summary: bool,

    /// Write output to file instead of stdout.
    ///
    /// Only valid with --json or --summary.
    #[arg(short = 'o', long, value_name = "PATH")]
    pub output: Option<String>,

    /// Suppress progress messages and warnings.
    ///
    /// Only affects stderr output; --json/--summary output is unaffected.
    #[arg(short = 'q', long)]
    pub quiet: bool,
}

impl Cli {
    /// Determine the output mode based on CLI flags.
    pub fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else if self.summary {
            OutputMode::Summary
        } else {
            OutputMode::Gui
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_parses_without_args() {
        let cli = Cli::try_parse_from(["wasm-poke"]).unwrap();
        assert!(cli.file.is_none());
        assert!(!cli.json);
        assert!(!cli.summary);
        assert_eq!(cli.output_mode(), OutputMode::Gui);
    }

    #[test]
    fn cli_parses_file_arg() {
        let cli = Cli::try_parse_from(["wasm-poke", "test.wasm"]).unwrap();
        assert_eq!(cli.file.as_deref(), Some("test.wasm"));
        assert_eq!(cli.output_mode(), OutputMode::Gui);
    }

    #[test]
    fn cli_parses_json_flag() {
        let cli = Cli::try_parse_from(["wasm-poke", "--json", "test.wasm"]).unwrap();
        assert!(cli.json);
        assert!(!cli.summary);
        assert_eq!(cli.output_mode(), OutputMode::Json);
    }

    #[test]
    fn cli_parses_summary_flag() {
        let cli = Cli::try_parse_from(["wasm-poke", "-s", "test.wasm"]).unwrap();
        assert!(!cli.json);
        assert!(cli.summary);
        assert_eq!(cli.output_mode(), OutputMode::Summary);
    }

    #[test]
    fn cli_rejects_both_json_and_summary() {
        let result = Cli::try_parse_from(["wasm-poke", "--json", "--summary", "test.wasm"]);
        assert!(result.is_err());
    }

    #[test]
    fn cli_parses_output_option() {
        let cli = Cli::try_parse_from(["wasm-poke", "--json", "-o", "out.json", "test.wasm"]).unwrap();
        assert_eq!(cli.output.as_deref(), Some("out.json"));
    }

    #[test]
    fn cli_parses_quiet_flag() {
        let cli = Cli::try_parse_from(["wasm-poke", "-q", "--json", "test.wasm"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn cli_command_is_valid() {
        Cli::command().debug_assert();
    }
}
