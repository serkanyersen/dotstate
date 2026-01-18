//! Main menu screen controller.
//!
//! This screen is the application's entry point after setup, showing
//! the main navigation menu.

use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::config::Config;
use crate::icons::Icons;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::theme;
use crate::ui::Screen as ScreenId;
use crate::utils::create_standard_layout;
use crate::version_check::UpdateInfo;
use crate::widgets::{Menu, MenuItem as MenuWidgetItem, MenuState};
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Wrap};

/// Menu items enum - defines the order and available menu options
/// This is the single source of truth for menu items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    ScanDotfiles,
    SyncWithRemote,
    ManageProfiles,
    ManagePackages,
    SetupRepository,
}

impl MenuItem {
    /// Get all menu items in display order
    pub fn all() -> Vec<MenuItem> {
        vec![
            MenuItem::ScanDotfiles,
            MenuItem::SyncWithRemote,
            MenuItem::ManageProfiles,
            MenuItem::ManagePackages,
            MenuItem::SetupRepository,
        ]
    }

    /// Check if this menu item requires repository setup
    pub fn requires_setup(&self) -> bool {
        match self {
            MenuItem::SetupRepository => false, // Always available
            _ => true,                          // All other items require setup
        }
    }

    /// Check if this menu item is enabled based on setup status
    pub fn is_enabled(&self, is_setup: bool) -> bool {
        !self.requires_setup() || is_setup
    }

    /// Get the icon for this menu item using the icon provider
    pub fn icon(&self, icons: &Icons) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => icons.folder(),
            MenuItem::SyncWithRemote => icons.sync(),
            MenuItem::ManageProfiles => icons.profile(),
            MenuItem::ManagePackages => icons.package(),
            MenuItem::SetupRepository => icons.git(),
        }
    }

    /// Get the display text for this menu item
    pub fn text(&self) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => "Manage Files",
            MenuItem::SyncWithRemote => "Sync with Remote",
            MenuItem::ManageProfiles => "Manage Profiles",
            MenuItem::ManagePackages => "Manage Packages",
            MenuItem::SetupRepository => "Setup git repository",
        }
    }

    /// Get the base color for this menu item
    pub fn color(&self, has_changes: bool) -> Color {
        let t = theme();
        match self {
            MenuItem::SyncWithRemote if has_changes => t.warning,
            _ => t.text,
        }
    }

    /// Get the explanation text for this menu item (uses icon provider)
    pub fn explanation(&self, icons: &Icons) -> Text<'static> {
        let t = theme();
        match self {
            MenuItem::ScanDotfiles => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Manage Your Dotfiles", t.title_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Keep your configuration files (like ", t.text_style()),
                        Span::styled(".zshrc", t.emphasis_style()),
                        Span::styled(", ", t.text_style()),
                        Span::styled(".vimrc", t.emphasis_style()),
                        Span::styled(", ", t.text_style()),
                        Span::styled(".gitconfig", t.emphasis_style()),
                        Span::styled(", etc.) synchronized across all your machines. ", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("When you select a file, it's automatically ", t.text_style()),
                        Span::styled("copied to your repository", t.success_style()),
                        Span::styled(" and a ", t.text_style()),
                        Span::styled("symlink", Style::default().fg(t.primary).add_modifier(Modifier::BOLD)),
                        Span::styled(" is created in its place. This means your files are safely backed up and version controlled.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled(" Tip: ", Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled("You can add custom files using the file browser, or use the CLI:\n", t.text_style()),
                        Span::styled("  dotstate add ~/.myconfig", t.emphasis_style()),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::SyncWithRemote => {
                let repo_name = crate::config::default_repo_name();
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Sync with Remote Repository", Style::default().fg(t.success).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Keep your dotfiles synchronized across all your devices. This feature ", t.text_style()),
                        Span::styled("commits", Style::default().fg(t.primary)),
                        Span::styled(" your local changes, ", t.text_style()),
                        Span::styled("pulls", Style::default().fg(t.tertiary)),
                        Span::styled(" any updates from the remote, and ", t.text_style()),
                        Span::styled("pushes", t.success_style()),
                        Span::styled(" everything back up.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Perfect for when you've made changes on one computer and want to sync them to another. ", t.text_style()),
                        Span::styled("All changes are automatically merged", t.success_style()),
                        Span::styled(" with your remote repository.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Your repository is called ", t.text_style()),
                        Span::styled(repo_name.clone(), Style::default().fg(t.text_emphasis).add_modifier(Modifier::BOLD)),
                        Span::styled(" and should be visible in your GitHub account.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled(" CLI: ", Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled("dotstate sync", t.emphasis_style()),
                        Span::styled(" - Same functionality from the command line", t.text_style()),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::ManageProfiles => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Manage Multiple Profiles", Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Create different sets of dotfiles for different contexts. Perfect for managing ", t.text_style()),
                        Span::styled("work", Style::default().fg(t.tertiary)),
                        Span::styled(" vs ", t.text_style()),
                        Span::styled("personal", t.success_style()),
                        Span::styled(" configurations, or different ", t.text_style()),
                        Span::styled("operating systems", Style::default().fg(t.primary)),
                        Span::styled(".", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Example: Switch between a ", t.text_style()),
                        Span::styled("Mac", t.emphasis_style()),
                        Span::styled(" profile with macOS-specific settings and a ", t.text_style()),
                        Span::styled("Linux", t.success_style()),
                        Span::styled(" profile for your servers.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Each profile maintains its own set of synced files and packages, so you can keep everything organized and context-specific.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled(" CLI: ", Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled("dotstate profile list", t.emphasis_style()),
                        Span::styled(" - List all profiles\n", t.text_style()),
                        Span::styled("  dotstate profile switch <name>", t.emphasis_style()),
                        Span::styled(" - Switch between profiles", t.text_style()),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::ManagePackages => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Manage CLI Tools & Dependencies", Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Ensure all your essential command-line tools are installed across your machines. ", t.text_style()),
                        Span::styled("Automatically detect", Style::default().fg(t.primary)),
                        Span::styled(" which packages are missing and ", t.text_style()),
                        Span::styled("install them with one command", t.success_style()),
                        Span::styled(".", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Great for setting up a new machine quickly! Just sync your dotfiles and install all your tools (like ", t.text_style()),
                        Span::styled("git", t.emphasis_style()),
                        Span::styled(", ", t.text_style()),
                        Span::styled("vim", t.emphasis_style()),
                        Span::styled(", ", t.text_style()),
                        Span::styled("node", t.emphasis_style()),
                        Span::styled(", etc.) in one go.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("Supports both ", t.text_style()),
                        Span::styled("managed packages", t.success_style()),
                        Span::styled(" (auto-detected from common package managers) and ", t.text_style()),
                        Span::styled("custom packages", Style::default().fg(t.primary)),
                        Span::styled(" with custom installation commands.", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled(" Example: ", Style::default().fg(t.secondary).add_modifier(Modifier::BOLD)),
                        Span::styled("Add packages like ", t.text_style()),
                        Span::styled("ripgrep", t.emphasis_style()),
                        Span::styled(" or ", t.text_style()),
                        Span::styled("fzf", t.emphasis_style()),
                        Span::styled(" to your profile, and they'll be installed automatically on new machines.", t.text_style()),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::SetupRepository => {
                let lines = vec![
                    Line::from(vec![Span::styled(
                        "Setup Git Repository",
                        Style::default()
                            .fg(t.text_emphasis)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Configure a git repository to store and sync your dotfiles. ",
                            t.text_style(),
                        ),
                        Span::styled(
                            "Choose how you want to set up:",
                            Style::default().fg(t.primary),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Option 1: ",
                            Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Create for me (GitHub)", t.success_style()),
                    ]),
                    Line::from(vec![Span::styled(
                        "  Automatically create a repository on GitHub.",
                        t.text_style(),
                    )]),
                    Line::from(vec![Span::styled(
                        "  Requires a GitHub Personal Access Token.",
                        t.text_style(),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Option 2: ",
                            Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Use my own repository", Style::default().fg(t.tertiary)),
                    ]),
                    Line::from(vec![Span::styled(
                        "  Use any git host (GitHub, GitLab, Bitbucket, etc.)",
                        t.text_style(),
                    )]),
                    Line::from(vec![Span::styled(
                        "  You set up the repo, dotstate just uses it.",
                        t.text_style(),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            icons.lightbulb(),
                            Style::default()
                                .fg(t.secondary)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            " Tip: ",
                            Style::default()
                                .fg(t.secondary)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            "Both options sync your dotfiles across machines!",
                            t.text_style(),
                        ),
                    ]),
                ];
                Text::from(lines)
            }
        }
    }

    /// Get the explanation panel icon (uses icon provider)
    pub fn explanation_icon(&self, icons: &Icons) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => icons.lightbulb(),
            MenuItem::SyncWithRemote => icons.sync(),
            MenuItem::ManageProfiles => icons.profile(),
            MenuItem::ManagePackages => icons.package(),
            MenuItem::SetupRepository => icons.git(),
        }
    }

    /// Get the explanation panel color
    pub fn explanation_color(&self) -> Color {
        theme().primary
    }

    /// Convert from index to MenuItem
    pub fn from_index(index: usize) -> Option<MenuItem> {
        Self::all().get(index).copied()
    }

    /// Convert MenuItem to index
    pub fn to_index(self) -> usize {
        Self::all()
            .iter()
            .position(|item| *item == self)
            .unwrap_or(0)
    }
}

use crate::services::git_service::GitStatus;

/// Main menu screen controller.
pub struct MainMenuScreen {
    selected_item: MenuItem,
    menu_state: MenuState,
    /// Clickable areas: (rect, MenuItem)
    clickable_areas: Vec<(Rect, MenuItem)>,
    /// Clickable area for update notification (shown as last menu item)
    update_clickable_area: Option<Rect>,
    /// Config for displaying stats
    config: Option<Config>,
    /// Detailed git status
    git_status: GitStatus,
    /// Update information if a new version is available
    update_info: Option<UpdateInfo>,
    /// Whether the update item is currently selected (instead of a menu item)
    is_update_selected: bool,
    /// Icon provider for rendering icons
    icons: Icons,
}

impl MainMenuScreen {
    /// Create a new main menu screen.
    pub fn new() -> Self {
        let mut menu_state = MenuState::new();
        let default_item = MenuItem::SetupRepository;
        let default_index = default_item.to_index();
        menu_state.select(Some(default_index));

        Self {
            selected_item: default_item,
            menu_state,
            clickable_areas: Vec::new(),
            update_clickable_area: None,
            config: None,
            git_status: GitStatus::default(),
            update_info: None,
            is_update_selected: false,
            icons: Icons::new(),
        }
    }

    /// Create and initialize with configuration.
    pub fn with_config(config: &Config, has_changes: bool) -> Self {
        let mut menu_state = MenuState::new();
        let default_item = if config.is_repo_configured() {
            MenuItem::ScanDotfiles
        } else {
            MenuItem::SetupRepository
        };
        let default_index = default_item.to_index();
        menu_state.select(Some(default_index));

        let git_status = GitStatus {
            has_changes,
            ..Default::default()
        };

        Self {
            selected_item: default_item,
            menu_state,
            clickable_areas: Vec::new(),
            update_clickable_area: None,
            config: Some(config.clone()),
            git_status,
            update_info: None,
            is_update_selected: false,
            icons: Icons::from_config(config),
        }
    }

    /// Initialize or reinitialize the screen with configuration.
    pub fn init_with_config(&mut self, config: &Config, has_changes: bool) {
        *self = Self::with_config(config, has_changes);
    }

    /// Set update information when a new version is available
    pub fn set_update_info(&mut self, info: Option<UpdateInfo>) {
        self.update_info = info;
    }

    /// Get the update info
    pub fn get_update_info(&self) -> Option<&UpdateInfo> {
        self.update_info.as_ref()
    }

    /// Check if the update menu item is currently selected
    pub fn is_update_item_selected(&self) -> bool {
        self.is_update_selected && self.update_info.is_some()
    }

    /// Get the total number of items (menu items + update item if available)
    fn total_items(&self) -> usize {
        let base = MenuItem::all().len();
        if self.update_info.is_some() {
            base + 1
        } else {
            base
        }
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        let menu_count = MenuItem::all();
        if self.is_update_selected {
            // Move from update item to last menu item
            self.is_update_selected = false;
            if let Some(item) = MenuItem::from_index(menu_count.len() - 1) {
                self.selected_item = item;
                let index = item.to_index();
                self.menu_state.select(Some(index));
            }
        } else {
            let current_index = self.selected_item.to_index();
            if current_index > 0 {
                if let Some(item) = MenuItem::from_index(current_index - 1) {
                    self.selected_item = item;
                    let index = item.to_index();
                    self.menu_state.select(Some(index));
                }
            } else {
                // cycle back to last menu item
                if let Some(item) = MenuItem::from_index(menu_count.len() - 1) {
                    self.selected_item = item;
                    let index = item.to_index();
                    self.menu_state.select(Some(index));
                }
            }
        }
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let menu_items = MenuItem::all();
        let menu_count = menu_items.len();
        let max_index = self.total_items().saturating_sub(1);

        if self.is_update_selected {
            // Already at the bottom
            return;
        }

        let current_index = self.selected_item.to_index();
        if current_index < max_index {
            if current_index < menu_count - 1 {
                // Move to next menu item
                if let Some(item) = MenuItem::from_index(current_index + 1) {
                    self.selected_item = item;
                    let index = item.to_index();
                    self.menu_state.select(Some(index));
                }
            } else if current_index == menu_count - 1 && self.update_info.is_some() {
                // Move from last menu item to update item
                self.is_update_selected = true;
                // Update menu_state to select the update item (which is at index menu_count)
                self.menu_state.select(Some(menu_count));
            }
        } else {
            // cycle back to first menu item
            if let Some(item) = MenuItem::from_index(0) {
                self.selected_item = item;
                let index = item.to_index();
                self.menu_state.select(Some(index));
            }
        }
    }

    /// Check if the app is set up (GitHub or Local mode configured)
    fn is_setup(&self) -> bool {
        self.config
            .as_ref()
            .map(|c| c.is_repo_configured())
            .unwrap_or(false)
    }

    /// Get the currently selected menu item
    pub fn selected_item(&self) -> MenuItem {
        self.selected_item
    }

    /// Set the selected item by MenuItem enum
    pub fn set_selected_item(&mut self, item: MenuItem) {
        self.selected_item = item;
        self.is_update_selected = false;
        let index = item.to_index();
        self.menu_state.select(Some(index));
    }

    /// Get the selected index (for backward compatibility)
    pub fn selected_index(&self) -> usize {
        self.selected_item.to_index()
    }

    pub fn set_git_status(&mut self, status: Option<GitStatus>) {
        if let Some(status) = status {
            self.git_status = status;
        } else {
            self.git_status = GitStatus::default();
        }
    }

    /// Update config (only updates config, doesn't change selection)
    pub fn update_config(&mut self, config: Config) {
        self.config = Some(config);
    }

    /// Get explanation text for selected menu item
    fn get_explanation(&self) -> Text<'static> {
        let t = theme();
        if self.is_update_item_selected() {
            if let Some(ref update_info) = self.update_info {
                let lines = vec![
                    Line::from(vec![Span::styled(
                        "Update Available!",
                        Style::default()
                            .fg(t.text_emphasis)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("A new version of DotState is available: ", t.text_style()),
                        Span::styled(
                            format!(
                                "{} → {}",
                                update_info.current_version, update_info.latest_version
                            ),
                            Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Update options:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(t.text_emphasis)),
                        Span::styled("Run: ", t.text_style()),
                        Span::styled("dotstate upgrade", Style::default().fg(t.primary)),
                    ]),
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(t.text_emphasis)),
                        Span::styled("Or: ", t.text_style()),
                        Span::styled(
                            "cargo install dotstate --force",
                            Style::default().fg(t.primary),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(t.text_emphasis)),
                        Span::styled("Or: ", t.text_style()),
                        Span::styled("brew upgrade dotstate", Style::default().fg(t.primary)),
                    ]),
                ];
                return Text::from(lines);
            }
        }
        self.selected_item.explanation(&self.icons)
    }

    /// Get stats text based on config
    fn get_stats(&self) -> String {
        use crate::config::RepoMode;

        if let Some(ref config) = self.config {
            if !config.is_repo_configured() {
                return "Please complete setup to see status".to_string();
            }

            // Get stats from manifest
            let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
                .unwrap_or_default();
            let synced_count = manifest
                .profiles
                .iter()
                .find(|p| p.name == config.active_profile)
                .map(|p| p.synced_files.len())
                .unwrap_or(0);
            let profile_count = manifest.profiles.len();
            let active_profile = &config.active_profile;

            // Show different info based on repo mode
            let repo_info = match config.repo_mode {
                RepoMode::GitHub => format!("Repository: {}", config.repo_name),
                RepoMode::Local => format!("Repository: {} (local)", config.repo_path.display()),
            };

            let mut stats = format!(
                "Synced Files: {}\nProfiles: {} (Active: {})\n{}",
                synced_count, profile_count, active_profile, repo_info
            );

            // Add git status info
            let status = &self.git_status;

            // Show ahead/behind info
            if status.ahead > 0 || status.behind > 0 {
                stats.push_str("\n\nRemote Status:");
                if status.behind > 0 {
                    stats.push_str(&format!(
                        "\n  ↓ {} commit(s) behind (pull needed)",
                        status.behind
                    ));
                }
                if status.ahead > 0 {
                    stats.push_str(&format!(
                        "\n  ↑ {} commit(s) ahead (push needed)",
                        status.ahead
                    ));
                }
            }

            // Add pending changes if any
            if !status.uncommitted_files.is_empty() {
                stats.push_str(&format!(
                    "\n\nPending Changes ({}):",
                    status.uncommitted_files.len()
                ));
                // Show first few files (limit to avoid overflow)
                let max_files = 5.min(status.uncommitted_files.len());
                for file in status.uncommitted_files.iter().take(max_files) {
                    // Remove status prefix (A, M, D) for display, or show it
                    let display_file = if file.len() > 2 && file.chars().nth(1) == Some(' ') {
                        &file[2..] // Skip "A ", "M ", "D "
                    } else {
                        file
                    };
                    stats.push_str(&format!("\n  • {}", display_file));
                }
                if status.uncommitted_files.len() > max_files {
                    stats.push_str(&format!(
                        "\n  ... and {} more",
                        status.uncommitted_files.len() - max_files
                    ));
                }
            } else if status.ahead == 0 && status.behind == 0 && status.has_changes {
                // Fallback if has_changes is true but list is empty (shouldn't happen but safe to handle)
                stats.push_str("\n\nPending Changes: Yes");
            }

            stats
        } else {
            "Please complete setup to see status".to_string()
        }
    }

    /// Render the main menu screen
    fn render_internal(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background - use theme background
        let t = theme();
        let background = Block::default().style(t.background_style());
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component (with logo on left)
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Dotfile Manager",
            "Manage your dotfiles with ease. Sync to GitHub, organize by profiles, and keep your configuration files safe."
        )?;

        // Split content into left and right panels
        let content_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Left panel
                Constraint::Percentage(50), // Right panel
            ])
            .split(content_chunk);

        // Menu items
        let menu_items = MenuItem::all();
        let is_setup = self.is_setup();

        // Convert to Menu widget items
        let mut widget_items: Vec<MenuWidgetItem> = menu_items
            .iter()
            .map(|menu_item| {
                let icon = menu_item.icon(&self.icons);
                let text = menu_item.text();
                let is_enabled = menu_item.is_enabled(is_setup);
                let has_action_needed = self.git_status.has_changes
                    || self.git_status.ahead > 0
                    || self.git_status.behind > 0;
                let color = menu_item.color(has_action_needed);

                let mut item = MenuWidgetItem::new(icon, text, color).enabled(is_enabled);

                // Add info for sync item if there are pending changes
                if *menu_item == MenuItem::SyncWithRemote && has_action_needed && is_enabled {
                    let mut info_parts = Vec::new();
                    if self.git_status.behind > 0 {
                        info_parts.push(format!("↓{}", self.git_status.behind));
                    }
                    if self.git_status.ahead > 0 {
                        info_parts.push(format!("↑{}", self.git_status.ahead));
                    }
                    if self.git_status.has_changes {
                        let count = self.git_status.uncommitted_files.len();
                        if count > 0 {
                            info_parts.push(format!("+{}", count));
                        } else {
                            info_parts.push("*".to_string());
                        }
                    }

                    if !info_parts.is_empty() {
                        item = item.info(info_parts.join(" "));
                    }
                }

                item
            })
            .collect();

        // Add update item if there's an update available
        if let Some(ref update_info) = self.update_info {
            let update_text = format!(
                "Update available: {} → {}",
                update_info.current_version, update_info.latest_version
            );
            widget_items.push(
                MenuWidgetItem::new(self.icons.update(), &update_text, t.text_emphasis)
                    .enabled(true),
            );
        }

        let menu_block = Block::default()
            .borders(Borders::ALL)
            .border_style(t.border_focused_style())
            .border_type(t.border_type(false))
            .title(format!(" {} Menu ", self.icons.menu()))
            .title_style(t.title_style())
            .title_alignment(Alignment::Center);

        let menu_inner = menu_block.inner(content_split[0]);

        // Create the menu widget (no highlight symbol, we'll use left border instead)
        let menu = Menu::new(widget_items);

        // Store clickable areas (3 lines per item)
        self.clickable_areas.clear();
        let clickable_areas = menu.clickable_areas(menu_inner);
        for (rect, index) in clickable_areas {
            if index < menu_items.len() {
                self.clickable_areas.push((rect, menu_items[index]));
            } else {
                // This is the update item
                self.update_clickable_area = Some(rect);
            }
        }

        // Render the block
        frame.render_widget(menu_block, content_split[0]);

        // Render the menu inside the block
        StatefulWidget::render(menu, menu_inner, frame.buffer_mut(), &mut self.menu_state);

        // Right panel: Explanation and stats
        let right_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60), // Explanation
                Constraint::Percentage(40), // Stats
            ])
            .split(content_split[1]);

        // Explanation block with colorful styling
        let (icon, color) = if self.is_update_item_selected() {
            (self.icons.update(), t.text_emphasis)
        } else {
            (
                self.selected_item.explanation_icon(&self.icons),
                self.selected_item.explanation_color(),
            )
        };

        let explanation_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
            .border_type(t.border_type(false))
            .title(format!(" {} What does this do? ", icon))
            .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let explanation_para = Paragraph::new(self.get_explanation())
            .wrap(Wrap { trim: true })
            .block(explanation_block);

        frame.render_widget(explanation_para, right_split[0]);

        // Stats block with colorful styling
        let has_pending =
            self.git_status.has_changes || self.git_status.ahead > 0 || self.git_status.behind > 0;
        let stats_color = if has_pending { t.warning } else { t.success };
        let stats_icon = if has_pending {
            self.icons.warning()
        } else {
            self.icons.success()
        };

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(stats_color))
            .border_type(t.border_type(false))
            .title(format!(" {} Status ", stats_icon))
            .title_style(
                Style::default()
                    .fg(stats_color)
                    .add_modifier(Modifier::BOLD),
            )
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        // Format stats with colors
        let stats_text = self.get_stats();
        let stats_lines: Vec<Line> = stats_text
            .lines()
            .map(|line| {
                if line.starts_with("Synced Files:") {
                    Line::from(vec![
                        Span::styled(
                            "Synced Files: ",
                            Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            line.strip_prefix("Synced Files: ").unwrap_or(""),
                            t.text_style(),
                        ),
                    ])
                } else if line.starts_with("Profiles:") {
                    Line::from(vec![
                        Span::styled(
                            "Profiles: ",
                            Style::default()
                                .fg(t.secondary)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            line.strip_prefix("Profiles: ").unwrap_or(""),
                            t.text_style(),
                        ),
                    ])
                } else if line.starts_with("Repository:") {
                    Line::from(vec![
                        Span::styled(
                            "Repository: ",
                            Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            line.strip_prefix("Repository: ").unwrap_or(""),
                            t.text_style(),
                        ),
                    ])
                } else if line.starts_with("Pending Changes") {
                    Line::from(vec![Span::styled(
                        line,
                        Style::default().fg(t.warning).add_modifier(Modifier::BOLD),
                    )])
                } else if line.starts_with("  •") {
                    Line::from(vec![
                        Span::styled("  • ", Style::default().fg(t.warning)),
                        Span::styled(line.strip_prefix("  • ").unwrap_or(""), t.text_style()),
                    ])
                } else if line.contains("... and") {
                    Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(t.text_muted)
                            .add_modifier(Modifier::ITALIC),
                    )])
                } else {
                    Line::from(Span::styled(line, t.text_style()))
                }
            })
            .collect();

        let stats_para = Paragraph::new(stats_lines)
            .wrap(Wrap { trim: true })
            .block(stats_block);

        frame.render_widget(stats_para, right_split[1]);

        // Footer with dynamic keybindings from keymap
        let footer_text = self
            .config
            .as_ref()
            .map(|c| {
                let t = theme();
                let theme_name = match t.theme_type {
                    crate::styles::ThemeType::Dark => "dark",
                    crate::styles::ThemeType::Light => "light",
                    crate::styles::ThemeType::SolarizedDark => "solarized-dark",
                    crate::styles::ThemeType::SolarizedLight => "solarized-light",
                    crate::styles::ThemeType::NoColor => "nocolor",
                    crate::styles::ThemeType::Midnight => "midnight",
                };
                c.keymap.footer_navigation(theme_name)
            })
            .unwrap_or_else(|| {
                "↑↓: Navigate | Enter: Select | q: Back | ?: Help | t: Theme".to_string()
            });
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    /// Handle menu item selection and return the appropriate action.
    fn handle_selection(&self, ctx: &ScreenContext) -> Result<ScreenAction> {
        if self.is_update_item_selected() {
            // Return action to show update info popup
            if let Some(info) = self.get_update_info() {
                let (title, content) = self.build_update_message(info);
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

    /// Build the update message from UpdateInfo.
    fn build_update_message(&self, info: &UpdateInfo) -> (String, String) {
        let title = format!(
            "{} Version {} Available!",
            self.icons.update(),
            info.latest_version
        );
        let content = format!(
            "{} New version available: {} → {}\n\n\
            Update options:\n\n\
            1. Using install script:\n\
            curl -fsSL {} | bash\n\n\
            2. Using Cargo:\n\
            cargo install dotstate --force\n\n\
            3. Using Homebrew:\n\
            brew upgrade dotstate\n\n\
            Visit release page for details.",
            self.icons.update(),
            info.current_version,
            info.latest_version,
            info.release_url
                .replace("/releases/tag/", "/releases/download/")
                .replace(
                    &info.latest_version,
                    &format!("{}/install.sh", info.latest_version)
                )
        );
        (title, content)
    }

    /// Handle mouse events
    fn handle_mouse_event(&mut self, event: Event) -> Result<bool> {
        let is_setup = self.is_setup();

        if let Event::Mouse(mouse) = event {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Check if click is on update item
                    if let Some(ref update_rect) = self.update_clickable_area {
                        if mouse.column >= update_rect.x
                            && mouse.column < update_rect.x + update_rect.width
                            && mouse.row >= update_rect.y
                            && mouse.row < update_rect.y + update_rect.height
                        {
                            self.is_update_selected = true;
                            // Update menu_state to select the update item
                            let menu_count = MenuItem::all().len();
                            self.menu_state.select(Some(menu_count));
                            return Ok(true); // Selection made
                        }
                    }

                    // Check if click is in any menu clickable area
                    let clicked_menu_item = self
                        .clickable_areas
                        .iter()
                        .find(|(rect, _)| {
                            mouse.column >= rect.x
                                && mouse.column < rect.x + rect.width
                                && mouse.row >= rect.y
                                && mouse.row < rect.y + rect.height
                        })
                        .map(|(_, menu_item)| *menu_item);

                    if let Some(menu_item) = clicked_menu_item {
                        self.set_selected_item(menu_item);
                        // Only trigger action if item is enabled
                        if menu_item.is_enabled(is_setup) {
                            return Ok(true); // Selection made
                        }
                    }
                }
                MouseEventKind::ScrollUp => {
                    self.move_up();
                }
                MouseEventKind::ScrollDown => {
                    self.move_down();
                }
                _ => {}
            }
        }
        Ok(false)
    }
}

impl Default for MainMenuScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for MainMenuScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &RenderContext) -> Result<()> {
        self.render_internal(frame, area)
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
                            self.move_up();
                            return Ok(ScreenAction::None);
                        }
                        Action::MoveDown => {
                            self.move_down();
                            return Ok(ScreenAction::None);
                        }
                        Action::Confirm => {
                            // Check for update item selection
                            if self.is_update_item_selected() {
                                if let Some(info) = self.get_update_info() {
                                    let (title, content) = self.build_update_message(info);
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

        // Handle mouse events
        if matches!(event, Event::Mouse(_)) && self.handle_mouse_event(event)? {
            // Mouse click triggered selection
            return self.handle_selection(ctx);
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        false // Main menu has no text inputs
    }

    fn on_enter(&mut self, ctx: &ScreenContext) -> Result<()> {
        // Re-initialize when entering the screen
        self.init_with_config(ctx.config, false);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            repo_path: PathBuf::from("/tmp/test-repo"),
            ..Default::default()
        }
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
