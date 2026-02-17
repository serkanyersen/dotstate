//! List navigation utilities for TUI components.
//!
//! This module provides a trait and extension methods for common list
//! navigation patterns like move up/down, page up/down, go to top/end.

use ratatui::widgets::ListState;

/// Extension trait for `ListState` that provides common navigation methods.
///
/// This trait extends `ListState` with convenient navigation methods that
/// are commonly used across different screens in the TUI.
pub trait ListStateExt {
    /// Move selection up by a specified number of items.
    /// If at the top, stays at the first item.
    fn move_up_by(&mut self, count: usize, total_items: usize);

    /// Move selection down by a specified number of items.
    /// If at the bottom, stays at the last item.
    fn move_down_by(&mut self, count: usize, total_items: usize);

    /// Move to the first item in the list.
    fn select_first_item(&mut self, total_items: usize);

    /// Move to the last item in the list.
    fn select_last_item(&mut self, total_items: usize);

    /// Page up by a specified page size.
    fn page_up(&mut self, page_size: usize, total_items: usize);

    /// Page down by a specified page size.
    fn page_down(&mut self, page_size: usize, total_items: usize);

    /// Wrap selection around when at boundaries.
    /// Moving up from first item goes to last, down from last goes to first.
    fn select_previous_wrap(&mut self, total_items: usize);

    /// Wrap selection around when at boundaries.
    /// Moving down from last item goes to first, up from first goes to last.
    fn select_next_wrap(&mut self, total_items: usize);

    /// Get the currently selected index, initializing to 0 if none selected.
    fn selected_or_first(&mut self, total_items: usize) -> Option<usize>;
}

impl ListStateExt for ListState {
    fn move_up_by(&mut self, count: usize, total_items: usize) {
        if total_items == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0);
        let new_index = current.saturating_sub(count);
        self.select(Some(new_index));
    }

    fn move_down_by(&mut self, count: usize, total_items: usize) {
        if total_items == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0);
        let new_index = (current + count).min(total_items.saturating_sub(1));
        self.select(Some(new_index));
    }

    fn select_first_item(&mut self, total_items: usize) {
        if total_items > 0 {
            self.select(Some(0));
        }
    }

    fn select_last_item(&mut self, total_items: usize) {
        if total_items > 0 {
            self.select(Some(total_items - 1));
        }
    }

    fn page_up(&mut self, page_size: usize, total_items: usize) {
        self.move_up_by(page_size, total_items);
    }

    fn page_down(&mut self, page_size: usize, total_items: usize) {
        self.move_down_by(page_size, total_items);
    }

    fn select_previous_wrap(&mut self, total_items: usize) {
        if total_items == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0);
        let new_index = if current == 0 {
            total_items - 1
        } else {
            current - 1
        };
        self.select(Some(new_index));
    }

    fn select_next_wrap(&mut self, total_items: usize) {
        if total_items == 0 {
            return;
        }
        let current = self.selected().unwrap_or(0);
        let new_index = if current >= total_items - 1 {
            0
        } else {
            current + 1
        };
        self.select(Some(new_index));
    }

    fn selected_or_first(&mut self, total_items: usize) -> Option<usize> {
        if total_items == 0 {
            return None;
        }
        if self.selected().is_none() {
            self.select(Some(0));
        }
        self.selected()
    }
}

/// Default page size for page up/down navigation.
pub const DEFAULT_PAGE_SIZE: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_up_by() {
        let mut state = ListState::default();
        state.select(Some(5));
        state.move_up_by(3, 10);
        assert_eq!(state.selected(), Some(2));

        // Test saturating at 0
        state.move_up_by(10, 10);
        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn test_move_down_by() {
        let mut state = ListState::default();
        state.select(Some(5));
        state.move_down_by(3, 10);
        assert_eq!(state.selected(), Some(8));

        // Test saturating at end
        state.move_down_by(10, 10);
        assert_eq!(state.selected(), Some(9));
    }

    #[test]
    fn test_select_first_last() {
        let mut state = ListState::default();
        state.select(Some(5));

        state.select_first_item(10);
        assert_eq!(state.selected(), Some(0));

        state.select_last_item(10);
        assert_eq!(state.selected(), Some(9));
    }

    #[test]
    fn test_wrap_navigation() {
        let mut state = ListState::default();
        state.select(Some(0));

        // Wrap from first to last
        state.select_previous_wrap(5);
        assert_eq!(state.selected(), Some(4));

        // Wrap from last to first
        state.select_next_wrap(5);
        assert_eq!(state.selected(), Some(0));
    }

    #[test]
    fn test_empty_list() {
        let mut state = ListState::default();

        state.move_up_by(1, 0);
        assert_eq!(state.selected(), None);

        state.move_down_by(1, 0);
        assert_eq!(state.selected(), None);

        state.select_first_item(0);
        assert_eq!(state.selected(), None);
    }

    #[test]
    fn test_selected_or_first() {
        let mut state = ListState::default();

        // No selection - should initialize to 0
        let idx = state.selected_or_first(5);
        assert_eq!(idx, Some(0));
        assert_eq!(state.selected(), Some(0));

        // Already selected - should return current
        state.select(Some(3));
        let idx = state.selected_or_first(5);
        assert_eq!(idx, Some(3));
    }
}
