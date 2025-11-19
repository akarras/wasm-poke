use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;

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

/// Call graph of direct calls between functions identified by global indices.
/// - `edges[src] = Vec<dst>` where each entry is a direct `call` target
/// - `has_indirect[src] = true` if the function contains any `call_indirect`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CallGraph {
    pub edges: HashMap<u32, Vec<u32>>,
    pub has_indirect: HashMap<u32, bool>,
}
