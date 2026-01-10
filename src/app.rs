use crate::components::package_manager::PackageManagerComponent;
use crate::components::profile_manager::ProfilePopupType;
use crate::components::{
    Component, ComponentAction, DotfileSelectionComponent, GitHubAuthComponent, MainMenuComponent,
    MenuItem, MessageComponent, ProfileManagerComponent, PushChangesComponent,
    SyncedFilesComponent,
};
use crate::config::{Config, GitHubConfig};
use crate::file_manager::FileManager;
use crate::git::GitManager;
use crate::github::GitHubClient;
use crate::styles::LIST_HIGHLIGHT_SYMBOL;
use crate::tui::Tui;
use crate::ui::{
    AddPackageField, GitHubAuthField, GitHubAuthStep, GitHubSetupStep, InstallationStep,
    PackagePopupType, PackageStatus, Screen, UiState,
};
use crate::utils::profile_manifest::{Package, PackageManager};
use anyhow::{Context, Result};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use syntect::parsing::SyntaxSet;
use tokio::runtime::Runtime;
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

/// Main application state
pub struct App {
    config: Config,
    config_path: PathBuf,
    #[allow(dead_code)]
    file_manager: FileManager,
    tui: Tui,
    ui_state: UiState,
    should_quit: bool,
    runtime: Runtime,
    /// Track the last screen to detect screen transitions
    last_screen: Option<Screen>,
    /// Component instances for screens with mouse support
    main_menu_component: MainMenuComponent,
    github_auth_component: GitHubAuthComponent,
    dotfile_selection_component: DotfileSelectionComponent,
    synced_files_component: SyncedFilesComponent,
    push_changes_component: PushChangesComponent,
    profile_manager_component: ProfileManagerComponent,
    package_manager_component: PackageManagerComponent,
    message_component: Option<MessageComponent>,
    // Syntax highlighting assets
    syntax_set: SyntaxSet,
    theme_set: syntect::highlighting::ThemeSet,
}

impl App {
    pub fn new() -> Result<Self> {
        let config_path = crate::utils::get_config_path();
        info!("Loading configuration from: {:?}", config_path);

        let config =
            Config::load_or_create(&config_path).context("Failed to load or create config")?;
        info!(
            "Configuration loaded: active_profile={}, repo_path={:?}",
            config.active_profile, config.repo_path
        );

        let file_manager = FileManager::new()?;
        let tui = Tui::new()?;
        let ui_state = UiState::new();

        let runtime = Runtime::new().context("Failed to create tokio runtime")?;

        // Initialize syntax highlighting
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = syntect::highlighting::ThemeSet::load_defaults();

        let has_changes = false; // Will be checked on first draw
        let config_clone = config.clone();
        Ok(Self {
            config_path,
            config,
            file_manager,
            tui,
            ui_state,
            should_quit: false,
            runtime,
            last_screen: None,
            main_menu_component: MainMenuComponent::new(has_changes),
            github_auth_component: GitHubAuthComponent::new(),
            dotfile_selection_component: DotfileSelectionComponent::new(),
            synced_files_component: SyncedFilesComponent::new(config_clone),
            push_changes_component: PushChangesComponent::new(),
            profile_manager_component: ProfileManagerComponent::new(),
            package_manager_component: PackageManagerComponent::new(),

            message_component: None,
            syntax_set,
            theme_set,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        info!("Entering TUI mode");
        self.tui.enter()?;

        // Check for updates if enabled in config
        if self.config.updates.check_enabled {
            info!("Checking for updates...");
            if let Some(update_info) =
                crate::version_check::check_for_updates(self.config.updates.check_interval_hours)
            {
                info!(
                    "New version available: {} -> {}",
                    update_info.current_version, update_info.latest_version
                );
                self.main_menu_component.set_update_info(Some(update_info));
            } else {
                info!("No updates available or check skipped (cached)");
            }
        }

        // Check if profile is deactivated and show warning
        if !self.config.profile_activated && self.config.is_repo_configured() {
            warn!("Profile '{}' is deactivated", self.config.active_profile);
            // Profile is deactivated - show warning message
            self.message_component = Some(MessageComponent::new(
                "Profile Deactivated".to_string(),
                format!(
                    "⚠️  Your profile '{}' is currently deactivated.\n\n\
                    Your symlinks have been removed and original files restored.\n\n\
                    To reactivate your profile and restore symlinks, run:\n\
                    \n\
                    \x1b[1m  dotstate activate\x1b[0m\n\n\
                    Or press any key to continue to the main menu.",
                    self.config.active_profile
                ),
                Screen::MainMenu,
            ));
        }

        // Always start with main menu (which is now the welcome screen)
        self.ui_state.current_screen = Screen::MainMenu;
        // Set last_screen to None so first draw will detect the transition
        self.last_screen = None;
        info!("Starting main event loop");

        // Main event loop
        loop {
            self.draw()?;

            if self.should_quit {
                break;
            }

            // Process GitHub setup state machine if active (before polling events)
            if let GitHubAuthStep::SetupStep(_) = self.ui_state.github_auth.step {
                self.process_github_setup_step()?;
            }

            // Process package checking if active (before polling events)
            // Note: Package checking can happen even when not on ManagePackages screen (e.g., after profile activation)
            {
                let state = &mut self.ui_state.package_manager;
                if state.is_checking {
                    // Check if we need to wait for a delay
                    if let Some(delay_until) = state.checking_delay_until {
                        if std::time::Instant::now() < delay_until {
                            // Still waiting, don't process yet - continue to next iteration
                        } else {
                            // Delay complete, clear it and process check
                            state.checking_delay_until = None;
                        }
                    }
                    // Process check (delay handled above)
                    let _ = state; // Release borrow before calling method
                    self.process_package_check_step()?;
                }

                // Process installation if active
                {
                    let state = &mut self.ui_state.package_manager;
                    if !matches!(state.installation_step, InstallationStep::NotStarted) {
                        trace!("Event loop: Processing installation step");
                    }
                }
                // Check again after releasing borrow
                if !matches!(
                    self.ui_state.package_manager.installation_step,
                    InstallationStep::NotStarted
                ) {
                    self.process_installation_step()?;
                }
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
                self.check_changes_to_push();
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

                let state = &mut self.ui_state.package_manager;
                state.packages = packages;
                // Initialize statuses as Unknown, but don't auto-check
                if state.package_statuses.len() != state.packages.len() {
                    state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                }
            } else if self.last_screen == Some(Screen::ManagePackages) {
                // We just left ManagePackages - clear installation state to prevent it from showing elsewhere
                let state = &mut self.ui_state.package_manager;
                if !matches!(state.installation_step, InstallationStep::NotStarted) {
                    state.installation_step = InstallationStep::NotStarted;
                    state.installation_output.clear();
                    state.installation_delay_until = None;
                }
            }
            self.last_screen = Some(current_screen);
        }

        // Update components with current state
        if self.ui_state.current_screen == Screen::MainMenu {
            self.main_menu_component
                .set_has_changes_to_push(self.ui_state.has_changes_to_push);
            self.main_menu_component
                .set_selected(self.ui_state.selected_index);
            // Update changed files for status display
            self.main_menu_component
                .update_changed_files(self.ui_state.sync_with_remote.changed_files.clone());
        }

        // Update GitHub auth component state
        if self.ui_state.current_screen == Screen::GitHubAuth {
            *self.github_auth_component.get_auth_state_mut() = self.ui_state.github_auth.clone();
        }

        // DotfileSelectionComponent just handles Clear widget, state stays in ui_state

        // Update synced files component config (only if on that screen to avoid unnecessary clones)
        if self.ui_state.current_screen == Screen::ViewSyncedFiles {
            self.synced_files_component
                .update_config(self.config.clone());
        }

        // Load changed files when entering PushChanges screen
        if self.ui_state.current_screen == Screen::SyncWithRemote
            && !self.ui_state.sync_with_remote.is_syncing
        {
            // Only load if we don't have files yet
            if self.ui_state.sync_with_remote.changed_files.is_empty() {
                self.load_changed_files();
            }
        }

        // Get profiles from manifest before the draw closure to avoid borrow issues
        let profile_selection_profiles: Vec<crate::utils::ProfileInfo> =
            if self.ui_state.current_screen == Screen::ProfileSelection {
                self.get_profiles().unwrap_or_default()
            } else {
                Vec::new()
            };

        // Clone config for main menu to avoid borrow issues in closure
        let config_clone = self.config.clone();

        // Get packages for ManagePackages screen (before closure to avoid borrow issues)
        let packages_for_manage: Vec<crate::utils::profile_manifest::Package> =
            if self.ui_state.current_screen == Screen::ManagePackages {
                self.get_active_profile_info()
                    .ok()
                    .flatten()
                    .map(|p| p.packages.clone())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

        self.tui.terminal_mut().draw(|frame| {
            let area = frame.area();
            match self.ui_state.current_screen {
                Screen::Welcome => {
                    // Welcome screen removed - redirect to MainMenu
                    self.ui_state.current_screen = Screen::MainMenu;
                    self.main_menu_component.update_config(config_clone.clone());
                    let _ = self.main_menu_component.render(frame, area);
                }
                Screen::MainMenu => {
                    // Show deactivation warning message if present
                    if let Some(ref mut msg_component) = self.message_component {
                        let _ = msg_component.render(frame, area);
                    } else {
                        // Pass config to main menu for stats
                        self.main_menu_component.update_config(config_clone.clone());
                        let _ = self.main_menu_component.render(frame, area);
                    }
                }
                Screen::GitHubAuth => {
                    // Sync state back after render (component may update it)
                    self.github_auth_component.update_config(config_clone.clone());
                    let _ = self.github_auth_component.render(frame, area);
                    self.ui_state.github_auth = self.github_auth_component.get_auth_state().clone();
                }
                Screen::DotfileSelection => {
                    // Component handles all rendering including Clear
                    // Get theme reference before mutable borrows
                    let syntax_theme = {
                        use crate::styles::theme as ui_theme;
                        let theme_type = ui_theme().theme_type;
                        let preferred_names = match theme_type {
                            crate::styles::ThemeType::Light => vec![
                                "base16-ocean.light",
                                "Solarized (light)",
                                "GitHub",
                            ],
                            crate::styles::ThemeType::Dark | crate::styles::ThemeType::NoColor => vec![
                                "base16-ocean.dark",
                                "base16-eighties.dark",
                                "base16-mocha.dark",
                                "InspiredGitHub",
                            ],
                        };

                        let mut theme_opt = None;
                        for name in &preferred_names {
                            if let Some(theme) = self.theme_set.themes.get(*name) {
                                theme_opt = Some(theme);
                                break;
                            }
                        }
                        theme_opt.unwrap_or_else(|| {
                            self.theme_set.themes.values().next()
                                .expect("No syntect themes available")
                        })
                    };

                    if let Err(e) = self.dotfile_selection_component.render_with_state(
                        frame,
                        area,
                        &mut self.ui_state,
                        &config_clone,
                        &self.syntax_set,
                        syntax_theme,
                    ) {
                        eprintln!("Error rendering dotfile selection: {}", e);
                    }
                }
                Screen::ViewSyncedFiles => {
                    let _ = self.synced_files_component.render(frame, area);
                }
                Screen::SyncWithRemote => {
                    // Component handles all rendering including Clear
                    // Get theme reference before mutable borrows
                    let syntax_theme = {
                        use crate::styles::theme as ui_theme;
                        let theme_type = ui_theme().theme_type;
                        let preferred_names = match theme_type {
                            crate::styles::ThemeType::Light => vec![
                                "base16-ocean.light",
                                "Solarized (light)",
                                "GitHub",
                            ],
                            crate::styles::ThemeType::Dark | crate::styles::ThemeType::NoColor => vec![
                                "base16-ocean.dark",
                                "base16-eighties.dark",
                                "base16-mocha.dark",
                                "InspiredGitHub",
                            ],
                        };

                        let mut theme_opt = None;
                        for name in &preferred_names {
                            if let Some(theme) = self.theme_set.themes.get(*name) {
                                theme_opt = Some(theme);
                                break;
                            }
                        }
                        theme_opt.unwrap_or_else(|| {
                            self.theme_set.themes.values().next()
                                .expect("No syntect themes available")
                        })
                    };

                    if let Err(e) = self.push_changes_component.render_with_state(
                        frame,
                        area,
                        &mut self.ui_state.sync_with_remote,
                        &config_clone,
                        &self.syntax_set,
                        syntax_theme,
                    ) {
                        eprintln!("Error rendering sync with remote: {}", e);
                    }
                }
                Screen::ManageProfiles => {
                    // Component handles all rendering including Clear
                    // Profiles already obtained before closure (we need to get them here too)
                    // Actually, we can't call self.get_profiles() inside the closure
                    // We need to get them before the closure
                    // For now, let's get them from the manifest directly
                    let repo_path = config_clone.repo_path.clone();
                    let profiles: Vec<crate::utils::ProfileInfo> = crate::utils::ProfileManifest::load_or_backfill(&repo_path)
                        .unwrap_or_default()
                        .profiles;
                    if let Err(e) = self.profile_manager_component.render_with_config(frame, area, &config_clone, &profiles, &mut self.ui_state.profile_manager) {
                        eprintln!("Error rendering profile manager: {}", e);
                    }
                }
                Screen::ManagePackages => {
                    let state = &mut self.ui_state.package_manager;
                    let config = &config_clone;
                    if let Err(e) = self.package_manager_component.render_with_state(frame, area, state, config, &packages_for_manage) {
                        eprintln!("Error rendering package manager: {}", e);
                    }
                }
                Screen::ProfileSelection => {
                    // Render profile selection screen
                    let state = &mut self.ui_state.profile_selection;

                    // Check if warning popup should be shown
                    if state.show_exit_warning {
                        use crate::utils::center_popup;
                        use crate::components::footer::Footer;
                        use ratatui::widgets::{Block, Borders, Paragraph, Clear};
                        use ratatui::prelude::*;

                        let popup_area = center_popup(area, 60, 35);
                        frame.render_widget(Clear, popup_area);

                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(8), // Warning text
                                Constraint::Min(0),    // Spacer
                                Constraint::Length(2), // Footer
                            ])
                            .split(popup_area);

                        let warning_text = "⚠️  Profile Selection Required\n\n\
                            You MUST select a profile before continuing.\n\
                            Activating a profile will replace your current dotfiles with symlinks.\n\
                            This action cannot be undone without restoring from backups.\n\n\
                            Please select a profile or create a new one.\n\
                            Press Esc again to cancel and return to main menu.".to_string();

                        let warning = Paragraph::new(warning_text)
                            .block(Block::default()
                                .borders(Borders::ALL)
                                .title("Exit Profile Selection")
                                .title_alignment(Alignment::Center)
                                .border_style(Style::default().fg(Color::Yellow)))
                            .wrap(ratatui::widgets::Wrap { trim: true })
                            .alignment(Alignment::Center);
                        frame.render_widget(warning, chunks[0]);

                        // Footer with instructions
                        let footer_text = "Esc: Cancel & Return to Main Menu";
                        let _ = Footer::render(frame, chunks[2], footer_text);
                        return;
                    }

                    // Check if create popup should be shown
                    if state.show_create_popup {
                        use crate::utils::center_popup;
                        use crate::components::footer::Footer;
                        use crate::components::input_field::InputField;
                        use ratatui::widgets::{Block, Borders, Paragraph, Clear};
                        use ratatui::prelude::*;

                        let popup_area = center_popup(area, 60, 12);
                        frame.render_widget(Clear, popup_area);

                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(3), // Title
                                Constraint::Length(3), // Input field
                                Constraint::Min(0),    // Spacer
                                Constraint::Length(2), // Footer
                            ])
                            .split(popup_area);

                        let title = Paragraph::new("Create New Profile")
                            .block(Block::default()
                                .borders(Borders::ALL)
                                .title("New Profile")
                                .title_alignment(Alignment::Center)
                                .border_style(Style::default().fg(Color::Cyan)))
                            .alignment(Alignment::Center);
                        frame.render_widget(title, chunks[0]);

                        if let Err(e) = InputField::render(
                            frame,
                            chunks[1],
                            &state.create_name_input,
                            state.create_name_cursor,
                            true,
                            "Profile Name:",
                            Some("Enter profile name"),
                            Alignment::Left,
                            false,
                        ) {
                            error!("Failed to render input field: {}", e);
                        }

                        let footer_text = "Enter: Create  |  Esc: Cancel";
                        let _ = Footer::render(frame, chunks[3], footer_text);
                        return;
                    }

                    // Build items list (profile_selection_profiles already obtained before closure)
                    // Add "Create New Profile" option at the end
                    let mut items: Vec<ListItem> = state.profiles.iter()
                        .map(|name| {
                            let profile = profile_selection_profiles.iter().find(|p| p.name == *name);
                            let description = profile.and_then(|p| p.description.as_ref())
                                .map(|d| format!(" - {}", d))
                                .unwrap_or_default();
                            let file_count = profile.map(|p| p.synced_files.len()).unwrap_or(0);
                            let file_text = if file_count == 1 {
                                "1 file".to_string()
                            } else {
                                format!("{} files", file_count)
                            };
                            ListItem::new(format!("{} {}{}", name, file_text, description))
                        })
                        .collect();
                    // Add "Create New Profile" option
                    items.push(ListItem::new("➕ Create New Profile (blank)").style(Style::default().fg(Color::Green)));

                    // Render the screen inline
                    use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
                    use crate::components::header::Header;
                    use crate::components::footer::Footer;
                    use crate::utils::create_standard_layout;
                    use ratatui::style::{Style, Color, Modifier};

                    frame.render_widget(Clear, area);

                    let background = Block::default()
                        .style(Style::default().bg(Color::Reset));
                    frame.render_widget(background, area);

                    let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

                    let _ = Header::render(
                        frame,
                        header_chunk,
                        "Select Profile to Activate",
                        "Choose which profile to activate after cloning the repository"
                    );

                    let list = List::new(items)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .title("Available Profiles")
                                .border_style(Style::default().fg(Color::Cyan))
                        )
                        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                        .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

                    frame.render_stateful_widget(list, content_chunk, &mut state.list_state);

                    let footer_text = "↑↓: Navigate | Enter: Activate/Create | C: Create New | Esc: Cancel (requires confirmation)";
                    let _ = Footer::render(frame, footer_chunk, footer_text);
                }
            }

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
        use crate::ui::{DotfileSelectionFocus, PackagePopupType, Screen};

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
                self.ui_state.dotfile_selection.adding_custom_file
                    || (self.ui_state.dotfile_selection.file_browser_path_focused
                        && self.ui_state.dotfile_selection.focus
                            == DotfileSelectionFocus::FileBrowserInput)
            }

            // Profile Selection - create popup name input
            Screen::ProfileSelection => self.ui_state.profile_selection.show_create_popup,

            // Manage Profiles - create/rename/delete popups
            Screen::ManageProfiles => {
                use crate::components::profile_manager::ProfilePopupType;
                matches!(
                    self.ui_state.profile_manager.popup_type,
                    ProfilePopupType::Create | ProfilePopupType::Rename | ProfilePopupType::Delete
                )
            }

            // Package Manager - add/edit/delete popups with text input
            Screen::ManagePackages => {
                matches!(
                    self.ui_state.package_manager.popup_type,
                    PackagePopupType::Add | PackagePopupType::Edit | PackagePopupType::Delete
                )
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
                | Action::Help => Some(action),
                // Block navigation and other actions while typing
                _ => None,
            }
        } else {
            Some(action)
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        // Global keymap-based handlers (help overlay)
        if let Event::Key(key) = &event {
            if key.kind == KeyEventKind::Press {
                if let Some(action) = self.get_action(key.code, key.modifiers) {
                    use crate::keymap::Action;
                    if action == Action::Help {
                        // Toggle help overlay
                        self.ui_state.show_help_overlay = !self.ui_state.show_help_overlay;
                        return Ok(());
                    }
                }
            }
        }

        // Close help overlay on any key if it's showing
        if self.ui_state.show_help_overlay
            && matches!(event, Event::Key(k) if k.kind == KeyEventKind::Press)
        {
            self.ui_state.show_help_overlay = false;
            return Ok(());
        }

        // Handle message component events first (e.g., deactivation warning on MainMenu)
        if let Some(ref mut msg_component) = self.message_component {
            if self.ui_state.current_screen == Screen::MainMenu {
                let action = msg_component.handle_event(event)?;
                if let ComponentAction::Navigate(Screen::MainMenu) = action {
                    // User dismissed the warning, clear it and show main menu
                    self.message_component = None;
                }
                return Ok(());
            }
        }

        // Let components handle events first (for mouse support)
        match self.ui_state.current_screen {
            Screen::MainMenu => {
                // Handle keyboard events with keymap
                if let Event::Key(key) = &event {
                    if key.kind == KeyEventKind::Press {
                        if let Some(action) = self.get_action(key.code, key.modifiers) {
                            use crate::keymap::Action;
                            match action {
                                Action::MoveUp => {
                                    self.main_menu_component.move_up();
                                    self.ui_state.selected_index =
                                        self.main_menu_component.selected_index();
                                    return Ok(());
                                }
                                Action::MoveDown => {
                                    self.main_menu_component.move_down();
                                    self.ui_state.selected_index =
                                        self.main_menu_component.selected_index();
                                    return Ok(());
                                }
                                Action::Confirm => {
                                    // Check for update item selection
                                    if self.main_menu_component.is_update_item_selected() {
                                        self.show_update_info_popup();
                                    } else {
                                        self.handle_menu_selection()?;
                                    }
                                    return Ok(());
                                }
                                Action::Quit | Action::Cancel => {
                                    self.should_quit = true;
                                    return Ok(());
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Pass mouse events to component
                if matches!(event, Event::Mouse(_)) {
                    let comp_action = self.main_menu_component.handle_event(event)?;
                    match comp_action {
                        ComponentAction::Update => {
                            self.ui_state.selected_index =
                                self.main_menu_component.selected_index();
                            // Mouse click also triggers selection
                            if self.main_menu_component.is_update_item_selected() {
                                self.show_update_info_popup();
                            } else {
                                self.handle_menu_selection()?;
                            }
                        }
                        ComponentAction::Custom(ref action_name)
                            if action_name == "show_update_info" =>
                        {
                            self.show_update_info_popup();
                        }
                        _ => {}
                    }
                }
                return Ok(());
            }
            Screen::GitHubAuth => {
                // Let component handle mouse events, but keyboard events go to app
                if matches!(event, Event::Mouse(_)) {
                    let action = self.github_auth_component.handle_event(event)?;
                    if action == ComponentAction::Update {
                        // Sync state back
                        self.ui_state.github_auth =
                            self.github_auth_component.get_auth_state().clone();
                    }
                    return Ok(());
                }
                // Keyboard events handled in app (complex logic)
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        self.handle_github_auth_input(key)?;
                        // Sync state to component
                        *self.github_auth_component.get_auth_state_mut() =
                            self.ui_state.github_auth.clone();
                    }
                }
                return Ok(());
            }
            Screen::ViewSyncedFiles => {
                let action = self.synced_files_component.handle_event(event)?;
                if let ComponentAction::Navigate(Screen::MainMenu) = action {
                    self.ui_state.current_screen = Screen::MainMenu;
                }
                return Ok(());
            }
            Screen::SyncWithRemote => {
                // Handle push changes events with keymap
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        if let Some(action) = self.get_action(key.code, key.modifiers) {
                            use crate::keymap::Action;
                            match action {
                                Action::Confirm => {
                                    // Start pushing if not already pushing and we have changes
                                    if !self.ui_state.sync_with_remote.is_syncing
                                        && !self.ui_state.sync_with_remote.changed_files.is_empty()
                                    {
                                        self.start_sync()?;
                                    }
                                }
                                Action::Quit | Action::Cancel => {
                                    // Close result popup or go back
                                    if self.ui_state.sync_with_remote.show_result_popup {
                                        self.ui_state.sync_with_remote.show_result_popup = false;
                                        self.ui_state.sync_with_remote.sync_result = None;
                                        self.ui_state.sync_with_remote.pulled_changes_count = None;
                                        self.ui_state.current_screen = Screen::MainMenu;
                                        self.ui_state.sync_with_remote =
                                            crate::ui::SyncWithRemoteState::default();
                                        self.check_changes_to_push();
                                    } else {
                                        self.ui_state.current_screen = Screen::MainMenu;
                                        self.ui_state.sync_with_remote =
                                            crate::ui::SyncWithRemoteState::default();
                                    }
                                }
                                Action::MoveUp => {
                                    self.ui_state.sync_with_remote.list_state.select_previous();
                                    self.update_diff_preview();
                                }
                                Action::MoveDown => {
                                    self.ui_state.sync_with_remote.list_state.select_next();
                                    self.update_diff_preview();
                                }
                                Action::ScrollUp => {
                                    self.ui_state.sync_with_remote.preview_scroll = self
                                        .ui_state
                                        .sync_with_remote
                                        .preview_scroll
                                        .saturating_sub(1);
                                }
                                Action::ScrollDown => {
                                    self.ui_state.sync_with_remote.preview_scroll += 1;
                                }
                                Action::PageUp => {
                                    if let Some(current) =
                                        self.ui_state.sync_with_remote.list_state.selected()
                                    {
                                        let new_index = current.saturating_sub(10);
                                        self.ui_state
                                            .sync_with_remote
                                            .list_state
                                            .select(Some(new_index));
                                        self.update_diff_preview();
                                    }
                                }
                                Action::PageDown => {
                                    if let Some(current) =
                                        self.ui_state.sync_with_remote.list_state.selected()
                                    {
                                        let new_index = (current + 10).min(
                                            self.ui_state
                                                .sync_with_remote
                                                .changed_files
                                                .len()
                                                .saturating_sub(1),
                                        );
                                        self.ui_state
                                            .sync_with_remote
                                            .list_state
                                            .select(Some(new_index));
                                        self.update_diff_preview();
                                    }
                                }
                                Action::GoToTop => {
                                    self.ui_state.sync_with_remote.list_state.select_first();
                                    self.update_diff_preview();
                                }
                                Action::GoToEnd => {
                                    self.ui_state.sync_with_remote.list_state.select_last();
                                    self.update_diff_preview();
                                }
                                _ => {}
                            }
                        }
                    }
                } else if let Event::Mouse(mouse) = event {
                    // Handle mouse events for list navigation
                    if let MouseEventKind::ScrollUp = mouse.kind {
                        self.ui_state.sync_with_remote.list_state.select_previous();
                        self.update_diff_preview();
                    } else if let MouseEventKind::ScrollDown = mouse.kind {
                        self.ui_state.sync_with_remote.list_state.select_next();
                        self.update_diff_preview();
                    } else if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        // Click to sync or close popup
                        if self.ui_state.sync_with_remote.show_result_popup {
                            // After sync, go directly to main menu
                            self.ui_state.sync_with_remote.show_result_popup = false;
                            self.ui_state.sync_with_remote.sync_result = None;
                            self.ui_state.sync_with_remote.pulled_changes_count = None;
                            self.ui_state.current_screen = Screen::MainMenu;
                            // Reset sync state
                            self.ui_state.sync_with_remote =
                                crate::ui::SyncWithRemoteState::default();
                            // Re-check for changes after sync
                            self.check_changes_to_push();
                        } else if !self.ui_state.sync_with_remote.is_syncing
                            && !self.ui_state.sync_with_remote.changed_files.is_empty()
                        {
                            self.start_sync()?;
                        }
                    }
                }
                return Ok(());
            }
            Screen::DotfileSelection => {
                // 1. Modal
                let is_modal = self.ui_state.dotfile_selection.show_custom_file_confirm;
                if is_modal {
                    let state = &mut self.ui_state.dotfile_selection;
                    if let Event::Key(key) = event {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                    // YES logic
                                    let full_path = state.custom_file_confirm_path.clone().unwrap();
                                    let relative_path =
                                        state.custom_file_confirm_relative.clone().unwrap();

                                    // Close confirmation modal
                                    state.show_custom_file_confirm = false;
                                    state.custom_file_confirm_path = None;
                                    state.custom_file_confirm_relative = None;

                                    // Release borrow
                                    let _ = state;

                                    // Sync the file
                                    if let Err(e) =
                                        self.add_custom_file_to_sync(&full_path, &relative_path)
                                    {
                                        let state = &mut self.ui_state.dotfile_selection;
                                        state.status_message =
                                            Some(format!("Error: Failed to sync file: {}", e));
                                        return Ok(());
                                    }

                                    // Re-scan to refresh the list
                                    self.scan_dotfiles()?;

                                    // Find and select the file in the list
                                    let state = &mut self.ui_state.dotfile_selection;
                                    if let Some(index) = state.dotfiles.iter().position(|d| {
                                        d.relative_path.to_string_lossy() == relative_path
                                    }) {
                                        state.dotfile_list_state.select(Some(index));
                                        state.selected_for_sync.insert(index);
                                    }
                                }
                                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                    // NO logic
                                    state.show_custom_file_confirm = false;
                                    state.custom_file_confirm_path = None;
                                    state.custom_file_confirm_relative = None;
                                }
                                _ => {}
                            }
                        }
                    }
                    return Ok(());
                }

                // 2. Input Mode
                if self.ui_state.input_mode_active {
                    if let Event::Key(key) = event {
                        if key.kind == KeyEventKind::Press {
                            // Check for Tab/NextTab in file browser mode - allow focus switching even in input mode
                            if self.ui_state.dotfile_selection.file_browser_mode {
                                if let Some(action) = self.get_action(key.code, key.modifiers) {
                                    use crate::keymap::Action;
                                    if matches!(action, Action::NextTab) {
                                        // Handle Tab focus switching in file browser
                                        use crate::ui::DotfileSelectionFocus;
                                        let state = &mut self.ui_state.dotfile_selection;
                                        state.focus = match state.focus {
                                            DotfileSelectionFocus::FileBrowserList => {
                                                state.file_browser_path_focused = false;
                                                DotfileSelectionFocus::FileBrowserPreview
                                            }
                                            DotfileSelectionFocus::FileBrowserPreview => {
                                                state.file_browser_path_focused = true;
                                                DotfileSelectionFocus::FileBrowserInput
                                            }
                                            DotfileSelectionFocus::FileBrowserInput => {
                                                state.file_browser_path_focused = false;
                                                DotfileSelectionFocus::FileBrowserList
                                            }
                                            _ => {
                                                state.file_browser_path_focused = false;
                                                DotfileSelectionFocus::FileBrowserList
                                            }
                                        };
                                        return Ok(());
                                    }
                                }
                                // Other input handling for file browser
                                self.handle_file_browser_input(key.code)?;
                            } else if self.ui_state.dotfile_selection.adding_custom_file {
                                if key.code == KeyCode::Esc {
                                    let state = &mut self.ui_state.dotfile_selection;
                                    if !state.custom_file_focused {
                                        state.adding_custom_file = false;
                                        state.custom_file_input.clear();
                                        return Ok(());
                                    }
                                }
                                self.handle_custom_file_input(key.code)?;
                            }
                        }
                    }
                    return Ok(());
                }

                // 3. Normal / Keymap
                use crate::ui::DotfileSelectionFocus;
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        let action = self.get_action(key.code, key.modifiers);
                        enum DotfileIntent {
                            None,
                            ToggleSelection(usize),
                            ToggleBackup,
                            CreateCustom,
                        }
                        let mut intent = DotfileIntent::None;

                        if let Some(action) = action {
                            use crate::keymap::Action;
                            let state = &mut self.ui_state.dotfile_selection;
                            match action {
                                Action::MoveUp => {
                                    if state.focus == DotfileSelectionFocus::FilesList {
                                        state.dotfile_list_state.select_previous();
                                        state.preview_scroll = 0;
                                    } else if state.focus == DotfileSelectionFocus::Preview
                                        && state.preview_scroll > 0
                                    {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_sub(1);
                                    }
                                }
                                Action::MoveDown => {
                                    if state.focus == DotfileSelectionFocus::FilesList {
                                        state.dotfile_list_state.select_next();
                                        state.preview_scroll = 0;
                                    } else if state.focus == DotfileSelectionFocus::Preview {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_add(1);
                                    }
                                }
                                Action::Confirm => {
                                    if state.status_message.is_some() {
                                        state.status_message = None;
                                    } else if let Some(idx) = state.dotfile_list_state.selected() {
                                        intent = DotfileIntent::ToggleSelection(idx);
                                    }
                                }
                                Action::NextTab => {
                                    if state.file_browser_mode {
                                        // In file browser mode: cycle through List -> Preview -> Input -> List
                                        state.focus = match state.focus {
                                            DotfileSelectionFocus::FileBrowserList => {
                                                state.file_browser_path_focused = false;
                                                DotfileSelectionFocus::FileBrowserPreview
                                            }
                                            DotfileSelectionFocus::FileBrowserPreview => {
                                                state.file_browser_path_focused = true;
                                                DotfileSelectionFocus::FileBrowserInput
                                            }
                                            DotfileSelectionFocus::FileBrowserInput => {
                                                state.file_browser_path_focused = false;
                                                DotfileSelectionFocus::FileBrowserList
                                            }
                                            _ => {
                                                state.file_browser_path_focused = false;
                                                DotfileSelectionFocus::FileBrowserList
                                            }
                                        };
                                    } else {
                                        // Normal mode: switch between FilesList and Preview
                                        state.focus = match state.focus {
                                            DotfileSelectionFocus::FilesList => {
                                                DotfileSelectionFocus::Preview
                                            }
                                            DotfileSelectionFocus::Preview => {
                                                DotfileSelectionFocus::FilesList
                                            }
                                            _ => DotfileSelectionFocus::FilesList,
                                        };
                                    }
                                }
                                Action::PageUp => {
                                    if state.focus == DotfileSelectionFocus::FilesList {
                                        if let Some(current) = state.dotfile_list_state.selected() {
                                            let new_index = current.saturating_sub(10);
                                            state.dotfile_list_state.select(Some(new_index));
                                            state.preview_scroll = 0;
                                        }
                                    } else if state.focus == DotfileSelectionFocus::Preview
                                        && state.preview_scroll > 0
                                    {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_sub(20);
                                    }
                                }
                                Action::PageDown => {
                                    if state.focus == DotfileSelectionFocus::FilesList {
                                        if let Some(current) = state.dotfile_list_state.selected() {
                                            let new_index = (current + 10)
                                                .min(state.dotfiles.len().saturating_sub(1));
                                            state.dotfile_list_state.select(Some(new_index));
                                            state.preview_scroll = 0;
                                        } else if !state.dotfiles.is_empty() {
                                            state
                                                .dotfile_list_state
                                                .select(Some(10.min(state.dotfiles.len() - 1)));
                                            state.preview_scroll = 0;
                                        }
                                    } else if state.focus == DotfileSelectionFocus::Preview {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_add(20);
                                    }
                                }
                                Action::ScrollUp => {
                                    if state.focus == DotfileSelectionFocus::Preview
                                        && state.preview_scroll > 0
                                    {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_sub(10);
                                    }
                                }
                                Action::ScrollDown => {
                                    if state.focus == DotfileSelectionFocus::Preview {
                                        state.preview_scroll =
                                            state.preview_scroll.saturating_add(10);
                                    }
                                }
                                Action::GoToTop => {
                                    if state.focus == DotfileSelectionFocus::FilesList {
                                        state.dotfile_list_state.select_first();
                                        state.preview_scroll = 0;
                                    } else if state.focus == DotfileSelectionFocus::Preview {
                                        state.preview_scroll = 0;
                                    }
                                }
                                Action::GoToEnd => {
                                    if state.focus == DotfileSelectionFocus::FilesList {
                                        state.dotfile_list_state.select_last();
                                        state.preview_scroll = 0;
                                    }
                                }
                                Action::Create => {
                                    intent = DotfileIntent::CreateCustom;
                                }
                                Action::ToggleBackup => {
                                    intent = DotfileIntent::ToggleBackup;
                                }
                                Action::Cancel | Action::Quit => {
                                    self.ui_state.current_screen = Screen::MainMenu;
                                }
                                _ => {}
                            }
                        }

                        // Execute Intent
                        match intent {
                            DotfileIntent::ToggleSelection(idx) => {
                                // Need to release state borrow?
                                // state was borrowed in the loop.
                                // Wait, intent lets us drop state.
                                let was_selected = self
                                    .ui_state
                                    .dotfile_selection
                                    .selected_for_sync
                                    .contains(&idx);
                                if was_selected {
                                    self.remove_file_from_sync(idx)?;
                                } else {
                                    self.add_file_to_sync(idx)?;
                                }
                            }
                            DotfileIntent::CreateCustom => {
                                let state = &mut self.ui_state.dotfile_selection;
                                state.adding_custom_file = true;
                                state.file_browser_mode = true;
                                state.file_browser_path = crate::utils::get_home_dir();
                                state.file_browser_selected = 0;
                                state.file_browser_path_input =
                                    state.file_browser_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor =
                                    state.file_browser_path_input.chars().count();
                                state.file_browser_path_focused = false;
                                state.file_browser_preview_scroll = 0;
                                state.focus = DotfileSelectionFocus::FileBrowserList;
                                self.refresh_file_browser()?;
                            }
                            DotfileIntent::ToggleBackup => {
                                let state = &mut self.ui_state.dotfile_selection;
                                state.backup_enabled = !state.backup_enabled;
                                self.config.backup_enabled = state.backup_enabled;
                                self.config.save(&self.config_path)?;
                            }
                            DotfileIntent::None => {}
                        }
                    }
                }
                return Ok(());
            }
            Screen::ProfileSelection => {
                // Check warning status (using separate scope or simple check)
                let show_exit_warning = self.ui_state.profile_selection.show_exit_warning;

                if show_exit_warning {
                    if let Event::Key(key) = event {
                        if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
                            self.ui_state.profile_selection.show_exit_warning = false;
                            self.ui_state.current_screen = Screen::MainMenu;
                            self.ui_state.profile_selection = Default::default();
                        }
                    }
                    return Ok(());
                }

                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        // Check for keymap action first, WITHOUT borrowing state
                        let action = self.get_action(key.code, key.modifiers);

                        // Define intent enum to separate decision from execution
                        enum ConfirmIntent {
                            None,
                            Create(String),
                            Activate(String),
                        }

                        let mut confirm_intent = ConfirmIntent::None;

                        if let Some(action) = action {
                            use crate::keymap::Action;
                            match action {
                                Action::MoveUp => {
                                    let state = &mut self.ui_state.profile_selection;
                                    if state.show_create_popup {
                                        use crate::utils::text_input::handle_cursor_movement;
                                        handle_cursor_movement(
                                            &state.create_name_input,
                                            &mut state.create_name_cursor,
                                            key.code,
                                        );
                                    } else if let Some(current) = state.list_state.selected() {
                                        if current > 0 {
                                            state.list_state.select(Some(current - 1));
                                        } else {
                                            state.list_state.select(Some(state.profiles.len()));
                                        }
                                    } else if !state.profiles.is_empty() {
                                        state.list_state.select(Some(state.profiles.len()));
                                    }
                                }
                                Action::MoveDown => {
                                    let state = &mut self.ui_state.profile_selection;
                                    if state.show_create_popup {
                                        use crate::utils::text_input::handle_cursor_movement;
                                        handle_cursor_movement(
                                            &state.create_name_input,
                                            &mut state.create_name_cursor,
                                            key.code,
                                        );
                                    } else if let Some(current) = state.list_state.selected() {
                                        if current < state.profiles.len() {
                                            state.list_state.select(Some(current + 1));
                                        } else {
                                            state.list_state.select(Some(0));
                                        }
                                    } else if !state.profiles.is_empty() {
                                        state.list_state.select(Some(0));
                                    }
                                }
                                Action::Confirm => {
                                    // Collect intent using state
                                    let state = &mut self.ui_state.profile_selection;
                                    if state.show_create_popup {
                                        let profile_name =
                                            state.create_name_input.trim().to_string();
                                        if !profile_name.is_empty() {
                                            state.show_create_popup = false;
                                            confirm_intent = ConfirmIntent::Create(profile_name);
                                        }
                                    } else if let Some(idx) = state.list_state.selected() {
                                        if idx == state.profiles.len() {
                                            // Create new profile selected
                                            state.show_create_popup = true;
                                            state.create_name_input.clear();
                                            state.create_name_cursor = 0;
                                        } else if let Some(name) = state.profiles.get(idx) {
                                            confirm_intent = ConfirmIntent::Activate(name.clone());
                                        }
                                    }
                                }
                                Action::Quit | Action::Cancel => {
                                    let state = &mut self.ui_state.profile_selection;
                                    if state.show_create_popup {
                                        state.show_create_popup = false;
                                        state.create_name_input.clear();
                                    } else {
                                        state.show_exit_warning = true;
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // Raw input for popup
                            let state = &mut self.ui_state.profile_selection;
                            if state.show_create_popup {
                                use crate::utils::text_input::handle_input;
                                handle_input(
                                    &mut state.create_name_input,
                                    &mut state.create_name_cursor,
                                    key.code,
                                );
                            }
                        }

                        // Execute intent (without state borrow)
                        match confirm_intent {
                            ConfirmIntent::Create(name) => {
                                match self.create_profile(&name, None, None) {
                                    Ok(_) => {
                                        let manifest = self.load_manifest()?;
                                        let state = &mut self.ui_state.profile_selection;
                                        state.profiles = manifest
                                            .profiles
                                            .iter()
                                            .map(|p| p.name.clone())
                                            .collect();
                                        if let Some(idx) =
                                            state.profiles.iter().position(|n| n == &name)
                                        {
                                            state.list_state.select(Some(idx));
                                        }

                                        // Activate logic
                                        if let Err(e) = self.activate_profile_after_setup(&name) {
                                            error!("Failed to activate: {}", e);
                                            self.message_component = Some(MessageComponent::new(
                                                "Activation Failed".to_string(),
                                                e.to_string(),
                                                Screen::MainMenu,
                                            ));
                                        } else {
                                            self.ui_state.current_screen = Screen::MainMenu;
                                            self.ui_state.profile_selection = Default::default();
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to create profile: {}", e);
                                        let state = &mut self.ui_state.profile_selection;
                                        state.show_create_popup = true;
                                        self.message_component = Some(MessageComponent::new(
                                            "Creation Failed".to_string(),
                                            format!("Failed to create profile: {}", e),
                                            Screen::ProfileSelection,
                                        ));
                                        // return Ok(()); // Fall through
                                    }
                                }
                            }
                            ConfirmIntent::Activate(name) => {
                                if let Err(e) = self.activate_profile_after_setup(&name) {
                                    error!("Failed to activate: {}", e);
                                    self.message_component = Some(MessageComponent::new(
                                        "Activation Failed".to_string(),
                                        e.to_string(),
                                        Screen::MainMenu,
                                    ));
                                } else {
                                    self.ui_state.current_screen = Screen::MainMenu;
                                    self.ui_state.profile_selection = Default::default();
                                }
                            }
                            ConfirmIntent::None => {}
                        }
                    }
                }
                return Ok(());
            }
            Screen::ManagePackages => {
                // Handle package manager events

                // Handle popup events FIRST - popups capture all events (like profile manager does)
                if self.ui_state.package_manager.popup_type != PackagePopupType::None {
                    let state = &mut self.ui_state.package_manager;
                    // Handle popup events inline
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            match state.popup_type {
                                PackagePopupType::Add | PackagePopupType::Edit => {
                                    use crate::ui::AddPackageField;
                                    match key.code {
                                        KeyCode::Esc => {
                                            state.popup_type = PackagePopupType::None;
                                        }
                                        KeyCode::Tab => {
                                            // Switch to next/previous field
                                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                                // Shift+Tab: previous field (go backwards)
                                                state.add_focused_field =
                                                    match state.add_focused_field {
                                                        AddPackageField::Name => {
                                                            // Wrap to last field
                                                            if state.add_is_custom {
                                                                AddPackageField::ExistenceCheck
                                                            } else {
                                                                AddPackageField::BinaryName
                                                            }
                                                        }
                                                        AddPackageField::Description => {
                                                            AddPackageField::Name
                                                        }
                                                        AddPackageField::Manager => {
                                                            AddPackageField::Description
                                                        }
                                                        AddPackageField::PackageName => {
                                                            AddPackageField::Manager
                                                        }
                                                        AddPackageField::BinaryName => {
                                                            if state.add_is_custom {
                                                                AddPackageField::Manager
                                                            } else {
                                                                AddPackageField::PackageName
                                                            }
                                                        }
                                                        AddPackageField::InstallCommand => {
                                                            AddPackageField::BinaryName
                                                        }
                                                        AddPackageField::ExistenceCheck => {
                                                            AddPackageField::InstallCommand
                                                        }
                                                        AddPackageField::ManagerCheck => {
                                                            // ManagerCheck is not shown in UI, but exists in enum
                                                            if state.add_is_custom {
                                                                AddPackageField::ExistenceCheck
                                                            } else {
                                                                AddPackageField::BinaryName
                                                            }
                                                        }
                                                    };
                                            } else {
                                                // Tab: next field (go forwards)
                                                state.add_focused_field =
                                                    match state.add_focused_field {
                                                        AddPackageField::Name => {
                                                            AddPackageField::Description
                                                        }
                                                        AddPackageField::Description => {
                                                            AddPackageField::Manager
                                                        }
                                                        AddPackageField::Manager => {
                                                            if state.add_is_custom {
                                                                AddPackageField::BinaryName
                                                            } else {
                                                                AddPackageField::PackageName
                                                            }
                                                        }
                                                        AddPackageField::PackageName => {
                                                            AddPackageField::BinaryName
                                                        }
                                                        AddPackageField::BinaryName => {
                                                            if state.add_is_custom {
                                                                AddPackageField::InstallCommand
                                                            } else {
                                                                AddPackageField::Name
                                                                // Wrap around for managed packages
                                                            }
                                                        }
                                                        AddPackageField::InstallCommand => {
                                                            AddPackageField::ExistenceCheck
                                                        }
                                                        AddPackageField::ExistenceCheck => {
                                                            AddPackageField::Name
                                                        } // Wrap around
                                                        AddPackageField::ManagerCheck => {
                                                            // ManagerCheck is not shown in UI, but exists in enum
                                                            AddPackageField::Name
                                                        }
                                                    };
                                            }
                                        }
                                        KeyCode::Up
                                        | KeyCode::Down
                                        | KeyCode::Left
                                        | KeyCode::Right => {
                                            if state.add_focused_field == AddPackageField::Manager {
                                                // Navigate through managers with arrow keys
                                                let manager_count = state.available_managers.len();
                                                if manager_count > 0 {
                                                    match key.code {
                                                        KeyCode::Right | KeyCode::Down => {
                                                            state.add_manager_selected =
                                                                (state.add_manager_selected + 1)
                                                                    % manager_count;
                                                        }
                                                        KeyCode::Left | KeyCode::Up => {
                                                            state.add_manager_selected = if state
                                                                .add_manager_selected
                                                                == 0
                                                            {
                                                                manager_count - 1
                                                            } else {
                                                                state.add_manager_selected - 1
                                                            };
                                                        }
                                                        _ => {}
                                                    }
                                                    state.add_manager = Some(
                                                        state.available_managers
                                                            [state.add_manager_selected]
                                                            .clone(),
                                                    );
                                                    state.add_is_custom = matches!(
                                                        state.available_managers
                                                            [state.add_manager_selected],
                                                        PackageManager::Custom
                                                    );
                                                }
                                            }
                                        }
                                        KeyCode::Char(' ') => {
                                            if state.add_focused_field == AddPackageField::Manager {
                                                // Space toggles/selects the current manager
                                                let manager_count = state.available_managers.len();
                                                if manager_count > 0 {
                                                    state.add_manager = Some(
                                                        state.available_managers
                                                            [state.add_manager_selected]
                                                            .clone(),
                                                    );
                                                    state.add_is_custom = matches!(
                                                        state.available_managers
                                                            [state.add_manager_selected],
                                                        PackageManager::Custom
                                                    );
                                                }
                                            } else {
                                                // Space in text fields - pass through to handle_package_popup_event
                                                self.handle_package_popup_event(event)?;
                                            }
                                        }
                                        KeyCode::Enter => {
                                            if state.add_focused_field == AddPackageField::Manager {
                                                // Enter selects the current manager
                                                let manager_count = state.available_managers.len();
                                                if manager_count > 0 {
                                                    state.add_manager = Some(
                                                        state.available_managers
                                                            [state.add_manager_selected]
                                                            .clone(),
                                                    );
                                                    state.add_is_custom = matches!(
                                                        state.available_managers
                                                            [state.add_manager_selected],
                                                        PackageManager::Custom
                                                    );
                                                }
                                            } else {
                                                // Save package
                                                // Release borrow before calling method
                                                let _ = state;
                                                if self.validate_and_save_package()? {
                                                    self.ui_state.package_manager.popup_type =
                                                        PackagePopupType::None;
                                                }
                                            }
                                        }
                                        _ => {
                                            // Delegate text input to handle_package_popup_event
                                            self.handle_package_popup_event(event)?;
                                        }
                                    }
                                }
                                PackagePopupType::Delete => {
                                    match key.code {
                                        KeyCode::Esc => {
                                            state.popup_type = PackagePopupType::None;
                                            state.delete_index = None;
                                            state.delete_confirm_input.clear();
                                            state.delete_confirm_cursor = 0;
                                        }
                                        KeyCode::Enter => {
                                            if state.delete_confirm_input.trim() == "DELETE" {
                                                if let Some(idx) = state.delete_index {
                                                    // Release borrow before calling method
                                                    let _ = state;
                                                    self.delete_package(idx)?;
                                                    // Re-borrow after method returns
                                                    let state = &mut self.ui_state.package_manager;
                                                    state.popup_type = PackagePopupType::None;
                                                    state.delete_index = None;
                                                    state.delete_confirm_input.clear();
                                                    state.delete_confirm_cursor = 0;
                                                }
                                            }
                                        }
                                        _ => {
                                            // Delegate text input to handle_package_popup_event
                                            self.handle_package_popup_event(event)?;
                                        }
                                    }
                                }
                                PackagePopupType::InstallMissing => {
                                    match key.code {
                                        KeyCode::Char('y')
                                        | KeyCode::Char('Y')
                                        | KeyCode::Enter => {
                                            // User confirmed - start installation
                                            let mut packages_to_install = Vec::new();
                                            for (idx, status) in
                                                state.package_statuses.iter().enumerate()
                                            {
                                                if matches!(status, PackageStatus::NotInstalled) {
                                                    packages_to_install.push(idx);
                                                }
                                            }

                                            if !packages_to_install.is_empty() {
                                                // Start installation
                                                if let Some(&first_idx) =
                                                    packages_to_install.first()
                                                {
                                                    let package_name =
                                                        state.packages[first_idx].name.clone();
                                                    let total = packages_to_install.len();
                                                    let mut install_list =
                                                        packages_to_install.clone();
                                                    install_list.remove(0);

                                                    state.installation_step =
                                                        InstallationStep::Installing {
                                                            package_index: first_idx,
                                                            package_name,
                                                            total_packages: total,
                                                            packages_to_install: install_list,
                                                            installed: Vec::new(),
                                                            failed: Vec::new(),
                                                            status_rx: None,
                                                        };
                                                    state.installation_output.clear();
                                                    state.installation_delay_until = Some(
                                                        std::time::Instant::now()
                                                            + Duration::from_millis(100),
                                                    );
                                                }
                                            }
                                            state.popup_type = PackagePopupType::None;
                                        }
                                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                            // User cancelled
                                            state.popup_type = PackagePopupType::None;
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ => {
                            // Delegate other events (like text input) to handle_package_popup_event
                            self.handle_package_popup_event(event)?;
                        }
                    }
                    return Ok(());
                }

                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        // Handle installation completion dismissal first (Local scope)
                        {
                            let state = &mut self.ui_state.package_manager;
                            if matches!(state.installation_step, InstallationStep::Complete { .. })
                            {
                                state.installation_step = InstallationStep::NotStarted;
                                state.installation_output.clear();
                                state.installation_delay_until = None;
                            }
                        }

                        let action = self.get_action(key.code, key.modifiers);
                        if let Some(action) = action {
                            use crate::keymap::Action;
                            let state = &mut self.ui_state.package_manager;
                            match action {
                                Action::MoveUp => {
                                    if !state.is_checking {
                                        state.list_state.select_previous();
                                    }
                                }
                                Action::MoveDown => {
                                    if !state.is_checking {
                                        state.list_state.select_next();
                                    }
                                }
                                Action::Refresh => {
                                    // Check All (Old 'c')
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                        && !state.packages.is_empty()
                                    {
                                        info!(
                                            "Starting check all packages ({} packages)",
                                            state.packages.len()
                                        );
                                        if state.package_statuses.len() != state.packages.len() {
                                            state.package_statuses =
                                                vec![PackageStatus::Unknown; state.packages.len()];
                                        }
                                        state.package_statuses =
                                            vec![PackageStatus::Unknown; state.packages.len()];
                                        state.is_checking = true;
                                        state.checking_index = None;
                                        state.checking_delay_until = Some(
                                            std::time::Instant::now() + Duration::from_millis(100),
                                        );
                                    }
                                }
                                Action::Sync | Action::Confirm => {
                                    // Check Selected (Old 's') + Enter
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                    {
                                        if let Some(selected_idx) = state.list_state.selected() {
                                            if selected_idx < state.packages.len() {
                                                let package_name =
                                                    state.packages[selected_idx].name.clone();
                                                info!("Starting check selected package: {} (index: {})", package_name, selected_idx);
                                                if state.package_statuses.len()
                                                    != state.packages.len()
                                                {
                                                    state.package_statuses =
                                                        vec![
                                                            PackageStatus::Unknown;
                                                            state.packages.len()
                                                        ];
                                                }
                                                state.package_statuses[selected_idx] =
                                                    PackageStatus::Unknown;
                                                state.is_checking = true;
                                                state.checking_index = Some(selected_idx);
                                                state.checking_delay_until = Some(
                                                    std::time::Instant::now()
                                                        + Duration::from_millis(100),
                                                );
                                            }
                                        }
                                    }
                                }
                                Action::Install => {
                                    // Install Missing (Old 'i')
                                    if state.popup_type == PackagePopupType::None
                                        && matches!(
                                            state.installation_step,
                                            InstallationStep::NotStarted
                                        )
                                        && !state.is_checking
                                    {
                                        let mut packages_to_install = Vec::new();
                                        for (idx, status) in
                                            state.package_statuses.iter().enumerate()
                                        {
                                            if matches!(status, PackageStatus::NotInstalled) {
                                                packages_to_install.push(idx);
                                            }
                                        }
                                        if !packages_to_install.is_empty() {
                                            info!(
                                                "Starting installation of {} missing package(s)",
                                                packages_to_install.len()
                                            );
                                            if let Some(&first_idx) = packages_to_install.first() {
                                                let package_name =
                                                    state.packages[first_idx].name.clone();
                                                let total = packages_to_install.len();
                                                let mut install_list = packages_to_install.clone();
                                                install_list.remove(0);

                                                state.installation_step =
                                                    InstallationStep::Installing {
                                                        package_index: first_idx,
                                                        package_name,
                                                        total_packages: total,
                                                        packages_to_install: install_list,
                                                        installed: Vec::new(),
                                                        failed: Vec::new(),
                                                        status_rx: None,
                                                    };
                                                state.installation_output.clear();
                                                state.installation_delay_until = Some(
                                                    std::time::Instant::now()
                                                        + Duration::from_millis(100),
                                                );
                                            }
                                        }
                                    }
                                }
                                Action::Create => {
                                    // Add Package (Old 'a')
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                    {
                                        let _ = state;
                                        self.start_add_package()?;
                                    }
                                }
                                Action::Edit => {
                                    // Edit Package (Old 'e')
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                    {
                                        if let Some(selected_idx) = state.list_state.selected() {
                                            if selected_idx < state.packages.len() {
                                                let _ = state;
                                                self.start_edit_package(selected_idx)?;
                                            }
                                        }
                                    }
                                }
                                Action::Delete => {
                                    // Delete Package (Old 'd')
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                    {
                                        if let Some(selected_idx) = state.list_state.selected() {
                                            if selected_idx < state.packages.len() {
                                                state.delete_index = Some(selected_idx);
                                                state.popup_type = PackagePopupType::Delete;
                                                state.delete_confirm_input.clear();
                                                state.delete_confirm_cursor = 0;
                                            }
                                        }
                                    }
                                }
                                Action::Cancel | Action::Quit => {
                                    if !state.is_checking {
                                        state.installation_step = InstallationStep::NotStarted;
                                        state.installation_output.clear();
                                        state.installation_delay_until = None;
                                        self.ui_state.current_screen = Screen::MainMenu;
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // Raw Fallback
                            let state = &mut self.ui_state.package_manager;
                            match key.code {
                                // Fallback for 's' if not mapped to Sync/Confirm (though Enter handles Confirm)
                                KeyCode::Char('s') | KeyCode::Char('S') => {
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                    {
                                        if let Some(selected_idx) = state.list_state.selected() {
                                            if selected_idx < state.packages.len() {
                                                // Copy Check Selected Logic
                                                let _package_name =
                                                    state.packages[selected_idx].name.clone();
                                                if state.package_statuses.len()
                                                    != state.packages.len()
                                                {
                                                    state.package_statuses =
                                                        vec![
                                                            PackageStatus::Unknown;
                                                            state.packages.len()
                                                        ];
                                                }
                                                state.package_statuses[selected_idx] =
                                                    PackageStatus::Unknown;
                                                state.is_checking = true;
                                                state.checking_index = Some(selected_idx);
                                                state.checking_delay_until = Some(
                                                    std::time::Instant::now()
                                                        + Duration::from_millis(100),
                                                );
                                            }
                                        }
                                    }
                                }
                                // Fallback for 'c' if not mapped to Refresh/Create
                                KeyCode::Char('c') | KeyCode::Char('C') => {
                                    // If actions didn't catch it, maybe it's Check All fallback?
                                    if state.popup_type == PackagePopupType::None
                                        && !state.is_checking
                                        && !state.packages.is_empty()
                                    {
                                        if state.package_statuses.len() != state.packages.len() {
                                            state.package_statuses =
                                                vec![PackageStatus::Unknown; state.packages.len()];
                                        }
                                        state.package_statuses =
                                            vec![PackageStatus::Unknown; state.packages.len()];
                                        state.is_checking = true;
                                        state.checking_index = None;
                                        state.checking_delay_until = Some(
                                            std::time::Instant::now() + Duration::from_millis(100),
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }

                return Ok(());
            }
            Screen::ManageProfiles => {
                // Get profiles from manifest
                let profiles = self.get_profiles().unwrap_or_default();

                // Handle popup events first
                if self.ui_state.profile_manager.popup_type != ProfilePopupType::None {
                    let state = &mut self.ui_state.profile_manager;
                    // Handle popup events inline
                    match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            match state.popup_type {
                                ProfilePopupType::Create => {
                                    use crate::components::profile_manager::CreateField;

                                    match key.code {
                                        KeyCode::Esc => {
                                            state.popup_type = ProfilePopupType::None;
                                        }
                                        KeyCode::Tab => {
                                            // Switch to next field
                                            if key.modifiers.contains(KeyModifiers::SHIFT) {
                                                // Shift+Tab: go to previous field
                                                state.create_focused_field = match state
                                                    .create_focused_field
                                                {
                                                    CreateField::Name => CreateField::CopyFrom,
                                                    CreateField::Description => CreateField::Name,
                                                    CreateField::CopyFrom => {
                                                        CreateField::Description
                                                    }
                                                };
                                            } else {
                                                // Tab: go to next field
                                                state.create_focused_field = match state
                                                    .create_focused_field
                                                {
                                                    CreateField::Name => CreateField::Description,
                                                    CreateField::Description => {
                                                        CreateField::CopyFrom
                                                    }
                                                    CreateField::CopyFrom => CreateField::Name,
                                                };
                                            }
                                        }
                                        KeyCode::BackTab => {
                                            // Shift+Tab: go to previous field
                                            state.create_focused_field = match state
                                                .create_focused_field
                                            {
                                                CreateField::Name => CreateField::CopyFrom,
                                                CreateField::Description => CreateField::Name,
                                                CreateField::CopyFrom => CreateField::Description,
                                            };
                                        }
                                        KeyCode::Up => {
                                            // Navigate Copy From list (index 0 = "Start Blank", 1+ = profiles)
                                            if state.create_focused_field == CreateField::CopyFrom {
                                                // Convert to UI index: None = 0, Some(idx) = idx + 1
                                                let ui_current =
                                                    if let Some(idx) = state.create_copy_from {
                                                        idx + 1
                                                    } else {
                                                        0
                                                    };

                                                if ui_current > 0 {
                                                    // Move up: if at profile, go to previous profile or "Start Blank"
                                                    if ui_current == 1 {
                                                        state.create_copy_from = None;
                                                    // Go to "Start Blank"
                                                    } else {
                                                        state.create_copy_from =
                                                            Some(ui_current - 2);
                                                        // Previous profile
                                                    }
                                                } else {
                                                    // At "Start Blank", wrap to last profile
                                                    if !profiles.is_empty() {
                                                        state.create_copy_from =
                                                            Some(profiles.len() - 1);
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Down => {
                                            // Navigate Copy From list (index 0 = "Start Blank", 1+ = profiles)
                                            if state.create_focused_field == CreateField::CopyFrom {
                                                // Convert to UI index: None = 0, Some(idx) = idx + 1
                                                let ui_current =
                                                    if let Some(idx) = state.create_copy_from {
                                                        idx + 1
                                                    } else {
                                                        0
                                                    };

                                                let max_ui_idx = profiles.len(); // Last UI index (profiles.len() because "Start Blank" is at 0)

                                                if ui_current < max_ui_idx {
                                                    // Move down: if at "Start Blank", go to first profile, otherwise next profile
                                                    if ui_current == 0 {
                                                        state.create_copy_from = Some(0);
                                                    // First profile
                                                    } else {
                                                        state.create_copy_from = Some(ui_current);
                                                        // Next profile
                                                    }
                                                } else {
                                                    // At last profile, wrap to "Start Blank"
                                                    state.create_copy_from = None;
                                                }
                                            }
                                        }
                                        KeyCode::Char(' ') => {
                                            // Toggle Copy From selection when space is pressed (only if Copy From is focused)
                                            if state.create_focused_field == CreateField::CopyFrom {
                                                // Get current UI index (0 = "Start Blank", 1+ = profiles)
                                                let ui_current =
                                                    if let Some(idx) = state.create_copy_from {
                                                        idx + 1
                                                    } else {
                                                        0
                                                    };

                                                if ui_current == 0 {
                                                    // "Start Blank" is already selected, keep it selected
                                                    state.create_copy_from = None;
                                                } else {
                                                    // Toggle profile selection
                                                    let profile_idx = ui_current - 1;
                                                    if state.create_copy_from == Some(profile_idx) {
                                                        state.create_copy_from = None;
                                                    // Deselect, go to "Start Blank"
                                                    } else {
                                                        state.create_copy_from = Some(profile_idx);
                                                        // Select this profile
                                                    }
                                                }
                                            } else {
                                                // Space is a regular character for Name and Description fields
                                                match state.create_focused_field {
                                                    CreateField::Name => {
                                                        crate::utils::text_input::handle_char_insertion(&mut state.create_name_input, &mut state.create_name_cursor, ' ');
                                                    }
                                                    CreateField::Description => {
                                                        crate::utils::text_input::handle_char_insertion(&mut state.create_description_input, &mut state.create_description_cursor, ' ');
                                                    }
                                                    CreateField::CopyFrom => {
                                                        // Already handled above
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Enter => {
                                            // Enter always creates the profile (if name is filled)
                                            // If Copy From is focused, select the current item first, then create
                                            if state.create_focused_field == CreateField::CopyFrom {
                                                // Get current UI index (0 = "Start Blank", 1+ = profiles)
                                                let ui_current =
                                                    if let Some(idx) = state.create_copy_from {
                                                        idx + 1
                                                    } else {
                                                        0
                                                    };

                                                if ui_current == 0 {
                                                    // "Start Blank" is selected, keep it
                                                    state.create_copy_from = None;
                                                } else {
                                                    // Select the current profile
                                                    let profile_idx = ui_current - 1;
                                                    state.create_copy_from = Some(profile_idx);
                                                }
                                            }

                                            // Create profile (Enter always creates, regardless of focus)
                                            if !state.create_name_input.is_empty() {
                                                let name = state.create_name_input.clone();
                                                let description =
                                                    if state.create_description_input.is_empty() {
                                                        None
                                                    } else {
                                                        Some(state.create_description_input.clone())
                                                    };
                                                let copy_from = state.create_copy_from;
                                                // Clone values before releasing borrow
                                                let name_clone = name.clone();
                                                let description_clone = description.clone();
                                                // Release borrow by ending scope
                                                {
                                                    let _ = state;
                                                }
                                                match self.create_profile(
                                                    &name_clone,
                                                    description_clone,
                                                    copy_from,
                                                ) {
                                                    Ok(_) => {
                                                        // Refresh config
                                                        self.config = Config::load_or_create(
                                                            &self.config_path,
                                                        )?;
                                                        self.ui_state.profile_manager.popup_type =
                                                            ProfilePopupType::None;
                                                        self.ui_state
                                                            .profile_manager
                                                            .create_name_input
                                                            .clear();
                                                        self.ui_state
                                                            .profile_manager
                                                            .create_description_input
                                                            .clear();
                                                        self.ui_state
                                                            .profile_manager
                                                            .create_focused_field =
                                                            CreateField::Name;
                                                        // Refresh list
                                                        if let Ok(profiles) = self.get_profiles() {
                                                            if !profiles.is_empty() {
                                                                let new_idx = profiles
                                                                    .iter()
                                                                    .position(|p| p.name == name)
                                                                    .unwrap_or(
                                                                        profiles
                                                                            .len()
                                                                            .saturating_sub(1),
                                                                    );
                                                                self.ui_state
                                                                    .profile_manager
                                                                    .list_state
                                                                    .select(Some(new_idx));
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to create profile: {}", e);
                                                        // Show error message in UI
                                                        self.message_component = Some(MessageComponent::new(
                                                            "Profile Creation Failed".to_string(),
                                                            format!("Failed to create profile '{}':\n{}", name, e),
                                                            Screen::ManageProfiles,
                                                        ));
                                                    }
                                                }
                                                return Ok(());
                                            }
                                        }
                                        KeyCode::Backspace => {
                                            match state.create_focused_field {
                                                CreateField::Name => {
                                                    if !state.create_name_input.is_empty() {
                                                        crate::utils::text_input::handle_backspace(
                                                            &mut state.create_name_input,
                                                            &mut state.create_name_cursor,
                                                        );
                                                    }
                                                }
                                                CreateField::Description => {
                                                    if !state.create_description_input.is_empty() {
                                                        crate::utils::text_input::handle_backspace(
                                                            &mut state.create_description_input,
                                                            &mut state.create_description_cursor,
                                                        );
                                                    }
                                                }
                                                CreateField::CopyFrom => {
                                                    // No-op for Copy From field
                                                }
                                            }
                                        }
                                        KeyCode::Delete => {
                                            match state.create_focused_field {
                                                CreateField::Name => {
                                                    if !state.create_name_input.is_empty() {
                                                        crate::utils::text_input::handle_delete(
                                                            &mut state.create_name_input,
                                                            &mut state.create_name_cursor,
                                                        );
                                                    }
                                                }
                                                CreateField::Description => {
                                                    if !state.create_description_input.is_empty() {
                                                        crate::utils::text_input::handle_delete(
                                                            &mut state.create_description_input,
                                                            &mut state.create_description_cursor,
                                                        );
                                                    }
                                                }
                                                CreateField::CopyFrom => {
                                                    // No-op for Copy From field
                                                }
                                            }
                                        }
                                        KeyCode::Left => {
                                            match state.create_focused_field {
                                                CreateField::Name => {
                                                    crate::utils::text_input::handle_cursor_movement(&state.create_name_input, &mut state.create_name_cursor, KeyCode::Left);
                                                }
                                                CreateField::Description => {
                                                    crate::utils::text_input::handle_cursor_movement(&state.create_description_input, &mut state.create_description_cursor, KeyCode::Left);
                                                }
                                                CreateField::CopyFrom => {
                                                    // No-op for Copy From field
                                                }
                                            }
                                        }
                                        KeyCode::Right => {
                                            match state.create_focused_field {
                                                CreateField::Name => {
                                                    crate::utils::text_input::handle_cursor_movement(&state.create_name_input, &mut state.create_name_cursor, KeyCode::Right);
                                                }
                                                CreateField::Description => {
                                                    crate::utils::text_input::handle_cursor_movement(&state.create_description_input, &mut state.create_description_cursor, KeyCode::Right);
                                                }
                                                CreateField::CopyFrom => {
                                                    // No-op for Copy From field
                                                }
                                            }
                                        }
                                        KeyCode::Char(c) => {
                                            match state.create_focused_field {
                                                CreateField::Name => {
                                                    crate::utils::text_input::handle_char_insertion(
                                                        &mut state.create_name_input,
                                                        &mut state.create_name_cursor,
                                                        c,
                                                    );
                                                }
                                                CreateField::Description => {
                                                    crate::utils::text_input::handle_char_insertion(
                                                        &mut state.create_description_input,
                                                        &mut state.create_description_cursor,
                                                        c,
                                                    );
                                                }
                                                CreateField::CopyFrom => {
                                                    // No-op for Copy From field (navigation handled by Up/Down)
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ProfilePopupType::Switch => {
                                    match key.code {
                                        KeyCode::Esc => {
                                            state.popup_type = ProfilePopupType::None;
                                        }
                                        KeyCode::Enter => {
                                            // Switch profile
                                            if let Some(idx) = state.list_state.selected() {
                                                if let Some(profile) = profiles.get(idx) {
                                                    let profile_name = profile.name.clone();
                                                    // Release borrows by ending scope
                                                    {
                                                        let _ = state;
                                                        let _ = profiles;
                                                    }
                                                    match self.switch_profile(&profile_name) {
                                                        Ok(_) => {
                                                            // Refresh config
                                                            self.config = Config::load_or_create(
                                                                &self.config_path,
                                                            )?;
                                                            self.ui_state
                                                                .profile_manager
                                                                .popup_type =
                                                                ProfilePopupType::None;
                                                            // Update list selection
                                                            if let Ok(profiles) =
                                                                self.get_profiles()
                                                            {
                                                                if !profiles.is_empty() {
                                                                    let new_idx = profiles
                                                                        .iter()
                                                                        .position(|p| {
                                                                            p.name == profile_name
                                                                        })
                                                                        .unwrap_or(0);
                                                                    self.ui_state
                                                                        .profile_manager
                                                                        .list_state
                                                                        .select(Some(new_idx));
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!(
                                                                "Failed to switch profile: {}",
                                                                e
                                                            );
                                                            // Show error message in UI
                                                            self.ui_state
                                                                .profile_manager
                                                                .popup_type =
                                                                ProfilePopupType::None;
                                                            self.message_component = Some(MessageComponent::new(
                                                                "Error".to_string(),
                                                                format!("Failed to switch profile: {}", e),
                                                                Screen::ManageProfiles,
                                                            ));
                                                        }
                                                    }
                                                    return Ok(());
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                ProfilePopupType::Rename => {
                                    match key.code {
                                        KeyCode::Esc => {
                                            state.popup_type = ProfilePopupType::None;
                                        }
                                        KeyCode::Enter => {
                                            // Rename profile
                                            if !state.rename_input.is_empty() {
                                                if let Some(idx) = state.list_state.selected() {
                                                    if let Some(profile) = profiles.get(idx) {
                                                        let old_name = profile.name.clone();
                                                        let new_name = state.rename_input.clone();
                                                        // Clone values before releasing borrows
                                                        let old_name_clone = old_name.clone();
                                                        let new_name_clone = new_name.clone();
                                                        // Release borrows by ending scope
                                                        {
                                                            let _ = state;
                                                            let _ = profiles;
                                                        }
                                                        match self.rename_profile(
                                                            &old_name_clone,
                                                            &new_name_clone,
                                                        ) {
                                                            Ok(_) => {
                                                                // Refresh config
                                                                self.config =
                                                                    Config::load_or_create(
                                                                        &self.config_path,
                                                                    )?;
                                                                self.ui_state
                                                                    .profile_manager
                                                                    .popup_type =
                                                                    ProfilePopupType::None;
                                                                // Update list selection
                                                                if let Ok(profiles) =
                                                                    self.get_profiles()
                                                                {
                                                                    if !profiles.is_empty() {
                                                                        let new_idx = profiles
                                                                            .iter()
                                                                            .position(|p| {
                                                                                p.name == new_name
                                                                            })
                                                                            .unwrap_or(0);
                                                                        self.ui_state
                                                                            .profile_manager
                                                                            .list_state
                                                                            .select(Some(new_idx));
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                error!(
                                                                    "Failed to rename profile: {}",
                                                                    e
                                                                );
                                                                // Show error message in UI
                                                                self.message_component = Some(MessageComponent::new(
                                                                    "Profile Rename Failed".to_string(),
                                                                    format!("Failed to rename profile '{}' to '{}':\n{}", old_name, new_name, e),
                                                                    Screen::ManageProfiles,
                                                                ));
                                                            }
                                                        }
                                                        return Ok(());
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Backspace => {
                                            if !state.rename_input.is_empty() {
                                                crate::utils::text_input::handle_backspace(
                                                    &mut state.rename_input,
                                                    &mut state.rename_cursor,
                                                );
                                            }
                                        }
                                        KeyCode::Delete => {
                                            if !state.rename_input.is_empty() {
                                                crate::utils::text_input::handle_delete(
                                                    &mut state.rename_input,
                                                    &mut state.rename_cursor,
                                                );
                                            }
                                        }
                                        KeyCode::Left => {
                                            crate::utils::text_input::handle_cursor_movement(
                                                &state.rename_input,
                                                &mut state.rename_cursor,
                                                KeyCode::Left,
                                            );
                                        }
                                        KeyCode::Right => {
                                            crate::utils::text_input::handle_cursor_movement(
                                                &state.rename_input,
                                                &mut state.rename_cursor,
                                                KeyCode::Right,
                                            );
                                        }
                                        KeyCode::Char(c) => {
                                            crate::utils::text_input::handle_char_insertion(
                                                &mut state.rename_input,
                                                &mut state.rename_cursor,
                                                c,
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                                ProfilePopupType::Delete => {
                                    match key.code {
                                        KeyCode::Esc => {
                                            state.popup_type = ProfilePopupType::None;
                                        }
                                        KeyCode::Enter => {
                                            // Delete profile
                                            if let Some(idx) = state.list_state.selected() {
                                                if let Some(profile) = profiles.get(idx) {
                                                    if state.delete_confirm_input == profile.name {
                                                        let profile_name = profile.name.clone();
                                                        let idx_clone = idx;
                                                        // Clone values before releasing borrows
                                                        let profile_name_clone =
                                                            profile_name.clone();
                                                        // Release borrows by ending scope
                                                        {
                                                            let _ = state;
                                                            let _ = profiles;
                                                        }
                                                        match self
                                                            .delete_profile(&profile_name_clone)
                                                        {
                                                            Ok(_) => {
                                                                // Refresh config
                                                                self.config =
                                                                    Config::load_or_create(
                                                                        &self.config_path,
                                                                    )?;
                                                                self.ui_state
                                                                    .profile_manager
                                                                    .popup_type =
                                                                    ProfilePopupType::None;
                                                                // Update list selection
                                                                if let Ok(profiles) =
                                                                    self.get_profiles()
                                                                {
                                                                    if !profiles.is_empty() {
                                                                        let new_idx = idx_clone
                                                                            .min(
                                                                                profiles
                                                                                    .len()
                                                                                    .saturating_sub(
                                                                                        1,
                                                                                    ),
                                                                            );
                                                                        self.ui_state
                                                                            .profile_manager
                                                                            .list_state
                                                                            .select(Some(new_idx));
                                                                    } else {
                                                                        self.ui_state
                                                                            .profile_manager
                                                                            .list_state
                                                                            .select(None);
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                error!(
                                                                    "Failed to delete profile: {}",
                                                                    e
                                                                );
                                                                // Show error message in UI
                                                                self.ui_state
                                                                    .profile_manager
                                                                    .popup_type =
                                                                    ProfilePopupType::None;
                                                                self.message_component = Some(MessageComponent::new(
                                                                    "Error".to_string(),
                                                                    format!("Failed to delete profile: {}", e),
                                                                    Screen::ManageProfiles,
                                                                ));
                                                            }
                                                        }
                                                        return Ok(());
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Backspace => {
                                            if !state.delete_confirm_input.is_empty() {
                                                crate::utils::text_input::handle_backspace(
                                                    &mut state.delete_confirm_input,
                                                    &mut state.delete_confirm_cursor,
                                                );
                                            }
                                        }
                                        KeyCode::Delete => {
                                            if !state.delete_confirm_input.is_empty() {
                                                crate::utils::text_input::handle_delete(
                                                    &mut state.delete_confirm_input,
                                                    &mut state.delete_confirm_cursor,
                                                );
                                            }
                                        }
                                        KeyCode::Left => {
                                            crate::utils::text_input::handle_cursor_movement(
                                                &state.delete_confirm_input,
                                                &mut state.delete_confirm_cursor,
                                                KeyCode::Left,
                                            );
                                        }
                                        KeyCode::Right => {
                                            crate::utils::text_input::handle_cursor_movement(
                                                &state.delete_confirm_input,
                                                &mut state.delete_confirm_cursor,
                                                KeyCode::Right,
                                            );
                                        }
                                        KeyCode::Char(c) => {
                                            crate::utils::text_input::handle_char_insertion(
                                                &mut state.delete_confirm_input,
                                                &mut state.delete_confirm_cursor,
                                                c,
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                                ProfilePopupType::None => {}
                            }
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                // Handle main view events
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if let Some(action) = self.get_action(key.code, key.modifiers) {
                            let state = &mut self.ui_state.profile_manager;
                            use crate::keymap::Action;
                            match action {
                                Action::MoveUp => {
                                    if let Some(current) = state.list_state.selected() {
                                        if current > 0 {
                                            state.list_state.select(Some(current - 1));
                                        }
                                    } else if !profiles.is_empty() {
                                        state.list_state.select(Some(profiles.len() - 1));
                                    }
                                }
                                Action::MoveDown => {
                                    if let Some(current) = state.list_state.selected() {
                                        if current < profiles.len().saturating_sub(1) {
                                            state.list_state.select(Some(current + 1));
                                        }
                                    } else if !profiles.is_empty() {
                                        state.list_state.select(Some(0));
                                    }
                                }
                                Action::Confirm => {
                                    // Open switch popup (only if not already active)
                                    if let Some(idx) = state.list_state.selected() {
                                        if let Some(profile) = profiles.get(idx) {
                                            if profile.name != self.config.active_profile {
                                                state.popup_type = ProfilePopupType::Switch;
                                            }
                                        }
                                    }
                                }
                                Action::Create => {
                                    // Open create popup - refresh config first to get latest profiles
                                    self.config = Config::load_or_create(&self.config_path)?;
                                    use crate::components::profile_manager::CreateField;
                                    state.popup_type = ProfilePopupType::Create;
                                    state.create_name_input.clear();
                                    state.create_name_cursor = 0;
                                    state.create_description_input.clear();
                                    state.create_description_cursor = 0;
                                    state.create_copy_from = None;
                                    state.create_focused_field = CreateField::Name;
                                }
                                Action::Edit => {
                                    // Open rename popup
                                    if let Some(idx) = state.list_state.selected() {
                                        if let Some(profile) = profiles.get(idx) {
                                            state.popup_type = ProfilePopupType::Rename;
                                            state.rename_input = profile.name.clone();
                                            state.rename_cursor = state.rename_input.len();
                                        }
                                    }
                                }
                                Action::Delete => {
                                    // Open delete popup
                                    if let Some(idx) = state.list_state.selected() {
                                        if let Some(profile) = profiles.get(idx) {
                                            if profile.name != self.config.active_profile {
                                                state.popup_type = ProfilePopupType::Delete;
                                                state.delete_confirm_input.clear();
                                                state.delete_confirm_cursor = 0;
                                            }
                                        }
                                    }
                                }
                                Action::Cancel | Action::Quit => {
                                    self.ui_state.current_screen = Screen::MainMenu;
                                }
                                _ => {}
                            }
                        }
                    }

                    Event::Mouse(mouse) => {
                        let state = &mut self.ui_state.profile_manager;
                        match mouse.kind {
                            crossterm::event::MouseEventKind::Down(
                                crossterm::event::MouseButton::Left,
                            ) => {
                                // Handle popup form field clicks
                                if state.popup_type == ProfilePopupType::Create {
                                    use crate::components::profile_manager::CreateField;
                                    // Check if click is on name field
                                    if let Some(name_area) = state.create_name_area {
                                        // Mouse coordinates are absolute screen coordinates
                                        // The area is also absolute (from the centered popup)
                                        if mouse.column >= name_area.x
                                            && mouse.column < name_area.x + name_area.width
                                            && mouse.row >= name_area.y
                                            && mouse.row < name_area.y + name_area.height
                                        {
                                            state.create_focused_field = CreateField::Name;
                                            // Set cursor position based on click
                                            // Account for left border (1 char) - InputField has borders
                                            let inner_x = name_area.x + 1;
                                            let click_x = if mouse.column > inner_x {
                                                (mouse.column as usize)
                                                    .saturating_sub(inner_x as usize)
                                            } else {
                                                0
                                            };
                                            state.create_name_cursor = click_x
                                                .min(state.create_name_input.chars().count());
                                            return Ok(());
                                        }
                                    }
                                    // Check if click is on description field
                                    if let Some(desc_area) = state.create_description_area {
                                        // Mouse coordinates are absolute screen coordinates
                                        // The area is also absolute (from the centered popup)
                                        if mouse.column >= desc_area.x
                                            && mouse.column < desc_area.x + desc_area.width
                                            && mouse.row >= desc_area.y
                                            && mouse.row < desc_area.y + desc_area.height
                                        {
                                            state.create_focused_field = CreateField::Description;
                                            // Set cursor position based on click
                                            // Account for left border (1 char) - InputField has borders
                                            let inner_x = desc_area.x + 1;
                                            let click_x = if mouse.column > inner_x {
                                                (mouse.column as usize)
                                                    .saturating_sub(inner_x as usize)
                                            } else {
                                                0
                                            };
                                            state.create_description_cursor = click_x.min(
                                                state.create_description_input.chars().count(),
                                            );
                                            return Ok(());
                                        }
                                    }
                                    // Check if click is on Copy From list area
                                    // The Copy From list is in chunks[3], but we don't store that area
                                    // For now, clicks on the list will be handled by the list widget itself
                                }

                                // Check if click is in profile list
                                for (rect, profile_idx) in &state.clickable_areas {
                                    if mouse.column >= rect.x
                                        && mouse.column < rect.x + rect.width
                                        && mouse.row >= rect.y
                                        && mouse.row < rect.y + rect.height
                                    {
                                        // Select the clicked profile
                                        state.list_state.select(Some(*profile_idx));
                                        return Ok(());
                                    }
                                }
                            }
                            crossterm::event::MouseEventKind::ScrollUp => {
                                if state.popup_type == ProfilePopupType::None {
                                    if let Some(selected) = state.list_state.selected() {
                                        if selected > 0 {
                                            state.list_state.select(Some(selected - 1));
                                        }
                                    }
                                }
                            }
                            crossterm::event::MouseEventKind::ScrollDown => {
                                if state.popup_type == ProfilePopupType::None {
                                    if let Some(selected) = state.list_state.selected() {
                                        if selected < profiles.len().saturating_sub(1) {
                                            state.list_state.select(Some(selected + 1));
                                        }
                                    } else if !profiles.is_empty() {
                                        state.list_state.select(Some(0));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                return Ok(());
            }
            _ => {
                // Fall through to old event handling for other screens
            }
        }

        // Old event handling for screens not yet converted to components
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {}
            Event::Mouse(mouse) => {
                self.handle_mouse(mouse)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Show the update info popup when user selects the update notification
    fn show_update_info_popup(&mut self) {
        if let Some(ref update_info) = self.main_menu_component.get_update_info().cloned() {
            let message = format!(
                "🎉 New version available: {} → {}\n\n\
                Update options:\n\n\
                1. Using install script:\n\
                   curl -fsSL {} | bash\n\n\
                2. Using Cargo:\n\
                   cargo install dotstate --force\n\n\
                3. Using Homebrew:\n\
                   brew upgrade dotstate\n\n\
                4. Using CLI (interactive):\n\
                   dotstate upgrade\n\n\
                Release notes:\n\
                {}\n\n\
                Press any key to close this popup.",
                update_info.current_version,
                update_info.latest_version,
                crate::version_check::UpdateInfo::install_script_url(),
                update_info.release_url
            );

            self.message_component = Some(MessageComponent::new(
                "Update Available".to_string(),
                message,
                Screen::MainMenu,
            ));
        }
    }

    fn handle_menu_selection(&mut self) -> Result<()> {
        // Check for changes when returning to menu
        if self.ui_state.current_screen == Screen::MainMenu {
            self.check_changes_to_push();
        }

        // Get the selected menu item from the component
        let selected_item = self.main_menu_component.selected_item();
        info!("Menu selection: {:?}", selected_item);

        match selected_item {
            MenuItem::SetupRepository => {
                // Setup git repository
                // Check if repo is already configured (either GitHub or Local mode)
                let is_configured = self.config.is_repo_configured();

                // Initialize auth state with current config values
                if is_configured {
                    self.ui_state.github_auth.repo_already_configured = true;
                    self.ui_state.github_auth.is_editing_token = false;
                    self.ui_state.github_auth.token_input = String::new(); // Clear for security
                                                                           // Load existing values
                    self.ui_state.github_auth.repo_name_input = self.config.repo_name.clone();
                    self.ui_state.github_auth.repo_location_input =
                        self.config.repo_path.to_string_lossy().to_string();
                    self.ui_state.github_auth.local_repo_path_input =
                        self.config.repo_path.to_string_lossy().to_string();
                    self.ui_state.github_auth.is_private = true; // Default to private
                                                                 // Set setup mode based on config
                    self.ui_state.github_auth.setup_mode = match self.config.repo_mode {
                        crate::config::RepoMode::GitHub => crate::ui::SetupMode::GitHub,
                        crate::config::RepoMode::Local => crate::ui::SetupMode::Local,
                    };
                } else {
                    self.ui_state.github_auth.repo_already_configured = false;
                    self.ui_state.github_auth.is_editing_token = false;
                    // Start in "Choosing" mode for new setup
                    self.ui_state.github_auth.setup_mode = crate::ui::SetupMode::Choosing;
                    self.ui_state.github_auth.mode_selection_index = 0;
                }

                self.ui_state.current_screen = Screen::GitHubAuth;
            }
            MenuItem::ScanDotfiles => {
                // Manage Files
                self.scan_dotfiles()?;
                // Reset state when entering the page
                self.ui_state.dotfile_selection.status_message = None;
                // Sync backup_enabled from config
                self.ui_state.dotfile_selection.backup_enabled = self.config.backup_enabled;
                self.ui_state.current_screen = Screen::DotfileSelection;
            }
            // MenuItem::ViewSyncedFiles => {
            //     // View Synced Files
            //     self.ui_state.current_screen = Screen::ViewSyncedFiles;
            // }
            MenuItem::SyncWithRemote => {
                // Sync with Remote - just navigate, don't sync yet
                self.ui_state.current_screen = Screen::SyncWithRemote;
                // Reset sync state
                self.ui_state.sync_with_remote = crate::ui::SyncWithRemoteState::default();
            }
            MenuItem::ManageProfiles => {
                // Manage Profiles
                self.ui_state.current_screen = Screen::ManageProfiles;
                // Initialize list state with first profile selected
                if let Ok(profiles) = self.get_profiles() {
                    if !profiles.is_empty() {
                        self.ui_state.profile_manager.list_state.select(Some(0));
                    }
                }
            }
            MenuItem::ManagePackages => {
                // Manage Packages
                self.ui_state.current_screen = Screen::ManagePackages;
                // Load packages from active profile
                if let Ok(Some(active_profile)) = self.get_active_profile_info() {
                    let packages = active_profile.packages.clone();
                    self.ui_state.package_manager.packages = packages;
                    self.ui_state.package_manager.package_statuses =
                        vec![PackageStatus::Unknown; self.ui_state.package_manager.packages.len()];
                    if !self.ui_state.package_manager.packages.is_empty() {
                        self.ui_state.package_manager.list_state.select(Some(0));
                    }
                }
            }
        }
        Ok(())
    }

    /// Check for changes to push and update UI state
    fn check_changes_to_push(&mut self) {
        self.ui_state.has_changes_to_push = false;
        self.ui_state.sync_with_remote.changed_files.clear();

        // Check if repository is configured and repo exists
        if !self.config.is_repo_configured() {
            return;
        }

        let repo_path = &self.config.repo_path;
        if !repo_path.exists() {
            return;
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(_) => return,
        };

        // Get changed files (this includes both uncommitted and unpushed)
        match git_mgr.get_changed_files() {
            Ok(files) => {
                self.ui_state.sync_with_remote.changed_files = files;
                self.ui_state.has_changes_to_push =
                    !self.ui_state.sync_with_remote.changed_files.is_empty();
            }
            Err(_) => {
                // Fallback to old method if get_changed_files fails
                // Check for uncommitted changes
                let has_uncommitted = git_mgr.has_uncommitted_changes().unwrap_or(false);

                // Check for unpushed commits
                let branch = git_mgr
                    .get_current_branch()
                    .unwrap_or_else(|| "main".to_string());
                let has_unpushed = git_mgr
                    .has_unpushed_commits("origin", &branch)
                    .unwrap_or(false);

                self.ui_state.has_changes_to_push = has_uncommitted || has_unpushed;
            }
        }
    }

    fn handle_github_auth_input(&mut self, key: KeyEvent) -> Result<()> {
        use crate::keymap::Action;
        use crate::ui::SetupMode;

        // Common action lookup
        let action = self.get_action(key.code, key.modifiers);

        let auth_state = &mut self.ui_state.github_auth;
        auth_state.error_message = None;

        // Handle setup mode selection first (before step-based handling)
        match auth_state.setup_mode {
            SetupMode::Choosing => {
                // Handle mode selection screen
                // Use Keymap Actions first
                if let Some(action) = action {
                    match action {
                        Action::MoveUp => {
                            if auth_state.mode_selection_index > 0 {
                                auth_state.mode_selection_index -= 1;
                            }
                            return Ok(());
                        }
                        Action::MoveDown => {
                            if auth_state.mode_selection_index < 1 {
                                auth_state.mode_selection_index += 1;
                            }
                            return Ok(());
                        }
                        Action::Confirm => {
                            // Select mode and transition
                            if auth_state.mode_selection_index == 0 {
                                auth_state.setup_mode = SetupMode::GitHub;
                            } else {
                                auth_state.setup_mode = SetupMode::Local;
                                auth_state.input_focused = true;
                            }
                            return Ok(());
                        }
                        Action::Cancel | Action::Quit => {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                // Fallback for Raw Enter/Esc if not mapped?
                // Standard maps Enter->Confirm, Esc->Cancel.
                // So should be covered.
            }
            SetupMode::Local => {
                // Handle local setup screen
                return self.handle_local_setup_input(key);
            }
            SetupMode::GitHub => {
                // Continue to existing GitHub setup handling below
            }
        }

        match auth_state.step {
            GitHubAuthStep::Input => {
                // Handle "Update Token" action if repo is configured
                if auth_state.repo_already_configured && !auth_state.is_editing_token {
                    // Check for 'u' (Update) or Action::Edit
                    if let Some(Action::Edit) = action {
                        // Enable token editing
                        auth_state.is_editing_token = true;
                        auth_state.token_input = String::new();
                        auth_state.cursor_position = 0;
                        auth_state.focused_field = GitHubAuthField::Token;
                        return Ok(());
                    }
                    // Fallback for raw 'u' if Edit is mapped to something else but 'u' is legacy
                    if key.code == KeyCode::Char('u') || key.code == KeyCode::Char('U') {
                        auth_state.is_editing_token = true;
                        auth_state.token_input = String::new();
                        auth_state.cursor_position = 0;
                        auth_state.focused_field = GitHubAuthField::Token;
                        return Ok(());
                    }

                    if let Some(Action::Cancel | Action::Quit) = action {
                        self.ui_state.current_screen = Screen::MainMenu;
                        *auth_state = Default::default();
                        return Ok(());
                    }

                    if matches!(key.code, KeyCode::Esc) {
                        // Fallback raw escape
                        self.ui_state.current_screen = Screen::MainMenu;
                        *auth_state = Default::default();
                        return Ok(());
                    }
                }

                // Check for Save action (Ctrl+S or equivalent)
                if matches!(action, Some(Action::Save)) {
                    if auth_state.repo_already_configured && auth_state.is_editing_token {
                        // Just update the token
                        self.update_github_token()?;
                    } else if !auth_state.repo_already_configured {
                        // Full setup - initialize state machine
                        let token = auth_state.token_input.trim().to_string();
                        let repo_name = self.config.repo_name.clone();

                        // Validate token format first
                        if !token.starts_with("ghp_") {
                            let actual_start = if token.len() >= 4 {
                                &token[..4]
                            } else {
                                "too short"
                            };
                            auth_state.error_message = Some(
                                format!("❌ Invalid token format: Must start with 'ghp_' but starts with '{}'.\n\
                                See help for more details.", actual_start)
                            );
                            return Ok(());
                        }

                        if token.len() < 40 {
                            auth_state.error_message = Some(format!(
                                "❌ Token appears incomplete: {} characters (expected 40+).",
                                token.len()
                            ));
                            return Ok(());
                        }

                        // Initialize setup state machine
                        auth_state.step =
                            GitHubAuthStep::SetupStep(crate::ui::GitHubSetupStep::Connecting);
                        auth_state.status_message = Some("🔌 Connecting to GitHub...".to_string());
                        auth_state.setup_data = Some(crate::ui::GitHubSetupData {
                            token,
                            repo_name,
                            username: None,
                            repo_exists: None,
                            is_private: auth_state.is_private,
                            delay_until: Some(
                                std::time::Instant::now() + Duration::from_millis(500),
                            ),
                            is_new_repo: false,
                        });
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                    }
                    return Ok(());
                }

                // Normal input handling - check for navigation/editing actions from keymap
                // Input mode allows NextTab, PrevTab, Home, End, Backspace, DeleteChar, Cancel, Confirm
                if let Some(act) = action {
                    match act {
                        Action::Cancel | Action::Quit => {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
                            return Ok(());
                        }
                        Action::NextTab if !auth_state.repo_already_configured => {
                            auth_state.focused_field = match auth_state.focused_field {
                                GitHubAuthField::Token => GitHubAuthField::RepoName,
                                GitHubAuthField::RepoName => GitHubAuthField::RepoLocation,
                                GitHubAuthField::RepoLocation => GitHubAuthField::IsPrivate,
                                GitHubAuthField::IsPrivate => GitHubAuthField::Token,
                            };
                            auth_state.cursor_position = match auth_state.focused_field {
                                GitHubAuthField::Token => auth_state.token_input.chars().count(),
                                GitHubAuthField::RepoName => {
                                    auth_state.repo_name_input.chars().count()
                                }
                                GitHubAuthField::RepoLocation => {
                                    auth_state.repo_location_input.chars().count()
                                }
                                GitHubAuthField::IsPrivate => 0,
                            };
                            return Ok(());
                        }
                        Action::PrevTab if !auth_state.repo_already_configured => {
                            auth_state.focused_field = match auth_state.focused_field {
                                GitHubAuthField::Token => GitHubAuthField::IsPrivate,
                                GitHubAuthField::RepoName => GitHubAuthField::Token,
                                GitHubAuthField::RepoLocation => GitHubAuthField::RepoName,
                                GitHubAuthField::IsPrivate => GitHubAuthField::RepoLocation,
                            };
                            auth_state.cursor_position = match auth_state.focused_field {
                                GitHubAuthField::Token => auth_state.token_input.chars().count(),
                                GitHubAuthField::RepoName => {
                                    auth_state.repo_name_input.chars().count()
                                }
                                GitHubAuthField::RepoLocation => {
                                    auth_state.repo_location_input.chars().count()
                                }
                                GitHubAuthField::IsPrivate => 0,
                            };
                            return Ok(());
                        }
                        Action::MoveLeft => {
                            let current_input = match auth_state.focused_field {
                                GitHubAuthField::Token => &auth_state.token_input,
                                GitHubAuthField::RepoName => &auth_state.repo_name_input,
                                GitHubAuthField::RepoLocation => &auth_state.repo_location_input,
                                GitHubAuthField::IsPrivate => "",
                            };
                            crate::utils::handle_cursor_movement(
                                current_input,
                                &mut auth_state.cursor_position,
                                KeyCode::Left,
                            );
                            return Ok(());
                        }
                        Action::MoveRight => {
                            let current_input = match auth_state.focused_field {
                                GitHubAuthField::Token => &auth_state.token_input,
                                GitHubAuthField::RepoName => &auth_state.repo_name_input,
                                GitHubAuthField::RepoLocation => &auth_state.repo_location_input,
                                GitHubAuthField::IsPrivate => "",
                            };
                            crate::utils::handle_cursor_movement(
                                current_input,
                                &mut auth_state.cursor_position,
                                KeyCode::Right,
                            );
                            return Ok(());
                        }
                        Action::Home => {
                            let text = match auth_state.focused_field {
                                GitHubAuthField::Token => &auth_state.token_input,
                                GitHubAuthField::RepoName => &auth_state.repo_name_input,
                                GitHubAuthField::RepoLocation => &auth_state.repo_location_input,
                                GitHubAuthField::IsPrivate => "",
                            };
                            crate::utils::handle_cursor_movement(
                                text,
                                &mut auth_state.cursor_position,
                                KeyCode::Home,
                            );
                            return Ok(());
                        }
                        Action::End => {
                            let text = match auth_state.focused_field {
                                GitHubAuthField::Token => &auth_state.token_input,
                                GitHubAuthField::RepoName => &auth_state.repo_name_input,
                                GitHubAuthField::RepoLocation => &auth_state.repo_location_input,
                                GitHubAuthField::IsPrivate => "",
                            };
                            crate::utils::handle_cursor_movement(
                                text,
                                &mut auth_state.cursor_position,
                                KeyCode::End,
                            );
                            return Ok(());
                        }
                        Action::Backspace => {
                            match auth_state.focused_field {
                                GitHubAuthField::Token => crate::utils::handle_backspace(
                                    &mut auth_state.token_input,
                                    &mut auth_state.cursor_position,
                                ),
                                GitHubAuthField::RepoName => crate::utils::handle_backspace(
                                    &mut auth_state.repo_name_input,
                                    &mut auth_state.cursor_position,
                                ),
                                GitHubAuthField::RepoLocation => crate::utils::handle_backspace(
                                    &mut auth_state.repo_location_input,
                                    &mut auth_state.cursor_position,
                                ),
                                GitHubAuthField::IsPrivate => {}
                            }
                            return Ok(());
                        }
                        Action::DeleteChar => {
                            match auth_state.focused_field {
                                GitHubAuthField::Token => crate::utils::handle_delete(
                                    &mut auth_state.token_input,
                                    &mut auth_state.cursor_position,
                                ),
                                GitHubAuthField::RepoName => crate::utils::handle_delete(
                                    &mut auth_state.repo_name_input,
                                    &mut auth_state.cursor_position,
                                ),
                                GitHubAuthField::RepoLocation => crate::utils::handle_delete(
                                    &mut auth_state.repo_location_input,
                                    &mut auth_state.cursor_position,
                                ),
                                GitHubAuthField::IsPrivate => {}
                            }
                            return Ok(());
                        }
                        Action::ToggleSelect => {
                            // Space toggle for IsPrivate
                            if auth_state.focused_field == GitHubAuthField::IsPrivate
                                && !auth_state.repo_already_configured
                            {
                                auth_state.is_private = !auth_state.is_private;
                            }
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                // Handle character input (only for text fields, not navigation)
                if let KeyCode::Char(c) = key.code {
                    // Regular char insertion for text fields
                    match auth_state.focused_field {
                        GitHubAuthField::Token
                            if (!auth_state.repo_already_configured
                                || auth_state.is_editing_token)
                                && !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) =>
                        {
                            crate::utils::handle_char_insertion(
                                &mut auth_state.token_input,
                                &mut auth_state.cursor_position,
                                c,
                            );
                        }
                        GitHubAuthField::RepoName
                            if !auth_state.repo_already_configured
                                && !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) =>
                        {
                            crate::utils::handle_char_insertion(
                                &mut auth_state.repo_name_input,
                                &mut auth_state.cursor_position,
                                c,
                            );
                        }
                        GitHubAuthField::RepoLocation
                            if !auth_state.repo_already_configured
                                && !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) =>
                        {
                            crate::utils::handle_char_insertion(
                                &mut auth_state.repo_location_input,
                                &mut auth_state.cursor_position,
                                c,
                            );
                        }
                        _ => {}
                    }
                }
            }
            GitHubAuthStep::Processing => {
                // Should allow Cancel/Esc. Enter continues if done.
                match action {
                    Some(Action::Confirm) => {
                        if !self.ui_state.profile_selection.profiles.is_empty() {
                            self.ui_state.current_screen = Screen::ProfileSelection;
                        } else {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
                        }
                    }
                    Some(Action::Cancel | Action::Quit) => {
                        self.ui_state.current_screen = Screen::MainMenu;
                        *auth_state = Default::default();
                    }
                    _ => {
                        // Fallback Raw
                        if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                            if !self.ui_state.profile_selection.profiles.is_empty() {
                                self.ui_state.current_screen = Screen::ProfileSelection;
                            } else {
                                self.ui_state.current_screen = Screen::MainMenu;
                                *auth_state = Default::default();
                            }
                        } else if matches!(key.code, KeyCode::Esc) {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
                        }
                    }
                }
            }
            GitHubAuthStep::SetupStep(_) => {
                if let Some(Action::Cancel | Action::Quit) = action {
                    *auth_state = Default::default();
                    self.ui_state.current_screen = Screen::MainMenu;
                } else if key.code == KeyCode::Esc {
                    *auth_state = Default::default();
                    self.ui_state.current_screen = Screen::MainMenu;
                }
            }
        }
        Ok(())
    }

    /// Handle keyboard input for local setup screen
    fn handle_local_setup_input(&mut self, key: KeyEvent) -> Result<()> {
        use crate::keymap::Action;
        use crate::ui::SetupMode;

        let action = self.get_action(key.code, key.modifiers);
        let auth_state = &mut self.ui_state.github_auth;
        auth_state.error_message = None;

        // If already configured, only allow Esc/Cancel to go back
        if auth_state.repo_already_configured {
            // Check Action::Cancel or Action::Quit
            if let Some(Action::Cancel | Action::Quit) = action {
                self.ui_state.current_screen = Screen::MainMenu;
                *auth_state = Default::default();
                return Ok(());
            }
            // Fallback for raw Esc if not mapped
            if key.code == KeyCode::Esc {
                self.ui_state.current_screen = Screen::MainMenu;
                *auth_state = Default::default();
            }
            return Ok(());
        }

        // Check for Action::Confirm or Action::Save to validate and save
        if matches!(action, Some(Action::Confirm | Action::Save)) {
            // Validate local repo
            let path_str = auth_state.local_repo_path_input.trim();
            if path_str.is_empty() {
                auth_state.error_message = Some("Please enter a repository path".to_string());
                return Ok(());
            }

            let expanded_path = crate::git::expand_path(path_str);
            let validation = crate::git::validate_local_repo(&expanded_path);

            if !validation.is_valid {
                auth_state.error_message = validation.error_message;
                return Ok(());
            }

            // Validation passed - save config
            auth_state.status_message = Some(format!(
                "✅ Valid repository found!\n\nRemote: {}\n\nSaving configuration...",
                validation.remote_url.as_deref().unwrap_or("unknown")
            ));

            // Update config for local mode
            self.config.repo_mode = crate::config::RepoMode::Local;
            self.config.repo_path = expanded_path.clone();
            self.config.github = None; // Clear GitHub config for local mode

            // Save config
            if let Err(e) = self.config.save(&crate::utils::get_config_path()) {
                auth_state.error_message = Some(format!("Failed to save config: {}", e));
                auth_state.status_message = None;
                return Ok(());
            }

            // Verify git repository can be opened
            if let Err(e) = crate::git::GitManager::open_or_init(&expanded_path) {
                auth_state.error_message = Some(format!("Failed to open repository: {}", e));
                auth_state.status_message = None;
                return Ok(());
            }

            // Load or create profile manifest
            let manifest =
                crate::utils::ProfileManifest::load_or_backfill(&expanded_path).unwrap_or_default();

            // Set up profile selection
            let profiles: Vec<String> = manifest.profiles.iter().map(|p| p.name.clone()).collect();

            if profiles.is_empty() {
                // No profiles found, create default profile
                let default_profile = "default";
                self.config.active_profile = default_profile.to_string();
                self.config.save(&crate::utils::get_config_path())?;

                // Go directly to main menu
                self.ui_state.current_screen = Screen::MainMenu;
                *auth_state = Default::default();

                // Update main menu config
                self.main_menu_component.update_config(self.config.clone());
            } else {
                // Show profile selection
                self.ui_state.profile_selection.profiles = profiles;
                self.ui_state.profile_selection.list_state.select(Some(0));
                self.ui_state.current_screen = Screen::ProfileSelection;
                *auth_state = Default::default();
            }

            return Ok(());
        }

        if let Some(Action::Cancel | Action::Quit) = action {
            auth_state.setup_mode = SetupMode::Choosing;
            auth_state.error_message = None;
            auth_state.status_message = None;
            return Ok(());
        }

        match key.code {
            KeyCode::Esc => {
                // Go back to mode selection
                auth_state.setup_mode = SetupMode::Choosing;
                auth_state.error_message = None;
                auth_state.status_message = None;
            }
            KeyCode::Char(c) => {
                crate::utils::handle_char_insertion(
                    &mut auth_state.local_repo_path_input,
                    &mut auth_state.local_repo_path_cursor,
                    c,
                );
            }
            KeyCode::Backspace => {
                crate::utils::handle_backspace(
                    &mut auth_state.local_repo_path_input,
                    &mut auth_state.local_repo_path_cursor,
                );
            }
            KeyCode::Delete => {
                crate::utils::handle_delete(
                    &mut auth_state.local_repo_path_input,
                    &mut auth_state.local_repo_path_cursor,
                );
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                crate::utils::handle_cursor_movement(
                    &auth_state.local_repo_path_input,
                    &mut auth_state.local_repo_path_cursor,
                    key.code,
                );
            }
            _ => {}
        }

        Ok(())
    }

    fn update_github_token(&mut self) -> Result<()> {
        let auth_state = &mut self.ui_state.github_auth;
        let token = auth_state.token_input.trim().to_string();

        // Validate token format
        if token.is_empty() {
            auth_state.error_message = Some("Token cannot be empty".to_string());
            return Ok(());
        }

        if !token.starts_with("ghp_") {
            auth_state.error_message =
                Some("Token format error: GitHub tokens must start with 'ghp_'".to_string());
            return Ok(());
        }

        if token.len() < 40 {
            auth_state.error_message = Some(format!(
                "Token appears incomplete: {} characters (expected 40+)",
                token.len()
            ));
            return Ok(());
        }

        // Validate token with GitHub API
        auth_state.status_message = Some("Validating token...".to_string());

        let rt = Runtime::new()?;
        let result = rt.block_on(async {
            let client = reqwest::Client::new();
            client
                .get("https://api.github.com/user")
                .header("Authorization", format!("Bearer {}", token))
                .header("User-Agent", "dotstate")
                .send()
                .await
        });

        match result {
            Ok(response) if response.status().is_success() => {
                // Token is valid, update config
                if let Some(github) = &mut self.config.github {
                    github.token = Some(token.clone());
                    self.config.save(&crate::utils::get_config_path())?;

                    auth_state.status_message = Some("Token updated successfully!".to_string());
                    auth_state.is_editing_token = false;
                    auth_state.token_input = String::new(); // Clear for security

                    // Sync back to component
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                } else {
                    auth_state.error_message = Some(
                        "GitHub configuration not found. Please complete setup first.".to_string(),
                    );
                    auth_state.status_message = None;
                }
            }
            Ok(response) => {
                let status = response.status();
                auth_state.error_message = Some(format!(
                    "Token validation failed: HTTP {}\nPlease check your token.",
                    status
                ));
                auth_state.status_message = None;
            }
            Err(e) => {
                auth_state.error_message = Some(format!(
                    "Network error: {}\nPlease check your internet connection.",
                    e
                ));
                auth_state.status_message = None;
            }
        }

        Ok(())
    }

    /// Process one step of the GitHub setup state machine
    /// Called from the event loop to allow UI updates between steps
    fn process_github_setup_step(&mut self) -> Result<()> {
        let auth_state = &mut self.ui_state.github_auth;

        // Get setup_data, cloning if needed to avoid borrow issues
        let setup_data_opt = auth_state.setup_data.clone();
        let mut setup_data = match setup_data_opt {
            Some(data) => data,
            None => {
                // No setup data, reset to input
                auth_state.step = GitHubAuthStep::Input;
                return Ok(());
            }
        };

        // Check if we need to wait for a delay
        if let Some(delay_until) = setup_data.delay_until {
            if std::time::Instant::now() < delay_until {
                // Still waiting, don't process yet - save state and return
                auth_state.setup_data = Some(setup_data);
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
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                    }
                    Err(e) => {
                        auth_state.error_message = Some(format!("❌ Authentication failed: {}", e));
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

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
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                    }
                    Err(e) => {
                        auth_state.error_message =
                            Some(format!("❌ Failed to clone repository: {}", e));
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                    }
                    Err(e) => {
                        auth_state.error_message =
                            Some(format!("❌ Failed to create repository: {}", e));
                        auth_state.status_message = None;
                        auth_state.step = GitHubAuthStep::Input;
                        auth_state.setup_data = None;
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

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
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                self.ui_state.profile_selection.profiles =
                    manifest.profiles.iter().map(|p| p.name.clone()).collect();
                if !self.ui_state.profile_selection.profiles.is_empty() {
                    self.ui_state.profile_selection.list_state.select(Some(0));
                }

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
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
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
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                // Drop mutable borrow of auth_state before calling scan_dotfiles
                // Note: drop() on a reference doesn't do anything, but we're explicitly ending the borrow scope
                let _ = auth_state;

                if should_scan_dotfiles {
                    // New install with default profile - go directly to dotfile selection
                    // Sync backup_enabled from config
                    self.ui_state.dotfile_selection.backup_enabled = self.config.backup_enabled;
                    // Trigger search for dotfiles
                    self.scan_dotfiles()?;
                    // Reset state when entering the page
                    self.ui_state.dotfile_selection.status_message = None;
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

    #[allow(dead_code)] // Kept for reference, but replaced by process_github_setup_step
    fn process_github_setup(&mut self) -> Result<()> {
        let auth_state = &mut self.ui_state.github_auth;

        // Set processing state FIRST before any blocking operations
        auth_state.step = GitHubAuthStep::Processing;
        auth_state.error_message = None;

        // Step 1: Connecting to GitHub - set this immediately
        auth_state.status_message = Some("🔌 Connecting to GitHub...".to_string());

        // Sync state to component so UI can render it
        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

        // Small delay to allow UI to render the progress screen
        // This gives the event loop a chance to process and render
        // Note: This won't work perfectly because we're blocking, but it helps
        std::thread::sleep(Duration::from_millis(300));

        // Trim whitespace from token
        let token = auth_state.token_input.trim().to_string();
        let repo_name = self.config.repo_name.clone();

        // Token validation - do not log token content for security

        // Validate token format before making API call
        if !token.starts_with("ghp_") {
            let actual_start = if token.len() >= 4 {
                &token[..4]
            } else {
                "too short"
            };
            auth_state.error_message = Some(format!(
                "❌ Invalid token format: Must start with 'ghp_' but starts with '{}'.\n\
                    Token length: {} characters.\n\
                    First 10 chars: '{}'\n\
                    Please check that you copied the entire token correctly.\n\
                    Make sure you're pasting the full token (40+ characters).",
                actual_start,
                token.len(),
                if token.len() >= 10 {
                    &token[..10]
                } else {
                    &token
                }
            ));
            auth_state.step = GitHubAuthStep::Input;
            auth_state.status_message = None;
            return Ok(());
        }

        if token.len() < 40 {
            auth_state.error_message = Some(format!(
                "❌ Token appears incomplete: {} characters (expected 40+).\n\
                    First 10 chars: '{}'\n\
                    Make sure you copied the entire token from GitHub.",
                token.len(),
                &token[..token.len().min(10)]
            ));
            auth_state.step = GitHubAuthStep::Input;
            auth_state.status_message = None;
            return Ok(());
        }

        // Step 2: Validating token
        auth_state.status_message = Some("🔑 Validating your token...".to_string());
        *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

        // Small delay for UX
        std::thread::sleep(Duration::from_millis(800));

        // Use the runtime to run async code
        let result = self.runtime.block_on(async {
            // Verify token and get user
            let client = GitHubClient::new(token.clone());
            let user = client.get_user().await?;

            // Step 3: Check if repo exists
            let repo_exists = client.repo_exists(&user.login, &repo_name).await?;

            Ok::<(String, bool), anyhow::Error>((user.login, repo_exists))
        });

        match result {
            Ok((username, exists)) => {
                let repo_path = self.config.repo_path.clone();

                // Step 3: Checking if repo exists (already done, but show status)
                auth_state.status_message = Some("🔍 Checking if repository exists...".to_string());
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                // Small delay for UX
                std::thread::sleep(Duration::from_millis(600));

                if exists {
                    // Step 4: Cloning the repo (or reusing existing)
                    auth_state.status_message = Some(format!(
                        "📥 Cloning repository {}/{}...",
                        username, repo_name
                    ));
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                    // Small delay before cloning
                    std::thread::sleep(Duration::from_millis(500));

                    let remote_url = format!("https://github.com/{}/{}.git", username, repo_name);

                    match GitManager::clone_or_open(&remote_url, &repo_path, Some(&token)) {
                        Ok((_, was_existing)) => {
                            auth_state.status_message = Some(if was_existing {
                                "✅ Using existing repository!".to_string()
                            } else {
                                "✅ Repository cloned successfully!".to_string()
                            });
                            *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                            std::thread::sleep(Duration::from_millis(500));
                        }
                        Err(e) => {
                            auth_state.error_message =
                                Some(format!("❌ Failed to clone repository: {}", e));
                            auth_state.status_message = None;
                            auth_state.step = GitHubAuthStep::Input;
                            auth_state.setup_data = None;
                            *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                            return Ok(());
                        }
                    }
                } else {
                    // Step 4: Creating new repository
                    auth_state.status_message = Some(format!(
                        "📦 Creating repository {}/{}...",
                        username, repo_name
                    ));
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                    // Small delay for UX
                    std::thread::sleep(Duration::from_millis(600));

                    // Create repository
                    let is_private = auth_state.is_private;
                    let create_result = self.runtime.block_on(async {
                        let client = GitHubClient::new(token.clone());
                        client
                            .create_repo(&repo_name, "My dotfiles managed by dotstate", is_private)
                            .await
                    });

                    match create_result {
                        Ok(_) => {
                            auth_state.status_message =
                                Some("⚙️  Initializing local repository...".to_string());
                            *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                            // Small delay for UX
                            std::thread::sleep(Duration::from_millis(500));

                            // Initialize local repository
                            std::fs::create_dir_all(&repo_path)
                                .context("Failed to create repository directory")?;

                            let mut git_mgr = GitManager::open_or_init(&repo_path)?;

                            // Add remote
                            let remote_url = format!(
                                "https://{}@github.com/{}/{}.git",
                                token, username, repo_name
                            );
                            // Add remote (this also sets up tracking)
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
                                profiles: vec![crate::utils::profile_manifest::ProfileInfo {
                                    name: default_profile_name.clone(),
                                    description: None, // Default profile, no description yet
                                    synced_files: Vec::new(),
                                    packages: Vec::new(),
                                }],
                            };
                            manifest.save(&repo_path)?;

                            git_mgr.commit_all("Initial commit")?;

                            // Get current branch name (should be 'main' after ensure_main_branch)
                            let current_branch = git_mgr
                                .get_current_branch()
                                .unwrap_or_else(|| self.config.default_branch.clone());

                            // Push to remote using the actual branch name and set upstream
                            git_mgr.push("origin", &current_branch, Some(&token))?;

                            // Ensure tracking is set up after push
                            git_mgr.set_upstream_tracking("origin", &current_branch)?;

                            // Update config with default profile name
                            self.config.active_profile = default_profile_name.clone();
                            self.config
                                .save(&self.config_path)
                                .context("Failed to save configuration")?;

                            auth_state.status_message = Some(
                                "✅ Repository created and initialized successfully".to_string(),
                            );
                            *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                        }
                        Err(e) => {
                            auth_state.error_message =
                                Some(format!("❌ Failed to create repository: {}", e));
                            auth_state.status_message = None;
                            auth_state.step = GitHubAuthStep::Input;
                            return Ok(());
                        }
                    }
                }

                // Update config
                self.config.github = Some(GitHubConfig {
                    owner: username.clone(),
                    repo: repo_name.clone(),
                    token: Some(token.clone()),
                });
                self.config.repo_name = repo_name.clone();
                self.config
                    .save(&self.config_path)
                    .context("Failed to save configuration")?;

                // Verify config was saved
                if !self.config_path.exists() {
                    auth_state.error_message = Some(
                        "Warning: Config file was not created. Please check permissions."
                            .to_string(),
                    );
                    auth_state.step = GitHubAuthStep::Input;
                    return Ok(());
                }

                // Discover profiles from the cloned repo
                if exists && repo_path.exists() {
                    auth_state.status_message = Some("🔎 Discovering profiles...".to_string());
                    *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                    // Small delay for UX
                    std::thread::sleep(Duration::from_millis(600));

                    // Discover profiles from manifest
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

                    // Set active profile to first one if available and not already set
                    if !manifest.profiles.is_empty() && self.config.active_profile.is_empty() {
                        self.config.active_profile = manifest.profiles[0].name.clone();
                    }

                    // Save updated config
                    self.config.save(&self.config_path)?;
                } else {
                    // For new repos, just reload config normally
                    self.config = Config::load_or_create(&self.config_path)?;
                }

                // Check if we have profiles to activate (only if repo was cloned, not created)
                if exists {
                    // Set up profile selection state from manifest
                    // Get manifest before borrowing ui_state (repo_path already cloned above)
                    let repo_path_clone = self.config.repo_path.clone();
                    let manifest =
                        crate::utils::ProfileManifest::load_or_backfill(&repo_path_clone)
                            .unwrap_or_default();
                    let profile_names: Vec<String> =
                        manifest.profiles.iter().map(|p| p.name.clone()).collect();
                    self.ui_state.profile_selection.profiles = profile_names;
                    if !self.ui_state.profile_selection.profiles.is_empty() {
                        self.ui_state.profile_selection.list_state.select(Some(0));
                    }

                    if !self.ui_state.profile_selection.profiles.is_empty() {
                        auth_state.status_message = Some(format!(
                            "✅ Setup complete!\n\nFound {} profile(s) in the repository.\n\nPress Enter to select which profile to activate.",
                            self.ui_state.profile_selection.profiles.len()
                        ));
                    } else {
                        // No profiles found
                        auth_state.status_message = Some(format!(
                            "✅ Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nNo profiles found. You can create one from the main menu.\n\nPress Enter to continue.",
                            username, repo_name, repo_path
                        ));
                    }
                } else {
                    // New repo - just created, no profiles to activate yet
                    // Reload config to ensure it's up to date
                    self.config = Config::load_or_create(&self.config_path)?;
                    auth_state.status_message = Some(format!(
                        "✅ Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nPress Enter to continue.",
                        username, repo_name, repo_path
                    ));
                }

                // Update component state
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();

                // Ensure step is set to Processing so user can press Enter to continue
                auth_state.step = GitHubAuthStep::Processing;
            }
            Err(e) => {
                // Show detailed error message
                let error_msg = format!("❌ Authentication failed: {}", e);
                auth_state.error_message = Some(error_msg);
                auth_state.status_message = None;
                auth_state.step = GitHubAuthStep::Input;
                *self.github_auth_component.get_auth_state_mut() = auth_state.clone();
                // Don't clear the token input so user can see what they entered
            }
        }

        Ok(())
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> Result<()> {
        use crossterm::event::{MouseButton, MouseEventKind};

        // Handle DotfileSelection screen mouse events
        if self.ui_state.current_screen == Screen::DotfileSelection {
            let state = &mut self.ui_state.dotfile_selection;
            use crate::ui::DotfileSelectionFocus;

            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    if state.file_browser_mode {
                        // File browser mode - scroll based on focus
                        match state.focus {
                            DotfileSelectionFocus::FileBrowserList => {
                                state.file_browser_list_state.select_previous();
                            }
                            DotfileSelectionFocus::FileBrowserPreview => {
                                if state.file_browser_preview_scroll > 0 {
                                    state.file_browser_preview_scroll =
                                        state.file_browser_preview_scroll.saturating_sub(1);
                                }
                            }
                            _ => {}
                        }
                    } else if state.adding_custom_file {
                        // Custom input mode - no scrolling
                    } else {
                        // Normal mode - scroll based on focus
                        match state.focus {
                            DotfileSelectionFocus::FilesList => {
                                state.dotfile_list_state.select_previous();
                                state.preview_scroll = 0;
                            }
                            DotfileSelectionFocus::Preview => {
                                if state.preview_scroll > 0 {
                                    state.preview_scroll = state.preview_scroll.saturating_sub(1);
                                }
                            }
                            _ => {}
                        }
                    }
                    return Ok(());
                }
                MouseEventKind::ScrollDown => {
                    if state.file_browser_mode {
                        // File browser mode - scroll based on focus
                        match state.focus {
                            DotfileSelectionFocus::FileBrowserList => {
                                state.file_browser_list_state.select_next();
                            }
                            DotfileSelectionFocus::FileBrowserPreview => {
                                state.file_browser_preview_scroll =
                                    state.file_browser_preview_scroll.saturating_add(1);
                            }
                            _ => {}
                        }
                    } else if state.adding_custom_file {
                        // Custom input mode - no scrolling
                    } else {
                        // Normal mode - scroll based on focus
                        match state.focus {
                            DotfileSelectionFocus::FilesList => {
                                state.dotfile_list_state.select_next();
                                state.preview_scroll = 0;
                            }
                            DotfileSelectionFocus::Preview => {
                                state.preview_scroll = state.preview_scroll.saturating_add(1);
                            }
                            _ => {}
                        }
                    }
                    return Ok(());
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    let terminal_size = self.tui.terminal_mut().size()?;
                    let header_height = 6; // Header is 6 lines
                    let footer_height = 1; // Footer is 1 line

                    if state.file_browser_mode {
                        // File browser popup - improved click detection
                        // Check if click is within popup area (centered, 80% width, 70% height)
                        let popup_width = (terminal_size.width as f32 * 0.8) as u16;
                        let popup_height = (terminal_size.height as f32 * 0.7) as u16;
                        let popup_x = (terminal_size.width - popup_width) / 2;
                        let popup_y = (terminal_size.height - popup_height) / 2;

                        if mouse.column >= popup_x
                            && mouse.column < popup_x + popup_width
                            && mouse.row >= popup_y
                            && mouse.row < popup_y + popup_height
                        {
                            let popup_inner_y = mouse.row.saturating_sub(popup_y);
                            let popup_inner_x = mouse.column.saturating_sub(popup_x);

                            // Layout: path display (1), path input (3), list+preview (min), footer (2)
                            if popup_inner_y < 1 {
                                // Clicked on path display - focus input
                                state.focus = DotfileSelectionFocus::FileBrowserInput;
                                state.file_browser_path_focused = true;
                            } else if (1..4).contains(&popup_inner_y) {
                                // Clicked on path input field
                                state.focus = DotfileSelectionFocus::FileBrowserInput;
                                state.file_browser_path_focused = true;
                            } else if popup_inner_y >= 4 && popup_inner_y < popup_height - 2 {
                                // Clicked in list/preview area
                                let list_preview_y = popup_inner_y - 4; // After path display and input

                                if popup_inner_x < popup_width / 2 {
                                    // Clicked on file browser list
                                    state.focus = DotfileSelectionFocus::FileBrowserList;
                                    // Calculate which item was clicked (accounting for list borders)
                                    // List has borders, so first clickable row is at y=1
                                    if list_preview_y >= 1 {
                                        let clicked_index = (list_preview_y - 1) as usize;
                                        if clicked_index < state.file_browser_entries.len() {
                                            state
                                                .file_browser_list_state
                                                .select(Some(clicked_index));
                                        }
                                    }
                                } else {
                                    // Clicked on preview pane
                                    state.focus = DotfileSelectionFocus::FileBrowserPreview;
                                }
                            }
                        }
                    } else if !state.adding_custom_file && mouse.row >= header_height as u16 {
                        // Normal mode - detect which pane was clicked
                        let content_start_y = header_height as u16;
                        let content_end_y = terminal_size.height.saturating_sub(footer_height);

                        if mouse.row >= content_start_y && mouse.row < content_end_y {
                            // Determine if click is in left half (files list) or right half (preview)
                            if mouse.column < terminal_size.width / 2 {
                                // Clicked on files list
                                state.focus = DotfileSelectionFocus::FilesList;

                                // Calculate which item was clicked
                                let clicked_row =
                                    mouse.row.saturating_sub(content_start_y) as usize;
                                if clicked_row < state.dotfiles.len() {
                                    state.dotfile_list_state.select(Some(clicked_row));
                                    state.preview_scroll = 0;
                                }
                            } else {
                                // Clicked on preview pane
                                state.focus = DotfileSelectionFocus::Preview;
                            }
                        }
                    }
                    return Ok(());
                }
                _ => {}
            }
        }

        // Handle GitHubAuth screen mouse events
        if let MouseEventKind::Down(button) = mouse.kind {
            if button == MouseButton::Left {
                let auth_state = &mut self.ui_state.github_auth;

                // Get terminal size to determine click areas
                let terminal_size = self.tui.terminal_mut().size()?;
                let mouse_x = mouse.column;

                // Check if click is in GitHub auth screen
                if self.ui_state.current_screen == Screen::GitHubAuth
                    && auth_state.step == GitHubAuthStep::Input
                {
                    // Check if click is in token input area (roughly row 4-6, column 2-78)
                    // This is approximate - we'd need to track exact widget positions for precision
                    // For now, clicking anywhere in the left half focuses token input
                    if mouse_x < terminal_size.width / 2 {
                        auth_state.input_focused = true;
                        // Move cursor to clicked position (approximate)
                        let relative_x = mouse_x.saturating_sub(2) as usize;
                        auth_state.cursor_position =
                            relative_x.min(auth_state.token_input.chars().count());
                    } else {
                        // Click in help area - unfocus input
                        auth_state.input_focused = false;
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle input for adding custom files
    fn handle_custom_file_input(&mut self, key_code: KeyCode) -> Result<()> {
        use crate::ui::DotfileSelectionFocus;
        let state = &mut self.ui_state.dotfile_selection;

        // When input is not focused, only allow Enter to focus or Esc to cancel
        if !state.custom_file_focused {
            match key_code {
                KeyCode::Enter => {
                    state.custom_file_focused = true;
                    return Ok(());
                }
                KeyCode::Esc => {
                    state.adding_custom_file = false;
                    state.custom_file_input.clear();
                    state.custom_file_cursor = 0;
                    return Ok(());
                }
                _ => {
                    // Ignore all other keys when not focused (including characters)
                    return Ok(());
                }
            }
        }

        // When focused, handle all input - characters are captured FIRST before any other logic
        match key_code {
            // Character input - capture ALL characters including 's', 'a', 'q', etc.
            // Text input handling - use text input utility
            KeyCode::Char(c) => {
                crate::utils::handle_char_insertion(
                    &mut state.custom_file_input,
                    &mut state.custom_file_cursor,
                    c,
                );
                return Ok(());
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                crate::utils::handle_cursor_movement(
                    &state.custom_file_input,
                    &mut state.custom_file_cursor,
                    key_code,
                );
            }
            KeyCode::Backspace => {
                crate::utils::handle_backspace(
                    &mut state.custom_file_input,
                    &mut state.custom_file_cursor,
                );
            }
            KeyCode::Delete => {
                crate::utils::handle_delete(
                    &mut state.custom_file_input,
                    &mut state.custom_file_cursor,
                );
            }
            KeyCode::Tab => {
                state.custom_file_focused = false;
            }
            KeyCode::Enter => {
                let path_str = state.custom_file_input.trim();
                if path_str.is_empty() {
                    state.status_message = Some("Error: File path cannot be empty".to_string());
                } else {
                    // Validate and add the file - use path utility
                    let path_str_clone = path_str.to_string();
                    let full_path = crate::utils::expand_path(path_str);

                    if !full_path.exists() {
                        state.status_message =
                            Some(format!("Error: File does not exist: {:?}", full_path));
                    } else {
                        // Calculate relative path
                        let home_dir = crate::utils::get_home_dir();
                        let relative_path = match full_path.strip_prefix(&home_dir) {
                            Ok(p) => p.to_string_lossy().to_string(),
                            Err(_) => path_str_clone.clone(),
                        };

                        // Store paths before releasing borrow
                        let relative_path_clone = relative_path.clone();
                        let full_path_clone = full_path.clone();

                        // Close custom input mode
                        state.adding_custom_file = false;
                        state.custom_file_input.clear();
                        state.custom_file_cursor = 0;
                        state.focus = DotfileSelectionFocus::FilesList;

                        // Comprehensive validation before showing confirmation
                        let previously_synced: std::collections::HashSet<String> = self
                            .get_active_profile_info()
                            .ok()
                            .flatten()
                            .map(|p| p.synced_files.iter().cloned().collect())
                            .unwrap_or_default();

                        let validation = crate::utils::sync_validation::validate_before_sync(
                            &relative_path_clone,
                            &full_path_clone,
                            &previously_synced,
                            &self.config.repo_path,
                        );

                        if !validation.is_safe {
                            let state = &mut self.ui_state.dotfile_selection;
                            state.status_message = validation.error_message.clone();
                            return Ok(());
                        }

                        // Show confirmation modal
                        let state = &mut self.ui_state.dotfile_selection;
                        state.show_custom_file_confirm = true;
                        state.custom_file_confirm_path = Some(full_path_clone.clone());
                        state.custom_file_confirm_relative = Some(relative_path_clone.clone());
                        state.file_browser_mode = false;
                        state.adding_custom_file = false;
                        state.file_browser_path_input.clear();
                        state.file_browser_path_cursor = 0;
                        state.focus = DotfileSelectionFocus::FilesList;

                        // Re-scan to refresh the list
                        self.scan_dotfiles()?;

                        // Add custom file to list if it's synced but not in scanned list
                        let state = &mut self.ui_state.dotfile_selection;
                        if !state
                            .dotfiles
                            .iter()
                            .any(|d| d.relative_path.to_string_lossy() == relative_path_clone)
                        {
                            // File is synced but not in default list, add it manually
                            use crate::file_manager::Dotfile;
                            state.dotfiles.push(Dotfile {
                                original_path: full_path_clone.clone(),
                                relative_path: PathBuf::from(&relative_path_clone),
                                synced: true,
                                description: None,
                            });
                        }

                        // Find and select the file in the list
                        if let Some(index) = state
                            .dotfiles
                            .iter()
                            .position(|d| d.relative_path.to_string_lossy() == relative_path_clone)
                        {
                            state.dotfile_list_state.select(Some(index));
                            // Mark as selected for sync
                            state.selected_for_sync.insert(index);
                        }
                    }
                }
            }
            KeyCode::Esc => {
                state.adding_custom_file = false;
                state.custom_file_input.clear();
                state.custom_file_cursor = 0;
            }
            _ => {}
        }

        Ok(())
    }

    /// Check if there are unsaved changes (selected files differ from synced files)
    /// Check if there are unsaved changes in the dotfile selection
    /// Compares the currently selected files with the synced files in the active profile
    #[allow(dead_code)]
    fn has_unsaved_changes(&self) -> bool {
        let state = &self.ui_state.dotfile_selection;

        // Get currently selected file paths
        let currently_selected: std::collections::HashSet<String> = state
            .selected_for_sync
            .iter()
            .filter_map(|&idx| {
                state
                    .dotfiles
                    .get(idx)
                    .map(|d| d.relative_path.to_string_lossy().to_string())
            })
            .collect();

        // Get previously synced files from active profile
        let previously_synced: std::collections::HashSet<String> = self
            .get_active_profile_info()
            .ok()
            .flatten()
            .map(|p| p.synced_files.iter().cloned().collect())
            .unwrap_or_default();

        // Check if they differ
        currently_selected != previously_synced
    }

    /// Add a single file to sync (copy to repo, create symlink, update manifest)
    fn add_file_to_sync(&mut self, file_index: usize) -> Result<()> {
        use crate::utils::{sync_validation, SymlinkManager};

        // Get profile info before borrowing state
        let profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();
        let previously_synced: std::collections::HashSet<String> = self
            .get_active_profile_info()
            .ok()
            .flatten()
            .map(|p| p.synced_files.iter().cloned().collect())
            .unwrap_or_default();

        let state = &mut self.ui_state.dotfile_selection;
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

        if previously_synced.contains(&relative_str) {
            debug!("File already synced: {}", relative_str);
            // Already synced, just mark as selected
            state.selected_for_sync.insert(file_index);
            return Ok(());
        }

        // VALIDATE BEFORE ANY OPERATIONS - prevent data loss
        let validation = sync_validation::validate_before_sync(
            &relative_str,
            &dotfile.original_path,
            &previously_synced,
            &repo_path,
        );
        if !validation.is_safe {
            let error_msg = validation
                .error_message
                .unwrap_or_else(|| "Cannot add this file or directory".to_string());
            state.status_message = Some(format!("Error: {}", error_msg));
            warn!("Validation failed for {}: {}", relative_str, error_msg);
            return Ok(());
        }

        // Create file manager for symlink resolution
        let file_manager = crate::file_manager::FileManager::new()?;

        // Validate symlink can be created before deleting original file
        let home_dir = crate::utils::get_home_dir();
        let target_path = home_dir.join(&dotfile.relative_path);
        let profile_path = repo_path.join(&profile_name);
        let repo_file_path = profile_path.join(&dotfile.relative_path);

        // Handle symlinks: resolve to original file for validation
        let original_source = if file_manager.is_symlink(&dotfile.original_path) {
            file_manager.resolve_symlink(&dotfile.original_path)?
        } else {
            dotfile.original_path.clone()
        };

        let symlink_validation = sync_validation::validate_symlink_creation(
            &original_source,
            &repo_file_path,
            &target_path,
        )
        .context("Failed to validate symlink creation")?;
        if !symlink_validation.is_safe {
            let error_msg = symlink_validation
                .error_message
                .unwrap_or_else(|| "Cannot create symlink".to_string());
            state.status_message = Some(format!("Error: {}", error_msg));
            warn!(
                "Symlink validation failed for {}: {}",
                relative_str, error_msg
            );
            return Ok(());
        }

        info!(
            "TUI: Adding file to sync: {} (profile: {})",
            relative_str, profile_name
        );
        debug!("TUI: Source path: {:?}", dotfile.original_path);
        debug!("TUI: Repo destination: {:?}", repo_file_path);
        debug!("TUI: Symlink target: {:?}", target_path);

        // Copy file to repo
        let file_manager = crate::file_manager::FileManager::new()?;

        // Create parent directories
        if let Some(parent) = repo_file_path.parent() {
            if !parent.exists() {
                debug!("TUI: Creating repo directory: {:?}", parent);
            }
            std::fs::create_dir_all(parent).context("Failed to create repo directory")?;
        }

        // Handle symlinks: resolve to original file
        let source_path = if file_manager.is_symlink(&dotfile.original_path) {
            debug!("TUI: Resolving symlink: {:?}", dotfile.original_path);
            let resolved = file_manager.resolve_symlink(&dotfile.original_path)?;
            debug!("TUI: Resolved symlink to: {:?}", resolved);
            resolved
        } else {
            dotfile.original_path.clone()
        };

        // Copy to repo FIRST (before deleting original)
        // This ensures we have a backup before any destructive operations
        info!("TUI: Copying file to repository...");
        file_manager
            .copy_to_repo(&source_path, &repo_file_path)
            .context("Failed to copy file to repo")?;
        info!("TUI: Successfully copied file to repository");

        // Create symlink using SymlinkManager
        info!("TUI: Creating symlink...");
        let backup_enabled = state.backup_enabled;
        let mut symlink_mgr = SymlinkManager::new_with_backup(repo_path.clone(), backup_enabled)?;
        symlink_mgr
            .activate_profile(&profile_name, std::slice::from_ref(&relative_str))
            .context("Failed to create symlink")?;
        info!("TUI: Successfully created symlink");

        // Update manifest
        let relative_str_clone = relative_str.clone();
        info!("TUI: Updating profile manifest...");
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
        let current_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .map(|p| p.synced_files.clone())
            .unwrap_or_default();
        if !current_files.contains(&relative_str) {
            debug!(
                "TUI: Adding {} to manifest for profile {}",
                relative_str, profile_name
            );
            let mut new_files = current_files;
            new_files.push(relative_str);
            manifest.update_synced_files(&profile_name, new_files)?;
            manifest.save(&repo_path)?;
            info!(
                "TUI: Updated manifest with new file: {}",
                relative_str_clone
            );
        } else {
            debug!("TUI: File already in manifest, skipping update");
        }

        // Mark as selected and synced
        state.selected_for_sync.insert(file_index);
        state.dotfiles[file_index].synced = true;

        info!("Successfully added file to sync: {}", relative_str_clone);
        Ok(())
    }

    /// Add a custom file directly to sync (bypasses scan_dotfiles since custom files aren't in default list)
    fn add_custom_file_to_sync(&mut self, full_path: &Path, relative_path: &str) -> Result<()> {
        use crate::utils::{sync_validation, SymlinkManager};

        // Get profile info before borrowing state
        let profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();
        let previously_synced: std::collections::HashSet<String> = self
            .get_active_profile_info()
            .ok()
            .flatten()
            .map(|p| p.synced_files.iter().cloned().collect())
            .unwrap_or_default();

        if previously_synced.contains(relative_path) {
            debug!("Custom file already synced: {}", relative_path);
            // Already synced, nothing to do
            return Ok(());
        }

        // VALIDATE BEFORE ANY OPERATIONS - prevent data loss
        let validation = sync_validation::validate_before_sync(
            relative_path,
            full_path,
            &previously_synced,
            &repo_path,
        );
        if !validation.is_safe {
            let error_msg = validation
                .error_message
                .unwrap_or_else(|| "Cannot add this file or directory".to_string());
            let state = &mut self.ui_state.dotfile_selection;
            state.status_message = Some(format!("Error: {}", error_msg));
            warn!(
                "Validation failed for custom file {}: {}",
                relative_path, error_msg
            );
            return Ok(());
        }

        // Create file manager for symlink resolution
        let file_manager = crate::file_manager::FileManager::new()?;

        // Validate symlink can be created before deleting original file
        let home_dir = crate::utils::get_home_dir();
        let target_path = home_dir.join(relative_path);
        let profile_path = repo_path.join(&profile_name);
        let relative_path_buf = PathBuf::from(relative_path);
        let repo_file_path = profile_path.join(&relative_path_buf);

        // Handle symlinks: resolve to original file for validation
        let original_source = if file_manager.is_symlink(full_path) {
            file_manager.resolve_symlink(full_path)?
        } else {
            full_path.to_path_buf()
        };

        let symlink_validation = sync_validation::validate_symlink_creation(
            &original_source,
            &repo_file_path,
            &target_path,
        )
        .context("Failed to validate symlink creation")?;
        if !symlink_validation.is_safe {
            let error_msg = symlink_validation
                .error_message
                .unwrap_or_else(|| "Cannot create symlink".to_string());
            let state = &mut self.ui_state.dotfile_selection;
            state.status_message = Some(format!("Error: {}", error_msg));
            warn!(
                "Symlink validation failed for custom file {}: {}",
                relative_path, error_msg
            );
            return Ok(());
        }

        info!(
            "TUI: Adding custom file to sync: {} -> {} (profile: {})",
            full_path.display(),
            relative_path,
            profile_name
        );
        debug!("TUI: Source path: {:?}", full_path);
        debug!("TUI: Repo destination: {:?}", repo_file_path);
        debug!("TUI: Symlink target: {:?}", target_path);

        // Copy file to repo (file_manager already created above for validation)

        // Create parent directories
        if let Some(parent) = repo_file_path.parent() {
            if !parent.exists() {
                debug!("TUI: Creating repo directory: {:?}", parent);
            }
            std::fs::create_dir_all(parent).context("Failed to create repo directory")?;
        }

        // Handle symlinks: resolve to original file
        let source_path = if file_manager.is_symlink(full_path) {
            debug!("TUI: Resolving symlink: {:?}", full_path);
            let resolved = file_manager.resolve_symlink(full_path)?;
            debug!("TUI: Resolved symlink to: {:?}", resolved);
            resolved
        } else {
            full_path.to_path_buf()
        };

        // Copy to repo FIRST (before deleting original)
        // This ensures we have a backup before any destructive operations
        info!("TUI: Copying custom file to repository...");
        file_manager
            .copy_to_repo(&source_path, &repo_file_path)
            .context("Failed to copy file to repo")?;
        info!("TUI: Successfully copied custom file to repository");

        // Create symlink using SymlinkManager
        info!("TUI: Creating symlink for custom file...");
        let backup_enabled = self.ui_state.dotfile_selection.backup_enabled;
        let mut symlink_mgr = SymlinkManager::new_with_backup(repo_path.clone(), backup_enabled)?;
        symlink_mgr
            .activate_profile(&profile_name, &[relative_path.to_string()])
            .context("Failed to create symlink")?;
        info!("TUI: Successfully created symlink for custom file");

        // Update manifest
        info!("TUI: Updating profile manifest for custom file...");
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
        let current_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .map(|p| p.synced_files.clone())
            .unwrap_or_default();
        if !current_files.contains(&relative_path.to_string()) {
            debug!(
                "TUI: Adding custom file {} to manifest for profile {}",
                relative_path, profile_name
            );
            let mut new_files = current_files;
            new_files.push(relative_path.to_string());
            manifest.update_synced_files(&profile_name, new_files)?;
            manifest.save(&repo_path)?;
        }

        // Check if this is a custom file (not in default dotfile candidates)
        use crate::dotfile_candidates::get_default_dotfile_paths;
        let default_paths = get_default_dotfile_paths();
        let is_custom = !default_paths.iter().any(|p| p == relative_path);

        if is_custom {
            // Add to config.custom_files if not already there
            if !self
                .config
                .custom_files
                .contains(&relative_path.to_string())
            {
                self.config.custom_files.push(relative_path.to_string());
                self.config.save(&self.config_path)?;
            }
        }

        // Don't add to dotfiles list here - it will be added after scan_dotfiles() is called
        // This function only handles the actual syncing (copy, symlink, manifest update)
        info!("Successfully added custom file to sync: {}", relative_path);
        Ok(())
    }

    /// Remove a single file from sync (restore from repo, remove symlink, update manifest)
    fn remove_file_from_sync(&mut self, file_index: usize) -> Result<()> {
        use crate::utils::SymlinkManager;

        // Get profile info before borrowing state
        let profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();
        let home_dir = crate::utils::get_home_dir();
        let previously_synced: std::collections::HashSet<String> = self
            .get_active_profile_info()
            .ok()
            .flatten()
            .map(|p| p.synced_files.iter().cloned().collect())
            .unwrap_or_default();

        let state = &mut self.ui_state.dotfile_selection;
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

        if !previously_synced.contains(&relative_str) {
            debug!("File not synced, skipping removal: {}", relative_str);
            // Not synced, just unmark as selected
            state.selected_for_sync.remove(&file_index);
            return Ok(());
        }

        info!(
            "Removing file from sync: {} (profile: {})",
            relative_str, profile_name
        );

        let target_path = home_dir.join(&dotfile.relative_path);
        let repo_file_path = repo_path.join(&profile_name).join(&dotfile.relative_path);

        // Restore file from repo if symlink exists
        if target_path.symlink_metadata().is_ok() {
            let metadata = target_path.symlink_metadata().unwrap();
            if metadata.is_symlink() {
                // Remove symlink
                std::fs::remove_file(&target_path).context("Failed to remove symlink")?;

                // Copy file from repo back to home
                if repo_file_path.exists() {
                    if repo_file_path.is_dir() {
                        crate::file_manager::copy_dir_all(&repo_file_path, &target_path)
                            .context("Failed to restore directory from repo")?;
                    } else {
                        std::fs::copy(&repo_file_path, &target_path)
                            .context("Failed to restore file from repo")?;
                    }
                }
            }
        }

        // Update symlink tracking
        let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;
        let remaining_files: Vec<String> = {
            let manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
            manifest
                .profiles
                .iter()
                .find(|p| p.name == profile_name)
                .map(|p| {
                    p.synced_files
                        .iter()
                        .filter(|f| f != &&relative_str)
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };

        // Deactivate and reactivate with remaining files
        let _ = symlink_mgr.deactivate_profile(&profile_name);
        if !remaining_files.is_empty() {
            let _ = symlink_mgr.activate_profile(&profile_name, &remaining_files);
        }

        // Remove from repo
        if repo_file_path.exists() {
            if repo_file_path.is_dir() {
                std::fs::remove_dir_all(&repo_file_path)
                    .context("Failed to remove directory from repo")?;
            } else {
                std::fs::remove_file(&repo_file_path).context("Failed to remove file from repo")?;
            }
        }

        // Update manifest
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
        manifest.update_synced_files(&profile_name, remaining_files)?;
        manifest.save(&repo_path)?;

        // Note: Don't remove from config.custom_files - custom files persist even if removed from sync
        // This allows users to re-add them easily later

        // Unmark as selected and synced
        state.selected_for_sync.remove(&file_index);
        state.dotfiles[file_index].synced = false;

        info!("Successfully removed file from sync: {}", relative_str);
        Ok(())
    }

    /// Scan for dotfiles and populate the selection state
    fn scan_dotfiles(&mut self) -> Result<()> {
        use crate::dotfile_candidates::get_default_dotfile_paths;

        let file_manager = crate::file_manager::FileManager::new()?;
        let dotfile_names = get_default_dotfile_paths();
        let mut found = file_manager.scan_dotfiles(&dotfile_names);

        // Mark files that are already synced - use active profile's synced_files from manifest
        let synced_set: std::collections::HashSet<String> = self
            .get_active_profile_info()
            .ok()
            .flatten()
            .map(|p| p.synced_files.iter().cloned().collect())
            .unwrap_or_default();

        let mut selected_indices = std::collections::HashSet::new();
        for (i, dotfile) in found.iter_mut().enumerate() {
            let relative_str = dotfile.relative_path.to_string_lossy().to_string();
            dotfile.synced = synced_set.contains(&relative_str);

            // If already synced, add to selected set
            if dotfile.synced {
                selected_indices.insert(i);
            }
        }

        // Add any synced files that aren't in the default list (custom files)
        let synced_files_not_found: Vec<String> = synced_set
            .iter()
            .filter(|s| {
                !found
                    .iter()
                    .any(|d| d.relative_path.to_string_lossy() == **s)
            })
            .cloned()
            .collect();

        let home_dir = crate::utils::get_home_dir();
        for synced_file in synced_files_not_found {
            let full_path = home_dir.join(&synced_file);
            if full_path.exists() {
                let relative_path_buf = PathBuf::from(&synced_file);
                let index = found.len();
                found.push(crate::file_manager::Dotfile {
                    original_path: full_path,
                    relative_path: relative_path_buf,
                    synced: true,
                    description: None,
                });
                selected_indices.insert(index);
            }
        }

        // Add custom files from config (even if not synced) - these are known files
        for custom_file in &self.config.custom_files {
            if !found
                .iter()
                .any(|d| d.relative_path.to_string_lossy() == *custom_file)
            {
                let full_path = home_dir.join(custom_file);
                if full_path.exists() {
                    let relative_path_buf = PathBuf::from(custom_file);
                    let index = found.len();
                    let is_synced = synced_set.contains(custom_file);
                    found.push(crate::file_manager::Dotfile {
                        original_path: full_path,
                        relative_path: relative_path_buf,
                        synced: is_synced,
                        description: None,
                    });
                    if is_synced {
                        selected_indices.insert(index);
                    }
                }
            }
        }

        self.ui_state.dotfile_selection.dotfiles = found;
        self.ui_state.dotfile_selection.selected_index = 0;
        self.ui_state.dotfile_selection.preview_index = None;
        self.ui_state.dotfile_selection.scroll_offset = 0;
        self.ui_state.dotfile_selection.preview_scroll = 0;
        self.ui_state.dotfile_selection.selected_for_sync = selected_indices;
        // Initialize ListState with first item selected if available
        if !self.ui_state.dotfile_selection.dotfiles.is_empty() {
            self.ui_state
                .dotfile_selection
                .dotfile_list_state
                .select(Some(0));
        } else {
            self.ui_state
                .dotfile_selection
                .dotfile_list_state
                .select(None);
        }

        Ok(())
    }

    /// Sync selected files to repository using SymlinkManager
    /// NOTE: This function is deprecated. Files are now synced immediately when selected/unselected.
    /// Kept for reference but should not be used.
    #[allow(dead_code)]
    fn sync_selected_files(&mut self) -> Result<()> {
        use crate::utils::SymlinkManager;

        // Get profile info before borrowing state
        let profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();
        let previously_synced: std::collections::HashSet<String> = self
            .get_active_profile_info()
            .ok()
            .flatten()
            .map(|p| p.synced_files.iter().cloned().collect())
            .unwrap_or_default();

        let state = &mut self.ui_state.dotfile_selection;
        let file_manager = crate::file_manager::FileManager::new()?;

        let mut synced_count = 0;
        let mut unsynced_count = 0;
        let mut errors = Vec::new();

        // Get list of currently selected indices
        let currently_selected: std::collections::HashSet<usize> =
            state.selected_for_sync.iter().cloned().collect();

        // Files to sync
        let mut files_to_sync: Vec<String> = Vec::new();

        // Step 1: Copy files to repo for newly selected files
        for &index in &currently_selected {
            if index >= state.dotfiles.len() {
                continue;
            }

            let dotfile = &state.dotfiles[index];
            let relative_str = dotfile.relative_path.to_string_lossy().to_string();

            // If not already synced, copy to repo
            if !previously_synced.contains(&relative_str) {
                let profile_path = repo_path.join(&profile_name);
                let repo_file_path = profile_path.join(&dotfile.relative_path);

                // Create parent directories in repo
                if let Some(parent) = repo_file_path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        errors.push(format!(
                            "Failed to create repo directory for {}: {}",
                            relative_str, e
                        ));
                        continue;
                    }
                }

                // Handle symlinks: resolve to original file
                let source_path = if file_manager.is_symlink(&dotfile.original_path) {
                    match file_manager.resolve_symlink(&dotfile.original_path) {
                        Ok(p) => p,
                        Err(e) => {
                            errors.push(format!(
                                "Failed to resolve symlink for {}: {}",
                                relative_str, e
                            ));
                            continue;
                        }
                    }
                } else {
                    dotfile.original_path.clone()
                };

                // Copy original file/directory to repo
                match file_manager.copy_to_repo(&source_path, &repo_file_path) {
                    Ok(_) => {
                        files_to_sync.push(relative_str.clone());
                        state.dotfiles[index].synced = true;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to copy {} to repo: {}", relative_str, e));
                    }
                }
            } else {
                files_to_sync.push(relative_str);
            }
        }

        // Step 2: Use SymlinkManager to activate with all selected files
        if !files_to_sync.is_empty() {
            // Use backup_enabled from UI state (which may have been toggled)
            let backup_enabled = state.backup_enabled;
            let mut symlink_mgr =
                SymlinkManager::new_with_backup(repo_path.clone(), backup_enabled)?;

            match symlink_mgr.activate_profile(&profile_name, &files_to_sync) {
                Ok(operations) => {
                    for op in operations {
                        if matches!(
                            op.status,
                            crate::utils::symlink_manager::OperationStatus::Success
                        ) {
                            synced_count += 1;
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Failed to activate symlinks: {}", e));
                }
            }
        }

        // Step 3: Handle unsyncing (deselected files)
        let dotfiles_to_unsync: Vec<(usize, String)> = state
            .dotfiles
            .iter()
            .enumerate()
            .filter(|(index, dotfile)| {
                let relative_str = dotfile.relative_path.to_string_lossy().to_string();
                previously_synced.contains(&relative_str) && !currently_selected.contains(index)
            })
            .map(|(index, dotfile)| (index, dotfile.relative_path.to_string_lossy().to_string()))
            .collect();

        if !dotfiles_to_unsync.is_empty() {
            let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;
            let home_dir = crate::utils::get_home_dir();

            for (index, relative_str) in dotfiles_to_unsync {
                let dotfile = &state.dotfiles[index];
                let target_path = home_dir.join(&dotfile.relative_path);
                let repo_file_path = repo_path.join(&profile_name).join(&dotfile.relative_path);

                // Step 1: If target is a symlink, restore the original file from repo BEFORE deleting from repo
                if target_path.symlink_metadata().is_ok() {
                    let metadata = target_path.symlink_metadata().unwrap();
                    if metadata.is_symlink() {
                        // It's a symlink, restore the actual file from repo
                        if repo_file_path.exists() {
                            // Remove the symlink first
                            if let Err(e) = std::fs::remove_file(&target_path) {
                                errors.push(format!(
                                    "Failed to remove symlink for {}: {}",
                                    relative_str, e
                                ));
                                continue;
                            }

                            // Copy the file from repo back to home directory
                            let copy_result = if repo_file_path.is_dir() {
                                crate::file_manager::copy_dir_all(&repo_file_path, &target_path)
                            } else {
                                std::fs::copy(&repo_file_path, &target_path)
                                    .map(|_| ())
                                    .context("Failed to copy file")
                            };

                            if let Err(e) = copy_result {
                                errors.push(format!(
                                    "Failed to restore {} from repo: {}",
                                    relative_str, e
                                ));
                                continue;
                            }

                            info!("Restored {} from repo before unsyncing", relative_str);
                        } else {
                            // Repo file doesn't exist, just remove the orphaned symlink
                            if let Err(e) = std::fs::remove_file(&target_path) {
                                errors.push(format!(
                                    "Failed to remove orphaned symlink for {}: {}",
                                    relative_str, e
                                ));
                            }
                            info!("Removed orphaned symlink for {}", relative_str);
                        }
                    }
                }

                // Step 2: Now remove from SymlinkManager tracking
                // Get remaining files before deactivating (need to get from manifest)
                let remaining_files: Vec<String> = {
                    // Clone what we need from state before the borrow
                    let relative_str_clone = relative_str.clone();
                    // Get from manifest (state is still borrowed, but we can work around it)
                    // Actually, we need to get this before the loop or restructure
                    // For now, just get it from the manifest directly
                    crate::utils::ProfileManifest::load_or_backfill(&repo_path)
                        .ok()
                        .and_then(|manifest| {
                            manifest
                                .profiles
                                .iter()
                                .find(|p| p.name == profile_name)
                                .map(|p| {
                                    p.synced_files
                                        .iter()
                                        .filter(|f| f != &&relative_str_clone)
                                        .cloned()
                                        .collect()
                                })
                        })
                        .unwrap_or_default()
                };

                match symlink_mgr.deactivate_profile(&profile_name) {
                    Ok(_) => {
                        // Re-activate with remaining files

                        if !remaining_files.is_empty() {
                            let _ = symlink_mgr.activate_profile(&profile_name, &remaining_files);
                        }
                    }
                    Err(e) => {
                        info!("Note: Could not update symlink tracking: {}", e);
                    }
                }

                // Step 3: Finally, remove from repo
                if repo_file_path.exists() {
                    let remove_result = if repo_file_path.is_dir() {
                        std::fs::remove_dir_all(&repo_file_path)
                    } else {
                        std::fs::remove_file(&repo_file_path)
                    };

                    if let Err(e) = remove_result {
                        errors.push(format!(
                            "Failed to remove {} from repo: {}",
                            relative_str, e
                        ));
                        continue;
                    }
                }

                unsynced_count += 1;
                state.dotfiles[index].synced = false;
            }
        }

        // Step 4: Update manifest with new synced files
        let new_synced_files: Vec<String> = state
            .dotfiles
            .iter()
            .enumerate()
            .filter(|(i, _)| currently_selected.contains(i))
            .map(|(_, d)| d.relative_path.to_string_lossy().to_string())
            .collect();

        // Update manifest (profile_name already cloned above)
        // Clone what we need from state before loading manifest
        let new_synced_files_clone = new_synced_files.clone();
        let profile_name_clone = profile_name.clone();
        let repo_path_clone = repo_path.clone();
        let synced_count_clone = synced_count;
        let unsynced_count_clone = unsynced_count;
        let errors_clone = errors.clone();

        // Release state borrow by ending its scope (using a block)
        {
            let _ = state;
        }

        // Now we can load manifest
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path_clone)?;
        manifest.update_synced_files(&profile_name_clone, new_synced_files_clone)?;
        manifest.save(&repo_path_clone)?;

        // Show summary
        let summary = if errors_clone.is_empty() {
            format!(
                "Sync Complete!\n\n✓ Synced: {} files\n✓ Unsynced: {} files\n\nAll operations completed successfully.",
                synced_count_clone, unsynced_count_clone
            )
        } else {
            format!(
                "Sync Completed with Errors\n\n✓ Synced: {} files\n✓ Unsynced: {} files\n\nErrors:\n{}\n\nSome operations failed. Please review the errors above.",
                synced_count_clone,
                unsynced_count_clone,
                errors_clone.join("\n")
            )
        };

        // Re-borrow state to set status message
        self.ui_state.dotfile_selection.status_message = Some(summary);
        Ok(())
    }

    /// Load changed files from git repository
    fn load_changed_files(&mut self) {
        let repo_path = &self.config.repo_path;

        // Check if repo exists
        if !repo_path.exists() {
            self.ui_state.sync_with_remote.changed_files = vec![];
            return;
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(_) => {
                self.ui_state.sync_with_remote.changed_files = vec![];
                return;
            }
        };

        // Get changed files
        match git_mgr.get_changed_files() {
            Ok(files) => {
                self.ui_state.sync_with_remote.changed_files = files;
                // Select first item if list is not empty
                if !self.ui_state.sync_with_remote.changed_files.is_empty() {
                    self.ui_state.sync_with_remote.list_state.select(Some(0));
                    self.update_diff_preview();
                }
            }
            Err(_) => {
                self.ui_state.sync_with_remote.changed_files = vec![];
            }
        }
    }

    /// Update the diff preview based on the selected file
    fn update_diff_preview(&mut self) {
        // Clear existing diff content
        self.ui_state.sync_with_remote.diff_content = None;

        let selected_idx = if let Some(idx) = self.ui_state.sync_with_remote.list_state.selected() {
            idx
        } else {
            return;
        };

        if selected_idx >= self.ui_state.sync_with_remote.changed_files.len() {
            return;
        }

        let file_info = &self.ui_state.sync_with_remote.changed_files[selected_idx];
        // Format is "X filename"
        let parts: Vec<&str> = file_info.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return;
        }
        let path_str = parts[1].trim();

        // Use repo_path from config
        let repo_path = &self.config.repo_path;

        if let Ok(git_mgr) = GitManager::open_or_init(repo_path) {
            if let Ok(Some(diff)) = git_mgr.get_diff_for_file(path_str) {
                self.ui_state.sync_with_remote.diff_content = Some(diff);
                self.ui_state.sync_with_remote.preview_scroll = 0;
            }
        }
    }

    /// Start pushing changes (async operation with progress updates)
    fn start_sync(&mut self) -> Result<()> {
        info!("Starting sync operation");

        // Check if repository is configured (GitHub or Local mode)
        if !self.config.is_repo_configured() {
            warn!("Sync attempted but repository not configured");
            self.ui_state.sync_with_remote.sync_result = Some(
                "Error: Repository not configured.\n\nPlease set up your repository first from the main menu.".to_string()
            );
            self.ui_state.sync_with_remote.show_result_popup = true;
            return Ok(());
        }

        let repo_path = self.config.repo_path.clone();

        // Check if repo exists
        if !repo_path.exists() {
            warn!("Sync attempted but repository not found: {:?}", repo_path);
            self.ui_state.sync_with_remote.sync_result = Some(format!(
                "Error: Repository not found at {:?}\n\nPlease sync some files first.",
                repo_path
            ));
            self.ui_state.sync_with_remote.show_result_popup = true;
            return Ok(());
        }

        // Mark as syncing
        self.ui_state.sync_with_remote.is_syncing = true;
        self.ui_state.sync_with_remote.sync_progress = Some("Committing changes...".to_string());

        // Don't call draw() here - let the main loop handle it
        // The next draw cycle will show the progress

        // Perform sync: commit -> pull with rebase -> push
        let git_mgr = match GitManager::open_or_init(&repo_path) {
            Ok(mgr) => mgr,
            Err(e) => {
                self.ui_state.sync_with_remote.is_syncing = false;
                self.ui_state.sync_with_remote.sync_progress = None;
                self.ui_state.sync_with_remote.sync_result =
                    Some(format!("Error: Failed to open repository: {}", e));
                self.ui_state.sync_with_remote.show_result_popup = true;
                return Ok(());
            }
        };

        let branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| self.config.default_branch.clone());

        // Get token based on repo mode
        // In Local mode, we use system credentials (no token needed)
        // In GitHub mode, we require a token
        let token_string = match self.config.repo_mode {
            crate::config::RepoMode::Local => None,
            crate::config::RepoMode::GitHub => self.config.get_github_token(),
        };
        let token = token_string.as_deref();

        // Only require token for GitHub mode
        if matches!(self.config.repo_mode, crate::config::RepoMode::GitHub) && token.is_none() {
            self.ui_state.sync_with_remote.is_syncing = false;
            self.ui_state.sync_with_remote.sync_progress = None;
            self.ui_state.sync_with_remote.sync_result = Some(
                "Error: GitHub token not found.\n\n\
                Please provide a GitHub token using one of these methods:\n\n\
                1. Set the DOTSTATE_GITHUB_TOKEN environment variable:\n\
                   export DOTSTATE_GITHUB_TOKEN=ghp_your_token_here\n\n\
                2. Configure it in the TUI by going to the main menu\n\n\
                Create a token at: https://github.com/settings/tokens\n\
                Required scope: repo (full control of private repositories)"
                    .to_string(),
            );
            self.ui_state.sync_with_remote.show_result_popup = true;
            return Ok(());
        }

        // Step 1: Commit all changes
        let commit_msg = git_mgr
            .generate_commit_message()
            .unwrap_or_else(|_| "Update dotfiles".to_string());
        let result = match git_mgr.commit_all(&commit_msg) {
            Ok(_) => {
                // Step 2: Pull with rebase
                self.ui_state.sync_with_remote.sync_progress =
                    Some("Pulling changes from remote...".to_string());

                match git_mgr.pull_with_rebase("origin", &branch, token) {
                    Ok(pulled_count) => {
                        self.ui_state.sync_with_remote.pulled_changes_count = Some(pulled_count);

                        // Step 3: Push to remote
                        self.ui_state.sync_with_remote.sync_progress =
                            Some("Pushing to remote...".to_string());

                        match git_mgr.push("origin", &branch, token) {
                            Ok(_) => {
                                let mut success_msg = format!("✓ Successfully synced with remote!\n\nBranch: {}\nRepository: {:?}", branch, repo_path);
                                if pulled_count > 0 {
                                    success_msg.push_str(&format!(
                                        "\n\nPulled {} change(s) from remote.",
                                        pulled_count
                                    ));
                                } else {
                                    success_msg.push_str("\n\nNo changes pulled from remote.");
                                }
                                success_msg
                            }
                            Err(e) => {
                                let mut error_msg =
                                    format!("Error: Failed to push to remote: {}", e);
                                let mut source = e.source();
                                while let Some(err) = source {
                                    error_msg.push_str(&format!("\n  Caused by: {}", err));
                                    source = err.source();
                                }
                                error_msg
                            }
                        }
                    }
                    Err(e) => {
                        let mut error_msg = format!("Error: Failed to pull from remote: {}", e);
                        let mut source = e.source();
                        while let Some(err) = source {
                            error_msg.push_str(&format!("\n  Caused by: {}", err));
                            source = err.source();
                        }
                        error_msg
                    }
                }
            }
            Err(e) => {
                // Include the full error chain for debugging
                let mut error_msg = format!("Error: Failed to commit changes: {}", e);
                let mut source = e.source();
                while let Some(err) = source {
                    error_msg.push_str(&format!("\n  Caused by: {}", err));
                    source = err.source();
                }
                error_msg
            }
        };

        // Update state with result
        self.ui_state.sync_with_remote.is_syncing = false;
        self.ui_state.sync_with_remote.sync_progress = None;
        self.ui_state.sync_with_remote.sync_result = Some(result);
        self.ui_state.sync_with_remote.show_result_popup = true;

        Ok(())
    }

    /// Pull changes from remote repository (deprecated - use sync instead)
    #[allow(dead_code)]
    fn pull_changes(&mut self) -> Result<()> {
        // Check if repository is configured (GitHub or Local mode)
        if !self.config.is_repo_configured() {
            self.ui_state.dotfile_selection.status_message = Some(
                "Error: Repository not configured.\n\nPlease set up your repository first from the main menu.".to_string()
            );
            return Ok(());
        }

        let repo_path = &self.config.repo_path;

        // Check if repo exists
        if !repo_path.exists() {
            self.ui_state.dotfile_selection.status_message = Some(format!(
                "Error: Repository not found at {:?}\n\nPlease sync some files first.",
                repo_path
            ));
            return Ok(());
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(e) => {
                self.ui_state.dotfile_selection.status_message =
                    Some(format!("Error: Failed to open repository: {}", e));
                return Ok(());
            }
        };

        // Get current branch
        let branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| self.config.default_branch.clone());

        // Get token based on repo mode (None for Local mode)
        let token_string = match self.config.repo_mode {
            crate::config::RepoMode::Local => None,
            crate::config::RepoMode::GitHub => self.config.get_github_token(),
        };
        let token = token_string.as_deref();

        // Only require token for GitHub mode
        if matches!(self.config.repo_mode, crate::config::RepoMode::GitHub) && token.is_none() {
            self.ui_state.dotfile_selection.status_message = Some(
                "Error: GitHub token not found.\n\n\
                Please provide a GitHub token using one of these methods:\n\n\
                1. Set the DOTSTATE_GITHUB_TOKEN environment variable:\n\
                   export DOTSTATE_GITHUB_TOKEN=ghp_your_token_here\n\n\
                2. Configure it in the TUI by going to the main menu\n\n\
                Create a token at: https://github.com/settings/tokens\n\
                Required scope: repo (full control of private repositories)"
                    .to_string(),
            );
            return Ok(());
        }

        match git_mgr.pull("origin", &branch, token) {
            Ok(_) => {
                self.ui_state.dotfile_selection.status_message = Some(
                    format!("✓ Successfully pulled changes from remote!\n\nBranch: {}\nRepository: {:?}\n\nNote: You may need to re-sync files if the repository structure changed.", branch, repo_path)
                );
            }
            Err(e) => {
                self.ui_state.dotfile_selection.status_message =
                    Some(format!("Error: Failed to pull from remote: {}", e));
            }
        }

        Ok(())
    }

    /// Handle input for file browser
    fn handle_file_browser_input(&mut self, key_code: KeyCode) -> Result<()> {
        use crate::ui::DotfileSelectionFocus;
        let state = &mut self.ui_state.dotfile_selection;

        // Handle path input if focused
        if state.file_browser_path_focused && state.focus == DotfileSelectionFocus::FileBrowserInput
        {
            match key_code {
                // Text input handling - use text input utility
                KeyCode::Char(c) => {
                    crate::utils::handle_char_insertion(
                        &mut state.file_browser_path_input,
                        &mut state.file_browser_path_cursor,
                        c,
                    );
                    return Ok(());
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                    crate::utils::handle_cursor_movement(
                        &state.file_browser_path_input,
                        &mut state.file_browser_path_cursor,
                        key_code,
                    );
                    return Ok(());
                }
                KeyCode::Backspace => {
                    crate::utils::handle_backspace(
                        &mut state.file_browser_path_input,
                        &mut state.file_browser_path_cursor,
                    );
                    return Ok(());
                }
                KeyCode::Delete => {
                    crate::utils::handle_delete(
                        &mut state.file_browser_path_input,
                        &mut state.file_browser_path_cursor,
                    );
                    return Ok(());
                }
                KeyCode::Enter => {
                    // Load path from input into file browser
                    let path_str = state.file_browser_path_input.trim();
                    if !path_str.is_empty() {
                        let full_path = crate::utils::expand_path(path_str);

                        if full_path.exists() {
                            if full_path.is_dir() {
                                state.file_browser_path = full_path.clone();
                                // Update path input to show the new directory
                                state.file_browser_path_input =
                                    state.file_browser_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor =
                                    state.file_browser_path_input.chars().count();
                                state.file_browser_list_state.select(Some(0));
                                state.focus = DotfileSelectionFocus::FileBrowserList;
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path =
                                    state.file_browser_path.clone();
                                self.refresh_file_browser()?;
                                return Ok(());
                            } else {
                                // It's a file - directly sync it
                                let home_dir = crate::utils::get_home_dir();
                                let relative_path = full_path
                                    .strip_prefix(&home_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| full_path.to_string_lossy().to_string());

                                // Close browser first
                                state.file_browser_mode = false;
                                state.adding_custom_file = false;
                                state.file_browser_path_input.clear();
                                state.file_browser_path_cursor = 0;
                                state.focus = DotfileSelectionFocus::FilesList;

                                // Store relative_path before releasing borrow
                                let relative_path_clone = relative_path.clone();

                                // Release borrow
                                let _ = state;

                                // Re-scan to include the new file
                                self.scan_dotfiles()?;

                                // Find the file index and sync it
                                let file_index = {
                                    let state = &self.ui_state.dotfile_selection;
                                    state.dotfiles.iter().position(|d| {
                                        d.relative_path.to_string_lossy() == relative_path_clone
                                    })
                                };

                                if let Some(index) = file_index {
                                    // Sync the file immediately
                                    let _ = self.add_file_to_sync(index);
                                    // Select the file
                                    let state = &mut self.ui_state.dotfile_selection;
                                    state.dotfile_list_state.select(Some(index));
                                }
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Tab => {
                    // Tab in input field - switch to next focus area (handled by Action::NextTab in main handler)
                    // For now, allow Tab to unfocus input and go to list
                    state.file_browser_path_focused = false;
                    state.focus = DotfileSelectionFocus::FileBrowserList;
                    return Ok(());
                }
                _ => {}
            }
        }

        // Handle other file browser input
        match key_code {
            KeyCode::Char('q') | KeyCode::Esc => {
                state.adding_custom_file = false;
                state.file_browser_mode = false;
                state.file_browser_path_input.clear();
                state.file_browser_path_cursor = 0;
                state.file_browser_path_focused = false;
                state.focus = DotfileSelectionFocus::FilesList;
            }
            // Tab handling is now done via Action::NextTab in the main event handler
            // This section only handles non-keymap keys for file browser
            KeyCode::Up => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    state.file_browser_list_state.select_previous();
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview
                    && state.file_browser_preview_scroll > 0
                {
                    state.file_browser_preview_scroll =
                        state.file_browser_preview_scroll.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    state.file_browser_list_state.select_next();
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    state.file_browser_preview_scroll =
                        state.file_browser_preview_scroll.saturating_add(1);
                }
            }
            KeyCode::Char('u') => {
                if state.focus == DotfileSelectionFocus::FileBrowserPreview
                    && state.file_browser_preview_scroll > 0
                {
                    state.file_browser_preview_scroll =
                        state.file_browser_preview_scroll.saturating_sub(10);
                }
            }
            KeyCode::Char('d') => {
                if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    state.file_browser_preview_scroll =
                        state.file_browser_preview_scroll.saturating_add(10);
                }
            }
            KeyCode::PageUp => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    if let Some(current) = state.file_browser_list_state.selected() {
                        let new_index = current.saturating_sub(10);
                        state.file_browser_list_state.select(Some(new_index));
                    }
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview
                    && state.file_browser_preview_scroll > 0
                {
                    state.file_browser_preview_scroll =
                        state.file_browser_preview_scroll.saturating_sub(20);
                }
            }
            KeyCode::PageDown => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    if let Some(current) = state.file_browser_list_state.selected() {
                        let new_index =
                            (current + 10).min(state.file_browser_entries.len().saturating_sub(1));
                        state.file_browser_list_state.select(Some(new_index));
                    } else if !state.file_browser_entries.is_empty() {
                        state
                            .file_browser_list_state
                            .select(Some(10.min(state.file_browser_entries.len() - 1)));
                    }
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    state.file_browser_preview_scroll =
                        state.file_browser_preview_scroll.saturating_add(20);
                }
            }
            KeyCode::Enter => {
                // Enter loads the selected path into custom file input
                if let Some(selected_index) = state.file_browser_list_state.selected() {
                    if selected_index < state.file_browser_entries.len() {
                        let selected = &state.file_browser_entries[selected_index];

                        if selected == Path::new("..") {
                            // Go to parent directory
                            if let Some(parent) = state.file_browser_path.parent() {
                                let parent_path = parent.to_path_buf();
                                state.file_browser_path = parent_path.clone();
                                // Update path input to show the new directory
                                state.file_browser_path_input =
                                    state.file_browser_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor =
                                    state.file_browser_path_input.chars().count();
                                state.file_browser_list_state.select(Some(0));
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path =
                                    state.file_browser_path.clone();
                                self.refresh_file_browser()?;
                                return Ok(());
                            }
                        } else if selected == Path::new(".") {
                            // Add current folder
                            let current_folder = state.file_browser_path.clone();
                            let home_dir = crate::utils::get_home_dir();
                            let relative_path = current_folder
                                .strip_prefix(&home_dir)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| current_folder.to_string_lossy().to_string());

                            // Sanity checks
                            let repo_path = &self.config.repo_path;
                            let (is_safe, reason) =
                                crate::utils::is_safe_to_add(&current_folder, repo_path);
                            if !is_safe {
                                let state = &mut self.ui_state.dotfile_selection;
                                state.status_message = Some(format!(
                                    "Error: {}. Path: {}",
                                    reason.unwrap_or_else(|| "Cannot add this folder".to_string()),
                                    current_folder.display()
                                ));
                                return Ok(());
                            }

                            // Check if it's a git repo
                            if crate::utils::is_git_repo(&current_folder) {
                                let state = &mut self.ui_state.dotfile_selection;
                                state.status_message = Some(format!(
                                    "Error: Cannot sync a git repository. Path contains a .git directory: {}",
                                    current_folder.display()
                                ));
                                return Ok(());
                            }

                            // Show confirmation modal
                            let state = &mut self.ui_state.dotfile_selection;
                            state.show_custom_file_confirm = true;
                            state.custom_file_confirm_path = Some(current_folder.clone());
                            state.custom_file_confirm_relative = Some(relative_path.clone());
                            state.file_browser_mode = false;
                            state.adding_custom_file = false;
                            state.file_browser_path_input.clear();
                            state.file_browser_path_cursor = 0;
                            state.focus = DotfileSelectionFocus::FilesList;
                            return Ok(());
                        } else {
                            let full_path = if selected.is_absolute() {
                                selected.clone()
                            } else {
                                state.file_browser_path.join(selected)
                            };

                            if full_path.is_dir() {
                                // Enter directory
                                state.file_browser_path = full_path.clone();
                                state.file_browser_list_state.select(Some(0));
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path =
                                    state.file_browser_path.clone();
                                self.refresh_file_browser()?;
                                return Ok(());
                            } else if full_path.is_file() {
                                // It's a file - directly sync it
                                let home_dir = crate::utils::get_home_dir();
                                let relative_path = full_path
                                    .strip_prefix(&home_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| full_path.to_string_lossy().to_string());

                                // Close browser first
                                state.file_browser_mode = false;
                                state.adding_custom_file = false;
                                state.file_browser_path_input.clear();
                                state.file_browser_path_cursor = 0;
                                state.focus = DotfileSelectionFocus::FilesList;

                                // Store paths before releasing borrow
                                let relative_path_clone = relative_path.clone();
                                let full_path_clone = full_path.clone();

                                // Release borrow
                                let _ = state;

                                // Add the file directly to the dotfiles list and sync it
                                self.add_custom_file_to_sync(
                                    &full_path_clone,
                                    &relative_path_clone,
                                )?;

                                // Re-scan to refresh the list (will include the file if it's in default paths)
                                self.scan_dotfiles()?;

                                // Find and select the file in the list
                                let state = &mut self.ui_state.dotfile_selection;
                                if let Some(index) = state.dotfiles.iter().position(|d| {
                                    d.relative_path.to_string_lossy() == relative_path_clone
                                }) {
                                    state.dotfile_list_state.select(Some(index));
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Refresh file browser entries for current directory
    fn refresh_file_browser(&mut self) -> Result<()> {
        let state = &mut self.ui_state.dotfile_selection;
        let path = &state.file_browser_path;

        let mut entries = Vec::new();

        // Add parent directory if not at root
        if path != Path::new("/") && path.parent().is_some() {
            entries.push(PathBuf::from(".."));
        }

        // Add special marker for "add this folder" (only if it's a directory and safe to add)
        let repo_path = &self.config.repo_path;
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
    fn load_manifest(&self) -> Result<crate::utils::ProfileManifest> {
        crate::utils::ProfileManifest::load_or_backfill(&self.config.repo_path)
    }

    /// Helper: Save manifest to repo
    fn save_manifest(&self, manifest: &crate::utils::ProfileManifest) -> Result<()> {
        manifest.save(&self.config.repo_path)
    }

    /// Helper: Get profiles from manifest
    fn get_profiles(&self) -> Result<Vec<crate::utils::ProfileInfo>> {
        Ok(self.load_manifest()?.profiles)
    }

    /// Helper: Get active profile info from manifest
    fn get_active_profile_info(&self) -> Result<Option<crate::utils::ProfileInfo>> {
        let manifest = self.load_manifest()?;
        Ok(manifest
            .profiles
            .into_iter()
            .find(|p| p.name == self.config.active_profile))
    }

    /// Create a new profile
    fn create_profile(
        &mut self,
        name: &str,
        description: Option<String>,
        copy_from: Option<usize>,
    ) -> Result<()> {
        use crate::utils::{sanitize_profile_name, validate_profile_name};

        // Validate and sanitize profile name
        let sanitized_name = sanitize_profile_name(name);
        if sanitized_name.is_empty() {
            return Err(anyhow::anyhow!("Profile name cannot be empty"));
        }

        // Get existing profile names from manifest
        let mut manifest = self.load_manifest()?;
        let existing_names: Vec<String> =
            manifest.profiles.iter().map(|p| p.name.clone()).collect();
        if let Err(e) = validate_profile_name(&sanitized_name, &existing_names) {
            return Err(anyhow::anyhow!("Invalid profile name: {}", e));
        }

        // Check if profile folder exists but is not in manifest
        let profile_path = self.config.repo_path.join(&sanitized_name);
        let folder_exists = profile_path.exists();
        let profile_in_manifest = existing_names.contains(&sanitized_name);

        if folder_exists && !profile_in_manifest {
            // Folder exists but profile not in manifest - this is a warning case
            // We'll handle this by asking user (but for now, we'll allow override)
            // In the future, this could be a confirmation dialog
            warn!("Profile folder '{}' already exists but is not in manifest. Will use existing folder.", sanitized_name);
        } else if folder_exists && profile_in_manifest {
            // Both exist - this is a duplicate name error
            return Err(anyhow::anyhow!(
                "Profile '{}' already exists in manifest",
                sanitized_name
            ));
        }

        // Create folder if it doesn't exist
        if !folder_exists {
            std::fs::create_dir_all(&profile_path).context("Failed to create profile directory")?;
        }

        // Copy files from source profile if specified
        let synced_files = if let Some(source_idx) = copy_from {
            if let Some(source_profile) = manifest.profiles.get(source_idx) {
                let source_profile_path = self.config.repo_path.join(&source_profile.name);

                // Copy all files from source profile
                for file in &source_profile.synced_files {
                    let source_file = source_profile_path.join(file);
                    let dest_file = profile_path.join(file);

                    if source_file.exists() {
                        // Create parent directories
                        if let Some(parent) = dest_file.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        // Copy file or directory
                        if source_file.is_dir() {
                            crate::file_manager::copy_dir_all(&source_file, &dest_file)?;
                        } else {
                            std::fs::copy(&source_file, &dest_file)?;
                        }
                    }
                }

                source_profile.synced_files.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Add profile to manifest with synced_files
        manifest.add_profile(sanitized_name.clone(), description);
        // Update synced_files for the newly added profile
        manifest.update_synced_files(&sanitized_name, synced_files)?;
        self.save_manifest(&manifest)?;

        info!("Created profile: {}", sanitized_name);
        Ok(())
    }

    /// Switch to a different profile
    fn switch_profile(&mut self, target_profile_name: &str) -> Result<()> {
        use crate::utils::SymlinkManager;

        // Get target profile from manifest
        let manifest = self.load_manifest()?;
        let target_profile = manifest
            .profiles
            .iter()
            .find(|p| p.name == target_profile_name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", target_profile_name))?;

        // Don't switch if already active
        if self.config.active_profile == target_profile_name {
            return Ok(());
        }

        let old_profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();

        // Use SymlinkManager to switch profiles
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(repo_path.clone(), self.config.backup_enabled)?;

        let switch_result = symlink_mgr.switch_profile(
            &old_profile_name,
            target_profile_name,
            &target_profile.synced_files,
        )?;

        // Update active profile in config
        self.config.active_profile = target_profile_name.to_string();
        self.config.save(&self.config_path)?;

        info!(
            "Switched from '{}' to '{}'",
            old_profile_name, target_profile_name
        );
        info!(
            "Removed {} symlinks, created {} symlinks",
            switch_result.removed.len(),
            switch_result.created.len()
        );

        // Phase 6: Check packages after profile switch
        if !target_profile.packages.is_empty() {
            info!(
                "Profile '{}' has {} packages, checking installation status",
                target_profile_name,
                target_profile.packages.len()
            );
            // Initialize package checking state
            let state = &mut self.ui_state.package_manager;
            state.packages = target_profile.packages.clone();
            state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
            state.is_checking = true;
            state.checking_index = None;
            state.checking_delay_until =
                Some(std::time::Instant::now() + Duration::from_millis(100));
        }

        Ok(())
    }

    /// Rename a profile
    fn rename_profile(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        use crate::utils::{sanitize_profile_name, validate_profile_name};

        // Validate new name
        let sanitized_name = sanitize_profile_name(new_name);
        if sanitized_name.is_empty() {
            return Err(anyhow::anyhow!("Profile name cannot be empty"));
        }

        // Get existing profile names from manifest
        let mut manifest = self.load_manifest()?;
        let existing_names: Vec<String> = manifest
            .profiles
            .iter()
            .filter(|p| p.name != old_name)
            .map(|p| p.name.clone())
            .collect();
        if let Err(e) = validate_profile_name(&sanitized_name, &existing_names) {
            return Err(anyhow::anyhow!("Invalid profile name: {}", e));
        }

        // Check if profile exists in manifest
        if !manifest.has_profile(old_name) {
            return Err(anyhow::anyhow!("Profile '{}' not found", old_name));
        }

        // Clone values we need before borrowing
        let repo_path = self.config.repo_path.clone();
        let was_active = self.config.active_profile == old_name;

        // Rename profile folder in repo
        let old_path = repo_path.join(old_name);
        let new_path = repo_path.join(&sanitized_name);

        if old_path.exists() {
            std::fs::rename(&old_path, &new_path).context("Failed to rename profile directory")?;
        }

        // Update active profile name if this was the active profile
        if was_active {
            self.config.active_profile = sanitized_name.clone();
            self.config.save(&self.config_path)?;
        }

        // Update profile manifest
        manifest.rename_profile(old_name, &sanitized_name)?;
        self.save_manifest(&manifest)?;

        // Update symlinks if profile is active (has symlinks)
        if self.config.profile_activated && was_active {
            use crate::utils::SymlinkManager;
            let mut symlink_mgr =
                SymlinkManager::new_with_backup(repo_path.clone(), self.config.backup_enabled)?;

            match symlink_mgr.rename_profile(old_name, &sanitized_name) {
                Ok(ops) => {
                    let success_count = ops
                        .iter()
                        .filter(|op| {
                            op.status == crate::utils::symlink_manager::OperationStatus::Success
                        })
                        .count();
                    info!("Updated {} symlinks for renamed profile", success_count);
                }
                Err(e) => {
                    error!("Failed to update symlinks after rename: {}", e);
                    // Don't fail the rename, but log the error
                }
            }
        }

        info!(
            "Renamed profile from '{}' to '{}'",
            old_name, sanitized_name
        );
        Ok(())
    }

    /// Delete a profile
    fn delete_profile(&mut self, profile_name: &str) -> Result<()> {
        // Cannot delete active profile
        if self.config.active_profile == profile_name {
            return Err(anyhow::anyhow!(
                "Cannot delete active profile '{}'. Please switch to another profile first.",
                profile_name
            ));
        }

        // Remove profile folder from repo
        let profile_path = self.config.repo_path.join(profile_name);
        if profile_path.exists() {
            std::fs::remove_dir_all(&profile_path).context("Failed to remove profile directory")?;
        }

        // Remove from manifest
        let mut manifest = self.load_manifest()?;
        if !manifest.remove_profile(profile_name) {
            return Err(anyhow::anyhow!("Profile '{}' not found", profile_name));
        }
        self.save_manifest(&manifest)?;

        info!("Deleted profile: {}", profile_name);
        Ok(())
    }

    /// Activate a profile after GitHub setup (includes syncing files from repo)
    fn activate_profile_after_setup(&mut self, profile_name: &str) -> Result<()> {
        use crate::utils::SymlinkManager;

        info!("Activating profile '{}' after setup", profile_name);

        // Set as active profile
        self.config.active_profile = profile_name.to_string();
        self.config.save(&self.config_path)?;

        // Get profile to activate from manifest
        let profile = self
            .get_profiles()?
            .into_iter()
            .find(|p| p.name == profile_name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_name))?;

        // Get files to sync from the profile
        let files_to_sync = profile.synced_files.clone();

        if files_to_sync.is_empty() {
            info!("Profile '{}' has no files to sync", profile_name);
            // Still mark as activated even if no files
            self.config.profile_activated = true;
            self.config.save(&self.config_path)?;
            return Ok(());
        }

        // Create SymlinkManager with backup enabled (from config)
        let mut symlink_mgr = SymlinkManager::new_with_backup(
            self.config.repo_path.clone(),
            self.config.backup_enabled,
        )?;

        // Activate profile (this will create symlinks and sync files)
        match symlink_mgr.activate_profile(profile_name, &files_to_sync) {
            Ok(operations) => {
                let success_count = operations
                    .iter()
                    .filter(|op| {
                        matches!(
                            op.status,
                            crate::utils::symlink_manager::OperationStatus::Success
                        )
                    })
                    .count();
                info!(
                    "Activated profile '{}' with {} files",
                    profile_name, success_count
                );

                // Mark as activated
                self.config.profile_activated = true;
                self.config.save(&self.config_path)?;

                // Phase 6: Check packages after activation
                if !profile.packages.is_empty() {
                    info!(
                        "Profile '{}' has {} packages, checking installation status",
                        profile_name,
                        profile.packages.len()
                    );
                    // Initialize package checking state
                    let state = &mut self.ui_state.package_manager;
                    state.packages = profile.packages.clone();
                    state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                    state.is_checking = true;
                    state.checking_index = None;
                    state.checking_delay_until =
                        Some(std::time::Instant::now() + Duration::from_millis(100));
                }

                Ok(())
            }
            Err(e) => {
                error!("Failed to activate profile '{}': {}", profile_name, e);
                Err(anyhow::anyhow!("Failed to activate profile: {}", e))
            }
        }
    }

    /// Start adding a new package
    fn start_add_package(&mut self) -> Result<()> {
        info!("Starting add package dialog");
        let state = &mut self.ui_state.package_manager;
        use crate::utils::package_manager::PackageManagerImpl;

        state.popup_type = PackagePopupType::Add;
        state.add_editing_index = None;
        state.add_name_input.clear();
        state.add_name_cursor = 0;
        state.add_description_input.clear();
        state.add_description_cursor = 0;
        state.add_package_name_input.clear();
        state.add_package_name_cursor = 0;
        state.add_binary_name_input.clear();
        state.add_binary_name_cursor = 0;
        state.add_install_command_input.clear();
        state.add_install_command_cursor = 0;
        state.add_existence_check_input.clear();
        state.add_existence_check_cursor = 0;
        state.add_manager_check_input.clear();
        state.add_manager_check_cursor = 0;
        state.add_focused_field = AddPackageField::Name;
        state.add_is_custom = false;

        // Initialize available managers
        state.available_managers = PackageManagerImpl::get_available_managers();
        info!("Available package managers: {:?}", state.available_managers);
        if !state.available_managers.is_empty() {
            state.add_manager = Some(state.available_managers[0].clone());
            state.add_manager_selected = 0;
            state.manager_list_state.select(Some(0));
            state.add_is_custom = matches!(state.available_managers[0], PackageManager::Custom);
            debug!(
                "Default manager selected: {:?}",
                state.available_managers[0]
            );
        } else {
            warn!("No package managers available");
        }

        Ok(())
    }

    /// Start editing an existing package
    fn start_edit_package(&mut self, index: usize) -> Result<()> {
        info!("Starting edit package dialog for index: {}", index);
        let state = &mut self.ui_state.package_manager;
        use crate::utils::package_manager::PackageManagerImpl;

        if let Some(package) = state.packages.get(index) {
            debug!(
                "Editing package: {} (manager: {:?})",
                package.name, package.manager
            );
            state.popup_type = PackagePopupType::Edit;
            state.add_editing_index = Some(index);
            state.add_name_input = package.name.clone();
            state.add_name_cursor = package.name.chars().count();
            state.add_description_input = package.description.clone().unwrap_or_default();
            state.add_description_cursor = state.add_description_input.chars().count();
            state.add_package_name_input = package.package_name.clone().unwrap_or_default();
            state.add_package_name_cursor = state.add_package_name_input.chars().count();
            state.add_binary_name_input = package.binary_name.clone();
            state.add_binary_name_cursor = package.binary_name.chars().count();
            state.add_install_command_input = package.install_command.clone().unwrap_or_default();
            state.add_install_command_cursor = state.add_install_command_input.chars().count();
            state.add_existence_check_input = package.existence_check.clone().unwrap_or_default();
            state.add_existence_check_cursor = state.add_existence_check_input.chars().count();
            state.add_manager_check_input = package.manager_check.clone().unwrap_or_default();
            state.add_manager_check_cursor = state.add_manager_check_input.chars().count();
            state.add_manager = Some(package.manager.clone());
            state.add_is_custom = matches!(package.manager, PackageManager::Custom);
            state.add_focused_field = AddPackageField::Name;

            // Initialize available managers
            state.available_managers = PackageManagerImpl::get_available_managers();
            // Find current manager in list
            if let Some(pos) = state
                .available_managers
                .iter()
                .position(|m| *m == package.manager)
            {
                state.add_manager_selected = pos;
                state.manager_list_state.select(Some(pos));
            } else {
                state.add_manager_selected = 0;
                state.manager_list_state.select(Some(0));
            }
        }

        Ok(())
    }

    /// Process one package check step (non-blocking, called from main event loop)
    fn process_package_check_step(&mut self) -> Result<()> {
        let state = &mut self.ui_state.package_manager;

        if state.packages.is_empty() {
            debug!("Package check: No packages to check");
            state.is_checking = false;
            return Ok(());
        }

        // Initialize statuses if needed
        if state.package_statuses.len() != state.packages.len() {
            state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
        }

        // If we have a specific index to check (from "Check Selected"), check only that one
        if let Some(index) = state.checking_index {
            if index < state.packages.len() {
                let package = &state.packages[index];
                info!(
                    "Checking selected package: {} (index: {})",
                    package.name, index
                );

                // Check if package exists (binary check + fallback)
                use crate::utils::package_installer::PackageInstaller;
                use crate::utils::package_manager::PackageManagerImpl;

                match PackageInstaller::check_exists(package) {
                    Ok((true, used_fallback)) => {
                        debug!(
                            "Package {} found (used_fallback: {})",
                            package.name, used_fallback
                        );
                        state.package_statuses[index] = PackageStatus::Installed;
                        info!("Package {} is installed", package.name);
                    }
                    Ok((false, _)) => {
                        // Package not found - check if manager is installed for installation purposes
                        if !PackageManagerImpl::is_manager_installed(&package.manager) {
                            warn!(
                                "Package {} not found and manager {:?} is not installed",
                                package.name, package.manager
                            );
                            state.package_statuses[index] = PackageStatus::Error(format!(
                                "Package not found and package manager '{:?}' is not installed",
                                package.manager
                            ));
                        } else {
                            debug!(
                                "Package {} not found (manager {:?} is available)",
                                package.name, package.manager
                            );
                            state.package_statuses[index] = PackageStatus::NotInstalled;
                            info!("Package {} is not installed", package.name);
                        }
                    }
                    Err(e) => {
                        error!("Error checking package {}: {}", package.name, e);
                        state.package_statuses[index] = PackageStatus::Error(e.to_string());
                    }
                }

                // Done checking selected package
                state.checking_index = None;
                state.is_checking = false;
                state.checking_delay_until = None;
                return Ok(());
            } else {
                warn!(
                    "Package check: Index {} out of bounds ({} packages)",
                    index,
                    state.packages.len()
                );
            }
        }

        // Find next unchecked package (for "Check All")
        let next_index = state
            .package_statuses
            .iter()
            .position(|s| matches!(s, PackageStatus::Unknown));

        if let Some(index) = next_index {
            state.checking_index = Some(index);
            let package = &state.packages[index];
            debug!(
                "Checking package {} (index: {}) - Check All mode",
                package.name, index
            );

            // Check if package exists (binary check + fallback)
            // This is a blocking call, but we'll add a delay after it
            use crate::utils::package_installer::PackageInstaller;
            use crate::utils::package_manager::PackageManagerImpl;

            match PackageInstaller::check_exists(package) {
                Ok((true, used_fallback)) => {
                    debug!(
                        "Package {} found (used_fallback: {})",
                        package.name, used_fallback
                    );
                    state.package_statuses[index] = PackageStatus::Installed;
                }
                Ok((false, _)) => {
                    // Package not found - check if manager is installed for installation purposes
                    if !PackageManagerImpl::is_manager_installed(&package.manager) {
                        warn!(
                            "Package {} not found and manager {:?} is not installed",
                            package.name, package.manager
                        );
                        state.package_statuses[index] = PackageStatus::Error(format!(
                            "Package not found and package manager '{:?}' is not installed",
                            package.manager
                        ));
                    } else {
                        debug!(
                            "Package {} not found (manager {:?} is available)",
                            package.name, package.manager
                        );
                        state.package_statuses[index] = PackageStatus::NotInstalled;
                    }
                }
                Err(e) => {
                    error!("Error checking package {}: {}", package.name, e);
                    state.package_statuses[index] = PackageStatus::Error(e.to_string());
                }
            }

            state.checking_index = None;

            // Add a small delay before checking next package (allows UI to update)
            state.checking_delay_until =
                Some(std::time::Instant::now() + Duration::from_millis(100));
        } else {
            // All packages checked
            let installed_count = state
                .package_statuses
                .iter()
                .filter(|s| matches!(s, PackageStatus::Installed))
                .count();
            let missing_count = state
                .package_statuses
                .iter()
                .filter(|s| matches!(s, PackageStatus::NotInstalled))
                .count();
            let error_count = state
                .package_statuses
                .iter()
                .filter(|s| matches!(s, PackageStatus::Error(_)))
                .count();

            info!(
                "Package check complete: {} installed, {} missing, {} errors",
                installed_count, missing_count, error_count
            );

            state.is_checking = false;
            state.checking_delay_until = None;

            // Check if any packages are missing and show prompt
            if missing_count > 0 {
                info!(
                    "{} package(s) need installation, showing install prompt",
                    missing_count
                );
                state.popup_type = PackagePopupType::InstallMissing;
            }
        }

        Ok(())
    }

    /// Handle popup events for package manager (text input and cursor movement only)
    /// Tab/Esc/Enter are handled inline in the main event handler
    fn handle_package_popup_event(&mut self, event: Event) -> Result<()> {
        let state = &mut self.ui_state.package_manager;
        use crate::utils::package_manager::PackageManagerImpl;
        use crate::utils::text_input::{
            handle_backspace, handle_char_insertion, handle_cursor_movement, handle_delete,
        };
        use crossterm::event::{KeyCode, KeyEventKind};

        match state.popup_type {
            PackagePopupType::Add | PackagePopupType::Edit => {
                match event {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        match key.code {
                            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                                // Handle cursor movement in focused field
                                match state.add_focused_field {
                                    AddPackageField::Name => {
                                        handle_cursor_movement(
                                            &state.add_name_input,
                                            &mut state.add_name_cursor,
                                            key.code,
                                        );
                                    }
                                    AddPackageField::Description => {
                                        handle_cursor_movement(
                                            &state.add_description_input,
                                            &mut state.add_description_cursor,
                                            key.code,
                                        );
                                    }
                                    AddPackageField::PackageName => {
                                        handle_cursor_movement(
                                            &state.add_package_name_input,
                                            &mut state.add_package_name_cursor,
                                            key.code,
                                        );
                                    }
                                    AddPackageField::BinaryName => {
                                        handle_cursor_movement(
                                            &state.add_binary_name_input,
                                            &mut state.add_binary_name_cursor,
                                            key.code,
                                        );
                                    }
                                    AddPackageField::InstallCommand => {
                                        handle_cursor_movement(
                                            &state.add_install_command_input,
                                            &mut state.add_install_command_cursor,
                                            key.code,
                                        );
                                    }
                                    AddPackageField::ExistenceCheck => {
                                        handle_cursor_movement(
                                            &state.add_existence_check_input,
                                            &mut state.add_existence_check_cursor,
                                            key.code,
                                        );
                                    }
                                    AddPackageField::ManagerCheck => {
                                        // ManagerCheck is not shown in UI, but exists in enum
                                    }
                                    AddPackageField::Manager => {
                                        // Manager selection handled by Up/Down
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                match state.add_focused_field {
                                    AddPackageField::Name => {
                                        handle_backspace(
                                            &mut state.add_name_input,
                                            &mut state.add_name_cursor,
                                        );
                                    }
                                    AddPackageField::Description => {
                                        handle_backspace(
                                            &mut state.add_description_input,
                                            &mut state.add_description_cursor,
                                        );
                                    }
                                    AddPackageField::PackageName => {
                                        let old_package_name = state.add_package_name_input.clone();
                                        handle_backspace(
                                            &mut state.add_package_name_input,
                                            &mut state.add_package_name_cursor,
                                        );
                                        // Update binary name suggestion when package name is edited
                                        let new_suggestion =
                                            PackageManagerImpl::suggest_binary_name(
                                                &state.add_package_name_input,
                                            );
                                        if state.add_binary_name_input.is_empty()
                                            || state.add_binary_name_input
                                                == PackageManagerImpl::suggest_binary_name(
                                                    &old_package_name,
                                                )
                                        {
                                            state.add_binary_name_input = new_suggestion;
                                            state.add_binary_name_cursor =
                                                state.add_binary_name_input.chars().count();
                                        }
                                    }
                                    AddPackageField::BinaryName => {
                                        handle_backspace(
                                            &mut state.add_binary_name_input,
                                            &mut state.add_binary_name_cursor,
                                        );
                                    }
                                    AddPackageField::InstallCommand => {
                                        handle_backspace(
                                            &mut state.add_install_command_input,
                                            &mut state.add_install_command_cursor,
                                        );
                                    }
                                    AddPackageField::ExistenceCheck => {
                                        handle_backspace(
                                            &mut state.add_existence_check_input,
                                            &mut state.add_existence_check_cursor,
                                        );
                                    }
                                    AddPackageField::ManagerCheck => {
                                        // ManagerCheck is not shown in UI, but exists in enum
                                    }
                                    AddPackageField::Manager => {}
                                }
                            }
                            KeyCode::Delete => {
                                match state.add_focused_field {
                                    AddPackageField::Name => {
                                        handle_delete(
                                            &mut state.add_name_input,
                                            &mut state.add_name_cursor,
                                        );
                                    }
                                    AddPackageField::Description => {
                                        handle_delete(
                                            &mut state.add_description_input,
                                            &mut state.add_description_cursor,
                                        );
                                    }
                                    AddPackageField::PackageName => {
                                        handle_delete(
                                            &mut state.add_package_name_input,
                                            &mut state.add_package_name_cursor,
                                        );
                                    }
                                    AddPackageField::BinaryName => {
                                        handle_delete(
                                            &mut state.add_binary_name_input,
                                            &mut state.add_binary_name_cursor,
                                        );
                                    }
                                    AddPackageField::InstallCommand => {
                                        handle_delete(
                                            &mut state.add_install_command_input,
                                            &mut state.add_install_command_cursor,
                                        );
                                    }
                                    AddPackageField::ExistenceCheck => {
                                        handle_delete(
                                            &mut state.add_existence_check_input,
                                            &mut state.add_existence_check_cursor,
                                        );
                                    }
                                    AddPackageField::ManagerCheck => {
                                        // ManagerCheck is not shown in UI, but exists in enum
                                    }
                                    AddPackageField::Manager => {}
                                }
                            }
                            KeyCode::Char(c) => {
                                match state.add_focused_field {
                                    AddPackageField::Name => {
                                        handle_char_insertion(
                                            &mut state.add_name_input,
                                            &mut state.add_name_cursor,
                                            c,
                                        );
                                    }
                                    AddPackageField::Description => {
                                        handle_char_insertion(
                                            &mut state.add_description_input,
                                            &mut state.add_description_cursor,
                                            c,
                                        );
                                    }
                                    AddPackageField::PackageName => {
                                        handle_char_insertion(
                                            &mut state.add_package_name_input,
                                            &mut state.add_package_name_cursor,
                                            c,
                                        );
                                        // Auto-suggest binary name only if it's empty or matches the previous suggestion
                                        // This allows it to update as user types, but stops if they manually edit it
                                        let current_suggestion =
                                            PackageManagerImpl::suggest_binary_name(
                                                &state.add_package_name_input,
                                            );
                                        if state.add_binary_name_input.is_empty()
                                            || state.add_binary_name_input
                                                == PackageManagerImpl::suggest_binary_name(
                                                    &state
                                                        .add_package_name_input
                                                        .chars()
                                                        .take(
                                                            state
                                                                .add_package_name_input
                                                                .chars()
                                                                .count()
                                                                .saturating_sub(1),
                                                        )
                                                        .collect::<String>(),
                                                )
                                        {
                                            state.add_binary_name_input = current_suggestion;
                                            state.add_binary_name_cursor =
                                                state.add_binary_name_input.chars().count();
                                        }
                                    }
                                    AddPackageField::BinaryName => {
                                        handle_char_insertion(
                                            &mut state.add_binary_name_input,
                                            &mut state.add_binary_name_cursor,
                                            c,
                                        );
                                    }
                                    AddPackageField::InstallCommand => {
                                        handle_char_insertion(
                                            &mut state.add_install_command_input,
                                            &mut state.add_install_command_cursor,
                                            c,
                                        );
                                    }
                                    AddPackageField::ExistenceCheck => {
                                        handle_char_insertion(
                                            &mut state.add_existence_check_input,
                                            &mut state.add_existence_check_cursor,
                                            c,
                                        );
                                    }
                                    AddPackageField::ManagerCheck => {
                                        // ManagerCheck is not shown in UI, but exists in enum
                                    }
                                    AddPackageField::Manager => {}
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            PackagePopupType::Delete => match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => match key.code {
                    KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                        handle_cursor_movement(
                            &state.delete_confirm_input,
                            &mut state.delete_confirm_cursor,
                            key.code,
                        );
                    }
                    KeyCode::Backspace => {
                        handle_backspace(
                            &mut state.delete_confirm_input,
                            &mut state.delete_confirm_cursor,
                        );
                    }
                    KeyCode::Delete => {
                        handle_delete(
                            &mut state.delete_confirm_input,
                            &mut state.delete_confirm_cursor,
                        );
                    }
                    KeyCode::Char(c) => {
                        handle_char_insertion(
                            &mut state.delete_confirm_input,
                            &mut state.delete_confirm_cursor,
                            c,
                        );
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }

        Ok(())
    }

    /// Validate and save package
    fn validate_and_save_package(&mut self) -> Result<bool> {
        // Clone data from state before calling methods that need immutable access
        let (
            name,
            description,
            package_name,
            binary_name,
            install_command,
            existence_check,
            manager_check,
            manager,
            is_custom,
            edit_idx,
            active_profile_name,
        ) = {
            let state = &self.ui_state.package_manager;
            (
                state.add_name_input.clone(),
                state.add_description_input.clone(),
                state.add_package_name_input.clone(),
                state.add_binary_name_input.clone(),
                state.add_install_command_input.clone(),
                state.add_existence_check_input.clone(),
                state.add_manager_check_input.clone(),
                state.add_manager.clone(),
                state.add_is_custom,
                state.add_editing_index,
                self.config.active_profile.clone(),
            )
        };

        if let Some(idx) = edit_idx {
            info!(
                "Validating and saving edited package: {} (index: {})",
                name, idx
            );
        } else {
            info!(
                "Validating and saving new package: {} (manager: {:?}, custom: {})",
                name, manager, is_custom
            );
        }

        // Validate required fields
        if name.trim().is_empty() {
            warn!("Package validation failed: name is empty");
            return Ok(false); // Name is required
        }

        if binary_name.trim().is_empty() {
            warn!("Package validation failed: binary_name is empty");
            return Ok(false); // Binary name is required
        }

        // Validate based on package type
        if is_custom {
            // Custom packages require install_command
            // existence_check is optional - if empty, we use binary name check
            if install_command.trim().is_empty() {
                warn!("Package validation failed: install_command is empty for custom package");
                return Ok(false);
            }
        } else {
            // Managed packages require package_name
            if package_name.trim().is_empty() {
                warn!("Package validation failed: package_name is empty for managed package");
                return Ok(false);
            }
        }

        // Get manager
        let manager = manager.ok_or_else(|| anyhow::anyhow!("Package manager not selected"))?;

        // Create package
        let package = Package {
            name: name.trim().to_string(),
            description: if description.trim().is_empty() {
                None
            } else {
                Some(description.trim().to_string())
            },
            manager: manager.clone(),
            package_name: if is_custom {
                None
            } else {
                Some(package_name.trim().to_string())
            },
            binary_name: binary_name.trim().to_string(),
            install_command: if is_custom {
                Some(install_command.trim().to_string())
            } else {
                None
            },
            existence_check: if is_custom {
                let trimmed = existence_check.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            } else {
                None
            },
            manager_check: if manager_check.trim().is_empty() {
                None
            } else {
                Some(manager_check.trim().to_string())
            },
        };

        // Save to manifest
        let manifest = self.load_manifest()?;

        if let Some(profile) = manifest
            .profiles
            .iter()
            .find(|p| p.name == active_profile_name)
        {
            let mut packages = profile.packages.clone();

            if let Some(edit_idx) = edit_idx {
                // Edit existing package
                if edit_idx < packages.len() {
                    let old_name = packages[edit_idx].name.clone();
                    info!(
                        "Updating package: {} -> {} (profile: {})",
                        old_name, package.name, active_profile_name
                    );
                    packages[edit_idx] = package;
                } else {
                    warn!(
                        "Edit index {} out of bounds ({} packages)",
                        edit_idx,
                        packages.len()
                    );
                }
            } else {
                // Add new package
                info!(
                    "Adding new package: {} (profile: {})",
                    package.name, active_profile_name
                );
                packages.push(package);
            }

            // Update manifest
            let mut updated_manifest = manifest;
            if let Some(profile) = updated_manifest
                .profiles
                .iter_mut()
                .find(|p| p.name == active_profile_name)
            {
                profile.packages = packages;
            }
            self.save_manifest(&updated_manifest)?;

            // Update state
            if let Ok(Some(active_profile)) = self.get_active_profile_info() {
                let state = &mut self.ui_state.package_manager;
                state.packages = active_profile.packages.clone();
                state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                if !state.packages.is_empty() {
                    // Select the newly added/edited package
                    let select_idx = if let Some(edit_idx) = edit_idx {
                        edit_idx.min(state.packages.len().saturating_sub(1))
                    } else {
                        state.packages.len().saturating_sub(1)
                    };
                    state.list_state.select(Some(select_idx));
                }
            }

            Ok(true)
        } else {
            Err(anyhow::anyhow!(
                "Active profile '{}' not found",
                active_profile_name
            ))
        }
    }

    /// Delete a package
    fn delete_package(&mut self, index: usize) -> Result<()> {
        let active_profile_name = self.config.active_profile.clone();
        let manifest = self.load_manifest()?;

        if let Some(profile) = manifest
            .profiles
            .iter()
            .find(|p| p.name == *active_profile_name)
        {
            let mut packages = profile.packages.clone();

            if index < packages.len() {
                let package_name = packages[index].name.clone();
                info!(
                    "Deleting package: {} (index: {}, profile: {})",
                    package_name, index, active_profile_name
                );
                packages.remove(index);
            } else {
                warn!(
                    "Delete package: Index {} out of bounds ({} packages)",
                    index,
                    packages.len()
                );
            }

            // Update manifest
            let mut updated_manifest = manifest;
            if let Some(profile) = updated_manifest
                .profiles
                .iter_mut()
                .find(|p| p.name == *active_profile_name)
            {
                profile.packages = packages;
            }
            self.save_manifest(&updated_manifest)?;

            // Update state
            if let Ok(Some(active_profile)) = self.get_active_profile_info() {
                let state = &mut self.ui_state.package_manager;
                state.packages = active_profile.packages.clone();
                state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
                if !state.packages.is_empty() {
                    let new_idx = index.min(state.packages.len().saturating_sub(1));
                    state.list_state.select(Some(new_idx));
                } else {
                    state.list_state.select(None);
                }
            }
        }

        Ok(())
    }

    /// Process one installation step (non-blocking, called from main event loop)
    fn process_installation_step(&mut self) -> Result<()> {
        use crate::utils::package_manager::PackageManagerImpl;

        let state = &mut self.ui_state.package_manager;

        match &mut state.installation_step {
            InstallationStep::NotStarted => {
                // Nothing to do
                trace!("process_installation_step: NotStarted");
            }
            InstallationStep::Installing {
                package_index,
                package_name,
                total_packages: _,
                packages_to_install,
                installed,
                failed,
                status_rx,
            } => {
                trace!("process_installation_step: Installing package_index={}, package_name={}, packages_remaining={}",
                    package_index, package_name, packages_to_install.len());

                // Check if we need to wait for a delay
                if let Some(delay_until) = state.installation_delay_until {
                    if std::time::Instant::now() < delay_until {
                        // Still waiting, don't process yet
                        trace!("process_installation_step: Still waiting for delay");
                        return Ok(());
                    }
                    // Delay complete, clear it
                    trace!("process_installation_step: Delay complete, clearing");
                    state.installation_delay_until = None;
                }

                // Get the package being installed
                if let Some(package) = state.packages.get(*package_index) {
                    info!(
                        "process_installation_step: Processing package: {} (manager: {:?})",
                        package.name, package.manager
                    );

                    // Check if manager is installed
                    if !PackageManagerImpl::is_manager_installed(&package.manager) {
                        warn!(
                            "process_installation_step: Package manager '{:?}' is not installed",
                            package.manager
                        );
                        let error_msg = format!(
                            "Package manager '{:?}' is not installed. {}",
                            package.manager,
                            PackageManagerImpl::installation_instructions(&package.manager)
                        );
                        failed.push((*package_index, error_msg));

                        // Move to next package
                        if let Some(&next_idx) = packages_to_install.first() {
                            *package_index = next_idx;
                            packages_to_install.remove(0);
                            *package_name = state.packages[next_idx].name.clone();
                            state.installation_output.clear();
                            state.installation_delay_until =
                                Some(std::time::Instant::now() + Duration::from_millis(100));
                        } else {
                            // All packages processed
                            let installed_clone = installed.clone();
                            let failed_clone = failed.clone();
                            state.installation_step = InstallationStep::Complete {
                                installed: installed_clone,
                                failed: failed_clone,
                            };
                        }
                        return Ok(());
                    }

                    // Check sudo requirement
                    if PackageManagerImpl::check_sudo_required(&package.manager) {
                        warn!(
                            "process_installation_step: Sudo password required for package {}",
                            package.name
                        );
                        let error_msg = "sudo password required. Please run this in a terminal or configure passwordless sudo.".to_string();
                        failed.push((*package_index, error_msg));

                        // Move to next package
                        if let Some(&next_idx) = packages_to_install.first() {
                            *package_index = next_idx;
                            packages_to_install.remove(0);
                            *package_name = state.packages[next_idx].name.clone();
                            state.installation_output.clear();
                            state.installation_delay_until =
                                Some(std::time::Instant::now() + Duration::from_millis(100));
                        } else {
                            // All packages processed
                            let installed_clone = installed.clone();
                            let failed_clone = failed.clone();
                            state.installation_step = InstallationStep::Complete {
                                installed: installed_clone,
                                failed: failed_clone,
                            };
                        }
                        return Ok(());
                    }

                    // Start installation (non-blocking using background thread)
                    use crate::ui::InstallationStatus;
                    use std::sync::mpsc;
                    use std::thread;

                    // Check if we already started this installation
                    if status_rx.is_none() {
                        info!("process_installation_step: Starting installation thread for package: {}", package.name);
                        // Start the installation process in a background thread
                        let package_clone = package.clone();
                        let package_name_for_log = package.name.clone();
                        let (tx, rx) = mpsc::channel();

                        // Spawn thread to run installation with real-time output streaming
                        thread::spawn(move || {
                            use std::io::{BufRead, BufReader};
                            use std::process::Stdio;

                            info!(
                                "Installation thread: Starting installation for package: {}",
                                package_name_for_log
                            );
                            let mut cmd =
                                PackageManagerImpl::get_install_command_builder(&package_clone);

                            // Set up stdout and stderr as piped for streaming
                            cmd.stdout(Stdio::piped());
                            cmd.stderr(Stdio::piped());

                            debug!("Installation thread: Command built, spawning process...");
                            match cmd.spawn() {
                                Ok(mut child) => {
                                    // Spawn thread to read stdout in real-time
                                    let stdout =
                                        child.stdout.take().expect("Failed to capture stdout");
                                    let tx_stdout = tx.clone();
                                    thread::spawn(move || {
                                        let reader = BufReader::new(stdout);
                                        #[allow(
                                            clippy::unnecessary_lazy_evaluations,
                                            clippy::lines_filter_map_ok
                                        )]
                                        for line in reader.lines().flatten() {
                                            if !line.trim().is_empty()
                                                && tx_stdout
                                                    .send(InstallationStatus::Output(line))
                                                    .is_err()
                                            {
                                                // Channel closed, stop reading
                                                break;
                                            }
                                        }
                                    });

                                    // Spawn thread to read stderr in real-time
                                    let stderr =
                                        child.stderr.take().expect("Failed to capture stderr");
                                    let tx_stderr = tx.clone();
                                    thread::spawn(move || {
                                        let reader = BufReader::new(stderr);
                                        #[allow(
                                            clippy::unnecessary_lazy_evaluations,
                                            clippy::lines_filter_map_ok
                                        )]
                                        for line in reader.lines().flatten() {
                                            if !line.trim().is_empty()
                                                && tx_stderr
                                                    .send(InstallationStatus::Output(format!(
                                                        "[stderr] {}",
                                                        line
                                                    )))
                                                    .is_err()
                                            {
                                                // Channel closed, stop reading
                                                break;
                                            }
                                        }
                                    });

                                    // Wait for process to complete
                                    match child.wait() {
                                        Ok(status) => {
                                            info!("Installation thread: Command executed, exit code: {:?}", status.code());
                                            // Send completion status
                                            if status.success() {
                                                info!("Installation thread: Installation succeeded for {}", package_name_for_log);
                                                let _ = tx.send(InstallationStatus::Complete {
                                                    success: true,
                                                    error: None,
                                                });
                                            } else {
                                                let error_msg = format!(
                                                    "Installation failed with exit code: {}",
                                                    status.code().unwrap_or(-1)
                                                );
                                                error!("Installation thread: Installation failed for {}: {}", package_name_for_log, error_msg);
                                                let _ = tx.send(InstallationStatus::Complete {
                                                    success: false,
                                                    error: Some(error_msg),
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            error!("Installation thread: Failed to wait for process for {}: {}", package_name_for_log, e);
                                            let _ = tx.send(InstallationStatus::Complete {
                                                success: false,
                                                error: Some(format!(
                                                    "Failed to wait for installation: {}",
                                                    e
                                                )),
                                            });
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(
                                        "Installation thread: Failed to spawn command for {}: {}",
                                        package_name_for_log, e
                                    );
                                    let _ = tx.send(InstallationStatus::Complete {
                                        success: false,
                                        error: Some(format!(
                                            "Failed to execute installation: {}",
                                            e
                                        )),
                                    });
                                }
                            }
                        });

                        *status_rx = Some(rx);
                        info!("process_installation_step: Installation thread spawned, channel receiver stored");
                    } else {
                        trace!("process_installation_step: Installation already started, checking for updates");
                    }

                    // Read available status updates (non-blocking)
                    if let Some(ref rx) = status_rx {
                        // Try to read all available updates
                        while let Ok(status) = rx.try_recv() {
                            match status {
                                InstallationStatus::Output(line) => {
                                    // Regular output line
                                    trace!(
                                        "process_installation_step: Received output line: {}",
                                        line
                                    );
                                    state.installation_output.push(line);
                                }
                                InstallationStatus::Complete { success, error } => {
                                    info!("process_installation_step: Received completion status: success={}, error={:?}", success, error);
                                    if success {
                                        installed.push(*package_index);
                                    } else {
                                        failed.push((
                                            *package_index,
                                            error.unwrap_or_else(|| "Unknown error".to_string()),
                                        ));
                                    }

                                    // Move to next package
                                    if let Some(&next_idx) = packages_to_install.first() {
                                        *package_index = next_idx;
                                        packages_to_install.remove(0);
                                        *package_name = state.packages[next_idx].name.clone();
                                        state.installation_output.clear();
                                        *status_rx = None;
                                        state.installation_delay_until = Some(
                                            std::time::Instant::now() + Duration::from_millis(100),
                                        );
                                    } else {
                                        // All packages processed
                                        let installed_clone = installed.clone();
                                        let failed_clone = failed.clone();
                                        state.installation_step = InstallationStep::Complete {
                                            installed: installed_clone,
                                            failed: failed_clone,
                                        };
                                    }
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }
            InstallationStep::Complete { installed, failed } => {
                // Installation complete, do nothing
                trace!(
                    "process_installation_step: Complete - installed: {}, failed: {}",
                    installed.len(),
                    failed.len()
                );
            }
        }

        Ok(())
    }
}
