//! Masked password input field.
//!
//! Wraps [`TextInput`] and renders the text as a repeated mask character
//! (default `'●'`). Supports toggling reveal mode.

use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use super::text_input::TextInput;
use crate::form::field::{FieldValue, FormField};
use crate::theme::theme;

/// A password field that masks its content.
///
/// # Builder example
///
/// ```rust,ignore
/// use tui_forge::form::fields::Password;
///
/// let pw = Password::new()
///     .placeholder("Enter password...")
///     .mask_char('*');
/// ```
#[derive(Debug, Clone)]
pub struct Password {
    inner: TextInput,
    revealed: bool,
    mask_char: char,
}

impl Default for Password {
    fn default() -> Self {
        Self::new()
    }
}

impl Password {
    // ----- construction / builder -----------------------------------------

    /// Create a new password field.
    pub fn new() -> Self {
        Self {
            inner: TextInput::new(),
            revealed: false,
            mask_char: '\u{25CF}', // ●
        }
    }

    /// Set placeholder text.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.inner = self.inner.placeholder(text);
        self
    }

    /// Set the character used for masking (default `'●'`).
    pub fn mask_char(mut self, c: char) -> Self {
        self.mask_char = c;
        self
    }

    // ----- typed API ------------------------------------------------------

    /// Get the plain-text password.
    pub fn text(&self) -> &str {
        self.inner.text()
    }

    /// Replace the text.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.inner.set_text(text);
    }

    /// Whether the field is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Toggle between masked and revealed display.
    pub fn toggle_reveal(&mut self) {
        self.revealed = !self.revealed;
    }

    /// Whether the password is currently shown in plain text.
    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    // ----- display helpers ------------------------------------------------

    fn display_text(&self) -> String {
        let text = self.inner.text();
        if text.is_empty() {
            String::new()
        } else if self.revealed {
            text.to_string()
        } else {
            self.mask_char
                .to_string()
                .repeat(text.chars().count())
        }
    }
}

// ---------------------------------------------------------------------------
// FormField implementation
// ---------------------------------------------------------------------------

impl FormField for Password {
    fn field_value(&self) -> FieldValue {
        self.inner.field_value()
    }

    fn set_field_value(&mut self, value: FieldValue) {
        self.inner.set_field_value(value);
    }

    fn clear(&mut self) {
        self.inner.clear();
        self.revealed = false;
    }

    fn captures_text(&self) -> bool {
        true
    }

    fn handle_key_event(&mut self, event: &KeyEvent) -> bool {
        self.inner.handle_key_event(event)
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let t = theme();

        let border_style = if focused {
            t.border_focused_style()
        } else {
            t.unfocused_border_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(t.border_type(focused))
            .border_style(border_style)
            .style(t.background_style());

        let inner_area = block.inner(area);

        let display = self.display_text();
        let is_empty = self.inner.text().is_empty();

        // Show placeholder when empty, masked/revealed text otherwise
        let (shown_text, text_style) = if is_empty {
            (String::new(), t.muted_style())
        } else {
            (display, t.text_style())
        };

        let paragraph = Paragraph::new(shown_text)
            .block(block)
            .style(text_style);

        frame.render_widget(paragraph, area);

        // Set cursor when focused
        if focused {
            let cursor_pos = self.inner.cursor();
            let text_len = self.inner.text().chars().count();
            let clamped = cursor_pos.min(text_len);
            let x = inner_area.x + clamped.min(inner_area.width as usize) as u16;
            let y = inner_area.y;
            frame.set_cursor_position((x, y));
        }
    }

    fn height(&self) -> u16 {
        3
    }
}
