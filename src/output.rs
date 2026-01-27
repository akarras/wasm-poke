//! Output generation for headless mode.
//!
//! Provides JSON and summary output formatters for `WasmModuleInfo` and `CallGraph`.

use std::io::Write;

use anyhow::Result;
use serde::Serialize;

use wasm_poke::{function_source_span, CallGraph, FunctionInfo, WasmModuleInfo};

/// Complete JSON output structure.
#[derive(Debug, Clone, Serialize)]
pub struct JsonOutput {
    /// Per-function information.
    pub functions: Vec<FunctionOutput>,
    /// Call graph edges and indirect call markers.
    pub call_graph: CallGraphOutput,
    /// Module-level summary statistics.
    pub summary: SummaryOutput,
}

/// JSON representation of a single function.
#[derive(Debug, Clone, Serialize)]
pub struct FunctionOutput {
    /// Global function index.
    pub index: u32,
    /// Best available display name (demangled if possible).
    pub name: String,
    /// Raw name from name section, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_name: Option<String>,
    /// Code size in bytes.
    pub code_size: u32,
    /// Percentage of total code size.
    pub percentage: f64,
    /// Export names that reference this function.
    pub exports: Vec<String>,
    /// Source span if DWARF info available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceOutput>,
}

/// JSON representation of a source span.
#[derive(Debug, Clone, Serialize)]
pub struct SourceOutput {
    /// Source file path.
    pub file: String,
    /// Start line number.
    pub start_line: u32,
    /// End line number.
    pub end_line: u32,
}

/// JSON representation of the call graph.
#[derive(Debug, Clone, Serialize)]
pub struct CallGraphOutput {
    /// Direct call edges as [src, dst] pairs.
    pub edges: Vec<[u32; 2]>,
    /// Function indices that contain indirect calls.
    pub functions_with_indirect: Vec<u32>,
}

/// JSON representation of module summary statistics.
#[derive(Debug, Clone, Serialize)]
pub struct SummaryOutput {
    /// Total code size across all defined functions.
    pub total_code_size: u64,
    /// Number of defined functions (with bodies).
    pub function_count: u32,
    /// Number of imported functions.
    pub imported_functions: u32,
    /// Number of exported functions.
    pub exported_functions: u32,
}

/// Build the complete JSON output structure from module info and call graph.
pub fn build_json_output(
    module: &WasmModuleInfo,
    call_graph: &CallGraph,
    wasm_bytes: &[u8],
) -> JsonOutput {
    // Build function outputs
    let functions: Vec<FunctionOutput> = module
        .functions
        .iter()
        .map(|f| {
            // Try to get source span
            let source = function_source_span(wasm_bytes, f.index)
                .first()
                .map(|span| SourceOutput {
                    file: span.file.clone(),
                    start_line: span.start_line,
                    end_line: span.end_line,
                });

            FunctionOutput {
                index: f.index,
                name: f.best_name(),
                raw_name: f.raw_name.clone(),
                code_size: f.code_size,
                percentage: module.percentage(f),
                exports: f.export_names.clone(),
                source,
            }
        })
        .collect();

    // Flatten call graph edges
    let mut edges: Vec<[u32; 2]> = Vec::new();
    for (&src, dsts) in &call_graph.edges {
        for &dst in dsts {
            edges.push([src, dst]);
        }
    }
    // Sort for deterministic output
    edges.sort();

    // Collect functions with indirect calls
    let mut functions_with_indirect: Vec<u32> = call_graph
        .has_indirect
        .iter()
        .filter_map(|(&idx, &has)| if has { Some(idx) } else { None })
        .collect();
    functions_with_indirect.sort();

    // Count exported functions
    let exported_functions = module
        .functions
        .iter()
        .filter(|f| !f.export_names.is_empty())
        .count() as u32;

    let summary = SummaryOutput {
        total_code_size: module.total_code_size,
        function_count: module.defined_functions,
        imported_functions: module.imported_functions,
        exported_functions,
    };

    let call_graph_output = CallGraphOutput {
        edges,
        functions_with_indirect,
    };

    JsonOutput {
        functions,
        call_graph: call_graph_output,
        summary,
    }
}

/// Output module info and call graph as pretty-printed JSON.
pub fn output_json(
    module: &WasmModuleInfo,
    call_graph: &CallGraph,
    wasm_bytes: &[u8],
    writer: &mut impl Write,
) -> Result<()> {
    let output = build_json_output(module, call_graph, wasm_bytes);
    let json = serde_json::to_string_pretty(&output)?;
    writeln!(writer, "{}", json)?;
    Ok(())
}

/// Output a human-readable summary of the module.
pub fn output_summary(module: &WasmModuleInfo, writer: &mut impl Write) -> Result<()> {
    writeln!(writer, "WebAssembly Module Summary")?;
    writeln!(writer, "==========================")?;
    writeln!(writer)?;

    // Statistics
    writeln!(writer, "Functions:")?;
    writeln!(writer, "  Defined:  {}", module.defined_functions)?;
    writeln!(writer, "  Imported: {}", module.imported_functions)?;

    let exported_count = module
        .functions
        .iter()
        .filter(|f| !f.export_names.is_empty())
        .count();
    writeln!(writer, "  Exported: {}", exported_count)?;
    writeln!(writer)?;

    writeln!(writer, "Total code size: {} bytes", module.total_code_size)?;
    writeln!(writer)?;

    // Top 20 functions by size
    writeln!(writer, "Top 20 Functions by Size:")?;
    writeln!(writer, "--------------------------")?;

    let mut sorted: Vec<&FunctionInfo> = module.functions.iter().collect();
    sorted.sort_by(|a, b| b.code_size.cmp(&a.code_size));

    for (i, func) in sorted.iter().take(20).enumerate() {
        let pct = module.percentage(func);
        writeln!(
            writer,
            "  {:2}. {} - {} bytes ({:.1}%)",
            i + 1,
            func.best_name(),
            func.code_size,
            pct
        )?;
    }

    if sorted.len() > 20 {
        writeln!(writer)?;
        writeln!(writer, "  ... and {} more functions", sorted.len() - 20)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_test_module() -> WasmModuleInfo {
        WasmModuleInfo {
            total_code_size: 1000,
            imported_functions: 2,
            defined_functions: 3,
            functions: vec![
                FunctionInfo {
                    index: 2,
                    code_size: 500,
                    body_range: None,
                    export_names: vec!["main".to_string()],
                    raw_name: Some("_start".to_string()),
                    demangled_name: Some("main".to_string()),
                },
                FunctionInfo {
                    index: 3,
                    code_size: 300,
                    body_range: None,
                    export_names: vec![],
                    raw_name: Some("helper".to_string()),
                    demangled_name: None,
                },
                FunctionInfo {
                    index: 4,
                    code_size: 200,
                    body_range: None,
                    export_names: vec![],
                    raw_name: None,
                    demangled_name: None,
                },
            ],
        }
    }

    fn make_test_call_graph() -> CallGraph {
        let mut edges = HashMap::new();
        edges.insert(2, vec![3, 4]);
        edges.insert(3, vec![4]);

        let mut has_indirect = HashMap::new();
        has_indirect.insert(2, false);
        has_indirect.insert(3, true);
        has_indirect.insert(4, false);

        CallGraph { edges, has_indirect }
    }

    #[test]
    fn json_output_structure() {
        let module = make_test_module();
        let call_graph = make_test_call_graph();
        let wasm_bytes: &[u8] = &[]; // Empty, no DWARF

        let output = build_json_output(&module, &call_graph, wasm_bytes);

        assert_eq!(output.functions.len(), 3);
        assert_eq!(output.summary.total_code_size, 1000);
        assert_eq!(output.summary.function_count, 3);
        assert_eq!(output.summary.imported_functions, 2);
        assert_eq!(output.summary.exported_functions, 1);

        // Check edges are flattened
        assert_eq!(output.call_graph.edges.len(), 3);
        assert!(output.call_graph.edges.contains(&[2, 3]));
        assert!(output.call_graph.edges.contains(&[2, 4]));
        assert!(output.call_graph.edges.contains(&[3, 4]));

        // Check indirect
        assert_eq!(output.call_graph.functions_with_indirect, vec![3]);
    }

    #[test]
    fn json_output_serializes() {
        let module = make_test_module();
        let call_graph = make_test_call_graph();
        let wasm_bytes: &[u8] = &[];

        let mut buffer = Vec::new();
        output_json(&module, &call_graph, wasm_bytes, &mut buffer).unwrap();

        let json_str = String::from_utf8(buffer).unwrap();
        assert!(json_str.contains("\"total_code_size\": 1000"));
        assert!(json_str.contains("\"name\": \"main\""));
    }

    #[test]
    fn summary_output_format() {
        let module = make_test_module();

        let mut buffer = Vec::new();
        output_summary(&module, &mut buffer).unwrap();

        let output = String::from_utf8(buffer).unwrap();
        assert!(output.contains("WebAssembly Module Summary"));
        assert!(output.contains("Defined:  3"));
        assert!(output.contains("Imported: 2"));
        assert!(output.contains("Exported: 1"));
        assert!(output.contains("Total code size: 1000 bytes"));
        assert!(output.contains("main - 500 bytes"));
    }
}
