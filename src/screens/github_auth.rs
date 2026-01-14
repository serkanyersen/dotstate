//! GitHub authentication screen controller.
//!
//! This screen handles the GitHub authentication and repository setup flow.
//! It supports two modes:
//! - GitHub mode: Creates/uses a GitHub repository with token authentication
//! - Local mode: Uses an existing local git repository

use crate::components::component::Component;
use crate::components::GitHubAuthComponent;
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::{GitHubAuthField, GitHubAuthState, GitHubAuthStep, SetupMode};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::Frame;

/// GitHub authentication screen controller.
///
/// This screen owns its state and handles both rendering and events.
pub struct GitHubAuthScreen {
    component: GitHubAuthComponent,
    /// Screen owns its state
    state: GitHubAuthState,
}

impl GitHubAuthScreen {
    /// Create a new GitHub auth screen.
    pub fn new() -> Self {
        Self {
            component: GitHubAuthComponent::new(),
            state: GitHubAuthState::default(),
        }
    }

    /// Update configuration.
    pub fn update_config(&mut self, config: Config) {
        self.component.update_config(config);
    }

    /// Get the current auth state.
    pub fn get_auth_state(&self) -> &GitHubAuthState {
        &self.state
    }

    /// Get mutable auth state.
    pub fn get_auth_state_mut(&mut self) -> &mut GitHubAuthState {
        &mut self.state
    }

    /// Render the screen.
    pub fn render_frame(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Sync state to component for rendering
        *self.component.get_auth_state_mut() = self.state.clone();
        self.component.render(frame, area)
    }

    /// Reset the screen state to default.
    pub fn reset(&mut self) {
        self.state = GitHubAuthState::default();
    }

    /// Check if the async setup step needs processing.
    /// Returns true if there's a setup step in progress that needs ticking.
    pub fn needs_tick(&self) -> bool {
        matches!(self.state.step, GitHubAuthStep::SetupStep(_))
    }

    /// Handle mode selection (Choosing mode).
    fn handle_mode_selection(
        &mut self,
        action: Option<crate::keymap::Action>,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    if self.state.mode_selection_index > 0 {
                        self.state.mode_selection_index -= 1;
                    }
                }
                Action::MoveDown => {
                    if self.state.mode_selection_index < 1 {
                        self.state.mode_selection_index += 1;
                    }
                }
                Action::Confirm => {
                    if self.state.mode_selection_index == 0 {
                        self.state.setup_mode = SetupMode::GitHub;
                    } else {
                        self.state.setup_mode = SetupMode::Local;
                        self.state.input_focused = true;
                    }
                }
                Action::Cancel | Action::Quit => {
                    self.reset();
                    return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
                }
                _ => {}
            }
        }
        Ok(ScreenAction::None)
    }

    /// Handle local setup input.
    fn handle_local_setup_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        ctx: &ScreenContext,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        let action = ctx.config.keymap.get_action(key.code, key.modifiers);
        self.state.error_message = None;

        // If already configured, only allow Esc/Cancel to go back
        if self.state.repo_already_configured {
            if let Some(Action::Cancel | Action::Quit) = action {
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
            return Ok(ScreenAction::None);
        }

        // Check for Action::Confirm or Action::Save to validate and save
        if matches!(action, Some(Action::Confirm | Action::Save)) {
            let path_str = self.state.local_repo_path_input.text_trimmed();
            if path_str.is_empty() {
                self.state.error_message = Some("Please enter a repository path".to_string());
                return Ok(ScreenAction::None);
            }

            let expanded_path = crate::git::expand_path(path_str);
            let validation = crate::git::validate_local_repo(&expanded_path);

            if !validation.is_valid {
                self.state.error_message = validation.error_message;
                return Ok(ScreenAction::None);
            }

            // Validation passed - signal to app to save config
            self.state.status_message = Some(format!(
                "✅ Valid repository found!\n\nRemote: {}\n\nSaving configuration...",
                validation.remote_url.as_deref().unwrap_or("unknown")
            ));

            // Load profiles from the repository
            let profiles = crate::utils::ProfileManifest::load_or_backfill(&expanded_path)
                .map(|m| m.profiles.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();

            return Ok(ScreenAction::SaveLocalRepoConfig {
                repo_path: expanded_path,
                profiles,
            });
        }

        if let Some(Action::Cancel | Action::Quit) = action {
            self.state.setup_mode = SetupMode::Choosing;
            self.state.error_message = None;
            self.state.status_message = None;
            return Ok(ScreenAction::None);
        }

        // Handle text input
        match key.code {
            KeyCode::Esc => {
                self.state.setup_mode = SetupMode::Choosing;
                self.state.error_message = None;
                self.state.status_message = None;
            }
            KeyCode::Char(c) => {
                self.state.local_repo_path_input.insert_char(c);
            }
            KeyCode::Backspace => {
                self.state.local_repo_path_input.backspace();
            }
            KeyCode::Delete => {
                self.state.local_repo_path_input.delete();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.state.local_repo_path_input.handle_key(key.code);
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    /// Handle GitHub setup input (Input step).
    fn handle_github_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        ctx: &ScreenContext,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        let action = ctx.config.keymap.get_action(key.code, key.modifiers);

        // Handle "Update Token" action if repo is configured
        if self.state.repo_already_configured && !self.state.is_editing_token {
            if let Some(Action::Edit) = action {
                self.state.is_editing_token = true;
                self.state.token_input.clear();
                self.state.focused_field = GitHubAuthField::Token;
                return Ok(ScreenAction::None);
            }
            if let Some(Action::Cancel | Action::Quit) = action {
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
        }

        // Check for Save/Confirm action
        if matches!(action, Some(Action::Save) | Some(Action::Confirm)) {
            if self.state.repo_already_configured && self.state.is_editing_token {
                // Just update the token
                let token = self.state.token_input.text_trimmed().to_string();
                return Ok(ScreenAction::UpdateGitHubToken { token });
            } else if !self.state.repo_already_configured {
                // Full setup - validate and start setup
                let token = self.state.token_input.text_trimmed().to_string();
                let repo_name = ctx.config.repo_name.clone();

                // Validate token format
                if !token.starts_with("ghp_") {
                    let actual_start = if token.len() >= 4 {
                        &token[..4]
                    } else {
                        "too short"
                    };
                    self.state.error_message = Some(format!(
                        "❌ Invalid token format: Must start with 'ghp_' but starts with '{}'.\n\
                         See help for more details.",
                        actual_start
                    ));
                    return Ok(ScreenAction::None);
                }

                if token.len() < 40 {
                    self.state.error_message = Some(format!(
                        "❌ Token appears incomplete: {} characters (expected 40+).",
                        token.len()
                    ));
                    return Ok(ScreenAction::None);
                }

                // Return action to start the setup
                return Ok(ScreenAction::StartGitHubSetup {
                    token,
                    repo_name,
                    is_private: self.state.is_private,
                });
            }
            return Ok(ScreenAction::None);
        }

        // Handle navigation and editing actions
        if let Some(act) = action {
            match act {
                Action::Cancel | Action::Quit => {
                    self.reset();
                    return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
                }
                Action::NextTab if !self.state.repo_already_configured => {
                    self.state.focused_field = match self.state.focused_field {
                        GitHubAuthField::Token => GitHubAuthField::RepoName,
                        GitHubAuthField::RepoName => GitHubAuthField::RepoLocation,
                        GitHubAuthField::RepoLocation => GitHubAuthField::IsPrivate,
                        GitHubAuthField::IsPrivate => GitHubAuthField::Token,
                    };
                    return Ok(ScreenAction::None);
                }
                Action::PrevTab if !self.state.repo_already_configured => {
                    self.state.focused_field = match self.state.focused_field {
                        GitHubAuthField::Token => GitHubAuthField::IsPrivate,
                        GitHubAuthField::RepoName => GitHubAuthField::Token,
                        GitHubAuthField::RepoLocation => GitHubAuthField::RepoName,
                        GitHubAuthField::IsPrivate => GitHubAuthField::RepoLocation,
                    };
                    return Ok(ScreenAction::None);
                }
                Action::MoveLeft => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_left(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_left(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_left(),
                        GitHubAuthField::IsPrivate if !self.state.repo_already_configured => {
                            self.state.is_private = !self.state.is_private;
                        }
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::MoveRight => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_right(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_right(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_right(),
                        GitHubAuthField::IsPrivate if !self.state.repo_already_configured => {
                            self.state.is_private = !self.state.is_private;
                        }
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::Home => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_home(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_home(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_home(),
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::End => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_end(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_end(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_end(),
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::Backspace => {
                    self.handle_backspace();
                    return Ok(ScreenAction::None);
                }
                Action::DeleteChar => {
                    self.handle_delete();
                    return Ok(ScreenAction::None);
                }
                Action::ToggleSelect => {
                    if self.state.focused_field == GitHubAuthField::IsPrivate
                        && !self.state.repo_already_configured
                    {
                        self.state.is_private = !self.state.is_private;
                    }
                    return Ok(ScreenAction::None);
                }
                _ => {}
            }
        }

        // Handle character input
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                self.handle_char_input(c);
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle Processing step events.
    fn handle_processing_input(
        &mut self,
        action: Option<crate::keymap::Action>,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        match action {
            Some(Action::Confirm) => {
                // Processing is done, navigate based on profiles
                // The app will handle profile selection setup
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
            Some(Action::Cancel | Action::Quit) => {
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }

    /// Handle SetupStep events (during async operations).
    fn handle_setup_step_input(
        &mut self,
        action: Option<crate::keymap::Action>,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        if let Some(Action::Cancel | Action::Quit) = action {
            self.reset();
            return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
        }
        Ok(ScreenAction::None)
    }

    // Helper methods

    fn handle_backspace(&mut self) {
        match self.state.focused_field {
            GitHubAuthField::Token => self.state.token_input.backspace(),
            GitHubAuthField::RepoName => self.state.repo_name_input.backspace(),
            GitHubAuthField::RepoLocation => self.state.repo_location_input.backspace(),
            GitHubAuthField::IsPrivate => {}
        }
    }

    fn handle_delete(&mut self) {
        match self.state.focused_field {
            GitHubAuthField::Token => self.state.token_input.delete(),
            GitHubAuthField::RepoName => self.state.repo_name_input.delete(),
            GitHubAuthField::RepoLocation => self.state.repo_location_input.delete(),
            GitHubAuthField::IsPrivate => {}
        }
    }

    fn handle_char_input(&mut self, c: char) {
        let can_edit_token = !self.state.repo_already_configured || self.state.is_editing_token;

        match self.state.focused_field {
            GitHubAuthField::Token if can_edit_token => {
                self.state.token_input.insert_char(c);
            }
            GitHubAuthField::RepoName if !self.state.repo_already_configured => {
                self.state.repo_name_input.insert_char(c);
            }
            GitHubAuthField::RepoLocation if !self.state.repo_already_configured => {
                self.state.repo_location_input.insert_char(c);
            }
            _ => {}
        }
    }
}

impl Default for GitHubAuthScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for GitHubAuthScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) -> Result<()> {
        // Sync state to component for rendering
        *self.component.get_auth_state_mut() = self.state.clone();
        self.component.render(frame, area)
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        self.state.error_message = None;

        // Handle keyboard events
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ScreenAction::None);
            }

            let action = ctx.config.keymap.get_action(key.code, key.modifiers);

            // Handle based on current mode and step
            match self.state.setup_mode {
                SetupMode::Choosing => {
                    return self.handle_mode_selection(action);
                }
                SetupMode::Local => {
                    return self.handle_local_setup_input(key, ctx);
                }
                SetupMode::GitHub => {
                    // Handle based on step
                    match self.state.step {
                        GitHubAuthStep::Input => {
                            return self.handle_github_input(key, ctx);
                        }
                        GitHubAuthStep::Processing => {
                            return self.handle_processing_input(action);
                        }
                        GitHubAuthStep::SetupStep(_) => {
                            return self.handle_setup_step_input(action);
                        }
                    }
                }
            }
        }

        // Handle mouse events
        if matches!(event, Event::Mouse(_)) {
            // Sync state to component
            *self.component.get_auth_state_mut() = self.state.clone();
            let component_action = self.component.handle_event(event)?;
            // Sync state back
            self.state = self.component.get_auth_state().clone();

            if component_action == crate::components::ComponentAction::Update {
                return Ok(ScreenAction::Refresh);
            }
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        self.state.input_focused
    }

    fn on_enter(&mut self, _ctx: &ScreenContext) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_auth_screen_creation() {
        let screen = GitHubAuthScreen::new();
        assert!(screen.is_input_focused());
    }

    #[test]
    fn test_get_auth_state() {
        let screen = GitHubAuthScreen::new();
        let state = screen.get_auth_state();
        assert_eq!(state.setup_mode, SetupMode::Choosing);
    }

    #[test]
    fn test_reset() {
        let mut screen = GitHubAuthScreen::new();
        screen.state.token_input = crate::utils::TextInput::with_text("test_token");
        screen.state.setup_mode = SetupMode::GitHub;
        screen.reset();
        assert!(screen.state.token_input.is_empty());
        assert_eq!(screen.state.setup_mode, SetupMode::Choosing);
    }

    #[test]
    fn test_needs_tick() {
        let mut screen = GitHubAuthScreen::new();
        assert!(!screen.needs_tick());

        screen.state.step = GitHubAuthStep::SetupStep(crate::ui::GitHubSetupStep::Connecting);
        assert!(screen.needs_tick());
    }
}
