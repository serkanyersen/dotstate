//! Single-line text input field.
//!
//! Ported from the `DotState` `TextInput` state machine and `TextInputWidget`
//! renderer, merged into a single self-contained struct that implements
//! [`FormField`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::form::field::{FieldValue, FormField};
use crate::theme::theme;

/// A single-line text input with cursor, placeholder, and optional max length.
///
/// # Builder example
///
/// ```rust,ignore
/// use tui_forge::form::fields::TextInput;
///
/// let input = TextInput::new()
///     .placeholder("Enter your name...")
///     .max_length(64);
/// ```
#[derive(Debug, Clone)]
pub struct TextInput {
    text: String,
    cursor: usize,
    placeholder: Option<String>,
    max_len: Option<usize>,
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl TextInput {
    // ----- construction / builder -----------------------------------------

    /// Create a new empty text input.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            placeholder: None,
            max_len: None,
        }
    }

    /// Set placeholder text shown when the field is empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set the maximum number of characters allowed.
    pub fn max_length(mut self, n: usize) -> Self {
        self.max_len = Some(n);
        self
    }

    // ----- typed API ------------------------------------------------------

    /// Get the current text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replace the text and move cursor to end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.chars().count();
    }

    /// Get the current cursor position (character offset).
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether the text is empty (after trimming whitespace).
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    // ----- internal editing helpers ---------------------------------------

    fn insert_char(&mut self, c: char) {
        if !c.is_ascii() || c.is_control() {
            return;
        }
        if let Some(max) = self.max_len {
            if self.text.chars().count() >= max {
                return;
            }
        }
        let byte_index = self
            .text
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor)
            .unwrap_or(self.text.len());
        self.text.insert(byte_index, c);
        self.cursor = (self.cursor + 1).min(self.text.chars().count());
    }

    fn backspace(&mut self) {
        if self.cursor > 0 {
            let before = self.text.chars().take(self.cursor - 1);
            let after = self.text.chars().skip(self.cursor);
            self.text = before.chain(after).collect();
            self.cursor -= 1;
        }
    }

    fn delete(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor < char_count {
            let before = self.text.chars().take(self.cursor);
            let after = self.text.chars().skip(self.cursor + 1);
            self.text = before.chain(after).collect();
        }
    }

    fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    fn move_right(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    // ----- display helpers ------------------------------------------------

    /// Text to display (real text or placeholder).
    fn display_text(&self) -> &str {
        if self.text.is_empty() {
            self.placeholder.as_deref().unwrap_or("")
        } else {
            &self.text
        }
    }
}

// ---------------------------------------------------------------------------
// FormField implementation
// ---------------------------------------------------------------------------

impl FormField for TextInput {
    fn field_value(&self) -> FieldValue {
        FieldValue::Text(self.text.clone())
    }

    fn set_field_value(&mut self, value: FieldValue) {
        if let FieldValue::Text(s) = value {
            self.set_text(s);
        }
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    fn captures_text(&self) -> bool {
        true
    }

    fn handle_key_event(&mut self, event: &KeyEvent) -> bool {
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        match event.code {
            KeyCode::Char(c) if !ctrl => {
                self.insert_char(c);
                true
            }
            // Ctrl+A  => Home
            KeyCode::Char('a') if ctrl => {
                self.move_home();
                true
            }
            // Ctrl+E  => End
            KeyCode::Char('e') if ctrl => {
                self.move_end();
                true
            }
            KeyCode::Backspace => {
                self.backspace();
                true
            }
            KeyCode::Delete => {
                self.delete();
                true
            }
            KeyCode::Left => {
                self.move_left();
                true
            }
            KeyCode::Right => {
                self.move_right();
                true
            }
            KeyCode::Home => {
                self.move_home();
                true
            }
            KeyCode::End => {
                self.move_end();
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let t = theme();

        // Border style
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

        let inner = block.inner(area);

        // Text style
        let text_style = if self.text.is_empty() {
            t.muted_style()
        } else {
            t.text_style()
        };

        let paragraph = Paragraph::new(self.display_text())
            .block(block)
            .style(text_style);

        frame.render_widget(paragraph, area);

        // Set cursor position when focused
        if focused {
            let clamped = self.cursor.min(self.text.chars().count());
            let x = inner.x + clamped.min(inner.width as usize) as u16;
            let y = inner.y;
            frame.set_cursor_position((x, y));
        }
    }

    fn height(&self) -> u16 {
        3 // border + content + border
    }
}
