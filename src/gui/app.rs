//! Main application struct and eframe::App implementation.
//!
//! Contains `WasmPokeApp` which holds all application state and implements
//! the egui rendering loop.

use std::path::PathBuf;

use eframe::egui;
use egui_dock::{DockArea, DockState, NodeIndex};

use crate::gui::state::SelectionState;
use crate::gui::tabs::TabKind;
use wasm_poke::{CallGraph, WasmModuleInfo};

/// Main application struct holding all state.
pub struct WasmPokeApp {
    // Analysis data (immutable after load)
    /// Parsed module information (function list, sizes, names).
    pub module: Option<WasmModuleInfo>,
    /// Call graph edges between functions.
    pub call_graph: Option<CallGraph>,
    /// Raw wasm bytes for disassembly and source mapping.
    pub wasm_bytes: Option<Vec<u8>>,
    /// Path to the loaded wasm file.
    pub wasm_path: Option<String>,

    // Selection state (single source of truth)
    /// Centralized selection state shared across all panels.
    pub selection: SelectionState,

    // UI layout
    /// Dock state for the panel layout.
    dock_state: DockState<TabKind>,
}

impl WasmPokeApp {
    /// Create a new application instance.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            module: None,
            call_graph: None,
            wasm_bytes: None,
            wasm_path: None,
            selection: SelectionState::default(),
            dock_state: Self::default_dock_state(),
        }
    }

    /// Create the default dock layout with four panels.
    fn default_dock_state() -> DockState<TabKind> {
        let mut state = DockState::new(vec![TabKind::FunctionList]);

        // Split: left panel (list/tree tabs), right panel (inspector)
        let surface = state.main_surface_mut();
        let [_left, _right] = surface.split_right(
            NodeIndex::root(),
            0.6, // Inspector gets 60% of width
            vec![TabKind::Inspector],
        );

        // Add more tabs to the left panel
        state.push_to_focused_leaf(TabKind::CallTree);
        state.push_to_focused_leaf(TabKind::SizeTree);

        state
    }

    /// Load a Wasm file via native file dialog and parse it.
    fn load_wasm_file(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("WebAssembly", &["wasm"])
            .pick_file()
        {
            self.load_wasm_from_path(path);
        }
    }

    /// Load and parse a Wasm file from a given path.
    fn load_wasm_from_path(&mut self, path: PathBuf) {
        match wasm_poke::parse_wasm(&path) {
            Ok(module) => {
                // Read bytes for call graph analysis
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let call_graph = wasm_poke::build_call_graph(&bytes).ok();

                        self.wasm_path = Some(path.display().to_string());
                        self.wasm_bytes = Some(bytes);
                        self.module = Some(module);
                        self.call_graph = call_graph;
                        self.selection = SelectionState::default();

                        log::info!("Loaded: {}", path.display());
                    }
                    Err(e) => {
                        log::error!("Failed to read file: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to parse wasm: {}", e);
            }
        }
    }
}

impl eframe::App for WasmPokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Global keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.ctrl) {
            self.load_wasm_file();
        }

        // Menu bar for file operations
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        self.load_wasm_file();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        // Main dock area - create tab viewer with references to app state
        let mut tab_viewer = WasmPokeTabViewer {
            module: self.module.as_ref(),
            wasm_path: self.wasm_path.as_deref(),
        };

        egui::CentralPanel::default().show(ctx, |_ui| {
            DockArea::new(&mut self.dock_state)
                .show(ctx, &mut tab_viewer);
        });
    }
}

/// TabViewer implementation that renders each panel type.
///
/// This struct holds references to the parts of WasmPokeApp needed for rendering,
/// avoiding the borrow checker issue with passing &mut self to both DockArea and TabViewer.
pub struct WasmPokeTabViewer<'a> {
    module: Option<&'a WasmModuleInfo>,
    wasm_path: Option<&'a str>,
}

impl egui_dock::TabViewer for WasmPokeTabViewer<'_> {
    type Tab = TabKind;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            TabKind::FunctionList => {
                if let Some(module) = self.module {
                    ui.horizontal(|ui| {
                        ui.label("Loaded:");
                        ui.label(self.wasm_path.unwrap_or("unknown"));
                    });
                    ui.separator();
                    ui.label(format!(
                        "{} defined functions, {} total code bytes",
                        module.defined_functions, module.total_code_size
                    ));
                } else {
                    ui.label("No file loaded. Use File -> Open to load a .wasm file.");
                }
            }
            TabKind::CallTree => {
                ui.label("Call Graph (Phase 3)");
            }
            TabKind::SizeTree => {
                ui.label("Size Tree (Phase 3)");
            }
            TabKind::Inspector => {
                ui.label("Inspector (Phase 4)");
            }
        }
    }
}
