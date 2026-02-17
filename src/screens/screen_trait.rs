//! Screen trait and associated types.
//!
//! This module defines the `Screen` trait which provides a cleaner alternative
//! to the existing `Component` trait. The key differences are:
//!
//! 1. Screens own their state instead of receiving it from outside
//! 2. Event handling returns an action instead of mutating external state
//! 3. Context objects provide read-only access to shared resources

use crate::config::Config;
use crate::ui::Screen as ScreenId;
use anyhow::Result;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::path::PathBuf;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Context provided for rendering screens.
///
/// This struct provides read-only access to resources needed for rendering,
/// such as syntax highlighting themes and configuration.
pub struct RenderContext<'a> {
    /// Application configuration.
    pub config: &'a Config,
    /// Syntax highlighting syntax set.
    pub syntax_set: &'a SyntaxSet,
    /// Syntax highlighting theme set.
    pub theme_set: &'a ThemeSet,
    /// Current syntax theme.
    pub syntax_theme: &'a Theme,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context.
    pub fn new(
        config: &'a Config,
        syntax_set: &'a SyntaxSet,
        theme_set: &'a ThemeSet,
        syntax_theme: &'a Theme,
    ) -> Self {
        Self {
            config,
            syntax_set,
            theme_set,
            syntax_theme,
        }
    }
}

/// Context provided for handling events.
///
/// This struct provides read access to configuration and other resources
/// needed for event handling.
pub struct ScreenContext<'a> {
    /// Application configuration.
    pub config: &'a Config,
    /// Configuration file path (for saving).
    pub config_path: &'a std::path::Path,
    /// Repository path.
    pub repo_path: &'a std::path::Path,
    /// Active profile name.
    pub active_profile: &'a str,
}

impl<'a> ScreenContext<'a> {
    /// Create a new screen context.
    #[must_use]
    pub fn new(config: &'a Config, config_path: &'a std::path::Path) -> Self {
        Self {
            config,
            config_path,
            repo_path: &config.repo_path,
            active_profile: &config.active_profile,
        }
    }
}

/// Actions that a screen can return after handling an event.
///
/// This enum allows screens to signal navigation and state changes without
/// directly mutating global state.
#[derive(Debug, Clone, Default)]
pub enum ScreenAction {
    /// No action needed, stay on current screen.
    #[default]
    None,
    /// Navigate to a different screen.
    Navigate(ScreenId),
    /// Navigate to a screen and pass data.
    NavigateWithMessage {
        screen: ScreenId,
        title: String,
        message: String,
    },
    /// Show a message popup.
    ShowMessage { title: String, content: String },
    /// Show a toast notification (non-blocking, auto-dismisses).
    ShowToast {
        message: String,
        variant: crate::widgets::ToastVariant,
    },
    /// Request to quit the application.
    Quit,
    /// Trigger a data refresh (e.g., reload dotfiles).
    Refresh,
    /// Mark that there are changes to push.
    SetHasChanges(bool),
    /// Update the config (signals app to reload).
    ConfigUpdated,
    /// Open help overlay.
    ShowHelp,
    /// Save local repository configuration and navigate to profile selection or main menu.
    SaveLocalRepoConfig {
        /// Path to the local repository.
        repo_path: PathBuf,
        /// List of profiles found in the repository (empty if none).
        profiles: Vec<String>,
    },
    /// Start the GitHub setup state machine.
    StartGitHubSetup {
        /// GitHub personal access token.
        token: String,
        /// Repository name.
        repo_name: String,
        /// Whether the repo should be private.
        is_private: bool,
    },
    /// Update the GitHub token only (for already configured repos).
    UpdateGitHubToken {
        /// New token to save.
        token: String,
    },
    /// Navigate to profile selection screen with profiles.
    ShowProfileSelection {
        /// List of profile names to choose from.
        profiles: Vec<String>,
    },
    /// Create a new profile and activate it (used during initial setup).
    CreateAndActivateProfile {
        /// Name of the profile to create.
        name: String,
    },
    /// Activate an existing profile (used during initial setup).
    ActivateProfile {
        /// Name of the profile to activate.
        name: String,
    },
    // Dotfile selection actions
    /// Scan for dotfiles and refresh the list.
    ScanDotfiles,
    /// Refresh the file browser entries.
    RefreshFileBrowser,
    /// Toggle file sync status (add or remove from sync).
    ToggleFileSync {
        /// Index of the file in the dotfiles list.
        file_index: usize,
        /// Whether the file is currently synced.
        is_synced: bool,
    },
    /// Add a custom file to sync after confirmation.
    AddCustomFileToSync {
        /// Full path to the file.
        full_path: PathBuf,
        /// Relative path (from home directory).
        relative_path: String,
    },
    /// Update backup enabled setting.
    SetBackupEnabled {
        /// Whether backups are enabled.
        enabled: bool,
    },
    /// Remove a custom file entry from the config (not synced, user-added).
    RemoveCustomFile {
        /// Index of the file in the dotfiles list.
        file_index: usize,
    },
    /// Move a file to/from common (shared across all profiles).
    MoveToCommon {
        /// Index of the file in the dotfiles list.
        file_index: usize,
        /// Whether the file is currently in common (true = move to profile, false = move to common).
        is_common: bool,
        /// Profiles that need cleanup (same file with same content) - only used when moving to common.
        profiles_to_cleanup: Vec<String>,
    },
    // Profile management actions
    /// Create a new profile.
    CreateProfile {
        /// Name of the new profile.
        name: String,
        /// Optional description.
        description: Option<String>,
        /// Index of profile to copy from.
        copy_from: Option<usize>,
    },
    /// Switch to a different profile by name.
    SwitchProfile {
        /// Name of the profile to switch to.
        name: String,
    },
    /// Rename a profile.
    RenameProfile {
        /// Current name of the profile.
        old_name: String,
        /// New name for the profile.
        new_name: String,
    },
    /// Delete a profile.
    DeleteProfile {
        /// Name of the profile to delete.
        name: String,
    },
    // Package management actions
    /// Trigger installation of all missing packages.
    InstallMissingPackages,
    // Settings actions
    /// Update a setting value.
    UpdateSetting {
        /// Name of the setting being updated.
        setting: String,
        /// Index of the selected option.
        option_index: usize,
    },
}

/// Result of processing a screen-specific action.
///
/// This allows screens to signal outcomes without directly modifying app state.
#[derive(Debug, Clone, Default)]
pub enum ActionResult {
    /// No result, action completed successfully
    #[default]
    None,
    /// Show a toast notification
    ShowToast {
        message: String,
        variant: crate::widgets::ToastVariant,
    },
    /// Show a modal dialog
    ShowDialog {
        title: String,
        content: String,
        variant: crate::widgets::DialogVariant,
    },
    /// Navigate to a different screen
    Navigate(ScreenId),
    /// Request config reload (after external modification)
    ConfigUpdated,
}

/// Trait for screen controllers.
///
/// This trait provides a cleaner alternative to the existing `Component` trait.
/// Screens that implement this trait own their state and handle both rendering
/// and events in a self-contained way.
///
/// # Example
///
/// ```rust,ignore
/// struct MyScreen {
///     state: MyState,
/// }
///
/// impl Screen for MyScreen {
///     fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
///         // Draw widgets
///         Ok(())
///     }
///
///     fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
///         match event {
///             Event::Key(key) => {
///                 // Handle key press
///                 Ok(ScreenAction::Navigate(Screen::MainMenu))
///             }
///             _ => Ok(ScreenAction::None),
///         }
///     }
/// }
/// ```
pub trait Screen {
    /// Render the screen.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to render to.
    /// * `area` - The area to render within.
    /// * `ctx` - Render context with shared resources.
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()>;

    /// Handle an input event.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to handle.
    /// * `ctx` - Screen context with configuration.
    ///
    /// # Returns
    ///
    /// An action indicating what should happen next.
    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction>;

    /// Check if a text input is currently focused.
    ///
    /// When true, navigation keybindings are disabled so users can type freely.
    fn is_input_focused(&self) -> bool {
        false
    }

    /// Called when the screen is entered (navigated to).
    ///
    /// This is useful for initializing state that depends on current config.
    fn on_enter(&mut self, _ctx: &ScreenContext) -> Result<()> {
        Ok(())
    }

    /// Called when the screen is exited (navigated away from).
    ///
    /// This is useful for cleanup or saving state.
    fn on_exit(&mut self, _ctx: &ScreenContext) -> Result<()> {
        Ok(())
    }
}
