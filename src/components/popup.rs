//! Popup widget for rendering custom content (forms, complex dialogs, etc.)
//!
//! Similar to Dialog but designed for custom content rendering rather than
//! simple text messages. Handles background dimming, clearing, positioning, and optional borders.

use crate::components::footer::Footer;
use crate::styles::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// Result of rendering a popup, containing area for content
#[derive(Debug)]
pub struct PopupRenderResult {
    /// Inner content area (inside the popup border, excluding title and footer)
    pub content_area: Rect,
}

/// Popup widget for custom content rendering
pub struct Popup<'a> {
    /// Width percentage (0-100)
    pub width_percent: u16,
    /// Height percentage (0-100)
    pub height_percent: u16,
    /// Whether to dim the background behind the popup
    pub dim_background: bool,
    /// Optional title to display at the top inside the popup
    pub title: Option<String>,
    /// Whether to show borders (default: true)
    pub show_border: bool,
    /// Optional footer text to display at the bottom inside the popup
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

    /// Set an optional title to display at the top inside the popup
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set whether to show borders (default: true)
    pub fn border(mut self, show: bool) -> Self {
        self.show_border = show;
        self
    }

    /// Set footer text to display at the bottom inside the popup
    pub fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Render the popup and return area for content
    ///
    /// This method:
    /// 1. Optionally dims the background
    /// 2. Calculates the centered popup area
    /// 3. Clears the popup area
    /// 4. Renders border if enabled
    /// 5. Renders title at the top (inside borders)
    /// 6. Renders footer at the bottom (inside borders)
    /// 7. Returns the remaining content area
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The parent area (usually the full terminal area)
    ///
    /// # Returns
    /// PopupRenderResult with content_area (excluding title and footer)
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

        // Render border if enabled
        let inner_area = if self.show_border {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(t.border_focused_type)
                .border_style(Style::default().fg(t.border_focused))
                .style(t.background_style());

            let inner = block.inner(popup_area);
            frame.render_widget(block, popup_area);
            inner
        } else {
            popup_area
        };

        // Build layout constraints for title, content, and footer
        let mut constraints = Vec::new();

        // Title takes 1 line if present
        if self.title.is_some() {
            constraints.push(Constraint::Length(1));
        }

        // Content takes remaining space
        constraints.push(Constraint::Min(0));

        // Footer takes 2 lines if present
        if self.footer.is_some() {
            constraints.push(Constraint::Length(2));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner_area);

        let mut chunk_idx = 0;

        // Render title if present
        if let Some(ref title_text) = self.title {
            let title_para = Paragraph::new(title_text.as_str())
                .alignment(Alignment::Center)
                .style(t.title_style());
            frame.render_widget(title_para, chunks[chunk_idx]);
            chunk_idx += 1;
        }

        // Content area is the middle chunk
        let content_area = chunks[chunk_idx];
        chunk_idx += 1;

        // Render footer if present
        if let Some(footer_text) = self.footer {
            let _ = Footer::render(frame, chunks[chunk_idx], footer_text);
        }

        PopupRenderResult { content_area }
    }
}

impl<'a> Default for Popup<'a> {
    fn default() -> Self {
        Self::new()
    }
}
