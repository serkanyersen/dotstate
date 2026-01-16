//! Dialog widget for confirmations, warnings, and errors
//!
//! Provides a self-contained widget that implements the Widget trait.
//! Handles centering, background dimming, borders, and content rendering.

use crate::styles::theme;
use ratatui::layout::Spacing;
use ratatui::prelude::*;
use ratatui::symbols::merge::MergeStrategy;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Widget, Wrap};

/// Dialog variant for different visual styles
#[derive(Debug, Clone, Copy, Default)]
pub enum DialogVariant {
    #[default]
    Default,
    Warning,
    Error,
}

impl DialogVariant {
    /// Get the prefix text for the variant
    fn prefix(&self) -> &'static str {
        match self {
            DialogVariant::Default => "",
            DialogVariant::Warning => "Warning",
            DialogVariant::Error => "Error",
        }
    }
}

/// Dialog widget - a self-contained confirmation/warning/error dialog
pub struct Dialog<'a> {
    /// Title shown in the title block
    pub title: &'a str,
    /// Content text to display
    pub content: &'a str,
    /// Width percentage (0-100), or None to auto-calculate based on content
    pub width_percent: Option<u16>,
    /// Minimum width in columns
    pub min_width: u16,
    /// Maximum width in columns
    pub max_width: u16,
    /// Height percentage (0-100)
    pub height_percent: u16,
    /// Visual variant (affects colors and title prefix)
    pub variant: DialogVariant,
    /// Whether to dim the background behind the dialog
    pub dim_background: bool,
    /// Footer text to display below the dialog (optional)
    pub footer: Option<&'a str>,
}

impl<'a> Dialog<'a> {
    /// Create a new dialog with title and content
    ///
    /// Width is automatically calculated based on content length,
    /// clamped between 50-80 columns by default.
    pub fn new(title: &'a str, content: &'a str) -> Self {
        Self {
            title,
            content,
            width_percent: None, // Auto-calculate based on content
            min_width: 60,
            max_width: 80,
            height_percent: 40,
            variant: DialogVariant::Default,
            dim_background: true,
            footer: None,
        }
    }

    /// Set the width as a percentage (0-100)
    /// This overrides auto-width calculation
    pub fn width(mut self, percent: u16) -> Self {
        self.width_percent = Some(percent);
        self
    }

    /// Set minimum width in columns (default: 60)
    pub fn min_width(mut self, columns: u16) -> Self {
        self.min_width = columns;
        self
    }

    /// Set maximum width in columns (default: 80)
    pub fn max_width(mut self, columns: u16) -> Self {
        self.max_width = columns;
        self
    }

    /// Set the height percentage (0-100)
    pub fn height(mut self, percent: u16) -> Self {
        self.height_percent = percent;
        self
    }

    /// Set the visual variant (affects border color and title prefix)
    pub fn variant(mut self, variant: DialogVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set whether to dim the background behind the dialog
    pub fn dim_background(mut self, dim: bool) -> Self {
        self.dim_background = dim;
        self
    }

    /// Set footer text to display below the dialog
    pub fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Internal rendering implementation
    fn render_impl(&self, area: Rect, buf: &mut Buffer) {
        let t = theme();

        // Build title with variant prefix first (needed for width calculation)
        let prefix = self.variant.prefix();
        let title_text = if !prefix.is_empty() {
            format!("{}: {}", prefix, self.title)
        } else {
            self.title.to_string()
        };

        // Calculate width (auto or percentage-based)
        let modal_width = if let Some(percent) = self.width_percent {
            // Use percentage-based width
            (area.width as f32 * (percent as f32 / 100.0)) as u16
        } else {
            // Auto-calculate based on content
            let title_len = title_text.len() as u16;
            let footer_len = self.footer.map(|f| f.len() as u16).unwrap_or(0);

            // Take the longest text, add padding (4 for horizontal padding * 2 sides = 8),
            // borders (2), and some breathing room (10)
            let suggested_width = title_len.max(footer_len) + 20;

            // Clamp between min and max, and don't exceed available width
            suggested_width.clamp(
                self.min_width,
                self.max_width.min(area.width.saturating_sub(4)),
            )
        };

        // Calculate minimum required height for the modal
        let has_footer = self.footer.is_some();
        let title_height = 3u16; // 2 borders + 1 text (with horizontal padding only)
        let footer_height = 3u16; // 2 borders + 1 text (with horizontal padding only)
        let min_content_height = 5u16; // Minimum content height

        // Total minimum height accounting for collapsed borders (each collapse saves 1 line)
        let min_total_height = if has_footer {
            title_height + min_content_height + footer_height - 2 // -2 for two collapsed borders
        } else {
            title_height + min_content_height - 1 // -1 for one collapsed border
        };

        // Calculate modal height
        let modal_height = (area.height as f32 * (self.height_percent as f32 / 100.0)) as u16;
        let modal_height = modal_height
            .max(min_total_height)
            .min(area.height.saturating_sub(2));

        // Center the modal
        let popup_x = area.x + (area.width.saturating_sub(modal_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(modal_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, modal_width, modal_height);
        // Optionally dim the background
        if self.dim_background {
            // Dim the entire background (page content becomes darker)
            let dim = Block::default().style(Style::default().bg(Color::Reset).fg(t.text_muted));
            Widget::render(dim, area, buf);
        }

        // Always clear the dialog area for clean rendering
        Widget::render(Clear, popup_area, buf);

        // Determine border style based on variant
        let border_style = match self.variant {
            DialogVariant::Default => Style::default().fg(t.border_focused),
            DialogVariant::Warning => Style::default().fg(t.warning),
            DialogVariant::Error => Style::default().fg(t.error),
        };

        // Create vertical layout with collapsed borders (web dialog style)
        // Three blocks: title, content, footer
        let constraints = if has_footer {
            vec![
                Constraint::Length(title_height),
                Constraint::Min(min_content_height), // Content block (flexible)
                Constraint::Length(footer_height),
            ]
        } else {
            vec![
                Constraint::Length(title_height),
                Constraint::Min(min_content_height), // Content block (flexible)
            ]
        };

        let layout = Layout::vertical(constraints)
            .spacing(Spacing::Overlap(1)) // Collapse borders
            .split(popup_area);

        let border_type = t.dialog_border_type;

        // Title block (top) - use horizontal padding only to save vertical space
        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(border_style)
            .padding(Padding::horizontal(2))
            .merge_borders(MergeStrategy::Exact)
            .style(t.background_style());

        let title_inner = title_block.inner(layout[0]);
        Widget::render(title_block, layout[0], buf);

        // Render title text as centered paragraph
        let title_para = Paragraph::new(title_text)
            .alignment(Alignment::Center)
            .style(t.text_style().add_modifier(Modifier::BOLD));
        Widget::render(title_para, title_inner, buf);

        // Content block (middle) - use horizontal padding only to maximize content space
        let content_block = Block::default()
            .borders(Borders::ALL)
            .border_type(border_type)
            .border_style(border_style)
            .padding(Padding::horizontal(2))
            .merge_borders(MergeStrategy::Exact)
            .style(t.background_style());

        let content_inner = content_block.inner(layout[1]);
        Widget::render(content_block, layout[1], buf);

        // Render content text (left-aligned, wrapped)
        let content_para = Paragraph::new(self.content)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left)
            .style(t.text_style());
        Widget::render(content_para, content_inner, buf);

        // Footer block (bottom) - optional
        if has_footer {
            let footer_block = Block::default()
                .borders(Borders::ALL)
                .border_type(border_type)
                .border_style(border_style)
                .padding(Padding::horizontal(2))
                .merge_borders(MergeStrategy::Exact)
                .style(t.background_style());

            let footer_inner = footer_block.inner(layout[2]);
            Widget::render(footer_block, layout[2], buf);

            // Render footer text
            if let Some(footer_text) = self.footer {
                let footer_para = Paragraph::new(footer_text)
                    .alignment(Alignment::Center)
                    .style(t.text_style().add_modifier(Modifier::BOLD));
                Widget::render(footer_para, footer_inner, buf);
            }
        }
    }
}

impl<'a> Widget for Dialog<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.render_impl(area, buf);
    }
}
