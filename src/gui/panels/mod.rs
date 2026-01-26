//! Panel implementations for the docking layout.

pub mod call_tree;
pub mod callers_tree;
pub mod function_list;
pub mod size_tree;

pub use call_tree::CallTreePanel;
pub use callers_tree::CallersTreePanel;
pub use function_list::FunctionListPanel;
pub use size_tree::SizeTreePanel;
