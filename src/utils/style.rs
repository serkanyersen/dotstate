use crate::styles::theme;
use ratatui::prelude::*;

/// Get the border style for a focused pane
#[must_use]
pub fn focused_border_style() -> Style {
    theme().border_focused_style()
}

/// Get the border style for an unfocused pane
#[must_use]
pub fn unfocused_border_style() -> Style {
    theme().border_style(false)
}

/// Get the border style for a disabled input
#[must_use]
pub fn disabled_border_style() -> Style {
    theme().border_style(false)
}

/// Get the text style for disabled input
#[must_use]
pub fn disabled_text_style() -> Style {
    theme().disabled_style()
}

/// Get the text style for a focused input field
/// Note: Currently not used, but kept for potential future use
#[allow(dead_code)]
#[must_use]
pub fn input_focused_style() -> Style {
    theme().emphasis_style()
}

/// Get the text style for an unfocused input field
/// Note: Currently not used, but kept for potential future use
#[allow(dead_code)]
#[must_use]
pub fn input_unfocused_style() -> Style {
    theme().muted_style()
}

/// Get the text style for placeholder text
#[must_use]
pub fn input_placeholder_style() -> Style {
    theme().muted_style()
}

/// Get the text style for normal input text
#[must_use]
pub fn input_text_style() -> Style {
    theme().text_style()
}
