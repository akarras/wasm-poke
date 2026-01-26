//! Tab types for the docking layout.
//!
//! Defines the different panel types that can be displayed in the dock area.

/// Tab types for the docking layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKind {
    /// Function list panel - shows all functions sorted by size.
    FunctionList,
    /// Call tree/graph panel - shows call relationships.
    CallTree,
    /// Size tree panel - shows cumulative size breakdown.
    SizeTree,
    /// Inspector panel - shows WAT disassembly and source mapping.
    Inspector,
}

impl TabKind {
    /// Returns the display title for this tab type.
    pub fn title(&self) -> &'static str {
        match self {
            TabKind::FunctionList => "Functions",
            TabKind::CallTree => "Call Graph",
            TabKind::SizeTree => "Size Tree",
            TabKind::Inspector => "Inspector",
        }
    }
}
