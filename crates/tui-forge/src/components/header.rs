use crate::theme::theme;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

/// Header component with builder pattern.
///
/// # Example
/// ```ignore
/// frame.render_widget(
///     Header::new("My App").description("Dashboard").subtitle("v1.0"),
///     header_area,
/// );
/// ```
pub struct Header<'a> {
    title: &'a str,
    description: &'a str,
    subtitle: &'a str,
}

impl<'a> Header<'a> {
    /// Create a new header with the given title.
    #[must_use]
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            description: "",
            subtitle: "",
        }
    }

    /// Set the description text displayed inside the header.
    #[must_use]
    pub fn description(mut self, description: &'a str) -> Self {
        self.description = description;
        self
    }

    /// Set the subtitle text displayed on the bottom-right border.
    ///
    /// This can be a version string, build date, or any short text.
    #[must_use]
    pub fn subtitle(mut self, subtitle: &'a str) -> Self {
        self.subtitle = subtitle;
        self
    }

    /// Attach a widget to the left side of the header, returning a
    /// [`HeaderWithWidget`] that also implements [`Widget`].
    #[must_use]
    pub fn with_widget<W: Widget>(self, widget: W, widget_width: u16) -> HeaderWithWidget<'a, W> {
        HeaderWithWidget {
            header: self,
            widget,
            widget_width,
        }
    }
}

impl Widget for Header<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = theme();

        let mut header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_focused_style())
            .border_type(t.border_type(false))
            .title(format!(" {} ", self.title))
            .title_style(t.title_style())
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 0, 0))
            .style(t.background_style());

        if !self.subtitle.is_empty() {
            header_block = header_block.title_bottom(
                Line::from(self.subtitle.to_string())
                    .right_aligned()
                    .style(t.muted_style()),
            );
        }

        let inner_area = header_block.inner(area);
        header_block.render(area, buf);

        if !self.description.is_empty() {
            let desc_lines = self.description.lines().count() as u16;
            let top_padding = (inner_area.height.saturating_sub(desc_lines)) / 2;

            let desc_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(top_padding), Constraint::Min(0)])
                .split(inner_area);

            Paragraph::new(self.description)
                .style(t.text_style())
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true })
                .render(desc_layout[1], buf);
        }
    }
}

/// Header with an embedded widget on the left side.
///
/// Created via [`Header::with_widget`].
pub struct HeaderWithWidget<'a, W: Widget> {
    header: Header<'a>,
    widget: W,
    widget_width: u16,
}

impl<W: Widget> Widget for HeaderWithWidget<'_, W> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let t = theme();

        let mut header_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_focused_style())
            .border_type(t.border_type(false))
            .title(format!(" {} ", self.header.title))
            .title_style(t.title_style())
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 0, 0))
            .style(t.background_style());

        if !self.header.subtitle.is_empty() {
            header_block = header_block.title_bottom(
                Line::from(self.header.subtitle.to_string())
                    .right_aligned()
                    .style(t.muted_style()),
            );
        }

        let inner_area = header_block.inner(area);
        header_block.render(area, buf);

        let total_widget_width = self.widget_width + 2;
        let horizontal_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(total_widget_width),
                Constraint::Min(0),
            ])
            .split(inner_area);

        let widget_block =
            Block::default().padding(ratatui::widgets::Padding::new(0, 1, 0, 0));
        let widget_area = widget_block.inner(horizontal_chunks[0]);
        widget_block.render(horizontal_chunks[0], buf);
        self.widget.render(widget_area, buf);

        if !self.header.description.is_empty() {
            let desc_area = horizontal_chunks[1];
            let desc_lines = self.header.description.lines().count() as u16;
            let top_padding = (desc_area.height.saturating_sub(desc_lines)) / 2;

            let desc_layout = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(top_padding), Constraint::Min(0)])
                .split(desc_area);

            Paragraph::new(self.header.description)
                .style(t.text_style())
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true })
                .render(desc_layout[1], buf);
        }
    }
}
