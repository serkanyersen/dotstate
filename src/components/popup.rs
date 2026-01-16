//! Popup widget for rendering custom content (forms, complex dialogs, etc.)
//!
//! Similar to Dialog but designed for custom content rendering rather than
//! simple text messages. Handles background dimming, clearing, positioning, and optional borders.

use crate::styles::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, BorderType, Clear};

/// Result of rendering a popup, containing areas for content and optional footer
#[derive(Debug)]
pub struct PopupRenderResult {
    /// Inner content area (inside the popup border)
    pub content_area: Rect,
    /// Footer area (below the popup, outside the border)
    pub footer_area: Rect,
}

/// Popup widget for custom content rendering
pub struct Popup<'a> {
    /// Width percentage (0-100)
    pub width_percent: u16,
    /// Height percentage (0-100)
    pub height_percent: u16,
    /// Whether to dim the background behind the popup
    pub dim_background: bool,
    /// Optional title for the border
    pub title: Option<String>,
    /// Whether to show borders (default: true)
    pub show_border: bool,
    /// Optional footer text to display below the popup
    pub footer: Option<&'a str>,
}

impl<'a> Popup<'a> {
    /// Create a new popup with default size (70% width, 50% height)
    pub fn new() -> Self {
        Self {
            width_percent: 70,
            height_percent: 50,
            dim_background: true,
            title: None,
            show_border: true,
            footer: None,
        }
    }

    /// Set the width percentage (0-100)
    pub fn width(mut self, percent: u16) -> Self {
        self.width_percent = percent;
        self
    }

    /// Set the height percentage (0-100)
    pub fn height(mut self, percent: u16) -> Self {
        self.height_percent = percent;
        self
    }

    /// Set whether to dim the background behind the popup
    pub fn dim_background(mut self, dim: bool) -> Self {
        self.dim_background = dim;
        self
    }

    /// Set an optional title for the border
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set whether to show borders (default: true)
    pub fn border(mut self, show: bool) -> Self {
        self.show_border = show;
        self
    }

    /// Set footer text to display below the popup
    pub fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Render the popup and return areas for content and optional footer
    ///
    /// This method:
    /// 1. Optionally dims the background
    /// 2. Calculates the centered popup area
    /// 3. Clears the popup area
    /// 4. Renders border if enabled
    /// 5. Returns areas for content and footer
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The parent area (usually the full terminal area)
    ///
    /// # Returns
    /// PopupRenderResult with content_area and footer_area
    pub fn render(&self, frame: &mut Frame, area: Rect) -> PopupRenderResult {
        let t = theme();

        // Calculate popup area
        let popup_width = (area.width as f32 * (self.width_percent as f32 / 100.0)) as u16;
        let popup_height = (area.height as f32 * (self.height_percent as f32 / 100.0)) as u16;
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Optionally dim the background
        if self.dim_background {
            // Dim the entire background (page content becomes darker)
            let dim = Block::default().style(Style::default().bg(Color::Reset).fg(t.text_muted));
            frame.render_widget(dim, area);
        }

        // Always clear the popup area for clean rendering
        frame.render_widget(Clear, popup_area);

        // Calculate footer area (below the popup, outside the border)
        let footer_height = if self.footer.is_some() { 1 } else { 0 };
        let footer_area = if footer_height > 0 {
            Rect::new(
                popup_area.x,
                popup_area.y + popup_area.height,
                popup_area.width,
                footer_height,
            )
        } else {
            Rect::new(0, 0, 0, 0) // Empty rect if no footer
        };

        // Render border if enabled
        let content_area = if self.show_border {
            let t = theme();
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(t.border_focused))
                .style(t.background_style());

            if let Some(ref title) = self.title {
                block = block.title(format!(" {} ", title));
            }

            let inner_area = block.inner(popup_area);
            frame.render_widget(block, popup_area);
            inner_area
        } else {
            popup_area
        };

        // Render footer if provided
        if let Some(footer_text) = self.footer {
            use ratatui::widgets::Paragraph;
            let footer = Paragraph::new(footer_text)
                .alignment(Alignment::Center)
                .style(t.text_style().add_modifier(Modifier::BOLD));
            frame.render_widget(footer, footer_area);
        }

        PopupRenderResult {
            content_area,
            footer_area,
        }
    }
}

impl<'a> Default for Popup<'a> {
    fn default() -> Self {
        Self::new()
    }
}
