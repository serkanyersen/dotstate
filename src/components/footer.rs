use crate::styles::theme;
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
        let t = theme();
        // Parse footer text and add colors to key hints
        let parts: Vec<&str> = text.split(" | ").collect();
        let mut spans = Vec::new();

        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" | ", t.muted_style()));
            }

            // Split on ": " to separate label from keys
            if let Some((label, keys)) = part.split_once(": ") {
                spans.push(Span::styled(format!("{}: ", label), t.title_style()));
                spans.push(Span::styled(keys, t.emphasis_style()));
            } else {
                spans.push(Span::styled(*part, t.text_style()));
            }
        }

        let footer_block = Block::default()
            .borders(Borders::TOP)
            .border_style(t.border_focused_style())
            .border_type(ratatui::widgets::BorderType::Rounded)
            .style(t.background_style());

        let footer_inner = footer_block.inner(area);
        let footer = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);

        frame.render_widget(footer_block, area);
        frame.render_widget(footer, footer_inner);

        Ok(2) // Footer uses 2 lines (1 for border, 1 for text)
    }
}
