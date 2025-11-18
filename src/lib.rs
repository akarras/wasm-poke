use std::collections::HashMap;
use std::ops::Range;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use anyhow::{anyhow, Context, Result};
use globset::GlobBuilder;
use object::{Object, ObjectSection};
use rustc_demangle::try_demangle;
use serde::{Deserialize, Serialize};
use wasmparser::{ExternalKind, Name, NameSectionReader, Operator, Parser, Payload, TypeRef};

// DWARF/source mapping deps
use addr2line::Context as Addr2LineContext;
use gimli::{self, EndianSlice, LittleEndian};

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

/// Filter functions using simple wildcard matching:
/// - `*` matches any sequence of characters (including empty)
/// - All other characters match literally
///
/// If the pattern has no '*', interpret it as substring by wrapping as *pattern*
/// Matching is case-insensitive.
pub fn filter_functions<'a>(funcs: &'a [FunctionInfo], pattern: &str) -> Vec<&'a FunctionInfo> {
    // Default to substring "contains" when no '*' is present by wrapping as *pattern*
    let normalized;
    let pat = if pattern.contains('*') {
        pattern.to_string()
    } else {
        normalized = format!("*{}*", pattern);
        normalized
    };

    // Compile matcher once per call and perform reference-based checks to avoid per-item allocations
    let glob = match GlobBuilder::new(&pat)
        .case_insensitive(true)
        .backslash_escape(false)
        .build()
    {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    let matcher = glob.compile_matcher();

    funcs
        .iter()
        .filter(|f| {
            if let Some(ref d) = f.demangled_name {
                if matcher.is_match(d) {
                    return true;
                }
            }
            if let Some(ref r) = f.raw_name {
                if matcher.is_match(r) {
                    return true;
                }
            }
            for e in &f.export_names {
                if matcher.is_match(e) {
                    return true;
                }
            }
            // fallback to func[index] only when no names available
            if f.demangled_name.is_none() && f.raw_name.is_none() && f.export_names.is_empty() {
                let tmp = format!("func[{}]", f.index);
                if matcher.is_match(&tmp) {
                    return true;
                }
            }
            false
        })
        .collect()
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
/// Matching is case-insensitive. If the pattern has no '*', we wrap as *pattern* (substring).
pub fn function_matches(func: &FunctionInfo, pattern: &str) -> bool {
    // keep existing behavior: if no '*' is present, wrap as *pattern*
    let normalized = if pattern.contains('*') {
        pattern.to_string()
    } else {
        format!("*{}*", pattern)
    };

    // build a case-insensitive glob matcher (compile once)
    let glob = match GlobBuilder::new(&normalized)
        .case_insensitive(true)
        .backslash_escape(false)
        .build()
    {
        Ok(g) => g,
        Err(_) => return false,
    };

    let matcher = glob.compile_matcher();

    // Avoid allocations: check borrowed names first, then fallback "func[index]" string
    if let Some(ref d) = func.demangled_name {
        if matcher.is_match(d) {
            return true;
        }
    }
    if let Some(ref r) = func.raw_name {
        if matcher.is_match(r) {
            return true;
        }
    }
    for e in &func.export_names {
        if matcher.is_match(e) {
            return true;
        }
    }

    // Only if no name matched, consider fallback name
    if func.demangled_name.is_none() && func.raw_name.is_none() && func.export_names.is_empty() {
        let tmp = format!("func[{}]", func.index);
        if matcher.is_match(&tmp) {
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

// ============== Inspect utilities: DWARF/source mapping ==============

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

// Cached DWARF/addr2line context to avoid reparsing per mapping call and to normalize addresses
// relative to the first function body. Wrapped in a Mutex to satisfy Sync bounds for global statics.
pub struct DwarfCtx {
    ctx: Addr2LineContext<EndianSlice<'static, LittleEndian>>,
    first_body_start: usize,
}
pub static DWARF_CONTEXT: OnceLock<Mutex<DwarfCtx>> = OnceLock::new();

/// Initialize and cache a global DWARF context and the first function body start offset.
/// Returns a reference to the cached context on success.
pub fn init_dwarf_context(wasm_bytes: &[u8]) -> Option<&'static Mutex<DwarfCtx>> {
    if let Some(existing) = DWARF_CONTEXT.get() {
        return Some(existing);
    }

    // Parse sections once and leak them to obtain 'static lifetimes for EndianSlice.
    let obj = object::File::parse(wasm_bytes).ok()?;
    let mut sec_map: HashMap<&'static str, &'static [u8]> = HashMap::new();
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

    let endian = LittleEndian;
    let dwarf = gimli::Dwarf::load(|id| {
        let name = id.name();
        let data = sec_map.get(name).copied().unwrap_or(&[]);
        Ok::<EndianSlice<'static, LittleEndian>, gimli::Error>(EndianSlice::new(data, endian))
    })
    .ok()?;

    let ctx = Addr2LineContext::from_dwarf(dwarf).ok()?;

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

/// Attempts to map a global function index and an operator offset within its body to a source location.
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

/// Compute the best-effort source span (file and start/end lines) for an entire function by
/// probing DWARF mappings across the function body. Returns None if no mapping is found.
/// Picks the dominant file by count and min/max line range.
pub fn function_source_span(wasm_bytes: &[u8], func_index: u32) -> Option<SourceSpan> {
    use std::collections::HashMap as Map;

    // Use global DWARF context (Mutex-backed) for all spans.
    let dc = if let Some(m) = DWARF_CONTEXT.get() {
        m.lock().ok()?
    } else {
        init_dwarf_context(wasm_bytes)?.lock().ok()?
    };

    // Locate function body start/end
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
                let file = loc
                    .file
                    .as_ref()
                    .map(|f| f.to_string())
                    .unwrap_or_else(|| "<unknown>".to_string());
                let entry = file_stats.entry(file).or_insert((
                    0,
                    loc.line.unwrap_or(0),
                    loc.column.unwrap_or(0),
                    0,
                    0,
                ));
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

/// Return the raw function body bytes for a given global function index, if available.
/// Uses `WasmModuleInfo.body_range` to slice into the original wasm bytes.
/// Returns None if the function is not found or has no recorded body range (e.g., import).
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

/// Disassemble a single function into a readable, WAT-like representation (string).
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

                    // Locals (list counts/types)
                    let mut locals_reader = body.get_locals_reader()?;
                    if locals_reader.get_count() > 0 {
                        out.push_str("  ;; locals:\n");
                        for _ in 0..locals_reader.get_count() {
                            let (cnt, ty) = locals_reader.read()?;
                            out.push_str(&format!("  ;;   count={} type={:?}\n", cnt, ty));
                        }
                    }

                    // Operators
                    out.push_str("  ;; body\n");
                    let br2 = body.get_binary_reader();
                    let body_range = br2.range();
                    let func_body_start = body_range.start;
                    let mut ops = body.get_operators_reader()?;
                    let mut indent: usize = 0;
                    while !ops.eof() {
                        let (op, off) = ops.read_with_offset()?;
                        let rel_off = off.saturating_sub(func_body_start);

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

                        let line = match op {
                            Operator::I32Const { value } => format!("{pad}i32.const {value}"),
                            Operator::I64Const { value } => format!("{pad}i64.const {value}"),
                            Operator::F32Const { value } => {
                                let bits = value.bits();
                                let val = f32::from_bits(bits);
                                format!("{pad}f32.const {}  ;; bits=0x{:08x}", val, bits)
                            }
                            Operator::F64Const { value } => {
                                let bits = value.bits();
                                let val = f64::from_bits(bits);
                                format!("{pad}f64.const {}  ;; bits=0x{:016x}", val, bits)
                            }
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
                            Operator::Call { function_index } => {
                                format!("{pad}call {function_index}")
                            }
                            Operator::CallIndirect { .. } => format!("{pad}call_indirect"),
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
                            Operator::Block { .. } => {
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
                            Operator::Br { relative_depth } => {
                                format!("{pad}br {relative_depth}")
                            }
                            Operator::BrIf { relative_depth } => {
                                format!("{pad}br_if {relative_depth}")
                            }
                            Operator::BrTable { .. } => format!("{pad}br_table"),
                            Operator::Return => format!("{pad}return"),
                            Operator::Drop => format!("{pad}drop"),
                            Operator::Select => format!("{pad}select"),
                            Operator::Nop => format!("{pad}nop"),
                            Operator::Unreachable => format!("{pad}unreachable"),
                            Operator::MemoryGrow { .. } => format!("{pad}memory.grow"),
                            Operator::MemorySize { .. } => format!("{pad}memory.size"),
                            other => format!("{pad};; {:?}", other),
                        };

                        out.push_str(&line);
                        out.push('\n');
                        let _ = rel_off; // currently unused in string version
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

                    // Operators: WAT-like mnemonics with indentation, carrying offsets (src mapping omitted by default)
                    let br2 = body.get_binary_reader();
                    let body_range = br2.range();
                    let func_body_start = body_range.start;

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

                        let text = match op {
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

                            // control flow with indentation
                            Operator::Block { .. } => {
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
                            src: None,
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

/// Convenience: map a structured WAT line (carrying body-relative offset) to a SourceLocation.
pub fn map_wat_line_to_source_cached(
    module: &WasmModuleInfo,
    wasm_bytes: &[u8],
    func_index: u32,
    wl: &WatLine,
) -> Option<SourceLocation> {
    map_instr_to_source_fast(module, wasm_bytes, func_index, wl.offset)
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
