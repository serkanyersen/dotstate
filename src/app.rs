use crate::config::Config;
use crate::screens::{
    ActionResult, MainMenuScreen, ManagePackagesScreen, ManageProfilesScreen,
    Screen as ScreenTrait, StorageSetupScreen, SyncWithRemoteScreen,
};
use crate::tui::Tui;
use crate::ui::{GitHubSetupStep, Screen, UiState};
use crate::widgets::{Dialog, DialogVariant, Toast, ToastManager};

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use syntect::parsing::SyntaxSet;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use tracing::{debug, error, info, trace, warn};
// Frame and Rect are used in function signatures but imported where needed

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
    /// Receiver for async storage setup step
    setup_step_handle: Option<crate::services::StepHandle>,
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
            setup_step_handle: None,
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
                        .map_err(|e| e.clone());
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

            // Check for storage setup step completion
            if let Some(handle) = &mut self.setup_step_handle {
                match handle.receiver.try_recv() {
                    Ok(Ok(result)) => {
                        self.handle_setup_step_result(result)?;
                        self.setup_step_handle = None;
                    }
                    Ok(Err(e)) => {
                        error!("Setup step failed: {}", e);
                        crate::services::StorageSetupService::cleanup_failed_setup(
                            &mut self.config,
                            &self.config_path,
                            true,
                        );
                        self.storage_setup_screen.get_state_mut().error_message =
                            Some(format!("Setup failed: {e}"));
                        self.storage_setup_screen.get_state_mut().step =
                            crate::screens::storage_setup::StorageSetupStep::Input;
                        self.setup_step_handle = None;
                    }
                    Err(oneshot::error::TryRecvError::Empty) => {
                        // Still running
                    }
                    Err(oneshot::error::TryRecvError::Closed) => {
                        warn!("Setup step channel closed unexpectedly");
                        self.setup_step_handle = None;
                    }
                }
            }

            if self.should_quit {
                break;
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

    /// Sync `input_mode_active` based on current focus states
    /// Called after event handling to keep input mode in sync with field focus
    fn sync_input_mode(&mut self) {
        use crate::ui::Screen;

        let is_input_focused = match self.ui_state.current_screen {
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

    /// Handle navigation-specific logic when navigating from `MainMenu`
    fn handle_menu_navigation(&mut self, target: Screen) -> Result<()> {
        match target {
            Screen::DotfileSelection => {
                // Check for changes when returning to menu
                self.trigger_git_status_check(false);
                // Reset state and scan dotfiles
                let state = self.dotfile_selection_screen.get_state_mut();
                state.status_message = None;
                state.backup_enabled = self.config.backup_enabled;
                self.dotfile_selection_screen.scan_dotfiles(&self.config)?;
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

    /// Process a `ScreenAction` returned from a screen's `handle_event` method.
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
                        Some(format!("Failed to save config: {e}"));
                    return Ok(());
                }

                // Verify git repository can be opened
                if let Err(e) = crate::git::GitManager::open_or_init(&repo_path) {
                    self.storage_setup_screen.get_state_mut().error_message =
                        Some(format!("Failed to open repository: {e}"));
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
                use crate::screens::storage_setup::StorageSetupStep;
                use crate::ui::GitHubSetupData;

                let data = GitHubSetupData {
                    token,
                    repo_name,
                    username: None,
                    repo_exists: None,
                    is_private,
                    delay_until: None,
                    is_new_repo: false,
                };

                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Processing(GitHubSetupStep::Connecting);
                state.status_message = Some("Connecting to GitHub...".to_string());
                state.setup_data = Some(data.clone());

                // Start async setup
                self.setup_step_handle = Some(crate::services::StorageSetupService::start_step(
                    &self.runtime,
                    GitHubSetupStep::Connecting,
                    data,
                    &self.config,
                ));
            }
            ScreenAction::UpdateGitHubToken { token } => {
                // Update the GitHub token with validation and remote URL update
                let github_config = if let Some(gh) = &self.config.github {
                    gh.clone()
                } else {
                    self.storage_setup_screen.get_state_mut().error_message =
                        Some("No GitHub configuration to update".to_string());
                    return Ok(());
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
                                Some(format!("Token cannot access repository {owner}/{repo}"));
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
                                Some(format!("Failed to save token: {e}"));
                            return Ok(());
                        }

                        // Show success and reset
                        self.storage_setup_screen.get_state_mut().status_message =
                            Some(format!("✅ Token updated for {owner}/{repo}"));
                        self.storage_setup_screen.get_state_mut().is_editing_token = false;
                        self.storage_setup_screen.get_state_mut().token_input =
                            crate::utils::TextInput::with_text("••••••••••••••••••••");
                    }
                    Err(e) => {
                        self.storage_setup_screen.get_state_mut().error_message =
                            Some(format!("Token validation failed: {e}"));
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
            // Profile selection actions - delegate to ProfileSelectionScreen
            ScreenAction::CreateAndActivateProfile { name } => {
                use crate::screens::profile_selection::ProfileSelectionAction;
                let result = self.profile_selection_screen.process_action(
                    ProfileSelectionAction::CreateAndActivateProfile { name },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::ActivateProfile { name } => {
                use crate::screens::profile_selection::ProfileSelectionAction;
                let result = self.profile_selection_screen.process_action(
                    ProfileSelectionAction::ActivateProfile { name },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            // Dotfile selection actions
            // Dotfile selection actions - delegate to screen
            ScreenAction::ScanDotfiles => {
                use crate::screens::dotfile_selection::DotfileAction;
                let result = self.dotfile_selection_screen.process_action(
                    DotfileAction::ScanDotfiles,
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::RefreshFileBrowser => {
                use crate::screens::dotfile_selection::DotfileAction;
                let result = self.dotfile_selection_screen.process_action(
                    DotfileAction::RefreshFileBrowser,
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::ToggleFileSync {
                file_index,
                is_synced,
            } => {
                use crate::screens::dotfile_selection::DotfileAction;
                let result = self.dotfile_selection_screen.process_action(
                    DotfileAction::ToggleFileSync {
                        file_index,
                        is_synced,
                    },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::AddCustomFileToSync {
                full_path,
                relative_path,
            } => {
                use crate::screens::dotfile_selection::DotfileAction;
                let result = self.dotfile_selection_screen.process_action(
                    DotfileAction::AddCustomFileToSync {
                        full_path,
                        relative_path,
                    },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::SetBackupEnabled { enabled } => {
                use crate::screens::dotfile_selection::DotfileAction;
                let result = self.dotfile_selection_screen.process_action(
                    DotfileAction::SetBackupEnabled { enabled },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
                // Also save config since backup_enabled is a config setting
                self.config.backup_enabled = enabled;
                self.config.save(&self.config_path)?;
            }
            // Profile management actions - delegate to ManageProfilesScreen
            ScreenAction::CreateProfile {
                name,
                description,
                copy_from,
            } => {
                use crate::screens::manage_profiles::ProfileAction;
                let result = self.manage_profiles_screen.process_action(
                    ProfileAction::CreateProfile {
                        name,
                        description,
                        copy_from,
                    },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::SwitchProfile { name } => {
                use crate::screens::manage_profiles::ProfileAction;
                let result = self.manage_profiles_screen.process_action(
                    ProfileAction::SwitchProfile { name },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::RenameProfile { old_name, new_name } => {
                use crate::screens::manage_profiles::ProfileAction;
                let result = self.manage_profiles_screen.process_action(
                    ProfileAction::RenameProfile { old_name, new_name },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::DeleteProfile { name } => {
                use crate::screens::manage_profiles::ProfileAction;
                let result = self.manage_profiles_screen.process_action(
                    ProfileAction::DeleteProfile { name },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
            ScreenAction::MoveToCommon {
                file_index,
                is_common,
                profiles_to_cleanup,
            } => {
                use crate::screens::dotfile_selection::DotfileAction;
                let result = self.dotfile_selection_screen.process_action(
                    DotfileAction::MoveToCommon {
                        file_index,
                        is_common,
                        profiles_to_cleanup,
                    },
                    &mut self.config,
                    &self.config_path,
                )?;
                self.handle_action_result(result)?;
            }
        }
        Ok(())
    }

    /// Handle an `ActionResult` from a screen's `process_action`
    fn handle_action_result(&mut self, result: ActionResult) -> Result<()> {
        match result {
            ActionResult::None => {}
            ActionResult::ShowToast { message, variant } => {
                self.toast_manager.push(Toast::new(message, variant));
            }
            ActionResult::ShowDialog {
                title,
                content,
                variant,
            } => {
                self.dialog_state = Some(DialogState {
                    title,
                    content,
                    variant,
                });
            }
            ActionResult::Navigate(screen) => {
                self.ui_state.current_screen = screen;
                self.call_on_enter(screen)?;
            }
            ActionResult::ConfigUpdated => {
                self.config = Config::load_or_create(&self.config_path)?;
            }
        }
        Ok(())
    }

    /// Call `on_enter` for the target screen when navigating
    fn call_on_enter(&mut self, target: Screen) -> Result<()> {
        use crate::screens::{Screen as ScreenTrait, ScreenContext};
        let ctx = ScreenContext::new(&self.config, &self.config_path);

        match target {
            Screen::MainMenu => self.main_menu_screen.on_enter(&ctx)?,
            Screen::DotfileSelection => self.dotfile_selection_screen.on_enter(&ctx)?,
            Screen::StorageSetup => self.storage_setup_screen.on_enter(&ctx)?,
            Screen::SyncWithRemote => self.sync_with_remote_screen.on_enter(&ctx)?,
            Screen::ManageProfiles => self.manage_profiles_screen.on_enter(&ctx)?,
            Screen::ProfileSelection => self.profile_selection_screen.on_enter(&ctx)?,
            Screen::ManagePackages => self.manage_packages_screen.on_enter(&ctx)?,
            Screen::Settings => self.settings_screen.on_enter(&ctx)?,
        }
        Ok(())
    }

    /// Handle the result of an async setup step
    fn handle_setup_step_result(&mut self, result: crate::services::StepResult) -> Result<()> {
        use crate::screens::storage_setup::StorageSetupStep;
        use crate::services::StepResult;

        match result {
            StepResult::Continue {
                next_step,
                setup_data,
                status_message,
                delay_ms,
            } => {
                let state = self.storage_setup_screen.get_state_mut();
                state.step = StorageSetupStep::Processing(next_step);
                state.status_message = Some(status_message);
                state.setup_data = Some(setup_data.clone());

                // If there's a delay, we schedule the next step after the delay
                if let Some(ms) = delay_ms {
                    // Set up delayed next step by updating delay_until in setup_data
                    let mut data_with_delay = setup_data.clone();
                    data_with_delay.delay_until =
                        Some(std::time::Instant::now() + std::time::Duration::from_millis(ms));
                    state.setup_data = Some(data_with_delay.clone());

                    // Start the next step immediately (it will handle the delay internally)
                    self.setup_step_handle =
                        Some(crate::services::StorageSetupService::start_step(
                            &self.runtime,
                            next_step,
                            data_with_delay,
                            &self.config,
                        ));
                } else {
                    // Start the next step immediately
                    self.setup_step_handle =
                        Some(crate::services::StorageSetupService::start_step(
                            &self.runtime,
                            next_step,
                            setup_data,
                            &self.config,
                        ));
                }
            }
            StepResult::Complete {
                setup_data: _,
                github_config,
                profiles,
                is_new_repo,
            } => {
                // Update config with GitHub info
                self.config.github = Some(github_config.clone());
                self.config.repo_name = github_config.repo;
                self.config.save(&self.config_path)?;

                // Reset screen state
                self.storage_setup_screen.reset();

                // Update profile selection screen with discovered profiles
                self.profile_selection_screen.set_profiles(profiles.clone());
                self.ui_state.profile_selection.profiles = profiles.clone();
                if !profiles.is_empty() {
                    self.ui_state.profile_selection.list_state.select(Some(0));
                }

                // Navigate based on profiles found
                if profiles.is_empty() {
                    self.ui_state.current_screen = Screen::MainMenu;
                } else if is_new_repo && profiles.len() == 1 {
                    // New repo with single profile - go to dotfile selection
                    self.config.active_profile = profiles[0].clone();
                    self.config.save(&self.config_path)?;

                    // Initialize dotfile selection screen
                    let dotfile_state = self.dotfile_selection_screen.get_state_mut();
                    dotfile_state.backup_enabled = self.config.backup_enabled;
                    dotfile_state.status_message = None;
                    self.dotfile_selection_screen.scan_dotfiles(&self.config)?;

                    self.ui_state.current_screen = Screen::DotfileSelection;
                    self.call_on_enter(Screen::DotfileSelection)?;
                } else {
                    // Multiple profiles - show selection
                    self.ui_state.current_screen = Screen::ProfileSelection;
                }
            }
            StepResult::Failed {
                error_message,
                cleanup_repo,
            } => {
                crate::services::StorageSetupService::cleanup_failed_setup(
                    &mut self.config,
                    &self.config_path,
                    cleanup_repo,
                );

                // Also reset UI state that may have been populated during failed setup
                self.ui_state.profile_selection.profiles.clear();
                self.ui_state.profile_selection.list_state.select(None);
                self.profile_selection_screen.set_profiles(Vec::new());

                let state = self.storage_setup_screen.get_state_mut();
                state.error_message = Some(error_message);
                state.step = StorageSetupStep::Input;
                state.setup_data = None;
            }
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
}
