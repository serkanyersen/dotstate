//! Multi-line text area field.
//!
//! A text area manages a vector of lines with row/column cursor tracking and
//! vertical scrolling. Implements [`FormField`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::form::field::{FieldValue, FormField};
use crate::theme::theme;

/// A multi-line text editing field.
///
/// # Builder example
///
/// ```rust,ignore
/// use tui_forge::form::fields::TextArea;
///
/// let area = TextArea::new()
///     .placeholder("Enter description...")
///     .visible_rows(6)
///     .max_lines(100);
/// ```
#[derive(Debug, Clone)]
pub struct TextArea {
    lines: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
    scroll_offset: usize,
    placeholder: Option<String>,
    visible_rows: u16,
    max_lines: Option<usize>,
}

impl Default for TextArea {
    fn default() -> Self {
        Self::new()
    }
}

impl TextArea {
    // ----- construction / builder -----------------------------------------

    /// Create a new empty text area with 4 visible rows.
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_offset: 0,
            placeholder: None,
            visible_rows: 4,
            max_lines: None,
        }
    }

    /// Set placeholder text shown when empty.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    /// Set the number of visible text rows (excluding borders).
    pub fn visible_rows(mut self, rows: u16) -> Self {
        self.visible_rows = rows.max(1);
        self
    }

    /// Set the maximum number of lines allowed.
    pub fn max_lines(mut self, n: usize) -> Self {
        self.max_lines = Some(n.max(1));
        self
    }

    // ----- typed API ------------------------------------------------------

    /// Get all text joined with newlines.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Replace all text and move cursor to end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text: String = text.into();
        self.lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.split('\n').map(String::from).collect()
        };
        self.cursor_row = self.lines.len() - 1;
        self.cursor_col = self.lines[self.cursor_row].chars().count();
        self.scroll_offset = 0;
    }

    /// Number of lines.
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Whether the text area is empty (all lines are whitespace).
    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }

    // ----- internal helpers -----------------------------------------------

    /// Ensure `cursor_col` does not exceed the current line length.
    fn clamp_col(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    /// Adjust scroll so the cursor row is visible.
    fn ensure_visible(&mut self) {
        let vis = self.visible_rows as usize;
        if self.cursor_row < self.scroll_offset {
            self.scroll_offset = self.cursor_row;
        } else if self.cursor_row >= self.scroll_offset + vis {
            self.scroll_offset = self.cursor_row.saturating_sub(vis - 1);
        }
    }

    fn insert_char(&mut self, c: char) {
        if !c.is_ascii() || c.is_control() {
            return;
        }
        let line = &mut self.lines[self.cursor_row];
        let byte_index = line
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_col)
            .unwrap_or(line.len());
        line.insert(byte_index, c);
        self.cursor_col += 1;
    }

    fn insert_newline(&mut self) {
        if let Some(max) = self.max_lines {
            if self.lines.len() >= max {
                return;
            }
        }
        let line = &self.lines[self.cursor_row];
        let byte_index = line
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_col)
            .unwrap_or(line.len());
        let remainder = self.lines[self.cursor_row][byte_index..].to_string();
        self.lines[self.cursor_row].truncate(byte_index);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, remainder);
        self.cursor_col = 0;
        self.ensure_visible();
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let before: String = line.chars().take(self.cursor_col - 1).collect();
            let after: String = line.chars().skip(self.cursor_col).collect();
            *line = before + &after;
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            // Merge with previous line
            let current = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&current);
            self.ensure_visible();
        }
    }

    fn delete(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            let line = &mut self.lines[self.cursor_row];
            let before: String = line.chars().take(self.cursor_col).collect();
            let after: String = line.chars().skip(self.cursor_col + 1).collect();
            *line = before + &after;
        } else if self.cursor_row + 1 < self.lines.len() {
            // Merge next line into current
            let next = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next);
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.ensure_visible();
        }
    }

    fn move_right(&mut self) {
        let line_len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
            self.ensure_visible();
        }
    }

    fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.clamp_col();
            self.ensure_visible();
        }
    }

    fn move_down(&mut self) {
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.clamp_col();
            self.ensure_visible();
        }
    }

    fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].chars().count();
    }
}

// ---------------------------------------------------------------------------
// FormField implementation
// ---------------------------------------------------------------------------

impl FormField for TextArea {
    fn field_value(&self) -> FieldValue {
        FieldValue::Text(self.text())
    }

    fn set_field_value(&mut self, value: FieldValue) {
        if let FieldValue::Text(s) = value {
            self.set_text(s);
        }
    }

    fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
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
            KeyCode::Char('a') if ctrl => {
                self.move_home();
                true
            }
            KeyCode::Char('e') if ctrl => {
                self.move_end();
                true
            }
            KeyCode::Enter => {
                self.insert_newline();
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
            KeyCode::Up => {
                self.move_up();
                true
            }
            KeyCode::Down => {
                self.move_down();
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
        let vis = inner.height as usize;

        // Build visible line text
        let is_content_empty = self.is_empty() && self.lines.len() == 1 && self.lines[0].is_empty();

        let display: String = if is_content_empty {
            self.placeholder.as_deref().unwrap_or("").to_string()
        } else {
            self.lines
                .iter()
                .skip(self.scroll_offset)
                .take(vis)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        };

        let text_style = if is_content_empty {
            t.muted_style()
        } else {
            t.text_style()
        };

        let paragraph = Paragraph::new(display).block(block).style(text_style);

        frame.render_widget(paragraph, area);

        // Cursor
        if focused {
            let visible_row = self.cursor_row.saturating_sub(self.scroll_offset);
            if visible_row < vis {
                let col = self
                    .cursor_col
                    .min(self.lines[self.cursor_row].chars().count());
                let x = inner.x + col.min(inner.width as usize) as u16;
                let y = inner.y + visible_row as u16;
                frame.set_cursor_position((x, y));
            }
        }
    }

    fn height(&self) -> u16 {
        self.visible_rows + 2 // visible rows + top border + bottom border
    }
}
