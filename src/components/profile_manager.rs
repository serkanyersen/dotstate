use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::config::Config;
use crate::utils::{create_standard_layout, center_popup, focused_border_style, unfocused_border_style};
use crate::components::input_field::InputField;

/// Profile manager popup types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilePopupType {
    None,
    Create,
    Switch,
    Rename,
    Delete,
}

/// Profile manager component state
#[derive(Debug, Clone)]
pub struct ProfileManagerState {
    pub list_state: ListState,
    pub clickable_areas: Vec<(Rect, usize)>, // (area, profile_index)
    pub popup_type: ProfilePopupType,
    // Create popup state
    pub create_name_input: String,
    pub create_name_cursor: usize,
    pub create_description_input: String,
    pub create_description_cursor: usize,
    pub create_copy_from: Option<usize>, // Index of profile to copy from
    // Rename popup state
    pub rename_input: String,
    pub rename_cursor: usize,
    // Delete popup state
    pub delete_confirm_input: String,
    pub delete_confirm_cursor: usize,
}

impl Default for ProfileManagerState {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
            clickable_areas: Vec::new(),
            popup_type: ProfilePopupType::None,
            create_name_input: String::new(),
            create_name_cursor: 0,
            create_description_input: String::new(),
            create_description_cursor: 0,
            create_copy_from: None,
            rename_input: String::new(),
            rename_cursor: 0,
            delete_confirm_input: String::new(),
            delete_confirm_cursor: 0,
        }
    }
}

/// Profile manager component
pub struct ProfileManagerComponent;

impl ProfileManagerComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for ProfileManagerComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // This method is required by the trait but we'll use render_with_config instead
        // Default implementation - should not be called
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Up => Ok(ComponentAction::Update),
                    KeyCode::Down => Ok(ComponentAction::Update),
                    KeyCode::Enter => Ok(ComponentAction::Update),
                    KeyCode::Char('c') | KeyCode::Char('C') => Ok(ComponentAction::Update),
                    KeyCode::Char('r') | KeyCode::Char('R') => Ok(ComponentAction::Update),
                    KeyCode::Char('d') | KeyCode::Char('D') => Ok(ComponentAction::Update),
                    KeyCode::Esc => Ok(ComponentAction::Quit),
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Mouse clicks are handled in app.rs where we have access to profiles
                        Ok(ComponentAction::None)
                    }
                    MouseEventKind::ScrollUp => {
                        // Mouse scroll is handled in app.rs where we have access to state
                        Ok(ComponentAction::None)
                    }
                    MouseEventKind::ScrollDown => {
                        // Scroll down in list - handled in app.rs
                        Ok(ComponentAction::None)
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            _ => Ok(ComponentAction::None),
        }
    }
}

impl ProfileManagerComponent {
    /// Render with config and state - this is the main render method
    pub fn render_with_config(&mut self, frame: &mut Frame, area: Rect, config: &Config, state: &mut ProfileManagerState) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        // Layout: Header, Content (split), Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let _ = Header::render(
            frame,
            header_chunk,
            "dotstate - Manage Profiles",
            "Manage different profiles for different machines. Each profile has its own set of synced dotfiles."
        )?;

        // Split content: Left (profiles list), Right (profile details)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(content_chunk);
        let left_chunk = chunks[0];
        let right_chunk = chunks[1];

        // Check if popup is active
        if state.popup_type != ProfilePopupType::None {
            self.render_popup(frame, area, config, state)?;
        } else {
            // Left: Profiles list
            self.render_profiles_list(frame, left_chunk, config, state)?;

            // Right: Profile details
            self.render_profile_details(frame, right_chunk, config, state)?;
        }

        // Footer
        let footer_text = match state.popup_type {
            ProfilePopupType::Create => "Tab: Next Field | Shift+Tab: Previous | Enter: Create | Esc: Cancel",
            ProfilePopupType::Switch => "Enter: Confirm Switch | Esc: Cancel",
            ProfilePopupType::Rename => "Enter: Confirm Rename | Esc: Cancel",
            ProfilePopupType::Delete => "Type profile name to confirm | Enter: Delete | Esc: Cancel",
            ProfilePopupType::None => "↑↓: Navigate | Enter: Switch Profile | C: Create | R: Rename | D: Delete | Esc: Back",
        };
        Footer::render(frame, footer_chunk, footer_text)?;

        Ok(())
    }

    /// Render the profiles list on the left
    fn render_profiles_list(&self, frame: &mut Frame, area: Rect, config: &Config, state: &mut ProfileManagerState) -> Result<()> {
        let profiles = &config.profiles;
        let active_profile = &config.active_profile;

        let items: Vec<ListItem> = profiles.iter()
            .enumerate()
            .map(|(idx, profile)| {
                let is_active = profile.name == *active_profile;
                let icon = if is_active { "⭐" } else { "  " };
                let name_style = if is_active {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let file_count = profile.synced_files.len();
                let file_text = if file_count == 1 {
                    "1 file".to_string()
                } else {
                    format!("{} files", file_count)
                };

                let text = format!("{} {} ({})", icon, profile.name, file_text);
                ListItem::new(text).style(name_style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Profiles")
                    .border_style(focused_border_style())
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            );

        // Store clickable areas for mouse support
        // Each list item is clickable
        state.clickable_areas.clear();
        for (idx, _) in profiles.iter().enumerate() {
            // Calculate the rect for each item (approximate, since List widget handles rendering)
            // We'll use the full width and estimate height per item
            let item_height = 1; // Each list item is typically 1 row
            let item_y = area.y + 1 + idx as u16; // +1 for border, +idx for item position
            if item_y < area.y + area.height - 1 { // Within visible area
                state.clickable_areas.push((
                    Rect {
                        x: area.x + 1, // +1 for left border
                        y: item_y,
                        width: area.width.saturating_sub(2), // -2 for borders
                        height: item_height,
                    },
                    idx,
                ));
            }
        }

        // Render with state
        frame.render_stateful_widget(list, area, &mut state.list_state);

        Ok(())
    }

    /// Render profile details on the right
    fn render_profile_details(&self, frame: &mut Frame, area: Rect, config: &Config, state: &ProfileManagerState) -> Result<()> {
        let profiles = &config.profiles;
        let active_profile = &config.active_profile;

        // Find selected profile (use selected index, fallback to active, then first)
        let profile = state.list_state.selected()
            .and_then(|idx| profiles.get(idx))
            .or_else(|| profiles.iter().find(|p| p.name == *active_profile))
            .or_else(|| profiles.first());

        let content = if let Some(profile) = profile {
            let is_active = profile.name == *active_profile;
            let status = if is_active {
                "● Active".to_string()
            } else {
                "○ Inactive".to_string()
            };

            let description = profile.description.as_ref()
                .map(|d| d.as_str())
                .unwrap_or("No description");

            let files_text = if profile.synced_files.is_empty() {
                "No files synced".to_string()
            } else {
                format!("{} files synced:", profile.synced_files.len())
            };

            let files_list = if profile.synced_files.is_empty() {
                String::new()
            } else {
                profile.synced_files.iter()
                    .take(10) // Show first 10
                    .map(|f| format!("  • {}", f))
                    .collect::<Vec<_>>()
                    .join("\n")
            };

            let more_text = if profile.synced_files.len() > 10 {
                format!("\n  ... and {} more", profile.synced_files.len() - 10)
            } else {
                String::new()
            };

            format!(
                "Name: {}\n\nStatus: {}\n\nDescription:\n{}\n\n{}\n{}",
                profile.name,
                status,
                description,
                files_text,
                files_list + &more_text
            )
        } else {
            "No profiles found.\n\nPress 'C' to create your first profile.".to_string()
        };

        let paragraph = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Profile Details")
                    .border_style(unfocused_border_style())
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);

        Ok(())
    }

    /// Render the active popup
    fn render_popup(&self, frame: &mut Frame, area: Rect, config: &Config, state: &ProfileManagerState) -> Result<()> {
        match state.popup_type {
            ProfilePopupType::Create => self.render_create_popup(frame, area, config, state),
            ProfilePopupType::Switch => self.render_switch_popup(frame, area, config, state),
            ProfilePopupType::Rename => self.render_rename_popup(frame, area, config, state),
            ProfilePopupType::Delete => self.render_delete_popup(frame, area, config, state),
            ProfilePopupType::None => Ok(()),
        }
    }

    /// Render create profile popup
    fn render_create_popup(&self, frame: &mut Frame, area: Rect, config: &Config, state: &ProfileManagerState) -> Result<()> {
        let popup_area = center_popup(area, 60, 50);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Name input
                Constraint::Length(3), // Description input
                Constraint::Length(3), // Copy from option
                Constraint::Min(0),    // Spacer
            ])
            .split(popup_area);

        // Title
        let title = Paragraph::new("Create New Profile")
            .block(Block::default().borders(Borders::ALL).title("Create Profile"))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Name input
        InputField::render(
            frame,
            chunks[1],
            &state.create_name_input,
            state.create_name_cursor,
            true, // Focused
            "Profile Name",
            Some("e.g., Personal-Mac, Work-Linux"),
            Alignment::Left,
            false,
        )?;

        // Description input
        InputField::render(
            frame,
            chunks[2],
            &state.create_description_input,
            state.create_description_cursor,
            false, // Not focused initially
            "Description (optional)",
            None,
            Alignment::Left,
            false,
        )?;

        // Copy from option (simplified for now - just show text)
        let copy_text = if let Some(idx) = state.create_copy_from {
            if let Some(profile) = config.profiles.get(idx) {
                format!("Copy files from: {}", profile.name)
            } else {
                "Start blank (no files)".to_string()
            }
        } else {
            "Start blank (no files)".to_string()
        };
        let copy_para = Paragraph::new(copy_text)
            .block(Block::default().borders(Borders::ALL).title("Copy From"))
            .wrap(Wrap { trim: true });
        frame.render_widget(copy_para, chunks[3]);

        Ok(())
    }

    /// Render switch profile confirmation popup
    fn render_switch_popup(&self, frame: &mut Frame, area: Rect, config: &Config, state: &ProfileManagerState) -> Result<()> {
        let popup_area = center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let selected_idx = state.list_state.selected();
        let current_profile = config.profiles.iter()
            .find(|p| p.name == config.active_profile);
        let target_profile = selected_idx.and_then(|idx| config.profiles.get(idx));

        let content = if let (Some(current), Some(target)) = (current_profile, target_profile) {
            format!(
                "Switch Profile\n\n\
                Current: {} ({} files)\n\
                Target: {} ({} files)\n\n\
                This will:\n\
                • Remove symlinks for current profile\n\
                • Create symlinks for target profile\n\
                • Backup existing files if needed\n\n\
                Continue?",
                current.name,
                current.synced_files.len(),
                target.name,
                target.synced_files.len()
            )
        } else {
            "Invalid profile selection".to_string()
        };

        let para = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title("Switch Profile"))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
        frame.render_widget(para, popup_area);

        Ok(())
    }

    /// Render rename profile popup
    fn render_rename_popup(&self, frame: &mut Frame, area: Rect, config: &Config, state: &ProfileManagerState) -> Result<()> {
        let popup_area = center_popup(area, 60, 30);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(3), // Input
                Constraint::Min(0),    // Spacer
            ])
            .split(popup_area);

        // Title
        let selected_idx = state.list_state.selected();
        let profile_name = selected_idx
            .and_then(|idx| config.profiles.get(idx))
            .map(|p| p.name.as_str())
            .unwrap_or("Profile");

        let title = Paragraph::new(format!("Rename Profile: {}", profile_name))
            .block(Block::default().borders(Borders::ALL).title("Rename Profile"))
            .alignment(Alignment::Center);
        frame.render_widget(title, chunks[0]);

        // Name input
        InputField::render(
            frame,
            chunks[1],
            &state.rename_input,
            state.rename_cursor,
            true,
            "New Name",
            Some("Enter new profile name"),
            Alignment::Left,
            false,
        )?;

        Ok(())
    }

    /// Render delete profile confirmation popup
    fn render_delete_popup(&self, frame: &mut Frame, area: Rect, config: &Config, state: &ProfileManagerState) -> Result<()> {
        let popup_area = center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let selected_idx = state.list_state.selected();
        let profile = selected_idx.and_then(|idx| config.profiles.get(idx));
        let is_active = profile.map(|p| p.name == config.active_profile).unwrap_or(false);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Warning text
                Constraint::Length(3), // Confirmation input
                Constraint::Min(0),    // Spacer
            ])
            .split(popup_area);

        let warning_text = if let Some(p) = profile {
            if is_active {
                format!(
                    "⚠️  WARNING: Cannot Delete Active Profile\n\n\
                    Profile '{}' is currently active.\n\
                    Please switch to another profile first."
                , p.name)
            } else {
                format!(
                    "⚠️  WARNING: Delete Profile\n\n\
                    This will permanently delete:\n\
                    • Profile '{}'\n\
                    • All {} synced files in the repo\n\
                    • Profile folder: ~/.dotstate/{}/\n\n\
                    Type the profile name to confirm:",
                    p.name,
                    p.synced_files.len(),
                    p.name
                )
            }
        } else {
            "Invalid profile selection".to_string()
        };

        let warning = Paragraph::new(warning_text)
            .block(Block::default().borders(Borders::ALL).title("Delete Profile"))
            .wrap(Wrap { trim: true });
        frame.render_widget(warning, chunks[0]);

        // Confirmation input (only if not active)
        if let Some(p) = profile {
            if !is_active {
                InputField::render(
                    frame,
                    chunks[1],
                    &state.delete_confirm_input,
                    state.delete_confirm_cursor,
                    true,
                    "Type profile name to confirm",
                    Some(&p.name),
                    Alignment::Left,
                    false,
                )?;
            }
        }

        Ok(())
    }
}

