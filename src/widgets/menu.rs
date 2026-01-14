//! Custom Menu widget for rendering menu items with card-like appearance.
//!
//! This widget renders menu items as taller cards (3 lines each) with better
//! visual separation and hierarchy compared to the standard List widget.

use crate::styles::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};

/// A single menu item
#[derive(Debug, Clone)]
pub struct MenuItem {
    /// Icon to display before the text
    pub icon: String,
    /// Display text for the menu item
    pub text: String,
    /// Color for the item (when enabled)
    pub color: Color,
    /// Whether the item is enabled (can be selected)
    pub enabled: bool,
    /// Optional additional info (e.g., "5 pending")
    pub info: Option<String>,
    /// Whether to center the text
    pub centered: bool,
}

impl MenuItem {
    /// Create a new menu item
    pub fn new(icon: impl Into<String>, text: impl Into<String>, color: Color) -> Self {
        Self {
            icon: icon.into(),
            text: text.into(),
            color,
            enabled: true,
            info: None,
            centered: false,
        }
    }

    /// Set whether the item is enabled
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set additional info text
    pub fn info(mut self, info: String) -> Self {
        self.info = Some(info);
        self
    }

    /// Set whether the text should be centered
    pub fn centered(mut self, centered: bool) -> Self {
        self.centered = centered;
        self
    }
}

/// State for the Menu widget
#[derive(Debug, Default, Clone)]
pub struct MenuState {
    /// Currently selected index
    selected: Option<usize>,
}

impl MenuState {
    /// Create a new menu state
    pub fn new() -> Self {
        Self { selected: None }
    }

    /// Select an item by index
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
    }

    /// Get the currently selected index
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
}

/// Custom Menu widget that renders items as cards
#[derive(Debug, Clone)]
pub struct Menu {
    /// List of menu items to render
    items: Vec<MenuItem>,
}

impl Menu {
    /// Create a new menu with items
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self { items }
    }

    /// Calculate the clickable area for each item
    /// Returns a vector of (Rect, index) tuples
    pub fn clickable_areas(&self, area: Rect) -> Vec<(Rect, usize)> {
        let mut areas = Vec::new();
        let item_height = 3; // Each card is 3 lines tall

        for (i, _) in self.items.iter().enumerate() {
            let y = area.y + (i * item_height) as u16;
            if y < area.y + area.height {
                areas.push((Rect::new(area.x, y, area.width, item_height as u16), i));
            }
        }

        areas
    }
}

impl StatefulWidget for Menu {
    type State = MenuState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let t = theme();
        let item_height = 3; // Each card is 3 lines tall (padding + content + padding)

        for (i, item) in self.items.iter().enumerate() {
            let y = area.y + (i * item_height) as u16;

            // Stop if we've run out of space
            if y + item_height as u16 > area.y + area.height {
                break;
            }

            let is_selected = state.selected == Some(i);

            // Determine colors based on state
            let (fg_color, bg_color) = if !item.enabled {
                (t.text_muted, t.background)
            } else if is_selected {
                (item.color, t.highlight_bg)
            } else {
                (item.color, t.background)
            };

            let style = Style::default().fg(fg_color).bg(bg_color);
            let bold_style = style.add_modifier(Modifier::BOLD);

            // Line 1: Empty padding (with background color if selected)
            let border_char = if is_selected { "▌" } else { " " };
            let padding_line = Line::from(vec![
                Span::styled(
                    border_char,
                    Style::default().fg(t.border_focused).bg(bg_color),
                ),
                Span::styled(" ".repeat(area.width.saturating_sub(1) as usize), style),
            ]);
            padding_line
                .clone()
                .render(Rect::new(area.x, y, area.width, 1), buf);

            // Line 2: Content (icon + text + info)
            let mut content_spans = vec![];

            if item.centered {
                // Build content to calculate width for centering
                let mut temp_content = vec![];
                temp_content.push(format!("{} {}", item.icon, item.text));
                if let Some(ref info) = item.info {
                    temp_content.push(format!(" ({})", info));
                }
                if !item.enabled {
                    temp_content.push(" (requires setup)".to_string());
                }
                let content_text = temp_content.join("");
                let content_width = content_text.len();

                // Calculate left padding for centering
                let border_size = 2;
                let available = (area.width as usize).saturating_sub(border_size);
                let left_pad = if content_width < available {
                    (available - content_width) / 2
                } else {
                    0
                };

                // Add border and padding
                if is_selected {
                    content_spans.push(Span::styled(
                        "▌",
                        Style::default().fg(t.border_focused).bg(bg_color),
                    ));
                    content_spans.push(Span::styled(" ".repeat(left_pad + 1), style));
                } else {
                    content_spans.push(Span::styled(" ".repeat(left_pad + 2), style));
                }

                // Add content
                content_spans.push(Span::styled(format!("{} ", item.icon), bold_style));
                content_spans.push(Span::styled(
                    &item.text,
                    if is_selected { bold_style } else { style },
                ));

                if let Some(ref info) = item.info {
                    content_spans.push(Span::styled(
                        format!(" ({})", info),
                        Style::default().fg(t.text).bg(bg_color),
                    ));
                }

                if !item.enabled {
                    content_spans.push(Span::styled(
                        " (requires setup)",
                        Style::default().fg(t.text).bg(bg_color),
                    ));
                }
            } else {
                // Left-aligned (default)
                // Add left border for selected item
                if is_selected {
                    content_spans.push(Span::styled(
                        "▌",
                        Style::default().fg(t.border_focused).bg(bg_color),
                    ));
                    content_spans.push(Span::styled(" ", style)); // Small padding after border with background
                } else {
                    content_spans.push(Span::styled("  ", style)); // Left padding with background
                }

                // Icon and text
                content_spans.push(Span::styled(format!("{} ", item.icon), bold_style));
                content_spans.push(Span::styled(
                    &item.text,
                    if is_selected { bold_style } else { style },
                ));

                // Additional info if present
                if let Some(ref info) = item.info {
                    content_spans.push(Span::styled(
                        format!(" ({})", info),
                        Style::default()
                            .fg(t.text_dimmed)
                            .bg(bg_color)
                            .add_modifier(Modifier::ITALIC),
                    ));
                }

                // Disabled indicator
                if !item.enabled {
                    content_spans.push(Span::styled(
                        " (requires setup)",
                        Style::default().fg(t.text_muted).bg(bg_color),
                    ));
                }
            }

            // Pad the rest of the line with background color
            let content_line = Line::from(content_spans);
            let content_width = content_line.width();
            let mut final_spans = content_line.spans;

            // Fill the rest of the line with background, accounting for what we've already rendered
            if content_width < area.width as usize {
                final_spans.push(Span::styled(
                    " ".repeat(area.width as usize - content_width),
                    style,
                ));
            }

            Line::from(final_spans).render(Rect::new(area.x, y + 1, area.width, 1), buf);

            // Line 3: Empty padding (with background color if selected)
            padding_line
                .clone()
                .render(Rect::new(area.x, y + 2, area.width, 1), buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_item_creation() {
        let item = MenuItem::new("📁", "Test Item", Color::Cyan);
        assert_eq!(item.icon, "📁");
        assert_eq!(item.text, "Test Item");
        assert!(item.enabled);
        assert!(item.info.is_none());
    }

    #[test]
    fn test_menu_item_disabled() {
        let item = MenuItem::new("📁", "Test", Color::Cyan).enabled(false);
        assert!(!item.enabled);
    }

    #[test]
    fn test_menu_state() {
        let mut state = MenuState::new();
        assert_eq!(state.selected(), None);

        state.select(Some(2));
        assert_eq!(state.selected(), Some(2));
    }

    #[test]
    fn test_clickable_areas() {
        let items = vec![
            MenuItem::new("1", "First", Color::Cyan),
            MenuItem::new("2", "Second", Color::Green),
        ];
        let menu = Menu::new(items);
        let area = Rect::new(0, 0, 50, 10);
        let areas = menu.clickable_areas(area);

        assert_eq!(areas.len(), 2);
        assert_eq!(areas[0].1, 0); // First item index
        assert_eq!(areas[1].1, 1); // Second item index
        assert_eq!(areas[0].0.height, 3); // Each item is 3 lines tall
    }
}
