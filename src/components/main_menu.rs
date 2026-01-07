use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::config::Config;
use crate::utils::create_standard_layout;
use crate::version_check::UpdateInfo;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, StatefulWidget,
    Wrap,
};

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

    /// Get the icon for this menu item
    pub fn icon(&self) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => "📁",
            MenuItem::SyncWithRemote => "🔄",
            MenuItem::ManageProfiles => "👤",
            MenuItem::ManagePackages => "📦",
            MenuItem::SetupRepository => "🔧",
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
        match self {
            MenuItem::SyncWithRemote if has_changes => Color::Yellow,
            _ => Color::White,
        }
    }

    /// Get the explanation text for this menu item
    pub fn explanation(&self) -> Text<'static> {
        match self {
            MenuItem::ScanDotfiles => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Manage Your Dotfiles", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Keep your configuration files (like "),
                        Span::styled(".zshrc", Style::default().fg(Color::Yellow)),
                        Span::raw(", "),
                        Span::styled(".vimrc", Style::default().fg(Color::Yellow)),
                        Span::raw(", "),
                        Span::styled(".gitconfig", Style::default().fg(Color::Yellow)),
                        Span::raw(", etc.) synchronized across all your machines. "),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("When you select a file, it's automatically "),
                        Span::styled("copied to your repository", Style::default().fg(Color::Green)),
                        Span::raw(" and a "),
                        Span::styled("symlink", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" is created in its place. This means your files are safely backed up and version controlled."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 Tip: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw("You can add custom files using the file browser, or use the CLI:\n"),
                        Span::styled("  dotstate add ~/.myconfig", Style::default().fg(Color::Yellow)),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::SyncWithRemote => {
                let repo_name = crate::config::default_repo_name();
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Sync with Remote Repository", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Keep your dotfiles synchronized across all your devices. This feature "),
                        Span::styled("commits", Style::default().fg(Color::Cyan)),
                        Span::raw(" your local changes, "),
                        Span::styled("pulls", Style::default().fg(Color::Blue)),
                        Span::raw(" any updates from the remote, and "),
                        Span::styled("pushes", Style::default().fg(Color::Green)),
                        Span::raw(" everything back up."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Perfect for when you've made changes on one computer and want to sync them to another. "),
                        Span::styled("All changes are automatically merged", Style::default().fg(Color::Green)),
                        Span::raw(" with your remote repository."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Your repository is called "),
                        Span::styled(repo_name.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" and should be visible in your GitHub account."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 CLI: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::styled("dotstate sync", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Same functionality from the command line"),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::ManageProfiles => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Manage Multiple Profiles", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Create different sets of dotfiles for different contexts. Perfect for managing "),
                        Span::styled("work", Style::default().fg(Color::Blue)),
                        Span::raw(" vs "),
                        Span::styled("personal", Style::default().fg(Color::Green)),
                        Span::raw(" configurations, or different "),
                        Span::styled("operating systems", Style::default().fg(Color::Cyan)),
                        Span::raw("."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Example: Switch between a "),
                        Span::styled("Mac", Style::default().fg(Color::Yellow)),
                        Span::raw(" profile with macOS-specific settings and a "),
                        Span::styled("Linux", Style::default().fg(Color::Green)),
                        Span::raw(" profile for your servers."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Each profile maintains its own set of synced files and packages, so you can keep everything organized and context-specific."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 CLI: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::styled("dotstate profile list", Style::default().fg(Color::Yellow)),
                        Span::raw(" - List all profiles\n"),
                        Span::styled("  dotstate profile switch <name>", Style::default().fg(Color::Yellow)),
                        Span::raw(" - Switch between profiles"),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::ManagePackages => {
                let lines = vec![
                    Line::from(vec![
                        Span::styled("Manage CLI Tools & Dependencies", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Ensure all your essential command-line tools are installed across your machines. "),
                        Span::styled("Automatically detect", Style::default().fg(Color::Cyan)),
                        Span::raw(" which packages are missing and "),
                        Span::styled("install them with one command", Style::default().fg(Color::Green)),
                        Span::raw("."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Great for setting up a new machine quickly! Just sync your dotfiles and install all your tools (like "),
                        Span::styled("git", Style::default().fg(Color::Yellow)),
                        Span::raw(", "),
                        Span::styled("vim", Style::default().fg(Color::Yellow)),
                        Span::raw(", "),
                        Span::styled("node", Style::default().fg(Color::Yellow)),
                        Span::raw(", etc.) in one go."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Supports both "),
                        Span::styled("managed packages", Style::default().fg(Color::Green)),
                        Span::raw(" (auto-detected from common package managers) and "),
                        Span::styled("custom packages", Style::default().fg(Color::Cyan)),
                        Span::raw(" with custom installation commands."),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 Example: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw("Add packages like "),
                        Span::styled("ripgrep", Style::default().fg(Color::Yellow)),
                        Span::raw(" or "),
                        Span::styled("fzf", Style::default().fg(Color::Yellow)),
                        Span::raw(" to your profile, and they'll be installed automatically on new machines."),
                    ]),
                ];
                Text::from(lines)
            }
            MenuItem::SetupRepository => {
                let lines = vec![
                    Line::from(vec![Span::styled(
                        "Setup Git Repository",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("Configure a git repository to store and sync your dotfiles. "),
                        Span::styled(
                            "Choose how you want to set up:",
                            Style::default().fg(Color::Cyan),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Option 1: ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Create for me (GitHub)", Style::default().fg(Color::Green)),
                    ]),
                    Line::from(vec![Span::raw(
                        "  Automatically create a repository on GitHub.",
                    )]),
                    Line::from(vec![Span::raw(
                        "  Requires a GitHub Personal Access Token.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "Option 2: ",
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Use my own repository", Style::default().fg(Color::Blue)),
                    ]),
                    Line::from(vec![Span::raw(
                        "  Use any git host (GitHub, GitLab, Bitbucket, etc.)",
                    )]),
                    Line::from(vec![Span::raw(
                        "  You set up the repo, dotstate just uses it.",
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "💡 Tip: ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw("Both options sync your dotfiles across machines!"),
                    ]),
                ];
                Text::from(lines)
            }
        }
    }

    /// Get the explanation panel icon
    pub fn explanation_icon(&self) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => "💡",
            MenuItem::SyncWithRemote => "🔄",
            MenuItem::ManageProfiles => "👤",
            MenuItem::ManagePackages => "📦",
            MenuItem::SetupRepository => "🔧",
        }
    }

    /// Get the explanation panel color
    pub fn explanation_color(&self) -> Color {
        Color::Cyan
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

/// Main menu component (also serves as welcome screen)
pub struct MainMenuComponent {
    selected_item: MenuItem, // Use MenuItem enum instead of index
    has_changes_to_push: bool,
    list_state: ListState,
    /// Clickable areas: (rect, MenuItem)
    clickable_areas: Vec<(Rect, MenuItem)>,
    /// Clickable area for update notification (shown as last menu item)
    update_clickable_area: Option<Rect>,
    /// Config for displaying stats
    config: Option<Config>,
    /// Changed files pending sync
    changed_files: Vec<String>,
    /// Update information if a new version is available
    update_info: Option<UpdateInfo>,
    /// Selected index (can be > menu items count if update item is selected)
    selected_index: usize,
}

impl MainMenuComponent {
    pub fn new(has_changes_to_push: bool) -> Self {
        let mut list_state = ListState::default();
        // Default to SetupRepository if not set up, otherwise first item
        let default_item = MenuItem::SetupRepository;
        let default_index = default_item.to_index();
        list_state.select(Some(default_index));

        Self {
            selected_item: default_item,
            has_changes_to_push,
            list_state,
            clickable_areas: Vec::new(),
            update_clickable_area: None,
            config: None,
            changed_files: Vec::new(),
            update_info: None,
            selected_index: default_index,
        }
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
    fn is_update_item_selected(&self) -> bool {
        self.update_info.is_some() && self.selected_index == MenuItem::all().len()
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
        let index = item.to_index();
        self.list_state.select(Some(index));
    }

    /// Set the selected item by index (for backward compatibility)
    pub fn set_selected(&mut self, index: usize) {
        if let Some(item) = MenuItem::from_index(index) {
            self.set_selected_item(item);
        }
    }

    /// Get the selected index (for backward compatibility)
    pub fn selected_index(&self) -> usize {
        self.selected_item.to_index()
    }

    pub fn set_has_changes_to_push(&mut self, has_changes: bool) {
        self.has_changes_to_push = has_changes;
    }

    pub fn update_config(&mut self, config: Config) {
        self.config = Some(config);
    }

    pub fn update_changed_files(&mut self, changed_files: Vec<String>) {
        self.changed_files = changed_files;
    }

    /// Get explanation text for selected menu item
    fn get_explanation(&self) -> Text<'static> {
        if self.is_update_item_selected() {
            if let Some(ref update_info) = self.update_info {
                let lines = vec![
                    Line::from(vec![Span::styled(
                        "Update Available!",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw("A new version of DotState is available: "),
                        Span::styled(
                            format!(
                                "{} → {}",
                                update_info.current_version, update_info.latest_version
                            ),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Update options:",
                        Style::default().add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(Color::Yellow)),
                        Span::raw("Run: "),
                        Span::styled("dotstate upgrade", Style::default().fg(Color::Cyan)),
                    ]),
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(Color::Yellow)),
                        Span::raw("Or: "),
                        Span::styled(
                            "cargo install dotstate --force",
                            Style::default().fg(Color::Cyan),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(Color::Yellow)),
                        Span::raw("Or: "),
                        Span::styled("brew upgrade dotstate", Style::default().fg(Color::Cyan)),
                    ]),
                ];
                return Text::from(lines);
            }
        }
        self.selected_item.explanation()
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

            // Add pending changes if any
            if !self.changed_files.is_empty() {
                stats.push_str(&format!(
                    "\n\nPending Changes ({}):",
                    self.changed_files.len()
                ));
                // Show first few files (limit to avoid overflow)
                let max_files = 5.min(self.changed_files.len());
                for file in self.changed_files.iter().take(max_files) {
                    // Remove status prefix (A, M, D) for display, or show it
                    let display_file = if file.len() > 2 && file.chars().nth(1) == Some(' ') {
                        &file[2..] // Skip "A ", "M ", "D "
                    } else {
                        file
                    };
                    stats.push_str(&format!("\n  • {}", display_file));
                }
                if self.changed_files.len() > max_files {
                    stats.push_str(&format!(
                        "\n  ... and {} more",
                        self.changed_files.len() - max_files
                    ));
                }
            }

            stats
        } else {
            "Please complete setup to see status".to_string()
        }
    }
}

impl Component for MainMenuComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default().style(Style::default().bg(Color::Black));
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

        // Menu items - now using MenuItem enum
        let menu_items = MenuItem::all();
        let is_setup = self.is_setup();
        let update_item_index = menu_items.len(); // Index where update item would be

        let mut items: Vec<ListItem> = menu_items
            .iter()
            .enumerate()
            .map(|(i, menu_item)| {
                let icon = menu_item.icon();
                let text = menu_item.text();
                let is_enabled = menu_item.is_enabled(is_setup);

                // Determine color based on enabled state
                let color = if !is_enabled {
                    Color::DarkGray // Disabled items in dark gray
                } else {
                    menu_item.color(self.has_changes_to_push)
                };

                let display_text = if *menu_item == MenuItem::SyncWithRemote
                    && self.has_changes_to_push
                    && is_enabled
                {
                    format!("{} {} ({} pending)", icon, text, self.changed_files.len())
                } else if !is_enabled {
                    format!("{} {} (requires setup)", icon, text)
                } else {
                    format!("{} {}", icon, text)
                };

                let style = if i == self.selected_index {
                    if is_enabled {
                        Style::default().fg(color).add_modifier(Modifier::BOLD)
                    } else {
                        // Disabled items stay gray even when selected
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default().fg(color)
                };
                ListItem::new(display_text).style(style)
            })
            .collect();

        // Add update item if there's an update available
        if let Some(ref update_info) = self.update_info {
            let is_selected = self.selected_index == update_item_index;
            let update_text = format!(
                "🎉 Update available: {} → {}",
                update_info.current_version, update_info.latest_version
            );
            let style = if is_selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            items.push(ListItem::new(update_text).style(style));
        }

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("📋 Menu")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        // Store clickable area for mouse support
        self.clickable_areas.clear();
        let list_inner = list_block.inner(content_split[0]);

        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol("» ");

        let item_height = 1;
        for (i, menu_item) in menu_items.iter().enumerate() {
            let y = list_inner.y + i as u16;
            if y < list_inner.y + list_inner.height {
                self.clickable_areas.push((
                    Rect::new(list_inner.x, y, list_inner.width, item_height),
                    *menu_item,
                ));
            }
        }

        // Add clickable area for update item if present
        if self.update_info.is_some() {
            let y = list_inner.y + menu_items.len() as u16;
            if y < list_inner.y + list_inner.height {
                self.update_clickable_area =
                    Some(Rect::new(list_inner.x, y, list_inner.width, item_height));
            }
        } else {
            self.update_clickable_area = None;
        }

        // Update list_state selection
        self.list_state.select(Some(self.selected_index));

        // Render list
        StatefulWidget::render(
            list,
            content_split[0],
            frame.buffer_mut(),
            &mut self.list_state,
        );

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
            ("🎉", Color::Yellow)
        } else {
            (
                self.selected_item.explanation_icon(),
                self.selected_item.explanation_color(),
            )
        };

        let explanation_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(format!("{} What does this do?", icon))
            .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let explanation_para = Paragraph::new(self.get_explanation())
            .wrap(Wrap { trim: true })
            .block(explanation_block);

        frame.render_widget(explanation_para, right_split[0]);

        // Stats block with colorful styling
        let has_pending = !self.changed_files.is_empty();
        let stats_color = if has_pending {
            Color::Yellow
        } else {
            Color::Green
        };
        let stats_icon = if has_pending { "⚠️" } else { "✅" };

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(stats_color))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(format!("{} Status", stats_icon))
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
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            line.strip_prefix("Synced Files: ").unwrap_or(""),
                            Style::default().fg(Color::White),
                        ),
                    ])
                } else if line.starts_with("Profiles:") {
                    Line::from(vec![
                        Span::styled(
                            "Profiles: ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            line.strip_prefix("Profiles: ").unwrap_or(""),
                            Style::default().fg(Color::White),
                        ),
                    ])
                } else if line.starts_with("Repository:") {
                    Line::from(vec![
                        Span::styled(
                            "Repository: ",
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            line.strip_prefix("Repository: ").unwrap_or(""),
                            Style::default().fg(Color::White),
                        ),
                    ])
                } else if line.starts_with("Pending Changes") {
                    Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )])
                } else if line.starts_with("  •") {
                    Line::from(vec![
                        Span::styled("  • ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            line.strip_prefix("  • ").unwrap_or(""),
                            Style::default().fg(Color::White),
                        ),
                    ])
                } else if line.contains("... and") {
                    Line::from(vec![Span::styled(
                        line,
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::ITALIC),
                    )])
                } else {
                    Line::from(Span::styled(line, Style::default().fg(Color::White)))
                }
            })
            .collect();

        let stats_para = Paragraph::new(stats_lines)
            .wrap(Wrap { trim: true })
            .block(stats_block);

        frame.render_widget(stats_para, right_split[1]);

        // Footer
        let _ = Footer::render(
            frame,
            footer_chunk,
            "↑↓: Navigate | Enter/Click: Select | q: Quit",
        )?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        let menu_items = MenuItem::all();
        let menu_count = menu_items.len();
        let max_index = self.total_items().saturating_sub(1);
        let is_setup = self.is_setup();

        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Up => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                            // Update selected_item if within menu items range
                            if self.selected_index < menu_count {
                                if let Some(item) = MenuItem::from_index(self.selected_index) {
                                    self.selected_item = item;
                                }
                            }
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    KeyCode::Down => {
                        if self.selected_index < max_index {
                            self.selected_index += 1;
                            // Update selected_item if within menu items range
                            if self.selected_index < menu_count {
                                if let Some(item) = MenuItem::from_index(self.selected_index) {
                                    self.selected_item = item;
                                }
                            }
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    KeyCode::Enter => {
                        if self.is_update_item_selected() {
                            // Trigger update action
                            Ok(ComponentAction::Custom("show_update_info".to_string()))
                        } else if self.selected_item.is_enabled(is_setup) {
                            Ok(ComponentAction::Update) // App will handle selection
                        } else {
                            Ok(ComponentAction::None) // Ignore Enter on disabled items
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => Ok(ComponentAction::Quit),
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Check if click is on update item
                        if let Some(ref update_rect) = self.update_clickable_area {
                            if mouse.column >= update_rect.x
                                && mouse.column < update_rect.x + update_rect.width
                                && mouse.row >= update_rect.y
                                && mouse.row < update_rect.y + update_rect.height
                            {
                                self.selected_index = menu_count; // Update item index
                                return Ok(ComponentAction::Custom("show_update_info".to_string()));
                            }
                        }

                        // Check if click is in any menu clickable area
                        for (rect, menu_item) in &self.clickable_areas {
                            if mouse.column >= rect.x
                                && mouse.column < rect.x + rect.width
                                && mouse.row >= rect.y
                                && mouse.row < rect.y + rect.height
                            {
                                self.selected_item = *menu_item;
                                self.selected_index = menu_item.to_index();
                                // Only trigger action if item is enabled
                                if menu_item.is_enabled(is_setup) {
                                    return Ok(ComponentAction::Update);
                                } else {
                                    // Just select it, don't trigger action
                                    return Ok(ComponentAction::None);
                                }
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                            if self.selected_index < menu_count {
                                if let Some(item) = MenuItem::from_index(self.selected_index) {
                                    self.selected_item = item;
                                }
                            }
                            return Ok(ComponentAction::Update);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if self.selected_index < max_index {
                            self.selected_index += 1;
                            if self.selected_index < menu_count {
                                if let Some(item) = MenuItem::from_index(self.selected_index) {
                                    self.selected_item = item;
                                }
                            }
                            return Ok(ComponentAction::Update);
                        }
                    }
                    _ => {}
                }
                Ok(ComponentAction::None)
            }
            _ => Ok(ComponentAction::None),
        }
    }
}
