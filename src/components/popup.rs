//! Popup widget for rendering custom content (forms, complex dialogs, etc.)
//!
//! Similar to Dialog but designed for custom content rendering rather than
//! simple text messages. Handles background dimming, clearing, positioning, and optional borders.

use crate::components::footer::Footer;
use crate::styles::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

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
    /// Minimum height in rows — popup expands to at least this many rows.
    /// If the parent area can't fit it, the popup renders a "too small" message instead.
    pub min_height: u16,
    /// Minimum width in cols — popup expands to at least this many cols.
    /// If the parent area can't fit it, the popup renders a "too small" message instead.
    pub min_width: u16,
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            width_percent: 70,
            height_percent: 50,
            // Defaults cover the smallest reasonable popup (borders + title +
            // one field + footer). Form popups should call `.min_height()` /
            // `.min_width()` with their actual layout sum.
            min_height: 8,
            min_width: 30,
            dim_background: true,
            title: None,
            show_border: true,
            footer: None,
        }
    }

    /// Set the width percentage (0-100)
    #[must_use]
    pub fn width(mut self, percent: u16) -> Self {
        self.width_percent = percent;
        self
    }

    /// Set the height percentage (0-100)
    #[must_use]
    pub fn height(mut self, percent: u16) -> Self {
        self.height_percent = percent;
        self
    }

    /// Set the minimum height in rows. The popup will be at least this tall
    /// regardless of `height_percent`. If the parent area can't fit this
    /// minimum, the popup renders a "Terminal too small" message instead.
    #[must_use]
    pub fn min_height(mut self, rows: u16) -> Self {
        self.min_height = rows;
        self
    }

    /// Set the minimum width in cols. The popup will be at least this wide
    /// regardless of `width_percent`. If the parent area can't fit this
    /// minimum, the popup renders a "Terminal too small" message instead.
    #[must_use]
    pub fn min_width(mut self, cols: u16) -> Self {
        self.min_width = cols;
        self
    }

    /// Set whether to dim the background behind the popup
    #[must_use]
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
    #[must_use]
    pub fn border(mut self, show: bool) -> Self {
        self.show_border = show;
        self
    }

    /// Set footer text to display at the bottom inside the popup
    #[must_use]
    pub fn footer(mut self, footer: &'a str) -> Self {
        self.footer = Some(footer);
        self
    }

    /// Render the popup and return area for content.
    ///
    /// Returns `None` when the parent area can't fit the popup's declared
    /// `min_width` × `min_height`. In that case a "Terminal too small"
    /// message is rendered into `area` and the caller MUST skip its own
    /// content rendering for this frame.
    ///
    /// This method:
    /// 1. Optionally dims the background
    /// 2. Calculates the centered popup area (clamped up to `min_width` /
    ///    `min_height`, and down to the parent area)
    /// 3. If too small, renders the fallback message and returns `None`
    /// 4. Clears the popup area
    /// 5. Renders border if enabled
    /// 6. Renders title at the top (inside borders)
    /// 7. Renders footer at the bottom (inside borders)
    /// 8. Returns `Some(PopupRenderResult)` with the remaining content area
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The parent area (usually the full terminal area)
    pub fn render(&self, frame: &mut Frame, area: Rect) -> Option<PopupRenderResult> {
        let t = theme();

        // If the parent can't fit our declared minimums, render the fallback
        // and bail. We don't try to partially render — half a form is worse
        // than no form.
        if area.width < self.min_width || area.height < self.min_height {
            render_too_small(frame, area, self.min_width, self.min_height);
            return None;
        }

        // Calculate popup area: percentage-of-parent, floored at the declared
        // minimum, and capped at the parent area so we never overflow.
        let popup_width = ((f32::from(area.width) * (f32::from(self.width_percent) / 100.0))
            as u16)
            .max(self.min_width)
            .min(area.width);
        let popup_height = ((f32::from(area.height) * (f32::from(self.height_percent) / 100.0))
            as u16)
            .max(self.min_height)
            .min(area.height);
        let popup_x = area.x + (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = area.y + (area.height.saturating_sub(popup_height)) / 2;
        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Optionally dim the background
        if self.dim_background {
            // Dim the entire background (page content becomes darker)
            let dim = Block::default().style(t.dim_style());
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

        Some(PopupRenderResult { content_area })
    }
}

impl Default for Popup<'_> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_with(width: u16, height: u16, build: impl FnOnce() -> Popup<'static>) -> Option<u16> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut content_height = None;
        terminal
            .draw(|frame| {
                let area = frame.area();
                let popup = build();
                content_height = popup.render(frame, area).map(|r| r.content_area.height);
            })
            .unwrap();
        content_height
    }

    #[test]
    fn renders_when_area_meets_min_size() {
        let result = render_with(60, 24, || {
            Popup::new()
                .min_height(20)
                .min_width(50)
                .title("Form")
                .footer("Enter: confirm")
        });
        assert!(
            result.is_some_and(|h| h > 0),
            "expected popup to render with content area, got {result:?}"
        );
    }

    #[test]
    fn returns_none_when_area_smaller_than_min_height() {
        // Regression for GitHub issue #53: form popups silently squished
        // when terminal too short.
        let result = render_with(80, 15, || {
            Popup::new()
                .min_height(22)
                .min_width(50)
                .title("Create Profile")
        });
        assert!(
            result.is_none(),
            "expected too-small fallback (None), got {result:?}"
        );
    }

    #[test]
    fn returns_none_when_area_smaller_than_min_width() {
        let result = render_with(40, 30, || Popup::new().min_height(20).min_width(60));
        assert!(
            result.is_none(),
            "expected too-small fallback (None), got {result:?}"
        );
    }

    #[test]
    fn percentage_height_floored_to_min() {
        // 10% of 30 = 3 rows, but min_height of 12 should win.
        let result = render_with(80, 30, || {
            Popup::new().height(10).min_height(12).min_width(40)
        });
        // content_area = popup_height (12) - borders (2) - title (1 if any) - footer (2 if any)
        assert!(
            result.is_some_and(|h| h >= 8),
            "expected content area floor honored, got {result:?}"
        );
    }
}

/// Render a centered "terminal too small" message into `area`.
fn render_too_small(frame: &mut Frame, area: Rect, min_width: u16, min_height: u16) {
    let t = theme();

    // Dim the background so the message stands out.
    frame.render_widget(Block::default().style(t.dim_style()), area);

    let msg = format!(
        "Terminal too small\n\nNeeds at least {min_width}×{min_height}\nCurrent: {}×{}",
        area.width, area.height
    );
    let para = Paragraph::new(msg)
        .alignment(Alignment::Center)
        .style(t.text_style().bg(t.background))
        .wrap(Wrap { trim: true });
    frame.render_widget(para, area);
}
