use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Common footer component
pub struct Footer;

impl Footer {
    /// Render a footer with the given text
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The area to render the footer in
    /// * `text` - The footer text to display
    ///
    /// # Returns
    /// The height used (2 lines: 1 for border, 1 for text)
    pub fn render(frame: &mut Frame, area: Rect, text: &str) -> Result<u16> {
        let footer_block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Black));

        let footer_inner = footer_block.inner(area);
        let footer = Paragraph::new(text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);

        frame.render_widget(footer_block, area);
        frame.render_widget(footer, footer_inner);

        Ok(2) // Footer uses 2 lines (1 for border, 1 for text)
    }
}

