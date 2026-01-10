//! Help Overlay Component
//!
//! Displays current keybindings when user presses '?' key.

use crate::keymap::Keymap;
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

        // Calculate centered popup area (90% width, 90% height for better visibility)
        let popup_width = (area.width as f32 * 0.90).min(100.0) as u16;
        let popup_height = (area.height as f32 * 0.90).min(50.0) as u16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        // Create the block
        let title = format!(" Keyboard Shortcuts - {} Preset ", keymap.preset.name());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(theme.primary));

        let inner_area = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        // Layout: preset selector + bindings list + footer
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4), // Preset selector
                Constraint::Min(5),    // Bindings
                Constraint::Length(3), // Footer
            ])
            .split(inner_area);

        // Render preset selector
        let current_preset = keymap.preset;
        let preset_lines = vec![
            Line::from(vec![
                Span::styled("Current Preset: ", Style::default().fg(theme.text)),
                Span::styled(
                    current_preset.name(),
                    Style::default()
                        .fg(theme.primary)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Switch preset: ", Style::default().fg(theme.text_muted)),
                Span::styled(
                    if current_preset == crate::keymap::KeymapPreset::Standard {
                        "[Standard]"
                    } else {
                        "Standard"
                    },
                    Style::default()
                        .fg(if current_preset == crate::keymap::KeymapPreset::Standard {
                            theme.primary
                        } else {
                            theme.text_muted
                        })
                        .add_modifier(if current_preset == crate::keymap::KeymapPreset::Standard {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw(" "),
                Span::styled(
                    if current_preset == crate::keymap::KeymapPreset::Vim {
                        "[Vim]"
                    } else {
                        "Vim"
                    },
                    Style::default()
                        .fg(if current_preset == crate::keymap::KeymapPreset::Vim {
                            theme.primary
                        } else {
                            theme.text_muted
                        })
                        .add_modifier(if current_preset == crate::keymap::KeymapPreset::Vim {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
                Span::raw(" "),
                Span::styled(
                    if current_preset == crate::keymap::KeymapPreset::Emacs {
                        "[Emacs]"
                    } else {
                        "Emacs"
                    },
                    Style::default()
                        .fg(if current_preset == crate::keymap::KeymapPreset::Emacs {
                            theme.primary
                        } else {
                            theme.text_muted
                        })
                        .add_modifier(if current_preset == crate::keymap::KeymapPreset::Emacs {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Press ", Style::default().fg(theme.text_muted)),
                Span::styled("1", Style::default().fg(theme.text_emphasis)),
                Span::styled(" / ", Style::default().fg(theme.text_muted)),
                Span::styled("2", Style::default().fg(theme.text_emphasis)),
                Span::styled(" / ", Style::default().fg(theme.text_muted)),
                Span::styled("3", Style::default().fg(theme.text_emphasis)),
                Span::styled(
                    " to switch, or any other key to close",
                    Style::default().fg(theme.text_muted),
                ),
            ]),
        ];
        let preset_paragraph = Paragraph::new(preset_lines)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        frame.render_widget(preset_paragraph, chunks[0]);

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
        frame.render_widget(bindings_paragraph, chunks[1]);

        // Footer with config location
        let footer_text = format!(
            "Edit keybindings in: {}\nPress 1/2/3 to switch preset, any other key to close",
            config_path
        );
        let footer = Paragraph::new(footer_text)
            .style(Style::default().fg(theme.text_muted))
            .alignment(Alignment::Center);
        frame.render_widget(footer, chunks[2]);

        Ok(())
    }
}
