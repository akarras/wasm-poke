//! Inspector panel with WAT disassembly display and keyboard navigation.
//!
//! This panel displays WAT instructions for the selected function with
//! vim-style keyboard navigation (j/k/g/G) and visual line highlighting.

use eframe::egui::{self, Key, RichText, ScrollArea};

use crate::gui::state::SelectionState;
use wasm_poke::{disassemble_function_wat_lines, function_body_bytes, WasmModuleInfo, WatLine};

/// Row height for WAT instruction lines.
const ROW_HEIGHT: f32 = 18.0;

/// Panel for displaying WAT disassembly of the selected function.
pub struct InspectorPanel {
    /// Cached function index to detect selection changes.
    cached_func_index: Option<u32>,
    /// Cached WAT lines for current function.
    cached_wat_lines: Vec<WatLine>,
    /// Cached function body bytes.
    cached_hex_bytes: Vec<u8>,
}

impl InspectorPanel {
    /// Create a new inspector panel with empty cache.
    pub fn new() -> Self {
        Self {
            cached_func_index: None,
            cached_wat_lines: Vec::new(),
            cached_hex_bytes: Vec::new(),
        }
    }

    /// Update cache if function selection changed.
    /// Returns true if cache was updated (function changed).
    fn update_cache(
        &mut self,
        func_index: u32,
        module: &WasmModuleInfo,
        wasm_bytes: &[u8],
        selection: &mut SelectionState,
    ) -> bool {
        if self.cached_func_index == Some(func_index) {
            return false;
        }

        // Function changed - update cache
        self.cached_func_index = Some(func_index);

        // Disassemble to WAT lines
        match disassemble_function_wat_lines(wasm_bytes, func_index) {
            Ok(lines) => {
                self.cached_wat_lines = lines;
            }
            Err(_) => {
                self.cached_wat_lines = vec![WatLine {
                    text: format!(";; Failed to disassemble function {}", func_index),
                    offset: 0,
                    indent: 0,
                    src: None,
                }];
            }
        }

        // Get function body bytes
        if let Some(bytes) = function_body_bytes(module, wasm_bytes, func_index) {
            self.cached_hex_bytes = bytes.to_vec();
        } else {
            self.cached_hex_bytes = Vec::new();
        }

        // Reset cursor when function changes
        selection.instruction_cursor = 0;

        true
    }

    /// Handle keyboard navigation for vim-style navigation.
    ///
    /// Returns Some(row_position) if the cursor changed and we should scroll to that row.
    fn handle_keyboard(
        &self,
        ctx: &egui::Context,
        selection: &mut SelectionState,
        line_count: usize,
    ) -> Option<usize> {
        if line_count == 0 {
            return None;
        }

        let current = selection.instruction_cursor;

        // Check modifiers
        let shift = ctx.input(|i| i.modifiers.shift);

        let new_pos = ctx.input(|i| {
            // j or ArrowDown: move down 1
            if i.key_pressed(Key::J) || i.key_pressed(Key::ArrowDown) {
                return Some(current.saturating_add(1).min(line_count - 1));
            }
            // k or ArrowUp: move up 1
            if i.key_pressed(Key::K) || i.key_pressed(Key::ArrowUp) {
                return Some(current.saturating_sub(1));
            }
            // g (without shift) or Home: jump to top
            if (i.key_pressed(Key::G) && !shift) || i.key_pressed(Key::Home) {
                return Some(0);
            }
            // G (with shift) or End: jump to bottom
            if (i.key_pressed(Key::G) && shift) || i.key_pressed(Key::End) {
                return Some(line_count - 1);
            }
            None
        });

        if let Some(new_pos) = new_pos {
            if new_pos != current {
                selection.instruction_cursor = new_pos;
                return Some(new_pos);
            }
        }

        None
    }

    /// Main render method for the inspector panel.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        module: &WasmModuleInfo,
        wasm_bytes: &[u8],
        selection: &mut SelectionState,
    ) {
        // Check if a function is selected
        let Some(func_index) = selection.last_selected else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a function from the list to view its WAT disassembly.");
            });
            return;
        };

        // Update cache if function changed
        let cache_updated = self.update_cache(func_index, module, wasm_bytes, selection);

        // Get function name for header
        let func_name = module
            .functions
            .iter()
            .find(|f| f.index == func_index)
            .map(|f| f.best_name())
            .unwrap_or_else(|| format!("func[{}]", func_index));

        // Header with function name
        ui.horizontal(|ui| {
            ui.label(RichText::new("WAT Disassembly:").strong());
            ui.label(&func_name);
        });
        ui.separator();

        let line_count = self.cached_wat_lines.len();

        // Handle keyboard navigation
        let scroll_to = self.handle_keyboard(ctx, selection, line_count);

        // Calculate visible area
        let available_height = ui.available_height();

        // Create scroll area with WAT display
        let mut scroll_area = ScrollArea::vertical()
            .id_salt("wat_scroll")
            .max_height(available_height);

        // Apply scroll_to_row if cursor changed via keyboard or cache updated
        if let Some(row) = scroll_to {
            scroll_area = scroll_area.vertical_scroll_offset(row as f32 * ROW_HEIGHT);
        } else if cache_updated && selection.instruction_cursor == 0 {
            scroll_area = scroll_area.vertical_scroll_offset(0.0);
        }

        scroll_area.show_rows(ui, ROW_HEIGHT, line_count, |ui, row_range| {
            let current_cursor = selection.instruction_cursor;

            for row_idx in row_range {
                if row_idx >= self.cached_wat_lines.len() {
                    continue;
                }

                let line = &self.cached_wat_lines[row_idx];
                let is_current = row_idx == current_cursor;

                // Reserve space for the row
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), ROW_HEIGHT),
                    egui::Sense::click(),
                );

                // Paint background highlight for current line BEFORE text
                if is_current {
                    let highlight_color = ui.visuals().selection.bg_fill;
                    ui.painter().rect_filled(rect, 0.0, highlight_color);
                }

                // Draw the text
                let text_color = if is_current {
                    ui.visuals().strong_text_color()
                } else {
                    ui.visuals().text_color()
                };

                // Position text within the row
                let text_pos = rect.left_top() + egui::vec2(4.0, 2.0);
                ui.painter().text(
                    text_pos,
                    egui::Align2::LEFT_TOP,
                    &line.text,
                    egui::FontId::monospace(14.0),
                    text_color,
                );

                // Handle click to set cursor
                if response.clicked() {
                    selection.instruction_cursor = row_idx;
                }
            }
        });
    }
}

impl Default for InspectorPanel {
    fn default() -> Self {
        Self::new()
    }
}
