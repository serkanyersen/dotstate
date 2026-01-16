//! Dotfile selection screen controller.
//!
//! This screen handles selecting and managing dotfiles for syncing.
//! It owns all state and rendering logic (self-contained screen).

use crate::components::file_preview::FilePreview;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::dialog::{Dialog, DialogVariant};
use crate::config::Config;
use crate::file_manager::Dotfile;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{theme as ui_theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::Screen as ScreenId;
use crate::utils::{
    center_popup, create_split_layout, create_standard_layout, focused_border_style,
    list_navigation::ListStateExt, unfocused_border_style, TextInput,
};
use crate::widgets::{TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
    ScrollbarOrientation, ScrollbarState, StatefulWidget, Wrap,
};
use ratatui::Frame;
use std::path::{Path, PathBuf};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

/// Display item for the dotfile list (header or file)
#[derive(Debug, Clone, PartialEq)]
enum DisplayItem {
    Header(String), // Section header
    File(usize),    // Index into state.dotfiles
}

/// Focus area in dotfile selection screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotfileSelectionFocus {
    FilesList,          // Files list pane is focused
    Preview,            // Preview pane is focused
    FileBrowserList,    // File browser list pane is focused
    FileBrowserPreview, // File browser preview pane is focused
    FileBrowserInput,   // File browser path input is focused
}

/// Dotfile selection state
#[derive(Debug)]
pub struct DotfileSelectionState {
    pub dotfiles: Vec<Dotfile>,
    pub preview_index: Option<usize>,
    pub preview_scroll: usize,
    pub selected_for_sync: std::collections::HashSet<usize>, // Indices of selected files
    pub dotfile_list_scrollbar: ScrollbarState,              // Scrollbar state for dotfile list
    pub dotfile_list_state: ListState, // ListState for main dotfile list (handles selection and scrolling)
    pub status_message: Option<String>, // For sync summary
    pub adding_custom_file: bool,      // Whether we're in "add custom file" mode
    pub custom_file_input: TextInput,  // Input for custom file path
    pub custom_file_focused: bool,     // Whether custom file input is focused
    pub file_browser_mode: bool,       // Whether we're in file browser mode
    pub file_browser_path: PathBuf,    // Current directory in file browser
    pub file_browser_selected: usize,  // Selected file index in browser
    pub file_browser_entries: Vec<PathBuf>, // Files/dirs in current directory
    pub file_browser_scrollbar: ScrollbarState, // Scrollbar state for file browser
    pub file_browser_list_state: ListState, // ListState for file browser (handles selection and scrolling)
    pub file_browser_preview_scroll: usize, // Scroll offset for file browser preview
    pub file_browser_path_input: TextInput, // Path input for file browser
    pub file_browser_path_focused: bool,    // Whether path input is focused
    pub focus: DotfileSelectionFocus,       // Which pane currently has focus
    pub backup_enabled: bool,               // Whether backups are enabled (tracks config value)
    // Custom file confirmation modal
    pub show_custom_file_confirm: bool, // Whether to show confirmation modal
    pub custom_file_confirm_path: Option<PathBuf>, // Full path to confirm
    pub custom_file_confirm_relative: Option<String>, // Relative path for confirmation
    // Move to/from common confirmation
    pub confirm_move: Option<usize>, // Index of dotfile to move (in dotfiles vec)
    // Move to common validation
    pub move_validation: Option<crate::utils::MoveToCommonValidation>, // Validation result when conflicts detected
}

impl Default for DotfileSelectionState {
    fn default() -> Self {
        Self {
            dotfiles: Vec::new(),
            preview_index: None,
            preview_scroll: 0,
            selected_for_sync: std::collections::HashSet::new(),
            dotfile_list_scrollbar: ScrollbarState::new(0),
            dotfile_list_state: ListState::default(),
            status_message: None,
            adding_custom_file: false,
            custom_file_input: TextInput::new(),
            custom_file_focused: true,
            file_browser_mode: false,
            file_browser_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            file_browser_selected: 0,
            file_browser_entries: Vec::new(),
            file_browser_scrollbar: ScrollbarState::new(0),
            file_browser_list_state: ListState::default(),
            file_browser_preview_scroll: 0,
            file_browser_path_input: TextInput::new(),
            file_browser_path_focused: false,
            focus: DotfileSelectionFocus::FilesList, // Start with files list focused
            backup_enabled: true,                    // Default to enabled
            show_custom_file_confirm: false,
            custom_file_confirm_path: None,
            custom_file_confirm_relative: None,
            confirm_move: None,
            move_validation: None,
        }
    }
}

/// Dotfile selection screen controller.
pub struct DotfileSelectionScreen {
    state: DotfileSelectionState,
}

impl DotfileSelectionScreen {
    /// Create a new dotfile selection screen.
    pub fn new() -> Self {
        Self {
            state: DotfileSelectionState::default(),
        }
    }

    /// Get the current state.
    pub fn get_state(&self) -> &DotfileSelectionState {
        &self.state
    }

    /// Get mutable state.
    pub fn get_state_mut(&mut self) -> &mut DotfileSelectionState {
        &mut self.state
    }

    /// Set backup enabled state.
    pub fn set_backup_enabled(&mut self, enabled: bool) {
        self.state.backup_enabled = enabled;
    }

    /// Generate display items for the list (headers and files)
    fn get_display_items(&self, profile_name: &str) -> Vec<DisplayItem> {
        let mut items = Vec::new();

        // 1. Common Files
        let common_indices: Vec<usize> = self
            .state
            .dotfiles
            .iter()
            .enumerate()
            .filter(|(_, d)| d.is_common)
            .map(|(i, _)| i)
            .collect();

        if !common_indices.is_empty() {
            items.push(DisplayItem::Header("Common Files (Shared)".to_string()));
            for idx in common_indices {
                items.push(DisplayItem::File(idx));
            }
        }

        // 2. Profile Files
        let profile_indices: Vec<usize> = self
            .state
            .dotfiles
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.is_common)
            .map(|(i, _)| i)
            .collect();

        if !profile_indices.is_empty() {
            if !items.is_empty() {
                items.push(DisplayItem::Header("".to_string())); // Spacer
            }
            items.push(DisplayItem::Header(format!(
                "Profile Files ({})",
                profile_name
            )));
            for idx in profile_indices {
                items.push(DisplayItem::File(idx));
            }
        }

        items
    }

    /// Handle modal confirmation events.
    fn handle_modal_event(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        match action {
            Some(Action::Yes) | Some(Action::Confirm) => {
                // YES logic - extract values and close modal
                let full_path = self.state.custom_file_confirm_path.clone().unwrap();
                let relative_path = self.state.custom_file_confirm_relative.clone().unwrap();
                self.state.show_custom_file_confirm = false;
                self.state.custom_file_confirm_path = None;
                self.state.custom_file_confirm_relative = None;

                Ok(ScreenAction::AddCustomFileToSync {
                    full_path,
                    relative_path,
                })
            }
            Some(Action::No) | Some(Action::Cancel) => {
                // NO logic - close modal
                self.state.show_custom_file_confirm = false;
                self.state.custom_file_confirm_path = None;
                self.state.custom_file_confirm_relative = None;
                Ok(ScreenAction::None)
            }
            _ => Ok(ScreenAction::None),
        }
    }

    /// Handle file browser input (when typing in path field).
    fn handle_file_browser_input(&mut self, key_code: KeyCode) -> Result<ScreenAction> {
        if !self.state.file_browser_path_focused
            || self.state.focus != DotfileSelectionFocus::FileBrowserInput
        {
            return Ok(ScreenAction::None);
        }

        match key_code {
            KeyCode::Char(c) => {
                self.state.file_browser_path_input.insert_char(c);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.state.file_browser_path_input.handle_key(key_code);
            }
            KeyCode::Backspace => {
                self.state.file_browser_path_input.backspace();
            }
            KeyCode::Delete => {
                self.state.file_browser_path_input.delete();
            }
            KeyCode::Enter => {
                // Load path from input
                let path_str = self.state.file_browser_path_input.text_trimmed();
                if !path_str.is_empty() {
                    let full_path = crate::utils::expand_path(path_str);

                    if full_path.exists() {
                        if full_path.is_dir() {
                            self.state.file_browser_path = full_path.clone();
                            self.state.file_browser_path_input.set_text(
                                self.state.file_browser_path.to_string_lossy().to_string(),
                            );
                            self.state.file_browser_list_state.select(Some(0));
                            self.state.focus = DotfileSelectionFocus::FileBrowserList;
                            return Ok(ScreenAction::RefreshFileBrowser);
                        } else {
                            // It's a file - sync it directly
                            let home_dir = crate::utils::get_home_dir();
                            let relative_path = full_path
                                .strip_prefix(&home_dir)
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|_| full_path.to_string_lossy().to_string());

                            // Close browser
                            self.state.file_browser_mode = false;
                            self.state.adding_custom_file = false;
                            self.state.file_browser_path_input.clear();
                            self.state.focus = DotfileSelectionFocus::FilesList;

                            return Ok(ScreenAction::AddCustomFileToSync {
                                full_path,
                                relative_path,
                            });
                        }
                    }
                }
            }
            KeyCode::Tab => {
                self.state.file_browser_path_focused = false;
                self.state.focus = DotfileSelectionFocus::FileBrowserList;
            }
            KeyCode::Esc => {
                // Close file browser modal
                self.state.file_browser_mode = false;
                self.state.adding_custom_file = false;
                self.state.file_browser_path_input.clear();
                self.state.focus = DotfileSelectionFocus::FilesList;
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    /// Handle custom file input (legacy mode, less common).
    fn handle_custom_file_input(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        // When input is not focused, only allow Enter to focus or Esc to cancel
        if !self.state.custom_file_focused {
            match key_code {
                KeyCode::Enter => {
                    self.state.custom_file_focused = true;
                    return Ok(ScreenAction::None);
                }
                KeyCode::Esc => {
                    self.state.adding_custom_file = false;
                    self.state.custom_file_input.clear();
                    return Ok(ScreenAction::None);
                }
                _ => return Ok(ScreenAction::None),
            }
        }

        // When focused, handle all input
        match key_code {
            KeyCode::Char(c) => {
                self.state.custom_file_input.insert_char(c);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.state.custom_file_input.handle_key(key_code);
            }
            KeyCode::Backspace => {
                self.state.custom_file_input.backspace();
            }
            KeyCode::Delete => {
                self.state.custom_file_input.delete();
            }
            KeyCode::Tab => {
                self.state.custom_file_focused = false;
            }
            KeyCode::Enter => {
                let path_str = self.state.custom_file_input.text_trimmed();
                if path_str.is_empty() {
                    self.state.status_message =
                        Some("Error: File path cannot be empty".to_string());
                } else {
                    let full_path = crate::utils::expand_path(path_str);

                    if !full_path.exists() {
                        self.state.status_message =
                            Some(format!("Error: File does not exist: {:?}", full_path));
                    } else {
                        // Calculate relative path
                        let home_dir = crate::utils::get_home_dir();
                        let relative_path = match full_path.strip_prefix(&home_dir) {
                            Ok(p) => p.to_string_lossy().to_string(),
                            Err(_) => path_str.to_string(),
                        };

                        // Close input mode
                        self.state.adding_custom_file = false;
                        self.state.custom_file_input.clear();
                        self.state.focus = DotfileSelectionFocus::FilesList;

                        // Validate before showing confirmation
                        let repo_path = &config.repo_path;
                        let (is_safe, reason) = crate::utils::is_safe_to_add(&full_path, repo_path);
                        if !is_safe {
                            self.state.status_message = Some(format!(
                                "Error: {}. Path: {}",
                                reason.unwrap_or_else(|| "Cannot add this file".to_string()),
                                full_path.display()
                            ));
                            return Ok(ScreenAction::None);
                        }

                        // Show confirmation modal
                        self.state.show_custom_file_confirm = true;
                        self.state.custom_file_confirm_path = Some(full_path);
                        self.state.custom_file_confirm_relative = Some(relative_path);
                    }
                }
            }
            KeyCode::Esc => {
                self.state.adding_custom_file = false;
                self.state.custom_file_input.clear();
                self.state.focus = DotfileSelectionFocus::FilesList;
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    /// Handle file browser list navigation and selection.
    fn handle_file_browser_list(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    self.state.file_browser_list_state.select_previous();
                }
                Action::MoveDown => {
                    self.state.file_browser_list_state.select_next();
                }
                Action::Confirm => {
                    if let Some(idx) = self.state.file_browser_list_state.selected() {
                        if idx < self.state.file_browser_entries.len() {
                            let entry = self.state.file_browser_entries[idx].clone();

                            // Handle special entries
                            if entry == Path::new("..") {
                                // Go to parent directory
                                if let Some(parent) = self.state.file_browser_path.parent() {
                                    self.state.file_browser_path = parent.to_path_buf();
                                    self.state.file_browser_path_input.set_text(
                                        self.state.file_browser_path.to_string_lossy().to_string(),
                                    );
                                    self.state.file_browser_list_state.select(Some(0));
                                    return Ok(ScreenAction::RefreshFileBrowser);
                                }
                            } else if entry == Path::new(".") {
                                // Add current folder
                                let current_folder = self.state.file_browser_path.clone();
                                let home_dir = crate::utils::get_home_dir();
                                let relative_path = current_folder
                                    .strip_prefix(&home_dir)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| {
                                        current_folder.to_string_lossy().to_string()
                                    });

                                // Validate
                                let repo_path = &config.repo_path;
                                let (is_safe, reason) =
                                    crate::utils::is_safe_to_add(&current_folder, repo_path);
                                if !is_safe {
                                    self.state.status_message = Some(format!(
                                            "Error: {}. Path: {}",
                                            reason.unwrap_or_else(
                                                || "Cannot add this folder".to_string()
                                            ),
                                            current_folder.display()
                                        ));
                                    return Ok(ScreenAction::None);
                                }

                                // Check if git repo
                                if crate::utils::is_git_repo(&current_folder) {
                                    self.state.status_message = Some(format!(
                                        "Error: Cannot sync a git repository. Path: {}",
                                        current_folder.display()
                                    ));
                                    return Ok(ScreenAction::None);
                                }

                                // Show confirmation modal
                                self.state.show_custom_file_confirm = true;
                                self.state.custom_file_confirm_path = Some(current_folder);
                                self.state.custom_file_confirm_relative = Some(relative_path);
                                self.state.file_browser_mode = false;
                                self.state.adding_custom_file = false;
                                self.state.file_browser_path_input.clear();
                                self.state.focus = DotfileSelectionFocus::FilesList;
                            } else {
                                // Regular file or directory
                                let full_path = if entry.is_absolute() {
                                    entry.clone()
                                } else {
                                    self.state.file_browser_path.join(&entry)
                                };

                                if full_path.is_dir() {
                                    // Navigate into directory
                                    self.state.file_browser_path = full_path.clone();
                                    self.state
                                        .file_browser_path_input
                                        .set_text(full_path.to_string_lossy().to_string());
                                    self.state.file_browser_list_state.select(Some(0));
                                    return Ok(ScreenAction::RefreshFileBrowser);
                                } else if full_path.is_file() {
                                    // Sync file directly
                                    let home_dir = crate::utils::get_home_dir();
                                    let relative_path = full_path
                                        .strip_prefix(&home_dir)
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|_| {
                                            full_path.to_string_lossy().to_string()
                                        });

                                    // Close browser
                                    self.state.file_browser_mode = false;
                                    self.state.adding_custom_file = false;
                                    self.state.file_browser_path_input.clear();
                                    self.state.focus = DotfileSelectionFocus::FilesList;

                                    return Ok(ScreenAction::AddCustomFileToSync {
                                        full_path,
                                        relative_path,
                                    });
                                }
                            }
                        }
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::FileBrowserPreview;
                    self.state.file_browser_path_focused = false;
                }
                Action::PageUp => {
                    self.state
                        .file_browser_list_state
                        .page_up(10, self.state.file_browser_entries.len());
                }
                Action::PageDown => {
                    self.state
                        .file_browser_list_state
                        .page_down(10, self.state.file_browser_entries.len());
                }
                Action::GoToTop => {
                    self.state.file_browser_list_state.select_first();
                }
                Action::GoToEnd => {
                    self.state.file_browser_list_state.select_last();
                }
                Action::Cancel | Action::Quit => {
                    // Close file browser
                    self.state.file_browser_mode = false;
                    self.state.adding_custom_file = false;
                    self.state.focus = DotfileSelectionFocus::FilesList;
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle file browser preview navigation.
    fn handle_file_browser_preview(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp | Action::ScrollUp => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_sub(1);
                }
                Action::MoveDown | Action::ScrollDown => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_add(1);
                }
                Action::PageUp => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_sub(20);
                }
                Action::PageDown => {
                    self.state.file_browser_preview_scroll =
                        self.state.file_browser_preview_scroll.saturating_add(20);
                }
                Action::GoToTop => {
                    self.state.file_browser_preview_scroll = 0;
                }
                Action::GoToEnd => {
                    self.state.file_browser_preview_scroll = 10000; // Will be clamped during render
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::FileBrowserInput;
                    self.state.file_browser_path_focused = true;
                }
                Action::Cancel | Action::Quit => {
                    self.state.file_browser_mode = false;
                    self.state.adding_custom_file = false;
                    self.state.focus = DotfileSelectionFocus::FilesList;
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle main dotfile list navigation and selection.
    fn handle_dotfile_list(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        let display_items = self.get_display_items(&config.active_profile);

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    if display_items.is_empty() {
                        return Ok(ScreenAction::None);
                    }

                    let current = self.state.dotfile_list_state.selected().unwrap_or(0);
                    // Find previous non-header item
                    let mut prev = current;
                    let mut found = false;
                    while prev > 0 {
                        prev -= 1;
                        if !matches!(display_items[prev], DisplayItem::Header(_)) {
                            found = true;
                            break;
                        }
                    }

                    if found {
                        self.state.dotfile_list_state.select(Some(prev));
                        self.state.preview_scroll = 0;
                    } else {
                        // If current is a header (which shouldn't happen usually but can at init),
                        // try to find first valid item from top
                        if matches!(display_items[current], DisplayItem::Header(_)) {
                            for (i, item) in display_items.iter().enumerate() {
                                if !matches!(item, DisplayItem::Header(_)) {
                                    self.state.dotfile_list_state.select(Some(i));
                                    break;
                                }
                            }
                        }
                    }
                }
                Action::MoveDown => {
                    if display_items.is_empty() {
                        return Ok(ScreenAction::None);
                    }

                    let current = self.state.dotfile_list_state.selected().unwrap_or(0);
                    // Find next non-header item
                    let mut next = current + 1;
                    while next < display_items.len() {
                        if !matches!(display_items[next], DisplayItem::Header(_)) {
                            self.state.dotfile_list_state.select(Some(next));
                            self.state.preview_scroll = 0;
                            break;
                        }
                        next += 1;
                    }

                    // If we didn't move and we are currently on a header (e.g. init), move to first valid
                    if next >= display_items.len()
                        && matches!(display_items[current], DisplayItem::Header(_))
                    {
                        // Try finding valid item from current downwards
                        let mut fix_idx = current + 1;
                        while fix_idx < display_items.len() {
                            if !matches!(display_items[fix_idx], DisplayItem::Header(_)) {
                                self.state.dotfile_list_state.select(Some(fix_idx));
                                break;
                            }
                            fix_idx += 1;
                        }
                    }
                }
                Action::Confirm => {
                    if self.state.status_message.is_some() {
                        self.state.status_message = None;
                    } else if let Some(idx) = self.state.dotfile_list_state.selected() {
                        if idx < display_items.len() {
                            if let DisplayItem::File(file_idx) = &display_items[idx] {
                                let is_synced = self.state.selected_for_sync.contains(file_idx);
                                return Ok(ScreenAction::ToggleFileSync {
                                    file_index: *file_idx,
                                    is_synced,
                                });
                            }
                        }
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::Preview;
                }
                Action::PageUp => {
                    if display_items.is_empty() {
                        return Ok(ScreenAction::None);
                    }
                    // Simple page up, then fix selection if on header
                    let current = self.state.dotfile_list_state.selected().unwrap_or(0);
                    let target = current.saturating_sub(10);
                    let mut next = target;

                    // Ensure we don't go below 0 (handled by usize)
                    // Fix if on header
                    if next < display_items.len()
                        && matches!(display_items[next], DisplayItem::Header(_))
                    {
                        next = next.saturating_add(1); // Move down one
                    }
                    if next >= display_items.len() {
                        next = current;
                    } // Fallback

                    self.state.dotfile_list_state.select(Some(next));
                    self.state.preview_scroll = 0;
                }
                Action::PageDown => {
                    if display_items.is_empty() {
                        return Ok(ScreenAction::None);
                    }
                    let current = self.state.dotfile_list_state.selected().unwrap_or(0);
                    let target = current.saturating_add(10);
                    let mut next = target;
                    if next >= display_items.len() {
                        next = display_items.len() - 1;
                    }

                    // Fix if on header
                    if matches!(display_items[next], DisplayItem::Header(_)) {
                        next = next.saturating_add(1);
                    }
                    if next >= display_items.len() {
                        next = current;
                    } // Fallback

                    self.state.dotfile_list_state.select(Some(next));
                    self.state.preview_scroll = 0;
                }
                Action::GoToTop => {
                    // Find first non-header item
                    if let Some(first_idx) = display_items
                        .iter()
                        .position(|item| matches!(item, DisplayItem::File(_)))
                    {
                        self.state.dotfile_list_state.select(Some(first_idx));
                    }
                    self.state.preview_scroll = 0;
                }
                Action::GoToEnd => {
                    // Find last non-header item
                    if let Some(last_idx) = display_items
                        .iter()
                        .rposition(|item| matches!(item, DisplayItem::File(_)))
                    {
                        self.state.dotfile_list_state.select(Some(last_idx));
                    }
                    self.state.preview_scroll = 0;
                }
                Action::Create => {
                    // Open file browser
                    self.state.adding_custom_file = true;
                    self.state.file_browser_mode = true;
                    self.state.file_browser_path = crate::utils::get_home_dir();
                    self.state.file_browser_selected = 0;
                    self.state
                        .file_browser_path_input
                        .set_text(self.state.file_browser_path.to_string_lossy().to_string());
                    self.state.file_browser_path_focused = false;
                    self.state.file_browser_preview_scroll = 0;
                    self.state.focus = DotfileSelectionFocus::FileBrowserList;
                    return Ok(ScreenAction::RefreshFileBrowser);
                }
                Action::ToggleBackup => {
                    self.state.backup_enabled = !self.state.backup_enabled;
                    return Ok(ScreenAction::SetBackupEnabled {
                        enabled: self.state.backup_enabled,
                    });
                }
                Action::Cancel | Action::Quit => {
                    return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                }
                Action::Move => {
                    if let Some(idx) = self.state.dotfile_list_state.selected() {
                        if idx < display_items.len() {
                            if let DisplayItem::File(file_idx) = &display_items[idx] {
                                let dotfile = &self.state.dotfiles[*file_idx];
                                if dotfile.synced {
                                    // Validate before showing confirmation
                                    if !dotfile.is_common {
                                        // Moving from profile to common - validate first
                                        let relative_path =
                                            dotfile.relative_path.to_string_lossy().to_string();
                                        match crate::utils::validate_move_to_common(
                                            &config.repo_path,
                                            &config.active_profile,
                                            &relative_path,
                                        ) {
                                            Ok(validation) => {
                                                self.state.move_validation = Some(validation);
                                                // If there are blocking conflicts, we'll show a different dialog
                                                // Otherwise, proceed with normal confirmation
                                                self.state.confirm_move = Some(*file_idx);
                                                return Ok(ScreenAction::Refresh);
                                            }
                                            Err(e) => {
                                                // Validation error - show error message
                                                return Ok(ScreenAction::ShowMessage {
                                                    title: "Validation Error".to_string(),
                                                    content: format!(
                                                        "Failed to validate move: {}",
                                                        e
                                                    ),
                                                });
                                            }
                                        }
                                    } else {
                                        // Moving from common to profile - no validation needed
                                        self.state.confirm_move = Some(*file_idx);
                                        return Ok(ScreenAction::Refresh);
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle preview pane navigation.
    fn handle_preview(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp | Action::ScrollUp => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_sub(1);
                }
                Action::MoveDown | Action::ScrollDown => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_add(1);
                }
                Action::PageUp => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_sub(20);
                }
                Action::PageDown => {
                    self.state.preview_scroll = self.state.preview_scroll.saturating_add(20);
                }
                Action::GoToTop => {
                    self.state.preview_scroll = 0;
                }
                Action::GoToEnd => {
                    // Calculate max scroll based on file content
                    if let Some(selected_index) = self.state.dotfile_list_state.selected() {
                        if selected_index < self.state.dotfiles.len() {
                            let dotfile = &self.state.dotfiles[selected_index];
                            if let Ok(content) = std::fs::read_to_string(&dotfile.original_path) {
                                let total_lines = content.lines().count();
                                let estimated_visible = 20;
                                self.state.preview_scroll =
                                    total_lines.saturating_sub(estimated_visible);
                            } else {
                                self.state.preview_scroll = 10000;
                            }
                        }
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::FilesList;
                }
                Action::Cancel | Action::Quit => {
                    return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                }
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }
    #[allow(clippy::too_many_arguments)] // Render function needs all these parameters
    fn render_file_browser(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        footer_chunk: Rect,
        config: &Config,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        // Create popup area (centered, 80% width, 70% height)
        let popup_area = center_popup(area, 80, 70);

        // Clear the popup area first (this is the key to making it a popup)
        frame.render_widget(Clear, popup_area);

        // File browser overlay - with path input field
        let browser_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Current path display
                Constraint::Length(3), // Path input field
                Constraint::Min(0),    // File list and preview
                Constraint::Length(2), // Footer (1 for border, 1 for text)
            ])
            .split(popup_area);

        // Current path display
        let path_display =
            Paragraph::new(self.state.file_browser_path.to_string_lossy().to_string()).block(
                Block::default()
                    // .borders(Borders::ALL)
                    .title(" Current Directory ")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().bg(Color::Reset)),
            );
        frame.render_widget(path_display, browser_chunks[0]);

        // Path input field
        let widget = TextInputWidget::new(&self.state.file_browser_path_input)
            .title("Path Input")
            .focused(self.state.file_browser_path_focused);
        frame.render_text_input_widget(widget, browser_chunks[1]);

        // Split list and preview
        let list_preview_chunks = create_split_layout(browser_chunks[2], &[50, 50]);

        // File list using ListState
        let items: Vec<ListItem> = self
            .state
            .file_browser_entries
            .iter()
            .map(|path| {
                let is_dir = if path == Path::new("..") {
                    true
                } else {
                    let full_path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        self.state.file_browser_path.join(path)
                    };
                    full_path.is_dir()
                };

                let name = if path == Path::new("..") {
                    ".. (parent)".to_string()
                } else if path == Path::new(".") {
                    ". (add this folder)".to_string()
                } else {
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string())
                };

                let icons = crate::icons::Icons::from_config(config);
                let prefix = if is_dir {
                    format!("{} ", icons.folder())
                } else {
                    format!("{} ", icons.file())
                };
                let display = format!("{}{}", prefix, name);

                ListItem::new(display)
            })
            .collect();

        // Update scrollbar state
        let total_items = self.state.file_browser_entries.len();
        let selected_index = self.state.file_browser_list_state.selected().unwrap_or(0);
        self.state.file_browser_scrollbar = self
            .state
            .file_browser_scrollbar
            .content_length(total_items)
            .position(selected_index);

        // Add focus indicator to file browser list
        let t = ui_theme();
        let list_title = " Select File or Directory (Enter to load path) ";
        let list_border_style = if self.state.focus == DotfileSelectionFocus::FileBrowserList {
            focused_border_style().bg(Color::Reset)
        } else {
            unfocused_border_style().bg(Color::Reset)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(list_title)
                    .border_type(BorderType::Rounded)
                    .title_alignment(Alignment::Center)
                    .border_style(list_border_style),
            )
            .highlight_style(t.highlight_style())
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        StatefulWidget::render(
            list,
            list_preview_chunks[0],
            frame.buffer_mut(),
            &mut self.state.file_browser_list_state,
        );

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_preview_chunks[0],
            &mut self.state.file_browser_scrollbar,
        );

        // Preview panel
        if let Some(selected_index) = self.state.file_browser_list_state.selected() {
            if selected_index < self.state.file_browser_entries.len() {
                let selected = &self.state.file_browser_entries[selected_index];
                let full_path = if selected == Path::new("..") {
                    self.state
                        .file_browser_path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("/"))
                } else if selected == Path::new(".") {
                    // Current folder
                    self.state.file_browser_path.clone()
                } else if selected.is_absolute() {
                    selected.to_path_buf()
                } else {
                    self.state.file_browser_path.join(selected)
                };

                let is_focused = self.state.focus == DotfileSelectionFocus::FileBrowserPreview;
                let preview_title = "Preview";

                FilePreview::render(
                    frame,
                    list_preview_chunks[1],
                    &full_path,
                    self.state.file_browser_preview_scroll,
                    is_focused,
                    Some(preview_title),
                    None,
                    syntax_set,
                    theme,
                )?;
            } else {
                let empty_preview = Paragraph::new("No selection").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(" Preview ")
                        .title_alignment(Alignment::Center),
                );
                frame.render_widget(empty_preview, list_preview_chunks[1]);
            }
        } else {
            let empty_preview = Paragraph::new("No selection").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Preview ")
                    .title_alignment(Alignment::Center),
            );
            frame.render_widget(empty_preview, list_preview_chunks[1]);
        }

        // Footer for file browser (inside popup)
        if browser_chunks.len() > 3 && browser_chunks[3].height > 0 {
            let footer_block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(t.text_muted))
                .style(Style::default().bg(Color::Reset));
            let footer_inner = footer_block.inner(browser_chunks[3]);
            let k = |a| config.keymap.get_key_display_for_action(a);
            let footer_text = format!(
                "{}: Switch Focus | {}: Navigate List | {}: Load Path | {}: Cancel",
                k(crate::keymap::Action::NextTab),
                config.keymap.navigation_display(),
                k(crate::keymap::Action::Confirm),
                k(crate::keymap::Action::Cancel)
            );
            let footer = Paragraph::new(footer_text)
                .style(Style::default().fg(t.text))
                .alignment(Alignment::Center);
            frame.render_widget(footer_block, browser_chunks[3]);
            frame.render_widget(footer, footer_inner);
        }

        // Also render main footer (outside popup, at bottom of screen)
        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "File Browser Active - {}: Cancel",
            k(crate::keymap::Action::Quit)
        );
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    fn render_custom_file_input(
        &mut self,
        frame: &mut Frame,
        content_chunk: Rect,
        footer_chunk: Rect,
        config: &Config,
    ) -> Result<()> {
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3), // Input field
            ])
            .split(content_chunk);

        let widget = TextInputWidget::new(&self.state.custom_file_input)
            .title("Custom File Path")
            .placeholder("Enter file path (e.g., ~/.myconfig or /path/to/file)")
            .title_alignment(Alignment::Center)
            .focused(self.state.custom_file_focused);
        frame.render_text_input_widget(widget, input_chunks[1]);

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Add File | {}: Cancel | Tab: Focus/Unfocus",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Quit)
        );
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn render_dotfile_list(
        &mut self,
        frame: &mut Frame,
        content_chunk: Rect,
        footer_chunk: Rect,
        config: &Config,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        // Split content into left (list + description) and right (preview)
        let (left_area, preview_area_opt) = if self.state.status_message.is_some() {
            (content_chunk, None::<Rect>)
        } else {
            let content_chunks = create_split_layout(content_chunk, &[50, 50]);
            (content_chunks[0], Some(content_chunks[1]))
        };
        let icons = crate::icons::Icons::from_config(config);
        // Split left area into list (top) and description (bottom)
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // List takes remaining space
                Constraint::Length(4), // Description block (3 lines + 1 border)
            ])
            .split(left_area);

        let list_area = left_chunks[0];
        let description_area = left_chunks[1];

        // File list using ListState - simplified, no descriptions inline
        let t = ui_theme();

        // Get display items (headers + files)
        let display_items = self.get_display_items(&config.active_profile);

        // Ensure valid selection (skip headers/spacers)
        // This handles initialization or if state gets desynced
        let current_sel = self.state.dotfile_list_state.selected().unwrap_or(0);
        if !display_items.is_empty() {
            // If selected index is out of bounds or points to a header, fix it
            let needs_fix = current_sel >= display_items.len()
                || matches!(display_items[current_sel], DisplayItem::Header(_));

            if needs_fix {
                // Try to find valid item from current position onwards
                let mut found = false;
                // First try current to end
                for (i, item) in display_items.iter().enumerate().skip(current_sel) {
                    if !matches!(item, DisplayItem::Header(_)) {
                        self.state.dotfile_list_state.select(Some(i));
                        found = true;
                        break;
                    }
                }
                // If not found, try from beginning
                if !found {
                    for (i, item) in display_items.iter().enumerate().take(current_sel) {
                        if !matches!(item, DisplayItem::Header(_)) {
                            self.state.dotfile_list_state.select(Some(i));
                            break;
                        }
                    }
                }
                // If still not found (e.g. only headers?), do nothing (or select None)
            }
        }

        // Count common vs profile files for title
        let common_count = self.state.dotfiles.iter().filter(|d| d.is_common).count();
        let profile_count = self.state.dotfiles.len() - common_count;

        #[allow(unused)] // list_idx is unused but is needed if we want to show tree structure
        let items: Vec<ListItem> = display_items
            .iter()
            .enumerate()
            .map(|(list_idx, item)| match item {
                DisplayItem::Header(title) => {
                    if title.is_empty() {
                        ListItem::new("").style(Style::default())
                    } else {
                        ListItem::new(title.to_string())
                            .style(Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD))
                    }
                }
                DisplayItem::File(idx) => {
                    let dotfile = &self.state.dotfiles[*idx];
                    let is_selected = self.state.selected_for_sync.contains(idx);
                    let sync_marker = if is_selected {
                        icons.check()
                    } else {
                        icons.uncheck()
                    };

                    // Indent files under headers
                    // Check if this is the last file in the section (next is header or end of list)

                    // let is_last_in_section = list_idx + 1 >= display_items.len()
                    //     || matches!(display_items[list_idx + 1], DisplayItem::Header(_));

                    // let prefix = if is_last_in_section {
                    //     "\u{2514}" // └
                    // } else {
                    //     "\u{251c}" // ├
                    // };

                    let prefix = "";

                    let style = if is_selected {
                        Style::default().fg(t.success)
                    } else {
                        t.text_style()
                    };

                    let path_str = dotfile.relative_path.to_string_lossy();
                    let content = ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(prefix.to_string(), Style::default()),
                        ratatui::text::Span::styled(
                            format!(" {}\u{2009}{}", sync_marker, path_str),
                            style,
                        ),
                    ]);
                    ListItem::new(content)
                }
            })
            .collect();

        // Update scrollbar state
        let total_items = display_items.len();
        let selected_index = self.state.dotfile_list_state.selected().unwrap_or(0);
        self.state.dotfile_list_scrollbar = self
            .state
            .dotfile_list_scrollbar
            .content_length(total_items)
            .position(selected_index);

        // Add focus indicator to files list with common/profile breakdown
        let list_title = if common_count > 0 {
            format!(
                " Dotfiles ({} common, {} profile) ",
                common_count, profile_count
            )
        } else {
            format!(" Found {} dotfiles ", self.state.dotfiles.len())
        };
        let list_border_style = if self.state.focus == DotfileSelectionFocus::FilesList {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(list_title)
                    .title_alignment(Alignment::Center)
                    .border_type(BorderType::Rounded)
                    .border_style(list_border_style),
            )
            .highlight_style(t.highlight_style())
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        StatefulWidget::render(
            list,
            list_area,
            frame.buffer_mut(),
            &mut self.state.dotfile_list_state,
        );

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_area,
            &mut self.state.dotfile_list_scrollbar,
        );

        // Get selected dotfile (if any)
        let selected_dotfile = if let Some(idx) = self.state.dotfile_list_state.selected() {
            if idx < display_items.len() {
                if let DisplayItem::File(file_idx) = &display_items[idx] {
                    Some(&self.state.dotfiles[*file_idx])
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Description block
        if let Some(dotfile) = selected_dotfile {
            let description_text = if let Some(desc) = &dotfile.description {
                desc.clone()
            } else {
                format!(
                    "No description available for {}",
                    dotfile.relative_path.to_string_lossy()
                )
            };

            let description_para = Paragraph::new(description_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Description ")
                        .border_type(BorderType::Rounded)
                        .title_alignment(Alignment::Center)
                        .border_style(unfocused_border_style()),
                )
                .wrap(Wrap { trim: true })
                .style(t.text_style());
            frame.render_widget(description_para, description_area);
        } else {
            let empty_desc = Paragraph::new("No file selected").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Description ")
                    .border_type(BorderType::Rounded)
                    .title_alignment(Alignment::Center)
                    .border_style(unfocused_border_style()),
            );
            frame.render_widget(empty_desc, description_area);
        }

        // Preview panel
        if let Some(preview_rect) = preview_area_opt {
            if let Some(dotfile) = selected_dotfile {
                let is_focused = self.state.focus == DotfileSelectionFocus::Preview;
                let preview_title = format!("Preview: {}", dotfile.relative_path.to_string_lossy());

                FilePreview::render(
                    frame,
                    preview_rect,
                    &dotfile.original_path,
                    self.state.preview_scroll,
                    is_focused,
                    Some(&preview_title),
                    None,
                    syntax_set,
                    theme,
                )?;
            } else {
                let empty_preview = Paragraph::new("No file selected").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Preview ")
                        .border_type(BorderType::Rounded)
                        .title_alignment(Alignment::Center),
                );
                frame.render_widget(empty_preview, preview_rect);
            }
        }

        // Status message overlay
        if let Some(status) = &self.state.status_message {
            let status_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(10)])
                .split(content_chunk);

            frame.render_widget(Clear, status_chunks[1]);
            frame.render_widget(
                Block::default().style(Style::default().bg(t.background)),
                status_chunks[1],
            );

            let status_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Sync Summary ")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(t.background));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, status_chunks[1]);
        }

        // Footer
        let backup_status = if self.state.backup_enabled {
            "ON"
        } else {
            "OFF"
        };
        let footer_text = if self.state.status_message.is_some() {
            let k = |a| config.keymap.get_key_display_for_action(a);
            format!("{}: Continue", k(crate::keymap::Action::Confirm))
        } else {
            let k = |a| config.keymap.get_key_display_for_action(a);

            // Determine move action text based on selected file
            let display_items = self.get_display_items(&config.active_profile);
            let move_text = self
                .state
                .dotfile_list_state
                .selected()
                .and_then(|idx| display_items.get(idx))
                .and_then(|item| match item {
                    DisplayItem::File(file_idx) => self.state.dotfiles.get(*file_idx),
                    _ => None,
                })
                .map(|dotfile| {
                    if dotfile.is_common {
                        "Move to Profile"
                    } else {
                        "Move to Common"
                    }
                })
                .unwrap_or("Move");

            format!(
                "Tab: Focus | {}: Navigate | Space/{}: Toggle | {}: {} | {}: Add Custom | {}: Backup ({}) | {}: Back",
                 config.keymap.navigation_display(),
                 k(crate::keymap::Action::Confirm),
                 k(crate::keymap::Action::Move),
                 move_text,
                 k(crate::keymap::Action::Create),
                 k(crate::keymap::Action::ToggleBackup),
                 backup_status,
                 k(crate::keymap::Action::Quit)
            )
        };

        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    fn render_custom_file_confirm(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        let path = self
            .state
            .custom_file_confirm_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let content = format!(
            "Path: {}\n\n\
            ⚠️  This will move this path to the storage repo and replace it with a symlink.\n\
            Make sure you know what you are doing.",
            path
        );

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Confirm | {}: Cancel",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Quit)
        );

        let dialog = Dialog::new("Confirm Add Custom File", &content)
            .height(40)
            .dim_background(true)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }

    fn handle_move_confirm(&mut self, key_code: KeyCode, config: &Config) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);

        // Handle common actions
        if let Some(action) = action {
            match action {
                crate::keymap::Action::Confirm => {
                    if let Some(idx) = self.state.confirm_move {
                        if idx < self.state.dotfiles.len() {
                            let dotfile = &self.state.dotfiles[idx];

                            // Check if we're in a blocked dialog (path conflict)
                            if let Some(ref validation) = self.state.move_validation {
                                let has_path_conflict = validation.conflicts.iter().any(|c| {
                                    matches!(c, crate::utils::MoveToCommonConflict::PathHierarchyConflict { .. })
                                });
                                if has_path_conflict {
                                    // Just close the dialog - can't proceed
                                    self.state.confirm_move = None;
                                    self.state.move_validation = None;
                                    return Ok(ScreenAction::Refresh);
                                }
                            }

                            // Get profiles to cleanup from validation
                            let profiles_to_cleanup = self
                                .state
                                .move_validation
                                .as_ref()
                                .map(|v| v.profiles_to_cleanup.clone())
                                .unwrap_or_default();

                            let action = ScreenAction::MoveToCommon {
                                file_index: idx,
                                is_common: dotfile.is_common,
                                profiles_to_cleanup,
                            };
                            self.state.confirm_move = None;
                            self.state.move_validation = None;
                            return Ok(action);
                        }
                    }
                    self.state.confirm_move = None;
                    self.state.move_validation = None;
                    return Ok(ScreenAction::Refresh);
                }
                crate::keymap::Action::Quit | crate::keymap::Action::Cancel => {
                    self.state.confirm_move = None;
                    self.state.move_validation = None;
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }

        // Handle explicit chars 'y' and 'n'
        match key_code {
            KeyCode::Char('y') | KeyCode::Char('f') => {
                if let Some(idx) = self.state.confirm_move {
                    if idx < self.state.dotfiles.len() {
                        let dotfile = &self.state.dotfiles[idx];

                        // Check if we're in a blocked dialog (path conflict)
                        if let Some(ref validation) = self.state.move_validation {
                            let has_path_conflict = validation.conflicts.iter().any(|c| {
                                matches!(
                                    c,
                                    crate::utils::MoveToCommonConflict::PathHierarchyConflict { .. }
                                )
                            });
                            if has_path_conflict {
                                // Just close the dialog - can't proceed
                                self.state.confirm_move = None;
                                self.state.move_validation = None;
                                return Ok(ScreenAction::Refresh);
                            }
                        }

                        // Get profiles to cleanup from validation
                        let profiles_to_cleanup = self
                            .state
                            .move_validation
                            .as_ref()
                            .map(|v| v.profiles_to_cleanup.clone())
                            .unwrap_or_default();

                        let action = ScreenAction::MoveToCommon {
                            file_index: idx,
                            is_common: dotfile.is_common,
                            profiles_to_cleanup,
                        };
                        self.state.confirm_move = None;
                        self.state.move_validation = None;
                        return Ok(action);
                    }
                }
                self.state.confirm_move = None;
                self.state.move_validation = None;
                Ok(ScreenAction::Refresh)
            }
            KeyCode::Char('n') => {
                self.state.confirm_move = None;
                self.state.move_validation = None;
                Ok(ScreenAction::Refresh)
            }
            _ => Ok(ScreenAction::None),
        }
    }

    fn render_move_confirm(&self, frame: &mut Frame, area: Rect, config: &Config) -> Result<()> {

        let dotfile_name = if let Some(idx) = self.state.confirm_move {
            if idx < self.state.dotfiles.len() {
                self.state.dotfiles[idx].relative_path.display().to_string()
            } else {
                "Unknown".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        let is_moving_to_common = if let Some(idx) = self.state.confirm_move {
            if idx < self.state.dotfiles.len() {
                !self.state.dotfiles[idx].is_common
            } else {
                false
            }
        } else {
            false
        };

        // Check validation result to determine which dialog to show
        if let Some(ref validation) = self.state.move_validation {
            // Check for blocking conflicts
            let has_blocking = validation.conflicts.iter().any(|c| {
                matches!(
                    c,
                    crate::utils::MoveToCommonConflict::DifferentContentInProfile { .. }
                        | crate::utils::MoveToCommonConflict::PathHierarchyConflict { .. }
                )
            });

            if has_blocking {
                // Check if it's a path hierarchy conflict (most critical)
                let has_path_conflict = validation.conflicts.iter().any(|c| {
                    matches!(
                        c,
                        crate::utils::MoveToCommonConflict::PathHierarchyConflict { .. }
                    )
                });

                if has_path_conflict {
                    return self.render_move_blocked_dialog(frame, area, config);
                } else {
                    // Different content conflict
                    return self.render_move_force_dialog(frame, area, config);
                }
            }
            // Otherwise fall through to normal confirmation (same content conflicts are auto-resolved)
        }

        // Normal confirmation dialog (no blocking conflicts)
        let title_text = if is_moving_to_common {
            "Confirm Move to Common"
        } else {
            "Confirm Move to Profile"
        };

        // Message
        let msg = if is_moving_to_common {
            format!(
                "Move '{}' to common files?\nIt will become available to all profiles.",
                dotfile_name
            )
        } else {
            format!(
                "Move '{}' back to profile?\nIt will no longer be available to other profiles.",
                dotfile_name
            )
        };

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Confirm | {}: Cancel",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Quit)
        );

        let dialog = Dialog::new(title_text, &msg)
            .height(20)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }

    fn render_move_force_dialog(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {

        let dotfile_name = if let Some(idx) = self.state.confirm_move {
            if idx < self.state.dotfiles.len() {
                self.state.dotfiles[idx].relative_path.display().to_string()
            } else {
                "Unknown".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        // Build conflict list
        let mut conflict_lines = Vec::new();
        if let Some(ref validation) = self.state.move_validation {
            for conflict in &validation.conflicts {
                if let crate::utils::MoveToCommonConflict::DifferentContentInProfile {
                    profile_name,
                    size_diff,
                } = conflict
                {
                    let size_text = if let Some((size1, size2)) = size_diff {
                        format!(" ({} vs {})", format_size(*size1), format_size(*size2))
                    } else {
                        String::new()
                    };
                    conflict_lines.push(format!("  • {}{}", profile_name, size_text));
                }
            }
        }

        let conflict_list = conflict_lines.join("\n");
        let msg = format!(
            "⚠ \"{}\" exists in other profiles with DIFFERENT\n\
            content:\n\n{}\n\n\
            If you proceed, their versions will be DELETED and\n\
            replaced with the common version.\n\n\
            Tip: To preserve different configs, remove them from\n\
            sync first in each profile.",
            dotfile_name, conflict_list
        );

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Force (delete others) | {}: Cancel",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Quit)
        );

        let dialog = Dialog::new("Content Differs", &msg)
            // .height(40)
            .variant(DialogVariant::Warning)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }

    fn render_move_blocked_dialog(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {

        let dotfile_name = if let Some(idx) = self.state.confirm_move {
            if idx < self.state.dotfiles.len() {
                self.state.dotfiles[idx].relative_path.display().to_string()
            } else {
                "Unknown".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        // Build conflict message
        let mut conflict_msg = String::new();
        if let Some(ref validation) = self.state.move_validation {
            for conflict in &validation.conflicts {
                if let crate::utils::MoveToCommonConflict::PathHierarchyConflict {
                    profile_name,
                    conflicting_path,
                    is_parent,
                } = conflict
                {
                    if *is_parent {
                        conflict_msg.push_str(&format!(
                            "  You are trying to move: {}\n\
                            But profile \"{}\" has: {} (directory)\n\n",
                            dotfile_name, profile_name, conflicting_path
                        ));
                    } else {
                        conflict_msg.push_str(&format!(
                            "  You are trying to move: {} (directory)\n\
                            But profile \"{}\" has: {}\n\n",
                            dotfile_name, profile_name, conflicting_path
                        ));
                    }
                }
            }
        }

        let msg = format!(
            "✗ Path conflict detected:\n\n{}\
            This would create an invalid state.\n\n\
            To fix: Remove the conflicting path from sync in the\n\
            affected profile first.",
            conflict_msg
        );

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!("{}: OK", k(crate::keymap::Action::Confirm));

        let dialog = Dialog::new("Cannot Move to Common", &msg)
            // .height(35)
            .variant(DialogVariant::Error)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }
}

// Helper function to format file sizes
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

impl Default for DotfileSelectionScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DotfileSelectionScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Clear the entire area first to prevent background bleed-through
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let background = Block::default().style(Style::default().bg(Color::Reset));
        frame.render_widget(background, area);

        // Layout: Title/Description, Content (list + preview), Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Manage Files",
            "Add or remove files to your repository. You can also add custom files. We have automatically detected some common dotfiles for you."
        )?;

        // Check if file browser is active - render as popup (instead of main content)
        if self.state.file_browser_mode {
            self.render_file_browser(
                frame,
                area,
                footer_chunk,
                ctx.config,
                ctx.syntax_set,
                ctx.syntax_theme,
            )?;
        } else if self.state.adding_custom_file {
            self.render_custom_file_input(frame, content_chunk, footer_chunk, ctx.config)?;
        } else {
            // Render main dotfile list content
            self.render_dotfile_list(
                frame,
                content_chunk,
                footer_chunk,
                ctx.config,
                ctx.syntax_set,
                ctx.syntax_theme,
            )?;
        }

        // Render modals on top of the content (not instead of it)
        if self.state.show_custom_file_confirm {
            self.render_custom_file_confirm(frame, area, ctx.config)?;
        } else if self.state.confirm_move.is_some() {
            // Move confirmation modals render on top of the main content
            self.render_move_confirm(frame, area, ctx.config)?;
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        // 1. Modal first - captures all events
        if self.state.show_custom_file_confirm {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    return self.handle_modal_event(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        if self.state.confirm_move.is_some() {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    return self.handle_move_confirm(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 2. File browser input mode
        if self.state.file_browser_mode
            && self.state.file_browser_path_focused
            && self.state.focus == DotfileSelectionFocus::FileBrowserInput
        {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    // Check for Tab/NextTab to switch focus
                    if let Some(action) = ctx.config.keymap.get_action(key.code, key.modifiers) {
                        use crate::keymap::Action;
                        if matches!(action, Action::NextTab) {
                            self.state.focus = DotfileSelectionFocus::FileBrowserList;
                            self.state.file_browser_path_focused = false;
                            return Ok(ScreenAction::None);
                        }
                    }
                    return self.handle_file_browser_input(key.code);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 3. Custom file input mode (legacy)
        if self.state.adding_custom_file && !self.state.file_browser_mode {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    return self.handle_custom_file_input(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 4. Normal navigation based on focus
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                if self.state.file_browser_mode {
                    // File browser mode navigation
                    match self.state.focus {
                        DotfileSelectionFocus::FileBrowserList => {
                            return self.handle_file_browser_list(key.code, ctx.config);
                        }
                        DotfileSelectionFocus::FileBrowserPreview => {
                            return self.handle_file_browser_preview(key.code, ctx.config);
                        }
                        DotfileSelectionFocus::FileBrowserInput => {
                            // Shouldn't reach here as input mode is handled above
                            return self.handle_file_browser_input(key.code);
                        }
                        _ => {}
                    }
                } else {
                    // Normal mode navigation
                    match self.state.focus {
                        DotfileSelectionFocus::FilesList => {
                            return self.handle_dotfile_list(key.code, ctx.config);
                        }
                        DotfileSelectionFocus::Preview => {
                            return self.handle_preview(key.code, ctx.config);
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        if self.state.file_browser_mode {
            self.state.file_browser_path_focused
                && self.state.focus == DotfileSelectionFocus::FileBrowserInput
        } else if self.state.adding_custom_file {
            self.state.custom_file_focused
        } else {
            false
        }
    }

    fn on_enter(&mut self, _ctx: &ScreenContext) -> Result<()> {
        // Request scan on enter - this will be handled by app
        // Note: We return None here but app should call scan_dotfiles when navigating to this screen
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dotfile_selection_screen_creation() {
        let screen = DotfileSelectionScreen::new();
        assert!(!screen.is_input_focused());
        assert!(screen.state.dotfiles.is_empty());
    }

    #[test]
    fn test_set_backup_enabled() {
        let mut screen = DotfileSelectionScreen::new();
        assert!(screen.state.backup_enabled);
        screen.set_backup_enabled(false);
        assert!(!screen.state.backup_enabled);
    }
}
