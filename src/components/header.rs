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
    /// * `title` - The title text (e.g., "dotzz - Main Menu")
    /// * `description` - The description text
    ///
    /// # Returns
    /// The height of the header (for layout calculations)
    pub fn render(frame: &mut Frame, area: Rect, title: &str, description: &str) -> Result<u16, anyhow::Error> {
        // Header block with cyan border
        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        // Split header area for title and description
        // Use a 3-line layout: 1 for title, 2 for description (with vertical centering)
        let header_inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title
                Constraint::Min(0),    // Flexible space for description
            ])
            .split(area);

        // For description, create a centered layout within its area
        let desc_area = header_inner[1];
        let desc_inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Flexible top padding
                Constraint::Length(2), // Description text (2 lines)
                Constraint::Min(0),    // Flexible bottom padding
            ])
            .split(desc_area);

        // Title paragraph
        let title_para = Paragraph::new(title)
            .style(Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        // Description paragraph - white color, centered horizontally and vertically
        let description_para = Paragraph::new(description)
            .style(Style::default().fg(Color::White)) // White color
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);

        // Render the bordered block first
        frame.render_widget(title_block, area);

        // Render title and description inside the block
        frame.render_widget(title_para, header_inner[0]);
        frame.render_widget(description_para, desc_inner[1]);

        // Return the height of the header
        // Actual calculation: Block with borders (2) + padding top (1) + title (1) + description area (needs 2 for text) + padding bottom (1) = 7
        // But we use the area height that was allocated, which should be 6 based on our constraint
        // The block padding and borders are included in the area, so the content fits within
        Ok(area.height)
    }
}

