use crate::theme::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

/// Footer component with builder pattern.
///
/// Automatically parses pipe-separated key hints and applies
/// theme-aware styling (e.g. `"Navigate: arrows | Quit: q"`).
///
/// # Example
/// ```ignore
/// frame.render_widget(
///     Footer::new("Navigate: arrows | Quit: q"),
///     footer_area,
/// );
/// ```
pub struct Footer<'a> {
    text: &'a str,
}

impl<'a> Footer<'a> {
    /// Create a new footer with the given key-hint text.
    ///
    /// Text is parsed as pipe-separated segments. Each segment is split
    /// on `": "` so that labels and keys get distinct styles.
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }
}

impl Widget for Footer<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = theme();

        // Parse footer text and add colors to key hints
        let parts: Vec<&str> = self.text.split(" | ").collect();
        let mut spans = Vec::new();

        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" | ", t.muted_style()));
            }

            // Split on ": " to separate label from keys
            if let Some((label, keys)) = part.split_once(": ") {
                spans.push(Span::styled(format!("{label}: "), t.title_style()));
                spans.push(Span::styled(keys, t.emphasis_style()));
            } else {
                spans.push(Span::styled(*part, t.text_style()));
            }
        }

        let footer_block = Block::default()
            .borders(Borders::TOP)
            .border_style(t.border_focused_style())
            .border_type(t.border_type(false))
            .style(t.background_style());

        let inner_area = footer_block.inner(area);
        footer_block.render(area, buf);

        Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .render(inner_area, buf);
    }
}
