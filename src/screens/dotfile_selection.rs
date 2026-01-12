//! Dotfile selection screen controller.
//!
//! This screen handles selecting and managing dotfiles for syncing.
//! It wraps the DotfileSelectionComponent for rendering and handles all events.

use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::{DotfileSelectionFocus, DotfileSelectionState, Screen as ScreenId};
use crate::utils::list_navigation::ListStateExt;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;
use std::path::Path;

/// Dotfile selection screen controller.
pub struct DotfileSelectionScreen {
    state: DotfileSelectionState,
}

impl DotfileSelectionScreen {
    /// Create a new dotfile selection screen.
    pub fn new() -> Self {
        Self {
            state: DotfileSelectionState::default(),
        }
    }

    /// Get the current state.
    pub fn get_state(&self) -> &DotfileSelectionState {
        &self.state
    }

    /// Get mutable state.
    pub fn get_state_mut(&mut self) -> &mut DotfileSelectionState {
        &mut self.state
    }

    /// Set backup enabled state.
    pub fn set_backup_enabled(&mut self, enabled: bool) {
        self.state.backup_enabled = enabled;
    }

    /// Handle modal confirmation events.
    fn handle_modal_event(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        match action {
            Some(Action::Yes) | Some(Action::Confirm) => {
                // YES logic - extract values and close modal
                let full_path = self.state.custom_file_confirm_path.clone().unwrap();
                let relative_path = self.state.custom_file_confirm_relative.clone().unwrap();
                self.state.show_custom_file_confirm = false;
                self.state.custom_file_confirm_path = None;
                self.state.custom_file_confirm_relative = None;

                Ok(ScreenAction::AddCustomFileToSync {
                    full_path,
                    relative_path,
                })
            }
            Some(Action::No) | Some(Action::Cancel) => {
                // NO logic - close modal
                self.state.show_custom_file_confirm = false;
                self.state.custom_file_confirm_path = None;
                self.state.custom_file_confirm_relative = None;
                Ok(ScreenAction::None)
            }
            _ => Ok(ScreenAction::None),
        }
    }

    /// Handle file browser input (when typing in path field).
    fn handle_file_browser_input(&mut self, key_code: KeyCode) -> Result<ScreenAction> {
        if !self.state.file_browser_path_focused
            || self.state.focus != DotfileSelectionFocus::FileBrowserInput
        {
            return Ok(ScreenAction::None);
        }

        match key_code {
            KeyCode::Char(c) => {
                crate::utils::handle_char_insertion(
                    &mut self.state.file_browser_path_input,
                    &mut self.state.file_browser_path_cursor,
                    c,
                );
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                let input = self.state.file_browser_path_input.clone();
                crate::utils::handle_cursor_movement(
                    &input,
                    &mut self.state.file_browser_path_cursor,
                    key_code,
                );
            }
            KeyCode::Backspace => {
                crate::utils::handle_backspace(
                    &mut self.state.file_browser_path_input,
                    &mut self.state.file_browser_path_cursor,
                );
            }
            KeyCode::Delete => {
                crate::utils::handle_delete(
                    &mut self.state.file_browser_path_input,
                    &mut self.state.file_browser_path_cursor,
                );
            }
            KeyCode::Enter => {
                // Load path from input
                let path_str = self.state.file_browser_path_input.trim();
                if !path_str.is_empty() {
                    let full_path = crate::utils::expand_path(path_str);

                    if full_path.exists() {
                        if full_path.is_dir() {
                            self.state.file_browser_path = full_path.clone();
                            self.state.file_browser_path_input =
                                self.state.file_browser_path.to_string_lossy().to_string();
                            self.state.file_browser_path_cursor =
                                self.state.file_browser_path_input.chars().count();
                            self.state.file_browser_list_state.select(Some(0));
                            self.state.focus = DotfileSelectionFocus::FileBrowserList;
                            return Ok(ScreenAction::RefreshFileBrowser);
                        } else {
                            // It's a file - sync it directly
                            let home_dir = crate::utils::get_home_dir();
                            let relative_path = full_path
                                .strip_prefix(&home_dir)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| full_path.to_string_lossy().to_string());

                            // Close browser
                            self.state.file_browser_mode = false;
                            self.state.adding_custom_file = false;
                            self.state.file_browser_path_input.clear();
                            self.state.file_browser_path_cursor = 0;
                            self.state.focus = DotfileSelectionFocus::FilesList;

                            return Ok(ScreenAction::AddCustomFileToSync {
                                full_path,
                                relative_path,
                            });
                        }
                    }
                }
            }
            KeyCode::Tab => {
                self.state.file_browser_path_focused = false;
                self.state.focus = DotfileSelectionFocus::FileBrowserList;
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    /// Handle custom file input (legacy mode, less common).
    fn handle_custom_file_input(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        // When input is not focused, only allow Enter to focus or Esc to cancel
        if !self.state.custom_file_focused {
            match key_code {
                KeyCode::Enter => {
                    self.state.custom_file_focused = true;
                    return Ok(ScreenAction::None);
                }
                KeyCode::Esc => {
                    self.state.adding_custom_file = false;
                    self.state.custom_file_input.clear();
                    self.state.custom_file_cursor = 0;
                    return Ok(ScreenAction::None);
                }
                _ => return Ok(ScreenAction::None),
            }
        }

        // When focused, handle all input
        match key_code {
            KeyCode::Char(c) => {
                crate::utils::handle_char_insertion(
                    &mut self.state.custom_file_input,
                    &mut self.state.custom_file_cursor,
                    c,
                );
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                let input = self.state.custom_file_input.clone();
                crate::utils::handle_cursor_movement(
                    &input,
                    &mut self.state.custom_file_cursor,
                    key_code,
                );
            }
            KeyCode::Backspace => {
                crate::utils::handle_backspace(
                    &mut self.state.custom_file_input,
                    &mut self.state.custom_file_cursor,
                );
            }
            KeyCode::Delete => {
                crate::utils::handle_delete(
                    &mut self.state.custom_file_input,
                    &mut self.state.custom_file_cursor,
                );
            }
            KeyCode::Tab => {
                self.state.custom_file_focused = false;
            }
            KeyCode::Enter => {
                let path_str = self.state.custom_file_input.trim();
                if path_str.is_empty() {
                    self.state.status_message =
                        Some("Error: File path cannot be empty".to_string());
                } else {
                    let full_path = crate::utils::expand_path(path_str);

                    if !full_path.exists() {
                        self.state.status_message =
                            Some(format!("Error: File does not exist: {:?}", full_path));
                    } else {
                        // Calculate relative path
                        let home_dir = crate::utils::get_home_dir();
                        let relative_path = match full_path.strip_prefix(&home_dir) {
                            Ok(p) => p.to_string_lossy().to_string(),
                            Err(_) => path_str.to_string(),
                        };

                        // Close input mode
                        self.state.adding_custom_file = false;
                        self.state.custom_file_input.clear();
                        self.state.custom_file_cursor = 0;
                        self.state.focus = DotfileSelectionFocus::FilesList;

                        // Validate before showing confirmation
                        let repo_path = &config.repo_path;
                        let (is_safe, reason) = crate::utils::is_safe_to_add(&full_path, repo_path);
                        if !is_safe {
                            self.state.status_message = Some(format!(
                                "Error: {}. Path: {}",
                                reason.unwrap_or_else(|| "Cannot add this file".to_string()),
                                full_path.display()
                            ));
                            return Ok(ScreenAction::None);
                        }

                        // Show confirmation modal
                        self.state.show_custom_file_confirm = true;
                        self.state.custom_file_confirm_path = Some(full_path);
                        self.state.custom_file_confirm_relative = Some(relative_path);
                    }
                }
            }
            KeyCode::Esc => {
                self.state.adding_custom_file = false;
                self.state.custom_file_input.clear();
                self.state.custom_file_cursor = 0;
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    /// Handle file browser list navigation and selection.
    fn handle_file_browser_list(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    self.state.file_browser_list_state.select_previous();
                }
                Action::MoveDown => {
                    self.state.file_browser_list_state.select_next();
                }
                Action::Confirm => {
                    if let Some(idx) = self.state.file_browser_list_state.selected() {
                        if idx < self.state.file_browser_entries.len() {
                            let entry = self.state.file_browser_entries[idx].clone();

                            // Handle special entries
                            if entry == Path::new("..") {
                                // Go to parent directory
                                if let Some(parent) = self.state.file_browser_path.parent() {
                                    self.state.file_browser_path = parent.to_path_buf();
                                    self.state.file_browser_path_input =
                                        self.state.file_browser_path.to_string_lossy().to_string();
                                    self.state.file_browser_path_cursor =
                                        self.state.file_browser_path_input.chars().count();
                                    self.state.file_browser_list_state.select(Some(0));
                                    return Ok(ScreenAction::RefreshFileBrowser);
                                }
                            } else if entry == Path::new(".") {
                                // Add current folder
                                let current_folder = self.state.file_browser_path.clone();
                                let home_dir = crate::utils::get_home_dir();
                                let relative_path = current_folder
                                    .strip_prefix(&home_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| {
                                        current_folder.to_string_lossy().to_string()
                                    });

                                // Validate
                                let repo_path = &config.repo_path;
                                let (is_safe, reason) =
                                    crate::utils::is_safe_to_add(&current_folder, repo_path);
                                if !is_safe {
                                    self.state.status_message = Some(format!(
                                            "Error: {}. Path: {}",
                                            reason.unwrap_or_else(
                                                || "Cannot add this folder".to_string()
                                            ),
                                            current_folder.display()
                                        ));
                                    return Ok(ScreenAction::None);
                                }

                                // Check if git repo
                                if crate::utils::is_git_repo(&current_folder) {
                                    self.state.status_message = Some(format!(
                                        "Error: Cannot sync a git repository. Path: {}",
                                        current_folder.display()
                                    ));
                                    return Ok(ScreenAction::None);
                                }

                                // Show confirmation modal
                                self.state.show_custom_file_confirm = true;
                                self.state.custom_file_confirm_path = Some(current_folder);
                                self.state.custom_file_confirm_relative = Some(relative_path);
                                self.state.file_browser_mode = false;
                                self.state.adding_custom_file = false;
                                self.state.file_browser_path_input.clear();
                                self.state.file_browser_path_cursor = 0;
                                self.state.focus = DotfileSelectionFocus::FilesList;
                            } else {
                                // Regular file or directory
                                let full_path = if entry.is_absolute() {
                                    entry.clone()
                                } else {
                                    self.state.file_browser_path.join(&entry)
                                };

                                if full_path.is_dir() {
                                    // Navigate into directory
                                    self.state.file_browser_path = full_path.clone();
                                    self.state.file_browser_path_input =
                                        full_path.to_string_lossy().to_string();
                                    self.state.file_browser_path_cursor =
                                        self.state.file_browser_path_input.chars().count();
                                    self.state.file_browser_list_state.select(Some(0));
                                    return Ok(ScreenAction::RefreshFileBrowser);
                                } else if full_path.is_file() {
                                    // Sync file directly
                                    let home_dir = crate::utils::get_home_dir();
                                    let relative_path = full_path
                                        .strip_prefix(&home_dir)
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|_| {
                                            full_path.to_string_lossy().to_string()
                                        });

                                    // Close browser
                                    self.state.file_browser_mode = false;
                                    self.state.adding_custom_file = false;
                                    self.state.file_browser_path_input.clear();
                                    self.state.file_browser_path_cursor = 0;
                                    self.state.focus = DotfileSelectionFocus::FilesList;

                                    return Ok(ScreenAction::AddCustomFileToSync {
                                        full_path,
                                        relative_path,
                                    });
                                }
                            }
                        }
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::FileBrowserPreview;
                    self.state.file_browser_path_focused = false;
                }
                Action::PageUp => {
                    self.state
                        .file_browser_list_state
                        .page_up(10, self.state.file_browser_entries.len());
                }
                Action::PageDown => {
                    self.state
                        .file_browser_list_state
                        .page_down(10, self.state.file_browser_entries.len());
                }
                Action::GoToTop => {
                    self.state.file_browser_list_state.select_first();
                }
                Action::GoToEnd => {
                    self.state.file_browser_list_state.select_last();
                }
                Action::Cancel | Action::Quit => {
                    // Close file browser
                    self.state.file_browser_mode = false;
                    self.state.adding_custom_file = false;
                    self.state.focus = DotfileSelectionFocus::FilesList;
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle file browser preview navigation.
    fn handle_file_browser_preview(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp | Action::ScrollUp => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_sub(1);
                }
                Action::MoveDown | Action::ScrollDown => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_add(1);
                }
                Action::PageUp => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_sub(20);
                }
                Action::PageDown => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_add(20);
                }
                Action::GoToTop => {
                    self.state.file_browser_preview_scroll = 0;
                }
                Action::GoToEnd => {
                    self.state.file_browser_preview_scroll = 10000; // Will be clamped during render
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::FileBrowserInput;
                    self.state.file_browser_path_focused = true;
                }
                Action::Cancel | Action::Quit => {
                    self.state.file_browser_mode = false;
                    self.state.adding_custom_file = false;
                    self.state.focus = DotfileSelectionFocus::FilesList;
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle main dotfile list navigation and selection.
    fn handle_dotfile_list(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    self.state.dotfile_list_state.select_previous();
                    self.state.preview_scroll = 0;
                }
                Action::MoveDown => {
                    self.state.dotfile_list_state.select_next();
                    self.state.preview_scroll = 0;
                }
                Action::Confirm => {
                    if self.state.status_message.is_some() {
                        self.state.status_message = None;
                    } else if let Some(idx) = self.state.dotfile_list_state.selected() {
                        let is_synced = self.state.selected_for_sync.contains(&idx);
                        return Ok(ScreenAction::ToggleFileSync {
                            file_index: idx,
                            is_synced,
                        });
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::Preview;
                }
                Action::PageUp => {
                    self.state
                        .dotfile_list_state
                        .page_up(10, self.state.dotfiles.len());
                    self.state.preview_scroll = 0;
                }
                Action::PageDown => {
                    self.state
                        .dotfile_list_state
                        .page_down(10, self.state.dotfiles.len());
                    self.state.preview_scroll = 0;
                }
                Action::GoToTop => {
                    self.state.dotfile_list_state.select_first();
                    self.state.preview_scroll = 0;
                }
                Action::GoToEnd => {
                    self.state.dotfile_list_state.select_last();
                    self.state.preview_scroll = 0;
                }
                Action::Create => {
                    // Open file browser
                    self.state.adding_custom_file = true;
                    self.state.file_browser_mode = true;
                    self.state.file_browser_path = crate::utils::get_home_dir();
                    self.state.file_browser_selected = 0;
                    self.state.file_browser_path_input =
                        self.state.file_browser_path.to_string_lossy().to_string();
                    self.state.file_browser_path_cursor =
                        self.state.file_browser_path_input.chars().count();
                    self.state.file_browser_path_focused = false;
                    self.state.file_browser_preview_scroll = 0;
                    self.state.focus = DotfileSelectionFocus::FileBrowserList;
                    return Ok(ScreenAction::RefreshFileBrowser);
                }
                Action::ToggleBackup => {
                    self.state.backup_enabled = !self.state.backup_enabled;
                    return Ok(ScreenAction::SetBackupEnabled {
                        enabled: self.state.backup_enabled,
                    });
                }
                Action::Cancel | Action::Quit => {
                    return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle preview pane navigation.
    fn handle_preview(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp | Action::ScrollUp => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_sub(1);
                }
                Action::MoveDown | Action::ScrollDown => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_add(1);
                }
                Action::PageUp => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_sub(20);
                }
                Action::PageDown => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_add(20);
                }
                Action::GoToTop => {
                    self.state.preview_scroll = 0;
                }
                Action::GoToEnd => {
                    // Calculate max scroll based on file content
                    if let Some(selected_index) = self.state.dotfile_list_state.selected() {
                        if selected_index < self.state.dotfiles.len() {
                            let dotfile = &self.state.dotfiles[selected_index];
                            if let Ok(content) = std::fs::read_to_string(&dotfile.original_path) {
                                let total_lines = content.lines().count();
                                let estimated_visible = 20;
                                self.state.preview_scroll =
                                    total_lines.saturating_sub(estimated_visible);
                            } else {
                                self.state.preview_scroll = 10000;
                            }
                        }
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::FilesList;
                }
                Action::Cancel | Action::Quit => {
                    return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }
}

impl Default for DotfileSelectionScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DotfileSelectionScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        use crate::components::dotfile_selection::DotfileSelectionComponent;

        let mut component = DotfileSelectionComponent::new();
        component.render_with_state(
            frame,
            area,
            &mut self.state,
            ctx.config,
            ctx.syntax_set,
            ctx.syntax_theme,
        )
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        // 1. Modal first - captures all events
        if self.state.show_custom_file_confirm {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    return self.handle_modal_event(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 2. File browser input mode
        if self.state.file_browser_mode
            && self.state.file_browser_path_focused
            && self.state.focus == DotfileSelectionFocus::FileBrowserInput
        {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    // Check for Tab/NextTab to switch focus
                    if let Some(action) = ctx.config.keymap.get_action(key.code, key.modifiers) {
                        use crate::keymap::Action;
                        if matches!(action, Action::NextTab) {
                            self.state.focus = DotfileSelectionFocus::FileBrowserList;
                            self.state.file_browser_path_focused = false;
                            return Ok(ScreenAction::None);
                        }
                    }
                    return self.handle_file_browser_input(key.code);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 3. Custom file input mode (legacy)
        if self.state.adding_custom_file && !self.state.file_browser_mode {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    return self.handle_custom_file_input(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 4. Normal navigation based on focus
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                if self.state.file_browser_mode {
                    // File browser mode navigation
                    match self.state.focus {
                        DotfileSelectionFocus::FileBrowserList => {
                            return self.handle_file_browser_list(key.code, ctx.config);
                        }
                        DotfileSelectionFocus::FileBrowserPreview => {
                            return self.handle_file_browser_preview(key.code, ctx.config);
                        }
                        DotfileSelectionFocus::FileBrowserInput => {
                            // Shouldn't reach here as input mode is handled above
                            return self.handle_file_browser_input(key.code);
                        }
                        _ => {}
                    }
                } else {
                    // Normal mode navigation
                    match self.state.focus {
                        DotfileSelectionFocus::FilesList => {
                            return self.handle_dotfile_list(key.code, ctx.config);
                        }
                        DotfileSelectionFocus::Preview => {
                            return self.handle_preview(key.code, ctx.config);
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        if self.state.file_browser_mode {
            self.state.file_browser_path_focused
                && self.state.focus == DotfileSelectionFocus::FileBrowserInput
        } else if self.state.adding_custom_file {
            self.state.custom_file_focused
        } else {
            false
        }
    }

    fn on_enter(&mut self, _ctx: &ScreenContext) -> Result<()> {
        // Request scan on enter - this will be handled by app
        // Note: We return None here but app should call scan_dotfiles when navigating to this screen
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dotfile_selection_screen_creation() {
        let screen = DotfileSelectionScreen::new();
        assert!(!screen.is_input_focused());
        assert!(screen.state.dotfiles.is_empty());
    }

    #[test]
    fn test_set_backup_enabled() {
        let mut screen = DotfileSelectionScreen::new();
        assert!(screen.state.backup_enabled);
        screen.set_backup_enabled(false);
        assert!(!screen.state.backup_enabled);
    }
}
