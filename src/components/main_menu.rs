use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::config::Config;
use crate::utils::create_standard_layout;

/// Menu items enum - defines the order and available menu options
/// This is the single source of truth for menu items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    ScanDotfiles,
    PushChanges,
    PullChanges,
    ManageProfiles,
    ManagePackages,
    SetupGitHub,
}

impl MenuItem {
    /// Get all menu items in display order
    pub fn all() -> Vec<MenuItem> {
        vec![
            MenuItem::ScanDotfiles,
            MenuItem::PushChanges,
            MenuItem::PullChanges,
            MenuItem::ManageProfiles,
            MenuItem::ManagePackages,
            MenuItem::SetupGitHub,
        ]
    }

    /// Check if this menu item requires GitHub setup
    pub fn requires_setup(&self) -> bool {
        match self {
            MenuItem::SetupGitHub => false, // Always available
            _ => true, // All other items require setup
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
            MenuItem::PushChanges => "📤",
            MenuItem::PullChanges => "📥",
            MenuItem::ManageProfiles => "👤",
            MenuItem::ManagePackages => "📦",
            MenuItem::SetupGitHub => "🔧",
        }
    }

    /// Get the display text for this menu item
    pub fn text(&self) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => "Scan & Select Dotfiles",
            MenuItem::PushChanges => "Push Changes",
            MenuItem::PullChanges => "Pull Changes",
            MenuItem::ManageProfiles => "Manage Profiles",
            MenuItem::ManagePackages => "Manage Packages",
            MenuItem::SetupGitHub => "Setup GitHub Repository",
        }
    }

    /// Get the base color for this menu item
    pub fn color(&self, has_changes: bool) -> Color {
        match self {
            MenuItem::PushChanges if has_changes => Color::Yellow,
            _ => Color::White,
        }
    }

    /// Get the explanation text for this menu item
    pub fn explanation(&self) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => "Scan your home directory for common dotfiles and configuration files. Preview their contents and select which ones you want to sync to GitHub.",
            MenuItem::PushChanges => "Commit and push any local changes to your GitHub repository. This will update your remote repository with the latest dotfile changes.",
            MenuItem::PullChanges => "Pull the latest changes from your GitHub repository. This will update your local repository with any changes made on other computers.",
            MenuItem::ManageProfiles => "Manage different profiles or sets of dotfiles. Create profiles for work, personal, different operating systems, etc.",
            MenuItem::ManagePackages => "Manage CLI tools and dependencies for your profile. Check if packages are installed and optionally install missing ones.",
            MenuItem::SetupGitHub => "Connect your GitHub account and create a repository to store your dotfiles. This will allow you to sync your configuration files across multiple computers.",
        }
    }

    /// Get the explanation panel icon
    pub fn explanation_icon(&self) -> &'static str {
        match self {
            MenuItem::ScanDotfiles => "💡",
            MenuItem::PushChanges => "📤",
            MenuItem::PullChanges => "📥",
            MenuItem::ManageProfiles => "👤",
            MenuItem::ManagePackages => "📦",
            MenuItem::SetupGitHub => "🔧",
        }
    }

    /// Get the explanation panel color
    pub fn explanation_color(&self) -> Color {
        match self {
            MenuItem::ScanDotfiles => Color::Cyan,
            MenuItem::PushChanges => Color::Green,
            MenuItem::PullChanges => Color::Cyan,
            MenuItem::ManageProfiles => Color::Magenta,
            MenuItem::ManagePackages => Color::Blue,
            MenuItem::SetupGitHub => Color::Cyan,
        }
    }

    /// Convert from index to MenuItem
    pub fn from_index(index: usize) -> Option<MenuItem> {
        Self::all().get(index).copied()
    }

    /// Convert MenuItem to index
    pub fn to_index(&self) -> usize {
        Self::all().iter().position(|item| item == self).unwrap_or(0)
    }
}

/// Main menu component (also serves as welcome screen)
pub struct MainMenuComponent {
    selected_item: MenuItem,  // Use MenuItem enum instead of index
    has_changes_to_push: bool,
    list_state: ListState,
    /// Clickable areas: (rect, MenuItem)
    clickable_areas: Vec<(Rect, MenuItem)>,
    /// Config for displaying stats
    config: Option<Config>,
    /// Changed files pending sync
    changed_files: Vec<String>,
}

impl MainMenuComponent {
    pub fn new(has_changes_to_push: bool) -> Self {
        let mut list_state = ListState::default();
        // Default to SetupGitHub if not set up, otherwise first item
        let default_item = MenuItem::SetupGitHub;
        list_state.select(Some(default_item.to_index()));

        Self {
            selected_item: default_item,
            has_changes_to_push,
            list_state,
            clickable_areas: Vec::new(),
            config: None,
            changed_files: Vec::new(),
        }
    }

    /// Check if the app is set up (GitHub configured)
    fn is_setup(&self) -> bool {
        self.config.as_ref().and_then(|c| c.github.as_ref()).is_some()
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
    fn get_explanation(&self) -> &'static str {
        self.selected_item.explanation()
    }

    /// Get stats text based on config
    fn get_stats(&self) -> String {
        if let Some(ref config) = self.config {
            if config.github.is_none() {
                return "Please complete setup to see status".to_string();
            }

            // Get stats from manifest
            let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
                .unwrap_or_default();
            let synced_count = manifest.profiles.iter()
                .find(|p| p.name == config.active_profile)
                .map(|p| p.synced_files.len())
                .unwrap_or(0);
            let profile_count = manifest.profiles.len();
            let active_profile = &config.active_profile;

            let mut stats = format!(
                "Synced Files: {}\nProfiles: {} (Active: {})\nRepository: {}",
                synced_count,
                profile_count,
                active_profile,
                config.repo_name
            );

            // Add pending changes if any
            if !self.changed_files.is_empty() {
                stats.push_str(&format!("\n\nPending Changes ({}):", self.changed_files.len()));
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
                    stats.push_str(&format!("\n  ... and {} more", self.changed_files.len() - max_files));
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
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component (with logo on left)
        let _ = Header::render(
            frame,
            header_chunk,
            "dotstate - Dotfile Manager",
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

        // Left panel: split vertically (welcome message on top, menu on bottom)
        let left_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(6), // Welcome message
                Constraint::Min(0),     // Menu items
            ])
            .split(content_split[0]);

        // Welcome message block with colorful styling
        let is_setup = self.config.as_ref().and_then(|c| c.github.as_ref()).is_some();
        let welcome_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if is_setup { Color::Green } else { Color::Blue }))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(if is_setup { "✨ Welcome" } else { "🚀 Welcome" })
            .title_style(Style::default()
                .fg(if is_setup { Color::Green } else { Color::Blue })
                .add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let welcome_text = if is_setup {
            "Welcome to dotstate! Your dotfiles are synced and ready.\n\nSelect an option from the menu below to manage your configuration files."
        } else {
            "Welcome to dotstate! Get started by setting up your GitHub repository to sync your dotfiles.\n\nThis will allow you to keep your configuration files backed up and synchronized across all your computers."
        };

        let welcome_para = Paragraph::new(welcome_text)
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true })
            .block(welcome_block);

        frame.render_widget(welcome_para, left_split[0]);

        // Menu items - now using MenuItem enum
        let menu_items = MenuItem::all();
        let selected_index = self.selected_item.to_index();
        let is_setup = self.is_setup();

        let items: Vec<ListItem> = menu_items
            .iter()
            .enumerate()
            .map(|(i, menu_item)| {
                let icon = menu_item.icon();
                let text = menu_item.text();
                let is_enabled = menu_item.is_enabled(is_setup);

                // Determine color based on enabled state
                let color = if !is_enabled {
                    Color::DarkGray  // Disabled items in dark gray
                } else {
                    menu_item.color(self.has_changes_to_push)
                };

                let display_text = if *menu_item == MenuItem::PushChanges && self.has_changes_to_push && is_enabled {
                    format!("{} {} ({} pending)", icon, text, self.changed_files.len())
                } else if !is_enabled {
                    format!("{} {} (requires setup)", icon, text)
                } else {
                    format!("{} {}", icon, text)
                };

                let style = if i == selected_index {
                    if is_enabled {
                        Style::default()
                            .fg(color)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        // Disabled items stay gray even when selected
                        Style::default()
                            .fg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default()
                        .fg(color)
                };
                ListItem::new(display_text).style(style)
            })
            .collect();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("📋 Menu")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        // Store clickable area for mouse support
        self.clickable_areas.clear();
        let list_inner = list_block.inner(left_split[1]);

        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            )
            .highlight_symbol("▶ ");

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

        // Render list
        StatefulWidget::render(list, left_split[1], frame.buffer_mut(), &mut self.list_state);

        // Right panel: Explanation and stats
        let right_split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60), // Explanation
                Constraint::Percentage(40), // Stats
            ])
            .split(content_split[1]);

        // Explanation block with colorful styling
        let icon = self.selected_item.explanation_icon();
        let color = self.selected_item.explanation_color();

        let explanation_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(format!("{} What does this do?", icon))
            .title_style(Style::default().fg(color).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let explanation_para = Paragraph::new(self.get_explanation())
            .style(Style::default().fg(Color::White))
            .wrap(Wrap { trim: true })
            .block(explanation_block);

        frame.render_widget(explanation_para, right_split[0]);

        // Stats block with colorful styling
        let has_pending = !self.changed_files.is_empty();
        let stats_color = if has_pending { Color::Yellow } else { Color::Green };
        let stats_icon = if has_pending { "⚠️" } else { "✅" };

        let stats_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(stats_color))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(format!("{} Status", stats_icon))
            .title_style(Style::default().fg(stats_color).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        // Format stats with colors
        let stats_text = self.get_stats();
        let stats_lines: Vec<Line> = stats_text
            .lines()
            .map(|line| {
                if line.starts_with("Synced Files:") {
                    Line::from(vec![
                        Span::styled("Synced Files: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            line.strip_prefix("Synced Files: ").unwrap_or(""),
                            Style::default().fg(Color::White)
                        ),
                    ])
                } else if line.starts_with("Profiles:") {
                    Line::from(vec![
                        Span::styled("Profiles: ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            line.strip_prefix("Profiles: ").unwrap_or(""),
                            Style::default().fg(Color::White)
                        ),
                    ])
                } else if line.starts_with("Repository:") {
                    Line::from(vec![
                        Span::styled("Repository: ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
                        Span::styled(
                            line.strip_prefix("Repository: ").unwrap_or(""),
                            Style::default().fg(Color::White)
                        ),
                    ])
                } else if line.starts_with("Pending Changes") {
                    Line::from(vec![
                        Span::styled(
                            line,
                            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                        ),
                    ])
                } else if line.starts_with("  •") {
                    Line::from(vec![
                        Span::styled("  • ", Style::default().fg(Color::Yellow)),
                        Span::styled(
                            line.strip_prefix("  • ").unwrap_or(""),
                            Style::default().fg(Color::White)
                        ),
                    ])
                } else if line.contains("... and") {
                    Line::from(vec![
                        Span::styled(
                            line,
                            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
                        ),
                    ])
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
        let _ = Footer::render(frame, footer_chunk, "↑↓: Navigate | Enter/Click: Select | q: Quit")?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        let menu_items = MenuItem::all();
        let max_index = menu_items.len().saturating_sub(1);
        let current_index = self.selected_item.to_index();
        let is_setup = self.is_setup();

        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Up => {
                        if current_index > 0 {
                            let new_index = current_index - 1;
                            if let Some(item) = MenuItem::from_index(new_index) {
                                self.selected_item = item;
                                self.list_state.select_previous();
                            }
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    KeyCode::Down => {
                        if current_index < max_index {
                            let new_index = current_index + 1;
                            if let Some(item) = MenuItem::from_index(new_index) {
                                self.selected_item = item;
                                self.list_state.select_next();
                            }
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    KeyCode::Enter => {
                        // Only allow Enter if the selected item is enabled
                        if self.selected_item.is_enabled(is_setup) {
                            Ok(ComponentAction::Update) // App will handle selection
                        } else {
                            Ok(ComponentAction::None) // Ignore Enter on disabled items
                        }
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        Ok(ComponentAction::Quit)
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Check if click is in any clickable area
                        for (rect, menu_item) in &self.clickable_areas {
                            if mouse.column >= rect.x
                                && mouse.column < rect.x + rect.width
                                && mouse.row >= rect.y
                                && mouse.row < rect.y + rect.height {
                                self.selected_item = *menu_item;
                                let index = menu_item.to_index();
                                self.list_state.select(Some(index));
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
                        if current_index > 0 {
                            let new_index = current_index - 1;
                            if let Some(item) = MenuItem::from_index(new_index) {
                                self.selected_item = item;
                                self.list_state.select_previous();
                            }
                            return Ok(ComponentAction::Update);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if current_index < max_index {
                            let new_index = current_index + 1;
                            if let Some(item) = MenuItem::from_index(new_index) {
                                self.selected_item = item;
                                self.list_state.select_next();
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
