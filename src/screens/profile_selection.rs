//! Profile selection screen controller.
//!
//! This screen handles profile selection after initial repository setup.
//! Users can select an existing profile or create a new one.

use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::screens::ActionResult;
use crate::services::ProfileService;
use crate::styles::theme;
use crate::ui::{ProfileSelectionState, Screen as ScreenId};
use crate::widgets::{DialogVariant, TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
use ratatui::Frame;
use std::path::Path;
use tracing::{error, info};

/// Actions that can be processed by the profile selection screen
#[derive(Debug, Clone)]
pub enum ProfileSelectionAction {
    /// Create a new profile and then activate it
    CreateAndActivateProfile { name: String },
    /// Activate an existing profile
    ActivateProfile { name: String },
}

/// Profile selection screen controller.
pub struct ProfileSelectionScreen {
    state: ProfileSelectionState,
}

impl ProfileSelectionScreen {
    /// Create a new profile selection screen.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ProfileSelectionState::default(),
        }
    }

    /// Get the current state.
    #[must_use]
    pub fn get_state(&self) -> &ProfileSelectionState {
        &self.state
    }

    /// Get mutable state.
    pub fn get_state_mut(&mut self) -> &mut ProfileSelectionState {
        &mut self.state
    }

    /// Reset the screen state.
    pub fn reset(&mut self) {
        self.state = ProfileSelectionState::default();
    }

    /// Set the profiles to select from.
    pub fn set_profiles(&mut self, profiles: Vec<String>) {
        self.state.profiles = profiles;
        if !self.state.profiles.is_empty() {
            self.state.list_state.select(Some(0));
        }
    }

    /// Render the exit warning popup.
    fn render_exit_warning(&self, frame: &mut Frame, area: Rect, config: &Config) {
        use crate::widgets::{Dialog, DialogVariant};

        let icons = crate::icons::Icons::from_config(config);
        let warning_text = format!(
            "{} Profile Selection Required\n\n\
            You MUST select a profile before continuing.\n\
            Activating a profile will replace your current dotfiles with symlinks.\n\
            This action cannot be undone without restoring from backups.\n\n\
            Please select a profile or create a new one.\n\
            Press Esc again to cancel and return to main menu.",
            icons.warning()
        );

        let footer_text = format!(
            "{}: Cancel & Return to Main Menu",
            config
                .keymap
                .get_key_display_for_action(crate::keymap::Action::Cancel)
        );

        let dialog = Dialog::new("Warning", &warning_text)
            .height(35)
            .variant(DialogVariant::Warning)
            .footer(&footer_text);
        frame.render_widget(dialog, area);
    }

    /// Render the create profile popup.
    fn render_create_popup(&mut self, frame: &mut Frame, area: Rect, config: &Config) {
        use crate::components::footer::Footer;
        use crate::utils::center_popup;

        let popup_area = center_popup(area, 50, 20);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Min(0),
                Constraint::Length(2),
            ])
            .split(popup_area);

        let custom_block = Block::default()
            .title(" Create New Profile ")
            .borders(Borders::ALL)
            .border_type(theme().border_type(false))
            .border_style(Style::default().fg(Color::Cyan));

        let widget = TextInputWidget::new(&self.state.create_name_input)
            .placeholder("Enter profile name...")
            .focused(true)
            .block(custom_block);

        frame.render_text_input_widget(widget, chunks[0]);

        let footer_text = format!(
            "{}: Create  |  {}: Cancel",
            config
                .keymap
                .get_key_display_for_action(crate::keymap::Action::Confirm),
            config
                .keymap
                .get_key_display_for_action(crate::keymap::Action::Cancel)
        );
        let _ = Footer::render(frame, chunks[2], &footer_text);
    }

    /// Render the main profile list.
    fn render_profile_list(&mut self, frame: &mut Frame, area: Rect, config: &Config) {
        use crate::components::footer::Footer;
        use crate::components::header::Header;
        use crate::styles::LIST_HIGHLIGHT_SYMBOL;
        use crate::utils::create_standard_layout;

        let icons = crate::icons::Icons::from_config(config);
        let (header_area, content_area, footer_area) = create_standard_layout(area, 5, 3);

        // Header
        let _ = Header::render(
            frame,
            header_area,
            "Select Profile to Activate",
            "Choose which profile to activate after setup",
        );

        // Build list items
        let mut items: Vec<ListItem> = self
            .state
            .profiles
            .iter()
            .map(|name| ListItem::new(format!("  {name}")))
            .collect();

        // Add "Create New Profile" option
        items.push(
            ListItem::new(format!("  {} Create New Profile", icons.create()))
                .style(Style::default().fg(Color::Cyan)),
        );

        let list = List::new(items)
            .block(
                Block::default()
                    .title(" Available Profiles ")
                    .borders(Borders::ALL)
                    .border_type(theme().border_type(false)),
            )
            .highlight_style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            )
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        frame.render_stateful_widget(list, content_area, &mut self.state.list_state);

        // Footer
        let footer_text = format!(
            "{}: Navigate | {}: Activate/Create | {}: Cancel",
            config.keymap.navigation_display(),
            config
                .keymap
                .get_key_display_for_action(crate::keymap::Action::Confirm),
            config
                .keymap
                .get_key_display_for_action(crate::keymap::Action::Cancel)
        );
        let _ = Footer::render(frame, footer_area, &footer_text);
    }

    /// Process a profile selection action.
    ///
    /// This method dispatches actions to the appropriate handler methods.
    ///
    /// # Arguments
    ///
    /// * `action` - The action to process.
    /// * `config` - Mutable reference to the application configuration.
    /// * `config_path` - Path to the configuration file.
    ///
    /// # Returns
    ///
    /// An `ActionResult` indicating the outcome of the action.
    pub fn process_action(
        &mut self,
        action: ProfileSelectionAction,
        config: &mut Config,
        config_path: &Path,
    ) -> Result<ActionResult> {
        match action {
            ProfileSelectionAction::CreateAndActivateProfile { name } => {
                // First create the profile
                match ProfileService::create_profile(&config.repo_path, &name, None, None) {
                    Ok(sanitized_name) => {
                        info!("Created profile '{}' during setup", sanitized_name);
                        // Then activate it
                        self.activate_profile(config, config_path, &sanitized_name)
                    }
                    Err(e) => {
                        error!("Failed to create profile '{}': {}", name, e);
                        Ok(ActionResult::ShowDialog {
                            title: "Profile Creation Failed".to_string(),
                            content: format!("Failed to create profile '{name}': {e}"),
                            variant: DialogVariant::Error,
                        })
                    }
                }
            }
            ProfileSelectionAction::ActivateProfile { name } => {
                self.activate_profile(config, config_path, &name)
            }
        }
    }

    /// Activate a profile and navigate to the main menu.
    ///
    /// This method sets the active profile in config, saves the config,
    /// calls `ProfileService` to activate the profile (create symlinks),
    /// marks the profile as activated, and navigates to the main menu.
    ///
    /// # Arguments
    ///
    /// * `config` - Mutable reference to the application configuration.
    /// * `config_path` - Path to the configuration file.
    /// * `profile_name` - Name of the profile to activate.
    ///
    /// # Returns
    ///
    /// An `ActionResult` indicating navigation or error dialog.
    fn activate_profile(
        &mut self,
        config: &mut Config,
        config_path: &Path,
        profile_name: &str,
    ) -> Result<ActionResult> {
        // Set active profile and save config
        config.active_profile = profile_name.to_string();
        if let Err(e) = config.save(config_path) {
            error!("Failed to save config with active profile: {}", e);
            return Ok(ActionResult::ShowDialog {
                title: "Configuration Error".to_string(),
                content: format!("Failed to save configuration: {e}"),
                variant: DialogVariant::Error,
            });
        }

        // Call ProfileService to activate the profile (create symlinks)
        match ProfileService::activate_profile(
            &config.repo_path,
            profile_name,
            config.backup_enabled,
        ) {
            Ok(result) => {
                info!(
                    "Activated profile '{}' with {} files",
                    profile_name, result.success_count
                );

                // Mark as activated and save config again
                config.profile_activated = true;
                if let Err(e) = config.save(config_path) {
                    error!("Failed to save config after activation: {}", e);
                    return Ok(ActionResult::ShowDialog {
                        title: "Configuration Error".to_string(),
                        content: format!("Failed to save configuration after activation: {e}"),
                        variant: DialogVariant::Error,
                    });
                }

                // Reset screen state
                self.reset();

                // Navigate to main menu
                Ok(ActionResult::Navigate(ScreenId::MainMenu))
            }
            Err(e) => {
                error!("Failed to activate profile '{}': {}", profile_name, e);
                Ok(ActionResult::ShowDialog {
                    title: "Activation Failed".to_string(),
                    content: format!("Failed to activate profile '{profile_name}': {e}"),
                    variant: DialogVariant::Error,
                })
            }
        }
    }
}

impl Default for ProfileSelectionScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ProfileSelectionScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Background
        let t = crate::styles::theme();
        let background = ratatui::widgets::Block::default().style(t.background_style());
        frame.render_widget(background, area);

        // Always render main content first
        if self.state.show_create_popup {
            self.render_create_popup(frame, area, ctx.config);
        } else {
            self.render_profile_list(frame, area, ctx.config);
        }

        // Render dialogs on top of the content (not instead of it)
        if self.state.show_exit_warning {
            self.render_exit_warning(frame, area, ctx.config);
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        // Handle exit warning
        if self.state.show_exit_warning {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
                    self.state.show_exit_warning = false;
                    self.reset();
                    return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                }
            }
            return Ok(ScreenAction::None);
        }

        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ScreenAction::None);
            }

            let action = ctx.config.keymap.get_action(key.code, key.modifiers);

            if let Some(action) = action {
                use crate::keymap::Action;
                match action {
                    Action::MoveUp => {
                        if self.state.show_create_popup {
                            // In popup, handle cursor movement
                            self.state.create_name_input.handle_action(Action::MoveUp);
                        } else if let Some(current) = self.state.list_state.selected() {
                            if current > 0 {
                                self.state.list_state.select(Some(current - 1));
                            } else {
                                // Wrap to bottom (including create option)
                                self.state
                                    .list_state
                                    .select(Some(self.state.profiles.len()));
                            }
                        } else if !self.state.profiles.is_empty() {
                            self.state
                                .list_state
                                .select(Some(self.state.profiles.len()));
                        }
                    }
                    Action::MoveDown => {
                        if self.state.show_create_popup {
                            self.state.create_name_input.handle_action(Action::MoveDown);
                        } else if let Some(current) = self.state.list_state.selected() {
                            if current < self.state.profiles.len() {
                                self.state.list_state.select(Some(current + 1));
                            } else {
                                // Wrap to top
                                self.state.list_state.select(Some(0));
                            }
                        } else if !self.state.profiles.is_empty() {
                            self.state.list_state.select(Some(0));
                        }
                    }
                    Action::Confirm => {
                        if self.state.show_create_popup {
                            let profile_name =
                                self.state.create_name_input.text_trimmed().to_string();
                            if !profile_name.is_empty() {
                                self.state.show_create_popup = false;
                                return Ok(ScreenAction::CreateAndActivateProfile {
                                    name: profile_name,
                                });
                            }
                        } else if let Some(idx) = self.state.list_state.selected() {
                            if idx == self.state.profiles.len() {
                                // "Create New Profile" selected
                                self.state.show_create_popup = true;
                                self.state.create_name_input.clear();
                            } else if let Some(name) = self.state.profiles.get(idx) {
                                let name = name.clone();
                                return Ok(ScreenAction::ActivateProfile { name });
                            }
                        }
                    }
                    Action::Quit | Action::Cancel => {
                        if self.state.show_create_popup {
                            self.state.show_create_popup = false;
                            self.state.create_name_input.clear();
                        } else {
                            self.state.show_exit_warning = true;
                        }
                    }
                    _ => {
                        // Forward other actions (like Backspace, etc.) to input if focused
                        if self.state.show_create_popup {
                            self.state.create_name_input.handle_action(action);
                        }
                    }
                }
            } else {
                // Raw input for create popup
                if self.state.show_create_popup {
                    self.state.create_name_input.handle_key(key.code);
                }
            }
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        self.state.show_create_popup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_selection_screen_creation() {
        let screen = ProfileSelectionScreen::new();
        assert!(!screen.is_input_focused());
        assert!(screen.state.profiles.is_empty());
    }

    #[test]
    fn test_set_profiles() {
        let mut screen = ProfileSelectionScreen::new();
        screen.set_profiles(vec!["default".to_string(), "work".to_string()]);
        assert_eq!(screen.state.profiles.len(), 2);
        assert_eq!(screen.state.list_state.selected(), Some(0));
    }

    #[test]
    fn test_reset() {
        let mut screen = ProfileSelectionScreen::new();
        screen.set_profiles(vec!["test".to_string()]);
        screen.state.show_create_popup = true;
        screen.reset();
        assert!(screen.state.profiles.is_empty());
        assert!(!screen.state.show_create_popup);
    }
}
