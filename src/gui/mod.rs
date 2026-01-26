//! GUI module for wasm-poke egui application.
//!
//! This module provides the egui-based graphical interface with dockable panels
//! for function analysis, call graphs, and source inspection.

mod app;
pub mod panels;
mod state;
mod tabs;

pub use app::WasmPokeApp;
pub use state::SelectionState;
pub use tabs::TabKind;
