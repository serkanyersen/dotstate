use anyhow::Result;
use ratatui::prelude::*;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use std::path::PathBuf;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, Theme};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Common file preview component
pub struct FilePreview;

impl FilePreview {
    /// Render a file preview with syntax highlighting
    ///
    /// # Arguments
    /// * `frame` - The frame to render to
    /// * `area` - The area to render the preview in
    /// * `file_path` - Path to the file to preview
    /// * `scroll_offset` - Number of lines to skip from the top
    /// * `focused` - Whether the preview pane is focused (for border color)
    /// * `title` - Optional custom title (defaults to "Preview")
    /// * `syntax_set` - Syntax definitions for highlighting
    /// * `theme` - Theme for highlighting
    pub fn render(
        frame: &mut Frame,
        area: Rect,
        file_path: &PathBuf,
        scroll_offset: usize,
        focused: bool,
        title: Option<&str>,
        content_override: Option<&str>,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        let preview_title = title.unwrap_or("Preview");
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        // Read file content or use override
        if file_path.is_file() || content_override.is_some() {
            let content_result = if let Some(content) = content_override {
                Ok(content.to_string())
            } else {
                std::fs::read_to_string(file_path)
            };

            match content_result {
                Ok(content) => {
                    let total_lines = content.lines().count().max(1);
                    let visible_height = area.height.saturating_sub(4) as usize; // Account for borders

                    // Determine syntax
                    let syntax = if let Some(content_str) = content_override {
                        // If content override is provided, try to detect syntax from content or default to Diff if it looks like one
                        if content_str.starts_with("diff --git") || content_str.starts_with("--- a/") {
                             syntax_set.find_syntax_by_name("Diff")
                                .or_else(|| syntax_set.find_syntax_by_extension("diff"))
                                .or_else(|| syntax_set.find_syntax_by_extension("patch"))
                                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                        } else {
                            // Try to guess from file extension first if path matches
                            syntax_set
                                .find_syntax_for_file(file_path)
                                .unwrap_or(None)
                                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                        }
                    } else {
                        // Standard detection logic
                        // First check for overrides based on filename
                        let file_name = file_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");

                        if file_name.ends_with("rc") || file_name.contains("profile") || file_name == ".aliases" || file_name == ".functions" {
                            // Assume shell for *rc files, profile, aliases, functions
                            syntax_set.find_syntax_by_name("Bourne Again Shell (bash)")
                                .or_else(|| syntax_set.find_syntax_by_extension("sh"))
                                .or_else(|| syntax_set.find_syntax_for_file(file_path).unwrap_or(None))
                                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                        } else if file_name.ends_with(".conf") || file_name.ends_with(".config") {
                             // Try to find a specific syntax, otherwise fallback to INI/Shell or just rely on extension
                             syntax_set.find_syntax_for_file(file_path)
                                .unwrap_or(None)
                                .or_else(|| syntax_set.find_syntax_by_extension("ini"))
                                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                        } else if file_name.ends_with(".vim") || file_name == ".vimrc" || file_name.contains("vim") {
                            syntax_set.find_syntax_by_extension("vim")
                                .or_else(|| syntax_set.find_syntax_by_name("VimL"))
                                .or_else(|| syntax_set.find_syntax_by_name("Vim Script"))
                                .or_else(|| syntax_set.find_syntax_by_extension("lua"))
                                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                        } else {
                            // Standard detection
                            syntax_set
                                .find_syntax_for_file(file_path)
                                .unwrap_or(None)
                                .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
                        }
                    };

                    let mut highlighter = HighlightLines::new(syntax, theme);

                    // Skip lines up to scroll_offset efficiently
                    let mut lines_iter = LinesWithEndings::from(&content);
                    for _ in 0..scroll_offset {
                        lines_iter.next();
                    }

                    // Process only visible lines
                    let mut preview_lines = Vec::new();
                    for line in lines_iter.take(visible_height) {
                        // Highlight the line
                        let ranges: Vec<(SyntectStyle, &str)> =
                            highlighter.highlight_line(line, syntax_set).unwrap_or_default();

                        // Convert to Ratatui spans
                        let spans: Vec<Span> = ranges
                            .into_iter()
                            .map(|(style, text)| {
                                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                                Span::styled(text.to_string(), Style::default().fg(fg))
                            })
                            .collect();
                        preview_lines.push(Line::from(spans));
                    }

                    // Create text with lines
                    let mut preview_text = Text::from(preview_lines);

                    // Add footer info if there are more lines
                    let end_line = (scroll_offset + visible_height).min(total_lines);
                    if total_lines > end_line {
                        preview_text.extend([
                            Line::from(""),
                            Line::from(""),
                            Line::from(format!(
                                "... ({} total lines, showing lines {}-{})",
                                total_lines,
                                scroll_offset + 1,
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
                    let mut scrollbar_state = ScrollbarState::new(total_lines)
                        .position(scroll_offset);

                    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓"))
                        .track_symbol(Some("│"))
                        .thumb_symbol("█");

                    frame.render_stateful_widget(
                        scrollbar,
                        area,
                        &mut scrollbar_state,
                    );
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

        Ok(())
    }
}
