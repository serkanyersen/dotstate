use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::message_box::MessageBox;
use crate::ui::SyncWithRemoteState;
use crate::utils::create_split_layout;
use crate::utils::{center_popup, create_standard_layout, focused_border_style};
use anyhow::Result;
use crossterm::event::Event;
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    StatefulWidget, Wrap,
};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

/// Push changes component - shows list of changes and allows pushing
pub struct PushChangesComponent;

impl PushChangesComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for PushChangesComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        Ok(())
    }

    fn handle_event(&mut self, _event: Event) -> Result<ComponentAction> {
        Ok(ComponentAction::None)
    }
}

impl PushChangesComponent {
    /// Render with state - shows changes list and handles push progress
    pub fn render_with_state(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &mut SyncWithRemoteState,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default().style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let description = if state.is_syncing {
            "Syncing with remote repository..."
        } else {
            "Review changes before syncing with remote"
        };
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Sync with Remote",
            description,
        )?;

        // Show result popup if push is complete
        if state.show_result_popup {
            // Make popup larger to show full error details
            let popup_area = center_popup(area, 80, 50);
            frame.render_widget(Clear, popup_area);

            let mut result_text = state
                .sync_result
                .as_deref()
                .unwrap_or("Unknown result")
                .to_string();
            // Add pulled changes count if available
            if let Some(pulled_count) = state.pulled_changes_count {
                if !result_text.contains("Pulled") {
                    if pulled_count > 0 {
                        result_text.push_str(&format!(
                            "\n\nPulled {} change(s) from remote.",
                            pulled_count
                        ));
                    } else {
                        result_text.push_str("\n\nNo changes pulled from remote.");
                    }
                }
            }
            let is_error = result_text.to_lowercase().contains("error")
                || result_text.to_lowercase().contains("failed");

            MessageBox::render(
                frame,
                popup_area,
                &result_text,
                None,
                if is_error {
                    Some(Color::Red)
                } else {
                    Some(Color::Green)
                },
            )?;
        } else if state.is_syncing {
            // Show progress message
            let progress_text = state.sync_progress.as_deref().unwrap_or("Processing...");
            let progress_para = Paragraph::new(progress_text)
                .style(Style::default().fg(Color::Yellow))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Progress")
                        .title_alignment(Alignment::Center)
                        .border_style(focused_border_style())
                        .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)),
                );
            frame.render_widget(progress_para, content_chunk);
        } else {
            // Show list of changed files
            if state.changed_files.is_empty() {
                let empty_message = Paragraph::new(
                    "No changes to sync.\n\nAll files are up to date with the remote repository.",
                )
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("No Changes")
                        .title_alignment(Alignment::Center)
                        .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)),
                );
                frame.render_widget(empty_message, content_chunk);
            } else {
                // Split content into List (Left) and Preview (Right)
                let chunks = create_split_layout(content_chunk, &[50, 50]);
                let list_area = chunks[0];
                let preview_area = chunks[1];

                // Update scrollbar state
                let total_items = state.changed_files.len();
                let selected_index = state.list_state.selected().unwrap_or(0);
                state.scrollbar_state = state
                    .scrollbar_state
                    .content_length(total_items)
                    .position(selected_index);

                let items: Vec<ListItem> = state
                    .changed_files
                    .iter()
                    .map(|file| {
                        let style = if file.starts_with("A ") {
                            Style::default().fg(Color::Green) // Added
                        } else if file.starts_with("M ") {
                            Style::default().fg(Color::Yellow) // Modified
                        } else if file.starts_with("D ") {
                            Style::default().fg(Color::Red) // Deleted
                        } else {
                            Style::default().fg(Color::White)
                        };
                        ListItem::new(file.as_str()).style(style)
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(focused_border_style())
                            .title(format!("Changed Files ({})", state.changed_files.len()))
                            .title_alignment(Alignment::Center)
                            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                    )
                    .highlight_style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED),
                    )
                    .highlight_symbol("> ");

                StatefulWidget::render(list, list_area, frame.buffer_mut(), &mut state.list_state);

                // Render scrollbar
                frame.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .begin_symbol(Some("↑"))
                        .end_symbol(Some("↓")),
                    list_area,
                    &mut state.scrollbar_state,
                );

                // Render Preview
                if let Some(selected_idx) = state.list_state.selected() {
                    if selected_idx < state.changed_files.len() {
                        let file_info = &state.changed_files[selected_idx];
                        // format is "X filename"
                        let parts: Vec<&str> = file_info.splitn(2, ' ').collect();
                        if parts.len() == 2 {
                            let path_str = parts[1].trim();
                            let path = std::path::PathBuf::from(path_str);
                            let preview_title = format!("Diff: {}", path_str);

                            use crate::components::file_preview::FilePreview;
                            FilePreview::render(
                                frame,
                                preview_area,
                                &path,
                                state.preview_scroll,
                                false, // Not focused for now (could add focus switching)
                                Some(&preview_title),
                                state.diff_content.as_deref(),
                                syntax_set,
                                theme,
                            )?;
                        }
                    }
                } else {
                    let empty_preview = Paragraph::new("Select a file to view changes")
                        .block(Block::default().borders(Borders::ALL).title("Preview"));
                    frame.render_widget(empty_preview, preview_area);
                }
            }
        }

        // Footer
        let footer_text = if state.show_result_popup {
            "Press any key or click to close"
        } else if state.is_syncing {
            "Syncing with remote..."
        } else if state.changed_files.is_empty() {
            "q/Esc: Back to Main Menu"
        } else {
            "Enter: Sync with Remote | ↑↓: Navigate | q/Esc: Back"
        };
        let _ = Footer::render(frame, footer_chunk, footer_text)?;

        Ok(())
    }
}
