//! Text input widget for rendering TextInput instances.
//!
//! This widget provides a centralized way to render text input fields with:
//! - Consistent styling across the application
//! - Cursor positioning when focused
//! - Placeholder text support
//! - Password masking
//! - Disabled state support
//! - Customizable title and borders

use crate::utils::text_input::TextInput;
use crate::utils::{
    disabled_border_style, disabled_text_style, focused_border_style, input_placeholder_style,
    input_text_style, unfocused_border_style,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// A widget for rendering TextInput with consistent styling.
///
/// # Example
/// ```
/// use dotstate::widgets::TextInputWidget;
/// use dotstate::utils::TextInput;
/// use ratatui::widgets::Block;
///
/// let input = TextInput::with_text("hello");
/// let widget = TextInputWidget::new(&input)
///     .title("Username")
///     .placeholder("Enter username...")
///     .focused(true);
/// // frame.render_widget(widget, area);
/// ```
pub struct TextInputWidget<'a> {
    /// Reference to the text input state
    input: &'a TextInput,
    /// Title for the input field
    title: Option<&'a str>,
    /// Placeholder text when empty
    placeholder: Option<&'a str>,
    /// Whether the input is focused
    focused: bool,
    /// Whether the input is disabled
    disabled: bool,
    /// Title alignment
    title_alignment: Alignment,
    /// Whether to mask the text (for passwords)
    masked: bool,
    /// Custom block (if None, default bordered block is used)
    block: Option<Block<'a>>,
}

impl<'a> TextInputWidget<'a> {
    /// Create a new text input widget.
    pub fn new(input: &'a TextInput) -> Self {
        Self {
            input,
            title: None,
            placeholder: None,
            focused: false,
            disabled: false,
            title_alignment: Alignment::Left,
            masked: false,
            block: None,
        }
    }

    /// Set the title for the input field.
    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = Some(placeholder);
        self
    }

    /// Set whether the input is focused.
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set whether the input is disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the title alignment.
    pub fn title_alignment(mut self, alignment: Alignment) -> Self {
        self.title_alignment = alignment;
        self
    }

    /// Set whether to mask the text (for passwords).
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// Set a custom block for the input.
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Get the display text (actual text, masked text, or placeholder).
    fn display_text(&self) -> String {
        let text = self.input.text();

        if text.is_empty() {
            self.placeholder.unwrap_or("").to_string()
        } else if self.masked {
            // Mask with bullets (same length as actual text)
            "•".repeat(text.chars().count())
        } else {
            text.to_string()
        }
    }

    /// Get the text style based on state.
    fn text_style(&self) -> Style {
        if self.disabled {
            disabled_text_style()
        } else if self.input.is_empty() {
            input_placeholder_style()
        } else {
            input_text_style()
        }
    }

    /// Get the border style based on state.
    fn border_style(&self) -> Style {
        if self.disabled {
            disabled_border_style()
        } else if self.focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        }
    }

    /// Create the block for the input.
    fn create_block(&self) -> Block<'a> {
        if let Some(block) = &self.block {
            // Use custom block but override border style
            block.clone().border_style(self.border_style())
        } else {
            // Default block
            let mut block = Block::default()
                .borders(Borders::ALL)
                .border_type(crate::styles::theme().border_type(self.focused))
                .border_style(self.border_style());

            if let Some(title) = self.title {
                block = block
                    .title(format!(" {} ", title))
                    .title_alignment(self.title_alignment);
            }

            block
        }
    }
}

impl<'a> Widget for TextInputWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = self.create_block();
        let inner = block.inner(area);

        // Render the paragraph
        let paragraph = Paragraph::new(self.display_text())
            .block(block)
            .style(self.text_style());

        paragraph.render(area, buf);

        // Set cursor position if focused and not disabled
        if self.focused && !self.disabled {
            let cursor_pos = self.input.cursor();
            let clamped_cursor = cursor_pos.min(self.input.text().chars().count());
            let x = inner.x + clamped_cursor.min(inner.width as usize) as u16;
            let y = inner.y;

            // Set cursor in buffer metadata (Frame will handle actual positioning)
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_skip(false);
            }
        }
    }
}

/// Extension trait for Frame to render TextInputWidget with cursor support.
///
/// Since the Widget trait doesn't have access to Frame, we need this extension
/// to properly set the cursor position.
pub trait TextInputWidgetExt {
    /// Render a TextInputWidget and set cursor position if focused.
    fn render_text_input_widget(&mut self, widget: TextInputWidget, area: Rect);
}

impl TextInputWidgetExt for Frame<'_> {
    fn render_text_input_widget(&mut self, widget: TextInputWidget, area: Rect) {
        let focused = widget.focused;
        let disabled = widget.disabled;
        let cursor_pos = widget.input.cursor();
        let text = widget.input.text();

        // Calculate block inner area for cursor positioning
        let block = widget.create_block();
        let inner = block.inner(area);

        // Render the widget
        self.render_widget(widget, area);

        // Set cursor position if focused and not disabled
        if focused && !disabled {
            let clamped_cursor = cursor_pos.min(text.chars().count());
            let x = inner.x + clamped_cursor.min(inner.width as usize) as u16;
            let y = inner.y;
            self.set_cursor_position((x, y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_creation() {
        let input = TextInput::new();
        let widget = TextInputWidget::new(&input);
        assert!(!widget.focused);
        assert!(!widget.disabled);
        assert!(!widget.masked);
    }

    #[test]
    fn test_widget_builder() {
        let input = TextInput::with_text("test");
        let widget = TextInputWidget::new(&input)
            .title("Test Input")
            .placeholder("Enter text")
            .focused(true)
            .disabled(false)
            .masked(true);

        assert!(widget.focused);
        assert!(!widget.disabled);
        assert!(widget.masked);
        assert_eq!(widget.title, Some("Test Input"));
        assert_eq!(widget.placeholder, Some("Enter text"));
    }

    #[test]
    fn test_display_text_empty_with_placeholder() {
        let input = TextInput::new();
        let widget = TextInputWidget::new(&input).placeholder("Enter text...");
        assert_eq!(widget.display_text(), "Enter text...");
    }

    #[test]
    fn test_display_text_normal() {
        let input = TextInput::with_text("hello");
        let widget = TextInputWidget::new(&input);
        assert_eq!(widget.display_text(), "hello");
    }

    #[test]
    fn test_display_text_masked() {
        let input = TextInput::with_text("password123");
        let widget = TextInputWidget::new(&input).masked(true);
        assert_eq!(widget.display_text(), "•••••••••••");
        assert_eq!(widget.display_text().chars().count(), 11);
    }

    #[test]
    fn test_text_style_disabled() {
        let input = TextInput::with_text("test");
        let widget = TextInputWidget::new(&input).disabled(true);
        // Just ensure it doesn't panic
        let _ = widget.text_style();
    }

    #[test]
    fn test_border_style_focused() {
        let input = TextInput::new();
        let widget = TextInputWidget::new(&input).focused(true);
        // Just ensure it doesn't panic
        let _ = widget.border_style();
    }
}
