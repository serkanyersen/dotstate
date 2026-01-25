use crate::config::{Config, GitHubConfig};
use crate::git::GitManager;
use crate::github::GitHubClient;
use crate::screens::{
    GitHubAuthScreen, MainMenuScreen, ManagePackagesScreen, ManageProfilesScreen,
    Screen as ScreenTrait, StorageSetupScreen, SyncWithRemoteScreen,
};
use crate::tui::Tui;
use crate::ui::{GitHubAuthStep, GitHubSetupStep, Screen, UiState};
use crate::widgets::{Dialog, DialogVariant, Toast, ToastManager};

use crate::screens::dotfile_selection::DotfileSelectionState;
use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use syntect::parsing::SyntaxSet;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tracing::{debug, error, info, trace, warn};
// Frame and Rect are used in function signatures but imported where needed

/// Count files in a directory (recursively)
/// List files and directories in a profile directory, returning relative paths from home
/// Files are stored in the repo as: repo_path/profile_name/.zshrc or repo_path/profile_name/.config/iTerm
/// We need to return them as: .zshrc or .config/iTerm (relative to home)
/// This function only lists top-level entries (files and directories), not recursively scanning directories.
/// This ensures that when a directory like .config/iTerm is synced, we symlink the directory itself,
/// not individual files inside it.
fn list_files_in_profile_dir(profile_dir: &Path, _repo_path: &Path) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    if profile_dir.is_dir() {
        for entry in fs::read_dir(profile_dir)? {
            let entry = entry?;
            let path = entry.path();
            // List both files and directories at the top level only
            if path.is_file() || path.is_symlink() || path.is_dir() {
                // Get relative path from profile directory
                if let Ok(relative) = path.strip_prefix(profile_dir) {
                    // Convert to string, handling the path properly
                    if let Some(relative_str) = relative.to_str() {
                        // Remove leading ./ if present
                        let clean_path = relative_str.strip_prefix("./").unwrap_or(relative_str);
                        entries.push(clean_path.to_string());
                    }
                }
            }
        }
    }
    Ok(entries)
}

/// State for showing a modal dialog
#[derive(Debug, Clone)]
struct DialogState {
    title: String,
    content: String,
    variant: DialogVariant,
}

/// Main application state
pub struct App {
    config: Config,
    config_path: PathBuf,
    tui: Tui,
    ui_state: UiState,
    should_quit: bool,
    runtime: Runtime,
    /// Track the last screen to detect screen transitions
    last_screen: Option<Screen>,
    /// Screen controllers (new architecture)
    main_menu_screen: MainMenuScreen,
    github_auth_screen: GitHubAuthScreen,
    storage_setup_screen: StorageSetupScreen,
    dotfile_selection_screen: crate::screens::DotfileSelectionScreen,
    sync_with_remote_screen: SyncWithRemoteScreen,
    profile_selection_screen: crate::screens::ProfileSelectionScreen,
    manage_profiles_screen: ManageProfilesScreen,
    manage_packages_screen: ManagePackagesScreen,
    settings_screen: crate::screens::SettingsScreen,
    /// Modal dialog state (for error messages, confirmations)
    dialog_state: Option<DialogState>,
    /// Toast notification manager for non-blocking notifications
    toast_manager: ToastManager,
    // Syntax highlighting assets
    syntax_set: SyntaxSet,
    theme_set: syntect::highlighting::ThemeSet,
    /// Track if we've checked for updates yet (deferred until after first render)
    has_checked_updates: bool,
    /// Receiver for async update check result (if check is in progress)
    /// Result is Ok(Some(UpdateInfo)) if update available, Ok(None) if no update, Err(String) if error
    update_check_receiver:
        Option<oneshot::Receiver<Result<Option<crate::version_check::UpdateInfo>, String>>>,
    /// Receiver for async git status check
    git_status_receiver: Option<oneshot::Receiver<crate::services::git_service::GitStatus>>,
    /// Last time git status was checked
    last_git_status_check: Option<std::time::Instant>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config_path = crate::utils::get_config_path();
        info!("Loading configuration from: {:?}", config_path);

        let config =
            Config::load_or_create(&config_path).context("Failed to load or create config")?;
        debug!(
            "Configuration loaded: active_profile={}, repo_path={:?}",
            config.active_profile, config.repo_path
        );

        let tui = Tui::new()?;
        let ui_state = UiState::new();

        let runtime = Runtime::new().context("Failed to create tokio runtime")?;

        // Initialize syntax highlighting
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();

        let has_changes = false; // Will be checked on first draw
        let _config_clone = config.clone();
        let main_menu_screen = MainMenuScreen::with_config(&config, has_changes);
        let app = Self {
            config_path,
            config,
            tui,
            ui_state,
            should_quit: false,
            runtime,
            last_screen: None,
            main_menu_screen,
            github_auth_screen: GitHubAuthScreen::new(),
            storage_setup_screen: StorageSetupScreen::new(),
            dotfile_selection_screen: crate::screens::DotfileSelectionScreen::new(),
            sync_with_remote_screen: SyncWithRemoteScreen::new(),
            profile_selection_screen: crate::screens::ProfileSelectionScreen::new(),
            manage_profiles_screen: ManageProfilesScreen::new(),
            manage_packages_screen: ManagePackagesScreen::new(),
            settings_screen: crate::screens::SettingsScreen::new(),

            dialog_state: None,
            toast_manager: ToastManager::new(),
            syntax_set,
            theme_set,
            has_checked_updates: false,
            update_check_receiver: None,
            git_status_receiver: None,
            last_git_status_check: None,
        };

        Ok(app)
    }

    pub fn run(&mut self) -> Result<()> {
        info!("Entering TUI mode");
        self.tui.enter()?;

        // Update check is deferred until after first render to avoid blocking startup
        // This allows the UI to appear immediately

        // Check if profile is deactivated and show warning
        if !self.config.profile_activated && self.config.is_repo_configured() {
            warn!("Profile '{}' is deactivated", self.config.active_profile);
            // Profile is deactivated - show warning message
            self.dialog_state = Some(DialogState {
                title: "Profile Deactivated".to_string(),
                content: format!(
                    "Your profile '{}' is currently deactivated.\n\n\
                    Your symlinks have been removed and original files restored.\n\n\
                    To reactivate your profile and restore symlinks, run:\n\n\
                    dotstate activate",
                    self.config.active_profile
                ),
                variant: DialogVariant::Warning,
            });
        }

        // Always start with main menu (which is now the welcome screen)
        self.ui_state.current_screen = Screen::MainMenu;
        // Set last_screen to None so first draw will detect the transition
        self.last_screen = None;
        info!("Starting main event loop");

        // Main event loop
        loop {
            self.draw()?;

            // Tick toast manager to remove expired toasts
            self.toast_manager.tick();

            // Start async update check after first render (non-blocking for UI)
            if !self.has_checked_updates
                && self.config.updates.check_enabled
                && self.update_check_receiver.is_none()
            {
                debug!("Spawning async update check (deferred until after first render)...");
                let (tx, rx) = oneshot::channel();
                thread::spawn(move || {
                    let result = crate::version_check::check_for_updates_with_result()
                        .map_err(|e| e.to_string());
                    // Ignore send error - receiver might be dropped if app quits
                    let _ = tx.send(result);
                });
                self.update_check_receiver = Some(rx);
            }

            // Check if update check result is ready (non-blocking)
            if let Some(receiver) = &mut self.update_check_receiver {
                match receiver.try_recv() {
                    Ok(Ok(Some(update_info))) => {
                        info!(
                            "New version available: {} -> {}",
                            update_info.current_version, update_info.latest_version
                        );
                        self.main_menu_screen.set_update_info(Some(update_info));
                        self.has_checked_updates = true;
                        self.update_check_receiver = None;
                    }
                    Ok(Ok(None)) => {
                        debug!("Update check completed: No updates available");
                        self.has_checked_updates = true;
                        self.update_check_receiver = None;
                    }
                    Ok(Err(e)) => {
                        debug!("Update check failed: {}", e);
                        self.has_checked_updates = true;
                        self.update_check_receiver = None;
                    }
                    Err(oneshot::error::TryRecvError::Empty) => {
                        // Still in progress, continue event loop
                    }
                    Err(oneshot::error::TryRecvError::Closed) => {
                        // Sender was dropped (shouldn't happen, but handle gracefully)
                        warn!("Update check channel closed unexpectedly");
                        self.has_checked_updates = true;
                        self.update_check_receiver = None;
                    }
                }
            }

            // Check for git status update (non-blocking)
            if let Some(receiver) = &mut self.git_status_receiver {
                match receiver.try_recv() {
                    Ok(status) => {
                        trace!("Git status update received");
                        // Update UI state
                        self.ui_state.git_status = Some(status.clone());
                        self.ui_state.has_changes_to_push = status.has_changes;

                        // Update changed files for sync screen and main menu (implicit via ui_state.git_status)
                        // But we also need to update the sync screen state directly if needed, or better,
                        // let the draw loop pick it up from UiState.
                        // Currently MainMenu checks syncing status in draw().

                        // We update the screen state's version of changed_files for compatibility
                        let sync_state = self.sync_with_remote_screen.get_state_mut();
                        sync_state.changed_files = status.uncommitted_files.clone();
                        sync_state.git_status = Some(status.clone());

                        self.git_status_receiver = None;
                        self.last_git_status_check = Some(std::time::Instant::now());
                    }
                    Err(oneshot::error::TryRecvError::Empty) => {} // Still running
                    Err(_) => {
                        self.git_status_receiver = None; // Failed or cancelled
                    }
                }
            }

            if self.should_quit {
                break;
            }

            // Process GitHub setup state machine if active (before polling events)
            // Check new storage setup screen first, fall back to old screen for legacy
            if self.storage_setup_screen.needs_tick() {
                self.process_storage_setup_step()?;
            } else if self.github_auth_screen.needs_tick() {
                self.process_github_setup_step()?;
            }

            // Process package checking and installation (managed by screen)
            // We call tick() on the manage_packages_screen to handle background tasks
            match self.manage_packages_screen.tick() {
                Ok(crate::screens::ScreenAction::Refresh) => {
                    // Redraw requested by tick (e.g. progress update)
                    // self.draw() happens next loop anyway if we don't block.
                    // But poll_event blocks.
                    // We rely on the poll timeout (250ms) to allow redraws.
                }
                Ok(action) => self.process_screen_action(action)?,
                Err(e) => error!("Error in package manager tick: {}", e),
            }

            // Poll for events with 250ms timeout
            if let Some(event) = self.tui.poll_event(Duration::from_millis(250))? {
                trace!("Event received: {:?}", event);
                if let Err(e) = self.handle_event(event) {
                    error!("Error handling event: {}", e);
                    return Err(e);
                }
                // Sync input mode based on current focus states
                self.sync_input_mode();
            }
        }

        info!("Exiting TUI");
        self.tui.exit()?;
        Ok(())
    }

    /// Cycle through themes: dark -> light -> nocolor -> midnight -> dark
    fn cycle_theme(&mut self) -> Result<()> {
        use crate::styles::ThemeType;

        let current_theme = self
            .config
            .theme
            .parse::<ThemeType>()
            .unwrap_or(ThemeType::Dark);

        // Cycle through themes: dark -> light -> solarized-dark -> solarized-light -> nocolor -> midnight -> dark
        let next_theme = match current_theme {
            ThemeType::Dark => ThemeType::Light,
            ThemeType::Light => ThemeType::SolarizedDark,
            ThemeType::SolarizedDark => ThemeType::SolarizedLight,
            ThemeType::SolarizedLight => ThemeType::NoColor,
            ThemeType::NoColor => ThemeType::Midnight,
            ThemeType::Midnight => ThemeType::Dark,
        };

        // Update config
        self.config.theme = match next_theme {
            ThemeType::Dark => "dark".to_string(),
            ThemeType::Light => "light".to_string(),
            ThemeType::SolarizedDark => "solarized-dark".to_string(),
            ThemeType::SolarizedLight => "solarized-light".to_string(),
            ThemeType::NoColor => "nocolor".to_string(),
            ThemeType::Midnight => "midnight".to_string(),
        };

        // Update NO_COLOR environment variable based on theme
        // This allows colors to be restored when cycling from nocolor to a color theme
        match next_theme {
            ThemeType::NoColor => {
                std::env::set_var("NO_COLOR", "1");
                info!("NO_COLOR environment variable set");
            }
            ThemeType::Dark
            | ThemeType::Light
            | ThemeType::SolarizedDark
            | ThemeType::SolarizedLight
            | ThemeType::Midnight => {
                // Unset NO_COLOR to allow colors
                // Note: Some libraries may have already checked NO_COLOR at startup,
                // but unsetting it allows future checks to see colors are enabled
                std::env::remove_var("NO_COLOR");
                info!("NO_COLOR environment variable removed");
            }
        }

        // Re-initialize theme
        crate::styles::init_theme(next_theme);
        info!("Theme changed to: {:?}", next_theme);

        // Save config
        if let Err(e) = self.config.save(&self.config_path) {
            warn!("Failed to save theme change: {}", e);
        } else {
            info!("Theme saved to config: {}", self.config.theme);
        }

        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        // Check for screen transitions and update state accordingly
        let current_screen = self.ui_state.current_screen;
        if self.last_screen != Some(current_screen) {
            // Screen changed - log the transition
            debug!(
                "Screen transition: {:?} -> {:?}",
                self.last_screen, current_screen
            );
            // Screen changed - check for changes when entering MainMenu
            if current_screen == Screen::MainMenu {
                self.trigger_git_status_check(true);
            }
            // Handle ManagePackages screen transitions
            if current_screen == Screen::ManagePackages {
                // Load packages from active profile first (before mutable borrow)
                let packages = self
                    .get_active_profile_info()
                    .ok()
                    .flatten()
                    .map(|p| p.packages.clone())
                    .unwrap_or_default();

                self.manage_packages_screen
                    .update_packages(packages, &self.config.active_profile);
            } else if self.last_screen == Some(Screen::ManagePackages) {
                // We just left ManagePackages - clear installation state to prevent it from showing elsewhere
                self.manage_packages_screen.reset_state();
            }

            // Handle ManageProfiles screen transitions - refresh cached profiles
            if current_screen == Screen::ManageProfiles {
                if let Err(e) = self
                    .manage_profiles_screen
                    .refresh_profiles(&self.config.repo_path)
                {
                    error!("Failed to refresh profiles: {}", e);
                }
            }
            self.last_screen = Some(current_screen);
        }

        // Update components with current state
        if self.ui_state.current_screen == Screen::MainMenu {
            self.main_menu_screen
                .set_git_status(self.ui_state.git_status.clone());
        }

        // DotfileSelectionScreen handles its own state and rendering

        // Load changed files when entering PushChanges screen
        if self.ui_state.current_screen == Screen::SyncWithRemote
            && !self.sync_with_remote_screen.get_state().is_syncing
        {
            // Only load if we don't have files yet
            if self
                .sync_with_remote_screen
                .get_state()
                .changed_files
                .is_empty()
            {
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                self.sync_with_remote_screen.load_changed_files(&ctx);
            }
        }

        // Clone config for main menu to avoid borrow issues in closure
        let config_clone = self.config.clone();

        self.tui.terminal_mut().draw(|frame| {
            let area = frame.area();
            match self.ui_state.current_screen {
                Screen::MainMenu => {
                    // Pass config to main menu for stats
                    self.main_menu_screen.update_config(config_clone.clone());
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.main_menu_screen.render(frame, area, &ctx) {
                        error!("Failed to render main menu screen: {}", e);
                    }
                }
                Screen::GitHubAuth => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.github_auth_screen.render(frame, area, &ctx) {
                        error!("Failed to render GitHubAuth screen: {}", e);
                    }
                    // Sync state back after render
                    self.ui_state.github_auth = self.github_auth_screen.get_auth_state().clone();
                }
                Screen::StorageSetup => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.storage_setup_screen.render(frame, area, &ctx) {
                        error!("Failed to render StorageSetup screen: {}", e);
                    }
                }
                Screen::DotfileSelection => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.dotfile_selection_screen.render(frame, area, &ctx) {
                        error!("Failed to render dotfile selection screen: {}", e);
                    }
                }
                Screen::SyncWithRemote => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.sync_with_remote_screen.render(frame, area, &ctx) {
                        error!("Failed to render sync with remote screen: {}", e);
                    }
                }
                Screen::ManageProfiles => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.manage_profiles_screen.render(frame, area, &ctx) {
                        error!("Failed to render manage profiles screen: {}", e);
                    }
                }
                Screen::ManagePackages => {
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.manage_packages_screen.render(frame, area, &ctx) {
                        error!("Failed to render manage packages screen: {}", e);
                    }
                }
                Screen::ProfileSelection => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &self.config,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.profile_selection_screen.render(frame, area, &ctx) {
                        error!("Failed to render profile selection screen: {}", e);
                    }
                }
                Screen::Settings => {
                    // Router pattern - delegate to screen's render method
                    use crate::screens::{RenderContext, Screen as ScreenTrait};
                    let syntax_theme = crate::utils::get_current_syntax_theme(&self.theme_set);
                    let ctx = RenderContext::new(
                        &config_clone,
                        &self.syntax_set,
                        &self.theme_set,
                        syntax_theme,
                    );
                    if let Err(e) = self.settings_screen.render(frame, area, &ctx) {
                        error!("Failed to render settings screen: {}", e);
                    }
                }
            }

            // Render dialog on top of screen content (modal overlay)
            if let Some(ref dialog) = self.dialog_state {
                let footer = "Press any key to continue";
                let dlg = Dialog::new(&dialog.title, &dialog.content)
                    .variant(dialog.variant)
                    .height(30)
                    .footer(footer);
                frame.render_widget(dlg, area);
            }

            // Render toast notifications (non-blocking, on top of content but below help overlay)
            self.toast_manager.render(frame, area);

            // Render help overlay on top of everything if active
            if self.ui_state.show_help_overlay {
                let config_path = self.config_path.to_string_lossy().to_string();
                let _ = crate::components::help_overlay::HelpOverlay::render(
                    frame,
                    area,
                    &self.config.keymap,
                    &config_path,
                );
            }
        })?;
        Ok(())
    }

    /// Sync input_mode_active based on current focus states
    /// Called after event handling to keep input mode in sync with field focus
    fn sync_input_mode(&mut self) {
        use crate::ui::Screen;

        let is_input_focused = match self.ui_state.current_screen {
            // GitHub Auth - check if editing text fields
            Screen::GitHubAuth => {
                self.ui_state.github_auth.input_focused
                    && matches!(
                        self.ui_state.github_auth.focused_field,
                        crate::ui::GitHubAuthField::Token
                            | crate::ui::GitHubAuthField::RepoName
                            | crate::ui::GitHubAuthField::RepoLocation
                    )
            }

            // Dotfile Selection - file browser path input
            Screen::DotfileSelection => {
                use crate::screens::Screen as ScreenTrait;
                self.dotfile_selection_screen.is_input_focused()
            }

            // Profile Selection - create popup name input
            Screen::ProfileSelection => {
                use crate::screens::Screen as ScreenTrait;
                self.profile_selection_screen.is_input_focused()
            }

            // Manage Profiles - delegated to screen
            Screen::ManageProfiles => {
                use crate::screens::Screen as ScreenTrait;
                self.manage_profiles_screen.is_input_focused()
            }

            // Package Manager - add/edit/delete popups with text input
            Screen::ManagePackages => {
                use crate::screens::Screen as ScreenTrait;
                self.manage_packages_screen.is_input_focused()
            }

            // Storage Setup - form has text input
            Screen::StorageSetup => {
                use crate::screens::Screen as ScreenTrait;
                self.storage_setup_screen.is_input_focused()
            }

            // Other screens don't have text input
            _ => false,
        };

        self.ui_state.input_mode_active = is_input_focused;
    }

    /// Get the action for a key event using the configured keymap
    /// Returns None if in input mode and the action is a navigation action
    fn get_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<crate::keymap::Action> {
        use crate::keymap::Action;

        let action = self.config.keymap.get_action(code, modifiers)?;

        // In input mode, only allow certain essential actions
        if self.ui_state.input_mode_active {
            match action {
                // Always allowed even in input mode
                Action::Cancel
                | Action::Confirm
                | Action::NextTab
                | Action::PrevTab
                | Action::Help
                // Text editing actions
                | Action::Backspace
                | Action::DeleteChar
                | Action::Home
                | Action::End
                | Action::MoveLeft
                | Action::MoveRight => Some(action),
                // Navigation actions allowed when Manager field is focused (handled per-screen)
                Action::MoveUp | Action::MoveDown => {
                    // Allow in input mode - individual screens will decide based on context
                    Some(action)
                }
                // Block other actions while typing
                _ => None,
            }
        } else {
            Some(action)
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        // Sync input mode at the start so global handlers know current focus state
        self.sync_input_mode();

        // Global keymap-based handlers (help overlay, theme cycling)
        if let Event::Key(key) = &event {
            if key.kind == KeyEventKind::Press {
                use crate::keymap::Action;
                use crossterm::event::KeyCode;

                // Theme cycling with 't' key (global, but skip if in input field)
                if key.code == KeyCode::Char('t') && key.modifiers.is_empty() {
                    // Don't cycle theme if user is typing in an input field - let 't' be used as input
                    if !self.ui_state.input_mode_active {
                        self.cycle_theme()?;
                        return Ok(());
                    }
                    // If in input mode, fall through to let 't' be processed as text input
                }

                if let Some(action) = self.get_action(key.code, key.modifiers) {
                    if action == Action::Help {
                        // Toggle help overlay
                        self.ui_state.show_help_overlay = !self.ui_state.show_help_overlay;
                        return Ok(());
                    }
                }
            }
        }

        // Handle help overlay interactions
        if self.ui_state.show_help_overlay
            && matches!(event, Event::Key(k) if k.kind == KeyEventKind::Press)
        {
            use crossterm::event::KeyCode;
            if let Event::Key(key) = event {
                // Allow preset switching with 1/2/3 keys
                let new_preset = match key.code {
                    KeyCode::Char('1') => Some(crate::keymap::KeymapPreset::Standard),
                    KeyCode::Char('2') => Some(crate::keymap::KeymapPreset::Vim),
                    KeyCode::Char('3') => Some(crate::keymap::KeymapPreset::Emacs),
                    _ => None,
                };

                if let Some(preset) = new_preset {
                    if self.config.keymap.preset != preset {
                        info!(
                            "Switching keymap preset from {:?} to {:?}",
                            self.config.keymap.preset, preset
                        );
                        self.config.keymap.preset = preset;
                        // Save config immediately
                        if let Err(e) = self.config.save(&self.config_path) {
                            warn!("Failed to save preset change: {}", e);
                        } else {
                            info!("Keymap preset changed to {:?}", preset);
                        }
                    }
                    // Don't close overlay when switching preset
                    return Ok(());
                }

                // Any other key closes the overlay
                self.ui_state.show_help_overlay = false;
                return Ok(());
            }
        }

        // Handle dialog events - any key dismisses it
        if self.dialog_state.is_some() {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    self.dialog_state = None;
                }
            }
            return Ok(());
        }

        // Let components handle events first (for mouse support)
        match self.ui_state.current_screen {
            Screen::MainMenu => {
                // Router pattern - delegate to screen's handle_event method
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.main_menu_screen.handle_event(event, &ctx)?;

                // Sync selected_index from screen component
                self.ui_state.selected_index = self.main_menu_screen.selected_index();

                // Handle menu-specific navigation logic before processing action
                if let crate::screens::ScreenAction::Navigate(target) = &action {
                    self.handle_menu_navigation(*target)?;
                }

                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::GitHubAuth => {
                // Router pattern - delegate to screen's handle_event method
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.github_auth_screen.handle_event(event, &ctx)?;

                // Sync state from screen back to ui_state (for legacy code that reads it)
                self.ui_state.github_auth = self.github_auth_screen.get_auth_state().clone();

                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::StorageSetup => {
                // Router pattern - delegate to screen's handle_event method
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.storage_setup_screen.handle_event(event, &ctx)?;
                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::SyncWithRemote => {
                // Router pattern - delegate to screen's handle_event method
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.sync_with_remote_screen.handle_event(event, &ctx)?;

                // Handle navigation actions that require app-level logic
                if let crate::screens::ScreenAction::Navigate(Screen::MainMenu) = &action {
                    // Reset screen state and check for changes after sync
                    self.sync_with_remote_screen.reset_state();
                    // Force a check since we just synced
                    self.trigger_git_status_check(true);
                }

                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::DotfileSelection => {
                // Router pattern - delegate to screen's handle_event method
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.dotfile_selection_screen.handle_event(event, &ctx)?;
                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::ProfileSelection => {
                // Router pattern - delegate to screen's handle_event method
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.profile_selection_screen.handle_event(event, &ctx)?;
                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::ManagePackages => {
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.manage_packages_screen.handle_event(event, &ctx)?;
                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::ManageProfiles => {
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.manage_profiles_screen.handle_event(event, &ctx)?;
                self.process_screen_action(action)?;
                Ok(())
            }
            Screen::Settings => {
                use crate::screens::ScreenContext;
                let ctx = ScreenContext::new(&self.config, &self.config_path);
                let action = self.settings_screen.handle_event(event, &ctx)?;
                self.process_screen_action(action)?;
                Ok(())
            }
        }
    }

    /// Trigger an async check for git status/updates
    ///
    /// # Arguments
    /// * `force` - If true, ignore rate limiting and force a check
    fn trigger_git_status_check(&mut self, force: bool) {
        // Don't spawn if already running
        if self.git_status_receiver.is_some() {
            return;
        }

        // Rate limit checks (unless forced)
        if !force {
            if let Some(last_check) = self.last_git_status_check {
                // Check at most every 30 seconds automatically
                if last_check.elapsed() < Duration::from_secs(30) {
                    return;
                }
            }
        }

        debug!("Triggering async git status check (force={})", force);
        let config_clone = self.config.clone();
        let (tx, rx) = oneshot::channel();

        // Spawn on thread
        thread::spawn(move || {
            let status =
                crate::services::git_service::GitService::fetch_and_check_status(&config_clone);
            // Ignore send error
            let _ = tx.send(status);
        });

        self.git_status_receiver = Some(rx);
    }

    /// Handle navigation-specific logic when navigating from MainMenu
    fn handle_menu_navigation(&mut self, target: Screen) -> Result<()> {
        match target {
            Screen::DotfileSelection => {
                // Check for changes when returning to menu
                self.trigger_git_status_check(false);
                // Get screen state and update it directly
                let state = self.dotfile_selection_screen.get_state_mut();
                state.status_message = None;
                state.backup_enabled = self.config.backup_enabled;
                Self::scan_dotfiles_into(&self.config, state)?;
            }
            Screen::GitHubAuth => {
                // Setup git repository
                let is_configured = self.config.is_repo_configured();

                // Initialize auth state with current config values
                if is_configured {
                    self.ui_state.github_auth.repo_already_configured = true;
                    self.ui_state.github_auth.is_editing_token = false;
                    self.ui_state.github_auth.token_input = crate::utils::TextInput::new(); // Clear for security
                    self.ui_state.github_auth.repo_name_input =
                        crate::utils::TextInput::with_text(self.config.repo_name.clone());
                    self.ui_state.github_auth.repo_location_input =
                        crate::utils::TextInput::with_text(
                            self.config.repo_path.to_string_lossy().to_string(),
                        );
                    self.ui_state.github_auth.local_repo_path_input =
                        crate::utils::TextInput::with_text(
                            self.config.repo_path.to_string_lossy().to_string(),
                        );
                    self.ui_state.github_auth.is_private = true; // Default to private
                                                                 // Set setup mode based on config
                    self.ui_state.github_auth.setup_mode = match self.config.repo_mode {
                        crate::config::RepoMode::GitHub => crate::ui::SetupMode::GitHub,
                        crate::config::RepoMode::Local => crate::ui::SetupMode::Local,
                    };
                } else {
                    self.ui_state.github_auth.repo_already_configured = false;
                    self.ui_state.github_auth.is_editing_token = false;
                    self.ui_state.github_auth.setup_mode = crate::ui::SetupMode::Choosing;
                    self.ui_state.github_auth.mode_selection_index = 0;
                }

                // Sync the initialized state to the screen controller once
                *self.github_auth_screen.get_auth_state_mut() = self.ui_state.github_auth.clone();
            }
            Screen::SyncWithRemote => {
                // Reset sync screen state
                self.sync_with_remote_screen.reset_state();
                // Trigger git status check to fetch ahead/behind commits
                self.trigger_git_status_check(true);
            }

            Screen::ManagePackages => {
                // Only update packages if the profile has changed, to avoid interrupting
                // any background checks or clearing state unnecessarily.
                if self.config.active_profile != self.manage_packages_screen.state.active_profile {
                    // Load packages from active profile into screen state
                    if let Ok(Some(active_profile)) = self.get_active_profile_info() {
                        self.manage_packages_screen
                            .update_packages(active_profile.packages, &self.config.active_profile);
                    } else {
                        self.manage_packages_screen
                            .update_packages(Vec::new(), &self.config.active_profile);
                    }
                }
            }
            Screen::StorageSetup => {
                // Reset the screen state when entering
                self.storage_setup_screen.reset();
            }
            _ => {}
        }
        Ok(())
    }

    /// Process a ScreenAction returned from a screen's handle_event method.
    fn process_screen_action(&mut self, action: crate::screens::ScreenAction) -> Result<()> {
        use crate::screens::ScreenAction;
        match action {
            ScreenAction::None => {
                // No action needed
            }
            ScreenAction::Navigate(target) => {
                self.ui_state.current_screen = target;
                // Call on_enter for the target screen
                self.call_on_enter(target)?;
            }
            ScreenAction::NavigateWithMessage {
                screen,
                title: _,
                message: _,
            } => {
                // TODO: Show message and navigate
                self.ui_state.current_screen = screen;
            }
            ScreenAction::ShowMessage { title, content } => {
                // Show message popup using Dialog
                self.dialog_state = Some(DialogState {
                    title,
                    content,
                    variant: DialogVariant::Error,
                });
            }
            ScreenAction::ShowToast { message, variant } => {
                // Show non-blocking toast notification
                self.toast_manager.push(Toast::new(message, variant));
            }
            ScreenAction::Quit => {
                self.should_quit = true;
            }
            ScreenAction::Refresh => {
                // Trigger a redraw
            }
            ScreenAction::InstallMissingPackages => {
                self.manage_packages_screen
                    .start_installing_missing_packages();
            }
            ScreenAction::UpdateSetting {
                setting,
                option_index,
            } => {
                // Apply the setting change using the same logic from SettingsScreen
                let changed = self.settings_screen.apply_setting_to_config(
                    &mut self.config,
                    &setting,
                    option_index,
                );
                if changed {
                    // Save config
                    if let Err(e) = self.config.save(&self.config_path) {
                        error!("Failed to save config after settings change: {}", e);
                    }
                }
            }
            ScreenAction::SetHasChanges(has_changes) => {
                self.ui_state.has_changes_to_push = has_changes;
            }
            ScreenAction::ConfigUpdated => {
                // Reload config if needed
            }
            ScreenAction::ShowHelp => {
                self.ui_state.show_help_overlay = true;
            }
            ScreenAction::SaveLocalRepoConfig {
                repo_path,
                profiles,
            } => {
                // Save local repo configuration
                self.config.repo_mode = crate::config::RepoMode::Local;
                self.config.repo_path = repo_path.clone();
                self.config.github = None;

                if let Err(e) = self.config.save(&self.config_path) {
                    self.storage_setup_screen.get_state_mut().error_message =
                        Some(format!("Failed to save config: {}", e));
                    return Ok(());
                }

                // Verify git repository can be opened
                if let Err(e) = crate::git::GitManager::open_or_init(&repo_path) {
                    self.storage_setup_screen.get_state_mut().error_message =
                        Some(format!("Failed to open repository: {}", e));
                    return Ok(());
                }

                if profiles.is_empty() {
                    // No profiles, create default and go to main menu
                    self.config.active_profile = "default".to_string();
                    let _ = self.config.save(&self.config_path);
                    self.storage_setup_screen.reset();
                    self.main_menu_screen.update_config(self.config.clone());
                    self.ui_state.current_screen = Screen::MainMenu;
                } else {
                    // Show profile selection
                    self.ui_state.profile_selection.profiles = profiles.clone();
                    self.ui_state.profile_selection.list_state.select(Some(0));

                    // Update the screen controller state as well
                    self.profile_selection_screen.set_profiles(profiles);

                    self.storage_setup_screen.reset();
                    self.ui_state.current_screen = Screen::ProfileSelection;
                }
            }
            ScreenAction::StartGitHubSetup {
                token,
                repo_name,
                is_private,
            } => {
                // Initialize the GitHub setup state machine
                use crate::screens::storage_setup::StorageSetupStep;
                use crate::ui::GitHubSetupData;
                use std::time::Duration;

                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Processing(GitHubSetupStep::Connecting);
                state.status_message = Some("🔌 Connecting to GitHub...".to_string());
                state.setup_data = Some(GitHubSetupData {
                    token,
                    repo_name,
                    username: None,
                    repo_exists: None,
                    is_private,
                    delay_until: Some(std::time::Instant::now() + Duration::from_millis(500)),
                    is_new_repo: false,
                });
            }
            ScreenAction::UpdateGitHubToken { token } => {
                // Update the GitHub token with validation and remote URL update
                let github_config = match &self.config.github {
                    Some(gh) => gh.clone(),
                    None => {
                        self.storage_setup_screen.get_state_mut().error_message =
                            Some("No GitHub configuration to update".to_string());
                        return Ok(());
                    }
                };

                // Show validating status
                self.storage_setup_screen.get_state_mut().status_message =
                    Some("Validating token access to repository...".to_string());

                // Validate the token by checking repo access (not user info)
                // This works with scoped tokens that only have repo access
                let owner = github_config.owner.clone();
                let repo = github_config.repo.clone();
                let validation_result = self.runtime.block_on(async {
                    let client = crate::github::GitHubClient::new(token.clone());
                    client.repo_exists(&owner, &repo).await
                });

                match validation_result {
                    Ok(exists) => {
                        if !exists {
                            self.storage_setup_screen.get_state_mut().error_message =
                                Some(format!("Token cannot access repository {}/{}", owner, repo));
                            return Ok(());
                        }

                        // Token can access repo - update config
                        if let Some(ref mut github) = self.config.github {
                            github.token = Some(token.clone());
                        }

                        // Update the git remote URL with new token
                        if self.config.repo_path.exists() {
                            match crate::git::GitManager::open_or_init(&self.config.repo_path) {
                                Ok(mut git_manager) => {
                                    if let Err(e) =
                                        git_manager.update_remote_token("origin", &token)
                                    {
                                        // Non-fatal: log warning but continue
                                        warn!("Failed to update remote URL with new token: {}", e);
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to open git repository to update token: {}", e);
                                }
                            }
                        }

                        // Save config
                        if let Err(e) = self.config.save(&self.config_path) {
                            self.storage_setup_screen.get_state_mut().error_message =
                                Some(format!("Failed to save token: {}", e));
                            return Ok(());
                        }

                        // Show success and reset
                        self.storage_setup_screen.get_state_mut().status_message =
                            Some(format!("✅ Token updated for {}/{}", owner, repo));
                        self.storage_setup_screen.get_state_mut().is_editing_token = false;
                        self.storage_setup_screen.get_state_mut().token_input =
                            crate::utils::TextInput::with_text("••••••••••••••••••••");
                    }
                    Err(e) => {
                        self.storage_setup_screen.get_state_mut().error_message =
                            Some(format!("Token validation failed: {}", e));
                    }
                }
            }
            ScreenAction::ShowProfileSelection { profiles } => {
                self.profile_selection_screen.set_profiles(profiles.clone());
                // Also update ui_state for legacy code
                self.ui_state.profile_selection.profiles = profiles;
                self.ui_state.profile_selection.list_state.select(Some(0));
                self.ui_state.current_screen = Screen::ProfileSelection;
            }
            ScreenAction::CreateAndActivateProfile { name } => {
                // Create a new profile and activate it
                use crate::services::ProfileService;
                match ProfileService::create_profile(&self.config.repo_path, &name, None, None) {
                    Ok(_) => {
                        // Activate the newly created profile
                        if let Err(e) = self.activate_profile_after_setup(&name) {
                            error!("Failed to activate profile: {}", e);
                            self.dialog_state = Some(DialogState {
                                title: "Activation Failed".to_string(),
                                content: e.to_string(),
                                variant: DialogVariant::Error,
                            });
                        } else {
                            self.profile_selection_screen.reset();
                            self.ui_state.profile_selection = Default::default();
                            self.ui_state.current_screen = Screen::MainMenu;
                        }
                    }
                    Err(e) => {
                        error!("Failed to create profile: {}", e);
                        self.dialog_state = Some(DialogState {
                            title: "Creation Failed".to_string(),
                            content: format!("Failed to create profile: {}", e),
                            variant: DialogVariant::Error,
                        });
                    }
                }
            }
            ScreenAction::ActivateProfile { name } => {
                // Activate an existing profile
                if let Err(e) = self.activate_profile_after_setup(&name) {
                    error!("Failed to activate profile: {}", e);
                    self.dialog_state = Some(DialogState {
                        title: "Activation Failed".to_string(),
                        content: e.to_string(),
                        variant: DialogVariant::Error,
                    });
                } else {
                    self.profile_selection_screen.reset();
                    self.ui_state.profile_selection = Default::default();
                    self.ui_state.current_screen = Screen::MainMenu;
                }
            }
            // Dotfile selection actions
            ScreenAction::ScanDotfiles => {
                let state = self.dotfile_selection_screen.get_state_mut();
                Self::scan_dotfiles_into(&self.config, state)?;
            }
            ScreenAction::RefreshFileBrowser => {
                let state = self.dotfile_selection_screen.get_state_mut();
                Self::refresh_file_browser_into(&self.config, state)?;
            }
            ScreenAction::ToggleFileSync {
                file_index,
                is_synced,
            } => {
                let state = self.dotfile_selection_screen.get_state_mut();
                let dotfile = state.dotfiles.get(file_index);
                let filename = dotfile
                    .map(|d| d.relative_path.to_string_lossy().to_string())
                    .unwrap_or_default();
                let is_common = dotfile.map(|d| d.is_common).unwrap_or(false);

                if is_synced {
                    // Check if trying to unsync a common file
                    if is_common {
                        self.dialog_state = Some(DialogState {
                            title: "Cannot Unsync Common File".to_string(),
                            content: format!(
                                "\"{}\" is a common file shared across all profiles.\n\n\
                                To remove it from sync, first move it to your profile \
                                using the 'Move to Profile' action, then unsync it.",
                                filename
                            ),
                            variant: DialogVariant::Warning,
                        });
                    } else {
                        Self::remove_file_from_sync_with_state(&self.config, state, file_index)?;
                        self.toast_manager.success(format!("Removed: {}", filename));
                    }
                } else {
                    Self::add_file_to_sync_with_state(&self.config, state, file_index)?;
                    // Check if there was an error (status_message was set)
                    if let Some(error_msg) = state.status_message.take() {
                        self.dialog_state = Some(DialogState {
                            title: "Sync Failed".to_string(),
                            content: error_msg,
                            variant: DialogVariant::Error,
                        });
                    } else {
                        self.toast_manager.success(format!("Added: {}", filename));
                    }
                }
            }
            ScreenAction::AddCustomFileToSync {
                full_path,
                relative_path,
            } => {
                // Handle custom file sync - may update config
                self.handle_add_custom_file_to_sync(full_path, relative_path)?;
            }
            ScreenAction::SetBackupEnabled { enabled } => {
                self.config.backup_enabled = enabled;
                self.config.save(&self.config_path)?;
            }
            ScreenAction::CreateProfile {
                name,
                description,
                copy_from,
            } => {
                use crate::services::ProfileService;
                match ProfileService::create_profile(
                    &self.config.repo_path,
                    &name,
                    description,
                    copy_from,
                ) {
                    Ok(_) => {
                        // Reload config - but create_profile doesn't affect config.toml
                        self.config = crate::config::Config::load_or_create(&self.config_path)?;
                        // Refresh profiles in screen
                        if let Err(e) = self
                            .manage_profiles_screen
                            .refresh_profiles(&self.config.repo_path)
                        {
                            error!("Failed to refresh profiles after creation: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to create profile: {}", e);
                        self.dialog_state = Some(DialogState {
                            title: "Profile Creation Failed".to_string(),
                            content: format!("Failed to create profile '{}':\n\n{}", name, e),
                            variant: DialogVariant::Error,
                        });
                    }
                }
            }
            ScreenAction::SwitchProfile { name } => {
                if let Err(e) = self.switch_profile(&name) {
                    error!("Failed to switch profile: {}", e);
                    self.dialog_state = Some(DialogState {
                        title: "Switch Profile Failed".to_string(),
                        content: format!("Failed to switch to profile '{}':\n\n{}", name, e),
                        variant: DialogVariant::Error,
                    });
                }
            }
            ScreenAction::RenameProfile { old_name, new_name } => {
                if let Err(e) = self.rename_profile(&old_name, &new_name) {
                    error!("Failed to rename profile: {}", e);
                    self.dialog_state = Some(DialogState {
                        title: "Rename Failed".to_string(),
                        content: format!("Failed to rename profile '{}':\n\n{}", old_name, e),
                        variant: DialogVariant::Error,
                    });
                } else {
                    // Refresh profiles in screen
                    if let Err(e) = self
                        .manage_profiles_screen
                        .refresh_profiles(&self.config.repo_path)
                    {
                        error!("Failed to refresh profiles after rename: {}", e);
                    }
                }
            }
            ScreenAction::DeleteProfile { name } => {
                use crate::services::ProfileService;
                match ProfileService::delete_profile(
                    &self.config.repo_path,
                    &name,
                    &self.config.active_profile,
                ) {
                    Ok(_) => {
                        // Reload config
                        match crate::config::Config::load_or_create(&self.config_path) {
                            Ok(config) => self.config = config,
                            Err(e) => error!("Failed to reload config: {}", e),
                        }
                        // Refresh profiles in screen
                        if let Err(e) = self
                            .manage_profiles_screen
                            .refresh_profiles(&self.config.repo_path)
                        {
                            error!("Failed to refresh profiles after deletion: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to delete profile: {}", e);
                        self.dialog_state = Some(DialogState {
                            title: "Delete Failed".to_string(),
                            content: format!("Failed to delete profile '{}':\n\n{}", name, e),
                            variant: DialogVariant::Error,
                        });
                    }
                }
            }
            ScreenAction::MoveToCommon {
                file_index,
                is_common,
                profiles_to_cleanup,
            } => {
                let state = self.dotfile_selection_screen.get_state_mut();
                if file_index < state.dotfiles.len() {
                    let dotfile = &state.dotfiles[file_index];
                    let relative_path = dotfile.relative_path.to_string_lossy().to_string();

                    use crate::services::SyncService;
                    // Note: is_common parameter tells us the CURRENT state of the file
                    if is_common {
                        // File is currently common -> Move FROM common TO profile
                        match SyncService::move_from_common(&self.config, &relative_path) {
                            Ok(_) => {
                                info!("Moved {} from common to profile", relative_path);
                                // Refresh list to update UI
                                Self::scan_dotfiles_into(&self.config, state)?;

                                // Verify the file is correctly marked as profile (not common)
                                if let Some(idx) = state.dotfiles.iter().position(|d| {
                                    d.relative_path.to_string_lossy() == relative_path
                                }) {
                                    // Extract flags before potentially mutating
                                    let file_is_common = state.dotfiles[idx].is_common;
                                    let file_is_synced = state.dotfiles[idx].synced;

                                    if file_is_common {
                                        // File found but still marked as common - force fix
                                        warn!(
                                            "File {} was moved to profile but still detected as common after scan. Forcing refresh.",
                                            relative_path
                                        );
                                        state.dotfiles[idx].is_common = false;
                                    }
                                    // Ensure it's still marked as synced
                                    if !file_is_synced {
                                        state.dotfiles[idx].synced = true;
                                        state.selected_for_sync.insert(idx);
                                    }
                                    state.dotfile_list_state.select(Some(idx));
                                } else {
                                    warn!(
                                        "File {} not found in dotfiles list after move to profile",
                                        relative_path
                                    );
                                }

                                // Show success toast
                                self.toast_manager
                                    .success(format!("Moved to profile: {}", relative_path));
                            }
                            Err(e) => {
                                error!("Failed to move from common: {}", e);
                                self.dialog_state = Some(DialogState {
                                    title: "Move Failed".to_string(),
                                    content: format!("Failed to move file from common:\n\n{}", e),
                                    variant: DialogVariant::Error,
                                });
                            }
                        }
                    } else {
                        // File is currently profile -> Move FROM profile TO common
                        // Use the cleanup version if we have profiles to cleanup
                        let result = if !profiles_to_cleanup.is_empty() {
                            SyncService::move_to_common_with_cleanup(
                                &self.config,
                                &relative_path,
                                &profiles_to_cleanup,
                            )
                        } else {
                            SyncService::move_to_common(&self.config, &relative_path)
                        };

                        match result {
                            Ok(_) => {
                                info!("Moved {} to common", relative_path);
                                // Refresh list to update UI
                                Self::scan_dotfiles_into(&self.config, state)?;

                                // Verify the file is correctly marked as common
                                if let Some(idx) = state.dotfiles.iter().position(|d| {
                                    d.relative_path.to_string_lossy() == relative_path
                                }) {
                                    let dotfile = &state.dotfiles[idx];
                                    if !dotfile.is_common {
                                        // File found but not marked as common - force fix
                                        warn!(
                                            "File {} was moved to common but not detected as common after scan. Forcing refresh.",
                                            relative_path
                                        );
                                        // Force the flags and selected_for_sync
                                        state.dotfiles[idx].is_common = true;
                                        state.dotfiles[idx].synced = true;
                                        state.selected_for_sync.insert(idx);
                                    }
                                    state.dotfile_list_state.select(Some(idx));
                                } else {
                                    warn!(
                                        "File {} not found in dotfiles list after move to common",
                                        relative_path
                                    );
                                }

                                // Show success toast
                                self.toast_manager
                                    .success(format!("Moved to common: {}", relative_path));
                            }
                            Err(e) => {
                                error!("Failed to move to common: {}", e);
                                self.dialog_state = Some(DialogState {
                                    title: "Move Failed".to_string(),
                                    content: format!("Failed to move file to common:\n\n{}", e),
                                    variant: DialogVariant::Error,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Call on_enter for the target screen when navigating
    fn call_on_enter(&mut self, target: Screen) -> Result<()> {
        use crate::screens::{Screen as ScreenTrait, ScreenContext};
        let ctx = ScreenContext::new(&self.config, &self.config_path);

        match target {
            Screen::MainMenu => self.main_menu_screen.on_enter(&ctx)?,
            Screen::DotfileSelection => self.dotfile_selection_screen.on_enter(&ctx)?,
            Screen::GitHubAuth => self.github_auth_screen.on_enter(&ctx)?,
            Screen::StorageSetup => self.storage_setup_screen.on_enter(&ctx)?,
            Screen::SyncWithRemote => self.sync_with_remote_screen.on_enter(&ctx)?,
            Screen::ManageProfiles => self.manage_profiles_screen.on_enter(&ctx)?,
            Screen::ProfileSelection => self.profile_selection_screen.on_enter(&ctx)?,
            Screen::ManagePackages => self.manage_packages_screen.on_enter(&ctx)?,
            Screen::Settings => self.settings_screen.on_enter(&ctx)?,
        }
        Ok(())
    }

    /// Process one step of the GitHub setup state machine
    /// Called from the event loop to allow UI updates between steps
    fn process_github_setup_step(&mut self) -> Result<()> {
        // Clone the screen's state to work with (avoids borrow checker issues)
        let mut auth_state = self.github_auth_screen.get_auth_state().clone();

        // Get setup_data, cloning if needed to avoid borrow issues
        let setup_data_opt = auth_state.setup_data.clone();
        let mut setup_data = match setup_data_opt {
            Some(data) => data,
            None => {
                // No setup data, reset to input
                auth_state.step = GitHubAuthStep::Input;
                *self.github_auth_screen.get_auth_state_mut() = auth_state;
                return Ok(());
            }
        };

        // Check if we need to wait for a delay
        if let Some(delay_until) = setup_data.delay_until {
            if std::time::Instant::now() < delay_until {
                // Still waiting, don't process yet - save state and return
                auth_state.setup_data = Some(setup_data);
                *self.github_auth_screen.get_auth_state_mut() = auth_state;
                return Ok(());
            }
            // Delay complete, clear it
            setup_data.delay_until = None;
        }

        // Process current step - extract the step from the enum
        let current_step = if let GitHubAuthStep::SetupStep(step) = auth_state.step {
            step
        } else {
            // Not in setup, clear data
            auth_state.setup_data = Some(setup_data);
            *self.github_auth_screen.get_auth_state_mut() = auth_state;
            return Ok(());
        };

        match current_step {
            GitHubSetupStep::Connecting => {
                // Move to validating token
                auth_state.step = GitHubAuthStep::SetupStep(GitHubSetupStep::ValidatingToken);
                auth_state.status_message = Some("🔑 Validating your token...".to_string());
                setup_data.delay_until =
                    Some(std::time::Instant::now() + Duration::from_millis(800));
                auth_state.setup_data = Some(setup_data);
                *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
            }
            GitHubSetupStep::ValidatingToken => {
                // Perform async validation
                let token = setup_data.token.clone();
                let repo_name = setup_data.repo_name.clone();

                let result = self.runtime.block_on(async {
                    let client = GitHubClient::new(token.clone());
                    let user = client.get_user().await?;
                    let repo_exists = client.repo_exists(&user.login, &repo_name).await?;
                    Ok::<(String, bool), anyhow::Error>((user.login, repo_exists))
                });

                match result {
                    Ok((username, exists)) => {
                        setup_data.username = Some(username.clone());
                        setup_data.repo_exists = Some(exists);
                        setup_data.delay_until =
                            Some(std::time::Instant::now() + Duration::from_millis(600));

                        // Move to checking repo step
                        auth_state.step = GitHubAuthStep::SetupStep(GitHubSetupStep::CheckingRepo);
                        auth_state.status_message =
                            Some("🔍 Checking if repository exists...".to_string());
                        auth_state.setup_data = Some(setup_data); // Save setup_data with username and repo_exists
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                    }
                    Err(e) => {
                        auth_state.error_message = Some(format!("❌ Authentication failed: {}", e));
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                        return Ok(());
                    }
                }
            }
            GitHubSetupStep::CheckingRepo => {
                // Move to next step based on whether repo exists
                // Ensure we have username and repo_exists set
                if setup_data.username.is_none() || setup_data.repo_exists.is_none() {
                    error!("Invalid state: username or repo_exists not set in CheckingRepo step");
                    auth_state.error_message = Some(
                        "❌ Internal error: Setup state is invalid. Please try again.".to_string(),
                    );
                    auth_state.status_message = None;
                    auth_state.step = GitHubAuthStep::Input;
                    auth_state.setup_data = None;
                    *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                    return Ok(());
                }

                if setup_data.repo_exists == Some(true) {
                    auth_state.step = GitHubAuthStep::SetupStep(GitHubSetupStep::CloningRepo);
                    let username = setup_data.username.as_ref().unwrap(); // Safe now after check
                    auth_state.status_message = Some(format!(
                        "📥 Cloning repository {}/{}...",
                        username, setup_data.repo_name
                    ));
                    setup_data.delay_until =
                        Some(std::time::Instant::now() + Duration::from_millis(500));
                    auth_state.setup_data = Some(setup_data);
                    *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                } else {
                    auth_state.step = GitHubAuthStep::SetupStep(GitHubSetupStep::CreatingRepo);
                    let username = setup_data.username.as_ref().unwrap(); // Safe now after check
                    auth_state.status_message = Some(format!(
                        "📦 Creating repository {}/{}...",
                        username, setup_data.repo_name
                    ));
                    setup_data.delay_until =
                        Some(std::time::Instant::now() + Duration::from_millis(600));
                    auth_state.setup_data = Some(setup_data);
                    *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                }
            }
            GitHubSetupStep::CloningRepo => {
                // Clone the repository (or reuse existing if valid)
                let username = setup_data.username.as_ref().unwrap();
                let repo_path = self.config.repo_path.clone();
                let token = setup_data.token.clone();
                let remote_url = format!(
                    "https://github.com/{}/{}.git",
                    username, setup_data.repo_name
                );

                match GitManager::clone_or_open(&remote_url, &repo_path, Some(&token)) {
                    Ok((_, was_existing)) => {
                        auth_state.status_message = Some(if was_existing {
                            "✅ Using existing repository!".to_string()
                        } else {
                            "✅ Repository cloned successfully!".to_string()
                        });
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();

                        // Update config
                        self.config.github = Some(GitHubConfig {
                            owner: username.clone(),
                            repo: setup_data.repo_name.clone(),
                            token: Some(token.clone()),
                        });
                        self.config.repo_name = setup_data.repo_name.clone();
                        self.config
                            .save(&self.config_path)
                            .context("Failed to save configuration")?;

                        // Move to discovering profiles
                        auth_state.step =
                            GitHubAuthStep::SetupStep(GitHubSetupStep::DiscoveringProfiles);
                        auth_state.status_message = Some("🔎 Discovering profiles...".to_string());
                        setup_data.delay_until =
                            Some(std::time::Instant::now() + Duration::from_millis(600));
                        auth_state.setup_data = Some(setup_data);
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                    }
                    Err(e) => {
                        auth_state.error_message =
                            Some(format!("❌ Failed to clone repository: {}", e));
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                        return Ok(());
                    }
                }
            }
            GitHubSetupStep::CreatingRepo => {
                // Create the repository
                // Validate username is set (needed for next step)
                if setup_data.username.is_none() {
                    error!("Invalid state: username not set in CreatingRepo step");
                    auth_state.error_message = Some(
                        "❌ Internal error: Username not available. Please try again.".to_string(),
                    );
                    auth_state.status_message = None;
                    auth_state.step = GitHubAuthStep::Input;
                    auth_state.setup_data = None;
                    *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                    return Ok(());
                }
                let token = setup_data.token.clone();
                let repo_name = setup_data.repo_name.clone();

                let is_private = setup_data.is_private;
                let create_result = self.runtime.block_on(async {
                    let client = GitHubClient::new(token.clone());
                    client
                        .create_repo(&repo_name, "My dotfiles managed by dotstate", is_private)
                        .await
                });

                match create_result {
                    Ok(_) => {
                        setup_data.delay_until =
                            Some(std::time::Instant::now() + Duration::from_millis(500));
                        setup_data.is_new_repo = true; // Mark as new repo creation
                        auth_state.step =
                            GitHubAuthStep::SetupStep(GitHubSetupStep::InitializingRepo);
                        auth_state.status_message =
                            Some("⚙️  Initializing local repository...".to_string());
                        auth_state.setup_data = Some(setup_data); // Save setup_data
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                    }
                    Err(e) => {
                        auth_state.error_message =
                            Some(format!("❌ Failed to create repository: {}", e));
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                        return Ok(());
                    }
                }
            }
            GitHubSetupStep::InitializingRepo => {
                // Initialize local repository
                let username = match setup_data.username.as_ref() {
                    Some(u) => u,
                    None => {
                        error!("Invalid state: username not set in InitializingRepo step");
                        auth_state.error_message = Some(
                            "❌ Internal error: Username not available. Please try again."
                                .to_string(),
                        );
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
                        return Ok(());
                    }
                };
                let token = setup_data.token.clone();
                let repo_name = setup_data.repo_name.clone();
                let repo_path = self.config.repo_path.clone();

                std::fs::create_dir_all(&repo_path)
                    .context("Failed to create repository directory")?;

                let mut git_mgr = GitManager::open_or_init(&repo_path)?;

                // Add remote
                let remote_url = format!(
                    "https://{}@github.com/{}/{}.git",
                    token, username, repo_name
                );
                git_mgr.add_remote("origin", &remote_url)?;

                // Create initial commit
                std::fs::write(
                    repo_path.join("README.md"),
                    format!("# {}\n\nDotfiles managed by dotstate", repo_name),
                )?;

                // Create profile manifest with default profile
                // Use "Personal" as default profile name if active_profile is empty
                let default_profile_name = if self.config.active_profile.is_empty() {
                    "Personal".to_string()
                } else {
                    self.config.active_profile.clone()
                };

                let manifest = crate::utils::ProfileManifest {
                    common: crate::utils::profile_manifest::CommonSection::default(),
                    profiles: vec![crate::utils::profile_manifest::ProfileInfo {
                        name: default_profile_name.clone(),
                        description: None, // Default profile, no description yet
                        synced_files: Vec::new(),
                        packages: Vec::new(),
                    }],
                };
                manifest.save(&repo_path)?;

                git_mgr.commit_all("Initial commit")?;

                let current_branch = git_mgr
                    .get_current_branch()
                    .unwrap_or_else(|| self.config.default_branch.clone());

                // Before pushing, fetch and merge any remote commits (GitHub might have created an initial commit)
                // This prevents "NotFastForward" errors
                if let Err(e) = git_mgr.pull("origin", &current_branch, Some(&token)) {
                    // If pull fails (e.g., remote branch doesn't exist yet), that's fine - we'll push
                    info!(
                        "Could not pull from remote (this is normal for new repos): {}",
                        e
                    );
                } else {
                    info!("Successfully pulled from remote before pushing");
                }

                git_mgr.push("origin", &current_branch, Some(&token))?;
                git_mgr.set_upstream_tracking("origin", &current_branch)?;

                // Update config
                self.config.github = Some(GitHubConfig {
                    owner: username.clone(),
                    repo: repo_name.clone(),
                    token: Some(token.clone()),
                });
                self.config.repo_name = repo_name.clone();
                self.config.active_profile = default_profile_name.clone();
                self.config
                    .save(&self.config_path)
                    .context("Failed to save configuration")?;

                // Load manifest and populate profile selection state
                let manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
                self.ui_state.profile_selection.profiles =
                    manifest.profiles.iter().map(|p| p.name.clone()).collect();
                if !self.ui_state.profile_selection.profiles.is_empty() {
                    self.ui_state.profile_selection.list_state.select(Some(0));
                }

                auth_state.status_message =
                    Some("✅ Repository created and initialized successfully".to_string());
                *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();

                // Move to complete step with delay to show success message
                auth_state.step = GitHubAuthStep::SetupStep(GitHubSetupStep::Complete);
                self.config = Config::load_or_create(&self.config_path)?;
                auth_state.status_message = Some(format!(
                    "✅ Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nPreparing profile selection...",
                    username, repo_name, repo_path
                ));
                // Add delay to show success message before transitioning
                setup_data.delay_until =
                    Some(std::time::Instant::now() + Duration::from_millis(2000));
                setup_data.is_new_repo = true; // Mark as new repo creation
                auth_state.setup_data = Some(setup_data);
                *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
            }
            GitHubSetupStep::DiscoveringProfiles => {
                // Discover profiles from the cloned repo
                let repo_path = self.config.repo_path.clone();

                // Load manifest - synced_files should already be in manifest
                let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;

                // If manifest has profiles but synced_files are empty, backfill from directory
                for profile_info in &mut manifest.profiles {
                    if profile_info.synced_files.is_empty() {
                        let profile_dir = repo_path.join(&profile_info.name);
                        if profile_dir.exists() && profile_dir.is_dir() {
                            profile_info.synced_files =
                                list_files_in_profile_dir(&profile_dir, &repo_path)
                                    .unwrap_or_default();
                        }
                    }
                }
                manifest.save(&repo_path)?;

                if !manifest.profiles.is_empty() && self.config.active_profile.is_empty() {
                    self.config.active_profile = manifest.profiles[0].name.clone();
                    self.config.save(&self.config_path)?;
                }

                // Set up profile selection state
                let profiles: Vec<String> =
                    manifest.profiles.iter().map(|p| p.name.clone()).collect();

                // Update legacy ui_state
                self.ui_state.profile_selection.profiles = profiles.clone();
                if !self.ui_state.profile_selection.profiles.is_empty() {
                    self.ui_state.profile_selection.list_state.select(Some(0));
                }

                // Update screen controller state
                self.profile_selection_screen.set_profiles(profiles);

                // Move to complete step - show success message in progress screen
                if !self.ui_state.profile_selection.profiles.is_empty() {
                    auth_state.status_message = Some(format!(
                        "✅ Setup complete!\n\nFound {} profile(s) in the repository.\n\nPreparing profile selection...",
                        self.ui_state.profile_selection.profiles.len()
                    ));
                } else {
                    // For new repos, we might not have username in setup_data
                    // Use config if available, otherwise use repo_name
                    let username = setup_data
                        .username
                        .as_ref()
                        .or_else(|| self.config.github.as_ref().map(|g| &g.owner))
                        .unwrap_or(&setup_data.repo_name);
                    let repo_name = setup_data.repo_name.clone();
                    auth_state.status_message = Some(format!(
                        "✅ Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nNo profiles found. You can create one from the main menu.\n\nPreparing main menu...",
                        username, repo_name, repo_path
                    ));
                }
                // Add a delay to show the success message before transitioning
                setup_data.delay_until =
                    Some(std::time::Instant::now() + Duration::from_millis(2000));
                auth_state.step = GitHubAuthStep::SetupStep(GitHubSetupStep::Complete);
                auth_state.setup_data = Some(setup_data);
                *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();
            }
            GitHubSetupStep::Complete => {
                // Delay complete, transition to next screen
                let profile_count = self.ui_state.profile_selection.profiles.len();
                let is_new_repo = setup_data.is_new_repo;

                // Determine target screen and whether to scan dotfiles
                let should_scan_dotfiles = is_new_repo && profile_count == 1;
                let target_screen = if profile_count > 0 {
                    if should_scan_dotfiles {
                        Screen::DotfileSelection
                    } else {
                        Screen::ProfileSelection
                    }
                } else {
                    Screen::MainMenu
                };

                // Update auth_state before dropping borrow
                auth_state.step = GitHubAuthStep::Input; // Reset to input state
                auth_state.status_message = None;
                auth_state.setup_data = None;
                *self.github_auth_screen.get_auth_state_mut() = auth_state.clone();

                // Drop mutable borrow of auth_state before calling scan_dotfiles
                // Note: drop() on a reference doesn't do anything, but we're explicitly ending the borrow scope
                let _ = auth_state;

                if should_scan_dotfiles {
                    // New install with default profile - go directly to dotfile selection
                    let state = self.dotfile_selection_screen.get_state_mut();
                    state.backup_enabled = self.config.backup_enabled;
                    state.status_message = None;
                    Self::scan_dotfiles_into(&self.config, state)?;
                }

                self.ui_state.current_screen = target_screen;
                return Ok(()); // Early return to avoid using auth_state after match
            }
        }

        // Save updated setup_data back (only if it wasn't already consumed/saved in the step)
        // Steps that complete set setup_data to None, so we only save if it's still needed
        if auth_state.setup_data.is_none()
            && matches!(auth_state.step, GitHubAuthStep::SetupStep(_))
        {
            // Only save if we're still in setup and data wasn't consumed
            // But actually, each step that needs to continue already saves it
            // So we only need to save if the step didn't save it yet
            // For now, let's not save here - each step handles its own saving
        }

        Ok(())
    }

    /// Process the storage setup state machine (for new StorageSetupScreen)
    fn process_storage_setup_step(&mut self) -> Result<()> {
        use crate::screens::storage_setup::StorageSetupStep;

        // Clone the screen's state to work with (avoids borrow checker issues)
        let state = self.storage_setup_screen.get_state();
        let step = state.step;
        let setup_data_opt = state.setup_data.clone();

        let mut setup_data = match setup_data_opt {
            Some(data) => data,
            None => {
                // No setup data, reset to input
                self.storage_setup_screen.get_state_mut().step = StorageSetupStep::Input;
                return Ok(());
            }
        };

        // Check if we need to wait for a delay
        if let Some(delay_until) = setup_data.delay_until {
            if std::time::Instant::now() < delay_until {
                return Ok(());
            }
            setup_data.delay_until = None;
        }

        // Extract the current step
        let current_step = match step {
            StorageSetupStep::Processing(s) => s,
            StorageSetupStep::Input => return Ok(()),
        };

        match current_step {
            GitHubSetupStep::Connecting => {
                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Processing(GitHubSetupStep::ValidatingToken);
                state.status_message = Some("🔑 Validating your token...".to_string());
                setup_data.delay_until =
                    Some(std::time::Instant::now() + Duration::from_millis(800));
                state.setup_data = Some(setup_data);
            }
            GitHubSetupStep::ValidatingToken => {
                let token = setup_data.token.clone();
                let repo_name = setup_data.repo_name.clone();

                let result = self.runtime.block_on(async {
                    let client = GitHubClient::new(token.clone());
                    let user = client.get_user().await?;
                    let repo_exists = client.repo_exists(&user.login, &repo_name).await?;
                    Ok::<(String, bool), anyhow::Error>((user.login, repo_exists))
                });

                match result {
                    Ok((username, exists)) => {
                        setup_data.username = Some(username);
                        setup_data.repo_exists = Some(exists);
                        setup_data.delay_until =
                            Some(std::time::Instant::now() + Duration::from_millis(600));

                        let state = self.storage_setup_screen.get_state_mut();
                        state.step = StorageSetupStep::Processing(GitHubSetupStep::CheckingRepo);
                        state.status_message =
                            Some("🔍 Checking if repository exists...".to_string());
                        state.setup_data = Some(setup_data);
                    }
                    Err(e) => {
                        let state = self.storage_setup_screen.get_state_mut();
                        state.error_message = Some(format!("❌ Authentication failed: {}", e));
                        state.status_message = None;
                        state.step = StorageSetupStep::Input;
                        state.setup_data = None;
                        return Ok(());
                    }
                }
            }
            GitHubSetupStep::CheckingRepo => {
                if setup_data.username.is_none() || setup_data.repo_exists.is_none() {
                    error!("Invalid state: username or repo_exists not set in CheckingRepo step");
                    let state = self.storage_setup_screen.get_state_mut();
                    state.error_message = Some(
                        "❌ Internal error: Setup state is invalid. Please try again.".to_string(),
                    );
                    state.status_message = None;
                    state.step = StorageSetupStep::Input;
                    state.setup_data = None;
                    return Ok(());
                }

                if setup_data.repo_exists == Some(true) {
                    let username = setup_data.username.as_ref().unwrap();
                    let state = self.storage_setup_screen.get_state_mut();
                    state.step = StorageSetupStep::Processing(GitHubSetupStep::CloningRepo);
                    state.status_message = Some(format!(
                        "📥 Cloning repository {}/{}...",
                        username, setup_data.repo_name
                    ));
                    setup_data.delay_until =
                        Some(std::time::Instant::now() + Duration::from_millis(500));
                    state.setup_data = Some(setup_data);
                } else {
                    let username = setup_data.username.as_ref().unwrap();
                    let state = self.storage_setup_screen.get_state_mut();
                    state.step = StorageSetupStep::Processing(GitHubSetupStep::CreatingRepo);
                    state.status_message = Some(format!(
                        "📦 Creating repository {}/{}...",
                        username, setup_data.repo_name
                    ));
                    setup_data.delay_until =
                        Some(std::time::Instant::now() + Duration::from_millis(600));
                    state.setup_data = Some(setup_data);
                }
            }
            GitHubSetupStep::CloningRepo => {
                let username = setup_data.username.as_ref().unwrap();
                let repo_path = self.config.repo_path.clone();
                let token = setup_data.token.clone();
                let remote_url = format!(
                    "https://github.com/{}/{}.git",
                    username, setup_data.repo_name
                );

                match GitManager::clone_or_open(&remote_url, &repo_path, Some(&token)) {
                    Ok((_, was_existing)) => {
                        let state = self.storage_setup_screen.get_state_mut();
                        state.status_message = Some(if was_existing {
                            "✅ Using existing repository!".to_string()
                        } else {
                            "✅ Repository cloned successfully!".to_string()
                        });

                        // Update config
                        self.config.github = Some(GitHubConfig {
                            owner: username.clone(),
                            repo: setup_data.repo_name.clone(),
                            token: Some(token.clone()),
                        });
                        self.config.repo_name = setup_data.repo_name.clone();
                        self.config
                            .save(&self.config_path)
                            .context("Failed to save configuration")?;

                        // Move to discovering profiles
                        state.step =
                            StorageSetupStep::Processing(GitHubSetupStep::DiscoveringProfiles);
                        state.status_message = Some("🔎 Discovering profiles...".to_string());
                        setup_data.delay_until =
                            Some(std::time::Instant::now() + Duration::from_millis(600));
                        state.setup_data = Some(setup_data);
                    }
                    Err(e) => {
                        let state = self.storage_setup_screen.get_state_mut();
                        state.error_message = Some(format!("❌ Failed to clone repository: {}", e));
                        state.status_message = None;
                        state.step = StorageSetupStep::Input;
                        state.setup_data = None;
                        return Ok(());
                    }
                }
            }
            GitHubSetupStep::CreatingRepo => {
                if setup_data.username.is_none() {
                    error!("Invalid state: username not set in CreatingRepo step");
                    let state = self.storage_setup_screen.get_state_mut();
                    state.error_message = Some(
                        "❌ Internal error: Username not available. Please try again.".to_string(),
                    );
                    state.status_message = None;
                    state.step = StorageSetupStep::Input;
                    state.setup_data = None;
                    return Ok(());
                }

                let token = setup_data.token.clone();
                let repo_name = setup_data.repo_name.clone();
                let is_private = setup_data.is_private;

                let create_result = self.runtime.block_on(async {
                    let client = GitHubClient::new(token.clone());
                    client
                        .create_repo(&repo_name, "My dotfiles managed by dotstate", is_private)
                        .await
                });

                match create_result {
                    Ok(_) => {
                        setup_data.delay_until =
                            Some(std::time::Instant::now() + Duration::from_millis(500));
                        setup_data.is_new_repo = true;

                        let state = self.storage_setup_screen.get_state_mut();
                        state.step =
                            StorageSetupStep::Processing(GitHubSetupStep::InitializingRepo);
                        state.status_message =
                            Some("⚙️  Initializing local repository...".to_string());
                        state.setup_data = Some(setup_data);
                    }
                    Err(e) => {
                        let state = self.storage_setup_screen.get_state_mut();
                        state.error_message =
                            Some(format!("❌ Failed to create repository: {}", e));
                        state.status_message = None;
                        state.step = StorageSetupStep::Input;
                        state.setup_data = None;
                        return Ok(());
                    }
                }
            }
            GitHubSetupStep::InitializingRepo => {
                let username = match setup_data.username.as_ref() {
                    Some(u) => u.clone(),
                    None => {
                        error!("Invalid state: username not set in InitializingRepo step");
                        let state = self.storage_setup_screen.get_state_mut();
                        state.error_message = Some(
                            "❌ Internal error: Username not available. Please try again."
                                .to_string(),
                        );
                        state.status_message = None;
                        state.step = StorageSetupStep::Input;
                        state.setup_data = None;
                        return Ok(());
                    }
                };

                let token = setup_data.token.clone();
                let repo_name = setup_data.repo_name.clone();
                let repo_path = self.config.repo_path.clone();

                std::fs::create_dir_all(&repo_path)
                    .context("Failed to create repository directory")?;

                let mut git_mgr = GitManager::open_or_init(&repo_path)?;

                // Add remote
                let remote_url = format!(
                    "https://{}@github.com/{}/{}.git",
                    token, username, repo_name
                );
                git_mgr.add_remote("origin", &remote_url)?;

                // Create initial commit
                std::fs::write(
                    repo_path.join("README.md"),
                    format!("# {}\n\nDotfiles managed by dotstate", repo_name),
                )?;

                // Create profile manifest with default profile
                let default_profile_name = if self.config.active_profile.is_empty() {
                    "Personal".to_string()
                } else {
                    self.config.active_profile.clone()
                };

                let manifest = crate::utils::ProfileManifest {
                    common: crate::utils::profile_manifest::CommonSection::default(),
                    profiles: vec![crate::utils::profile_manifest::ProfileInfo {
                        name: default_profile_name.clone(),
                        description: None,
                        synced_files: Vec::new(),
                        packages: Vec::new(),
                    }],
                };
                manifest.save(&repo_path)?;

                git_mgr.commit_all("Initial commit")?;

                let current_branch = git_mgr
                    .get_current_branch()
                    .unwrap_or_else(|| self.config.default_branch.clone());

                // Before pushing, fetch and merge any remote commits
                if let Err(e) = git_mgr.pull("origin", &current_branch, Some(&token)) {
                    info!(
                        "Could not pull from remote (this is normal for new repos): {}",
                        e
                    );
                } else {
                    info!("Successfully pulled from remote before pushing");
                }

                git_mgr.push("origin", &current_branch, Some(&token))?;
                git_mgr.set_upstream_tracking("origin", &current_branch)?;

                // Update config
                self.config.github = Some(GitHubConfig {
                    owner: username.clone(),
                    repo: repo_name.clone(),
                    token: Some(token.clone()),
                });
                self.config.repo_name = repo_name.clone();
                self.config.active_profile = default_profile_name.clone();
                self.config
                    .save(&self.config_path)
                    .context("Failed to save configuration")?;

                // Load manifest and populate profile selection state
                let manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
                self.ui_state.profile_selection.profiles =
                    manifest.profiles.iter().map(|p| p.name.clone()).collect();
                if !self.ui_state.profile_selection.profiles.is_empty() {
                    self.ui_state.profile_selection.list_state.select(Some(0));
                }

                // Move to complete step with delay
                setup_data.delay_until =
                    Some(std::time::Instant::now() + Duration::from_millis(2000));
                setup_data.is_new_repo = true;

                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Processing(GitHubSetupStep::Complete);
                self.config = Config::load_or_create(&self.config_path)?;
                state.status_message = Some(format!(
                    "✅ Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nPreparing profile selection...",
                    username, repo_name, repo_path
                ));
                state.setup_data = Some(setup_data);
            }
            GitHubSetupStep::DiscoveringProfiles => {
                let repo_path = self.config.repo_path.clone();

                // Load manifest
                let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;

                // Backfill synced_files if empty
                for profile_info in &mut manifest.profiles {
                    if profile_info.synced_files.is_empty() {
                        let profile_dir = repo_path.join(&profile_info.name);
                        if profile_dir.exists() && profile_dir.is_dir() {
                            profile_info.synced_files =
                                list_files_in_profile_dir(&profile_dir, &repo_path)
                                    .unwrap_or_default();
                        }
                    }
                }
                manifest.save(&repo_path)?;

                if !manifest.profiles.is_empty() && self.config.active_profile.is_empty() {
                    self.config.active_profile = manifest.profiles[0].name.clone();
                    self.config.save(&self.config_path)?;
                }

                // Set up profile selection state
                let profiles: Vec<String> =
                    manifest.profiles.iter().map(|p| p.name.clone()).collect();

                self.ui_state.profile_selection.profiles = profiles.clone();
                if !self.ui_state.profile_selection.profiles.is_empty() {
                    self.ui_state.profile_selection.list_state.select(Some(0));
                }
                self.profile_selection_screen.set_profiles(profiles);

                // Move to complete step
                let profile_count = self.ui_state.profile_selection.profiles.len();
                setup_data.delay_until =
                    Some(std::time::Instant::now() + Duration::from_millis(2000));

                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Processing(GitHubSetupStep::Complete);
                if profile_count > 0 {
                    state.status_message = Some(format!(
                        "✅ Setup complete!\n\nFound {} profile(s) in the repository.\n\nPreparing profile selection...",
                        profile_count
                    ));
                } else {
                    let username = setup_data
                        .username
                        .as_ref()
                        .or_else(|| self.config.github.as_ref().map(|g| &g.owner))
                        .unwrap_or(&setup_data.repo_name);
                    state.status_message = Some(format!(
                        "✅ Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nNo profiles found. You can create one from the main menu.\n\nPreparing main menu...",
                        username, setup_data.repo_name, repo_path
                    ));
                }
                state.setup_data = Some(setup_data);
            }
            GitHubSetupStep::Complete => {
                // Delay complete, transition to next screen
                let profile_count = self.ui_state.profile_selection.profiles.len();
                let is_new_repo = setup_data.is_new_repo;

                // Determine target screen
                let should_scan_dotfiles = is_new_repo && profile_count == 1;
                let target_screen = if profile_count > 0 {
                    if should_scan_dotfiles {
                        Screen::DotfileSelection
                    } else {
                        Screen::ProfileSelection
                    }
                } else {
                    Screen::MainMenu
                };

                // Reset screen state
                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Input;
                state.status_message = None;
                state.setup_data = None;

                if should_scan_dotfiles {
                    let dotfile_state = self.dotfile_selection_screen.get_state_mut();
                    dotfile_state.backup_enabled = self.config.backup_enabled;
                    dotfile_state.status_message = None;
                    Self::scan_dotfiles_into(&self.config, dotfile_state)?;
                }

                self.ui_state.current_screen = target_screen;
                return Ok(());
            }
        }

        Ok(())
    }

    /// Add a single file to sync (copy to repo, create symlink, update manifest)
    fn add_file_to_sync_with_state(
        config: &crate::config::Config,
        state: &mut DotfileSelectionState,
        file_index: usize,
    ) -> Result<()> {
        use crate::services::SyncService;

        if file_index >= state.dotfiles.len() {
            warn!(
                "File index {} out of bounds ({} files)",
                file_index,
                state.dotfiles.len()
            );
            return Ok(());
        }

        let dotfile = &state.dotfiles[file_index];
        let relative_str = dotfile.relative_path.to_string_lossy().to_string();
        let full_path = dotfile.original_path.clone();
        let backup_enabled = state.backup_enabled;

        // Use service to add file to sync
        match SyncService::add_file_to_sync(config, &full_path, &relative_str, backup_enabled)? {
            crate::services::sync_service::AddFileResult::Success => {
                state.selected_for_sync.insert(file_index);
                state.dotfiles[file_index].synced = true;
                info!("Successfully added file to sync: {}", relative_str);
            }
            crate::services::sync_service::AddFileResult::AlreadySynced => {
                state.selected_for_sync.insert(file_index);
                debug!("File already synced: {}", relative_str);
            }
            crate::services::sync_service::AddFileResult::ValidationFailed(error_msg) => {
                state.status_message = Some(format!("Error: {}", error_msg));
                warn!("Validation failed for {}: {}", relative_str, error_msg);
            }
        }

        Ok(())
    }

    /// Handle adding a custom file to sync, including state and config updates
    fn handle_add_custom_file_to_sync(
        &mut self,
        full_path: PathBuf,
        relative_path: String,
    ) -> Result<()> {
        use crate::services::SyncService;

        let backup_enabled = self.dotfile_selection_screen.get_state().backup_enabled;

        // Use service to add file to sync
        match SyncService::add_file_to_sync(
            &self.config,
            &full_path,
            &relative_path,
            backup_enabled,
        )? {
            crate::services::sync_service::AddFileResult::Success => {
                // Check if this is a custom file (not in default dotfile candidates)
                if SyncService::is_custom_file(&relative_path) {
                    // Add to config.custom_files if not already there
                    if !self.config.custom_files.contains(&relative_path) {
                        self.config.custom_files.push(relative_path.clone());
                        self.config.save(&self.config_path)?;
                    }
                }
                info!("Successfully added custom file to sync: {}", relative_path);

                // Re-scan to refresh the list
                let state = self.dotfile_selection_screen.get_state_mut();
                Self::scan_dotfiles_into(&self.config, state)?;

                // Find and select the file in the list
                let state = self.dotfile_selection_screen.get_state_mut();
                if let Some(index) = state
                    .dotfiles
                    .iter()
                    .position(|d| d.relative_path.to_string_lossy() == relative_path)
                {
                    state.dotfile_list_state.select(Some(index));
                    state.selected_for_sync.insert(index);
                }
                // Show success toast
                self.toast_manager
                    .success(format!("Added: {}", relative_path));
            }
            crate::services::sync_service::AddFileResult::AlreadySynced => {
                debug!("Custom file already synced: {}", relative_path);
            }
            crate::services::sync_service::AddFileResult::ValidationFailed(error_msg) => {
                warn!(
                    "Validation failed for custom file {}: {}",
                    relative_path, error_msg
                );
                self.dialog_state = Some(DialogState {
                    title: "Sync Failed".to_string(),
                    content: error_msg,
                    variant: DialogVariant::Error,
                });
            }
        }

        Ok(())
    }

    /// Remove a single file from sync (restore from repo, remove symlink, update manifest)
    fn remove_file_from_sync_with_state(
        config: &crate::config::Config,
        state: &mut DotfileSelectionState,
        file_index: usize,
    ) -> Result<()> {
        use crate::services::SyncService;

        if file_index >= state.dotfiles.len() {
            warn!(
                "File index {} out of bounds ({} files)",
                file_index,
                state.dotfiles.len()
            );
            return Ok(());
        }

        let relative_str = state.dotfiles[file_index]
            .relative_path
            .to_string_lossy()
            .to_string();

        // Use service to remove file from sync
        match SyncService::remove_file_from_sync(config, &relative_str)? {
            crate::services::sync_service::RemoveFileResult::Success => {
                // Unmark as selected and synced
                state.selected_for_sync.remove(&file_index);
                state.dotfiles[file_index].synced = false;
                info!("Successfully removed file from sync: {}", relative_str);
            }
            crate::services::sync_service::RemoveFileResult::NotSynced => {
                debug!("File not synced, skipping removal: {}", relative_str);
                state.selected_for_sync.remove(&file_index);
            }
        }

        Ok(())
    }

    /// Scan for dotfiles and populate the selection state
    fn scan_dotfiles_into(
        config: &crate::config::Config,
        state: &mut DotfileSelectionState,
    ) -> Result<()> {
        use crate::services::SyncService;

        // Use service to scan dotfiles
        let found = SyncService::scan_dotfiles(config)?;

        // Build selected indices for synced files
        let selected_indices: std::collections::HashSet<usize> = found
            .iter()
            .enumerate()
            .filter(|(_, d)| d.synced)
            .map(|(i, _)| i)
            .collect();

        // Update state
        state.dotfiles = found;
        state.preview_index = None;
        state.preview_scroll = 0;
        state.selected_for_sync = selected_indices;

        // Initialize ListState with first item selected if available
        if !state.dotfiles.is_empty() {
            state.dotfile_list_state.select(Some(0));
        } else {
            state.dotfile_list_state.select(None);
        }

        Ok(())
    }

    /// Refresh file browser entries for current directory
    fn refresh_file_browser_into(
        config: &crate::config::Config,
        state: &mut DotfileSelectionState,
    ) -> Result<()> {
        let path = &state.file_browser_path;

        let mut entries = Vec::new();

        // Add parent directory if not at root
        if path != Path::new("/") && path.parent().is_some() {
            entries.push(PathBuf::from(".."));
        }

        // Add special marker for "add this folder" (only if it's a directory and safe to add)
        let repo_path = &config.repo_path;
        let (is_safe, _) = crate::utils::is_safe_to_add(path, repo_path);
        if is_safe && path.is_dir() {
            entries.push(PathBuf::from(".")); // Special marker for "add this folder"
        }

        // Read directory entries
        if let Ok(entries_iter) = std::fs::read_dir(path) {
            for entry in entries_iter.flatten() {
                let entry_path = entry.path();
                // Show all files for now (user can navigate)
                entries.push(entry_path);
            }
        }

        // Sort: special entries first (.. and .), then directories, then files, both alphabetically
        entries.sort_by(|a, b| {
            let a_is_special = a == Path::new("..") || a == Path::new(".");
            let b_is_special = b == Path::new("..") || b == Path::new(".");

            // Special entries come first, with .. before .
            if a_is_special && b_is_special {
                if a == Path::new("..") {
                    return std::cmp::Ordering::Less;
                } else {
                    return std::cmp::Ordering::Greater;
                }
            }
            if a_is_special {
                return std::cmp::Ordering::Less;
            }
            if b_is_special {
                return std::cmp::Ordering::Greater;
            }

            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();

            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => {
                    let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    a_name.cmp(b_name)
                }
            }
        });

        state.file_browser_entries = entries;

        // Update ListState selection to be within bounds
        if let Some(current_selection) = state.file_browser_list_state.selected() {
            if current_selection >= state.file_browser_entries.len() {
                if state.file_browser_entries.is_empty() {
                    state.file_browser_list_state.select(None);
                } else {
                    state
                        .file_browser_list_state
                        .select(Some(state.file_browser_entries.len() - 1));
                }
            }
        } else if !state.file_browser_entries.is_empty() {
            // If nothing selected, select first item
            state.file_browser_list_state.select(Some(0));
        }

        Ok(())
    }

    /// Helper: Load manifest from repo
    #[allow(dead_code)]
    fn load_manifest(&self) -> Result<crate::utils::ProfileManifest> {
        crate::services::ProfileService::load_manifest(&self.config.repo_path)
    }

    /// Helper: Get active profile info from manifest
    fn get_active_profile_info(&self) -> Result<Option<crate::utils::ProfileInfo>> {
        crate::services::ProfileService::get_profile_info(
            &self.config.repo_path,
            &self.config.active_profile,
        )
    }

    /// Switch to a different profile
    fn switch_profile(&mut self, target_profile_name: &str) -> Result<()> {
        use crate::services::ProfileService;

        // Don't switch if already active
        if self.config.active_profile == target_profile_name {
            return Ok(());
        }

        let old_profile_name = self.config.active_profile.clone();

        // Use service to switch profiles
        let switch_result = ProfileService::switch_profile(
            &self.config.repo_path,
            &old_profile_name,
            target_profile_name,
            self.config.backup_enabled,
        )?;

        // Update active profile in config
        self.config.active_profile = target_profile_name.to_string();
        self.config.save(&self.config_path)?;

        // Check packages after profile switch if the new profile has packages
        if !switch_result.packages.is_empty() {
            info!(
                "Profile '{}' has {} packages, checking installation status",
                target_profile_name,
                switch_result.packages.len()
            );
            // Initialize package checking state
            self.manage_packages_screen
                .update_packages(switch_result.packages, target_profile_name);
            self.manage_packages_screen.start_checking();
        }

        Ok(())
    }

    /// Rename a profile
    fn rename_profile(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        use crate::services::ProfileService;

        let was_active = self.config.active_profile == old_name;
        let is_activated = self.config.profile_activated && was_active;

        // Use service to rename profile
        let sanitized_name = ProfileService::rename_profile(
            &self.config.repo_path,
            old_name,
            new_name,
            is_activated,
            self.config.backup_enabled,
        )?;

        // Update active profile name if this was the active profile
        if was_active {
            self.config.active_profile = sanitized_name;
            self.config.save(&self.config_path)?;
        }

        Ok(())
    }

    /// Activate a profile after GitHub setup (includes syncing files from repo)
    fn activate_profile_after_setup(&mut self, profile_name: &str) -> Result<()> {
        use crate::services::ProfileService;

        info!("Activating profile '{}' after setup", profile_name);

        // Set as active profile
        self.config.active_profile = profile_name.to_string();
        self.config.save(&self.config_path)?;

        // Use service to activate profile
        let activation_result = ProfileService::activate_profile(
            &self.config.repo_path,
            profile_name,
            self.config.backup_enabled,
        )?;

        // Mark as activated
        self.config.profile_activated = true;
        self.config.save(&self.config_path)?;

        // Check packages after activation if the profile has packages
        if !activation_result.packages.is_empty() {
            info!(
                "Profile '{}' has {} packages, checking installation status",
                profile_name,
                activation_result.packages.len()
            );
            // Initialize package checking state
            self.manage_packages_screen
                .update_packages(activation_result.packages, profile_name);
            self.manage_packages_screen.start_checking();
        }

        Ok(())
    }
}
