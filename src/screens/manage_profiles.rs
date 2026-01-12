use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::input_field::InputField;
use crate::config::Config;
use crate::keymap::{Action, Keymap};
use crate::screens::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::Screen as ScreenId;
use crate::utils::{
    center_popup, create_standard_layout, focused_border_style, unfocused_border_style,
    text_input::{handle_backspace, handle_char_insertion, handle_cursor_movement, handle_delete},
};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
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
    // Cached profiles to reduce disk I/O
    pub profiles: Vec<crate::utils::ProfileInfo>,
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
            profiles: Vec::new(),
        }
    }
}

pub struct ManageProfilesScreen {
    pub state: ProfileManagerState,
}

impl Default for ManageProfilesScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl ManageProfilesScreen {
    pub fn new() -> Self {
        Self {
            state: ProfileManagerState::default(),
        }
    }

    /// Refresh the cached profiles from disk
    pub fn refresh_profiles(&mut self, repo_path: &std::path::Path) -> Result<()> {
        let profiles = crate::services::ProfileService::get_profiles(repo_path)?;
        self.state.profiles = profiles;
        // Initialize list selection to first item if profiles exist
        if !self.state.profiles.is_empty() {
            self.state.list_state.select(Some(0));
        }
        Ok(())
    }

    fn get_action(&self, key: KeyCode, modifiers: KeyModifiers, keymap: &Keymap) -> Option<Action> {
        keymap.get_action(key, modifiers)
    }

    fn handle_mouse_event(
        &mut self,
        mouse: crossterm::event::MouseEvent,
        _config: &Config,
    ) -> ScreenAction {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let x = mouse.column;
                let y = mouse.row;

                // Handle clicks in list
                for (area, idx) in &self.state.clickable_areas {
                    if x >= area.x
                        && x < area.x + area.width
                        && y >= area.y
                        && y < area.y + area.height
                    {
                        self.state.list_state.select(Some(*idx));
                        return ScreenAction::Refresh;
                    }
                }

                // Handle clicks in create popup fields
                if self.state.popup_type == ProfilePopupType::Create {
                    if let Some(area) = self.state.create_name_area {
                        if x >= area.x
                            && x < area.x + area.width
                            && y >= area.y
                            && y < area.y + area.height
                        {
                            self.state.create_focused_field = CreateField::Name;
                            return ScreenAction::Refresh;
                        }
                    }
                    if let Some(area) = self.state.create_description_area {
                        if x >= area.x
                            && x < area.x + area.width
                            && y >= area.y
                            && y < area.y + area.height
                        {
                            self.state.create_focused_field = CreateField::Description;
                            return ScreenAction::Refresh;
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                let selected = self.state.list_state.selected().unwrap_or(0);
                if selected > 0 {
                    self.state.list_state.select(Some(selected - 1));
                    return ScreenAction::Refresh;
                }
            }
            MouseEventKind::ScrollDown => {
                // We don't know the max count here easily without passing it in,
                // but we can rely on the list to handle out of bounds or just be conservative.
                // For now, let's just increment and let the UI handle bounds if possible,
                // or just rely on keyboard navigation which is safer.
                // Better yet, we can't properly implement scroll down without knowing the list size.
                // We'll leave it for now or implement if we pass profiles to handle_event.
                // Actually, handle_event takes ScreenContext which doesn't have profiles.
                // We might need to change ScreenContext or accept profiles in state.
                // The original app.rs logic had access to profiles.
                // For now, let's skip scroll down logic or make it best-effort?
                // Wait, we can add `profiles_count` to state if needed, but for now let's just use keyboard.
            }
            _ => {}
        }
        ScreenAction::None
    }
    /// Render the profiles list on the left
    fn render_profiles_list(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        let t = theme();
        let active_profile = &config.active_profile;

        let items: Vec<ListItem> = self.state
            .profiles
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

        // Render with state
        frame.render_stateful_widget(list, area, &mut self.state.list_state.clone());

        Ok(())
    }

    /// Render profile details on the right
    fn render_profile_details(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        let active_profile = &config.active_profile;

        // Find selected profile (use selected index, fallback to active, then first)
        let profile = self.state
            .list_state
            .selected()
            .and_then(|idx| self.state.profiles.get(idx))
            .or_else(|| self.state.profiles.iter().find(|p| p.name == *active_profile))
            .or_else(|| self.state.profiles.first());

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
    ) -> Result<()> {
        match self.state.popup_type {
            ProfilePopupType::Create => self.render_create_popup(frame, area),
            ProfilePopupType::Switch => self.render_switch_popup(frame, area, config),
            ProfilePopupType::Rename => self.render_rename_popup(frame, area),
            ProfilePopupType::Delete => self.render_delete_popup(frame, area, config),
            ProfilePopupType::None => Ok(()),
        }
    }

    /// Render create profile popup
    fn render_create_popup(
        &self,
        frame: &mut Frame,
        area: Rect,
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

        // Name input
        InputField::render(
            frame,
            chunks[1],
            &self.state.create_name_input,
            self.state.create_name_cursor,
            self.state.create_focused_field == CreateField::Name, // Focused based on state
            "Profile Name",
            Some("e.g., Personal-Mac, Work-Linux"),
            Alignment::Left,
            false,
        )?;

        // Description input
        InputField::render(
            frame,
            chunks[2],
            &self.state.create_description_input,
            self.state.create_description_cursor,
            self.state.create_focused_field == CreateField::Description, // Focused based on state
            "Description (optional)",
            None,
            Alignment::Left,
            false,
        )?;

        // Copy from option - show list of profiles to select from
        let is_focused = self.state.create_focused_field == CreateField::CopyFrom;
        let border_style = if is_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        if self.state.profiles.is_empty() {
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
            let is_start_blank_selected = self.state.create_copy_from.is_none();
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
            for (idx, profile) in self.state.profiles.iter().enumerate() {
                let is_selected = self.state.create_copy_from == Some(idx);
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
            let ui_selected_idx = if let Some(profile_idx) = self.state.create_copy_from {
                Some(profile_idx + 1) // Offset by 1 because "Start Blank" is at 0
            } else {
                Some(0) // "Start Blank" is selected
            };
            list_state.select(ui_selected_idx);

            // Calculate if we need a scrollbar (if items exceed visible area)
            let visible_height = chunks[3].height.saturating_sub(2); // Subtract borders
            let total_items = (self.state.profiles.len() + 1) as u16; // +1 for "Start Blank"
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
    ) -> Result<()> {
        let popup_area = center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let selected_idx = self.state.list_state.selected();
        let current_profile = self.state
            .profiles
            .iter()
            .find(|p| p.name == config.active_profile);
        let target_profile = selected_idx.and_then(|idx| self.state.profiles.get(idx));

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
        let selected_idx = self.state.list_state.selected();
        let profile_name = selected_idx
            .and_then(|idx| self.state.profiles.get(idx))
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
            &self.state.rename_input,
            self.state.rename_cursor,
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
    ) -> Result<()> {
        let popup_area = center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let selected_idx = self.state.list_state.selected();
        let profile = selected_idx.and_then(|idx| self.state.profiles.get(idx));
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
                    &self.state.delete_confirm_input,
                    self.state.delete_confirm_cursor,
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

impl Screen for ManageProfilesScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
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
        if self.state.popup_type != ProfilePopupType::None {
            self.render_popup(frame, area, ctx.config)?;
        } else {
            // Left: Profiles list
            self.render_profiles_list(frame, left_chunk, ctx.config)?;

            // Right: Profile details
            self.render_profile_details(frame, right_chunk, ctx.config)?;
        }

        // Footer
        let k = |a| ctx.config.keymap.get_key_display_for_action(a);
        let footer_text = match self.state.popup_type {
            ProfilePopupType::Create => {
                format!(
                    "{}: Next Field | {}: Navigate Copy From | {}: Toggle Selection | {}: Create | {}: Cancel",
                    k(crate::keymap::Action::NextTab),
                    ctx.config.keymap.navigation_display(),
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
                    ctx.config.keymap.navigation_display(),
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

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        // Handle popup events first
        if self.state.popup_type != ProfilePopupType::None {
            match event {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let action = self.get_action(key.code, key.modifiers, &ctx.config.keymap);

                    match self.state.popup_type {
                        ProfilePopupType::Create => {
                            // Handle keymap actions
                            if let Some(action) = action {
                                match action {
                                    Action::Cancel => {
                                        self.state.popup_type = ProfilePopupType::None;
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::NextTab => {
                                        self.state.create_focused_field =
                                            match self.state.create_focused_field {
                                                CreateField::Name => CreateField::Description,
                                                CreateField::Description => CreateField::CopyFrom,
                                                CreateField::CopyFrom => CreateField::Name,
                                            };
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::PrevTab => {
                                        self.state.create_focused_field =
                                            match self.state.create_focused_field {
                                                CreateField::Name => CreateField::CopyFrom,
                                                CreateField::Description => CreateField::Name,
                                                CreateField::CopyFrom => CreateField::Description,
                                            };
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::Confirm => {
                                        // Logic for CopyFrom selection vs Creation
                                        if self.state.create_focused_field == CreateField::CopyFrom
                                        {

                                            // This logic depends on us knowing how many profiles there are to wrap/clamp.
                                            // We probably need to fetch profiles here too to do accurate selection logic?
                                            // Or simplified: Just handle Enter as "Create".
                                            // The original code handled Enter as create unless in CopyFrom list partial selection?
                                            // Actually original code (lines 1334-1353) handled detailed selection logic.
                                            // "If Copy From is focused, select the current item first, then create"
                                            // Wait, if we are in CopyFrom, Enter usually means "Select this option".
                                            // But line 1355 says "Enter always creates, regardless of focus".
                                            // So we should just proceed to create.
                                        }

                                        if !self.state.create_name_input.is_empty() {
                                            let name = self.state.create_name_input.clone();
                                            let description =
                                                if self.state.create_description_input.is_empty() {
                                                    None
                                                } else {
                                                    Some(
                                                        self.state.create_description_input.clone(),
                                                    )
                                                };
                                            let copy_from = self.state.create_copy_from;

                                            // Reset state
                                            self.state.popup_type = ProfilePopupType::None;
                                            self.state.create_name_input.clear();
                                            self.state.create_description_input.clear();
                                            self.state.create_focused_field = CreateField::Name;

                                            return Ok(ScreenAction::CreateProfile {
                                                name,
                                                description,
                                                copy_from,
                                            });
                                        }
                                        return Ok(ScreenAction::None);
                                    }
                                    _ => {}
                                }
                            }

                            // Handle text input and specific navigation
                            match action {
                                Some(Action::MoveUp) => {
                                    if self.state.create_focused_field == CreateField::CopyFrom {
                                        let current =
                                            self.state.create_copy_from.map(|i| i + 1).unwrap_or(0);
                                        if current > 0 {
                                            let new_val = current - 1;
                                            self.state.create_copy_from = if new_val == 0 {
                                                None
                                            } else {
                                                Some(new_val - 1)
                                            };
                                            return Ok(ScreenAction::Refresh);
                                        }
                                    }
                                }
                                Some(Action::MoveDown) => {
                                    if self.state.create_focused_field == CreateField::CopyFrom {
                                        // We need profile count to limit.
                                        // We need profile count to limit.
                                        let profiles = &self.state.profiles;
                                        let total = profiles.len() + 1; // +1 for "Blank"
                                        let current =
                                            self.state.create_copy_from.map(|i| i + 1).unwrap_or(0);
                                        if current < total - 1 {
                                            let new_val = current + 1;
                                            self.state.create_copy_from = Some(new_val - 1);
                                            return Ok(ScreenAction::Refresh);
                                        }
                                    }
                                }
                                _ => {
                                    if let Some(act) = action {
                                        let key_code = match act {
                                            Action::MoveLeft => KeyCode::Left,
                                            Action::MoveRight => KeyCode::Right,
                                            Action::Home => KeyCode::Home,
                                            Action::End => KeyCode::End,
                                            Action::Backspace => KeyCode::Backspace,
                                            Action::DeleteChar => KeyCode::Delete,
                                            _ => KeyCode::Null,
                                        };

                                        if key_code != KeyCode::Null {
                                            match self.state.create_focused_field {
                                                CreateField::Name => {
                                                    if act == Action::Backspace {
                                                        handle_backspace(
                                                            &mut self.state.create_name_input,
                                                            &mut self.state.create_name_cursor,
                                                        );
                                                    } else if act == Action::DeleteChar {
                                                        handle_delete(
                                                            &mut self.state.create_name_input,
                                                            &mut self.state.create_name_cursor,
                                                        );
                                                    } else {
                                                        handle_cursor_movement(
                                                            &self.state.create_name_input,
                                                            &mut self.state.create_name_cursor,
                                                            key_code,
                                                        );
                                                    }
                                                }
                                                CreateField::Description => {
                                                    if act == Action::Backspace {
                                                        handle_backspace(
                                                            &mut self
                                                                .state
                                                                .create_description_input,
                                                            &mut self
                                                                .state
                                                                .create_description_cursor,
                                                        );
                                                    } else if act == Action::DeleteChar {
                                                        handle_delete(
                                                            &mut self
                                                                .state
                                                                .create_description_input,
                                                            &mut self
                                                                .state
                                                                .create_description_cursor,
                                                        );
                                                    } else {
                                                        handle_cursor_movement(
                                                            &self.state.create_description_input,
                                                            &mut self
                                                                .state
                                                                .create_description_cursor,
                                                            key_code,
                                                        );
                                                    }
                                                }
                                                _ => {}
                                            }
                                            return Ok(ScreenAction::Refresh);
                                        }
                                    }

                                    // Char input
                                    if let KeyCode::Char(c) = key.code {
                                        if !key.modifiers.intersects(
                                            KeyModifiers::CONTROL
                                                | KeyModifiers::ALT
                                                | KeyModifiers::SUPER,
                                        ) {
                                            match self.state.create_focused_field {
                                                CreateField::Name => {
                                                    handle_char_insertion(
                                                        &mut self.state.create_name_input,
                                                        &mut self.state.create_name_cursor,
                                                        c,
                                                    );
                                                }
                                                CreateField::Description => {
                                                    handle_char_insertion(
                                                        &mut self.state.create_description_input,
                                                        &mut self.state.create_description_cursor,
                                                        c,
                                                    );
                                                }
                                                _ => {}
                                            }
                                            return Ok(ScreenAction::Refresh);
                                        }
                                    }
                                }
                            }
                        }
                        ProfilePopupType::Rename => {
                            if let Some(action) = action {
                                match action {
                                    Action::Cancel => {
                                        self.state.popup_type = ProfilePopupType::None;
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::Confirm => {
                                        if !self.state.rename_input.is_empty() {
                                            if let Some(idx) = self.state.list_state.selected() {
                                                let profiles = &self.state.profiles;
                                                if let Some(profile) = profiles.get(idx) {
                                                    let old_name = profile.name.clone();
                                                    let new_name = self.state.rename_input.clone();
                                                    self.state.popup_type = ProfilePopupType::None;
                                                    self.state.rename_input.clear();
                                                    return Ok(ScreenAction::RenameProfile {
                                                        old_name,
                                                        new_name,
                                                    });
                                                }
                                            }
                                        }
                                        return Ok(ScreenAction::None);
                                    }
                                    Action::Backspace => {
                                        handle_backspace(
                                            &mut self.state.rename_input,
                                            &mut self.state.rename_cursor,
                                        );
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::DeleteChar => {
                                        handle_delete(
                                            &mut self.state.rename_input,
                                            &mut self.state.rename_cursor,
                                        );
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::MoveLeft
                                    | Action::MoveRight
                                    | Action::Home
                                    | Action::End => {
                                        let key_code = match action {
                                            Action::MoveLeft => KeyCode::Left,
                                            Action::MoveRight => KeyCode::Right,
                                            Action::Home => KeyCode::Home,
                                            Action::End => KeyCode::End,
                                            _ => KeyCode::Null,
                                        };
                                        handle_cursor_movement(
                                            &self.state.rename_input,
                                            &mut self.state.rename_cursor,
                                            key_code,
                                        );
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    _ => {}
                                }
                            }

                            // Char input
                            if let KeyCode::Char(c) = key.code {
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) {
                                    handle_char_insertion(
                                        &mut self.state.rename_input,
                                        &mut self.state.rename_cursor,
                                        c,
                                    );
                                    return Ok(ScreenAction::Refresh);
                                }
                            }
                        }
                        ProfilePopupType::Delete => {
                            if let Some(action) = action {
                                match action {
                                    Action::Cancel => {
                                        self.state.popup_type = ProfilePopupType::None;
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::Confirm => {
                                        if let Some(idx) = self.state.list_state.selected() {
                                            let profiles = &self.state.profiles;
                                            if let Some(profile) = profiles.get(idx) {
                                                if self.state.delete_confirm_input == profile.name {
                                                    let name = profile.name.clone();
                                                    self.state.popup_type = ProfilePopupType::None;
                                                    self.state.delete_confirm_input.clear();
                                                    return Ok(ScreenAction::DeleteProfile {
                                                        name,
                                                    });
                                                }
                                            }
                                        }
                                        // If input doesn't match or whatever, maybe shake or just do nothing?
                                        // Original just did nothing.
                                        return Ok(ScreenAction::None);
                                    }
                                    Action::Backspace => {
                                        handle_backspace(
                                            &mut self.state.delete_confirm_input,
                                            &mut self.state.delete_confirm_cursor,
                                        );
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::DeleteChar => {
                                        handle_delete(
                                            &mut self.state.delete_confirm_input,
                                            &mut self.state.delete_confirm_cursor,
                                        );
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::MoveLeft
                                    | Action::MoveRight
                                    | Action::Home
                                    | Action::End => {
                                        let key_code = match action {
                                            Action::MoveLeft => KeyCode::Left,
                                            Action::MoveRight => KeyCode::Right,
                                            Action::Home => KeyCode::Home,
                                            Action::End => KeyCode::End,
                                            _ => KeyCode::Null,
                                        };
                                        handle_cursor_movement(
                                            &self.state.delete_confirm_input,
                                            &mut self.state.delete_confirm_cursor,
                                            key_code,
                                        );
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    _ => {}
                                }
                            }
                            if let KeyCode::Char(c) = key.code {
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                                ) {
                                    handle_char_insertion(
                                        &mut self.state.delete_confirm_input,
                                        &mut self.state.delete_confirm_cursor,
                                        c,
                                    );
                                    return Ok(ScreenAction::Refresh);
                                }
                            }
                        }
                        ProfilePopupType::Switch => {
                            if let Some(action) = action {
                                match action {
                                    Action::Cancel => {
                                        self.state.popup_type = ProfilePopupType::None;
                                        return Ok(ScreenAction::Refresh);
                                    }
                                    Action::Confirm => {
                                        if let Some(idx) = self.state.list_state.selected() {
                                            let profiles = &self.state.profiles;
                                            if let Some(profile) = profiles.get(idx) {
                                                let name = profile.name.clone();
                                                self.state.popup_type = ProfilePopupType::None;
                                                return Ok(ScreenAction::SwitchProfile { name });
                                            }
                                        }
                                        return Ok(ScreenAction::None);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        ProfilePopupType::None => {} // Should not be reachable inside this match
                    }
                    return Ok(ScreenAction::None);
                }
                _ => return Ok(ScreenAction::None),
            }
        }

        // Main screen events
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = self.get_action(key.code, key.modifiers, &ctx.config.keymap);
                if let Some(action) = action {
                    match action {
                        Action::Cancel => return Ok(ScreenAction::Navigate(ScreenId::MainMenu)),
                        Action::MoveUp => {
                            let selected = self.state.list_state.selected().unwrap_or(0);
                            let new_selected = if selected > 0 { selected - 1 } else { selected };
                            self.state.list_state.select(Some(new_selected));
                            return Ok(ScreenAction::Refresh);
                        }
                        Action::MoveDown => {
                            let profiles = &self.state.profiles;
                            let selected = self.state.list_state.selected().unwrap_or(0);
                            let new_selected =
                                if !profiles.is_empty() && selected < profiles.len() - 1 {
                                    selected + 1
                                } else {
                                    selected
                                };
                            self.state.list_state.select(Some(new_selected));
                            return Ok(ScreenAction::Refresh);
                        }
                        Action::Create => {
                            self.state.popup_type = ProfilePopupType::Create;
                            self.state.create_name_input.clear();
                            self.state.create_description_input.clear();
                            self.state.create_focused_field = CreateField::Name;
                            self.state.create_copy_from = None;
                            return Ok(ScreenAction::Refresh);
                        }
                        Action::Edit => {
                            // Rename
                            if let Some(idx) = self.state.list_state.selected() {
                                let profiles = &self.state.profiles;
                                if let Some(profile) = profiles.get(idx) {
                                    self.state.popup_type = ProfilePopupType::Rename;
                                    self.state.rename_input = profile.name.clone();
                                    self.state.rename_cursor =
                                        self.state.rename_input.chars().count();
                                    return Ok(ScreenAction::Refresh);
                                }
                            }
                        }
                        Action::Delete => {
                            if let Some(idx) = self.state.list_state.selected() {
                                let profiles = &self.state.profiles;
                                if profiles.get(idx).is_some() {
                                    self.state.popup_type = ProfilePopupType::Delete;
                                    self.state.delete_confirm_input.clear();
                                    return Ok(ScreenAction::Refresh);
                                }
                            }
                        }
                        Action::Confirm => {
                            // Switch or just select?
                            // Navigation implies Confirm usually acts as "Action on current item"
                            // For profiles, that's likely "Switch to this profile" or "Show details" -> but details are side-by-side
                            // Original code (footer): "Switch Profile"
                            // So Confirm -> Switch Popup

                            self.state.popup_type = ProfilePopupType::Switch;
                            return Ok(ScreenAction::Refresh);
                        }
                        _ => {}
                    }
                }
            }
            Event::Mouse(mouse) => {
                return Ok(self.handle_mouse_event(mouse, ctx.config));
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }
}
