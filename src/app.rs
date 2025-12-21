use anyhow::{Context, Result};
use crate::config::{Config, GitHubConfig};
use crate::file_manager::FileManager;
use crate::github::GitHubClient;
use crate::git::GitManager;
use crate::tui::Tui;
use crate::ui::{UiState, Screen, GitHubAuthStep, GitHubAuthField, ProfileSelectionState};
use crate::components::{MainMenuComponent, GitHubAuthComponent, SyncedFilesComponent, MessageComponent, DotfileSelectionComponent, PushChangesComponent, ProfileManagerComponent, ComponentAction, Component, MenuItem};
use crate::components::profile_manager::ProfilePopupType;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::runtime::Runtime;
use tracing::{error, info};
use ratatui::prelude::*;

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
    message_component: Option<MessageComponent>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config_path = crate::utils::get_config_path();

        let config = Config::load_or_create(&config_path)?;
        let file_manager = FileManager::new()?;
        let tui = Tui::new()?;
        let ui_state = UiState::new();
        let runtime = Runtime::new().context("Failed to create tokio runtime")?;

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
            message_component: None,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.tui.enter()?;

        // Check if profile is deactivated and show warning
        if !self.config.profile_activated && self.config.github.is_some() {
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

        // Main event loop
        loop {
            self.draw()?;

            if self.should_quit {
                break;
            }

            // Poll for events with 250ms timeout
            if let Some(event) = self.tui.poll_event(Duration::from_millis(250))? {
                self.handle_event(event)?;
            }
        }

        self.tui.exit()?;
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        // Check for screen transitions and update state accordingly
        let current_screen = self.ui_state.current_screen;
        if self.last_screen != Some(current_screen) {
            // Screen changed - check for changes when entering MainMenu
            if current_screen == Screen::MainMenu {
                self.check_changes_to_push();
            }
            self.last_screen = Some(current_screen);
        }

        // Update components with current state
        if self.ui_state.current_screen == Screen::MainMenu {
            self.main_menu_component.set_has_changes_to_push(self.ui_state.has_changes_to_push);
            self.main_menu_component.set_selected(self.ui_state.selected_index);
            // Update changed files for status display
            self.main_menu_component.update_changed_files(self.ui_state.push_changes.changed_files.clone());
        }

        // Update GitHub auth component state
        if self.ui_state.current_screen == Screen::GitHubAuth {
            *self.github_auth_component.get_auth_state_mut() = self.ui_state.github_auth.clone();
        }

        // DotfileSelectionComponent just handles Clear widget, state stays in ui_state

        // Update synced files component config (only if on that screen to avoid unnecessary clones)
        if self.ui_state.current_screen == Screen::ViewSyncedFiles {
            self.synced_files_component.update_config(self.config.clone());
        }

        // Load changed files when entering PushChanges screen
        if self.ui_state.current_screen == Screen::PushChanges && !self.ui_state.push_changes.is_pushing {
            // Only load if we don't have files yet
            if self.ui_state.push_changes.changed_files.is_empty() {
                self.load_changed_files();
            }
        }

        // Create/update message component if needed (for PullChanges only now)
        if self.ui_state.current_screen == Screen::PullChanges {
            let title = "Pull Changes";
            let message = self.ui_state.dotfile_selection.status_message
                .as_deref()
                .unwrap_or("Processing...")
                .to_string();
            self.message_component = Some(MessageComponent::new(
                title.to_string(),
                message,
                self.ui_state.current_screen,
            ));
        }

        self.tui.terminal_mut().draw(|frame| {
            let area = frame.size();
            match self.ui_state.current_screen {
                Screen::Welcome => {
                    // Welcome screen removed - redirect to MainMenu
                    self.ui_state.current_screen = Screen::MainMenu;
                    self.main_menu_component.update_config(self.config.clone());
                    let _ = self.main_menu_component.render(frame, area);
                }
                Screen::MainMenu => {
                    // Show deactivation warning message if present
                    if let Some(ref mut msg_component) = self.message_component {
                        let _ = msg_component.render(frame, area);
                    } else {
                        // Pass config to main menu for stats
                        self.main_menu_component.update_config(self.config.clone());
                        let _ = self.main_menu_component.render(frame, area);
                    }
                }
                Screen::GitHubAuth => {
                    // Sync state back after render (component may update it)
                    let _ = self.github_auth_component.render(frame, area);
                    self.ui_state.github_auth = self.github_auth_component.get_auth_state().clone();
                }
                Screen::DotfileSelection => {
                    // Component handles all rendering including Clear
                    if let Err(e) = self.dotfile_selection_component.render_with_state(frame, area, &mut self.ui_state) {
                        eprintln!("Error rendering dotfile selection: {}", e);
                    }
                }
                Screen::ViewSyncedFiles => {
                    let _ = self.synced_files_component.render(frame, area);
                }
                Screen::PushChanges => {
                    // Component handles all rendering including Clear
                    if let Err(e) = self.push_changes_component.render_with_state(frame, area, &mut self.ui_state.push_changes) {
                        eprintln!("Error rendering push changes: {}", e);
                    }
                }
                Screen::PullChanges => {
                    if let Some(ref mut msg_component) = self.message_component {
                        let _ = msg_component.render(frame, area);
                    }
                }
                Screen::ManageProfiles => {
                    // Component handles all rendering including Clear
                    if let Err(e) = self.profile_manager_component.render_with_config(frame, area, &self.config, &mut self.ui_state.profile_manager) {
                        eprintln!("Error rendering profile manager: {}", e);
                    }
                }
                Screen::ProfileSelection => {
                    // Render profile selection screen
                    // Clone config profiles before closure to avoid borrow issues
                    let config_profiles = self.config.profiles.clone();
                    let state = &mut self.ui_state.profile_selection;

                    // Build items list
                    let items: Vec<ListItem> = state.profiles.iter()
                        .map(|name| {
                            let profile = config_profiles.iter().find(|p| p.name == *name);
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

                    // Render the screen inline
                    use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
                    use crate::components::header::Header;
                    use crate::components::footer::Footer;
                    use crate::utils::create_standard_layout;
                    use ratatui::style::{Style, Color, Modifier};

                    frame.render_widget(Clear, area);

                    let background = Block::default()
                        .style(Style::default().bg(Color::Black));
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
                        .highlight_symbol("> ");

                    frame.render_stateful_widget(list, content_chunk, &mut state.list_state);

                    let footer_text = "↑↓: Navigate | Enter: Activate Profile | Esc: Skip";
                    let _ = Footer::render(frame, footer_chunk, footer_text);
                }
            }
        })?;
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        // Handle message component events first (e.g., deactivation warning on MainMenu)
        if let Some(ref mut msg_component) = self.message_component {
            if self.ui_state.current_screen == Screen::MainMenu {
                let action = msg_component.handle_event(event)?;
                match action {
                    ComponentAction::Navigate(Screen::MainMenu) => {
                        // User dismissed the warning, clear it and show main menu
                        self.message_component = None;
                    }
                    _ => {}
                }
                return Ok(());
            }
        }

        // Let components handle events first (for mouse support)
        match self.ui_state.current_screen {
            Screen::MainMenu => {
                // Check if Enter was pressed before moving event
                let is_enter = matches!(event, Event::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Enter);

                let action = self.main_menu_component.handle_event(event)?;
                match action {
                    ComponentAction::Update => {
                        // Update selected index from component
                        self.ui_state.selected_index = self.main_menu_component.selected_index();
                        // Handle Enter key for menu selection
                        if is_enter {
                            self.handle_menu_selection()?;
                        }
                    }
                    ComponentAction::Quit => {
                        self.should_quit = true;
                    }
                    _ => {}
                }
                return Ok(());
            }
            Screen::GitHubAuth => {
                // Let component handle mouse events, but keyboard events go to app
                if matches!(event, Event::Mouse(_)) {
                    let action = self.github_auth_component.handle_event(event)?;
                    if action == ComponentAction::Update {
                        // Sync state back
                        self.ui_state.github_auth = self.github_auth_component.get_auth_state().clone();
                    }
                    return Ok(());
                }
                // Keyboard events handled in app (complex logic)
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        self.handle_github_auth_input(key)?;
                        // Sync state to component
                        *self.github_auth_component.get_auth_state_mut() = self.ui_state.github_auth.clone();
                    }
                }
                return Ok(());
            }
            Screen::ViewSyncedFiles => {
                let action = self.synced_files_component.handle_event(event)?;
                match action {
                    ComponentAction::Navigate(Screen::MainMenu) => {
                        self.ui_state.current_screen = Screen::MainMenu;
                    }
                    _ => {}
                }
                return Ok(());
            }
            Screen::PushChanges => {
                // Handle push changes events
                if let Event::Key(key) = event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Enter => {
                                // Start pushing if not already pushing and we have changes
                                if !self.ui_state.push_changes.is_pushing
                                    && !self.ui_state.push_changes.changed_files.is_empty() {
                                    self.start_push()?;
                                }
                            }
                            KeyCode::Char('q') | KeyCode::Esc => {
                                // Close result popup or go back
                                if self.ui_state.push_changes.show_result_popup {
                                    self.ui_state.push_changes.show_result_popup = false;
                                    self.ui_state.push_changes.push_result = None;
                                    // Re-check for changes
                                    self.check_changes_to_push();
                                } else {
                                    self.ui_state.current_screen = Screen::MainMenu;
                                    // Reset push state
                                    self.ui_state.push_changes = crate::ui::PushChangesState::default();
                                }
                            }
                            KeyCode::Up => {
                                self.ui_state.push_changes.list_state.select_previous();
                            }
                            KeyCode::Down => {
                                self.ui_state.push_changes.list_state.select_next();
                            }
                            KeyCode::PageUp => {
                                if let Some(current) = self.ui_state.push_changes.list_state.selected() {
                                    let new_index = current.saturating_sub(10);
                                    self.ui_state.push_changes.list_state.select(Some(new_index));
                                }
                            }
                            KeyCode::PageDown => {
                                if let Some(current) = self.ui_state.push_changes.list_state.selected() {
                                    let new_index = (current + 10).min(self.ui_state.push_changes.changed_files.len().saturating_sub(1));
                                    self.ui_state.push_changes.list_state.select(Some(new_index));
                                }
                            }
                            KeyCode::Home => {
                                self.ui_state.push_changes.list_state.select_first();
                            }
                            KeyCode::End => {
                                self.ui_state.push_changes.list_state.select_last();
                            }
                            _ => {}
                        }
                    }
                } else if let Event::Mouse(mouse) = event {
                    // Handle mouse events for list navigation
                    if let MouseEventKind::ScrollUp = mouse.kind {
                        self.ui_state.push_changes.list_state.select_previous();
                    } else if let MouseEventKind::ScrollDown = mouse.kind {
                        self.ui_state.push_changes.list_state.select_next();
                    } else if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                        // Click to push or close popup
                        if self.ui_state.push_changes.show_result_popup {
                            self.ui_state.push_changes.show_result_popup = false;
                            self.ui_state.push_changes.push_result = None;
                            self.check_changes_to_push();
                        } else if !self.ui_state.push_changes.is_pushing
                            && !self.ui_state.push_changes.changed_files.is_empty() {
                            self.start_push()?;
                        }
                    }
                }
                return Ok(());
            }
            Screen::PullChanges => {
                if let Some(ref mut msg_component) = self.message_component {
                    let action = msg_component.handle_event(event)?;
                    match action {
                        ComponentAction::Navigate(Screen::MainMenu) => {
                            self.ui_state.current_screen = Screen::MainMenu;
                            self.ui_state.dotfile_selection.status_message = None;
                            self.message_component = None;
                            // Re-check for changes after push/pull
                            self.check_changes_to_push();
                        }
                        _ => {}
                    }
                }
                return Ok(());
            }
            Screen::ManageProfiles => {
                let profiles = &self.config.profiles;
                let state = &mut self.ui_state.profile_manager;

                // Handle popup events first
                if state.popup_type != ProfilePopupType::None {
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
                                                state.create_focused_field = match state.create_focused_field {
                                                    CreateField::Name => CreateField::CopyFrom,
                                                    CreateField::Description => CreateField::Name,
                                                    CreateField::CopyFrom => CreateField::Description,
                                                };
                                            } else {
                                                // Tab: go to next field
                                                state.create_focused_field = match state.create_focused_field {
                                                    CreateField::Name => CreateField::Description,
                                                    CreateField::Description => CreateField::CopyFrom,
                                                    CreateField::CopyFrom => CreateField::Name,
                                                };
                                            }
                                        }
                                        KeyCode::BackTab => {
                                            // Shift+Tab: go to previous field
                                            state.create_focused_field = match state.create_focused_field {
                                                CreateField::Name => CreateField::CopyFrom,
                                                CreateField::Description => CreateField::Name,
                                                CreateField::CopyFrom => CreateField::Description,
                                            };
                                        }
                                        KeyCode::Up => {
                                            // Navigate Copy From list (index 0 = "Start Blank", 1+ = profiles)
                                            if state.create_focused_field == CreateField::CopyFrom {
                                                // Convert to UI index: None = 0, Some(idx) = idx + 1
                                                let ui_current = if let Some(idx) = state.create_copy_from {
                                                    idx + 1
                                                } else {
                                                    0
                                                };

                                                if ui_current > 0 {
                                                    // Move up: if at profile, go to previous profile or "Start Blank"
                                                    if ui_current == 1 {
                                                        state.create_copy_from = None; // Go to "Start Blank"
                                                    } else {
                                                        state.create_copy_from = Some(ui_current - 2); // Previous profile
                                                    }
                                                } else {
                                                    // At "Start Blank", wrap to last profile
                                                    if !profiles.is_empty() {
                                                        state.create_copy_from = Some(profiles.len() - 1);
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Down => {
                                            // Navigate Copy From list (index 0 = "Start Blank", 1+ = profiles)
                                            if state.create_focused_field == CreateField::CopyFrom {
                                                // Convert to UI index: None = 0, Some(idx) = idx + 1
                                                let ui_current = if let Some(idx) = state.create_copy_from {
                                                    idx + 1
                                                } else {
                                                    0
                                                };

                                                let max_ui_idx = profiles.len(); // Last UI index (profiles.len() because "Start Blank" is at 0)

                                                if ui_current < max_ui_idx {
                                                    // Move down: if at "Start Blank", go to first profile, otherwise next profile
                                                    if ui_current == 0 {
                                                        state.create_copy_from = Some(0); // First profile
                                                    } else {
                                                        state.create_copy_from = Some(ui_current); // Next profile
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
                                                let ui_current = if let Some(idx) = state.create_copy_from {
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
                                                        state.create_copy_from = None; // Deselect, go to "Start Blank"
                                                    } else {
                                                        state.create_copy_from = Some(profile_idx); // Select this profile
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
                                                let ui_current = if let Some(idx) = state.create_copy_from {
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
                                                let description = if state.create_description_input.is_empty() {
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
                                                match self.create_profile(&name_clone, description_clone, copy_from) {
                                                    Ok(_) => {
                                                        // Refresh config
                                                        self.config = Config::load_or_create(&self.config_path)?;
                                                        self.ui_state.profile_manager.popup_type = ProfilePopupType::None;
                                                        self.ui_state.profile_manager.create_name_input.clear();
                                                        self.ui_state.profile_manager.create_description_input.clear();
                                                        self.ui_state.profile_manager.create_focused_field = CreateField::Name;
                                                        // Refresh list
                                                        if !self.config.profiles.is_empty() {
                                                            let new_idx = self.config.profiles.iter()
                                                                .position(|p| p.name == name)
                                                                .unwrap_or(self.config.profiles.len() - 1);
                                                            self.ui_state.profile_manager.list_state.select(Some(new_idx));
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!("Failed to create profile: {}", e);
                                                        // TODO: Show error message in UI
                                                    }
                                                }
                                                return Ok(());
                                            }
                                        }
                                        KeyCode::Backspace => {
                                            match state.create_focused_field {
                                                CreateField::Name => {
                                                    if !state.create_name_input.is_empty() {
                                                        crate::utils::text_input::handle_backspace(&mut state.create_name_input, &mut state.create_name_cursor);
                                                    }
                                                }
                                                CreateField::Description => {
                                                    if !state.create_description_input.is_empty() {
                                                        crate::utils::text_input::handle_backspace(&mut state.create_description_input, &mut state.create_description_cursor);
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
                                                        crate::utils::text_input::handle_delete(&mut state.create_name_input, &mut state.create_name_cursor);
                                                    }
                                                }
                                                CreateField::Description => {
                                                    if !state.create_description_input.is_empty() {
                                                        crate::utils::text_input::handle_delete(&mut state.create_description_input, &mut state.create_description_cursor);
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
                                                    crate::utils::text_input::handle_char_insertion(&mut state.create_name_input, &mut state.create_name_cursor, c);
                                                }
                                                CreateField::Description => {
                                                    crate::utils::text_input::handle_char_insertion(&mut state.create_description_input, &mut state.create_description_cursor, c);
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
                                                            self.config = Config::load_or_create(&self.config_path)?;
                                                            self.ui_state.profile_manager.popup_type = ProfilePopupType::None;
                                                            // Update list selection
                                                            if !self.config.profiles.is_empty() {
                                                                let new_idx = self.config.profiles.iter()
                                                                    .position(|p| p.name == profile_name)
                                                                    .unwrap_or(0);
                                                                self.ui_state.profile_manager.list_state.select(Some(new_idx));
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!("Failed to switch profile: {}", e);
                                                            // Show error message in UI
                                                            self.ui_state.profile_manager.popup_type = ProfilePopupType::None;
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
                                                        match self.rename_profile(&old_name_clone, &new_name_clone) {
                                                            Ok(_) => {
                                                                // Refresh config
                                                                self.config = Config::load_or_create(&self.config_path)?;
                                                                self.ui_state.profile_manager.popup_type = ProfilePopupType::None;
                                                                // Update list selection
                                                                if !self.config.profiles.is_empty() {
                                                                    let new_idx = self.config.profiles.iter()
                                                                        .position(|p| p.name == new_name)
                                                                        .unwrap_or(0);
                                                                    self.ui_state.profile_manager.list_state.select(Some(new_idx));
                                                                }
                                                            }
                                                            Err(e) => {
                                                                error!("Failed to rename profile: {}", e);
                                                                // TODO: Show error message in UI
                                                            }
                                                        }
                                                        return Ok(());
                                                    }
                                                }
                                            }
                                        }
                                        KeyCode::Backspace => {
                                            if !state.rename_input.is_empty() {
                                                crate::utils::text_input::handle_backspace(&mut state.rename_input, &mut state.rename_cursor);
                                            }
                                        }
                                        KeyCode::Delete => {
                                            if !state.rename_input.is_empty() {
                                                crate::utils::text_input::handle_delete(&mut state.rename_input, &mut state.rename_cursor);
                                            }
                                        }
                                        KeyCode::Left => {
                                            crate::utils::text_input::handle_cursor_movement(&state.rename_input, &mut state.rename_cursor, KeyCode::Left);
                                        }
                                        KeyCode::Right => {
                                            crate::utils::text_input::handle_cursor_movement(&state.rename_input, &mut state.rename_cursor, KeyCode::Right);
                                        }
                                        KeyCode::Char(c) => {
                                            crate::utils::text_input::handle_char_insertion(&mut state.rename_input, &mut state.rename_cursor, c);
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
                                                        let profile_name_clone = profile_name.clone();
                                                        // Release borrows by ending scope
                                                        {
                                                            let _ = state;
                                                            let _ = profiles;
                                                        }
                                                        match self.delete_profile(&profile_name_clone) {
                                                            Ok(_) => {
                                                                // Refresh config
                                                                self.config = Config::load_or_create(&self.config_path)?;
                                                                self.ui_state.profile_manager.popup_type = ProfilePopupType::None;
                                                                // Update list selection
                                                                if !self.config.profiles.is_empty() {
                                                                    let new_idx = idx_clone.min(self.config.profiles.len().saturating_sub(1));
                                                                    self.ui_state.profile_manager.list_state.select(Some(new_idx));
                                                                } else {
                                                                    self.ui_state.profile_manager.list_state.select(None);
                                                                }
                                                            }
                                                            Err(e) => {
                                                                error!("Failed to delete profile: {}", e);
                                                                // Show error message in UI
                                                                self.ui_state.profile_manager.popup_type = ProfilePopupType::None;
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
                                                crate::utils::text_input::handle_backspace(&mut state.delete_confirm_input, &mut state.delete_confirm_cursor);
                                            }
                                        }
                                        KeyCode::Delete => {
                                            if !state.delete_confirm_input.is_empty() {
                                                crate::utils::text_input::handle_delete(&mut state.delete_confirm_input, &mut state.delete_confirm_cursor);
                                            }
                                        }
                                        KeyCode::Left => {
                                            crate::utils::text_input::handle_cursor_movement(&state.delete_confirm_input, &mut state.delete_confirm_cursor, KeyCode::Left);
                                        }
                                        KeyCode::Right => {
                                            crate::utils::text_input::handle_cursor_movement(&state.delete_confirm_input, &mut state.delete_confirm_cursor, KeyCode::Right);
                                        }
                                        KeyCode::Char(c) => {
                                            crate::utils::text_input::handle_char_insertion(&mut state.delete_confirm_input, &mut state.delete_confirm_cursor, c);
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
                        match key.code {
                            KeyCode::Up => {
                                if let Some(current) = state.list_state.selected() {
                                    if current > 0 {
                                        state.list_state.select(Some(current - 1));
                                    }
                                } else if !profiles.is_empty() {
                                    state.list_state.select(Some(profiles.len() - 1));
                                }
                            }
                            KeyCode::Down => {
                                if let Some(current) = state.list_state.selected() {
                                    if current < profiles.len().saturating_sub(1) {
                                        state.list_state.select(Some(current + 1));
                                    }
                                } else if !profiles.is_empty() {
                                    state.list_state.select(Some(0));
                                }
                            }
                            KeyCode::Enter => {
                                // Open switch popup (only if not already active)
                                if let Some(idx) = state.list_state.selected() {
                                    if let Some(profile) = profiles.get(idx) {
                                        if profile.name != self.config.active_profile {
                                            state.popup_type = ProfilePopupType::Switch;
                                        }
                                        // If already active, do nothing (no popup)
                                    }
                                }
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
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
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                // Open rename popup
                                if let Some(idx) = state.list_state.selected() {
                                    if let Some(profile) = profiles.get(idx) {
                                        state.popup_type = ProfilePopupType::Rename;
                                        state.rename_input = profile.name.clone();
                                        state.rename_cursor = state.rename_input.len();
                                    }
                                }
                            }
                            KeyCode::Char('d') | KeyCode::Char('D') => {
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
                            KeyCode::Esc => {
                                self.ui_state.current_screen = Screen::MainMenu;
                            }
                            _ => {}
                        }
                    }
                    Event::Mouse(mouse) => {
                        match mouse.kind {
                            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
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
                                            && mouse.row < name_area.y + name_area.height {
                                            state.create_focused_field = CreateField::Name;
                                            // Set cursor position based on click
                                            // Account for left border (1 char) - InputField has borders
                                            let inner_x = name_area.x + 1;
                                            let click_x = if mouse.column > inner_x {
                                                (mouse.column as usize).saturating_sub(inner_x as usize)
                                            } else {
                                                0
                                            };
                                            state.create_name_cursor = click_x.min(state.create_name_input.chars().count());
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
                                            && mouse.row < desc_area.y + desc_area.height {
                                            state.create_focused_field = CreateField::Description;
                                            // Set cursor position based on click
                                            // Account for left border (1 char) - InputField has borders
                                            let inner_x = desc_area.x + 1;
                                            let click_x = if mouse.column > inner_x {
                                                (mouse.column as usize).saturating_sub(inner_x as usize)
                                            } else {
                                                0
                                            };
                                            state.create_description_cursor = click_x.min(state.create_description_input.chars().count());
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
                                        && mouse.row < rect.y + rect.height {
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
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match self.ui_state.current_screen {
                    Screen::DotfileSelection => {
                        self.handle_dotfile_selection_input(key.code)?;
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) => {
                self.handle_mouse(mouse)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_menu_selection(&mut self) -> Result<()> {
        // Check for changes when returning to menu
        if self.ui_state.current_screen == Screen::MainMenu {
            self.check_changes_to_push();
        }

        // Get the selected menu item from the component
        let selected_item = self.main_menu_component.selected_item();

        match selected_item {
            MenuItem::SetupGitHub => {
                // Setup GitHub Repository
                // Check if repo is already configured
                let is_configured = self.config.github.is_some();

                // Initialize auth state with current config values
                if is_configured {
                    self.ui_state.github_auth.repo_already_configured = true;
                    self.ui_state.github_auth.is_editing_token = false;
                    self.ui_state.github_auth.token_input = String::new(); // Clear for security
                    // Load existing values
                    self.ui_state.github_auth.repo_name_input = self.config.repo_name.clone();
                    self.ui_state.github_auth.repo_location_input = self.config.repo_path.to_string_lossy().to_string();
                    self.ui_state.github_auth.is_private = true; // Default to private
                } else {
                    self.ui_state.github_auth.repo_already_configured = false;
                    self.ui_state.github_auth.is_editing_token = false;
                }

                self.ui_state.current_screen = Screen::GitHubAuth;
            }
            MenuItem::ScanDotfiles => {
                // Scan & Select Dotfiles
                self.scan_dotfiles()?;
                // Reset state when entering the page
                self.ui_state.dotfile_selection.show_unsaved_warning = false;
                self.ui_state.dotfile_selection.status_message = None;
                // Sync backup_enabled from config
                self.ui_state.dotfile_selection.backup_enabled = self.config.backup_enabled;
                self.ui_state.current_screen = Screen::DotfileSelection;
            }
            // MenuItem::ViewSyncedFiles => {
            //     // View Synced Files
            //     self.ui_state.current_screen = Screen::ViewSyncedFiles;
            // }
            MenuItem::PushChanges => {
                // Push Changes - just navigate, don't push yet
                self.ui_state.current_screen = Screen::PushChanges;
                // Reset push state
                self.ui_state.push_changes = crate::ui::PushChangesState::default();
            }
            MenuItem::PullChanges => {
                // Pull Changes
                self.ui_state.current_screen = Screen::PullChanges;
                self.pull_changes()?;
            }
            MenuItem::ManageProfiles => {
                // Manage Profiles
                self.ui_state.current_screen = Screen::ManageProfiles;
                // Initialize list state with first profile selected
                if !self.config.profiles.is_empty() {
                    self.ui_state.profile_manager.list_state.select(Some(0));
                }
            }
        }
        Ok(())
    }

    /// Check for changes to push and update UI state
    fn check_changes_to_push(&mut self) {
        self.ui_state.has_changes_to_push = false;
        self.ui_state.push_changes.changed_files.clear();

        // Check if GitHub is configured and repo exists
        if self.config.github.is_none() {
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
                self.ui_state.push_changes.changed_files = files;
                self.ui_state.has_changes_to_push = !self.ui_state.push_changes.changed_files.is_empty();
            }
            Err(_) => {
                // Fallback to old method if get_changed_files fails
                // Check for uncommitted changes
                let has_uncommitted = match git_mgr.has_uncommitted_changes() {
                    Ok(true) => true,
                    _ => false,
                };

                // Check for unpushed commits
                let branch = git_mgr.get_current_branch()
                    .unwrap_or_else(|| "main".to_string());
                let has_unpushed = match git_mgr.has_unpushed_commits("origin", &branch) {
                    Ok(true) => true,
                    _ => false,
                };

                self.ui_state.has_changes_to_push = has_uncommitted || has_unpushed;
            }
        }
    }

    fn handle_github_auth_input(&mut self, key: KeyEvent) -> Result<()> {
        let auth_state = &mut self.ui_state.github_auth;
        auth_state.error_message = None;

        match auth_state.step {
            GitHubAuthStep::Input => {
                // Handle "Update Token" action if repo is configured
                if auth_state.repo_already_configured && !auth_state.is_editing_token {
                    match key.code {
                        KeyCode::Char('u') | KeyCode::Char('U') => {
                            // Enable token editing
                            auth_state.is_editing_token = true;
                            auth_state.token_input = String::new(); // Clear token for new input
                            auth_state.cursor_position = 0;
                            auth_state.focused_field = GitHubAuthField::Token;
                            return Ok(());
                        }
                        KeyCode::Esc => {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
                            return Ok(());
                        }
                        _ => {
                            // Ignore other keys when repo is configured and not editing
                            return Ok(());
                        }
                    }
                }

                // Check for Ctrl+S
                if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if auth_state.repo_already_configured && auth_state.is_editing_token {
                        // Just update the token
                        self.update_github_token()?;
                    } else if !auth_state.repo_already_configured {
                        // Full setup
                        self.process_github_setup()?;
                    }
                    return Ok(());
                }

                // Normal input handling (new setup or token editing mode)
                match key.code {
                    // Tab: Navigate to next field (only if not repo configured)
                    KeyCode::Tab if !auth_state.repo_already_configured => {
                        auth_state.focused_field = match auth_state.focused_field {
                            GitHubAuthField::Token => GitHubAuthField::RepoName,
                            GitHubAuthField::RepoName => GitHubAuthField::RepoLocation,
                            GitHubAuthField::RepoLocation => GitHubAuthField::IsPrivate,
                            GitHubAuthField::IsPrivate => GitHubAuthField::Token,
                        };
                        // Reset cursor position to end of new field
                        auth_state.cursor_position = match auth_state.focused_field {
                            GitHubAuthField::Token => auth_state.token_input.chars().count(),
                            GitHubAuthField::RepoName => auth_state.repo_name_input.chars().count(),
                            GitHubAuthField::RepoLocation => auth_state.repo_location_input.chars().count(),
                            GitHubAuthField::IsPrivate => 0,
                        };
                    }
                    // BackTab: Navigate to previous field (Shift+Tab) (only if not repo configured)
                    KeyCode::BackTab if !auth_state.repo_already_configured => {
                        auth_state.focused_field = match auth_state.focused_field {
                            GitHubAuthField::Token => GitHubAuthField::IsPrivate,
                            GitHubAuthField::RepoName => GitHubAuthField::Token,
                            GitHubAuthField::RepoLocation => GitHubAuthField::RepoName,
                            GitHubAuthField::IsPrivate => GitHubAuthField::RepoLocation,
                        };
                        auth_state.cursor_position = match auth_state.focused_field {
                            GitHubAuthField::Token => auth_state.token_input.chars().count(),
                            GitHubAuthField::RepoName => auth_state.repo_name_input.chars().count(),
                            GitHubAuthField::RepoLocation => auth_state.repo_location_input.chars().count(),
                            GitHubAuthField::IsPrivate => 0,
                        };
                    }
                    KeyCode::Char(c) => {
                        // Handle Space for visibility toggle
                        if c == ' ' && auth_state.focused_field == GitHubAuthField::IsPrivate && !auth_state.repo_already_configured {
                            auth_state.is_private = !auth_state.is_private;
                        } else {
                            // Regular character input (only if not disabled)
                            match auth_state.focused_field {
                                GitHubAuthField::Token if !auth_state.repo_already_configured || auth_state.is_editing_token => {
                                    crate::utils::handle_char_insertion(
                                        &mut auth_state.token_input,
                                        &mut auth_state.cursor_position,
                                        c
                                    );
                                }
                                GitHubAuthField::RepoName if !auth_state.repo_already_configured => {
                                    crate::utils::handle_char_insertion(
                                        &mut auth_state.repo_name_input,
                                        &mut auth_state.cursor_position,
                                        c
                                    );
                                }
                                GitHubAuthField::RepoLocation if !auth_state.repo_already_configured => {
                                    crate::utils::handle_char_insertion(
                                        &mut auth_state.repo_location_input,
                                        &mut auth_state.cursor_position,
                                        c
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                    // Navigation within input
                    KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                        let current_input = match auth_state.focused_field {
                            GitHubAuthField::Token => &auth_state.token_input,
                            GitHubAuthField::RepoName => &auth_state.repo_name_input,
                            GitHubAuthField::RepoLocation => &auth_state.repo_location_input,
                            GitHubAuthField::IsPrivate => "",
                        };
                        crate::utils::handle_cursor_movement(
                            current_input,
                            &mut auth_state.cursor_position,
                            key.code
                        );
                    }
                    // Backspace
                    KeyCode::Backspace => {
                        match auth_state.focused_field {
                            GitHubAuthField::Token => {
                                crate::utils::handle_backspace(
                                    &mut auth_state.token_input,
                                    &mut auth_state.cursor_position
                                );
                            }
                            GitHubAuthField::RepoName => {
                                crate::utils::handle_backspace(
                                    &mut auth_state.repo_name_input,
                                    &mut auth_state.cursor_position
                                );
                            }
                            GitHubAuthField::RepoLocation => {
                                crate::utils::handle_backspace(
                                    &mut auth_state.repo_location_input,
                                    &mut auth_state.cursor_position
                                );
                            }
                            GitHubAuthField::IsPrivate => {}
                        }
                    }
                    // Delete
                    KeyCode::Delete => {
                        match auth_state.focused_field {
                            GitHubAuthField::Token => {
                                crate::utils::handle_delete(
                                    &mut auth_state.token_input,
                                    &mut auth_state.cursor_position
                                );
                            }
                            GitHubAuthField::RepoName => {
                                crate::utils::handle_delete(
                                    &mut auth_state.repo_name_input,
                                    &mut auth_state.cursor_position
                                );
                            }
                            GitHubAuthField::RepoLocation => {
                                crate::utils::handle_delete(
                                    &mut auth_state.repo_location_input,
                                    &mut auth_state.cursor_position
                                );
                            }
                            GitHubAuthField::IsPrivate => {}
                        }
                    }
                    KeyCode::Esc => {
                        self.ui_state.current_screen = Screen::MainMenu;
                        *auth_state = Default::default();
                    }
                    _ => {}
                }
            }
            GitHubAuthStep::Processing => {
                // Allow user to continue after processing completes
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        // If setup was successful, check if we need to select a profile
                        if auth_state.error_message.is_none() && auth_state.status_message.is_some() {
                            // Check if we have profiles to select from (repo was cloned)
                            if !self.ui_state.profile_selection.profiles.is_empty() &&
                               auth_state.status_message.as_ref().map(|s| s.contains("Found")).unwrap_or(false) {
                                // Go to profile selection screen
                                self.ui_state.current_screen = Screen::ProfileSelection;
                            } else {
                                // No profile selection needed, go to main menu
                                self.ui_state.current_screen = Screen::MainMenu;
                                *auth_state = Default::default();
                            }
                        }
                    }
                    KeyCode::Esc => {
                        // Reset and go back
                        self.ui_state.current_screen = Screen::MainMenu;
                        *auth_state = Default::default();
                    }
                    _ => {}
                }
            }
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
            auth_state.error_message = Some(
                "Token format error: GitHub tokens must start with 'ghp_'".to_string()
            );
            return Ok(());
        }

        if token.len() < 40 {
            auth_state.error_message = Some(
                format!("Token appears incomplete: {} characters (expected 40+)", token.len())
            );
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
                    auth_state.error_message = Some("GitHub configuration not found. Please complete setup first.".to_string());
                    auth_state.status_message = None;
                }
            }
            Ok(response) => {
                let status = response.status();
                auth_state.error_message = Some(
                    format!("Token validation failed: HTTP {}\nPlease check your token.", status)
                );
                auth_state.status_message = None;
            }
            Err(e) => {
                auth_state.error_message = Some(
                    format!("Network error: {}\nPlease check your internet connection.", e)
                );
                auth_state.status_message = None;
            }
        }

        Ok(())
    }

    fn process_github_setup(&mut self) -> Result<()> {
        let auth_state = &mut self.ui_state.github_auth;
        auth_state.step = GitHubAuthStep::Processing;
        auth_state.error_message = None;
        auth_state.status_message = Some("Verifying token...".to_string());

        // We'll process this in the event loop to avoid blocking
        // For now, let's do it synchronously with the runtime
        // Trim whitespace from token
        let token = auth_state.token_input.trim().to_string();
        let repo_name = self.config.repo_name.clone();

        // Token validation - do not log token content for security

        // Validate token format before making API call
        if !token.starts_with("ghp_") {
            let actual_start = if token.len() >= 4 { &token[..4] } else { "too short" };
            auth_state.error_message = Some(
                format!(
                    "Invalid token format: Must start with 'ghp_' but starts with '{}'.\n\
                    Token length: {} characters.\n\
                    First 10 chars: '{}'\n\
                    Please check that you copied the entire token correctly.\n\
                    Make sure you're pasting the full token (40+ characters).",
                    actual_start,
                    token.len(),
                    if token.len() >= 10 { &token[..10] } else { &token }
                )
            );
            auth_state.step = GitHubAuthStep::Input;
            return Ok(());
        }

        if token.len() < 40 {
            auth_state.error_message = Some(
                format!(
                    "Token appears incomplete: {} characters (expected 40+).\n\
                    First 10 chars: '{}'\n\
                    Make sure you copied the entire token from GitHub.",
                    token.len(),
                    &token[..token.len().min(10)]
                )
            );
            auth_state.step = GitHubAuthStep::Input;
            return Ok(());
        }

        // Use the runtime to run async code
        let result = self.runtime.block_on(async {
            // Verify token and get user
            let client = GitHubClient::new(token.clone());
            let user = client.get_user().await?;

            // Check if repo exists
            let repo_exists = client.repo_exists(&user.login, &repo_name).await?;

            Ok::<(String, bool), anyhow::Error>((user.login, repo_exists))
        });

        match result {
            Ok((username, exists)) => {
                let repo_path = self.config.repo_path.clone();

                if exists {
                    auth_state.status_message = Some(format!("Repository exists. Cloning {}/{}...", username, repo_name));

                    // Remove existing directory if it exists
                    if repo_path.exists() {
                        std::fs::remove_dir_all(&repo_path)
                            .context("Failed to remove existing directory")?;
                    }

                    // Clone existing repository using git2
                    let remote_url = format!("https://github.com/{}/{}.git", username, repo_name);
                    match GitManager::clone(&remote_url, &repo_path, Some(&token)) {
                        Ok(_) => {
                            auth_state.status_message = Some(format!("Repository cloned successfully to: {:?}", repo_path));
                        }
                        Err(e) => {
                            auth_state.error_message = Some(format!("Failed to clone repository: {}", e));
                            auth_state.step = GitHubAuthStep::Input;
                            return Ok(());
                        }
                    }
                } else {
                    auth_state.status_message = Some(format!("Creating repository: {}/{}", username, repo_name));

                    // Create repository
                    let create_result = self.runtime.block_on(async {
                        let client = GitHubClient::new(token.clone());
                        client.create_repo(&repo_name, "My dotfiles managed by dotstate", false).await
                    });

                    match create_result {
                        Ok(_) => {
                            auth_state.status_message = Some(format!("Repository created. Initializing local repo..."));

                            // Initialize local repository
                            std::fs::create_dir_all(&repo_path)
                                .context("Failed to create repository directory")?;

                            let mut git_mgr = GitManager::open_or_init(&repo_path)?;

                            // Add remote
                            let remote_url = format!("https://{}@github.com/{}/{}.git", token, username, repo_name);
                            // Add remote (this also sets up tracking)
                            git_mgr.add_remote("origin", &remote_url)?;

                            // Create initial commit
                            std::fs::write(repo_path.join("README.md"),
                                format!("# {}\n\nDotfiles managed by dotstate", repo_name))?;

                            // Create profile manifest with default profile
                            let manifest = crate::utils::ProfileManifest {
                                profiles: vec![crate::utils::ProfileInfo {
                                    name: self.config.active_profile.clone(),
                                    description: self.config.profiles.first()
                                        .and_then(|p| p.description.clone()),
                                }],
                            };
                            manifest.save(&repo_path)?;

                            git_mgr.commit_all("Initial commit")?;

                            // Get current branch name (should be 'main' after ensure_main_branch)
                            let current_branch = git_mgr.get_current_branch()
                                .unwrap_or_else(|| self.config.default_branch.clone());

                            // Push to remote using the actual branch name and set upstream
                            git_mgr.push("origin", &current_branch, Some(&token))?;

                            // Ensure tracking is set up after push
                            git_mgr.set_upstream_tracking("origin", &current_branch)?;

                            auth_state.status_message = Some(format!("Repository created and initialized successfully"));
                        }
                        Err(e) => {
                            auth_state.error_message = Some(format!("Failed to create repository: {}", e));
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
                self.config.save(&self.config_path)
                    .context("Failed to save configuration")?;

                // Verify config was saved
                if !self.config_path.exists() {
                    auth_state.error_message = Some("Warning: Config file was not created. Please check permissions.".to_string());
                    auth_state.step = GitHubAuthStep::Input;
                    return Ok(());
                }

                auth_state.status_message = Some(format!(
                    "GitHub setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\nConfig: {:?}\n\nPress Enter to continue.",
                    username, repo_name, repo_path, self.config_path
                ));
            }
            Err(e) => {
                // Show detailed error message
                let error_msg = format!("Authentication failed: {}", e);
                auth_state.error_message = Some(error_msg);
                auth_state.status_message = None;
                auth_state.step = GitHubAuthStep::Input;
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
                                    state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_sub(1);
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
                                state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_add(1);
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
                MouseEventKind::Down(button) if button == MouseButton::Left => {
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

                        if mouse.column >= popup_x && mouse.column < popup_x + popup_width &&
                           mouse.row >= popup_y && mouse.row < popup_y + popup_height {
                            let popup_inner_y = mouse.row.saturating_sub(popup_y);
                            let popup_inner_x = mouse.column.saturating_sub(popup_x);

                            // Layout: path display (1), path input (3), list+preview (min), footer (2)
                            if popup_inner_y < 1 {
                                // Clicked on path display - focus input
                                state.focus = DotfileSelectionFocus::FileBrowserInput;
                                state.file_browser_path_focused = true;
                            } else if popup_inner_y >= 1 && popup_inner_y < 4 {
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
                                            state.file_browser_list_state.select(Some(clicked_index));
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
                                let clicked_row = mouse.row.saturating_sub(content_start_y) as usize;
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
                if self.ui_state.current_screen == Screen::GitHubAuth {
                    match auth_state.step {
                        GitHubAuthStep::Input => {
                            // Check if click is in token input area (roughly row 4-6, column 2-78)
                            // This is approximate - we'd need to track exact widget positions for precision
                            // For now, clicking anywhere in the left half focuses token input
                            if mouse_x < terminal_size.width / 2 {
                                auth_state.input_focused = true;
                                // Move cursor to clicked position (approximate)
                                let relative_x = mouse_x.saturating_sub(2) as usize;
                                auth_state.cursor_position = relative_x.min(auth_state.token_input.chars().count());
                            } else {
                                // Click in help area - unfocus input
                                auth_state.input_focused = false;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Handle input for dotfile selection screen
    fn handle_dotfile_selection_input(&mut self, key_code: KeyCode) -> Result<()> {
        let state = &mut self.ui_state.dotfile_selection;

        // PRIORITY 1: Handle unsaved warning popup FIRST (blocks all other input)
        if state.show_unsaved_warning {
            match key_code {
                KeyCode::Esc => {
                    // Cancel - close popup and stay on page
                    state.show_unsaved_warning = false;
                    return Ok(());
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    // Save changes
                    state.show_unsaved_warning = false;
                    // Release borrow by ending scope
                    {
                        let _ = state;
                    }
                    self.sync_selected_files()?;
                    self.ui_state.current_screen = Screen::MainMenu;
                    return Ok(());
                }
                KeyCode::Char('d') | KeyCode::Char('D') => {
                    // Discard changes
                    state.show_unsaved_warning = false;
                    // Release borrow by ending scope
                    {
                        let _ = state;
                    }
                    // Reload selection from config to discard changes
                    self.scan_dotfiles()?;
                    self.ui_state.current_screen = Screen::MainMenu;
                    return Ok(());
                }
                _ => {
                    // Ignore all other keys when popup is showing
                    return Ok(());
                }
            }
        }

        // PRIORITY 2: Handle custom file input mode
        if state.adding_custom_file {
            if state.file_browser_mode {
                return self.handle_file_browser_input(key_code);
            } else {
                // Only allow Esc to exit, everything else goes to input handler
                if key_code == KeyCode::Esc && !state.custom_file_focused {
                    state.adding_custom_file = false;
                    state.custom_file_input.clear();
                    return Ok(());
                }
                return self.handle_custom_file_input(key_code);
            }
        }

        // PRIORITY 3: Normal dotfile selection input handling
        use crate::ui::DotfileSelectionFocus;
        match key_code {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Check for unsaved changes before leaving
                // Need to check before using state to avoid borrow conflicts
                let has_unsaved = {
                    let currently_selected: std::collections::HashSet<String> = state.selected_for_sync
                        .iter()
                        .filter_map(|&idx| {
                            state.dotfiles.get(idx)
                                .map(|d| d.relative_path.to_string_lossy().to_string())
                        })
                        .collect();

                    let previously_synced: std::collections::HashSet<String> = if let Some(profile) = self.config.get_active_profile() {
                        profile.synced_files.iter().cloned().collect()
                    } else {
                        std::collections::HashSet::new()
                    };

                    currently_selected != previously_synced
                };

                if has_unsaved {
                    state.show_unsaved_warning = true;
                } else {
                    self.ui_state.current_screen = Screen::MainMenu;
                }
            }
            KeyCode::Tab => {
                // Switch focus between FilesList and Preview
                state.focus = match state.focus {
                    DotfileSelectionFocus::FilesList => DotfileSelectionFocus::Preview,
                    DotfileSelectionFocus::Preview => DotfileSelectionFocus::FilesList,
                    _ => DotfileSelectionFocus::FilesList,
                };
            }
            KeyCode::Up => {
                // Only navigate files list if it's focused
                if state.focus == DotfileSelectionFocus::FilesList {
                    state.dotfile_list_state.select_previous();
                    // Reset preview scroll when changing selection
                    state.preview_scroll = 0;
                } else if state.focus == DotfileSelectionFocus::Preview {
                    // Scroll preview up
                    if state.preview_scroll > 0 {
                        state.preview_scroll = state.preview_scroll.saturating_sub(1);
                    }
                }
            }
            KeyCode::Down => {
                // Only navigate files list if it's focused
                if state.focus == DotfileSelectionFocus::FilesList {
                    state.dotfile_list_state.select_next();
                    // Reset preview scroll when changing selection
                    state.preview_scroll = 0;
                } else if state.focus == DotfileSelectionFocus::Preview {
                    // Scroll preview down
                    state.preview_scroll = state.preview_scroll.saturating_add(1);
                }
            }
            KeyCode::Enter if state.status_message.is_some() => {
                // Clear status message after sync summary
                state.status_message = None;
            }
            KeyCode::Enter => {
                // Toggle selection
                if let Some(selected_index) = state.dotfile_list_state.selected() {
                    if state.selected_for_sync.contains(&selected_index) {
                        state.selected_for_sync.remove(&selected_index);
                    } else {
                        state.selected_for_sync.insert(selected_index);
                    }
                }
            }
            KeyCode::PageUp => {
                // Jump up by 10 items in list (only if files list is focused)
                if state.focus == DotfileSelectionFocus::FilesList {
                    if let Some(current) = state.dotfile_list_state.selected() {
                        let new_index = current.saturating_sub(10);
                        state.dotfile_list_state.select(Some(new_index));
                        state.preview_scroll = 0;
                    }
                } else if state.focus == DotfileSelectionFocus::Preview {
                    // Scroll preview up by more
                    if state.preview_scroll > 0 {
                        state.preview_scroll = state.preview_scroll.saturating_sub(20);
                    }
                }
            }
            KeyCode::PageDown => {
                // Jump down by 10 items in list (only if files list is focused)
                if state.focus == DotfileSelectionFocus::FilesList {
                    if let Some(current) = state.dotfile_list_state.selected() {
                        let new_index = (current + 10).min(state.dotfiles.len().saturating_sub(1));
                        state.dotfile_list_state.select(Some(new_index));
                        state.preview_scroll = 0;
                    } else if !state.dotfiles.is_empty() {
                        state.dotfile_list_state.select(Some(10.min(state.dotfiles.len() - 1)));
                        state.preview_scroll = 0;
                    }
                } else if state.focus == DotfileSelectionFocus::Preview {
                    // Scroll preview down by more
                    state.preview_scroll = state.preview_scroll.saturating_add(20);
                }
            }
            KeyCode::Char('u') => {
                // Scroll preview up (only if preview is focused)
                if state.focus == DotfileSelectionFocus::Preview {
                    if state.preview_scroll > 0 {
                        state.preview_scroll = state.preview_scroll.saturating_sub(10);
                    }
                }
            }
            KeyCode::Char('d') => {
                // Scroll preview down (only if preview is focused)
                if state.focus == DotfileSelectionFocus::Preview {
                    state.preview_scroll = state.preview_scroll.saturating_add(10);
                }
            }
            KeyCode::Home => {
                // Go to first item (only if files list is focused)
                if state.focus == DotfileSelectionFocus::FilesList {
                    state.dotfile_list_state.select_first();
                    state.preview_scroll = 0;
                } else if state.focus == DotfileSelectionFocus::Preview {
                    // Scroll to top of preview
                    state.preview_scroll = 0;
                }
            }
            KeyCode::End => {
                // Go to last item (only if files list is focused)
                if state.focus == DotfileSelectionFocus::FilesList {
                    state.dotfile_list_state.select_last();
                    state.preview_scroll = 0;
                }
                // Note: No "scroll to bottom" for preview as we don't know the total length
            }
            KeyCode::Char('s') => {
                // Sync selected files
                self.sync_selected_files()?;
                return Ok(());
            }
            KeyCode::Char('a') => {
                // Add custom file - start with file browser
                state.adding_custom_file = true;
                state.file_browser_mode = true;
                state.file_browser_path = crate::utils::get_home_dir();
                state.file_browser_selected = 0;
                // Initialize path input with current directory
                state.file_browser_path_input = state.file_browser_path.to_string_lossy().to_string();
                state.file_browser_path_cursor = state.file_browser_path_input.chars().count();
                state.file_browser_path_focused = false;
                state.file_browser_preview_scroll = 0;
                state.focus = DotfileSelectionFocus::FileBrowserList; // Start with list focused
                self.refresh_file_browser()?;
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                // Toggle backup enabled
                state.backup_enabled = !state.backup_enabled;
                // Save to config
                self.config.backup_enabled = state.backup_enabled;
                self.config.save(&self.config_path)?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Handle input for adding custom files
    fn handle_custom_file_input(&mut self, key_code: KeyCode) -> Result<()> {
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
                crate::utils::handle_char_insertion(&mut state.custom_file_input, &mut state.custom_file_cursor, c);
                return Ok(());
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                crate::utils::handle_cursor_movement(&state.custom_file_input, &mut state.custom_file_cursor, key_code);
            }
            KeyCode::Backspace => {
                crate::utils::handle_backspace(&mut state.custom_file_input, &mut state.custom_file_cursor);
            }
            KeyCode::Delete => {
                crate::utils::handle_delete(&mut state.custom_file_input, &mut state.custom_file_cursor);
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
                        state.status_message = Some(format!("Error: File does not exist: {:?}", full_path));
                    } else {
                        // Calculate relative path
                        let home_dir = crate::utils::get_home_dir();
                        let relative_path = match full_path.strip_prefix(&home_dir) {
                            Ok(p) => p.to_string_lossy().to_string(),
                            Err(_) => path_str_clone.clone(),
                        };

                        // Store values we need before releasing borrow
                        let relative_path_clone = relative_path.clone();

                        // Re-scan to include the new file
                        self.scan_dotfiles()?;

                        // Re-borrow state to update UI
                        let state = &mut self.ui_state.dotfile_selection;
                        if let Some(index) = state.dotfiles.iter().position(|d| d.relative_path.to_string_lossy() == relative_path_clone) {
                            state.dotfile_list_state.select(Some(index));
                        }

                        state.adding_custom_file = false;
                        state.custom_file_input.clear();
                        state.status_message = Some(format!("✓ Added custom file: {}", relative_path_clone));
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
        let currently_selected: std::collections::HashSet<String> = state.selected_for_sync
            .iter()
            .filter_map(|&idx| {
                state.dotfiles.get(idx)
                    .map(|d| d.relative_path.to_string_lossy().to_string())
            })
            .collect();

        // Get previously synced files from active profile
        let previously_synced: std::collections::HashSet<String> = if let Some(profile) = self.config.get_active_profile() {
            profile.synced_files.iter().cloned().collect()
        } else {
            std::collections::HashSet::new()
        };

        // Check if they differ
        currently_selected != previously_synced
    }

    /// Scan for dotfiles and populate the selection state
    fn scan_dotfiles(&mut self) -> Result<()> {
        use crate::dotfile_candidates::get_default_dotfile_paths;

        let file_manager = crate::file_manager::FileManager::new()?;
        let dotfile_names = get_default_dotfile_paths();
        let mut found = file_manager.scan_dotfiles(&dotfile_names);

        // Mark files that are already synced
        let synced_set: std::collections::HashSet<String> = self.config.synced_files.iter()
            .cloned()
            .collect();

        let mut selected_indices = std::collections::HashSet::new();
        for (i, dotfile) in found.iter_mut().enumerate() {
            let relative_str = dotfile.relative_path.to_string_lossy().to_string();
            dotfile.synced = synced_set.contains(&relative_str);

            // If already synced, add to selected set
            if dotfile.synced {
                selected_indices.insert(i);
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
            self.ui_state.dotfile_selection.dotfile_list_state.select(Some(0));
        } else {
            self.ui_state.dotfile_selection.dotfile_list_state.select(None);
        }

        Ok(())
    }

    /// Sync selected files to repository using SymlinkManager
    fn sync_selected_files(&mut self) -> Result<()> {
        use crate::utils::SymlinkManager;

        let state = &mut self.ui_state.dotfile_selection;
        let file_manager = crate::file_manager::FileManager::new()?;
        let profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();

        let mut synced_count = 0;
        let mut unsynced_count = 0;
        let mut errors = Vec::new();

        // Get list of currently selected indices
        let currently_selected: std::collections::HashSet<usize> = state.selected_for_sync.iter().cloned().collect();

        // Get list of previously synced files from the active profile
        let previously_synced: std::collections::HashSet<String> = if let Some(profile) = self.config.get_active_profile() {
            profile.synced_files.iter().cloned().collect()
        } else {
            self.config.synced_files.iter().cloned().collect()
        };

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
                        errors.push(format!("Failed to create repo directory for {}: {}", relative_str, e));
                        continue;
                    }
                }

                // Handle symlinks: resolve to original file
                let source_path = if file_manager.is_symlink(&dotfile.original_path) {
                    match file_manager.resolve_symlink(&dotfile.original_path) {
                        Ok(p) => p,
                        Err(e) => {
                            errors.push(format!("Failed to resolve symlink for {}: {}", relative_str, e));
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
            let mut symlink_mgr = SymlinkManager::new_with_backup(
                repo_path.clone(),
                backup_enabled
            )?;

            match symlink_mgr.activate_profile(&profile_name, &files_to_sync) {
                Ok(operations) => {
                    for op in operations {
                        if matches!(op.status, crate::utils::symlink_manager::OperationStatus::Success) {
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
    let dotfiles_to_unsync: Vec<(usize, String)> = state.dotfiles.iter()
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
                            errors.push(format!("Failed to remove symlink for {}: {}", relative_str, e));
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
                            errors.push(format!("Failed to restore {} from repo: {}", relative_str, e));
                            continue;
                        }

                        info!("Restored {} from repo before unsyncing", relative_str);
                    } else {
                        // Repo file doesn't exist, just remove the orphaned symlink
                        if let Err(e) = std::fs::remove_file(&target_path) {
                            errors.push(format!("Failed to remove orphaned symlink for {}: {}", relative_str, e));
                        }
                        info!("Removed orphaned symlink for {}", relative_str);
                    }
                }
            }

            // Step 2: Now remove from SymlinkManager tracking
            match symlink_mgr.deactivate_profile(&profile_name) {
                Ok(_) => {
                    // Re-activate with remaining files
                    let remaining_files: Vec<String> = self.config.get_active_profile()
                        .map(|p| p.synced_files.clone())
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|f| f != &relative_str)
                        .collect();

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
                    errors.push(format!("Failed to remove {} from repo: {}", relative_str, e));
                    continue;
                }
            }

            unsynced_count += 1;
            state.dotfiles[index].synced = false;
        }
    }

        // Step 4: Update config
        let new_synced_files: Vec<String> = state.dotfiles.iter()
            .enumerate()
            .filter(|(i, _)| currently_selected.contains(i))
            .map(|(_, d)| d.relative_path.to_string_lossy().to_string())
            .collect();

        if let Some(profile) = self.config.get_active_profile_mut() {
            profile.synced_files = new_synced_files.clone();
        }
        self.config.synced_files = new_synced_files;
        self.config.save(&self.config_path)?;

        // Show summary
        let summary = if errors.is_empty() {
            format!(
                "Sync Complete!\n\n✓ Synced: {} files\n✓ Unsynced: {} files\n\nAll operations completed successfully.",
                synced_count, unsynced_count
            )
        } else {
            format!(
                "Sync Completed with Errors\n\n✓ Synced: {} files\n✓ Unsynced: {} files\n\nErrors:\n{}\n\nSome operations failed. Please review the errors above.",
                synced_count,
                unsynced_count,
                errors.join("\n")
            )
        };

        state.status_message = Some(summary);
        Ok(())
    }

    /// Load changed files from git repository
    fn load_changed_files(&mut self) {
        let repo_path = &self.config.repo_path;

        // Check if repo exists
        if !repo_path.exists() {
            self.ui_state.push_changes.changed_files = vec![];
            return;
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(_) => {
                self.ui_state.push_changes.changed_files = vec![];
                return;
            }
        };

        // Get changed files
        match git_mgr.get_changed_files() {
            Ok(files) => {
                self.ui_state.push_changes.changed_files = files;
                // Select first item if list is not empty
                if !self.ui_state.push_changes.changed_files.is_empty() {
                    self.ui_state.push_changes.list_state.select(Some(0));
                }
            }
            Err(_) => {
                self.ui_state.push_changes.changed_files = vec![];
            }
        }
    }

    /// Start pushing changes (async operation with progress updates)
    fn start_push(&mut self) -> Result<()> {
        // Check if GitHub is configured
        if self.config.github.is_none() {
            self.ui_state.push_changes.push_result = Some(
                "Error: GitHub repository not configured.\n\nPlease set up your GitHub repository first from the main menu.".to_string()
            );
            self.ui_state.push_changes.show_result_popup = true;
            return Ok(());
        }

        let repo_path = self.config.repo_path.clone();

        // Check if repo exists
        if !repo_path.exists() {
            self.ui_state.push_changes.push_result = Some(
                format!("Error: Repository not found at {:?}\n\nPlease sync some files first.", repo_path)
            );
            self.ui_state.push_changes.show_result_popup = true;
            return Ok(());
        }

        // Mark as pushing
        self.ui_state.push_changes.is_pushing = true;
        self.ui_state.push_changes.push_progress = Some("Committing changes...".to_string());

        // Don't call draw() here - let the main loop handle it
        // The next draw cycle will show the progress

        // Perform commit
        let git_mgr = match GitManager::open_or_init(&repo_path) {
            Ok(mgr) => mgr,
            Err(e) => {
                self.ui_state.push_changes.is_pushing = false;
                self.ui_state.push_changes.push_progress = None;
                self.ui_state.push_changes.push_result = Some(format!("Error: Failed to open repository: {}", e));
                self.ui_state.push_changes.show_result_popup = true;
                return Ok(());
            }
        };

        let branch = git_mgr.get_current_branch()
            .unwrap_or_else(|| self.config.default_branch.clone());

        // Commit all changes
        let result = match git_mgr.commit_all("Update dotfiles") {
            Ok(_) => {
                // Update progress
                self.ui_state.push_changes.push_progress = Some("Pushing to remote...".to_string());
                // Don't call draw() here - let the main loop handle it

                // Push to remote - get token from config
                let token = self.config.github.as_ref()
                    .and_then(|gh| gh.token.as_deref());
                match git_mgr.push("origin", &branch, token) {
                    Ok(_) => {
                        format!("✓ Successfully pushed changes to GitHub!\n\nBranch: {}\nRepository: {:?}", branch, repo_path)
                    }
                    Err(e) => {
                        // Include the full error chain for debugging
                        let mut error_msg = format!("Error: Failed to push to remote: {}", e);
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
        self.ui_state.push_changes.is_pushing = false;
        self.ui_state.push_changes.push_progress = None;
        self.ui_state.push_changes.push_result = Some(result);
        self.ui_state.push_changes.show_result_popup = true;

        Ok(())
    }

    /// Pull changes from GitHub repository
    fn pull_changes(&mut self) -> Result<()> {
        // Check if GitHub is configured
        if self.config.github.is_none() {
            self.ui_state.dotfile_selection.status_message = Some(
                "Error: GitHub repository not configured.\n\nPlease set up your GitHub repository first from the main menu.".to_string()
            );
            return Ok(());
        }

        let repo_path = &self.config.repo_path;

        // Check if repo exists
        if !repo_path.exists() {
            self.ui_state.dotfile_selection.status_message = Some(
                format!("Error: Repository not found at {:?}\n\nPlease sync some files first.", repo_path)
            );
            return Ok(());
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(e) => {
                self.ui_state.dotfile_selection.status_message = Some(
                    format!("Error: Failed to open repository: {}", e)
                );
                return Ok(());
            }
        };

        // Get current branch
        let branch = git_mgr.get_current_branch()
            .unwrap_or_else(|| self.config.default_branch.clone());

        // Pull from remote
        match git_mgr.pull("origin", &branch) {
            Ok(_) => {
                self.ui_state.dotfile_selection.status_message = Some(
                    format!("✓ Successfully pulled changes from GitHub!\n\nBranch: {}\nRepository: {:?}\n\nNote: You may need to re-sync files if the repository structure changed.", branch, repo_path)
                );
            }
            Err(e) => {
                self.ui_state.dotfile_selection.status_message = Some(
                    format!("Error: Failed to pull from remote: {}", e)
                );
            }
        }

        Ok(())
    }

    /// Handle input for file browser
    fn handle_file_browser_input(&mut self, key_code: KeyCode) -> Result<()> {
        use crate::ui::DotfileSelectionFocus;
        let state = &mut self.ui_state.dotfile_selection;

        // Handle path input if focused
        if state.file_browser_path_focused && state.focus == DotfileSelectionFocus::FileBrowserInput {
            match key_code {
                // Text input handling - use text input utility
                KeyCode::Char(c) => {
                    crate::utils::handle_char_insertion(&mut state.file_browser_path_input, &mut state.file_browser_path_cursor, c);
                    return Ok(());
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                    crate::utils::handle_cursor_movement(&state.file_browser_path_input, &mut state.file_browser_path_cursor, key_code);
                    return Ok(());
                }
                KeyCode::Backspace => {
                    crate::utils::handle_backspace(&mut state.file_browser_path_input, &mut state.file_browser_path_cursor);
                    return Ok(());
                }
                KeyCode::Delete => {
                    crate::utils::handle_delete(&mut state.file_browser_path_input, &mut state.file_browser_path_cursor);
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
                                state.file_browser_path_input = state.file_browser_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor = state.file_browser_path_input.chars().count();
                                state.file_browser_list_state.select(Some(0));
                                state.focus = DotfileSelectionFocus::FileBrowserList;
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path = state.file_browser_path.clone();
                                self.refresh_file_browser()?;
                                return Ok(());
                            } else {
                                // It's a file - load it into custom file input and close browser
                                let home_dir = crate::utils::get_home_dir();
                                let relative_path = full_path.strip_prefix(&home_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| full_path.to_string_lossy().to_string());
                                state.file_browser_mode = false;
                                state.custom_file_input = relative_path;
                                state.custom_file_cursor = state.custom_file_input.len();
                                state.custom_file_focused = true;
                                state.file_browser_path_input.clear();
                                state.file_browser_path_cursor = 0;
                                state.focus = DotfileSelectionFocus::CustomInput;
                            }
                        }
                    }
                    return Ok(());
                }
                KeyCode::Tab => {
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
            KeyCode::Tab => {
                // Cycle focus: List -> Preview -> Input -> List
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
            }
            KeyCode::Up => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    state.file_browser_list_state.select_previous();
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    if state.file_browser_preview_scroll > 0 {
                        state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_sub(1);
                    }
                }
            }
            KeyCode::Down => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    state.file_browser_list_state.select_next();
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_add(1);
                }
            }
            KeyCode::Char('u') => {
                if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    if state.file_browser_preview_scroll > 0 {
                        state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_sub(10);
                    }
                }
            }
            KeyCode::Char('d') => {
                if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_add(10);
                }
            }
            KeyCode::PageUp => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    if let Some(current) = state.file_browser_list_state.selected() {
                        let new_index = current.saturating_sub(10);
                        state.file_browser_list_state.select(Some(new_index));
                    }
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    if state.file_browser_preview_scroll > 0 {
                        state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_sub(20);
                    }
                }
            }
            KeyCode::PageDown => {
                if state.focus == DotfileSelectionFocus::FileBrowserList {
                    if let Some(current) = state.file_browser_list_state.selected() {
                        let new_index = (current + 10).min(state.file_browser_entries.len().saturating_sub(1));
                        state.file_browser_list_state.select(Some(new_index));
                    } else if !state.file_browser_entries.is_empty() {
                        state.file_browser_list_state.select(Some(10.min(state.file_browser_entries.len() - 1)));
                    }
                } else if state.focus == DotfileSelectionFocus::FileBrowserPreview {
                    state.file_browser_preview_scroll = state.file_browser_preview_scroll.saturating_add(20);
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
                                state.file_browser_path_input = state.file_browser_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor = state.file_browser_path_input.chars().count();
                                state.file_browser_list_state.select(Some(0));
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path = state.file_browser_path.clone();
                                self.refresh_file_browser()?;
                                return Ok(());
                            }
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
                                self.ui_state.dotfile_selection.file_browser_path = state.file_browser_path.clone();
                                self.refresh_file_browser()?;
                                return Ok(());
                            } else if full_path.is_file() {
                                // Load file path into custom file input and close browser
                                let home_dir = crate::utils::get_home_dir();
                                let relative_path = full_path.strip_prefix(&home_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| full_path.to_string_lossy().to_string());

                                state.file_browser_mode = false;
                                state.custom_file_input = relative_path;
                                state.custom_file_cursor = state.custom_file_input.len();
                                state.custom_file_focused = true;
                                // Keep the path input value (don't clear it)
                                // state.file_browser_path_input stays as is
                                state.file_browser_path_cursor = state.file_browser_path_input.chars().count();
                                state.file_browser_path_focused = false;
                                state.focus = DotfileSelectionFocus::CustomInput;
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

        // Read directory entries
        if let Ok(entries_iter) = std::fs::read_dir(path) {
            for entry in entries_iter {
                if let Ok(entry) = entry {
                    let entry_path = entry.path();
                    // Show all files for now (user can navigate)
                    entries.push(entry_path);
                }
            }
        }

        // Sort: directories first, then files, both alphabetically
        entries.sort_by(|a, b| {
            let a_is_dir = if a == Path::new("..") {
                true
            } else {
                a.is_dir()
            };
            let b_is_dir = if b == Path::new("..") {
                true
            } else {
                b.is_dir()
            };

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
                    state.file_browser_list_state.select(Some(state.file_browser_entries.len() - 1));
                }
            }
        } else if !state.file_browser_entries.is_empty() {
            // If nothing selected, select first item
            state.file_browser_list_state.select(Some(0));
        }

        Ok(())
    }

    /// Create a new profile
    fn create_profile(&mut self, name: &str, description: Option<String>, copy_from: Option<usize>) -> Result<()> {
        use crate::utils::{validate_profile_name, sanitize_profile_name};
        use crate::config::Profile;

        // Validate and sanitize profile name
        let sanitized_name = sanitize_profile_name(name);
        if sanitized_name.is_empty() {
            return Err(anyhow::anyhow!("Profile name cannot be empty"));
        }

        let existing_names: Vec<String> = self.config.profiles.iter().map(|p| p.name.clone()).collect();
        if let Err(e) = validate_profile_name(&sanitized_name, &existing_names) {
            return Err(anyhow::anyhow!("Invalid profile name: {}", e));
        }

        // Create profile folder in repo
        let profile_path = self.config.repo_path.join(&sanitized_name);
        if profile_path.exists() {
            return Err(anyhow::anyhow!("Profile folder already exists: {:?}", profile_path));
        }

        std::fs::create_dir_all(&profile_path)
            .context("Failed to create profile directory")?;

        // Copy files from source profile if specified
        let synced_files = if let Some(source_idx) = copy_from {
            if let Some(source_profile) = self.config.profiles.get(source_idx) {
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

        // Create profile and add to config
        let profile = Profile::new(sanitized_name.clone(), description.clone());
        let mut profile = profile;
        profile.synced_files = synced_files;

        self.config.add_profile(profile);
        self.config.save(&self.config_path)?;

        // Update profile manifest in repo (backfill if needed)
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&self.config.repo_path)
            .unwrap_or_else(|_| crate::utils::ProfileManifest { profiles: Vec::new() });
        manifest.add_profile(sanitized_name.clone(), description);
        manifest.save(&self.config.repo_path)?;

        info!("Created profile: {}", sanitized_name);
        Ok(())
    }

    /// Switch to a different profile
    fn switch_profile(&mut self, target_profile_name: &str) -> Result<()> {
        use crate::utils::SymlinkManager;

        // Validate target profile exists
        let target_profile = self.config.get_profile(target_profile_name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", target_profile_name))?;

        // Don't switch if already active
        if self.config.active_profile == target_profile_name {
            return Ok(());
        }

        let old_profile_name = self.config.active_profile.clone();
        let repo_path = self.config.repo_path.clone();

        // Use SymlinkManager to switch profiles
        let mut symlink_mgr = SymlinkManager::new_with_backup(
            repo_path.clone(),
            self.config.backup_enabled
        )?;

        let switch_result = symlink_mgr.switch_profile(
            &old_profile_name,
            target_profile_name,
            &target_profile.synced_files,
        )?;

        // Update active profile in config
        self.config.active_profile = target_profile_name.to_string();
        self.config.save(&self.config_path)?;

        info!("Switched from '{}' to '{}'", old_profile_name, target_profile_name);
        info!("Removed {} symlinks, created {} symlinks", switch_result.removed.len(), switch_result.created.len());

        Ok(())
    }

    /// Rename a profile
    fn rename_profile(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        use crate::utils::{validate_profile_name, sanitize_profile_name};

        // Validate new name
        let sanitized_name = sanitize_profile_name(new_name);
        if sanitized_name.is_empty() {
            return Err(anyhow::anyhow!("Profile name cannot be empty"));
        }

        let existing_names: Vec<String> = self.config.profiles.iter()
            .filter(|p| p.name != old_name)
            .map(|p| p.name.clone())
            .collect();
        if let Err(e) = validate_profile_name(&sanitized_name, &existing_names) {
            return Err(anyhow::anyhow!("Invalid profile name: {}", e));
        }

        // Clone values we need before borrowing
        let repo_path = self.config.repo_path.clone();
        let was_active = self.config.active_profile == old_name;

        // Get the profile and update name
        {
            let profile = self.config.get_profile_mut(old_name)
                .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", old_name))?;
            profile.name = sanitized_name.clone();
        }

        // Rename profile folder in repo
        let old_path = repo_path.join(old_name);
        let new_path = repo_path.join(&sanitized_name);

        if old_path.exists() {
            std::fs::rename(&old_path, &new_path)
                .context("Failed to rename profile directory")?;
        }

        // Update active profile name if this was the active profile
        if was_active {
            self.config.active_profile = sanitized_name.clone();
        }

        self.config.save(&self.config_path)?;

        // Update profile manifest in repo (backfill if needed)
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&self.config.repo_path)
            .unwrap_or_else(|_| crate::utils::ProfileManifest { profiles: Vec::new() });
        let _ = manifest.rename_profile(old_name, &sanitized_name);
        manifest.save(&self.config.repo_path)?;

        // Update symlinks if profile is active (has symlinks)
        if self.config.profile_activated && was_active {
            use crate::utils::SymlinkManager;
            let mut symlink_mgr = SymlinkManager::new_with_backup(
                repo_path.clone(),
                self.config.backup_enabled,
            )?;

            match symlink_mgr.rename_profile(old_name, &sanitized_name) {
                Ok(ops) => {
                    let success_count = ops.iter()
                        .filter(|op| op.status == crate::utils::symlink_manager::OperationStatus::Success)
                        .count();
                    info!("Updated {} symlinks for renamed profile", success_count);
                }
                Err(e) => {
                    error!("Failed to update symlinks after rename: {}", e);
                    // Don't fail the rename, but log the error
                }
            }
        }

        info!("Renamed profile from '{}' to '{}'", old_name, sanitized_name);
        Ok(())
    }

    /// Delete a profile
    fn delete_profile(&mut self, profile_name: &str) -> Result<()> {
        // Cannot delete active profile
        if self.config.active_profile == profile_name {
            return Err(anyhow::anyhow!("Cannot delete active profile '{}'. Please switch to another profile first.", profile_name));
        }

        // Remove profile folder from repo
        let profile_path = self.config.repo_path.join(profile_name);
        if profile_path.exists() {
            std::fs::remove_dir_all(&profile_path)
                .context("Failed to remove profile directory")?;
        }

        // Remove from config
        if !self.config.remove_profile(profile_name) {
            return Err(anyhow::anyhow!("Profile '{}' not found", profile_name));
        }

        self.config.save(&self.config_path)?;

        // Update profile manifest in repo (backfill if needed)
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&self.config.repo_path)
            .unwrap_or_else(|_| crate::utils::ProfileManifest { profiles: Vec::new() });
        manifest.remove_profile(profile_name);
        manifest.save(&self.config.repo_path)?;

        info!("Deleted profile: {}", profile_name);
        Ok(())
    }

    /// Render profile selection screen (after GitHub setup)
    fn render_profile_selection_screen(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem};
        use crate::components::header::Header;
        use crate::components::footer::Footer;
        use crate::utils::create_standard_layout;
        use ratatui::style::{Style, Color, Modifier};

        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let _ = Header::render(
            frame,
            header_chunk,
            "Select Profile to Activate",
            "Choose which profile to activate after cloning the repository"
        )?;

        // Content: List of profiles
        let state = &mut self.ui_state.profile_selection;
        // Clone config profiles to avoid borrow checker issues in closure
        let config_profiles = self.config.profiles.clone();
        let items: Vec<ListItem> = state.profiles.iter()
            .map(|name| {
                let profile = config_profiles.iter().find(|p| p.name == *name);
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

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Available Profiles")
                    .border_style(Style::default().fg(Color::Cyan))
            )
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, content_chunk, &mut state.list_state);

        // Footer
        let footer_text = "↑↓: Navigate | Enter: Activate Profile | Esc: Skip";
        let _ = Footer::render(frame, footer_chunk, footer_text)?;

        Ok(())
    }

    /// Activate a profile after GitHub setup (includes syncing files from repo)
    fn activate_profile_after_setup(&mut self, profile_name: &str) -> Result<()> {
        use crate::utils::SymlinkManager;

        info!("Activating profile '{}' after setup", profile_name);

        // Set as active profile
        self.config.active_profile = profile_name.to_string();
        self.config.save(&self.config_path)?;

        // Get profile to activate
        let profile = self.config.profiles.iter()
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
            self.config.backup_enabled
        )?;

        // Activate profile (this will create symlinks and sync files)
        match symlink_mgr.activate_profile(profile_name, &files_to_sync) {
            Ok(operations) => {
                let success_count = operations.iter()
                    .filter(|op| matches!(op.status, crate::utils::symlink_manager::OperationStatus::Success))
                    .count();
                info!("Activated profile '{}' with {} files", profile_name, success_count);

                // Mark as activated
                self.config.profile_activated = true;
                self.config.save(&self.config_path)?;

                Ok(())
            }
            Err(e) => {
                error!("Failed to activate profile '{}': {}", profile_name, e);
                Err(anyhow::anyhow!("Failed to activate profile: {}", e))
            }
        }
    }
}



