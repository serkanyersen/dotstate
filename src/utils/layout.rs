use ratatui::prelude::*;

/// Create a standard vertical layout with header, content, and footer
///
/// # Arguments
/// * `area` - The area to split
/// * `header_height` - Height of the header (default: 6 for Header component)
/// * `footer_height` - Height of the footer (default: 2 for Footer component)
///
/// # Returns
/// Tuple of (`header_area`, `content_area`, `footer_area`)
#[must_use]
pub fn create_standard_layout(
    area: Rect,
    header_height: u16,
    footer_height: u16,
) -> (Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(0),
            Constraint::Length(footer_height),
        ])
        .split(area);

    (chunks[0], chunks[1], chunks[2])
}

/// Create a horizontal split layout with given percentages
///
/// # Arguments
/// * `area` - The area to split
/// * `percentages` - Vector of percentages for each section (must sum to 100)
///
/// # Returns
/// Vector of Rects for each section
#[must_use]
pub fn create_split_layout(area: Rect, percentages: &[u16]) -> Vec<Rect> {
    let constraints: Vec<Constraint> = percentages
        .iter()
        .map(|&p| Constraint::Percentage(p))
        .collect();

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area)
        .to_vec()
}

/// Create a centered popup area
///
/// # Arguments
/// * `area` - The parent area
/// * `width_percent` - Width percentage (0-100)
/// * `height_percent` - Height percentage (0-100)
///
/// # Returns
/// Centered Rect for the popup
#[must_use]
pub fn center_popup(area: Rect, width_percent: u16, height_percent: u16) -> Rect {
    let popup_width = (f32::from(area.width) * (f32::from(width_percent) / 100.0)) as u16;
    let popup_height = (f32::from(area.height) * (f32::from(height_percent) / 100.0)) as u16;
    let popup_x = (area.width - popup_width) / 2;
    let popup_y = (area.height - popup_height) / 2;

    Rect::new(popup_x, popup_y, popup_width, popup_height)
}
