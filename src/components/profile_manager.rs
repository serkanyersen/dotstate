use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::input_field::InputField;
use crate::config::Config;
use crate::styles::{theme, LIST_HIGHLIGHT_SYMBOL};
use crate::utils::{
    center_popup, create_standard_layout, focused_border_style, unfocused_border_style,
};
use anyhow::Result;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    Wrap,
};

/// Profile manager popup types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfilePopupType {
    None,
    Create,
    Switch,
    Rename,
    Delete,
}

/// Which field is focused in the create popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateField {
    Name,
    Description,
    CopyFrom,
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
    pub create_focused_field: CreateField, // Which field is focused
    // Rename popup state
    pub rename_input: String,
    pub rename_cursor: usize,
    // Delete popup state
    pub delete_confirm_input: String,
    pub delete_confirm_cursor: usize,
    // Clickable areas for form fields (for mouse support)
    pub create_name_area: Option<Rect>,
    pub create_description_area: Option<Rect>,
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
            create_focused_field: CreateField::Name,
            rename_input: String::new(),
            rename_cursor: 0,
            delete_confirm_input: String::new(),
            delete_confirm_cursor: 0,
            create_name_area: None,
            create_description_area: None,
        }
    }
}

/// Profile manager component
pub struct ProfileManagerComponent;

impl Default for ProfileManagerComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileManagerComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for ProfileManagerComponent {
    /// Render the component (required by Component trait)
    ///
    /// This method is not used - `render_with_config` is used instead.
    /// This is kept to satisfy the Component trait interface.
    fn render(&mut self, _frame: &mut Frame, _area: Rect) -> Result<()> {
        // This method is required by the trait but we'll use render_with_config instead
        // Default implementation - should not be called
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        // Note: This method is not actually called - event handling is done in app.rs
        // This is kept to satisfy the Component trait interface
        // All keyboard events are handled in app.rs using the keymap system
        match event {
            Event::Key(_) => {
                // Keyboard events are handled in app.rs, not here
                Ok(ComponentAction::None)
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
    pub fn render_with_config(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &mut ProfileManagerState,
    ) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let background = Block::default().style(Style::default().bg(Color::Reset));
        frame.render_widget(background, area);

        // Layout: Header, Content (split), Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Manage Profiles",
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
            self.render_popup(frame, area, config, profiles, &mut *state)?;
        } else {
            // Left: Profiles list
            self.render_profiles_list(frame, left_chunk, config, profiles, state)?;

            // Right: Profile details
            self.render_profile_details(frame, right_chunk, config, profiles, state)?;
        }

        // Footer
        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = match state.popup_type {
            ProfilePopupType::Create => {
                format!(
                    "{}: Next Field | {}: Navigate Copy From | {}: Toggle Selection | {}: Create | {}: Cancel",
                    k(crate::keymap::Action::NextTab),
                    config.keymap.navigation_display(),
                    k(crate::keymap::Action::ToggleSelect),
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Cancel)
                )
            }
            ProfilePopupType::Switch => {
                format!(
                    "{}: Confirm Switch | {}: Cancel",
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Cancel)
                )
            }
            ProfilePopupType::Rename => {
                format!(
                    "{}: Confirm Rename | {}: Cancel",
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Cancel)
                )
            }
            ProfilePopupType::Delete => {
                format!(
                    "Type profile name to confirm | {}: Delete | {}: Cancel",
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Cancel)
                )
            }
            ProfilePopupType::None => {
                format!(
                    "{}: Navigate | {}: Switch Profile | {}: Create | {}: Rename | {}: Delete | {}: Back",
                    config.keymap.navigation_display(),
                    k(crate::keymap::Action::Confirm),
                    k(crate::keymap::Action::Create),
                    k(crate::keymap::Action::Edit),
                    k(crate::keymap::Action::Delete),
                    k(crate::keymap::Action::Cancel)
                )
            }
        };
        Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    /// Render the profiles list on the left
    fn render_profiles_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &mut ProfileManagerState,
    ) -> Result<()> {
        let t = theme();
        let active_profile = &config.active_profile;

        let items: Vec<ListItem> = profiles
            .iter()
            .map(|profile| {
                let is_active = profile.name == *active_profile;
                let icon = if is_active { "⭐" } else { "  " };
                let name_style = if is_active {
                    Style::default()
                        .fg(t.text_emphasis)
                        .add_modifier(Modifier::BOLD)
                } else {
                    t.text_style()
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
                    .border_style(focused_border_style()),
            )
            .highlight_style(t.highlight_style())
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        // Store clickable areas for mouse support
        // Each list item is clickable
        state.clickable_areas.clear();
        for (idx, _) in profiles.iter().enumerate() {
            // Calculate the rect for each item (approximate, since List widget handles rendering)
            // We'll use the full width and estimate height per item
            let item_height = 1; // Each list item is typically 1 row
            let item_y = area.y + 1 + idx as u16; // +1 for border, +idx for item position
            if item_y < area.y + area.height - 1 {
                // Within visible area
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
    fn render_profile_details(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &ProfileManagerState,
    ) -> Result<()> {
        let active_profile = &config.active_profile;

        // Find selected profile (use selected index, fallback to active, then first)
        let profile = state
            .list_state
            .selected()
            .and_then(|idx| profiles.get(idx))
            .or_else(|| profiles.iter().find(|p| p.name == *active_profile))
            .or_else(|| profiles.first());

        let t = theme();
        if let Some(profile) = profile {
            let is_active = profile.name == *active_profile;
            let status = if is_active {
                ("● Active", t.success)
            } else {
                ("○ Inactive", t.text_emphasis)
            };

            let description = profile.description.as_deref().unwrap_or("No description");

            let files_text = if profile.synced_files.is_empty() {
                "No files synced".to_string()
            } else {
                format!("{} files synced:", profile.synced_files.len())
            };

            let files_list = if profile.synced_files.is_empty() {
                String::new()
            } else {
                profile
                    .synced_files
                    .iter()
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

            // Create styled text with colors
            use ratatui::text::{Line, Span};
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(
                        "Name: ",
                        Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(&profile.name, t.text_style()),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "Status: ",
                        Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(status.0, Style::default().fg(status.1)),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Description:",
                    Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    description,
                    Style::default().fg(t.text_muted),
                )]),
                Line::from(""),
                Line::from(vec![Span::styled(
                    &files_text,
                    Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                )]),
            ];

            if !files_list.is_empty() {
                for line in files_list.lines() {
                    lines.push(Line::from(vec![Span::styled(line, t.text_style())]));
                }
            }
            if !more_text.is_empty() {
                lines.push(Line::from(vec![Span::styled(
                    &more_text,
                    Style::default().fg(t.text_muted),
                )]));
            }

            let text = ratatui::text::Text::from(lines);

            let paragraph = Paragraph::new(text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Profile Details")
                        .border_style(unfocused_border_style())
                        .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                )
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, area);
        } else {
            let paragraph =
                Paragraph::new("No profiles found.\n\nPress 'C' to create your first profile.")
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Profile Details")
                            .border_style(unfocused_border_style())
                            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                    )
                    .wrap(Wrap { trim: true });
            frame.render_widget(paragraph, area);
        }

        Ok(())
    }

    /// Render the active popup
    fn render_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &mut ProfileManagerState,
    ) -> Result<()> {
        match state.popup_type {
            ProfilePopupType::Create => {
                self.render_create_popup(frame, area, config, profiles, state)
            }
            ProfilePopupType::Switch => {
                self.render_switch_popup(frame, area, config, profiles, state)
            }
            ProfilePopupType::Rename => {
                self.render_rename_popup(frame, area, config, profiles, state)
            }
            ProfilePopupType::Delete => {
                self.render_delete_popup(frame, area, config, profiles, state)
            }
            ProfilePopupType::None => Ok(()),
        }
    }

    /// Render create profile popup
    fn render_create_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        _config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &mut ProfileManagerState,
    ) -> Result<()> {
        let popup_area = center_popup(area, 60, 50);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title (no border)
                Constraint::Length(3), // Name input
                Constraint::Length(3), // Description input
                Constraint::Min(8),    // Copy from option (at least 8 lines, can grow)
                Constraint::Min(0),    // Spacer
            ])
            .split(popup_area);

        let t = theme();
        // Title (no border, just text)
        let title = Paragraph::new("Create New Profile")
            .alignment(Alignment::Center)
            .style(t.title_style());
        frame.render_widget(title, chunks[0]);

        // Store clickable areas for mouse support
        state.create_name_area = Some(chunks[1]);
        state.create_description_area = Some(chunks[2]);

        // Name input
        InputField::render(
            frame,
            chunks[1],
            &state.create_name_input,
            state.create_name_cursor,
            state.create_focused_field == CreateField::Name, // Focused based on state
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
            state.create_focused_field == CreateField::Description, // Focused based on state
            "Description (optional)",
            None,
            Alignment::Left,
            false,
        )?;

        // Copy from option - show list of profiles to select from
        let is_focused = state.create_focused_field == CreateField::CopyFrom;
        let border_style = if is_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        if profiles.is_empty() {
            let copy_para = Paragraph::new("No profiles available to copy from")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Copy From")
                        .border_style(border_style),
                )
                .wrap(Wrap { trim: true });
            frame.render_widget(copy_para, chunks[3]);
        } else {
            // Create a list with "Start Blank" first, then profiles
            let mut items = Vec::new();

            // Add "Start Blank" option at the start
            let is_start_blank_selected = state.create_copy_from.is_none();
            let start_blank_prefix = if is_start_blank_selected {
                "✓ "
            } else {
                "  "
            };
            let start_blank_style = if is_start_blank_selected {
                Style::default().fg(t.success)
            } else {
                t.text_style()
            };
            items.push(
                ListItem::new(format!("{}Start Blank", start_blank_prefix))
                    .style(start_blank_style),
            );

            // Add profiles (offset by 1 because "Start Blank" is at index 0)
            for (idx, profile) in profiles.iter().enumerate() {
                let is_selected = state.create_copy_from == Some(idx);
                let prefix = if is_selected { "✓ " } else { "  " };
                let style = if is_selected {
                    Style::default().fg(t.success)
                } else {
                    t.text_style()
                };
                let file_count = profile.synced_files.len();
                let file_text = if file_count == 0 {
                    " (no files)".to_string()
                } else if file_count == 1 {
                    " (1 file)".to_string()
                } else {
                    format!(" ({} files)", file_count)
                };
                let text = format!("{}{}{}", prefix, profile.name, file_text);
                items.push(ListItem::new(text).style(style));
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Copy From")
                        .border_style(border_style),
                )
                .highlight_style(t.highlight_style())
                .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

            // Create a temporary list state for rendering
            // Index 0 = "Start Blank" (None), Index 1+ = profile at (idx - 1)
            let mut list_state = ListState::default();
            let ui_selected_idx = if let Some(profile_idx) = state.create_copy_from {
                Some(profile_idx + 1) // Offset by 1 because "Start Blank" is at 0
            } else {
                Some(0) // "Start Blank" is selected
            };
            list_state.select(ui_selected_idx);

            // Calculate if we need a scrollbar (if items exceed visible area)
            let visible_height = chunks[3].height.saturating_sub(2); // Subtract borders
            let total_items = (profiles.len() + 1) as u16; // +1 for "Start Blank"
            let needs_scrollbar = total_items > visible_height;

            // Render the list
            frame.render_stateful_widget(list, chunks[3], &mut list_state);

            // Render scrollbar if needed
            if needs_scrollbar {
                use ratatui::widgets::ScrollbarState;

                let selected_pos = list_state.selected().unwrap_or(0);
                let mut scrollbar_state =
                    ScrollbarState::new(total_items as usize).position(selected_pos);

                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(Some("↑"))
                    .end_symbol(Some("↓"));

                frame.render_stateful_widget(scrollbar, chunks[3], &mut scrollbar_state);
            }
        }

        Ok(())
    }

    /// Render switch profile confirmation popup
    fn render_switch_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &ProfileManagerState,
    ) -> Result<()> {
        let popup_area = center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let selected_idx = state.list_state.selected();
        let current_profile = profiles.iter().find(|p| p.name == config.active_profile);
        let target_profile = selected_idx.and_then(|idx| profiles.get(idx));

        let content = if let (Some(current), Some(target)) = (current_profile, target_profile) {
            format!(
                "Switch Profile\n\n\
                Current: {} ({} files)\n\
                Target: {} ({} files)\n\n\
                This will:\n\
                • Remove symlinks for current profile\n\
                • Create symlinks for target profile\n\
                • Backup existing files (if backups are enabled)\n\n\
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
            .block(Block::default().borders(Borders::ALL))
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center);
        frame.render_widget(para, popup_area);

        Ok(())
    }

    /// Render rename profile popup
    fn render_rename_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        _config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &ProfileManagerState,
    ) -> Result<()> {
        let t = theme();
        let popup_area = center_popup(area, 60, 30);
        frame.render_widget(Clear, popup_area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Title (no border)
                Constraint::Length(3), // Input
                Constraint::Min(0),    // Spacer
            ])
            .split(popup_area);

        // Title (no border, just text)
        let selected_idx = state.list_state.selected();
        let profile_name = selected_idx
            .and_then(|idx| profiles.get(idx))
            .map(|p| p.name.as_str())
            .unwrap_or("Profile");

        let title = Paragraph::new(format!("Rename Profile: {}", profile_name))
            .alignment(Alignment::Center)
            .style(t.title_style());
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
    fn render_delete_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[crate::utils::ProfileInfo],
        state: &ProfileManagerState,
    ) -> Result<()> {
        let popup_area = center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let selected_idx = state.list_state.selected();
        let profile = selected_idx.and_then(|idx| profiles.get(idx));
        let active_profile = &config.active_profile;
        let is_active = profile.map(|p| p.name == *active_profile).unwrap_or(false);

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
                    Please switch to another profile first.",
                    p.name
                )
            } else {
                format!(
                    "⚠️  WARNING: Delete Profile\n\n\
                    This will permanently delete:\n\
                    • Profile '{}'\n\
                    • All {} synced files in the repo\n\
                    • Profile folder: ~/.config/dotstate/storage/{}/\n\n\
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
            .block(Block::default().borders(Borders::ALL))
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
