use ratatui::layout::{Position, Rect};

/// Generic utility for tracking clickable screen regions.
///
/// Stores `(Rect, T)` pairs during `render()`, then hit-tests them in `handle_event()`.
/// Replaces the 4-line bounds-check pattern that was copy-pasted across screens.
#[derive(Debug, Clone)]
pub struct MouseRegions<T> {
    regions: Vec<(Rect, T)>,
}

impl<T> MouseRegions<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Clear all stored regions (call at the start of each render).
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Register a clickable region with its associated value.
    pub fn add(&mut self, area: Rect, item: T) {
        self.regions.push((area, item));
    }

    /// Hit-test: find the first region containing the given column and row.
    #[must_use]
    pub fn hit_test(&self, col: u16, row: u16) -> Option<&T> {
        let pos = Position::new(col, row);
        self.regions
            .iter()
            .find(|(rect, _)| rect.contains(pos))
            .map(|(_, item)| item)
    }
}

impl<T> Default for MouseRegions<T> {
    fn default() -> Self {
        Self::new()
    }
}
