use anyhow::{Context, Result};
use crate::config::{Config, GitHubConfig};
use crate::file_manager::FileManager;
use crate::github::GitHubClient;
use crate::git::GitManager;
use crate::tui::Tui;
use crate::ui::{UiState, Screen, GitHubAuthStep, render_dotfile_selection};
use crate::components::{WelcomeComponent, MainMenuComponent, GitHubAuthComponent, SyncedFilesComponent, MessageComponent, DotfileSelectionComponent, ComponentAction, Component};
use crossterm::event::{Event, KeyCode, KeyEventKind};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::runtime::Runtime;
use tracing::debug;

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
    welcome_component: WelcomeComponent,
    main_menu_component: MainMenuComponent,
    github_auth_component: GitHubAuthComponent,
    dotfile_selection_component: DotfileSelectionComponent,
    synced_files_component: SyncedFilesComponent,
    message_component: Option<MessageComponent>,
}

impl App {
    pub fn new() -> Result<Self> {
        let config_path = dirs::config_dir()
            .unwrap_or_else(|| crate::utils::get_home_dir())
            .join("dotzz")
            .join("config.toml");

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
            welcome_component: WelcomeComponent::new(),
            main_menu_component: MainMenuComponent::new(has_changes),
            github_auth_component: GitHubAuthComponent::new(),
            dotfile_selection_component: DotfileSelectionComponent::new(),
            synced_files_component: SyncedFilesComponent::new(config_clone),
            message_component: None,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        self.tui.enter()?;

        // Determine initial screen based on whether GitHub is configured
        if self.config.github.is_none() {
            self.ui_state.current_screen = Screen::Welcome;
        } else {
            self.ui_state.current_screen = Screen::MainMenu;
        }
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

        // Create/update message component if needed
        if matches!(self.ui_state.current_screen, Screen::PushChanges | Screen::PullChanges) {
            let title = if self.ui_state.current_screen == Screen::PushChanges {
                "Push Changes"
            } else {
                "Pull Changes"
            };
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
                    let _ = self.welcome_component.render(frame, area);
                }
                Screen::MainMenu => {
                    let _ = self.main_menu_component.render(frame, area);
                }
                Screen::GitHubAuth => {
                    // Sync state back after render (component may update it)
                    let _ = self.github_auth_component.render(frame, area);
                    self.ui_state.github_auth = self.github_auth_component.get_auth_state().clone();
                }
                Screen::DotfileSelection => {
                    // Clear is handled by component, then render
                    let _ = self.dotfile_selection_component.render(frame, area);
                    // Now render the actual content (component ensures Clear is called first)
                    if let Err(e) = render_dotfile_selection(frame, &mut self.ui_state) {
                        eprintln!("Error rendering dotfile selection: {}", e);
                    }
                }
                Screen::ViewSyncedFiles => {
                    let _ = self.synced_files_component.render(frame, area);
                }
                Screen::PushChanges | Screen::PullChanges => {
                    if let Some(ref mut msg_component) = self.message_component {
                        let _ = msg_component.render(frame, area);
                    }
                }
            }
        })?;
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        // Let components handle events first (for mouse support)
        match self.ui_state.current_screen {
            Screen::Welcome => {
                match self.welcome_component.handle_event(event)? {
                    ComponentAction::Update => {
                        // Check if GitHub is configured
                        if self.config.github.is_none() {
                            self.ui_state.current_screen = Screen::GitHubAuth;
                        } else {
                            self.ui_state.current_screen = Screen::MainMenu;
                        }
                    }
                    ComponentAction::Quit => {
                        self.should_quit = true;
                    }
                    _ => {}
                }
                return Ok(());
            }
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
                        self.handle_github_auth_input(key.code)?;
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
            Screen::PushChanges | Screen::PullChanges => {
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

        match self.ui_state.selected_index {
            0 => {
                // Setup GitHub Repository
                self.ui_state.current_screen = Screen::GitHubAuth;
            }
            1 => {
                // Scan & Select Dotfiles
                self.scan_dotfiles()?;
                self.ui_state.current_screen = Screen::DotfileSelection;
            }
            2 => {
                // View Synced Files
                self.ui_state.current_screen = Screen::ViewSyncedFiles;
            }
            3 => {
                // Push Changes
                self.ui_state.current_screen = Screen::PushChanges;
                self.push_changes()?;
            }
            4 => {
                // Pull Changes
                self.ui_state.current_screen = Screen::PullChanges;
                self.pull_changes()?;
            }
            5 => {
                // Manage Profiles
                // TODO: Implement
            }
            _ => {}
        }
        Ok(())
    }

    /// Check for changes to push and update UI state
    fn check_changes_to_push(&mut self) {
        self.ui_state.has_changes_to_push = false;

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

    fn handle_github_auth_input(&mut self, key_code: KeyCode) -> Result<()> {
        let auth_state = &mut self.ui_state.github_auth;
        auth_state.error_message = None;

        match auth_state.step {
            GitHubAuthStep::TokenInput => {
                // Check if input is focused
                if !auth_state.input_focused {
                    // Input not focused - handle navigation/help keys
                    match key_code {
                        KeyCode::Tab | KeyCode::Char('?') | KeyCode::F(1) => {
                            auth_state.show_help = !auth_state.show_help;
                        }
                        KeyCode::Up if auth_state.show_help => {
                            if auth_state.help_scroll > 0 {
                                auth_state.help_scroll -= 1;
                            }
                        }
                        KeyCode::Down if auth_state.show_help => {
                            auth_state.help_scroll += 1;
                        }
                        KeyCode::Enter => {
                            // Focus input when Enter is pressed
                            auth_state.input_focused = true;
                        }
                        KeyCode::Esc => {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
                        }
                        _ => {}
                    }
                    return Ok(());
                }

                // When input is focused, only handle input-related keys
                match key_code {
                    // Character input - use text input utility
                    KeyCode::Char(c) => {
                        crate::utils::handle_char_insertion(&mut auth_state.token_input, &mut auth_state.cursor_position, c);
                        debug!("Received character: '{}' (code: {})", c, c as u32);
                    }
                    // Navigation within input - use text input utility
                    KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                        crate::utils::handle_cursor_movement(&auth_state.token_input, &mut auth_state.cursor_position, key_code);
                    }
                    // Editing - use text input utility
                    KeyCode::Backspace => {
                        crate::utils::handle_backspace(&mut auth_state.token_input, &mut auth_state.cursor_position);
                    }
                    KeyCode::Delete => {
                        crate::utils::handle_delete(&mut auth_state.token_input, &mut auth_state.cursor_position);
                    }
                    // Tab to unfocus input
                    KeyCode::Tab => {
                        auth_state.input_focused = false;
                    }
                    // Help scroll (only when help is visible)
                    KeyCode::Up if auth_state.show_help => {
                        if auth_state.help_scroll > 0 {
                            auth_state.help_scroll -= 1;
                        }
                    }
                    KeyCode::Down if auth_state.show_help => {
                        auth_state.help_scroll += 1;
                    }
                    KeyCode::Enter => {
                        // Prevent duplicate processing
                        if auth_state.step == GitHubAuthStep::Processing {
                            return Ok(());
                        }

                        let token = auth_state.token_input.trim();
                        if token.is_empty() {
                            auth_state.error_message = Some("Token cannot be empty".to_string());
                        } else if !token.starts_with("ghp_") {
                            auth_state.error_message = Some(
                                "Token format error: GitHub tokens must start with 'ghp_'.\n\
                                Make sure you copied the entire token from GitHub."
                                    .to_string()
                            );
                        } else if token.len() < 40 {
                            auth_state.error_message = Some(
                                format!(
                                    "Token appears incomplete: {} characters (expected 40+).\n\
                                    Make sure you copied the entire token from GitHub.",
                                    token.len()
                                )
                            );
                        } else {
                            // Trim and update the token input
                            auth_state.token_input = token.to_string();
                            auth_state.cursor_position = auth_state.token_input.len();
                            // Start processing immediately (no repo name input needed)
                            self.process_github_setup()?;
                        }
                    }
                    KeyCode::Esc => {
                        self.ui_state.current_screen = Screen::MainMenu;
                        *auth_state = Default::default();
                    }
                    _ => {
                        // All other keys are ignored when input is focused
                        // This prevents accidental triggering of commands
                    }
                }
            }
            GitHubAuthStep::Processing => {
                // Allow user to continue after processing completes
                match key_code {
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        // If setup was successful, go to main menu
                        if auth_state.error_message.is_none() && auth_state.status_message.is_some() {
                            self.ui_state.current_screen = Screen::MainMenu;
                            *auth_state = Default::default();
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
            auth_state.step = GitHubAuthStep::TokenInput;
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
            auth_state.step = GitHubAuthStep::TokenInput;
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
                            auth_state.step = GitHubAuthStep::TokenInput;
                            return Ok(());
                        }
                    }
                } else {
                    auth_state.status_message = Some(format!("Creating repository: {}/{}", username, repo_name));

                    // Create repository
                    let create_result = self.runtime.block_on(async {
                        let client = GitHubClient::new(token.clone());
                        client.create_repo(&repo_name, "My dotfiles managed by dotzz", false).await
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
                            git_mgr.add_remote("origin", &remote_url)?;

                            // Create initial commit
                            std::fs::write(repo_path.join("README.md"),
                                format!("# {}\n\nDotfiles managed by dotzz", repo_name))?;
                            git_mgr.commit_all("Initial commit")?;

                            // Get current branch name (might be 'master' or 'main')
                            let current_branch = git_mgr.get_current_branch()
                                .unwrap_or_else(|| "main".to_string());

                            // Push to remote using the actual branch name
                            git_mgr.push("origin", &current_branch)?;

                            auth_state.status_message = Some(format!("Repository created and initialized successfully"));
                        }
                        Err(e) => {
                            auth_state.error_message = Some(format!("Failed to create repository: {}", e));
                            auth_state.step = GitHubAuthStep::TokenInput;
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
                    auth_state.step = GitHubAuthStep::TokenInput;
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
                auth_state.step = GitHubAuthStep::TokenInput;
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
                        GitHubAuthStep::TokenInput => {
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

        // If we're adding a custom file, handle file browser or input
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

        // Normal dotfile selection input handling
        use crate::ui::DotfileSelectionFocus;
        match key_code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.ui_state.current_screen = Screen::MainMenu;
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
                        let should_rescan = !self.config.default_dotfiles.contains(&relative_path);

                        if should_rescan {
                            self.config.default_dotfiles.push(relative_path.clone());
                            self.config.save(&self.config_path)?;
                        }

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

    /// Scan for dotfiles and populate the selection state
    fn scan_dotfiles(&mut self) -> Result<()> {
        let file_manager = crate::file_manager::FileManager::new()?;
        let dotfile_names = &self.config.default_dotfiles;
        let mut found = file_manager.scan_dotfiles(dotfile_names);

        // Mark files that are already synced
        let synced_set: std::collections::HashSet<String> = self.config.synced_files.iter()
            .map(|s| s.clone())
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

    /// Sync selected files to repository
    fn sync_selected_files(&mut self) -> Result<()> {
        let state = &mut self.ui_state.dotfile_selection;
        let file_manager = crate::file_manager::FileManager::new()?;
        let profile = &self.config.active_profile;
        let repo_path = &self.config.repo_path;

        let mut synced_count = 0;
        let mut unsynced_count = 0;
        let mut errors = Vec::new();

        // Get list of currently selected indices
        let currently_selected: std::collections::HashSet<usize> = state.selected_for_sync.iter().cloned().collect();

        // Get list of previously synced files (from config)
        let previously_synced: std::collections::HashSet<String> = self.config.synced_files.iter()
            .map(|s| s.clone())
            .collect();

        // Sync newly selected files
        for &index in &currently_selected {
            if index >= state.dotfiles.len() {
                continue;
            }

            let dotfile = &state.dotfiles[index];
            let relative_str = dotfile.relative_path.to_string_lossy().to_string();

            // If not already synced, sync it
            if !previously_synced.contains(&relative_str) {
                match file_manager.sync_file(dotfile, repo_path, profile) {
                    Ok(_) => {
                        synced_count += 1;
                        // Update dotfile synced status
                        state.dotfiles[index].synced = true;
                    }
                    Err(e) => {
                        errors.push(format!("Failed to sync {}: {}", relative_str, e));
                    }
                }
            }
        }

        // Unsync files that were deselected
        let dotfiles_to_unsync: Vec<(usize, String)> = state.dotfiles.iter()
            .enumerate()
            .filter(|(index, dotfile)| {
                let relative_str = dotfile.relative_path.to_string_lossy().to_string();
                previously_synced.contains(&relative_str) && !currently_selected.contains(index)
            })
            .map(|(index, dotfile)| (index, dotfile.relative_path.to_string_lossy().to_string()))
            .collect();

        for (index, relative_str) in dotfiles_to_unsync {
            let dotfile = &state.dotfiles[index];
            match file_manager.unsync_file(dotfile, repo_path, profile) {
                Ok(_) => {
                    unsynced_count += 1;
                    // Update dotfile synced status
                    state.dotfiles[index].synced = false;
                }
                Err(e) => {
                    errors.push(format!("Failed to unsync {}: {}", relative_str, e));
                }
            }
        }

        // Update config with new synced files list
        self.config.synced_files = state.dotfiles.iter()
            .enumerate()
            .filter(|(i, _)| currently_selected.contains(i))
            .map(|(_, d)| d.relative_path.to_string_lossy().to_string())
            .collect();

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

        // Store summary in status message
        state.status_message = Some(summary);

        Ok(())
    }

    /// Push changes to GitHub repository
    fn push_changes(&mut self) -> Result<()> {
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
            .unwrap_or_else(|| "main".to_string());

        // Commit all changes
        match git_mgr.commit_all("Update dotfiles") {
            Ok(_) => {
                // Push to remote
                match git_mgr.push("origin", &branch) {
                    Ok(_) => {
                        self.ui_state.dotfile_selection.status_message = Some(
                            format!("✓ Successfully pushed changes to GitHub!\n\nBranch: {}\nRepository: {:?}", branch, repo_path)
                        );
                    }
                    Err(e) => {
                        self.ui_state.dotfile_selection.status_message = Some(
                            format!("Error: Failed to push to remote: {}", e)
                        );
                    }
                }
            }
            Err(e) => {
                self.ui_state.dotfile_selection.status_message = Some(
                    format!("Error: Failed to commit changes: {}", e)
                );
            }
        }

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
            .unwrap_or_else(|| "main".to_string());

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
                                state.file_browser_path_input = full_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor = state.file_browser_path_input.chars().count();
                                state.file_browser_list_state.select(Some(0));
                                state.focus = DotfileSelectionFocus::FileBrowserList;
                                // Refresh after updating path
                                let path = state.file_browser_path.clone();
                                self.ui_state.dotfile_selection.file_browser_path = path;
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
                                state.file_browser_path_input = parent_path.to_string_lossy().to_string();
                                state.file_browser_path_cursor = state.file_browser_path_input.chars().count();
                                state.file_browser_list_state.select(Some(0));
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path = parent_path;
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
                                let dir_path = full_path.clone();
                                state.file_browser_path = dir_path.clone();
                                state.file_browser_list_state.select(Some(0));
                                // Refresh after updating path
                                self.ui_state.dotfile_selection.file_browser_path = dir_path;
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
}


