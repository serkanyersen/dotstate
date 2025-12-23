use crate::utils::{
    disabled_border_style, disabled_text_style, focused_border_style, input_placeholder_style,
    input_text_style, unfocused_border_style,
};
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
    /// * `disabled` - Whether the input is disabled
    ///
    /// # Returns
    /// Result indicating success or failure
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        text: &str,
        cursor_pos: usize,
        focused: bool,
        title: &str,
        placeholder: Option<&str>,
        title_alignment: Alignment,
        disabled: bool,
    ) -> Result<()> {
        // Determine display text
        let display_text = if text.is_empty() {
            placeholder.unwrap_or("").to_string()
        } else {
            text.to_string()
        };

        // Style based on disabled/focus state
        let border_style = if disabled {
            disabled_border_style()
        } else if focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let text_style = if disabled {
            disabled_text_style()
        } else if text.is_empty() {
            input_placeholder_style()
        } else {
            input_text_style()
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

        // Set cursor position if focused and not disabled
        if focused && !disabled {
            let clamped_cursor = cursor_pos.min(text.chars().count());
            let x = input_inner.x + clamped_cursor.min(input_inner.width as usize) as u16;
            let y = input_inner.y;
            frame.set_cursor_position((x, y));
        }

        Ok(())
    }
}
