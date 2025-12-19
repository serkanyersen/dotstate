use anyhow::Result;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::text::{Line, Text};
use std::path::PathBuf;

/// Common file preview component
pub struct FilePreview;

impl FilePreview {
    /// Render a file preview with proper whitespace handling
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
                            Line::from(format!("... ({} total lines, showing lines {}-{})",
                                total_lines,
                                start_line + 1,
                                end_line
                            ))
                        ]);
                    }

                    let preview = Paragraph::new(preview_text)
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .title(preview_title)
                            .title_alignment(Alignment::Center)
                            .border_style(border_style))
                        .wrap(Wrap { trim: false }); // Don't trim whitespace

                    frame.render_widget(preview, area);
                }
                Err(_) => {
                    let error_text = format!("Unable to read file: {:?}", file_path);
                    let preview = Paragraph::new(error_text)
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .title(preview_title)
                            .title_alignment(Alignment::Center)
                            .border_style(border_style));
                    frame.render_widget(preview, area);
                }
            }
        } else if file_path.is_dir() {
            let dir_text = format!("Directory: {:?}\n\nPress Enter to open", file_path);
            let preview = Paragraph::new(dir_text)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(preview_title)
                    .title_alignment(Alignment::Center)
                    .border_style(border_style));
            frame.render_widget(preview, area);
        } else {
            let path_text = format!("Path: {:?}", file_path);
            let preview = Paragraph::new(path_text)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(preview_title)
                    .title_alignment(Alignment::Center)
                    .border_style(border_style));
            frame.render_widget(preview, area);
        }

        Ok(()) // Return Ok since we're rendering directly
    }
}

