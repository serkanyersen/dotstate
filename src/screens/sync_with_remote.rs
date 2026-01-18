//! Sync with remote screen controller.
//!
//! This screen handles syncing changes with the remote repository (push/pull).

use crate::components::file_preview::FilePreview;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{theme as ui_theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::{Screen as ScreenId, SyncWithRemoteState};
use crate::utils::{create_split_layout, create_standard_layout, focused_border_style};
use anyhow::Result;
use crossterm::event::Event;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    StatefulWidget, Wrap,
};
use ratatui::Frame;

/// Sync with remote screen controller.
///
/// This screen handles reviewing and syncing changes with the remote repository.
/// It owns its state and handles both rendering and event handling.
pub struct SyncWithRemoteScreen {
    /// Screen state
    pub state: SyncWithRemoteState,
}

impl SyncWithRemoteScreen {
    /// Create a new sync with remote screen.
    pub fn new() -> Self {
        Self {
            state: SyncWithRemoteState::default(),
        }
    }

    /// Get a reference to the state.
    pub fn get_state(&self) -> &SyncWithRemoteState {
        &self.state
    }

    /// Get a mutable reference to the state.
    pub fn get_state_mut(&mut self) -> &mut SyncWithRemoteState {
        &mut self.state
    }

    /// Reset state to default.
    pub fn reset_state(&mut self) {
        self.state = SyncWithRemoteState::default();
    }

    /// Load changed files from git repository
    pub fn load_changed_files(&mut self, ctx: &ScreenContext) {
        use crate::services::GitService;
        self.state.changed_files = GitService::load_changed_files(&ctx.config.repo_path);
        // Select first item if list is not empty
        if !self.state.changed_files.is_empty() {
            self.state.list_state.select(Some(0));
            self.update_diff_preview(ctx);
        }
    }

    /// Update the diff preview based on the selected file
    fn update_diff_preview(&mut self, ctx: &ScreenContext) {
        use crate::services::GitService;
        self.state.diff_content = None;

        let selected_idx = match self.state.list_state.selected() {
            Some(idx) => idx,
            None => return,
        };

        if selected_idx >= self.state.changed_files.len() {
            return;
        }

        let file_info = &self.state.changed_files[selected_idx];
        if let Some(diff) = GitService::get_diff_for_file(&ctx.config.repo_path, file_info) {
            self.state.diff_content = Some(diff);
            self.state.preview_scroll = 0;
        }
    }

    /// Start syncing changes (push/pull)
    fn start_sync(&mut self, ctx: &ScreenContext) -> Result<()> {
        use crate::services::GitService;
        use tracing::info;

        info!("Starting sync operation");

        // Mark as syncing
        self.state.is_syncing = true;
        self.state.sync_progress = Some("Syncing...".to_string());

        // Perform sync using service
        let result = GitService::sync(ctx.config);

        // Update state with result
        self.state.is_syncing = false;
        self.state.sync_progress = None;
        self.state.sync_result = Some(result.message);
        self.state.pulled_changes_count = result.pulled_count;
        self.state.show_result_popup = true;

        Ok(())
    }

    /// Render the result popup
    fn render_result_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &crate::config::Config,
    ) -> Result<()> {
        use crate::components::dialog::{Dialog, DialogVariant};

        let result_text = self
            .state
            .sync_result
            .as_deref()
            .unwrap_or("Unknown result")
            .to_string();

        let is_error = result_text.to_lowercase().contains("error")
            || result_text.to_lowercase().contains("failed");

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!("{}: Close", k(crate::keymap::Action::Confirm));

        let dialog = Dialog::new(
            if is_error {
                "Sync Error"
            } else {
                "Sync Result"
            },
            &result_text,
        )
        .height(50)
        .variant(if is_error {
            DialogVariant::Error
        } else {
            DialogVariant::Default
        })
        .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }

    /// Render the syncing progress indicator
    fn render_progress(&self, frame: &mut Frame, content_chunk: Rect) {
        let t = ui_theme();
        let progress_text = self
            .state
            .sync_progress
            .as_deref()
            .unwrap_or("Processing...");
        let progress_para = Paragraph::new(progress_text)
            .style(Style::default().fg(t.warning))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ui_theme().border_type(false))
                    .title(" Progress ")
                    .title_alignment(Alignment::Center)
                    .border_style(focused_border_style())
                    .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)),
            );
        frame.render_widget(progress_para, content_chunk);
    }

    /// Render the changed files list and diff preview
    fn render_changes_list(
        &mut self,
        frame: &mut Frame,
        content_chunk: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let t = ui_theme();

        let has_remote_changes = if let Some(status) = &self.state.git_status {
            status.ahead > 0 || status.behind > 0
        } else {
            false
        };

        if self.state.changed_files.is_empty() && !has_remote_changes {
            let empty_message = Paragraph::new(
                "No changes to sync.\n\nAll files are up to date with the remote repository.",
            )
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(ui_theme().border_type(false))
                    .title(" No Changes ")
                    .title_alignment(Alignment::Center)
                    .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)),
            );
            frame.render_widget(empty_message, content_chunk);
            return Ok(());
        }

        // If we have remote changes but no local file changes, show status summary
        if self.state.changed_files.is_empty() && has_remote_changes {
            let status = self.state.git_status.as_ref().unwrap();
            let mut msg = String::from("Ready to Sync:\n\n");
            if status.behind > 0 {
                msg.push_str(&format!(
                    "• {} commit(s) behind remote (will pull)\n",
                    status.behind
                ));
            }
            if status.ahead > 0 {
                msg.push_str(&format!(
                    "• {} commit(s) ahead of remote (will push)\n",
                    status.ahead
                ));
            }

            let status_para = Paragraph::new(msg)
                .style(t.text_style())
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(ui_theme().border_type(false))
                        .title(" Sync Status ")
                        .title_alignment(Alignment::Center)
                        .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)),
                );
            frame.render_widget(status_para, content_chunk);
            return Ok(());
        }

        // Split content into List (Left) and Preview (Right)
        let chunks = create_split_layout(content_chunk, &[50, 50]);
        let list_area = chunks[0];
        let preview_area = chunks[1];

        // Update scrollbar state
        let total_items = self.state.changed_files.len();
        let selected_index = self.state.list_state.selected().unwrap_or(0);
        self.state.scrollbar_state = self
            .state
            .scrollbar_state
            .content_length(total_items)
            .position(selected_index);

        let items: Vec<ListItem> = self
            .state
            .changed_files
            .iter()
            .map(|file| {
                let style = if file.starts_with("A ") {
                    Style::default().fg(t.success) // Added
                } else if file.starts_with("M ") {
                    Style::default().fg(t.warning) // Modified
                } else if file.starts_with("D ") {
                    Style::default().fg(t.error) // Deleted
                } else {
                    t.text_style()
                };
                ListItem::new(file.as_str()).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(focused_border_style())
                    .border_type(ui_theme().border_type(false))
                    .title(format!(
                        " Changed Files ({}) ",
                        self.state.changed_files.len()
                    ))
                    .title_alignment(Alignment::Center)
                    .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
            )
            .highlight_style(t.highlight_style())
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        StatefulWidget::render(
            list,
            list_area,
            frame.buffer_mut(),
            &mut self.state.list_state,
        );

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_area,
            &mut self.state.scrollbar_state,
        );

        // Render Preview
        if let Some(selected_idx) = self.state.list_state.selected() {
            if selected_idx < self.state.changed_files.len() {
                let file_info = &self.state.changed_files[selected_idx];
                // format is "X filename"
                let parts: Vec<&str> = file_info.splitn(2, ' ').collect();
                if parts.len() == 2 {
                    let path_str = parts[1].trim();
                    let path = std::path::PathBuf::from(path_str);
                    let preview_title = format!("Diff: {}", path_str);

                    FilePreview::render(
                        frame,
                        preview_area,
                        &path,
                        self.state.preview_scroll,
                        false, // Not focused for now
                        Some(&preview_title),
                        self.state.diff_content.as_deref(),
                        ctx.syntax_set,
                        ctx.syntax_theme,
                    )?;
                }
            }
        } else {
            let empty_preview = Paragraph::new("Select a file to view changes")
                .block(Block::default().borders(Borders::ALL).title(" Preview "));
            frame.render_widget(empty_preview, preview_area);
        }

        Ok(())
    }
}

impl Default for SyncWithRemoteScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for SyncWithRemoteScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let t = ui_theme();
        let background = Block::default().style(t.background_style());
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let description = if self.state.is_syncing {
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

        // Always render main content first
        if self.state.is_syncing {
            self.render_progress(frame, content_chunk);
        } else {
            self.render_changes_list(frame, content_chunk, ctx)?;
        }

        // Render popups on top of the content (not instead of it)
        if self.state.show_result_popup {
            self.render_result_popup(frame, area, ctx.config)?;
        }

        // Footer
        let k = |a| ctx.config.keymap.get_key_display_for_action(a);

        let has_remote_changes = if let Some(status) = &self.state.git_status {
            status.ahead > 0 || status.behind > 0
        } else {
            false
        };
        let can_sync = !self.state.changed_files.is_empty() || has_remote_changes;

        let footer_text = if self.state.show_result_popup {
            "Press any key or click to close".to_string()
        } else if self.state.is_syncing {
            "Syncing with remote...".to_string()
        } else if !can_sync {
            format!("{}: Back to Main Menu", k(crate::keymap::Action::Cancel))
        } else {
            format!(
                "{}: Sync with Remote | {}: Navigate | {}: Back",
                k(crate::keymap::Action::Confirm),
                ctx.config.keymap.navigation_display(),
                k(crate::keymap::Action::Cancel)
            )
        };
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        use crate::keymap::Action;
        use crossterm::event::{KeyEventKind, MouseButton, MouseEventKind};

        // Handle keyboard events
        if let Event::Key(key) = &event {
            if key.kind == KeyEventKind::Press {
                if let Some(action) = ctx.config.keymap.get_action(key.code, key.modifiers) {
                    match action {
                        Action::Confirm => {
                            // Start pushing if not already pushing and we have changes (local or remote)
                            let has_remote_changes = if let Some(status) = &self.state.git_status {
                                status.ahead > 0 || status.behind > 0
                            } else {
                                false
                            };

                            if !self.state.is_syncing
                                && (!self.state.changed_files.is_empty() || has_remote_changes)
                            {
                                self.start_sync(ctx)?;
                            }
                            return Ok(ScreenAction::None);
                        }
                        Action::Quit | Action::Cancel => {
                            // Close result popup or go back
                            if self.state.show_result_popup {
                                self.state.show_result_popup = false;
                                self.state.sync_result = None;
                                self.state.pulled_changes_count = None;
                                return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                            } else {
                                return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                            }
                        }
                        Action::MoveUp => {
                            self.state.list_state.select_previous();
                            self.update_diff_preview(ctx);
                            return Ok(ScreenAction::None);
                        }
                        Action::MoveDown => {
                            self.state.list_state.select_next();
                            self.update_diff_preview(ctx);
                            return Ok(ScreenAction::None);
                        }
                        Action::ScrollUp => {
                            self.state.preview_scroll = self.state.preview_scroll.saturating_sub(1);
                            return Ok(ScreenAction::None);
                        }
                        Action::ScrollDown => {
                            self.state.preview_scroll += 1;
                            return Ok(ScreenAction::None);
                        }
                        Action::PageUp => {
                            if let Some(current) = self.state.list_state.selected() {
                                let new_index = current.saturating_sub(10);
                                self.state.list_state.select(Some(new_index));
                                self.update_diff_preview(ctx);
                            }
                            return Ok(ScreenAction::None);
                        }
                        Action::PageDown => {
                            if let Some(current) = self.state.list_state.selected() {
                                let new_index = (current + 10)
                                    .min(self.state.changed_files.len().saturating_sub(1));
                                self.state.list_state.select(Some(new_index));
                                self.update_diff_preview(ctx);
                            }
                            return Ok(ScreenAction::None);
                        }
                        Action::GoToTop => {
                            self.state.list_state.select_first();
                            self.update_diff_preview(ctx);
                            return Ok(ScreenAction::None);
                        }
                        Action::GoToEnd => {
                            self.state.list_state.select_last();
                            self.update_diff_preview(ctx);
                            return Ok(ScreenAction::None);
                        }
                        _ => {}
                    }
                }
            }
        } else if let Event::Mouse(mouse) = event {
            // Handle mouse events for list navigation
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    self.state.list_state.select_previous();
                    self.update_diff_preview(ctx);
                    return Ok(ScreenAction::None);
                }
                MouseEventKind::ScrollDown => {
                    self.state.list_state.select_next();
                    self.update_diff_preview(ctx);
                    return Ok(ScreenAction::None);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    // Click to sync or close popup
                    if self.state.show_result_popup {
                        // After sync, go directly to main menu
                        self.state.show_result_popup = false;
                        self.state.sync_result = None;
                        self.state.pulled_changes_count = None;
                        return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                    }
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        false // No text inputs on this screen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_with_remote_screen_creation() {
        let screen = SyncWithRemoteScreen::new();
        assert!(!screen.is_input_focused());
        assert!(screen.state.changed_files.is_empty());
    }

    #[test]
    fn test_reset_state() {
        let mut screen = SyncWithRemoteScreen::new();
        screen.state.is_syncing = true;
        screen.reset_state();
        assert!(!screen.state.is_syncing);
    }
}
