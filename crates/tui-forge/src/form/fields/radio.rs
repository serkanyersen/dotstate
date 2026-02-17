//! Single-select radio box field.
//!
//! Presents a list of options where exactly one is selected at a time.
//! Implements [`FormField`].

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::form::field::{FieldValue, FormField};
use crate::theme::theme;

/// A single-select radio button group.
///
/// # Example
///
/// ```rust,ignore
/// use tui_forge::form::fields::RadioBox;
///
/// let radio = RadioBox::new(vec!["Small", "Medium", "Large"]);
/// ```
#[derive(Debug, Clone)]
pub struct RadioBox {
    options: Vec<String>,
    selected: usize,
}

impl RadioBox {
    // ----- construction ---------------------------------------------------

    /// Create a new radio box with the given options (first selected by default).
    pub fn new(options: Vec<impl Into<String>>) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
        }
    }

    // ----- typed API ------------------------------------------------------

    /// Index of the currently selected option.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Label of the currently selected option.
    pub fn selected_label(&self) -> &str {
        self.options
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }

    /// Programmatically set the selected index.
    ///
    /// If the index is out of bounds it is clamped to the last option.
    pub fn set_selected(&mut self, index: usize) {
        if !self.options.is_empty() {
            self.selected = index.min(self.options.len() - 1);
        }
    }
}

// ---------------------------------------------------------------------------
// FormField implementation
// ---------------------------------------------------------------------------

impl FormField for RadioBox {
    fn field_value(&self) -> FieldValue {
        FieldValue::Choice(self.selected)
    }

    fn set_field_value(&mut self, value: FieldValue) {
        if let FieldValue::Choice(i) = value {
            self.set_selected(i);
        }
    }

    fn clear(&mut self) {
        self.selected = 0;
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
                if self.selected > 0 {
                    self.selected -= 1;
                }
                true
            }
            KeyCode::Down => {
                if self.selected + 1 < self.options.len() {
                    self.selected += 1;
                }
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
                let marker = if i == self.selected {
                    "(\u{25CF})" // (●)
                } else {
                    "( )"
                };
                let text = format!("{marker} {label}");

                let style = if focused && i == self.selected {
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
