use ratatui::prelude::*;

/// Get the border style for a focused pane
#[allow(dead_code)]
pub fn focused_border_style() -> Style {
    Style::default().fg(Color::Cyan)
}

/// Get the border style for an unfocused pane
#[allow(dead_code)]
pub fn unfocused_border_style() -> Style {
    Style::default()
}

/// Get the text style for a focused input field
#[allow(dead_code)]
pub fn input_focused_style() -> Style {
    Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

/// Get the text style for an unfocused input field
#[allow(dead_code)]
pub fn input_unfocused_style() -> Style {
    Style::default().fg(Color::Gray)
}

/// Get the text style for placeholder text
#[allow(dead_code)]
pub fn input_placeholder_style() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Get the text style for normal input text
#[allow(dead_code)]
pub fn input_text_style() -> Style {
    Style::default().fg(Color::White)
}

