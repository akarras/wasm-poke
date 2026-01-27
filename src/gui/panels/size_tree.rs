//! Size Tree panel showing cumulative size impact through the call graph.
//!
//! This panel displays functions and their cumulative reachable size (what would
//! be removed if the function and all its unique callees were eliminated).
//! Features:
//! - Cumulative size calculation using unique_cumulative_size
//! - Size display: "name - X.X KiB (Y.Y%)"
//! - Color-coded background: warm (orange) with intensity based on size
//! - Same cycle detection and depth limit as CallTreePanel
//! - Keyboard navigation (j/k, arrows, Enter/Space, g/G)
//! - Filter search with match highlighting

use std::collections::HashSet;

use bytesize::ByteSize;
use egui::collapsing_header::CollapsingState;
use eframe::egui::{self, Key};

use crate::gui::state::SelectionState;
use wasm_poke::{function_matches, unique_cumulative_size, CallGraph, FunctionInfo, WasmModuleInfo};

/// Maximum tree depth to prevent UI performance issues.
const MAX_DEPTH: usize = 5;

/// Panel for displaying the size tree (cumulative size impact) for the selected function.
pub struct SizeTreePanel {
    /// Current filter input text.
    filter_text: String,
    /// Whether the filter input currently has focus.
    filter_focused: bool,
    /// Currently focused node path for keyboard navigation.
    focus_path: Option<Vec<(u32, usize)>>,
}

impl SizeTreePanel {
    /// Create a new size tree panel.
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            filter_focused: false,
            focus_path: None,
        }
    }

    /// Check if a subtree contains any function matching the filter.
    fn subtree_contains_match(
        func_index: u32,
        filter: &str,
        module: &WasmModuleInfo,
        graph: &CallGraph,
        visited: &mut HashSet<u32>,
        depth: usize,
    ) -> bool {
        if depth >= MAX_DEPTH || visited.contains(&func_index) {
            return false;
        }

        // Check current node
        if let Some(func) = module.functions.iter().find(|f| f.index == func_index) {
            if function_matches(func, filter) {
                return true;
            }
        }

        // Check children
        visited.insert(func_index);
        let has_match = if let Some(children) = graph.edges.get(&func_index) {
            children.iter().any(|&child| {
                Self::subtree_contains_match(child, filter, module, graph, visited, depth + 1)
            })
        } else {
            false
        };
        visited.remove(&func_index);
        has_match
    }

    /// Check if a function matches the filter.
    fn matches_filter(func: &FunctionInfo, filter: &str) -> bool {
        if filter.is_empty() {
            return true;
        }
        function_matches(func, filter)
    }

    /// Handle keyboard input for tree navigation.
    fn handle_keyboard(
        &mut self,
        ctx: &egui::Context,
        selection: &mut SelectionState,
        visible_nodes: &[Vec<(u32, usize)>],
    ) {
        if self.filter_focused || visible_nodes.is_empty() {
            return;
        }

        // Find current focus position in visible_nodes
        let current_pos = self
            .focus_path
            .as_ref()
            .and_then(|fp| visible_nodes.iter().position(|p| p == fp))
            .unwrap_or(0);

        ctx.input(|i| {
            // j / ArrowDown: move focus down
            if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
                let new_pos = (current_pos + 1).min(visible_nodes.len().saturating_sub(1));
                if new_pos < visible_nodes.len() {
                    self.focus_path = Some(visible_nodes[new_pos].clone());
                }
            }

            // k / ArrowUp: move focus up
            if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
                let new_pos = current_pos.saturating_sub(1);
                if new_pos < visible_nodes.len() {
                    self.focus_path = Some(visible_nodes[new_pos].clone());
                }
            }

            // Enter / Space: select focused node
            if i.key_pressed(Key::Enter) || i.key_pressed(Key::Space) {
                if let Some(ref path) = self.focus_path {
                    if let Some(&(func_index, _)) = path.last() {
                        selection.select_single(func_index);
                    }
                }
            }

            // ArrowRight: expand focused node
            if i.key_pressed(Key::ArrowRight) {
                if let Some(ref path) = self.focus_path {
                    selection.expanded_nodes.insert(path.clone());
                }
            }

            // ArrowLeft: collapse focused node OR move to parent
            if i.key_pressed(Key::ArrowLeft) {
                if let Some(ref path) = self.focus_path {
                    if selection.expanded_nodes.contains(path) {
                        // Node is expanded: collapse it
                        selection.expanded_nodes.remove(path);
                    } else if path.len() > 1 {
                        // Node is collapsed or leaf: move focus to parent
                        let mut parent_path = path.clone();
                        parent_path.pop();
                        self.focus_path = Some(parent_path);
                    }
                }
            }

            // g: jump to top (first visible node)
            if i.key_pressed(Key::G) && !i.modifiers.shift {
                if !visible_nodes.is_empty() {
                    self.focus_path = Some(visible_nodes[0].clone());
                }
            }

            // G (Shift+g): jump to bottom (last visible node)
            if i.key_pressed(Key::G) && i.modifiers.shift {
                if !visible_nodes.is_empty() {
                    self.focus_path = Some(visible_nodes[visible_nodes.len() - 1].clone());
                }
            }
        });
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
        // Filter input at top
        ui.horizontal(|ui| {
            ui.label("Filter:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.filter_text)
                    .hint_text("Type to filter...")
                    .desired_width(200.0),
            );
            self.filter_focused = response.has_focus();
        });

        ui.separator();

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

        // Collect visible nodes for keyboard navigation
        let mut visible_nodes: Vec<Vec<(u32, usize)>> = Vec::new();

        // Scroll area for the tree
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Initialize visited set for cycle detection
                let mut visited = HashSet::new();

                // Render the tree starting from root
                self.render_tree_node(
                    ctx,
                    ui,
                    root_index,
                    0,
                    vec![],
                    &mut visited,
                    module,
                    graph,
                    selection,
                    &mut visible_nodes,
                );
            });

        // Handle keyboard navigation after rendering
        self.handle_keyboard(ctx, selection, &visible_nodes);
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
        &self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        func_index: u32,
        depth: usize,
        path: Vec<(u32, usize)>,
        visited: &mut HashSet<u32>,
        module: &WasmModuleInfo,
        graph: &CallGraph,
        selection: &mut SelectionState,
        visible_nodes: &mut Vec<Vec<(u32, usize)>>,
    ) {
        // Get function info
        let func = module.functions.iter().find(|f| f.index == func_index);
        let name = if let Some(f) = func {
            f.best_name()
        } else {
            format!("func[{}]", func_index)
        };

        // Check filter - show node if matches or subtree contains match
        let filter = &self.filter_text;
        if !filter.is_empty() {
            let node_matches = func.map(|f| Self::matches_filter(f, filter)).unwrap_or(false);
            let mut filter_visited = HashSet::new();
            let subtree_matches =
                Self::subtree_contains_match(func_index, filter, module, graph, &mut filter_visited, depth);
            if !node_matches && !subtree_matches {
                return; // Skip this node and its subtree
            }
        }

        // Check if this node matches filter (for highlighting)
        let matches_filter = !filter.is_empty()
            && func.map(|f| Self::matches_filter(f, filter)).unwrap_or(false);

        // Calculate cumulative size for this function
        let (cumulative_size, _) = unique_cumulative_size(func_index, module, graph);
        let pct = if module.total_code_size > 0 {
            cumulative_size as f64 / module.total_code_size as f64 * 100.0
        } else {
            0.0
        };
        let size_str = ByteSize::b(cumulative_size).to_string_as(false);

        // Build node path
        let node_path = if depth == 0 {
            vec![(func_index, 0)]
        } else {
            path.clone()
        };

        // Check if this node is focused
        let is_focused = self.focus_path.as_ref() == Some(&node_path);

        // Check for recursion (cycle detection)
        let is_recursive = visited.contains(&func_index);

        // Get callees for this function
        let callees = graph.edges.get(&func_index);
        let has_children = callees.map(|c| !c.is_empty()).unwrap_or(false);

        // Check if we've hit the depth limit
        let at_depth_limit = depth >= MAX_DEPTH;

        // Generate unique ID for this node based on path
        let id = ui.make_persistent_id(("size_tree_node", node_path.clone()));

        // Get background color based on size
        let bg_color = Self::size_to_background_color(cumulative_size, module.total_code_size);

        // Format label text - bold if matches filter
        let label_text = format!("{} - {} ({:.1}%)", name, size_str, pct);
        let label_rich = if matches_filter {
            egui::RichText::new(&label_text).strong().color(egui::Color32::YELLOW)
        } else {
            egui::RichText::new(&label_text)
        };

        // Add to visible nodes list
        visible_nodes.push(node_path.clone());

        if is_recursive {
            // Show recursive marker instead of expanding
            ui.horizontal(|ui| {
                // Draw background
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                // Focus indicator
                if is_focused {
                    ui.painter()
                        .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE), egui::StrokeKind::Outside);
                }

                ui.add_space(16.0 * depth as f32);
                let response = ui.add(
                    egui::Label::new(label_rich.clone().color(egui::Color32::GRAY))
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

                // Focus indicator
                if is_focused {
                    ui.painter()
                        .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE), egui::StrokeKind::Outside);
                }

                ui.add_space(16.0 * depth as f32);
                let response = ui.add(egui::Label::new(label_rich).sense(egui::Sense::click()));
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
            // Check if expanded via keyboard (in expanded_nodes)
            let keyboard_expanded = selection.expanded_nodes.contains(&node_path);

            // Expandable node with children - default open at depth 0
            let default_open = depth == 0 || keyboard_expanded;
            let state = CollapsingState::load_with_default_open(ctx, id, default_open);

            // Mark as visited before processing children
            visited.insert(func_index);

            let is_open = state.is_open();
            let header_response = state.show_header(ui, |ui: &mut egui::Ui| {
                // Draw background for the header row
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                // Focus indicator
                if is_focused {
                    ui.painter()
                        .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE), egui::StrokeKind::Outside);
                }

                let response = ui.add(egui::Label::new(label_rich).sense(egui::Sense::click()));
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
            header_response.body(|ui: &mut egui::Ui| {
                if let Some(callees) = callees {
                    for (child_pos, &callee_index) in callees.iter().enumerate() {
                        let mut child_path = node_path.clone();
                        child_path.push((callee_index, child_pos));

                        self.render_tree_node(
                            ctx,
                            ui,
                            callee_index,
                            depth + 1,
                            child_path,
                            visited,
                            module,
                            graph,
                            selection,
                            visible_nodes,
                        );
                    }
                }
            });

            // Sync expanded state to selection
            if is_open && !keyboard_expanded {
                selection.expanded_nodes.insert(node_path.clone());
            } else if !is_open && keyboard_expanded {
                selection.expanded_nodes.remove(&node_path);
            }

            // Backtrack: remove from visited after processing children
            visited.remove(&func_index);
        } else {
            // Leaf node (no children)
            ui.horizontal(|ui| {
                // Draw background
                let rect = ui.available_rect_before_wrap();
                ui.painter().rect_filled(rect, 0.0, bg_color);

                // Focus indicator
                if is_focused {
                    ui.painter()
                        .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE), egui::StrokeKind::Outside);
                }

                ui.add_space(16.0 * depth as f32);
                let response = ui.add(egui::Label::new(label_rich).sense(egui::Sense::click()));
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
