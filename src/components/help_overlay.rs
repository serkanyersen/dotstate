//! Help Overlay Component
//!
//! Displays current keybindings when user presses '?' key.

use crate::keymap::{Action, Keymap};
use crate::styles::theme;
use anyhow::Result;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Renders the help overlay showing current keybindings
pub struct HelpOverlay;

impl HelpOverlay {
    /// Render the help overlay in the center of the screen
    pub fn render(frame: &mut Frame, area: Rect, keymap: &Keymap, config_path: &str) -> Result<()> {
        let theme = theme();

        // Calculate centered popup area (70% width, 80% height)
        let popup_width = (area.width as f32 * 0.70).min(80.0) as u16;
        let popup_height = (area.height as f32 * 0.80).min(40.0) as u16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        // Create the block
        let title = format!(" Keyboard Shortcuts ({}) ", keymap.preset.name());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(theme.primary));

        let inner_area = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        // Layout: bindings list + footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),    // Bindings
                Constraint::Length(3), // Footer
            ])
            .split(inner_area);

        // Group bindings by category
        let bindings = keymap.all_bindings();
        let mut lines: Vec<Line> = Vec::new();

        // Add header
        lines.push(Line::from(""));

        // Group by category
        let mut current_category = "";
        for binding in &bindings {
            let category = binding.action.category();
            if category != current_category {
                if !current_category.is_empty() {
                    lines.push(Line::from("")); // Blank line between categories
                }
                lines.push(Line::from(vec![Span::styled(
                    format!("  {} ", category),
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )]));
                current_category = category;
            }

            // Format: "    key      description"
            let key_display = binding.display();
            let description = binding.get_description();
            lines.push(Line::from(vec![
                Span::styled(
                    format!("    {:12}", key_display),
                    Style::default().fg(theme.text_emphasis),
                ),
                Span::raw(description),
            ]));
        }

        let bindings_paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left);
        frame.render_widget(bindings_paragraph, chunks[0]);

        // Footer with config location
        let footer_text = format!(
            "Edit keybindings in: {}\nPress any key to close",
            config_path
        );
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(theme.text_muted))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[1]);

        Ok(())
    }
}
