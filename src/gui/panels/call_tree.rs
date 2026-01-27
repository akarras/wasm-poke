//! Call Tree panel showing downstream function calls.
//!
//! This panel displays functions called by the selected function (and what those call,
//! recursively) in an expandable tree structure. Features:
//! - Expand/collapse tree nodes via click
//! - Cycle detection with "(recursive)" marker
//! - Depth limit of 5 levels
//! - Selection sync with function list
//! - Keyboard navigation (j/k, arrows, Enter/Space, g/G)
//! - Filter search with match highlighting

use std::collections::HashSet;

use bytesize::ByteSize;
use egui::collapsing_header::CollapsingState;
use eframe::egui::{self, Key};

use crate::gui::state::SelectionState;
use crate::gui::tabs::TabKind;
use wasm_poke::{function_matches, CallGraph, FunctionInfo, WasmModuleInfo};

/// Maximum tree depth to prevent UI performance issues.
const MAX_DEPTH: usize = 5;

/// Panel for displaying the call tree (downstream calls) for the selected function.
pub struct CallTreePanel {
    /// Current filter input text.
    filter_text: String,
    /// Whether the filter input currently has focus.
    filter_focused: bool,
    /// Currently focused node path for keyboard navigation.
    focus_path: Option<Vec<(u32, usize)>>,
}

impl CallTreePanel {
    /// Create a new call tree panel.
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

    /// Main render method for the call tree panel.
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

        // Handle keyboard navigation after rendering (only if this tab is active)
        if selection.active_tab == TabKind::CallTree {
            self.handle_keyboard(ctx, selection, &visible_nodes);
        }
    }

    /// Recursively render a tree node and its children.
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
        let (name, size_str) = if let Some(f) = func {
            let size = ByteSize::b(f.code_size as u64).to_string_as(false);
            (f.best_name(), size)
        } else {
            // This might be an imported function (no body)
            (format!("func[{}]", func_index), "imported".to_string())
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

        // Build node path for this node
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
        let id = ui.make_persistent_id(("call_tree_node", node_path.clone()));

        // Format label text - bold if matches filter
        let label_text = format!("{} ({})", name, size_str);
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
                // Focus indicator
                if is_focused {
                    let rect = ui.available_rect_before_wrap();
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
                // Focus indicator
                if is_focused {
                    let rect = ui.available_rect_before_wrap();
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
            let header_response = state.show_header(ui, |ui| {
                // Focus indicator
                if is_focused {
                    let rect = ui.available_rect_before_wrap();
                    ui.painter()
                        .rect_stroke(rect, 0.0, egui::Stroke::new(2.0, egui::Color32::LIGHT_BLUE), egui::StrokeKind::Outside);
                }
                let response = ui.add(egui::Label::new(label_rich).sense(egui::Sense::click()));
                if response.clicked() {
                    selection.select_single(func_index);
                }
            });
            header_response.body(|ui| {
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
                // Focus indicator
                if is_focused {
                    let rect = ui.available_rect_before_wrap();
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

impl Default for CallTreePanel {
    fn default() -> Self {
        Self::new()
    }
}
