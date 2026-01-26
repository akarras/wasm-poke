//! Call Tree panel showing downstream function calls.
//!
//! This panel displays functions called by the selected function (and what those call,
//! recursively) in an expandable tree structure. Features:
//! - Expand/collapse tree nodes via click
//! - Cycle detection with "(recursive)" marker
//! - Depth limit of 5 levels
//! - Selection sync with function list

use std::collections::HashSet;

use bytesize::ByteSize;
use egui::collapsing_header::CollapsingState;
use eframe::egui;

use crate::gui::state::SelectionState;
use wasm_poke::{CallGraph, WasmModuleInfo};

/// Maximum tree depth to prevent UI performance issues.
const MAX_DEPTH: usize = 5;

/// Panel for displaying the call tree (downstream calls) for the selected function.
pub struct CallTreePanel {
    /// Current filter input text (for future use).
    filter_text: String,
    /// Whether the filter input currently has focus.
    filter_focused: bool,
}

impl CallTreePanel {
    /// Create a new call tree panel.
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            filter_focused: false,
        }
    }

    /// Main render method for the call tree panel.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        module: &WasmModuleInfo,
        call_graph: Option<&CallGraph>,
        selection: &mut SelectionState,
    ) {
        // Check if we have a function selected
        let Some(root_index) = selection.primary_selection() else {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label("Select a function to see its call tree");
            });
            return;
        };

        // Check if we have a call graph
        let Some(graph) = call_graph else {
            ui.label("No call graph available");
            return;
        };

        // Get the root function info
        let Some(root_func) = module.functions.iter().find(|f| f.index == root_index) else {
            ui.label(format!("Function {} not found in module", root_index));
            return;
        };

        // Header with root function name
        ui.horizontal(|ui| {
            ui.label("Calls from:");
            ui.strong(root_func.best_name());
        });

        ui.separator();

        // Scroll area for the tree
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Initialize visited set for cycle detection
                let mut visited = HashSet::new();

                // Render the tree starting from root
                Self::render_tree_node(
                    ctx,
                    ui,
                    root_index,
                    0,
                    vec![],
                    &mut visited,
                    module,
                    graph,
                    selection,
                );
            });
    }

    /// Recursively render a tree node and its children.
    fn render_tree_node(
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        func_index: u32,
        depth: usize,
        path: Vec<(u32, usize)>,
        visited: &mut HashSet<u32>,
        module: &WasmModuleInfo,
        graph: &CallGraph,
        selection: &mut SelectionState,
    ) {
        // Get function info
        let func = module.functions.iter().find(|f| f.index == func_index);
        let (name, size_str) = if let Some(f) = func {
            let size = ByteSize::b(f.code_size as u64).to_string_as(false);
            (f.best_name(), size)
        } else {
            // This might be an imported function (no body)
            (format!("func[{}]", func_index), "imported".to_string())
        };

        // Check for recursion (cycle detection)
        let is_recursive = visited.contains(&func_index);

        // Get callees for this function
        let callees = graph.edges.get(&func_index);
        let has_children = callees.map(|c| !c.is_empty()).unwrap_or(false);

        // Check if we've hit the depth limit
        let at_depth_limit = depth >= MAX_DEPTH;

        // Generate unique ID for this node based on path
        let id = ui.make_persistent_id(("call_tree_node", path.clone()));

        if is_recursive {
            // Show recursive marker instead of expanding
            ui.horizontal(|ui| {
                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{} ({})", name, size_str))
                            .color(egui::Color32::GRAY),
                    )
                    .sense(egui::Sense::click()),
                );
                ui.label(
                    egui::RichText::new("(recursive)")
                        .color(egui::Color32::YELLOW)
                        .italics(),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
        } else if at_depth_limit && has_children {
            // Show depth limit marker
            ui.horizontal(|ui| {
                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(format!("{} ({})", name, size_str))
                        .sense(egui::Sense::click()),
                );
                ui.label(
                    egui::RichText::new("...")
                        .color(egui::Color32::GRAY)
                        .italics(),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
        } else if has_children {
            // Expandable node with children
            let state = CollapsingState::load_with_default_open(ctx, id, depth == 0);

            // Mark as visited before processing children
            visited.insert(func_index);

            let header_response = state.show_header(ui, |ui| {
                let response = ui.add(
                    egui::Label::new(format!("{} ({})", name, size_str))
                        .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
            header_response.body(|ui| {
                    if let Some(callees) = callees {
                        for (child_pos, &callee_index) in callees.iter().enumerate() {
                            let mut child_path = path.clone();
                            child_path.push((func_index, child_pos));

                            Self::render_tree_node(
                                ctx,
                                ui,
                                callee_index,
                                depth + 1,
                                child_path,
                                visited,
                                module,
                                graph,
                                selection,
                            );
                        }
                    }
                });

            // Backtrack: remove from visited after processing children
            visited.remove(&func_index);
        } else {
            // Leaf node (no children)
            ui.horizontal(|ui| {
                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(format!("{} ({})", name, size_str))
                        .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
        }
    }
}

impl Default for CallTreePanel {
    fn default() -> Self {
        Self::new()
    }
}
