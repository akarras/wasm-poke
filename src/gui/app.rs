//! Main application struct and eframe::App implementation.
//!
//! Contains `WasmPokeApp` which holds all application state and implements
//! the egui rendering loop.

use egui_dock::{DockArea, DockState, NodeIndex};

use crate::gui::state::SelectionState;
use crate::gui::tabs::TabKind;
use crate::{CallGraph, WasmModuleInfo};

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
}

impl eframe::App for WasmPokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Menu bar for file operations
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open...").clicked() {
                        // File loading implemented in Plan 02
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        // Main dock area
        egui::CentralPanel::default().show(ctx, |_ui| {
            DockArea::new(&mut self.dock_state)
                .show(ctx, &mut WasmPokeTabViewer { app: self });
        });
    }
}

/// TabViewer implementation that renders each panel type.
pub struct WasmPokeTabViewer<'a> {
    app: &'a mut WasmPokeApp,
}

impl egui_dock::TabViewer for WasmPokeTabViewer<'_> {
    type Tab = TabKind;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        tab.title().into()
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match tab {
            TabKind::FunctionList => {
                if let Some(module) = &self.app.module {
                    ui.label(format!("{} functions", module.defined_functions));
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
