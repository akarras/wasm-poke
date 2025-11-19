use crate::model::{FunctionInfo, WasmModuleInfo};
use anyhow::{anyhow, Context, Result};
use rustc_demangle::try_demangle;
use std::collections::HashMap;
use std::path::Path;
use wasmparser::{BinaryReader, ExternalKind, Name, NameSectionReader, Parser, Payload, TypeRef};

/// Parse a WebAssembly module from the given file path, producing structured size and naming info.
pub fn parse_wasm<P: AsRef<Path>>(path: P) -> Result<WasmModuleInfo> {
    let data = std::fs::read(&path)
        .with_context(|| format!("Failed to read file {}", path.as_ref().display()))?;
    parse_wasm_from_bytes(&data)
}

/// Parse a WebAssembly module from in-memory bytes.
/// This is the core parser used by tests and the CLI.
pub fn parse_wasm_from_bytes(bytes: &[u8]) -> Result<WasmModuleInfo> {
    let mut imported_funcs: u32 = 0;
    let mut defined_funcs_seen: u32 = 0;

    // temp storage while walking the module
    let mut body_sizes: Vec<(
        u32, /*global idx*/
        u32, /*size*/
        Option<std::ops::Range<usize>>,
    )> = Vec::new();
    let mut export_map: HashMap<u32, Vec<String>> = HashMap::new();
    let mut name_map: HashMap<u32, String> = HashMap::new();

    // We use a forward-only parser over the payloads.
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload?;
        match payload {
            Payload::ImportSection(s) => {
                for import in s {
                    let import = import?;
                    if matches!(import.ty, TypeRef::Func(_)) {
                        imported_funcs = imported_funcs
                            .checked_add(1)
                            .ok_or_else(|| anyhow!("imported function count overflow"))?;
                    }
                }
            }
            Payload::ExportSection(s) => {
                for export in s {
                    let export = export?;
                    if export.kind == ExternalKind::Func {
                        export_map
                            .entry(export.index)
                            .or_default()
                            .push(export.name.to_string());
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                // Each entry corresponds to one defined function in order.
                let defined_idx = defined_funcs_seen;
                let global_idx = imported_funcs
                    .checked_add(defined_idx)
                    .ok_or_else(|| anyhow!("function index overflow"))?;

                // size and byte range of the function body
                let r = body.get_binary_reader();
                let size = r.bytes_remaining() as u32;
                let body_range = {
                    let range = r.range();
                    Some(range.start..range.end)
                };

                body_sizes.push((global_idx, size, body_range));
                defined_funcs_seen += 1;
            }
            Payload::CustomSection(cs) => {
                // Parse the "name" custom section if present.
                if cs.name() == "name" {
                    // Safe to parse with NameSectionReader; it expects the raw custom section bytes.
                    let ns = NameSectionReader::new(BinaryReader::new(cs.data(), cs.data_offset()));
                    for sub in ns {
                        match sub? {
                            Name::Function(fnames) => {
                                for naming in fnames {
                                    let naming = naming?;
                                    name_map.insert(naming.index, naming.name.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Build FunctionInfo list from the collected sizes. Attach names and exports.
    let mut total_code_size: u64 = 0;
    let mut functions: Vec<FunctionInfo> = Vec::with_capacity(body_sizes.len());

    for (global_idx, size, range) in body_sizes {
        total_code_size = total_code_size.saturating_add(size as u64);
        // Preferred raw name from the name section, if any.
        let raw_name = name_map.get(&global_idx).cloned();
        // All export names (if any).
        let export_names = export_map.get(&global_idx).cloned().unwrap_or_default();

        // Try to demangle a reasonable candidate: prefer raw name, then first export name.
        let demangled_name = raw_name
            .as_deref()
            .and_then(|n| try_demangle(n).ok().map(|d| d.to_string()))
            .or_else(|| {
                export_names
                    .first()
                    .and_then(|n| try_demangle(n).ok().map(|d| d.to_string()))
            });

        functions.push(FunctionInfo {
            index: global_idx,
            code_size: size,
            body_range: range,
            export_names,
            raw_name,
            demangled_name,
        });
    }

    Ok(WasmModuleInfo {
        total_code_size,
        imported_functions: imported_funcs,
        defined_functions: defined_funcs_seen,
        functions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Parser smoke test on an empty-module-like bytes will fail nicely.
    #[test]
    fn parse_invalid_bytes() {
        let bytes = b"\0asm\x01\0\0\0"; // valid header + empty module
        let res = parse_wasm_from_bytes(bytes);
        // It's valid empty module; no code section means zero functions.
        let info = res.expect("empty module should parse");
        assert_eq!(info.defined_functions, 0);
        assert_eq!(info.functions.len(), 0);
        assert_eq!(info.total_code_size, 0);
    }
}
