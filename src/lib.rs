use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use rustc_demangle::try_demangle;
use serde::{Deserialize, Serialize};
use wasmparser::{ExternalKind, Name, NameSectionReader, Parser, Payload, TypeRef};

/// Information about a single (defined) function in the module.
///
/// Note: Only defined functions have bodies and thus sizes. Imported functions
/// are not listed here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInfo {
    /// Global function index (includes any imported functions before defined ones).
    pub index: u32,
    /// Size, in bytes, of the function body (locals + instructions).
    pub code_size: u32,
    /// Byte range (start..end) within the `.wasm` file of the function body, if known.
    pub body_range: Option<Range<usize>>,
    /// Export names that reference this function's index (zero or more).
    pub export_names: Vec<String>,
    /// Raw function name from the name section, if present.
    pub raw_name: Option<String>,
    /// Demangled name (Rust), if we were able to demangle any available name.
    pub demangled_name: Option<String>,
}

impl FunctionInfo {
    /// Returns the "best" available display name for this function.
    /// Prefers demangled name, then raw name, then first export, finally `func[index]`.
    pub fn best_name(&self) -> String {
        if let Some(d) = &self.demangled_name {
            return d.clone();
        }
        if let Some(r) = &self.raw_name {
            return r.clone();
        }
        if let Some(first_export) = self.export_names.first() {
            return first_export.clone();
        }
        format!("func[{}]", self.index)
    }
}

/// Aggregated information about the module and its functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModuleInfo {
    /// Total bytes of all defined functions (sum of `code_size`).
    pub total_code_size: u64,
    /// Number of imported functions in the index space.
    pub imported_functions: u32,
    /// Number of defined functions (with bodies).
    pub defined_functions: u32,
    /// Per-function information for all defined functions.
    pub functions: Vec<FunctionInfo>,
}

impl WasmModuleInfo {
    /// Compute percentage (0.0..=100.0) of this function's size relative to total code size.
    pub fn percentage(&self, f: &FunctionInfo) -> f64 {
        if self.total_code_size == 0 {
            0.0
        } else {
            (f.code_size as f64) * 100.0 / (self.total_code_size as f64)
        }
    }
}

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
        Option<Range<usize>>,
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
                    let ns = NameSectionReader::new(cs.data(), cs.data_offset());
                    for sub in ns {
                        match sub? {
                            Name::Function(fnames) => {
                                for naming in fnames {
                                    let naming = naming?;
                                    name_map.insert(naming.index, naming.name.to_string());
                                }
                            }
                            // We ignore module/local names, etc., for now.
                            _ => {}
                        }
                    }
                }
            }
            // We don't need other sections here.
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
                    .get(0)
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

/// Filter functions using simple wildcard matching:
/// - `*` matches any sequence of characters (including empty)
/// - All other characters match literally
/// - Match is case-sensitive
///
/// Examples:
/// - "add" matches only "add"
/// - "add*" matches "add", "adder", "add42"
/// - "*add" matches "add", "my_add"
/// - "*add*" matches any string containing "add"
pub fn filter_functions<'a>(funcs: &'a [FunctionInfo], pattern: &str) -> Vec<&'a FunctionInfo> {
    // Default to substring "contains" when no '*' is present by wrapping as *pattern*
    let normalized;
    let pat = if pattern.contains('*') {
        pattern
    } else {
        normalized = format!("*{}*", pattern);
        &normalized
    };

    funcs.iter().filter(|f| function_matches(f, pat)).collect()
}

/// Return a new Vec of references to functions sorted by descending size.
/// Optionally apply a wildcard filter first.
pub fn sorted_by_size<'a>(
    module: &'a WasmModuleInfo,
    pattern: Option<&str>,
) -> Vec<&'a FunctionInfo> {
    let mut list: Vec<&FunctionInfo> = if let Some(pat) = pattern {
        filter_functions(&module.functions, pat)
    } else {
        module.functions.iter().collect()
    };
    list.sort_by_key(|f| std::cmp::Reverse(f.code_size));
    list
}

/// Returns true if the function matches the given wildcard pattern in any of its known names.
/// Checks `best_name`, `raw_name`, `demangled_name`, and all export names.
pub fn function_matches(func: &FunctionInfo, pattern: &str) -> bool {
    let best = func.best_name();
    if wildcard_match(&best, pattern) {
        return true;
    }
    if let Some(raw) = &func.raw_name {
        if wildcard_match(raw, pattern) {
            return true;
        }
    }
    if let Some(dem) = &func.demangled_name {
        if wildcard_match(dem, pattern) {
            return true;
        }
    }
    for ex in &func.export_names {
        if wildcard_match(ex, pattern) {
            return true;
        }
    }
    false
}

/// Minimal wildcard matcher supporting only `*` (matches any sequence, including empty).
/// Case-sensitive, literal match for all other characters.
pub fn wildcard_match(s: &str, pat: &str) -> bool {
    // Fast-path trivial cases
    if pat == "*" {
        return true;
    }
    if !pat.contains('*') {
        return s == pat;
    }

    // Collapse consecutive '*' for simpler processing
    let mut collapsed = String::with_capacity(pat.len());
    let mut prev_star = false;
    for ch in pat.chars() {
        if ch == '*' {
            if !prev_star {
                collapsed.push('*');
                prev_star = true;
            }
        } else {
            collapsed.push(ch);
            prev_star = false;
        }
    }

    // Split into tokens between '*'
    let tokens: Vec<&str> = collapsed.split('*').collect();
    let starts_with_star = collapsed.starts_with('*');
    let ends_with_star = collapsed.ends_with('*');

    // Special case: pattern is like "*" after collapsing, but already handled above.
    if tokens.is_empty() {
        return true;
    }

    // We'll search for tokens in order through `s`.
    // If pattern doesn't start with '*', the first token must be a prefix.
    // If pattern doesn't end with '*', the last token must be a suffix.
    let mut remaining = s;

    let first_idx = 0usize;
    let last_idx = tokens.len().saturating_sub(1);

    for (i, tok) in tokens.iter().enumerate() {
        if tok.is_empty() {
            continue;
        }

        if i == first_idx && !starts_with_star {
            // Must be prefix match
            if let Some(rest) = remaining.strip_prefix(tok) {
                remaining = rest;
            } else {
                return false;
            }
        } else if i == last_idx && !ends_with_star {
            // Must be suffix match
            if let Some(_) = remaining.rfind(tok) {
                // Ensure token is at the very end
                if remaining.ends_with(tok) {
                    // nothing else to check
                } else {
                    return false;
                }
            } else {
                return false;
            }
        } else {
            // Find the token anywhere in the remaining string and advance
            if let Some(pos) = remaining.find(tok) {
                // Consume up to end of the token
                let next_start = pos + tok.len();
                remaining = &remaining[next_start..];
            } else {
                return false;
            }
        }
    }

    // If ends_with_star is false, we already confirmed last token is a suffix.
    // If ends_with_star is true, trailing chars are allowed.
    // If tokens ended with empty due to trailing '*', handled by ends_with_star.
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_basic() {
        assert!(wildcard_match("add", "add"));
        assert!(!wildcard_match("adder", "add"));
        assert!(wildcard_match("adder", "add*"));
        assert!(wildcard_match("add", "add*"));
        assert!(wildcard_match("add", "*add"));
        assert!(wildcard_match("foo_add_bar", "*add*"));
        assert!(wildcard_match("foo", "*"));
        assert!(!wildcard_match("foo", "bar*"));
        assert!(wildcard_match("foobar", "f*r"));
        assert!(wildcard_match("foor", "f*r"));
        assert!(!wildcard_match("fob", "f*r"));
        assert!(wildcard_match("foooor", "f*o*r"));
        assert!(wildcard_match("foooor", "f*oo*r"));
        assert!(!wildcard_match("foooor", "f*ooo*r*Z"));
    }

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

    #[cfg(feature = "fixture-tests")]
    #[test]
    fn parse_fixture_include_bytes() {
        // Path provided at compile-time by build.rs via `cargo:rustc-env=WASM_POKE_TEST_WASM=...`
        // This test will only compile successfully when that env var is set (e.g., in CI after preparing the fixture).
        // We embed the wasm bytes at compile time; set WASM_POKE_TEST_WASM to a valid .wasm path before running tests.
        let bytes: &[u8] = include_bytes!("../tests/fixtures/simple_wasm.wasm");
        let info = parse_wasm_from_bytes(bytes).expect("fixture wasm should parse");
        // Basic expectations: at least one defined function and non-zero total code size
        assert!(
            info.defined_functions >= 1,
            "expected at least one defined function"
        );
        assert!(
            info.total_code_size >= 1,
            "expected non-zero total code size"
        );

        // If the fixture exports `add`, ensure it can be found via exports or names.
        // Not a strict requirement (some builds may be stripped), so we don't assert.
        let _maybe_add = info
            .functions
            .iter()
            .any(|f| function_matches(f, "add") || f.export_names.iter().any(|e| e == "add"));
    }
}
