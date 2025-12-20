use ratatui::prelude::*;

/// Get the border style for a focused pane
pub fn focused_border_style() -> Style {
    Style::default().fg(Color::LightGreen)
}

/// Get the border style for an unfocused pane
pub fn unfocused_border_style() -> Style {
    Style::default().fg(Color::White)
}

/// Get the border style for a disabled input
pub fn disabled_border_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Get the text style for disabled input
pub fn disabled_text_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Get the text style for a focused input field
/// Note: Currently not used, but kept for potential future use
#[allow(dead_code)]
pub fn input_focused_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

/// Get the text style for an unfocused input field
/// Note: Currently not used, but kept for potential future use
#[allow(dead_code)]
pub fn input_unfocused_style() -> Style {
    Style::default().fg(Color::Gray)
}

/// Get the text style for placeholder text
pub fn input_placeholder_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Get the text style for normal input text
pub fn input_text_style() -> Style {
    Style::default().fg(Color::White)
}

