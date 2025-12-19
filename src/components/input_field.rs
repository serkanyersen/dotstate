use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Common input field component
pub struct InputField;

impl InputField {
    /// Render an input field with cursor positioning
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The area to render the input in
    /// * `text` - The current text value
    /// * `cursor_pos` - The cursor position (in characters)
    /// * `focused` - Whether the input is focused
    /// * `title` - The title/label for the input
    /// * `placeholder` - Placeholder text when input is empty
    /// * `title_alignment` - Alignment of the title (Left or Center)
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        text: &str,
        cursor_pos: usize,
        focused: bool,
        title: &str,
        placeholder: Option<&str>,
        title_alignment: Alignment,
    ) -> Result<()> {
        // Determine display text
        let display_text = if text.is_empty() {
            placeholder.unwrap_or("").to_string()
        } else {
            text.to_string()
        };

        // Style based on focus state
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let text_style = if text.is_empty() {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::White)
        };

        // Create input block
        let input_block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(title_alignment)
            .border_style(border_style);

        let input_inner = input_block.inner(area);

        // Render input paragraph
        let input_paragraph = Paragraph::new(display_text.as_str())
            .block(input_block)
            .style(text_style);

        frame.render_widget(input_paragraph, area);

        // Set cursor position if focused
        if focused {
            let clamped_cursor = cursor_pos.min(text.chars().count());
            let x = input_inner.x + clamped_cursor.min(input_inner.width as usize) as u16;
            let y = input_inner.y;
            frame.set_cursor(x, y);
        }

        Ok(())
    }
}

