use crate::styles::theme;
use crate::widgets::DotstateLogo;
use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

/// Common header component for all screens
pub struct Header;

impl Header {
    /// Render a header with title and description
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The area to render the header in
    /// * `title` - The title text (e.g., "dotstate - Main Menu")
    /// * `description` - The description text
    ///
    /// # Returns
    /// The height of the header (for layout calculations)
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        title: &str,
        description: &str,
    ) -> Result<u16, anyhow::Error> {
        let app_version = env!("CARGO_PKG_VERSION");
        let t = theme();
        // Main header block with theme border, padding, and title
        let header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_focused_style())
            .border_type(t.border_type(false))
            .title(format!(" {title} "))
            .title_style(t.title_style())
            .title_alignment(Alignment::Center)
            .title_bottom(
                Line::from(format!("v{app_version}"))
                    .right_aligned()
                    .style(t.muted_style()),
            )
            .padding(ratatui::widgets::Padding::new(1, 1, 0, 0))
            .style(t.background_style());

        // Get the inner area (inside borders and padding)
        let inner_area = header_block.inner(area);

        // Render the header block
        frame.render_widget(header_block, area);

        // Split horizontally: logo on left, description on right
        let logo = DotstateLogo::regular();
        let logo_width = logo.width() + 2; // Add padding for spacing
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(logo_width),
                Constraint::Min(0), // Rest for description
            ])
            .split(inner_area);

        // Logo area (borderless block for positioning)
        let logo_block = Block::default().padding(ratatui::widgets::Padding::new(0, 1, 0, 0));
        let logo_area = logo_block.inner(horizontal_chunks[0]);
        frame.render_widget(logo_block, horizontal_chunks[0]);
        frame.render_widget(logo, logo_area);

        // Description area (borderless, just for padding)
        let desc_area = Block::default()
            .padding(ratatui::widgets::Padding::new(0, 0, 0, 0))
            .inner(horizontal_chunks[1]);

        // Center description vertically
        let desc_lines = description.lines().count() as u16;
        let desc_height = desc_area.height;
        let top_padding = (desc_height.saturating_sub(desc_lines)) / 2;

        let desc_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(top_padding), Constraint::Min(0)])
            .split(desc_area);

        let description_para = Paragraph::new(description)
            .style(t.text_style())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        // Render description
        frame.render_widget(description_para, desc_layout[1]);

        Ok(area.height)
    }
}
