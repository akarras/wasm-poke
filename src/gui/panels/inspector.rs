//! Inspector panel with three-panel view: Hex, WAT, and Source.
//!
//! This panel displays WAT disassembly, hex bytes, and source code for the
//! selected function with vim-style keyboard navigation (j/k/g/G) and
//! synchronized visual highlighting across all panels.

use std::collections::HashMap;

use eframe::egui::{self, Key, RichText, ScrollArea};

use crate::gui::state::SelectionState;
use wasm_poke::{
    disassemble_function_wat_lines, function_body_bytes, map_instr_to_source_fast,
    SourceLocation, WasmModuleInfo, WatLine,
};

/// Row height for WAT instruction lines.
const ROW_HEIGHT: f32 = 18.0;

/// Panel for displaying WAT disassembly, hex bytes, and source code.
pub struct InspectorPanel {
    /// Cached function index to detect selection changes.
    cached_func_index: Option<u32>,
    /// Cached WAT lines for current function.
    cached_wat_lines: Vec<WatLine>,
    /// Cached function body bytes.
    cached_hex_bytes: Vec<u8>,
    /// Cached source mappings: instruction index -> SourceLocation.
    cached_source_mappings: Vec<Option<SourceLocation>>,
    /// Cached source file content: path -> lines.
    source_file_cache: HashMap<String, Vec<String>>,
    /// Current source file path (for display).
    cached_source_path: Option<String>,
    /// Current source file lines (for current function).
    cached_source_lines: Vec<String>,
    /// Current source line number (1-indexed, from DWARF).
    cached_current_source_line: Option<u32>,
    /// Last cursor position that triggered scroll (to avoid redundant scrolls).
    last_scrolled_cursor: Option<usize>,
}

impl InspectorPanel {
    /// Create a new inspector panel with empty cache.
    pub fn new() -> Self {
        Self {
            cached_func_index: None,
            cached_wat_lines: Vec::new(),
            cached_hex_bytes: Vec::new(),
            cached_source_mappings: Vec::new(),
            source_file_cache: HashMap::new(),
            cached_source_path: None,
            cached_source_lines: Vec::new(),
            cached_current_source_line: None,
            last_scrolled_cursor: None,
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

        // Compute source mappings for each WAT line
        self.cached_source_mappings = self
            .cached_wat_lines
            .iter()
            .map(|wl| map_instr_to_source_fast(module, wasm_bytes, func_index, wl.offset))
            .collect();

        // Determine primary source file (most common in mappings)
        let mut file_counts: HashMap<&str, usize> = HashMap::new();
        for loc in self.cached_source_mappings.iter().flatten() {
            *file_counts.entry(&loc.file).or_insert(0) += 1;
        }
        self.cached_source_path = file_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(path, _)| path.to_string());

        // Load source file if found
        if let Some(ref path) = self.cached_source_path {
            if !self.source_file_cache.contains_key(path) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let lines: Vec<String> = content.lines().map(String::from).collect();
                    self.source_file_cache.insert(path.clone(), lines);
                }
            }
            self.cached_source_lines = self
                .source_file_cache
                .get(path)
                .cloned()
                .unwrap_or_default();
        } else {
            self.cached_source_lines = Vec::new();
        }

        // Reset cursor when function changes
        selection.instruction_cursor = 0;

        true
    }

    /// Get the current source line number for the given cursor position.
    fn current_source_line(&self, cursor: usize) -> Option<u32> {
        self.cached_source_mappings
            .get(cursor)
            .and_then(|opt| opt.as_ref())
            .map(|loc| loc.line)
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
            ui.label(RichText::new("Inspector:").strong());
            ui.label(&func_name);
        });
        ui.separator();

        let line_count = self.cached_wat_lines.len();

        // Handle keyboard navigation
        let keyboard_scroll = self.handle_keyboard(ctx, selection, line_count);

        // Update current source line based on cursor
        self.cached_current_source_line = self.current_source_line(selection.instruction_cursor);

        // Determine if we need to scroll (cursor changed from keyboard, click, or function change)
        let scroll_to = if cache_updated {
            // Function changed - always scroll to new cursor position (which is 0)
            self.last_scrolled_cursor = Some(selection.instruction_cursor);
            Some(selection.instruction_cursor)
        } else if keyboard_scroll.is_some() {
            // Keyboard navigation changed cursor
            self.last_scrolled_cursor = Some(selection.instruction_cursor);
            keyboard_scroll
        } else if self.last_scrolled_cursor != Some(selection.instruction_cursor) {
            // Click changed cursor (detected by mismatch)
            self.last_scrolled_cursor = Some(selection.instruction_cursor);
            Some(selection.instruction_cursor)
        } else {
            None
        };

        // Calculate total available dimensions
        let available_width = ui.available_width();
        let available_height = ui.available_height();

        // Three-panel layout using horizontal with sized children
        ui.horizontal(|ui| {
            // Hex panel (~20% width)
            let hex_width = available_width * 0.20;
            ui.allocate_ui_with_layout(
                egui::vec2(hex_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.show_hex_panel(ui, selection.instruction_cursor, scroll_to);
                },
            );

            ui.separator();

            // WAT panel (~45% width)
            let wat_width = available_width * 0.45;
            ui.allocate_ui_with_layout(
                egui::vec2(wat_width, available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.show_wat_panel(ui, selection, scroll_to);
                },
            );

            ui.separator();

            // Source panel (remaining width)
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), available_height),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.show_source_panel(ui, scroll_to);
                },
            );
        });
    }

    /// Render the WAT disassembly panel.
    fn show_wat_panel(
        &self,
        ui: &mut egui::Ui,
        selection: &mut SelectionState,
        scroll_to: Option<usize>,
    ) {
        ui.label(RichText::new("WAT").strong());
        ui.separator();

        let line_count = self.cached_wat_lines.len();
        let available_height = ui.available_height();

        // Create scroll area with WAT display
        let mut scroll_area = ScrollArea::vertical()
            .id_salt("wat_scroll")
            .max_height(available_height);

        // Apply scroll offset if cursor changed (position row near top of view)
        if let Some(row) = scroll_to {
            // Calculate offset that positions the row in view with some padding
            let offset = (row as f32 * ROW_HEIGHT).max(0.0);
            scroll_area = scroll_area.vertical_scroll_offset(offset);
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

    /// Render the hex bytes panel.
    fn show_hex_panel(&self, ui: &mut egui::Ui, cursor: usize, scroll_to: Option<usize>) {
        ui.label(RichText::new("Hex Bytes").strong());
        ui.separator();

        if self.cached_hex_bytes.is_empty() {
            ui.label("No bytes");
            return;
        }

        // Calculate byte range for current instruction
        let highlight_range = self.instruction_byte_range(cursor);

        let bytes_per_row = 8; // Narrow panel
        let row_count = (self.cached_hex_bytes.len() + bytes_per_row - 1) / bytes_per_row;

        // Calculate which hex row to scroll to based on highlighted byte range
        let scroll_to_hex_row = scroll_to.and_then(|_| {
            highlight_range.as_ref().map(|r| r.start / bytes_per_row)
        });

        let mut scroll_area = ScrollArea::vertical().id_salt("hex_scroll");

        if let Some(row) = scroll_to_hex_row {
            let offset = (row as f32 * ROW_HEIGHT).max(0.0);
            scroll_area = scroll_area.vertical_scroll_offset(offset);
        }

        scroll_area.show_rows(ui, ROW_HEIGHT, row_count, |ui, row_range| {
                for row_idx in row_range {
                    let start = row_idx * bytes_per_row;
                    let end = (start + bytes_per_row).min(self.cached_hex_bytes.len());
                    let row_bytes = &self.cached_hex_bytes[start..end];

                    ui.horizontal(|ui| {
                        // Offset gutter
                        ui.label(
                            RichText::new(format!("{:04x}:", start))
                                .monospace()
                                .weak(),
                        );

                        // Hex bytes with highlighting
                        for (i, byte) in row_bytes.iter().enumerate() {
                            let byte_offset = start + i;
                            let is_highlighted = highlight_range
                                .as_ref()
                                .map(|r| r.contains(&byte_offset))
                                .unwrap_or(false);

                            let text = RichText::new(format!("{:02x}", byte)).monospace();
                            if is_highlighted {
                                ui.label(text.background_color(ui.visuals().selection.bg_fill));
                            } else {
                                ui.label(text);
                            }
                        }
                    });
                }
            });
    }

    /// Calculate the byte range for the current instruction.
    fn instruction_byte_range(&self, cursor: usize) -> Option<std::ops::Range<usize>> {
        let current = self.cached_wat_lines.get(cursor)?;
        let next = self.cached_wat_lines.get(cursor + 1);

        let start = current.offset;
        let end = next.map(|n| n.offset).unwrap_or_else(|| {
            // Last instruction: estimate 4 bytes or to end of bytes
            (start + 4).min(self.cached_hex_bytes.len())
        });

        // Only return valid ranges
        if start < end && start < self.cached_hex_bytes.len() {
            Some(start..end)
        } else {
            None
        }
    }

    /// Render the source code panel.
    fn show_source_panel(&self, ui: &mut egui::Ui, scroll_to: Option<usize>) {
        // Header with file path
        if let Some(ref path) = self.cached_source_path {
            // Show just filename, not full path
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            ui.label(RichText::new(format!("Source: {}", filename)).strong());
        } else {
            ui.label(RichText::new("Source").strong());
        }
        ui.separator();

        // Handle no source info
        if self.cached_source_lines.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No source info available");
            });
            return;
        }

        // Get current source line (1-indexed)
        let current_line = self.cached_current_source_line;

        let line_count = self.cached_source_lines.len();

        // Scroll to current source line (convert 1-indexed to 0-indexed)
        let scroll_to_source_row = scroll_to.and_then(|_| {
            current_line.map(|l| (l.saturating_sub(1)) as usize)
        });

        let mut scroll_area = ScrollArea::vertical().id_salt("source_scroll");

        if let Some(row) = scroll_to_source_row {
            let offset = (row as f32 * ROW_HEIGHT).max(0.0);
            scroll_area = scroll_area.vertical_scroll_offset(offset);
        }

        scroll_area.show_rows(ui, ROW_HEIGHT, line_count, |ui, row_range| {
                for line_idx in row_range {
                    let line_num = line_idx + 1; // 1-indexed
                    let is_highlighted = current_line == Some(line_num as u32);

                    // Reserve space for the row
                    let (rect, _response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), ROW_HEIGHT),
                        egui::Sense::hover(),
                    );

                    // Paint background highlight for current line
                    if is_highlighted {
                        ui.painter()
                            .rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                    }

                    // Line number gutter
                    let gutter_text = format!("{:4} ", line_num);
                    let gutter_pos = rect.left_top() + egui::vec2(2.0, 2.0);
                    ui.painter().text(
                        gutter_pos,
                        egui::Align2::LEFT_TOP,
                        &gutter_text,
                        egui::FontId::monospace(14.0),
                        ui.visuals().weak_text_color(),
                    );

                    // Source line text
                    let line_text = self
                        .cached_source_lines
                        .get(line_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");

                    let text_color = if is_highlighted {
                        ui.visuals().strong_text_color()
                    } else {
                        ui.visuals().text_color()
                    };

                    // Position after gutter (5 chars * ~8px per char)
                    let text_pos = rect.left_top() + egui::vec2(42.0, 2.0);
                    ui.painter().text(
                        text_pos,
                        egui::Align2::LEFT_TOP,
                        line_text,
                        egui::FontId::monospace(14.0),
                        text_color,
                    );
                }
            });
    }
}

impl Default for InspectorPanel {
    fn default() -> Self {
        Self::new()
    }
}
