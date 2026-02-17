//! Multi-select checkbox field.
//!
//! Presents a list of options where zero or more can be selected.
//! Implements [`FormField`].

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::form::field::{FieldValue, FormField};
use crate::theme::theme;

/// A multi-select checkbox list.
///
/// # Example
///
/// ```rust,ignore
/// use tui_forge::form::fields::Checkbox;
///
/// let cb = Checkbox::new(vec!["Rust", "Go", "Python"]);
/// ```
#[derive(Debug, Clone)]
pub struct Checkbox {
    options: Vec<String>,
    selected: Vec<bool>,
    cursor: usize,
}

impl Checkbox {
    // ----- construction ---------------------------------------------------

    /// Create a new checkbox with the given options (none selected).
    pub fn new(options: Vec<impl Into<String>>) -> Self {
        let options: Vec<String> = options.into_iter().map(Into::into).collect();
        let len = options.len();
        Self {
            options,
            selected: vec![false; len],
            cursor: 0,
        }
    }

    // ----- typed API ------------------------------------------------------

    /// Indices of all selected options.
    pub fn selected_indices(&self) -> Vec<usize> {
        self.selected
            .iter()
            .enumerate()
            .filter_map(|(i, &s)| if s { Some(i) } else { None })
            .collect()
    }

    /// Labels of all selected options.
    pub fn selected_labels(&self) -> Vec<&str> {
        self.selected
            .iter()
            .enumerate()
            .filter_map(|(i, &s)| if s { Some(self.options[i].as_str()) } else { None })
            .collect()
    }

    /// Whether a specific index is checked.
    pub fn is_checked(&self, index: usize) -> bool {
        self.selected.get(index).copied().unwrap_or(false)
    }

    /// Programmatically set the checked state of an option.
    pub fn set_checked(&mut self, index: usize, checked: bool) {
        if let Some(slot) = self.selected.get_mut(index) {
            *slot = checked;
        }
    }

    /// Toggle the checked state of a specific option.
    pub fn toggle(&mut self, index: usize) {
        if let Some(slot) = self.selected.get_mut(index) {
            *slot = !*slot;
        }
    }

    /// Check all options.
    pub fn check_all(&mut self) {
        self.selected.iter_mut().for_each(|s| *s = true);
    }

    /// Uncheck all options.
    pub fn uncheck_all(&mut self) {
        self.selected.iter_mut().for_each(|s| *s = false);
    }
}

// ---------------------------------------------------------------------------
// FormField implementation
// ---------------------------------------------------------------------------

impl FormField for Checkbox {
    fn field_value(&self) -> FieldValue {
        FieldValue::Choices(self.selected_indices())
    }

    fn set_field_value(&mut self, value: FieldValue) {
        if let FieldValue::Choices(indices) = value {
            self.uncheck_all();
            for i in indices {
                self.set_checked(i, true);
            }
        }
    }

    fn clear(&mut self) {
        self.uncheck_all();
        self.cursor = 0;
    }

    fn captures_text(&self) -> bool {
        false
    }

    fn handle_key_event(&mut self, event: &KeyEvent) -> bool {
        if self.options.is_empty() {
            return false;
        }
        match event.code {
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.options.len() {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Char(' ') => {
                self.toggle(self.cursor);
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let t = theme();

        let lines: Vec<Line<'_>> = self
            .options
            .iter()
            .enumerate()
            .map(|(i, label)| {
                let checked = self.selected.get(i).copied().unwrap_or(false);
                let marker = if checked { "[*]" } else { "[ ]" };
                let text = format!("{marker} {label}");

                let style = if focused && i == self.cursor {
                    t.highlight_style()
                } else {
                    t.text_style()
                };

                Line::from(Span::styled(text, style))
            })
            .collect();

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, area);
    }

    fn height(&self) -> u16 {
        self.options.len() as u16
    }
}
