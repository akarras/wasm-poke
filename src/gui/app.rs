//! Main application struct and eframe::App implementation.
//!
//! Contains `WasmPokeApp` which holds all application state and implements
//! the egui rendering loop.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use egui_dock::{DockArea, DockState, NodeIndex};

use crate::gui::panels::{CallTreePanel, CallersTreePanel, FunctionListPanel, InspectorPanel, SizeTreePanel};
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
    /// Reverse call graph: callee -> [callers]
    /// Computed once on file load for Callers tree view.
    pub reverse_graph: Option<HashMap<u32, Vec<u32>>>,
    /// Raw wasm bytes for disassembly and source mapping.
    pub wasm_bytes: Option<Vec<u8>>,
    /// Path to the loaded wasm file.
    pub wasm_path: Option<String>,

    // Selection state (single source of truth)
    /// Centralized selection state shared across all panels.
    pub selection: SelectionState,

    // Panel state
    /// Function list panel state (filter, sort, cache).
    function_list_panel: FunctionListPanel,
    /// Call tree panel state.
    call_tree_panel: CallTreePanel,
    /// Callers tree panel state.
    callers_tree_panel: CallersTreePanel,
    /// Size tree panel state.
    size_tree_panel: SizeTreePanel,
    /// Inspector panel state.
    inspector_panel: InspectorPanel,

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
            reverse_graph: None,
            wasm_bytes: None,
            wasm_path: None,
            selection: SelectionState::default(),
            function_list_panel: FunctionListPanel::new(),
            call_tree_panel: CallTreePanel::new(),
            callers_tree_panel: CallersTreePanel::new(),
            size_tree_panel: SizeTreePanel::new(),
            inspector_panel: InspectorPanel::new(),
            dock_state: Self::default_dock_state(),
        }
    }

    /// Build reverse call graph: callee -> [callers]
    /// Used by Callers tree to show "who calls this function"
    fn build_reverse_graph(graph: &CallGraph) -> HashMap<u32, Vec<u32>> {
        let mut reverse: HashMap<u32, Vec<u32>> = HashMap::new();
        for (&caller, callees) in &graph.edges {
            for &callee in callees {
                reverse.entry(callee).or_default().push(caller);
            }
        }
        reverse
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
        state.push_to_focused_leaf(TabKind::Callers);
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
                        let reverse_graph = call_graph.as_ref().map(Self::build_reverse_graph);

                        self.wasm_path = Some(path.display().to_string());
                        self.wasm_bytes = Some(bytes);
                        self.module = Some(module);
                        self.call_graph = call_graph;
                        self.reverse_graph = reverse_graph;
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
            ctx,
            module: self.module.as_ref(),
            call_graph: self.call_graph.as_ref(),
            reverse_graph: self.reverse_graph.as_ref(),
            wasm_bytes: self.wasm_bytes.as_deref(),
            selection: &mut self.selection,
            inspector_panel: &mut self.inspector_panel,
            function_list_panel: &mut self.function_list_panel,
            call_tree_panel: &mut self.call_tree_panel,
            callers_tree_panel: &mut self.callers_tree_panel,
            size_tree_panel: &mut self.size_tree_panel,
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
    ctx: &'a egui::Context,
    module: Option<&'a WasmModuleInfo>,
    call_graph: Option<&'a CallGraph>,
    reverse_graph: Option<&'a HashMap<u32, Vec<u32>>>,
    wasm_bytes: Option<&'a [u8]>,
    selection: &'a mut SelectionState,
    inspector_panel: &'a mut InspectorPanel,
    function_list_panel: &'a mut FunctionListPanel,
    call_tree_panel: &'a mut CallTreePanel,
    callers_tree_panel: &'a mut CallersTreePanel,
    size_tree_panel: &'a mut SizeTreePanel,
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
                    self.function_list_panel.show(
                        self.ctx,
                        ui,
                        module,
                        self.call_graph,
                        self.selection,
                    );
                } else {
                    ui.label("No file loaded. Use File -> Open to load a .wasm file.");
                }
            }
            TabKind::CallTree => {
                if let Some(module) = self.module {
                    self.call_tree_panel.show(
                        self.ctx,
                        ui,
                        module,
                        self.call_graph,
                        self.selection,
                    );
                } else {
                    ui.label("No file loaded. Use File -> Open to load a .wasm file.");
                }
            }
            TabKind::Callers => {
                if let Some(module) = self.module {
                    self.callers_tree_panel.show(
                        self.ctx,
                        ui,
                        module,
                        self.reverse_graph,
                        self.selection,
                    );
                } else {
                    ui.label("No file loaded. Use File -> Open to load a .wasm file.");
                }
            }
            TabKind::SizeTree => {
                if let Some(module) = self.module {
                    self.size_tree_panel.show(
                        self.ctx,
                        ui,
                        module,
                        self.call_graph,
                        self.selection,
                    );
                } else {
                    ui.label("No file loaded. Use File -> Open to load a .wasm file.");
                }
            }
            TabKind::Inspector => {
                if let (Some(module), Some(wasm_bytes)) = (self.module, self.wasm_bytes) {
                    self.inspector_panel.show(
                        self.ctx,
                        ui,
                        module,
                        wasm_bytes,
                        self.selection,
                    );
                } else {
                    ui.label("No file loaded. Use File -> Open to load a .wasm file.");
                }
            }
        }
    }
}
