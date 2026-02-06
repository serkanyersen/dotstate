//! Profile selection popup component.
//!
//! A popup for selecting or creating a profile during onboarding.
//! Features inline profile creation without nested popups.

use crate::components::{Popup, PopupRenderResult};
use crate::config::Config;
use crate::keymap::Action;
use crate::styles::theme;
use crate::utils::{
    focused_border_style, unfocused_border_style, MouseRegions, ProfileManifest, TextInput,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap};

/// Result of profile selection
#[derive(Debug, Clone)]
pub enum ProfileSelectionResult {
    /// User selected an existing profile
    SelectExisting(String),
    /// User wants to create and activate a new profile
    CreateNew(String),
    /// User cancelled
    Cancelled,
}

/// Profile selection popup state and rendering
pub struct ProfileSelectionPopup {
    /// Available profiles (loaded from manifest)
    profiles: Vec<ProfileInfo>,
    /// List selection state
    list_state: ListState,
    /// Text input for new profile name (used when "Create New" is selected)
    create_input: TextInput,
    /// Whether the popup is visible
    visible: bool,
    /// Clickable regions for list items
    mouse_regions: MouseRegions<usize>,
    /// Stored list area for scroll hit-testing
    list_area: Option<Rect>,
}

/// Information about a profile for display
#[derive(Debug, Clone)]
struct ProfileInfo {
    name: String,
    description: Option<String>,
    file_count: usize,
    files: Vec<String>,
}

impl ProfileSelectionPopup {
    /// Create a new profile selection popup
    #[must_use]
    pub fn new() -> Self {
        Self {
            profiles: Vec::new(),
            list_state: ListState::default(),
            create_input: TextInput::new(),
            visible: false,
            mouse_regions: MouseRegions::new(),
            list_area: None,
        }
    }

    /// Show the popup and load profiles from the given repo path
    pub fn show(&mut self, repo_path: &std::path::Path) -> Result<()> {
        self.visible = true;
        self.create_input.clear();
        self.load_profiles(repo_path)?;

        // Select first item (or "Create New" if no profiles)
        if self.profiles.is_empty() {
            self.list_state.select(Some(0)); // Create New option
        } else {
            self.list_state.select(Some(0)); // First profile
        }

        Ok(())
    }

    /// Hide the popup
    pub fn hide(&mut self) {
        self.visible = false;
        self.create_input.clear();
    }

    /// Check if popup is visible
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Load profiles from the manifest
    fn load_profiles(&mut self, repo_path: &std::path::Path) -> Result<()> {
        self.profiles.clear();

        if let Ok(manifest) = ProfileManifest::load(repo_path) {
            for profile in &manifest.profiles {
                self.profiles.push(ProfileInfo {
                    name: profile.name.clone(),
                    description: profile.description.clone(),
                    file_count: profile.synced_files.len(),
                    files: profile.synced_files.clone(),
                });
            }
        }

        // Sort profiles alphabetically
        self.profiles.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(())
    }

    /// Get the total number of items (profiles + "Create New" option)
    fn item_count(&self) -> usize {
        self.profiles.len() + 1 // +1 for "Create New" option
    }

    /// Check if "Create New" option is currently selected
    fn is_create_new_selected(&self) -> bool {
        self.list_state.selected() == Some(self.profiles.len())
    }

    /// Handle keyboard input
    ///
    /// Returns `Some(result)` if the popup should close with a result,
    /// or `None` if the popup should stay open.
    pub fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        config: &Config,
    ) -> Option<ProfileSelectionResult> {
        let action = config.keymap.get_action(code, modifiers);

        // When "Create New" is selected, handle character input first
        if self.is_create_new_selected() {
            if let KeyCode::Char(c) = code {
                if !modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
                {
                    self.create_input.insert_char(c);
                    return None;
                }
            }
        }

        // Handle actions
        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    // Clear create input when navigating away from Create New
                    if self.is_create_new_selected() {
                        self.create_input.clear();
                    }

                    if let Some(current) = self.list_state.selected() {
                        if current > 0 {
                            self.list_state.select(Some(current - 1));
                        } else {
                            // Wrap to bottom
                            self.list_state.select(Some(self.item_count() - 1));
                        }
                    }
                }
                Action::MoveDown => {
                    // Clear create input when navigating away from Create New
                    if self.is_create_new_selected() {
                        self.create_input.clear();
                    }

                    if let Some(current) = self.list_state.selected() {
                        if current < self.item_count() - 1 {
                            self.list_state.select(Some(current + 1));
                        } else {
                            // Wrap to top
                            self.list_state.select(Some(0));
                        }
                    }
                }
                Action::Confirm => {
                    if let Some(idx) = self.list_state.selected() {
                        if idx < self.profiles.len() {
                            // Selected an existing profile
                            let name = self.profiles[idx].name.clone();
                            return Some(ProfileSelectionResult::SelectExisting(name));
                        } else {
                            // Selected "Create New"
                            let name = self.create_input.text_trimmed().to_string();
                            if !name.is_empty() {
                                return Some(ProfileSelectionResult::CreateNew(name));
                            }
                            // Empty name - do nothing (user needs to type a name)
                        }
                    }
                }
                Action::Cancel => {
                    if self.is_create_new_selected() && !self.create_input.text().is_empty() {
                        // First Esc clears the input
                        self.create_input.clear();
                    } else {
                        // Second Esc (or first if input empty) cancels
                        return Some(ProfileSelectionResult::Cancelled);
                    }
                }
                Action::Backspace => {
                    if self.is_create_new_selected() {
                        self.create_input.handle_action(Action::Backspace);
                    }
                }
                Action::MoveLeft => {
                    if self.is_create_new_selected() {
                        self.create_input.handle_action(Action::MoveLeft);
                    }
                }
                Action::MoveRight => {
                    if self.is_create_new_selected() {
                        self.create_input.handle_action(Action::MoveRight);
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Handle mouse events.
    ///
    /// Returns `Some(result)` if the popup should close with a result,
    /// or `None` if the popup should stay open.
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<ProfileSelectionResult> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(&idx) = self.mouse_regions.hit_test(mouse.column, mouse.row) {
                    self.list_state.select(Some(idx));
                    if idx < self.profiles.len() {
                        let name = self.profiles[idx].name.clone();
                        return Some(ProfileSelectionResult::SelectExisting(name));
                    }
                    // Clicked "Create New" — just select it, don't act
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(area) = self.list_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        if let Some(current) = self.list_state.selected() {
                            let new = current.saturating_sub(3);
                            self.list_state.select(Some(new));
                        }
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(area) = self.list_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        if let Some(current) = self.list_state.selected() {
                            let max = self.item_count().saturating_sub(1);
                            let new = (current + 3).min(max);
                            self.list_state.select(Some(new));
                        }
                    }
                }
            }
            _ => {}
        }
        None
    }

    /// Render the popup
    pub fn render(&mut self, frame: &mut Frame, area: Rect, config: &Config) {
        if !self.visible {
            return;
        }

        let icons = crate::icons::Icons::from_config(config);
        let t = theme();

        // Build footer text
        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Navigate | {}: Select | {}: Cancel",
            config.keymap.navigation_display(),
            k(Action::Confirm),
            k(Action::Cancel)
        );

        // Render popup frame
        let result: PopupRenderResult = Popup::new()
            .width(70)
            .height(60)
            .title("Select Profile to Activate")
            .dim_background(true)
            .footer(&footer_text)
            .render(frame, area);

        // Split content: Left (profile list), Right (profile preview)
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(result.content_area);

        // Render profile list
        self.render_profile_list(frame, chunks[0], &icons, &t);

        // Render profile preview
        self.render_profile_preview(frame, chunks[1], &icons, &t);
    }

    /// Render the profile list (left panel)
    fn render_profile_list(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        icons: &crate::icons::Icons,
        t: &crate::styles::Theme,
    ) {
        // Store list area for mouse scroll hit-testing
        self.list_area = Some(area);
        self.mouse_regions.clear();

        // Calculate inner area (inside borders) for click regions
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let item_count = self.item_count();
        let scroll_offset = self.list_state.offset();
        for i in 0..item_count {
            let visible_idx = i.saturating_sub(scroll_offset);
            if i >= scroll_offset && (visible_idx as u16) < inner.height {
                let row_rect = Rect::new(inner.x, inner.y + visible_idx as u16, inner.width, 1);
                self.mouse_regions.add(row_rect, i);
            }
        }

        let mut items: Vec<ListItem> = Vec::new();

        // Add existing profiles
        for (idx, profile) in self.profiles.iter().enumerate() {
            let is_selected = self.list_state.selected() == Some(idx);
            let file_info = if profile.file_count == 0 {
                "(empty)".to_string()
            } else if profile.file_count == 1 {
                "(1 file)".to_string()
            } else {
                format!("({} files)", profile.file_count)
            };

            let style = if is_selected {
                Style::default().fg(t.primary).add_modifier(Modifier::BOLD)
            } else {
                t.text_style()
            };

            let line = Line::from(vec![
                Span::styled(format!("  {} ", icons.profile()), style),
                Span::styled(&profile.name, style),
                Span::styled(format!(" {}", file_info), Style::default().fg(t.text_muted)),
            ]);
            items.push(ListItem::new(line));
        }

        // Add "Create New Profile" option
        let create_idx = self.profiles.len();
        let is_create_selected = self.list_state.selected() == Some(create_idx);
        let create_item = self.render_create_new_item(is_create_selected, icons, t);
        items.push(create_item);

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Profiles ")
                    .border_type(t.border_focused_type)
                    .border_style(focused_border_style()),
            )
            .highlight_style(Style::default().bg(t.highlight_bg))
            .highlight_symbol(crate::styles::LIST_HIGHLIGHT_SYMBOL);

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    /// Render the "Create New Profile" list item
    fn render_create_new_item(
        &self,
        is_selected: bool,
        icons: &crate::icons::Icons,
        t: &crate::styles::Theme,
    ) -> ListItem<'static> {
        let input_text = self.create_input.text();

        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let line = if is_selected && !input_text.is_empty() {
            // Show input with cursor
            let cursor_pos = self.create_input.cursor();
            let (before, after) = input_text.split_at(cursor_pos.min(input_text.len()));

            Line::from(vec![
                Span::styled(format!("  {} ", icons.create()), style),
                Span::styled(before.to_string(), style),
                Span::styled("│", Style::default().fg(Color::Yellow)), // Cursor
                Span::styled(after.to_string(), style),
            ])
        } else if is_selected {
            // Selected but no input yet - show placeholder
            Line::from(vec![
                Span::styled(format!("  {} ", icons.create()), style),
                Span::styled(
                    "Type name to create...",
                    Style::default()
                        .fg(t.text_muted)
                        .add_modifier(Modifier::ITALIC),
                ),
            ])
        } else {
            // Not selected
            Line::from(vec![
                Span::styled(format!("  {} ", icons.create()), style),
                Span::styled("Create New Profile", style),
            ])
        };

        ListItem::new(line)
    }

    /// Render the profile preview (right panel)
    fn render_profile_preview(
        &self,
        frame: &mut Frame,
        area: Rect,
        icons: &crate::icons::Icons,
        t: &crate::styles::Theme,
    ) {
        let content = if let Some(idx) = self.list_state.selected() {
            if idx < self.profiles.len() {
                // Show existing profile details
                self.build_profile_preview(&self.profiles[idx], icons, t)
            } else {
                // Show "Create New" preview
                self.build_create_new_preview(icons, t)
            }
        } else {
            Text::from("Select a profile")
        };

        let paragraph = Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Profile Details ")
                    .border_type(t.border_type(false))
                    .border_style(unfocused_border_style())
                    .padding(Padding::new(1, 1, 1, 1)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    /// Build the preview content for an existing profile
    fn build_profile_preview<'a>(
        &self,
        profile: &ProfileInfo,
        _icons: &crate::icons::Icons,
        t: &crate::styles::Theme,
    ) -> Text<'a> {
        let description = profile.description.as_deref().unwrap_or("No description");

        let files_text = if profile.file_count == 0 {
            "No files synced".to_string()
        } else {
            format!("{} files synced:", profile.file_count)
        };

        let files_list: String = if profile.files.is_empty() {
            String::new()
        } else {
            profile
                .files
                .iter()
                .take(10)
                .map(|f| format!("  • {f}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let more_text = if profile.file_count > 10 {
            format!("\n  ... and {} more", profile.file_count - 10)
        } else {
            String::new()
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    "Name: ",
                    Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                ),
                Span::styled(profile.name.clone(), t.text_style()),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Description:",
                Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                description.to_string(),
                Style::default().fg(t.text_muted),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                files_text,
                Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
            )]),
        ];

        if !files_list.is_empty() {
            for line in files_list.lines() {
                lines.push(Line::from(vec![Span::styled(
                    line.to_string(),
                    t.text_style(),
                )]));
            }
        }

        if !more_text.is_empty() {
            lines.push(Line::from(vec![Span::styled(
                more_text,
                Style::default().fg(t.text_muted),
            )]));
        }

        Text::from(lines)
    }

    /// Build the preview content for creating a new profile
    fn build_create_new_preview<'a>(
        &self,
        _icons: &crate::icons::Icons,
        t: &crate::styles::Theme,
    ) -> Text<'a> {
        let input_text = self.create_input.text();

        let name_display = if input_text.is_empty() {
            "(enter a name)".to_string()
        } else {
            input_text.to_string()
        };

        let name_style = if input_text.is_empty() {
            Style::default()
                .fg(t.text_muted)
                .add_modifier(Modifier::ITALIC)
        } else {
            t.text_style()
        };

        Text::from(vec![
            Line::from(vec![
                Span::styled(
                    "Name: ",
                    Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                ),
                Span::styled(name_display, name_style),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "This will create a new empty profile.",
                Style::default().fg(t.text_muted),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "After activation, you can add dotfiles",
                Style::default().fg(t.text_muted),
            )]),
            Line::from(vec![Span::styled(
                "to sync from the main menu.",
                Style::default().fg(t.text_muted),
            )]),
        ])
    }
}

impl Default for ProfileSelectionPopup {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_creation() {
        let popup = ProfileSelectionPopup::new();
        assert!(!popup.is_visible());
        assert!(popup.profiles.is_empty());
    }

    #[test]
    fn test_item_count_with_no_profiles() {
        let popup = ProfileSelectionPopup::new();
        assert_eq!(popup.item_count(), 1); // Just "Create New"
    }

    #[test]
    fn test_is_create_new_selected() {
        let mut popup = ProfileSelectionPopup::new();
        popup.list_state.select(Some(0));
        assert!(popup.is_create_new_selected()); // With no profiles, index 0 is Create New
    }
}
