//! Sync with remote screen controller.
//!
//! This screen handles syncing changes with the remote repository (push/pull).
//! Note: Event handling is still partially in app.rs.

use crate::components::PushChangesComponent;
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::{Screen as ScreenId, SyncWithRemoteState};
use anyhow::Result;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

/// Sync with remote screen controller.
///
/// # Migration Status
///
/// Event handling is still in app.rs (lines ~942-1000).
/// The screen is mostly presentation - the sync logic is in GitService.
pub struct SyncWithRemoteScreen {
    component: PushChangesComponent,
    /// State is owned here but synced with ui_state during transition
    pub state: SyncWithRemoteState,
}

impl SyncWithRemoteScreen {
    /// Create a new sync with remote screen.
    pub fn new() -> Self {
        Self {
            component: PushChangesComponent::new(),
            state: SyncWithRemoteState::default(),
        }
    }

    /// Render with all required context.
    pub fn render_with_context(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        self.component.render_with_state(
            frame,
            area,
            &mut self.state,
            config,
            syntax_set,
            theme,
        )
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
        let result = GitService::sync(&ctx.config);

        // Update state with result
        self.state.is_syncing = false;
        self.state.sync_progress = None;
        self.state.sync_result = Some(result.message);
        self.state.pulled_changes_count = result.pulled_count;
        self.state.show_result_popup = true;

        Ok(())
    }
}

impl Default for SyncWithRemoteScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for SyncWithRemoteScreen {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, _ctx: &RenderContext) -> Result<()> {
        // Note: Use render_with_context instead as this screen needs syntax highlighting
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
                            // Start pushing if not already pushing and we have changes
                            if !self.state.is_syncing && !self.state.changed_files.is_empty() {
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
