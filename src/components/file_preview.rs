use anyhow::Result;
use ratatui::prelude::*;
use ratatui::text::{Line, Text};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use std::path::PathBuf;

/// Common file preview component
pub struct FilePreview;

impl FilePreview {
    /// Render a file preview with proper whitespace handling and scrollbar
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The area to render the preview in
    /// * `file_path` - Path to the file to preview
    /// * `scroll_offset` - Number of lines to skip from the top
    /// * `focused` - Whether the preview pane is focused (for border color)
    /// * `title` - Optional custom title (defaults to "Preview")
    ///
    /// # Returns
    /// Result indicating success or failure
    ///
    /// # Scrollbar Tutorial
    /// A scrollbar shows the user's position in a scrollable content area.
    /// In Ratatui, scrollbars require three pieces of information:
    /// 1. **Total Content Size**: How many lines/items exist in total
    /// 2. **Visible Area Size**: How many lines/items can be seen at once
    /// 3. **Current Position**: Where in the content we're currently viewing
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        file_path: &PathBuf,
        scroll_offset: usize,
        focused: bool,
        title: Option<&str>,
    ) -> Result<()> {
        let preview_title = title.unwrap_or("Preview");
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        // Read file content and render with proper whitespace preservation
        if file_path.is_file() {
            match std::fs::read_to_string(file_path) {
                Ok(content) => {
                    // Split by newline to preserve line structure and whitespace
                    let all_lines: Vec<&str> = content.split('\n').collect();
                    let total_lines = all_lines.len();
                    let visible_height = area.height.saturating_sub(4) as usize; // Account for borders

                    // Get lines to display
                    let start_line = scroll_offset.min(total_lines.saturating_sub(1));
                    let end_line = (start_line + visible_height).min(total_lines);

                    // Convert to Line objects using raw() to preserve all whitespace
                    let preview_lines: Vec<Line> = all_lines[start_line..end_line]
                        .iter()
                        .map(|line| Line::raw(*line)) // Use raw() to preserve whitespace
                        .collect();

                    // Create text with lines
                    let mut preview_text = Text::from(preview_lines);

                    // Add footer info if there are more lines
                    if total_lines > end_line {
                        preview_text.extend([
                            Line::from(""),
                            Line::from(""),
                            Line::from(format!(
                                "... ({} total lines, showing lines {}-{})",
                                total_lines,
                                start_line + 1,
                                end_line
                            )),
                        ]);
                    }

                    let preview = Paragraph::new(preview_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title(preview_title)
                                .title_alignment(Alignment::Center)
                                .border_style(border_style),
                        )
                        .wrap(Wrap { trim: false }); // Don't trim whitespace

                    frame.render_widget(preview, area);

                    // === SCROLLBAR IMPLEMENTATION ===
                    // Now let's add a scrollbar to show the user's position in the file

                    // Step 1: Create a ScrollbarState
                    // This struct tracks the scrollbar's position and content size
                    let mut scrollbar_state = ScrollbarState::new(total_lines)
                        // `new(total_lines)` sets the total content size
                        // In our case, it's the total number of lines in the file
                        .position(scroll_offset);
                    // `.position(scroll_offset)` sets where we're currently viewing
                    // This is the line number at the top of the visible area

                    // Step 2: Create the Scrollbar widget
                    // The scrollbar is a visual indicator on the right edge
                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        // VerticalRight means the scrollbar appears on the right edge
                        // (There's also VerticalLeft, HorizontalTop, HorizontalBottom)
                        .begin_symbol(Some("↑")) // Arrow at the top
                        .end_symbol(Some("↓")) // Arrow at the bottom
                        .track_symbol(Some("│")) // The track/rail the thumb moves on
                        .thumb_symbol("█"); // The draggable part (shows current position)
                                            // Note: The thumb size is automatically calculated based on
                                            // visible_area / total_content ratio

                    // Step 3: Render the scrollbar
                    // We render it in the same `area` as the paragraph
                    // The scrollbar automatically positions itself on the right edge
                    frame.render_stateful_widget(
                        scrollbar,            // The widget to render
                        area,                 // Where to render it (same as the preview)
                        &mut scrollbar_state, // The state that controls its position
                    );
                    // Note: `render_stateful_widget` is used instead of `render_widget`
                    // because the scrollbar needs to know about the state (position, size)
                }
                Err(_) => {
                    let error_text = format!("Unable to read file: {:?}", file_path);
                    let preview = Paragraph::new(error_text).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(preview_title)
                            .title_alignment(Alignment::Center)
                            .border_style(border_style),
                    );
                    frame.render_widget(preview, area);
                }
            }
        } else if file_path.is_dir() {
            let dir_text = format!("Directory: {:?}\n\nPress Enter to open", file_path);
            let preview = Paragraph::new(dir_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(preview_title)
                    .title_alignment(Alignment::Center)
                    .border_style(border_style),
            );
            frame.render_widget(preview, area);
        } else {
            let path_text = format!("Path: {:?}", file_path);
            let preview = Paragraph::new(path_text).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(preview_title)
                    .title_alignment(Alignment::Center)
                    .border_style(border_style),
            );
            frame.render_widget(preview, area);
        }

        Ok(()) // Return Ok since we're rendering directly
    }
}
