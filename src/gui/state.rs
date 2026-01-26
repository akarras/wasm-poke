//! Centralized selection state for wasm-poke.
//!
//! This module defines `SelectionState`, the single source of truth for all
//! selection-related state in the application. ALL panels read/write this
//! struct; never duplicate selection state to prevent sync bugs.

use std::collections::HashSet;

/// Centralized selection state - single source of truth for all views.
///
/// ALL panels read/write this struct; never duplicate selection state.
/// This design prevents the TUI sync bugs (wat_cursor, source_scroll desync)
/// that motivated the egui rewrite.
#[derive(Default)]
pub struct SelectionState {
    /// Currently selected function by INDEX (not list position).
    /// Using index survives filter changes.
    pub selected_function: Option<u32>,

    /// Cursor position in WAT instruction list.
    pub instruction_cursor: usize,

    /// Expanded nodes in tree views, identified by path from root.
    /// Each element is (function_index, child_position_in_parent).
    pub expanded_nodes: HashSet<Vec<(u32, usize)>>,
}
