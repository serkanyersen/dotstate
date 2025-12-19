/// Calculate the X coordinate for a cursor position in text
///
/// # Arguments
/// * `text` - The text string
/// * `cursor_pos` - Cursor position in characters
/// * `area_width` - Width of the area
///
/// # Returns
/// X coordinate for the cursor
#[allow(dead_code)]
pub fn calculate_cursor_x(text: &str, cursor_pos: usize, area_width: u16) -> u16 {
    let clamped_pos = cursor_pos.min(text.chars().count());
    clamped_pos.min(area_width as usize) as u16
}

/// Clamp cursor position to valid range for text
///
/// # Arguments
/// * `text` - The text string
/// * `pos` - Desired cursor position
///
/// # Returns
/// Clamped cursor position
#[allow(dead_code)]
pub fn clamp_cursor_position(text: &str, pos: usize) -> usize {
    pos.min(text.chars().count())
}

/// Truncate text with ellipsis if it exceeds max width
///
/// # Arguments
/// * `text` - Text to truncate
/// * `max_width` - Maximum width in characters
///
/// # Returns
/// Truncated string with ellipsis if needed
#[allow(dead_code)]
pub fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
    if text.chars().count() <= max_width {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

