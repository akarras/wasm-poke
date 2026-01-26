//! Callers Tree panel showing upstream function calls.
//!
//! This panel displays functions that CALL the selected function (and what calls those,
//! recursively) in an expandable tree structure. Features:
//! - Expand/collapse tree nodes via click
//! - Cycle detection with "(recursive)" marker
//! - Depth limit of 5 levels
//! - Selection sync with function list

use std::collections::{HashMap, HashSet};

use bytesize::ByteSize;
use eframe::egui;
use egui::collapsing_header::CollapsingState;

use crate::gui::state::SelectionState;
use wasm_poke::WasmModuleInfo;

/// Maximum tree depth to prevent UI performance issues.
const MAX_DEPTH: usize = 5;

/// Panel for displaying the callers tree (upstream calls) for the selected function.
pub struct CallersTreePanel {
    /// Current filter input text (for future use).
    filter_text: String,
    /// Whether the filter input currently has focus.
    filter_focused: bool,
}

impl CallersTreePanel {
    /// Create a new callers tree panel.
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            filter_focused: false,
        }
    }

    /// Main render method for the callers tree panel.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        module: &WasmModuleInfo,
        reverse_graph: Option<&HashMap<u32, Vec<u32>>>,
        selection: &mut SelectionState,
    ) {
        // Check if we have a function selected
        let Some(root_index) = selection.primary_selection() else {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label("Select a function to see its callers");
            });
            return;
        };

        // Check if we have a reverse graph
        let Some(graph) = reverse_graph else {
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
            ui.label("Callers of:");
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

    /// Recursively render a tree node and its children (callers).
    fn render_tree_node(
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        func_index: u32,
        depth: usize,
        path: Vec<(u32, usize)>,
        visited: &mut HashSet<u32>,
        module: &WasmModuleInfo,
        reverse_graph: &HashMap<u32, Vec<u32>>,
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

        // Get callers for this function (who calls me)
        let callers = reverse_graph.get(&func_index);
        let has_children = callers.map(|c| !c.is_empty()).unwrap_or(false);

        // Check if we've hit the depth limit
        let at_depth_limit = depth >= MAX_DEPTH;

        // Generate unique ID for this node based on path
        let id = ui.make_persistent_id(("callers_tree_node", path.clone()));

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

            let header_response = state.show_header(ui, |ui: &mut egui::Ui| {
                let response = ui.add(
                    egui::Label::new(format!("{} ({})", name, size_str))
                        .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
            header_response.body(|ui: &mut egui::Ui| {
                if let Some(callers) = callers {
                    for (child_pos, &caller_index) in callers.iter().enumerate() {
                        let mut child_path = path.clone();
                        child_path.push((func_index, child_pos));

                        Self::render_tree_node(
                            ctx,
                            ui,
                            caller_index,
                            depth + 1,
                            child_path,
                            visited,
                            module,
                            reverse_graph,
                            selection,
                        );
                    }
                }
            });

            // Backtrack: remove from visited after processing children
            visited.remove(&func_index);
        } else {
            // Leaf node (no callers - this is an entry point or uncalled function)
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

impl Default for CallersTreePanel {
    fn default() -> Self {
        Self::new()
    }
}
