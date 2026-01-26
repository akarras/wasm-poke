//! Function list panel with virtualized table rendering.
//!
//! This panel displays all functions in a scrollable table with sortable columns
//! (Name, Size, Calls) and a filter input for live filtering.
//!
//! Supports vim-style keyboard navigation (j/k/g/G) and multi-select click interactions
//! (Ctrl+click, Shift+click).

use bytesize::ByteSize;
use eframe::egui::{self, Key};
use egui_extras::{Column, TableBuilder};

use crate::gui::state::SelectionState;
use wasm_poke::{function_matches, CallGraph, WasmModuleInfo};

/// Row height for the virtualized table.
const ROW_HEIGHT: f32 = 20.0;

/// Which column to sort by.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SortColumn {
    Name,
    #[default]
    Size,
    Calls,
}

/// Panel for displaying the function list with filtering and sorting.
pub struct FunctionListPanel {
    /// Current filter input text.
    filter_text: String,
    /// Which column is being sorted.
    sort_column: SortColumn,
    /// Sort direction (true = ascending, false = descending).
    sort_ascending: bool,
    /// Cached indices into module.functions after filter/sort.
    cached_indices: Vec<usize>,
    /// Whether the cache needs rebuilding.
    cache_dirty: bool,
    /// Total function count (for "N of M" display).
    total_function_count: usize,
    /// Whether selection just changed (triggers scroll_to_row).
    selection_changed: bool,
    /// Whether the filter input currently has focus.
    filter_focused: bool,
}

impl FunctionListPanel {
    /// Create a new function list panel with default settings.
    pub fn new() -> Self {
        Self {
            filter_text: String::new(),
            sort_column: SortColumn::Size,
            sort_ascending: false, // Default: descending by size
            cached_indices: Vec::new(),
            cache_dirty: true,
            total_function_count: 0,
            selection_changed: false,
            filter_focused: false,
        }
    }

    /// Rebuild the cached indices based on current filter and sort settings.
    fn rebuild_cache(&mut self, module: &WasmModuleInfo, call_graph: Option<&CallGraph>) {
        self.total_function_count = module.functions.len();

        // Filter functions
        let filtered: Vec<usize> = module
            .functions
            .iter()
            .enumerate()
            .filter(|(_, func)| function_matches(func, &self.filter_text))
            .map(|(i, _)| i)
            .collect();

        // Sort the filtered indices
        let mut sorted = filtered;
        match self.sort_column {
            SortColumn::Name => {
                sorted.sort_by(|&a, &b| {
                    let name_a = module.functions[a].best_name().to_lowercase();
                    let name_b = module.functions[b].best_name().to_lowercase();
                    if self.sort_ascending {
                        name_a.cmp(&name_b)
                    } else {
                        name_b.cmp(&name_a)
                    }
                });
            }
            SortColumn::Size => {
                sorted.sort_by(|&a, &b| {
                    let size_a = module.functions[a].code_size;
                    let size_b = module.functions[b].code_size;
                    if self.sort_ascending {
                        size_a.cmp(&size_b)
                    } else {
                        size_b.cmp(&size_a)
                    }
                });
            }
            SortColumn::Calls => {
                sorted.sort_by(|&a, &b| {
                    let calls_a = Self::count_calls(module.functions[a].index, call_graph);
                    let calls_b = Self::count_calls(module.functions[b].index, call_graph);
                    if self.sort_ascending {
                        calls_a.cmp(&calls_b)
                    } else {
                        calls_b.cmp(&calls_a)
                    }
                });
            }
        }

        self.cached_indices = sorted;
        self.cache_dirty = false;
    }

    /// Count incoming calls to a function (how many times it's called by others).
    fn count_calls(func_index: u32, call_graph: Option<&CallGraph>) -> usize {
        let Some(cg) = call_graph else {
            return 0;
        };
        // Count how many times func_index appears in any edge list
        cg.edges
            .values()
            .flat_map(|targets| targets.iter())
            .filter(|&&target| target == func_index)
            .count()
    }

    /// Handle keyboard navigation for vim-style navigation.
    ///
    /// Returns Some(row_position) if the focus changed and we should scroll to that row.
    fn handle_keyboard(
        &mut self,
        ctx: &egui::Context,
        selection: &mut SelectionState,
        module: &WasmModuleInfo,
        visible_rows: usize,
    ) -> Option<usize> {
        let filtered_count = self.cached_indices.len();

        // Early return if no functions or filter has focus
        if filtered_count == 0 || self.filter_focused {
            return None;
        }

        // Get current focus position in the filtered list
        let current_pos = selection.focus_index.and_then(|focus| {
            self.cached_indices
                .iter()
                .position(|&i| module.functions[i].index == focus)
        });

        // Default to 0 if no focus
        let current_pos = current_pos.unwrap_or(0);

        // Check modifiers
        let (shift, ctrl) = ctx.input(|i| (i.modifiers.shift, i.modifiers.ctrl || i.modifiers.command));

        // Determine new position based on key pressed
        // Returns (new_position, is_navigation_key) where is_navigation_key means
        // shift should extend selection (j/k/arrows), vs jump keys (g/G/Home/End)
        // where shift is part of the key binding itself
        let (new_pos, is_navigation) = ctx.input(|i| {
            // j or ArrowDown: move down 1 (navigation - shift extends)
            if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
                return (Some(current_pos.saturating_add(1).min(filtered_count - 1)), true);
            }
            // k or ArrowUp: move up 1 (navigation - shift extends)
            if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
                return (Some(current_pos.saturating_sub(1)), true);
            }
            // g (without shift) or Home: jump to top (jump - shift is NOT extend)
            if (i.key_pressed(Key::G) && !shift) || i.key_pressed(Key::Home) {
                return (Some(0), false);
            }
            // G (with shift) or End: jump to bottom (jump - shift is part of key binding)
            if (i.key_pressed(Key::G) && shift) || i.key_pressed(Key::End) {
                return (Some(filtered_count - 1), false);
            }
            // Ctrl+d: half-page down (navigation - shift extends)
            if i.key_pressed(Key::D) && ctrl {
                let half_page = visible_rows / 2;
                return (Some(current_pos.saturating_add(half_page).min(filtered_count - 1)), true);
            }
            // Ctrl+u: half-page up (navigation - shift extends)
            if i.key_pressed(Key::U) && ctrl {
                let half_page = visible_rows / 2;
                return (Some(current_pos.saturating_sub(half_page)), true);
            }
            (None, false)
        });

        // If position changed, update selection
        if let Some(new_pos) = new_pos {
            if new_pos != current_pos || selection.focus_index.is_none() {
                let func_index = module.functions[self.cached_indices[new_pos]].index;

                if shift && is_navigation {
                    // Shift held: extend selection from last to new
                    if let Some(from) = selection.last_selected {
                        // Find from position in cached_indices
                        let from_pos = self.cached_indices
                            .iter()
                            .position(|&i| module.functions[i].index == from);

                        if let Some(from_pos) = from_pos {
                            let (start, end) = if from_pos <= new_pos {
                                (from_pos, new_pos)
                            } else {
                                (new_pos, from_pos)
                            };
                            let indices = (start..=end)
                                .map(|i| module.functions[self.cached_indices[i]].index);
                            selection.extend_select_indices(indices);
                            selection.focus_index = Some(func_index);
                        } else {
                            selection.select_single(func_index);
                        }
                    } else {
                        selection.select_single(func_index);
                    }
                } else {
                    // No shift: single select
                    selection.select_single(func_index);
                }

                self.selection_changed = true;
                return Some(new_pos);
            }
        }

        None
    }

    /// Main render method for the function list panel.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        module: &WasmModuleInfo,
        call_graph: Option<&CallGraph>,
        selection: &mut SelectionState,
    ) {
        // Reset selection_changed flag at start of frame
        self.selection_changed = false;

        // Filter input
        ui.horizontal(|ui| {
            ui.label("Filter:");
            let filter_id = ui.make_persistent_id("function_filter");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.filter_text)
                    .id(filter_id)
                    .hint_text("Type to filter...")
                    .desired_width(200.0),
            );
            if response.changed() {
                self.cache_dirty = true;
            }
            // Track if filter has focus (to disable vim keys while typing)
            self.filter_focused = response.has_focus();

            // Match count
            if self.cache_dirty {
                self.rebuild_cache(module, call_graph);
            }
            let match_count = self.cached_indices.len();
            if self.filter_text.is_empty() {
                ui.label(format!("{} functions", match_count));
            } else {
                ui.label(format!("{} of {} functions", match_count, self.total_function_count));
            }
        });

        ui.separator();

        // Rebuild cache if needed
        if self.cache_dirty {
            self.rebuild_cache(module, call_graph);
        }

        // Calculate visible rows for half-page navigation
        let available_height = ui.available_height();
        let visible_rows = (available_height / ROW_HEIGHT).floor() as usize;
        let visible_rows = visible_rows.max(1);

        // Handle keyboard navigation before building table
        let scroll_to = self.handle_keyboard(ctx, selection, module, visible_rows);

        // Build table with optional scroll_to_row
        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(200.0).clip(true)) // Name
            .column(Column::auto().at_least(80.0)) // Size
            .column(Column::auto().at_least(60.0)) // Calls
            .max_scroll_height(available_height);

        // Apply scroll_to_row if selection changed via keyboard
        if let Some(row_pos) = scroll_to {
            table = table.scroll_to_row(row_pos, Some(egui::Align::Center));
        }

        table
            .header(ROW_HEIGHT, |mut header| {
                // Name column header
                header.col(|ui| {
                    let is_active = self.sort_column == SortColumn::Name;
                    let label = if is_active {
                        if self.sort_ascending {
                            "Name ^"
                        } else {
                            "Name v"
                        }
                    } else {
                        "Name"
                    };
                    if ui.selectable_label(is_active, label).clicked() {
                        if self.sort_column == SortColumn::Name {
                            self.sort_ascending = !self.sort_ascending;
                        } else {
                            self.sort_column = SortColumn::Name;
                            self.sort_ascending = true;
                        }
                        self.cache_dirty = true;
                    }
                });
                // Size column header
                header.col(|ui| {
                    let is_active = self.sort_column == SortColumn::Size;
                    let label = if is_active {
                        if self.sort_ascending {
                            "Size ^"
                        } else {
                            "Size v"
                        }
                    } else {
                        "Size"
                    };
                    if ui.selectable_label(is_active, label).clicked() {
                        if self.sort_column == SortColumn::Size {
                            self.sort_ascending = !self.sort_ascending;
                        } else {
                            self.sort_column = SortColumn::Size;
                            self.sort_ascending = false; // Default descending for size
                        }
                        self.cache_dirty = true;
                    }
                });
                // Calls column header
                header.col(|ui| {
                    let is_active = self.sort_column == SortColumn::Calls;
                    let label = if is_active {
                        if self.sort_ascending {
                            "Calls ^"
                        } else {
                            "Calls v"
                        }
                    } else {
                        "Calls"
                    };
                    if ui.selectable_label(is_active, label).clicked() {
                        if self.sort_column == SortColumn::Calls {
                            self.sort_ascending = !self.sort_ascending;
                        } else {
                            self.sort_column = SortColumn::Calls;
                            self.sort_ascending = false; // Default descending for calls
                        }
                        self.cache_dirty = true;
                    }
                });
            })
            .body(|body| {
                // Clone the indices to avoid borrow issues
                let indices = self.cached_indices.clone();
                body.rows(ROW_HEIGHT, indices.len(), |mut row| {
                    let row_idx = row.index();
                    let func_idx = indices[row_idx];
                    let func = &module.functions[func_idx];
                    let func_index = func.index;
                    let is_selected = selection.is_selected(func_index);

                    row.set_selected(is_selected);

                    // Helper closure to handle clicks with modifier support
                    // Returns true if selection was changed
                    let mut handle_click = |ui: &egui::Ui, clicked: bool| -> bool {
                        if clicked {
                            let modifiers = ui.input(|i| i.modifiers);

                            if modifiers.ctrl || modifiers.command {
                                // Ctrl+click (or Cmd on Mac): toggle selection
                                selection.toggle_select(func_index);
                            } else if modifiers.shift {
                                // Shift+click: extend selection from last_selected to this
                                if let Some(from) = selection.last_selected {
                                    // Find positions of 'from' and 'func_index' in cached_indices
                                    let from_pos = indices.iter().position(|&i| module.functions[i].index == from);
                                    let to_pos = row_idx;
                                    if let Some(from_pos) = from_pos {
                                        let (start, end) = if from_pos <= to_pos {
                                            (from_pos, to_pos)
                                        } else {
                                            (to_pos, from_pos)
                                        };
                                        // Use extend_select_indices for filtered list range
                                        let range_indices = (start..=end)
                                            .map(|i| module.functions[indices[i]].index);
                                        selection.extend_select_indices(range_indices);
                                        selection.focus_index = Some(func_index);
                                    } else {
                                        selection.select_single(func_index);
                                    }
                                } else {
                                    selection.select_single(func_index);
                                }
                            } else {
                                // Plain click: single select
                                selection.select_single(func_index);
                            }
                            return true;
                        }
                        false
                    };

                    // Track if any column click changed selection
                    let mut clicked_in_row = false;

                    // Name column
                    row.col(|ui| {
                        let name = func.best_name();
                        let response = ui.add(
                            egui::Label::new(&name)
                                .truncate()
                                .sense(egui::Sense::click()),
                        );
                        if handle_click(ui, response.clicked()) {
                            clicked_in_row = true;
                        }
                        response.on_hover_text(&name);
                    });

                    // Size column
                    row.col(|ui| {
                        // Use to_string_as(false) for IEC units (KiB, MiB, etc.)
                        let size_str = ByteSize::b(func.code_size as u64).to_string_as(false);
                        let response = ui.add(
                            egui::Label::new(&size_str).sense(egui::Sense::click()),
                        );
                        if handle_click(ui, response.clicked()) {
                            clicked_in_row = true;
                        }
                    });

                    // Calls column
                    row.col(|ui| {
                        let calls = Self::count_calls(func.index, call_graph);
                        let response = ui.add(
                            egui::Label::new(calls.to_string()).sense(egui::Sense::click()),
                        );
                        if handle_click(ui, response.clicked()) {
                            clicked_in_row = true;
                        }
                    });

                    // Note: Click-based selection doesn't need scroll_to_row since user
                    // already clicked on a visible row. We track clicked_in_row for
                    // potential future use (e.g., triggering inspector updates).
                    let _ = clicked_in_row;
                });
            });

        // Request repaint if selection changed to ensure smooth updates
        if self.selection_changed {
            ctx.request_repaint();
        }
    }
}

impl Default for FunctionListPanel {
    fn default() -> Self {
        Self::new()
    }
}
