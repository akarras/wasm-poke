use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use object::{Object, ObjectSection};
use rustc_demangle::try_demangle;
use serde::{Deserialize, Serialize};
use wasmparser::{ExternalKind, Name, NameSectionReader, Operator, Parser, Payload, TypeRef};

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

/// Call graph of direct calls between functions identified by global indices.
/// - `edges[src] = Vec<dst>` where each entry is a direct `call` target
/// - `has_indirect[src] = true` if the function contains any `call_indirect`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallGraph {
    pub edges: HashMap<u32, Vec<u32>>,
    pub has_indirect: HashMap<u32, bool>,
}

/// Build a direct call graph by scanning operators in each defined function body.
/// - Only direct `call` operators contribute edges
/// - `call_indirect` is recorded via `has_indirect` but no edges are added
pub fn build_call_graph(bytes: &[u8]) -> Result<CallGraph> {
    let mut imported_funcs: u32 = 0;
    let mut defined_funcs_seen: u32 = 0;

    let mut graph = CallGraph::default();

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
            Payload::CodeSectionEntry(body) => {
                // current function (global index)
                let defined_idx = defined_funcs_seen;
                let src = imported_funcs
                    .checked_add(defined_idx)
                    .ok_or_else(|| anyhow!("function index overflow"))?;

                // ensure entries
                graph.edges.entry(src).or_default();
                graph.has_indirect.entry(src).or_insert(false);

                // iterate operators and collect direct call targets
                let mut ops = body.get_operators_reader()?;
                while !ops.eof() {
                    let op = ops.read()?;
                    match op {
                        Operator::Call { function_index } => {
                            graph.edges.entry(src).or_default().push(function_index);
                        }
                        Operator::CallIndirect { .. } => {
                            graph.has_indirect.insert(src, true);
                        }
                        _ => {}
                    }
                }

                defined_funcs_seen += 1;
            }
            _ => {}
        }
    }

    Ok(graph)
}

/// Compute the cumulative unique size (in bytes) reachable from `root` via direct calls.
/// - Sizes are taken from `module` (defined functions only); imports have no size
/// - Each reachable function's size is counted at most once (no double counting)
/// Returns (total_bytes, unique_node_count)
pub fn unique_cumulative_size(
    root: u32,
    module: &WasmModuleInfo,
    graph: &CallGraph,
) -> (u64, usize) {
    use std::collections::{HashMap as Map, HashSet};

    // Map global index -> size for defined functions
    let mut size_map: Map<u32, u32> = Map::with_capacity(module.functions.len());
    for f in &module.functions {
        size_map.insert(f.index, f.code_size);
    }

    let mut visited: HashSet<u32> = HashSet::new();
    let mut stack: Vec<u32> = vec![root];
    let mut total: u64 = 0;

    while let Some(node) = stack.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.insert(node);
        if let Some(sz) = size_map.get(&node) {
            total = total.saturating_add(*sz as u64);
        }
        if let Some(children) = graph.edges.get(&node) {
            // push children in reverse so earlier ones appear first when popping
            for &c in children.iter().rev() {
                if !visited.contains(&c) {
                    stack.push(c);
                }
            }
        }
    }

    (total, visited.len())
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

// Inspect utilities
//
// Source mapping (DWARF) API
//
// Note: This is a placeholder API for mapping instruction offsets to source locations.
// Full DWARF parsing and address translation will be added in a subsequent change.
// The function returns None until DWARF/source map resolution is implemented.

// Cached DWARF/addr2line context to avoid reparsing per mapping call and to normalize addresses
// relative to the first function body. Wrapped in a Mutex to satisfy Sync bounds for global statics.
struct DwarfCtx {
    ctx: addr2line::Context<gimli::EndianSlice<'static, gimli::LittleEndian>>,
    first_body_start: usize,
}

static DWARF_CONTEXT: OnceLock<Mutex<DwarfCtx>> = OnceLock::new();

/// Initialize and cache a global DWARF context and the first function body start offset.
/// Returns a reference to the cached context on success.
pub fn init_dwarf_context(wasm_bytes: &[u8]) -> Option<&'static Mutex<DwarfCtx>> {
    if let Some(existing) = DWARF_CONTEXT.get() {
        return Some(existing);
    }

    // Parse sections once and leak them to obtain 'static lifetimes for EndianSlice.
    let obj = object::File::parse(wasm_bytes).ok()?;
    let mut sec_map: std::collections::HashMap<&'static str, &'static [u8]> =
        std::collections::HashMap::new();

    for name in [
        ".debug_abbrev",
        ".debug_info",
        ".debug_line",
        ".debug_str",
        ".debug_ranges",
        ".debug_rnglists",
        ".debug_str_offsets",
        ".debug_addr",
        ".debug_aranges",
        ".debug_line_str",
        ".debug_loclists",
        ".debug_loc",
        ".debug_types",
    ] {
        if let Some(sec) = obj.section_by_name(name) {
            let data = if let Ok(cow) = sec.uncompressed_data() {
                cow.into_owned()
            } else {
                match sec.data() {
                    Ok(d) => d.to_vec(),
                    Err(_) => Vec::new(),
                }
            };
            if !data.is_empty() {
                let leaked: &'static [u8] = Box::leak(data.into_boxed_slice());
                sec_map.insert(name, leaked);
            }
        }
    }

    let endian = gimli::LittleEndian;
    let dwarf = gimli::Dwarf::load(|id| {
        let name = id.name();
        let data = sec_map.get(name).copied().unwrap_or(&[]);
        Ok::<gimli::EndianSlice<'static, gimli::LittleEndian>, gimli::Error>(
            gimli::EndianSlice::new(data, endian),
        )
    })
    .ok()?;

    let ctx = addr2line::Context::from_dwarf(dwarf).ok()?;

    // Compute first function body start once for module-relative address translation
    let mut first_body_start: usize = 0;
    for payload in wasmparser::Parser::new(0).parse_all(wasm_bytes) {
        if let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload {
            first_body_start = body.get_binary_reader().range().start;
            break;
        }
    }

    let _ = DWARF_CONTEXT.set(Mutex::new(DwarfCtx {
        ctx,
        first_body_start,
    }));
    DWARF_CONTEXT.get()
}

/// Fast mapping helper that avoids scanning the entire wasm by using precomputed module body ranges.

pub fn map_instr_to_source_fast(
    module: &WasmModuleInfo,
    wasm_bytes: &[u8],
    func_index: u32,
    body_offset: usize,
) -> Option<SourceLocation> {
    let f = module.functions.iter().find(|f| f.index == func_index)?;

    let range = f.body_range.as_ref()?;

    let dc = if let Some(m) = DWARF_CONTEXT.get() {
        m.lock().ok()?
    } else {
        init_dwarf_context(wasm_bytes)?.lock().ok()?
    };

    let base = range.start.saturating_sub(dc.first_body_start);

    let address = (base + body_offset) as u64;

    let mut loc = dc.ctx.find_location(address).ok().flatten();

    if loc.is_none() {
        let max_probe = 4096usize;

        for delta in 0..max_probe {
            let probe_addr = (base + delta) as u64;

            if let Ok(found) = dc.ctx.find_location(probe_addr) {
                if found.is_some() {
                    loc = found;

                    break;
                }
            }
        }
    }

    let loc = loc?;

    let file = loc
        .file
        .as_ref()
        .map(|f| f.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());

    let line = loc.line.unwrap_or(0);

    let column = loc.column.unwrap_or(0);

    Some(SourceLocation { file, line, column })
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct SourceLocation {
    pub file: String,

    pub line: u32,

    pub column: u32,
}

/// Attempts to map a global function index and an operator offset within its body to a source location.
/// - `wasm_bytes`: the entire wasm module bytes
/// - `func_index`: global function index for the target function
/// - `body_offset`: byte offset within the function body (relative to the start of the body)
/// Returns a SourceLocation if debug information is available and can be resolved.

// Cached DWARF/addr2line context to avoid reparsing per mapping call and
// to normalize addresses relative to the first function body.

/// Initialize and cache a global DWARF context and the first function body start offset.
/// Returns a reference to the cached context on success.

pub fn map_instr_to_source(
    wasm_bytes: &[u8],

    func_index: u32,

    body_offset: usize,
) -> Option<SourceLocation> {
    // Ensure DWARF context is cached and locked
    let dc = if let Some(m) = DWARF_CONTEXT.get() {
        m.lock().ok()?
    } else {
        init_dwarf_context(wasm_bytes)?.lock().ok()?
    };

    // Determine function body start

    let mut imported_funcs: u32 = 0;

    let mut defined_funcs_seen: u32 = 0;

    let mut func_body_start: Option<usize> = None;

    for payload in wasmparser::Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.ok()?;

        match payload {
            wasmparser::Payload::ImportSection(s) => {
                for import in s {
                    let import = import.ok()?;

                    if matches!(import.ty, wasmparser::TypeRef::Func(_)) {
                        imported_funcs = imported_funcs.checked_add(1)?;
                    }
                }
            }

            wasmparser::Payload::CodeSectionEntry(body) => {
                let r = body.get_binary_reader();

                let range = r.range();

                let this_global = imported_funcs.checked_add(defined_funcs_seen)?;

                if this_global == func_index {
                    func_body_start = Some(range.start);

                    break;
                }

                defined_funcs_seen += 1;
            }

            _ => {}
        }
    }

    let body_start = func_body_start?;

    // Normalize to module-relative address space used by addr2line

    let base = body_start.saturating_sub(dc.first_body_start);

    let address = (base + body_offset) as u64;

    // Resolve address; if not found, probe within a small window
    let mut loc = dc.ctx.find_location(address).ok().flatten();

    if loc.is_none() {
        for delta in 0..4096usize {
            let probe_addr = (base + delta) as u64;

            if let Ok(found) = dc.ctx.find_location(probe_addr) {
                if found.is_some() {
                    loc = found;

                    break;
                }
            }
        }
    }

    let loc = loc?;

    let file = loc
        .file
        .as_ref()
        .map(|f| f.to_string())
        .unwrap_or_else(|| "<unknown>".to_string());

    let line = loc.line.unwrap_or(0);

    let column = loc.column.unwrap_or(0);

    Some(SourceLocation { file, line, column })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// Compute the best-effort source span (file and start/end lines) for an entire function by
/// probing DWARF mappings across the function body. Returns None if no mapping is found.
/// Notes:
/// - Uses map_instr_to_source over function-body-relative offsets, picking the dominant file
///   (most occurrences) and min/max line range for that file.
/// - Columns are best-effort (min for start, max for end on their respective lines).

pub fn function_source_span(wasm_bytes: &[u8], func_index: u32) -> Option<SourceSpan> {
    use std::collections::HashMap as Map;

    // Use global DWARF context (Mutex-backed) for all spans.
    let dc = if let Some(m) = DWARF_CONTEXT.get() {
        m.lock().ok()?
    } else {
        init_dwarf_context(wasm_bytes)?.lock().ok()?
    };

    // Locate function body start/end (module walk is cheap; DWARF stays cached).
    let mut imported_funcs: u32 = 0;
    let mut defined_funcs_seen: u32 = 0;
    let mut func_body_start: Option<usize> = None;
    let mut func_body_end: Option<usize> = None;

    for payload in wasmparser::Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.ok()?;
        match payload {
            wasmparser::Payload::ImportSection(s) => {
                for import in s {
                    let import = import.ok()?;

                    if matches!(import.ty, wasmparser::TypeRef::Func(_)) {
                        imported_funcs = imported_funcs.checked_add(1)?;
                    }
                }
            }

            wasmparser::Payload::CodeSectionEntry(body) => {
                let r = body.get_binary_reader();

                let range = r.range();

                let this_global = imported_funcs.checked_add(defined_funcs_seen)?;

                if this_global == func_index {
                    func_body_start = Some(range.start);

                    func_body_end = Some(range.end);

                    break;
                }

                defined_funcs_seen += 1;
            }

            _ => {}
        }
    }

    let start = func_body_start?;

    let end = func_body_end?;

    let body_len = end.saturating_sub(start);

    if body_len == 0 {
        return None;
    }

    // Probe offsets across the body to gather source lines grouped by file

    // Map: file -> (count, min_line, min_col, max_line, max_col)

    let mut file_stats: Map<String, (u32, u32, u32, u32, u32)> = Map::new();

    // Normalize to the module-relative "address space" used by addr2line context.
    let base = start.saturating_sub(dc.first_body_start);

    // Sample offsets 0..min(body_len, 8192) stepping by 4 bytes (coarse but fast)

    let max_probe = body_len.min(8192);

    let mut offset = 0usize;

    while offset < max_probe {
        let address = (base + offset) as u64;
        if let Ok(loc_opt) = dc.ctx.find_location(address) {
            if let Some(loc) = loc_opt {
                let entry = file_stats
                    .entry(
                        loc.file
                            .as_ref()
                            .map(|f| f.to_string())
                            .unwrap_or_else(|| "<unknown>".to_string()),
                    )
                    .or_insert((0, loc.line.unwrap_or(0), loc.column.unwrap_or(0), 0, 0));
                // Update counters and ranges
                entry.0 = entry.0.saturating_add(1);

                if let Some(l) = loc.line {
                    entry.1 = entry.1.min(l);
                    entry.3 = entry.3.max(l);
                }
                if let Some(c) = loc.column {
                    entry.2 = entry.2.min(c);
                    entry.4 = entry.4.max(c);
                }
            }
        }
        offset += 4;
    }

    if file_stats.is_empty() {
        return None;
    }

    // Choose the dominant file by count

    let (file, (_count, min_line, min_col, max_line, max_col)) =
        file_stats.into_iter().max_by_key(|(_, v)| v.0).unwrap();

    Some(SourceSpan {
        file,

        start_line: min_line,

        start_column: min_col,

        end_line: max_line,

        end_column: max_col,
    })
}

pub fn function_body_bytes<'a>(
    module: &WasmModuleInfo,
    wasm_bytes: &'a [u8],
    func_index: u32,
) -> Option<&'a [u8]> {
    let f = module.functions.iter().find(|f| f.index == func_index)?;

    let range = f.body_range.as_ref()?;

    if range.end <= wasm_bytes.len() {
        Some(&wasm_bytes[range.start..range.end])
    } else {
        None
    }
}

pub fn map_instr_to_source_cached(
    module: &WasmModuleInfo,
    wasm_bytes: &[u8],
    func_index: u32,
    body_offset: usize,
) -> Option<SourceLocation> {
    map_instr_to_source_fast(module, wasm_bytes, func_index, body_offset)
}

/// Convenience: map a structured WAT line (carrying body-relative offset) to a SourceLocation.
pub fn map_wat_line_to_source_cached(
    module: &WasmModuleInfo,
    wasm_bytes: &[u8],
    func_index: u32,
    wl: &WatLine,
) -> Option<SourceLocation> {
    map_instr_to_source_cached(module, wasm_bytes, func_index, wl.offset)
}

/// Produce a simple hex dump (with ASCII gutter) of the provided bytes.
/// `width` controls the number of bytes per line (commonly 16).
pub fn hexdump(bytes: &[u8], width: usize) -> String {
    let mut out = String::new();
    if width == 0 {
        return out;
    }
    let mut offset: usize = 0;
    while offset < bytes.len() {
        let end = (offset + width).min(bytes.len());
        let slice = &bytes[offset..end];

        // offset
        out.push_str(&format!("{:08x}: ", offset));

        // hex bytes
        for i in 0..width {
            if offset + i < end {
                out.push_str(&format!("{:02x} ", slice[i]));
            } else {
                out.push_str("   ");
            }
        }

        // ascii gutter
        out.push_str(" |");
        for &b in slice {
            let ch = if b.is_ascii_graphic() || b == b' ' {
                b as char
            } else {
                '.'
            };
            out.push(ch);
        }
        out.push_str("|\n");

        offset += width;
    }
    out
}

/// Disassemble a single function into a readable, WAT-like representation.
/// Notes:
/// - This is not a full WAT printer; it lists locals and operators using wasmparser's operator decoding.
/// - Useful for quick inspection alongside the hex dump.
/// - Only direct function selection by global index is supported.
/// - Indirect calls will appear as `CallIndirect` operators.
pub fn disassemble_function_wat_bytes(wasm_bytes: &[u8], target_func_index: u32) -> Result<String> {
    let mut imported_funcs: u32 = 0;
    let mut defined_funcs_seen: u32 = 0;

    for payload in Parser::new(0).parse_all(wasm_bytes) {
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
            Payload::CodeSectionEntry(body) => {
                let defined_idx = defined_funcs_seen;
                let this_global = imported_funcs
                    .checked_add(defined_idx)
                    .ok_or_else(|| anyhow!("function index overflow"))?;

                if this_global == target_func_index {
                    let mut out = String::new();
                    out.push_str(&format!(";; func [{}]\n", target_func_index));
                    out.push_str("(func\n");

                    // Locals
                    let mut locals_reader = body.get_locals_reader()?;
                    if locals_reader.get_count() > 0 {
                        out.push_str("  ;; locals:\n");
                        for _ in 0..locals_reader.get_count() {
                            let (cnt, ty) = locals_reader.read()?;
                            out.push_str(&format!("  ;;   count={} type={:?}\n", cnt, ty));
                        }
                    }

                    // Operators (pretty WAT-like mnemonics with brief comments + per-instruction source and indentation)
                    out.push_str("  ;; body\n");
                    // Determine the absolute start of this function body in the wasm for offset mapping
                    let br2 = body.get_binary_reader();
                    let body_range = br2.range();
                    let func_body_start = body_range.start;
                    let mut ops = body.get_operators_reader()?;
                    let mut indent: usize = 0;
                    while !ops.eof() {
                        // read operator with its absolute offset in the wasm bytes
                        let (op, off) = ops.read_with_offset()?;
                        // compute function-body-relative offset
                        let rel_off = off.saturating_sub(func_body_start);
                        // best-effort source location for this operator
                        let src = map_instr_to_source(wasm_bytes, target_func_index, rel_off);
                        let src_comment = src
                            .as_ref()
                            .map(|l| format!("  ;; {}:{}", l.file, l.line))
                            .unwrap_or_default();
                        // compute indentation string
                        // Handle Else/End adjustments before printing
                        let mut current_indent = indent;
                        match op {
                            Operator::Else => {
                                if indent > 0 {
                                    indent -= 1;
                                }
                                current_indent = indent;
                            }
                            Operator::End => {
                                if indent > 0 {
                                    indent -= 1;
                                }
                                current_indent = indent;
                            }
                            _ => {}
                        }
                        let pad = "  ".repeat(current_indent);

                        match op {
                            // constants
                            Operator::I32Const { value } => {
                                out.push_str(&format!("{pad}i32.const {value}{src_comment}\n"));
                            }
                            Operator::I64Const { value } => {
                                out.push_str(&format!("{pad}i64.const {value}{src_comment}\n"));
                            }
                            Operator::F32Const { value } => {
                                let bits = value.bits();
                                let val = f32::from_bits(bits);
                                out.push_str(&format!(
                                    "{pad}f32.const {}  ;; bits=0x{:08x}\n",
                                    val, bits
                                ));
                            }
                            Operator::F64Const { value } => {
                                let bits = value.bits();
                                let val = f64::from_bits(bits);
                                out.push_str(&format!(
                                    "{pad}f64.const {}  ;; bits=0x{:016x}\n",
                                    val, bits
                                ));
                            }

                            // locals/globals
                            Operator::LocalGet { local_index } => {
                                out.push_str(&format!(
                                    "{pad}local.get {local_index}{src_comment}\n"
                                ));
                            }
                            Operator::LocalSet { local_index } => {
                                out.push_str(&format!(
                                    "{pad}local.set {local_index}{src_comment}\n"
                                ));
                            }
                            Operator::LocalTee { local_index } => {
                                out.push_str(&format!(
                                    "{pad}local.tee {local_index}{src_comment}\n"
                                ));
                            }
                            Operator::GlobalGet { global_index } => {
                                out.push_str(&format!(
                                    "{pad}global.get {global_index}{src_comment}\n"
                                ));
                            }
                            Operator::GlobalSet { global_index } => {
                                out.push_str(&format!(
                                    "{pad}global.set {global_index}{src_comment}\n"
                                ));
                            }

                            // calls
                            Operator::Call { function_index } => {
                                out.push_str(&format!("{pad}call {function_index}{src_comment}\n"));
                            }
                            Operator::CallIndirect { .. } => {
                                out.push_str(&format!("{pad}call_indirect  ;; indirect call\n"));
                            }

                            // numeric ops (common)
                            Operator::I32Add => {
                                out.push_str(&format!("{pad}i32.add{src_comment}\n"))
                            }
                            Operator::I32Sub => {
                                out.push_str(&format!("{pad}i32.sub{src_comment}\n"))
                            }
                            Operator::I32Mul => {
                                out.push_str(&format!("{pad}i32.mul{src_comment}\n"))
                            }
                            Operator::I64Add => {
                                out.push_str(&format!("{pad}i64.add{src_comment}\n"))
                            }
                            Operator::I64Sub => {
                                out.push_str(&format!("{pad}i64.sub{src_comment}\n"))
                            }
                            Operator::I64Mul => {
                                out.push_str(&format!("{pad}i64.mul{src_comment}\n"))
                            }
                            Operator::F32Add => {
                                out.push_str(&format!("{pad}f32.add{src_comment}\n"))
                            }
                            Operator::F32Sub => {
                                out.push_str(&format!("{pad}f32.sub{src_comment}\n"))
                            }
                            Operator::F32Mul => {
                                out.push_str(&format!("{pad}f32.mul{src_comment}\n"))
                            }
                            Operator::F64Add => {
                                out.push_str(&format!("{pad}f64.add{src_comment}\n"))
                            }
                            Operator::F64Sub => {
                                out.push_str(&format!("{pad}f64.sub{src_comment}\n"))
                            }
                            Operator::F64Mul => {
                                out.push_str(&format!("{pad}f64.mul{src_comment}\n"))
                            }

                            // comparisons (subset)
                            Operator::I32Eq => out.push_str(&format!("{pad}i32.eq{src_comment}\n")),
                            Operator::I32Ne => out.push_str(&format!("{pad}i32.ne{src_comment}\n")),
                            Operator::I32LtS => {
                                out.push_str(&format!("{pad}i32.lt_s{src_comment}\n"))
                            }
                            Operator::I32LtU => {
                                out.push_str(&format!("{pad}i32.lt_u{src_comment}\n"))
                            }
                            Operator::I32GtS => {
                                out.push_str(&format!("{pad}i32.gt_s{src_comment}\n"))
                            }
                            Operator::I32GtU => {
                                out.push_str(&format!("{pad}i32.gt_u{src_comment}\n"))
                            }

                            // memory loads/stores (subset)
                            Operator::I32Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}i32.load offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::I64Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}i64.load offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::F32Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}f32.load offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::F64Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}f64.load offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::I32Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}i32.store offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::I64Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}i64.store offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::F32Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}f32.store offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }
                            Operator::F64Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                out.push_str(&format!(
                                    "{pad}f64.store offset={} align={}{src_comment}\n",
                                    memarg.offset, align
                                ));
                            }

                            // control flow with indentation
                            Operator::Block { .. } => {
                                out.push_str(&format!("{pad}block{src_comment}\n"));
                                indent = indent.saturating_add(1);
                            }
                            Operator::Loop { .. } => {
                                out.push_str(&format!("{pad}loop{src_comment}\n"));
                                indent = indent.saturating_add(1);
                            }
                            Operator::If { .. } => {
                                out.push_str(&format!("{pad}if{src_comment}\n"));
                                indent = indent.saturating_add(1);
                            }
                            Operator::Else => {
                                out.push_str(&format!("{pad}else{src_comment}\n"));
                                indent = indent.saturating_add(1);
                            }
                            Operator::End => {
                                out.push_str(&format!("{pad}end{src_comment}\n"));
                            }
                            Operator::Br { relative_depth } => {
                                out.push_str(&format!("{pad}br {relative_depth}{src_comment}\n"));
                            }
                            Operator::BrIf { relative_depth } => {
                                out.push_str(&format!(
                                    "{pad}br_if {relative_depth}{src_comment}\n"
                                ));
                            }
                            Operator::BrTable { .. } => {
                                out.push_str(&format!("{pad}br_table  ;; branch table\n"));
                            }

                            // misc
                            Operator::Return => {
                                out.push_str(&format!("{pad}return{src_comment}\n"))
                            }
                            Operator::Drop => out.push_str(&format!("{pad}drop{src_comment}\n")),
                            Operator::Select => {
                                out.push_str(&format!("{pad}select{src_comment}\n"))
                            }
                            Operator::Nop => out.push_str(&format!("{pad}nop{src_comment}\n")),
                            Operator::Unreachable => {
                                out.push_str(&format!("{pad}unreachable{src_comment}\n"))
                            }
                            Operator::MemoryGrow { .. } => {
                                out.push_str(&format!("{pad}memory.grow{src_comment}\n"))
                            }
                            Operator::MemorySize { .. } => {
                                out.push_str(&format!("{pad}memory.size{src_comment}\n"))
                            }

                            // default: leave a comment with the raw debug form
                            _ => {
                                out.push_str(&format!("{pad};; {:?}{src_comment}\n", op));
                            }
                        }
                    }

                    out.push_str(")\n");
                    return Ok(out);
                }

                defined_funcs_seen += 1;
            }
            _ => {}
        }
    }

    Err(anyhow!("function index {} not found", target_func_index))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatLine {
    pub text: String,
    pub offset: usize,               // function-body-relative byte offset
    pub indent: usize,               // visual indentation level
    pub src: Option<SourceLocation>, // mapped source location if available
}

/// Disassemble a single function into structured WAT-like lines.
/// - Returns header/locals/body markers and a closing paren as separate lines.
/// - Operator lines include indentation and carry function-body-relative offsets and optional source locations.
/// - No inline source comments are embedded in `text`; consumers can use `src` to coordinate highlighting.
pub fn disassemble_function_wat_lines(
    wasm_bytes: &[u8],
    target_func_index: u32,
) -> Result<Vec<WatLine>> {
    let mut imported_funcs: u32 = 0;
    let mut defined_funcs_seen: u32 = 0;

    for payload in Parser::new(0).parse_all(wasm_bytes) {
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
            Payload::CodeSectionEntry(body) => {
                let defined_idx = defined_funcs_seen;
                let this_global = imported_funcs
                    .checked_add(defined_idx)
                    .ok_or_else(|| anyhow!("function index overflow"))?;

                if this_global == target_func_index {
                    let mut lines: Vec<WatLine> = Vec::new();

                    // Header
                    lines.push(WatLine {
                        text: format!(";; func [{}]", target_func_index),
                        offset: 0,
                        indent: 0,
                        src: None,
                    });
                    lines.push(WatLine {
                        text: "(func".to_string(),
                        offset: 0,
                        indent: 0,
                        src: None,
                    });

                    // Locals
                    let mut locals_reader = body.get_locals_reader()?;
                    if locals_reader.get_count() > 0 {
                        lines.push(WatLine {
                            text: "  ;; locals:".to_string(),
                            offset: 0,
                            indent: 0,
                            src: None,
                        });
                        for _ in 0..locals_reader.get_count() {
                            let (cnt, ty) = locals_reader.read()?;
                            lines.push(WatLine {
                                text: format!("  ;;   count={} type={:?}", cnt, ty),
                                offset: 0,
                                indent: 0,
                                src: None,
                            });
                        }
                    }

                    // Operators body marker
                    lines.push(WatLine {
                        text: "  ;; body".to_string(),
                        offset: 0,
                        indent: 0,
                        src: None,
                    });

                    // Operators: WAT-like mnemonics with indentation, carrying offsets and src
                    let br2 = body.get_binary_reader();
                    let body_range = br2.range();
                    let func_body_start = body_range.start;

                    // name map omitted for performance in inspect; avoid re-parsing the module here
                    let name_map: Option<std::collections::HashMap<u32, String>> = None;
                    let mut ops = body.get_operators_reader()?;
                    let mut indent: usize = 0;

                    while !ops.eof() {
                        let (op, abs_off) = ops.read_with_offset()?;
                        let rel_off = abs_off.saturating_sub(func_body_start);
                        // Adjust indent for Else/End before printing
                        let mut current_indent = indent;
                        match op {
                            Operator::Else => {
                                if indent > 0 {
                                    indent -= 1;
                                }
                                current_indent = indent;
                            }
                            Operator::End => {
                                if indent > 0 {
                                    indent -= 1;
                                }
                                current_indent = indent;
                            }
                            _ => {}
                        }
                        let pad = "  ".repeat(current_indent);

                        // Skip per-op source mapping for performance; source pane will use function span
                        let src = None;

                        // Emit mnemonic text without inline comments
                        let text = match op {
                            // constants
                            Operator::I32Const { value } => format!("{pad}i32.const {value}"),
                            Operator::I64Const { value } => format!("{pad}i64.const {value}"),
                            Operator::F32Const { value } => {
                                let bits = value.bits();
                                let val = f32::from_bits(bits);
                                format!("{pad}f32.const {val}  ;; bits=0x{bits:08x}")
                            }
                            Operator::F64Const { value } => {
                                let bits = value.bits();
                                let val = f64::from_bits(bits);
                                format!("{pad}f64.const {val}  ;; bits=0x{bits:016x}")
                            }

                            // locals/globals
                            Operator::LocalGet { local_index } => {
                                format!("{pad}local.get {local_index}")
                            }
                            Operator::LocalSet { local_index } => {
                                format!("{pad}local.set {local_index}")
                            }
                            Operator::LocalTee { local_index } => {
                                format!("{pad}local.tee {local_index}")
                            }
                            Operator::GlobalGet { global_index } => {
                                format!("{pad}global.get {global_index}")
                            }
                            Operator::GlobalSet { global_index } => {
                                format!("{pad}global.set {global_index}")
                            }

                            // calls
                            Operator::Call { function_index } => {
                                // Keep only the index here to avoid heavy name resolution in inspect hot path
                                format!("{pad}call {function_index}")
                            }
                            Operator::CallIndirect { .. } => {
                                format!("{pad}call_indirect")
                            }

                            // numeric ops (subset)
                            Operator::I32Add => format!("{pad}i32.add"),
                            Operator::I32Sub => format!("{pad}i32.sub"),
                            Operator::I32Mul => format!("{pad}i32.mul"),
                            Operator::I64Add => format!("{pad}i64.add"),
                            Operator::I64Sub => format!("{pad}i64.sub"),
                            Operator::I64Mul => format!("{pad}i64.mul"),
                            Operator::F32Add => format!("{pad}f32.add"),
                            Operator::F32Sub => format!("{pad}f32.sub"),
                            Operator::F32Mul => format!("{pad}f32.mul"),
                            Operator::F64Add => format!("{pad}f64.add"),
                            Operator::F64Sub => format!("{pad}f64.sub"),
                            Operator::F64Mul => format!("{pad}f64.mul"),

                            // compares (subset)
                            Operator::I32Eq => format!("{pad}i32.eq"),
                            Operator::I32Ne => format!("{pad}i32.ne"),
                            Operator::I32LtS => format!("{pad}i32.lt_s"),
                            Operator::I32LtU => format!("{pad}i32.lt_u"),
                            Operator::I32GtS => format!("{pad}i32.gt_s"),
                            Operator::I32GtU => format!("{pad}i32.gt_u"),

                            // memory (subset)
                            Operator::I32Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}i32.load offset={} align={}", memarg.offset, align)
                            }
                            Operator::I64Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}i64.load offset={} align={}", memarg.offset, align)
                            }
                            Operator::F32Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}f32.load offset={} align={}", memarg.offset, align)
                            }
                            Operator::F64Load { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}f64.load offset={} align={}", memarg.offset, align)
                            }
                            Operator::I32Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}i32.store offset={} align={}", memarg.offset, align)
                            }
                            Operator::I64Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}i64.store offset={} align={}", memarg.offset, align)
                            }
                            Operator::F32Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}f32.store offset={} align={}", memarg.offset, align)
                            }
                            Operator::F64Store { memarg } => {
                                let align = 1u64 << memarg.align;
                                format!("{pad}f64.store offset={} align={}", memarg.offset, align)
                            }

                            // control flow with indentation
                            Operator::Block { .. } => {
                                // print then increase indent
                                let t = format!("{pad}block");
                                indent = indent.saturating_add(1);
                                t
                            }
                            Operator::Loop { .. } => {
                                let t = format!("{pad}loop");
                                indent = indent.saturating_add(1);
                                t
                            }
                            Operator::If { .. } => {
                                let t = format!("{pad}if");
                                indent = indent.saturating_add(1);
                                t
                            }
                            Operator::Else => {
                                let t = format!("{pad}else");
                                indent = indent.saturating_add(1);
                                t
                            }
                            Operator::End => format!("{pad}end"),

                            // branches
                            Operator::Br { relative_depth } => {
                                format!("{pad}br {relative_depth}")
                            }
                            Operator::BrIf { relative_depth } => {
                                format!("{pad}br_if {relative_depth}")
                            }
                            Operator::BrTable { .. } => format!("{pad}br_table"),

                            // misc
                            Operator::Return => format!("{pad}return"),
                            Operator::Drop => format!("{pad}drop"),
                            Operator::Select => format!("{pad}select"),
                            Operator::Nop => format!("{pad}nop"),
                            Operator::Unreachable => format!("{pad}unreachable"),
                            Operator::MemoryGrow { .. } => format!("{pad}memory.grow"),
                            Operator::MemorySize { .. } => format!("{pad}memory.size"),

                            // default
                            other => format!("{pad};; {:?}", other),
                        };

                        lines.push(WatLine {
                            text,
                            offset: rel_off,
                            indent: current_indent,
                            src,
                        });
                    }

                    // Closing
                    lines.push(WatLine {
                        text: ")".to_string(),
                        offset: 0,
                        indent: 0,
                        src: None,
                    });

                    return Ok(lines);
                }

                defined_funcs_seen += 1;
            }
            _ => {}
        }
    }

    Err(anyhow!("function index {} not found", target_func_index))
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

    #[cfg(feature = "fixture-tests")]
    #[test]
    fn dwarf_sections_and_mapping_exist() {
        // Include the fixture wasm bytes built for wasm32-unknown-unknown (debug).
        let bytes: &[u8] = include_bytes!("../tests/fixtures/simple_wasm.wasm");

        // Assert there are DWARF sections present in the wasm so that source mapping is possible.
        // Bring the Object trait into scope for method resolution.
        use object::Object as _;
        let obj = object::File::parse(bytes).expect("parse wasm object");
        let has_dwarf = [
            ".debug_info",
            ".debug_abbrev",
            ".debug_line",
            ".debug_str",
            ".debug_ranges",
            ".debug_rnglists",
            ".debug_str_offsets",
            ".debug_addr",
            ".debug_aranges",
            ".debug_line_str",
        ]
        .iter()
        .any(|name| {
            obj.section_by_name(name)
                .map(|s| s.size() > 0)
                .unwrap_or(false)
        });

        assert!(
            has_dwarf,
            "expected at least one DWARF .debug_* section in the fixture wasm"
        );

        // Parse module and pick the first defined function; ensure we can resolve a source location
        // for offset 0 within the function body (heuristic).
        let info = parse_wasm_from_bytes(bytes).expect("fixture wasm should parse");
        let first = info
            .functions
            .first()
            .expect("fixture should have a defined function");
        let loc = map_instr_to_source(bytes, first.index, 0);

        assert!(
            loc.is_some(),
            "expected to resolve at least one source location for the first function"
        );
    }
}
