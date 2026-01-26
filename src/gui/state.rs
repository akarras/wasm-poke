//! Centralized selection state for wasm-poke.
//!
//! This module defines `SelectionState`, the single source of truth for all
//! selection-related state in the application. ALL panels read/write this
//! struct; never duplicate selection state to prevent sync bugs.

use std::collections::{BTreeSet, HashSet};

/// Centralized selection state - single source of truth for all views.
///
/// ALL panels read/write this struct; never duplicate selection state.
/// This design prevents the TUI sync bugs (wat_cursor, source_scroll desync)
/// that motivated the egui rewrite.
///
/// # Multi-select behavior
///
/// - `selected_functions`: All currently selected function indices (BTreeSet for ordered iteration)
/// - `last_selected`: Most recently selected function (drives inspector display)
/// - `focus_index`: Keyboard navigation focus (may differ from selection)
///
/// # Selection operations
///
/// - Single click: `select_single()` - clears selection, selects one
/// - Ctrl+click: `toggle_select()` - toggles individual function
/// - Shift+click: `extend_select()` or `extend_select_indices()` - range selection
#[derive(Default)]
pub struct SelectionState {
    /// Currently selected functions by INDEX (not list position).
    /// Using BTreeSet for deterministic ordered iteration.
    /// Using index survives filter changes.
    pub selected_functions: BTreeSet<u32>,

    /// Most recently selected function index.
    /// This is what the inspector displays.
    pub last_selected: Option<u32>,

    /// Keyboard navigation focus index.
    /// May differ from selection (e.g., navigating with arrows before pressing Enter).
    pub focus_index: Option<u32>,

    /// Cursor position in WAT instruction list.
    pub instruction_cursor: usize,

    /// Expanded nodes in tree views, identified by path from root.
    /// Each element is (function_index, child_position_in_parent).
    pub expanded_nodes: HashSet<Vec<(u32, usize)>>,
}

impl SelectionState {
    /// Select a single function, clearing all other selections.
    ///
    /// Used for regular click without modifiers.
    pub fn select_single(&mut self, index: u32) {
        self.selected_functions.clear();
        self.selected_functions.insert(index);
        self.last_selected = Some(index);
        self.focus_index = Some(index);
    }

    /// Toggle selection of a function.
    ///
    /// Used for Ctrl+click behavior.
    pub fn toggle_select(&mut self, index: u32) {
        if self.selected_functions.contains(&index) {
            self.selected_functions.remove(&index);
            // Update last_selected if we just deselected it
            if self.last_selected == Some(index) {
                self.last_selected = self.selected_functions.last().copied();
            }
        } else {
            self.selected_functions.insert(index);
            self.last_selected = Some(index);
        }
        self.focus_index = Some(index);
    }

    /// Extend selection with a contiguous range of indices.
    ///
    /// Used for Shift+click range selection when list is unfiltered.
    /// Inserts all indices from `from` to `to` inclusive.
    pub fn extend_select(&mut self, from: u32, to: u32) {
        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        for i in start..=end {
            self.selected_functions.insert(i);
        }
        self.last_selected = Some(to);
        self.focus_index = Some(to);
    }

    /// Extend selection with an arbitrary set of indices.
    ///
    /// Used for Shift+click range selection in filtered lists where
    /// the visible range may not be contiguous indices.
    pub fn extend_select_indices(&mut self, indices: impl Iterator<Item = u32>) {
        let mut last = None;
        for index in indices {
            self.selected_functions.insert(index);
            last = Some(index);
        }
        if let Some(idx) = last {
            self.last_selected = Some(idx);
            self.focus_index = Some(idx);
        }
    }

    /// Clear all selection state.
    pub fn clear_selection(&mut self) {
        self.selected_functions.clear();
        self.last_selected = None;
        self.focus_index = None;
    }

    /// Check if a function is selected.
    pub fn is_selected(&self, index: u32) -> bool {
        self.selected_functions.contains(&index)
    }

    /// Get the primary selection for inspector integration.
    ///
    /// Returns the most recently selected function.
    pub fn primary_selection(&self) -> Option<u32> {
        self.last_selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_single() {
        let mut state = SelectionState::default();
        state.select_single(5);

        assert_eq!(state.selected_functions.len(), 1);
        assert!(state.is_selected(5));
        assert_eq!(state.last_selected, Some(5));
        assert_eq!(state.focus_index, Some(5));

        // Selecting another clears previous
        state.select_single(10);
        assert_eq!(state.selected_functions.len(), 1);
        assert!(!state.is_selected(5));
        assert!(state.is_selected(10));
    }

    #[test]
    fn test_toggle_select() {
        let mut state = SelectionState::default();
        state.select_single(5);
        state.toggle_select(10);

        assert_eq!(state.selected_functions.len(), 2);
        assert!(state.is_selected(5));
        assert!(state.is_selected(10));
        assert_eq!(state.last_selected, Some(10));

        // Toggle off
        state.toggle_select(5);
        assert_eq!(state.selected_functions.len(), 1);
        assert!(!state.is_selected(5));
        assert!(state.is_selected(10));
    }

    #[test]
    fn test_extend_select() {
        let mut state = SelectionState::default();
        state.select_single(5);
        state.extend_select(5, 10);

        assert_eq!(state.selected_functions.len(), 6);
        for i in 5..=10 {
            assert!(state.is_selected(i));
        }
        assert_eq!(state.last_selected, Some(10));
    }

    #[test]
    fn test_extend_select_indices() {
        let mut state = SelectionState::default();
        state.extend_select_indices([1, 3, 5, 7].into_iter());

        assert_eq!(state.selected_functions.len(), 4);
        assert!(state.is_selected(1));
        assert!(state.is_selected(3));
        assert!(state.is_selected(5));
        assert!(state.is_selected(7));
        assert!(!state.is_selected(2));
        assert_eq!(state.last_selected, Some(7));
    }

    #[test]
    fn test_clear_selection() {
        let mut state = SelectionState::default();
        state.select_single(5);
        state.toggle_select(10);
        state.clear_selection();

        assert!(state.selected_functions.is_empty());
        assert_eq!(state.last_selected, None);
        assert_eq!(state.focus_index, None);
    }

    #[test]
    fn test_primary_selection() {
        let mut state = SelectionState::default();
        assert_eq!(state.primary_selection(), None);

        state.select_single(5);
        assert_eq!(state.primary_selection(), Some(5));

        state.toggle_select(10);
        assert_eq!(state.primary_selection(), Some(10));
    }
}
