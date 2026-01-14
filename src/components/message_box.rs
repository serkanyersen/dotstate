use crate::styles::theme;
use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};

/// Message box component for displaying status/error/info messages
pub struct MessageBox;

impl MessageBox {
    /// Render a message box with optional title and color
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The area to render the message box in
    /// * `message` - The message text to display
    /// * `title` - Optional title (defaults to "Message")
    /// * `color` - Optional color for the border (defaults to primary for info)
    ///
    /// # Returns
    /// Result indicating success or failure
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        message: &str,
        title: Option<&str>,
        color: Option<Color>,
    ) -> Result<()> {
        let t = theme();
        let title_text = title.unwrap_or("Message");
        let border_color = color.unwrap_or(t.primary);

        // Detect if message is an error
        let is_error = message.to_lowercase().contains("error")
            || message.to_lowercase().contains("failed")
            || message.to_lowercase().contains("fail");

        let final_color = if is_error { t.error } else { border_color };

        let final_title = if is_error { "Error" } else { title_text };

        let message_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(final_title)
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(final_color))
            .padding(ratatui::widgets::Padding::new(2, 2, 2, 2));

        let message_para = Paragraph::new(message)
            .style(t.text_style())
            .wrap(Wrap { trim: true })
            .block(message_block);

        frame.render_widget(message_para, area);

        Ok(())
    }

    /// Render an error message box
    #[allow(dead_code)]
    pub fn render_error(frame: &mut Frame, area: Rect, message: &str) -> Result<()> {
        let t = theme();
        Self::render(frame, area, message, Some("Error"), Some(t.error))
    }

    /// Render a status/info message box
    #[allow(dead_code)]
    pub fn render_status(frame: &mut Frame, area: Rect, message: &str) -> Result<()> {
        let t = theme();
        Self::render(frame, area, message, Some("Status"), Some(t.primary))
    }

    /// Render a success message box
    #[allow(dead_code)]
    pub fn render_success(frame: &mut Frame, area: Rect, message: &str) -> Result<()> {
        let t = theme();
        Self::render(frame, area, message, Some("Success"), Some(t.success))
    }
}
