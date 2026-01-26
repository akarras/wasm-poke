//! Size Tree panel showing cumulative size impact through the call graph.
//!
//! This panel displays functions and their cumulative reachable size (what would
//! be removed if the function and all its unique callees were eliminated).
//! Features:
//! - Cumulative size calculation using unique_cumulative_size
//! - Size display: "name - X.X KiB (Y.Y%)"
//! - Color-coded background: warm (orange) with intensity based on size
//! - Same cycle detection and depth limit as CallTreePanel

use std::collections::HashSet;

use bytesize::ByteSize;
use egui::collapsing_header::CollapsingState;
use eframe::egui;

use crate::gui::state::SelectionState;
use wasm_poke::{unique_cumulative_size, CallGraph, WasmModuleInfo};

/// Maximum tree depth to prevent UI performance issues.
const MAX_DEPTH: usize = 5;

/// Panel for displaying the size tree (cumulative size impact) for the selected function.
pub struct SizeTreePanel {
    /// Current filter input text (for future use).
    filter_text: String,
    /// Whether the filter input currently has focus.
    filter_focused: bool,
}

impl SizeTreePanel {
    /// Create a new size tree panel.
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            filter_focused: false,
        }
    }

    /// Main render method for the size tree panel.
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
                ui.label("Select a function to see its size impact");
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

        // Calculate cumulative size for root
        let (root_cumulative, _) = unique_cumulative_size(root_index, module, graph);
        let root_pct = if module.total_code_size > 0 {
            root_cumulative as f64 / module.total_code_size as f64 * 100.0
        } else {
            0.0
        };
        let root_size_str = ByteSize::b(root_cumulative).to_string_as(false);

        // Header with root function name and cumulative size
        ui.horizontal(|ui| {
            ui.label("Size impact from:");
            ui.strong(format!(
                "{} - {} ({:.1}%)",
                root_func.best_name(),
                root_size_str,
                root_pct
            ));
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

    /// Calculate background color based on cumulative size relative to total.
    /// Uses logarithmic scale for better visual differentiation.
    fn size_to_background_color(cumulative: u64, total: u64) -> egui::Color32 {
        if total == 0 {
            return egui::Color32::TRANSPARENT;
        }
        let ratio = (cumulative as f64 / total as f64).min(1.0);
        // Logarithmic scale for better visual differentiation
        // ln(1 + x) ranges from 0 to ln(2) for x in [0, 1]
        let intensity = if ratio > 0.0 {
            (ratio.ln_1p() / 1.0_f64.ln_1p()).max(0.0).min(1.0)
        } else {
            0.0
        };

        // Warm color (orange-ish) for size visualization
        // Alpha from 0.05 (faint) to 0.4 (strong)
        let alpha = (0.05 + 0.35 * intensity as f32) * 255.0;
        egui::Color32::from_rgba_unmultiplied(255, 150, 50, alpha as u8)
    }

    /// Recursively render a tree node and its children with size information.
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
        let name = if let Some(f) = func {
            f.best_name()
        } else {
            format!("func[{}]", func_index)
        };

        // Calculate cumulative size for this function
        let (cumulative_size, _) = unique_cumulative_size(func_index, module, graph);
        let pct = if module.total_code_size > 0 {
            cumulative_size as f64 / module.total_code_size as f64 * 100.0
        } else {
            0.0
        };
        let size_str = ByteSize::b(cumulative_size).to_string_as(false);

        // Check for recursion (cycle detection)
        let is_recursive = visited.contains(&func_index);

        // Get callees for this function
        let callees = graph.edges.get(&func_index);
        let has_children = callees.map(|c| !c.is_empty()).unwrap_or(false);

        // Check if we've hit the depth limit
        let at_depth_limit = depth >= MAX_DEPTH;

        // Generate unique ID for this node based on path
        let id = ui.make_persistent_id(("size_tree_node", path.clone()));

        // Get background color based on size
        let bg_color = Self::size_to_background_color(cumulative_size, module.total_code_size);

        if is_recursive {
            // Show recursive marker instead of expanding
            ui.horizontal(|ui| {
                // Draw background
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{} - {} ({:.1}%)", name, size_str, pct))
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
                // Draw background
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(format!("{} - {} ({:.1}%)", name, size_str, pct))
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
                // Draw background for the header row
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                let response = ui.add(
                    egui::Label::new(format!("{} - {} ({:.1}%)", name, size_str, pct))
                        .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
            header_response.body(|ui: &mut egui::Ui| {
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
                // Draw background
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(format!("{} - {} ({:.1}%)", name, size_str, pct))
                        .sense(egui::Sense::click()),
                );
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
        }
    }
}

impl Default for SizeTreePanel {
    fn default() -> Self {
        Self::new()
    }
}
