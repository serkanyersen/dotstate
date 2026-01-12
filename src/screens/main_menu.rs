//! Main menu screen controller.
//!
//! This screen is the application's entry point after setup, showing
//! the main navigation menu.

use crate::components::component::Component;
use crate::components::{ComponentAction, MainMenuComponent, MenuItem};
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::Screen as ScreenId;
use crate::version_check::UpdateInfo;
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;

/// Main menu screen controller.
///
/// This screen wraps the existing `MainMenuComponent` and implements the
/// `Screen` trait to provide a cleaner interface for the app router.
pub struct MainMenuScreen {
    component: Option<MainMenuComponent>,
}

impl MainMenuScreen {
    /// Create a new main menu screen.
    ///
    /// Note: The component is not initialized until `init_with_config` is called.
    pub fn new() -> Self {
        Self { component: None }
    }

    /// Create and initialize with configuration.
    pub fn with_config(config: &Config, has_changes: bool) -> Self {
        Self {
            component: Some(MainMenuComponent::new(has_changes, config)),
        }
    }

    /// Initialize or reinitialize the screen with configuration.
    pub fn init_with_config(&mut self, config: &Config, has_changes: bool) {
        self.component = Some(MainMenuComponent::new(has_changes, config));
    }

    /// Get a reference to the component, panics if not initialized.
    fn component(&self) -> &MainMenuComponent {
        self.component.as_ref().expect("MainMenuScreen not initialized")
    }

    /// Get a mutable reference to the component, panics if not initialized.
    fn component_mut(&mut self) -> &mut MainMenuComponent {
        self.component.as_mut().expect("MainMenuScreen not initialized")
    }

    /// Render the main menu (simple wrapper for backward compatibility).
    pub fn render_frame(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        self.component_mut().render(frame, area)
    }

    /// Set update info to display update notification.
    pub fn set_update_info(&mut self, info: Option<UpdateInfo>) {
        if let Some(ref mut component) = self.component {
            component.set_update_info(info);
        }
    }

    /// Update whether there are changes to push.
    pub fn set_has_changes_to_push(&mut self, has_changes: bool) {
        if let Some(ref mut component) = self.component {
            component.set_has_changes_to_push(has_changes);
        }
    }

    /// Update the list of changed files for display.
    pub fn update_changed_files(&mut self, changed_files: Vec<String>) {
        if let Some(ref mut component) = self.component {
            component.update_changed_files(changed_files);
        }
    }

    /// Update configuration.
    pub fn update_config(&mut self, config: Config) {
        if let Some(ref mut component) = self.component {
            component.update_config(config);
        }
    }

    /// Get the selected index.
    pub fn selected_index(&self) -> usize {
        self.component().selected_index()
    }

    /// Move selection up.
    pub fn move_up(&mut self) {
        self.component_mut().move_up();
    }

    /// Move selection down.
    pub fn move_down(&mut self) {
        self.component_mut().move_down();
    }

    /// Handle a mouse event (for backward compatibility).
    pub fn handle_mouse_event(&mut self, event: Event) -> Result<ComponentAction> {
        self.component_mut().handle_event(event)
    }

    /// Get the currently selected menu item.
    pub fn selected_item(&self) -> MenuItem {
        self.component().selected_item()
    }

    /// Check if the update notification is selected.
    pub fn is_update_item_selected(&self) -> bool {
        self.component().is_update_item_selected()
    }

    /// Get update info if available.
    pub fn get_update_info(&self) -> Option<&UpdateInfo> {
        self.component().get_update_info()
    }

    /// Build the update message from UpdateInfo.
    fn build_update_message(info: &UpdateInfo) -> (String, String) {
        let title = format!("🎉 Version {} Available!", info.latest_version);
        let content = format!(
            "🎉 New version available: {} → {}\n\n\
            Update options:\n\n\
            1. Using install script:\n\
            curl -fsSL {} | bash\n\n\
            2. Using Cargo:\n\
            cargo install dotstate --force\n\n\
            3. Using Homebrew:\n\
            brew upgrade dotstate\n\n\
            Visit release page for details.",
            info.current_version,
            info.latest_version,
            info.release_url.replace("/releases/tag/", "/releases/download/").replace(&info.latest_version, &format!("{}/install.sh", info.latest_version))
        );
        (title, content)
    }

    /// Handle menu item selection and return the appropriate action.
    fn handle_selection(&self, ctx: &ScreenContext) -> Result<ScreenAction> {
        if self.is_update_item_selected() {
            // Return action to show update info popup
            if let Some(info) = self.get_update_info() {
                let (title, content) = Self::build_update_message(info);
                return Ok(ScreenAction::ShowMessage { title, content });
            }
        }

        let item = self.selected_item();
        let is_setup = ctx.config.repo_path.exists();

        // Check if item requires setup
        if item.requires_setup() && !is_setup {
            return Ok(ScreenAction::Navigate(ScreenId::GitHubAuth));
        }

        // Navigate based on selected item
        match item {
            MenuItem::ScanDotfiles => Ok(ScreenAction::Navigate(ScreenId::DotfileSelection)),
            MenuItem::SyncWithRemote => Ok(ScreenAction::Navigate(ScreenId::SyncWithRemote)),
            MenuItem::ManageProfiles => Ok(ScreenAction::Navigate(ScreenId::ManageProfiles)),
            MenuItem::ManagePackages => Ok(ScreenAction::Navigate(ScreenId::ManagePackages)),
            MenuItem::SetupRepository => Ok(ScreenAction::Navigate(ScreenId::GitHubAuth)),
        }
    }
}

impl Default for MainMenuScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for MainMenuScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) -> Result<()> {
        self.component_mut().render(frame, area)
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        // Handle keyboard events
        if let Event::Key(key) = &event {
            if key.kind == KeyEventKind::Press {
                // Use keymap from context
                if let Some(action) = ctx.config.keymap.get_action(key.code, key.modifiers) {
                    use crate::keymap::Action;
                    match action {
                        Action::MoveUp => {
                            self.component_mut().move_up();
                            return Ok(ScreenAction::None);
                        }
                        Action::MoveDown => {
                            self.component_mut().move_down();
                            return Ok(ScreenAction::None);
                        }
                        Action::Confirm => {
                            // Check for update item selection
                            if self.is_update_item_selected() {
                                if let Some(info) = self.get_update_info() {
                                    let (title, content) = Self::build_update_message(info);
                                    return Ok(ScreenAction::ShowMessage { title, content });
                                }
                            } else {
                                return self.handle_selection(ctx);
                            }
                        }
                        Action::Quit | Action::Cancel => {
                            return Ok(ScreenAction::Quit);
                        }
                        _ => {}
                    }
                }
            }
        }

        // Handle mouse events through component
        if matches!(event, Event::Mouse(_)) {
            let comp_action = self.component_mut().handle_event(event)?;
            match comp_action {
                ComponentAction::Update => {
                    // Mouse click triggers selection
                    return self.handle_selection(ctx);
                }
                ComponentAction::Custom(ref action_name) if action_name == "show_update_info" => {
                    if let Some(info) = self.get_update_info() {
                        let (title, content) = Self::build_update_message(info);
                        return Ok(ScreenAction::ShowMessage { title, content });
                    }
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        false // Main menu has no text inputs
    }

    fn on_enter(&mut self, ctx: &ScreenContext) -> Result<()> {
        // Re-initialize when entering the screen
        // has_changes will be set by the app
        self.init_with_config(ctx.config, false);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> Config {
        let mut config = Config::default();
        config.repo_path = PathBuf::from("/tmp/test-repo");
        config
    }

    #[test]
    fn test_main_menu_screen_creation() {
        let config = test_config();
        let screen = MainMenuScreen::with_config(&config, false);
        assert!(!screen.is_update_item_selected());
    }

    #[test]
    fn test_selected_item_default() {
        let mut config = test_config();
        // Mark as configured so default is ScanDotfiles
        config.github = Some(crate::config::GitHubConfig {
            owner: "testuser".to_string(),
            repo: "dotfiles".to_string(),
            token: Some("test-token".to_string()),
        });
        let screen = MainMenuScreen::with_config(&config, false);
        // Default should be first item (ScanDotfiles) when configured
        assert_eq!(screen.selected_item(), MenuItem::ScanDotfiles);
    }

    #[test]
    fn test_selected_item_unconfigured() {
        let config = test_config();
        // Not configured, so default should be SetupRepository
        let screen = MainMenuScreen::with_config(&config, false);
        assert_eq!(screen.selected_item(), MenuItem::SetupRepository);
    }
}
