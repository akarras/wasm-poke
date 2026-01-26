//! Function list panel with virtualized table rendering.
//!
//! This panel displays all functions in a scrollable table with sortable columns
//! (Name, Size, Calls) and a filter input for live filtering.

use bytesize::ByteSize;
use eframe::egui;
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

    /// Main render method for the function list panel.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        module: &WasmModuleInfo,
        call_graph: Option<&CallGraph>,
        selection: &mut SelectionState,
    ) {
        // Filter input
        ui.horizontal(|ui| {
            ui.label("Filter:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.filter_text)
                    .hint_text("Type to filter...")
                    .desired_width(200.0),
            );
            if response.changed() {
                self.cache_dirty = true;
            }

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

        // Table
        let available_height = ui.available_height();
        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto().at_least(200.0).clip(true)) // Name
            .column(Column::auto().at_least(80.0)) // Size
            .column(Column::auto().at_least(60.0)) // Calls
            .max_scroll_height(available_height)
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
                    let is_selected = selection.is_selected(func.index);

                    row.set_selected(is_selected);

                    // Name column
                    row.col(|ui| {
                        let name = func.best_name();
                        let response = ui.add(
                            egui::Label::new(&name)
                                .truncate()
                                .sense(egui::Sense::click()),
                        );
                        if response.clicked() {
                            selection.select_single(func.index);
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
                        if response.clicked() {
                            selection.select_single(func.index);
                        }
                    });

                    // Calls column
                    row.col(|ui| {
                        let calls = Self::count_calls(func.index, call_graph);
                        let response = ui.add(
                            egui::Label::new(calls.to_string()).sense(egui::Sense::click()),
                        );
                        if response.clicked() {
                            selection.select_single(func.index);
                        }
                    });
                });
            });
    }
}

impl Default for FunctionListPanel {
    fn default() -> Self {
        Self::new()
    }
}
