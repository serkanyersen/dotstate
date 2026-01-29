//! Dotfile selection screen controller.
//!
//! This screen handles selecting and managing dotfiles for syncing.
//! It owns all state and rendering logic (self-contained screen).

use crate::components::file_preview::FilePreview;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::{FileBrowser, FileBrowserResult};
use crate::config::Config;
use crate::file_manager::Dotfile;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::screens::ActionResult;
use crate::services::SyncService;
use crate::styles::{theme as ui_theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::Screen as ScreenId;
use crate::utils::{
    create_split_layout, create_standard_layout, focused_border_style, unfocused_border_style,
    TextInput,
};
use crate::widgets::{Dialog, DialogVariant};
use crate::widgets::{TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use tracing::{debug, info, warn};

use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, StatefulWidget, Wrap,
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

/// Actions that can be processed by the dotfile selection screen
#[derive(Debug, Clone)]
pub enum DotfileAction {
    /// Scan for dotfiles and refresh the list
    ScanDotfiles,
    /// Refresh the file browser entries
    RefreshFileBrowser,
    /// Toggle file sync status (add or remove from sync)
    ToggleFileSync { file_index: usize, is_synced: bool },
    /// Add a custom file to sync
    AddCustomFileToSync {
        full_path: PathBuf,
        relative_path: String,
    },
    /// Update backup enabled setting
    SetBackupEnabled { enabled: bool },
    /// Move a file to/from common
    MoveToCommon {
        file_index: usize,
        is_common: bool,
        profiles_to_cleanup: Vec<String>,
    },
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
    // Unsync common file confirmation
    pub confirm_unsync_common: Option<usize>, // Index of common file to unsync
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
            confirm_unsync_common: None,
        }
    }
}

/// Dotfile selection screen controller.
pub struct DotfileSelectionScreen {
    state: DotfileSelectionState,
    /// File browser component
    file_browser: FileBrowser,
}

impl DotfileSelectionScreen {
    /// Create a new dotfile selection screen.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: DotfileSelectionState::default(),
            file_browser: FileBrowser::new(),
        }
    }

    /// Get the current state.
    #[must_use]
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
                items.push(DisplayItem::Header(String::new())); // Spacer
            }
            items.push(DisplayItem::Header(format!(
                "Profile Files ({profile_name})"
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
            Some(Action::Yes | Action::Confirm) => {
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
            Some(Action::No | Action::Cancel) => {
                // NO logic - close modal
                self.state.show_custom_file_confirm = false;
                self.state.custom_file_confirm_path = None;
                self.state.custom_file_confirm_relative = None;
                Ok(ScreenAction::None)
            }
            _ => Ok(ScreenAction::None),
        }
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
                    return Ok(ScreenAction::ShowMessage {
                        title: "Invalid Path".to_string(),
                        content: "File path cannot be empty".to_string(),
                    });
                } else {
                    let full_path = crate::utils::expand_path(path_str);

                    if full_path.exists() {
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
                            return Ok(ScreenAction::ShowMessage {
                                title: "Cannot Add File".to_string(),
                                content: format!(
                                    "{}.\n\nPath: {}",
                                    reason.unwrap_or_else(|| "Cannot add this file".to_string()),
                                    full_path.display()
                                ),
                            });
                        }

                        // Show confirmation modal
                        self.state.show_custom_file_confirm = true;
                        self.state.custom_file_confirm_path = Some(full_path);
                        self.state.custom_file_confirm_relative = Some(relative_path);
                    } else {
                        return Ok(ScreenAction::ShowMessage {
                            title: "File Not Found".to_string(),
                            content: format!("File does not exist: {full_path:?}"),
                        });
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
                    if let Some(idx) = self.state.dotfile_list_state.selected() {
                        if idx < display_items.len() {
                            if let DisplayItem::File(file_idx) = &display_items[idx] {
                                let is_synced = self.state.selected_for_sync.contains(file_idx);
                                let dotfile = &self.state.dotfiles[*file_idx];

                                // If unsyncing a common file, show confirmation dialog
                                if is_synced && dotfile.is_common {
                                    self.state.confirm_unsync_common = Some(*file_idx);
                                    return Ok(ScreenAction::Refresh);
                                }

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
                    self.file_browser.open(crate::utils::get_home_dir());
                    return Ok(ScreenAction::None);
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
                                                        "Failed to validate move: {e}"
                                                    ),
                                                });
                                            }
                                        }
                                    }
                                    // Moving from common to profile - no validation needed
                                    self.state.confirm_move = Some(*file_idx);
                                    return Ok(ScreenAction::Refresh);
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
        let content_chunks = create_split_layout(content_chunk, &[50, 50]);
        let left_area = content_chunks[0];
        let preview_area = content_chunks[1];
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
                        ListItem::new(title.clone())
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
                            format!(" {sync_marker}\u{2009}{path_str}"),
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
            format!(" Dotfiles ({common_count} common, {profile_count} profile) ")
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
                    .border_type(
                        t.border_type(self.state.focus == DotfileSelectionFocus::FilesList),
                    )
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
                        .border_type(t.border_type(false))
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
                    .border_type(ui_theme().border_type(false))
                    .title_alignment(Alignment::Center)
                    .border_style(unfocused_border_style()),
            );
            frame.render_widget(empty_desc, description_area);
        }

        // Preview panel
        if let Some(dotfile) = selected_dotfile {
            let is_focused = self.state.focus == DotfileSelectionFocus::Preview;
            let preview_title = format!("Preview: {}", dotfile.relative_path.to_string_lossy());

            FilePreview::render(
                frame,
                preview_area,
                &dotfile.original_path,
                self.state.preview_scroll,
                is_focused,
                Some(&preview_title),
                None,
                syntax_set,
                theme,
                config,
            )?;
        } else {
            let empty_preview = Paragraph::new("No file selected").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Preview ")
                    .border_type(ui_theme().border_type(false))
                    .title_alignment(Alignment::Center),
            );
            frame.render_widget(empty_preview, preview_area);
        }

        // Footer
        let backup_status = if self.state.backup_enabled {
            "ON"
        } else {
            "OFF"
        };
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
            .map_or("Move", |dotfile| {
                if dotfile.is_common {
                    "Move to Profile"
                } else {
                    "Move to Common"
                }
            });

        let footer_text = format!(
            "Tab: Focus | {}: Navigate | Space/{}: Toggle | {}: {} | {}: Add Custom | {}: Backup ({}) | {}: Back",
             config.keymap.navigation_display(),
             k(crate::keymap::Action::Confirm),
             k(crate::keymap::Action::Move),
             move_text,
             k(crate::keymap::Action::Create),
             k(crate::keymap::Action::ToggleBackup),
             backup_status,
             k(crate::keymap::Action::Quit)
        );

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
            .map_or_else(|| "Unknown".to_string(), |p| p.display().to_string());

        let content = format!(
            "Path: {path}\n\n\
            ⚠️  This will move this path to the storage repo and replace it with a symlink.\n\
            Make sure you know what you are doing."
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
                            // Include both auto-resolvable (same content) AND forced (different content) profiles
                            let profiles_to_cleanup = self
                                .state
                                .move_validation
                                .as_ref()
                                .map(|v| {
                                    let mut profiles = v.profiles_to_cleanup.clone();
                                    // When user confirms/forces, also include profiles with different content
                                    for conflict in &v.conflicts {
                                        if let crate::utils::MoveToCommonConflict::DifferentContentInProfile {
                                            profile_name,
                                            ..
                                        } = conflict
                                        {
                                            if !profiles.contains(profile_name) {
                                                profiles.push(profile_name.clone());
                                            }
                                        }
                                    }
                                    profiles
                                })
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
            KeyCode::Char('y' | 'f') => {
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
                        // Include both auto-resolvable (same content) AND forced (different content) profiles
                        let profiles_to_cleanup = self
                            .state
                            .move_validation
                            .as_ref()
                            .map(|v| {
                                let mut profiles = v.profiles_to_cleanup.clone();
                                // When user forces, also include profiles with different content
                                for conflict in &v.conflicts {
                                    if let crate::utils::MoveToCommonConflict::DifferentContentInProfile {
                                        profile_name,
                                        ..
                                    } = conflict
                                    {
                                        if !profiles.contains(profile_name) {
                                            profiles.push(profile_name.clone());
                                        }
                                    }
                                }
                                profiles
                            })
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

    fn handle_unsync_common_confirm(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<ScreenAction> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);

        // Handle common actions
        if let Some(action) = action {
            match action {
                crate::keymap::Action::Confirm => {
                    if let Some(idx) = self.state.confirm_unsync_common {
                        self.state.confirm_unsync_common = None;
                        return Ok(ScreenAction::ToggleFileSync {
                            file_index: idx,
                            is_synced: true,
                        });
                    }
                    self.state.confirm_unsync_common = None;
                    return Ok(ScreenAction::Refresh);
                }
                crate::keymap::Action::Quit | crate::keymap::Action::Cancel => {
                    self.state.confirm_unsync_common = None;
                    return Ok(ScreenAction::Refresh);
                }
                _ => {}
            }
        }

        // Handle explicit chars 'y' and 'n'
        match key_code {
            KeyCode::Char('y') => {
                if let Some(idx) = self.state.confirm_unsync_common {
                    self.state.confirm_unsync_common = None;
                    return Ok(ScreenAction::ToggleFileSync {
                        file_index: idx,
                        is_synced: true,
                    });
                }
                self.state.confirm_unsync_common = None;
                Ok(ScreenAction::Refresh)
            }
            KeyCode::Char('n') => {
                self.state.confirm_unsync_common = None;
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
                }
                // Different content conflict
                return self.render_move_force_dialog(frame, area, config);
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
                "Move '{dotfile_name}' to common files?\nIt will become available to all profiles."
            )
        } else {
            format!(
                "Move '{dotfile_name}' back to profile?\nIt will no longer be available to other profiles."
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
                    conflict_lines.push(format!("  • {profile_name}{size_text}"));
                }
            }
        }

        let conflict_list = conflict_lines.join("\n");
        let msg = format!(
            "⚠ \"{dotfile_name}\" exists in other profiles with DIFFERENT\n\
            content:\n\n{conflict_list}\n\n\
            If you proceed, their versions will be DELETED and\n\
            replaced with the common version.\n\n\
            Tip: To preserve different configs, remove them from\n\
            sync first in each profile."
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
                            "  You are trying to move: {dotfile_name}\n\
                            But profile \"{profile_name}\" has: {conflicting_path} (directory)\n\n"
                        ));
                    } else {
                        conflict_msg.push_str(&format!(
                            "  You are trying to move: {dotfile_name} (directory)\n\
                            But profile \"{profile_name}\" has: {conflicting_path}\n\n"
                        ));
                    }
                }
            }
        }

        let msg = format!(
            "✗ Path conflict detected:\n\n{conflict_msg}\
            This would create an invalid state.\n\n\
            To fix: Remove the conflicting path from sync in the\n\
            affected profile first."
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

    fn render_unsync_common_confirm(
        &self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
    ) -> Result<()> {
        let dotfile_name = if let Some(idx) = self.state.confirm_unsync_common {
            if idx < self.state.dotfiles.len() {
                self.state.dotfiles[idx].relative_path.display().to_string()
            } else {
                "Unknown".to_string()
            }
        } else {
            "Unknown".to_string()
        };

        let msg = format!(
            "Remove '{dotfile_name}' from sync?\n\n\
            This file is in 'common' and is shared across ALL profiles.\n\
            Removing it will affect every profile that uses it."
        );

        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}/y: Confirm | {}/n: Cancel",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Cancel)
        );

        let dialog = Dialog::new("Remove Common File", &msg)
            .variant(DialogVariant::Warning)
            .footer(&footer_text);
        frame.render_widget(dialog, area);

        Ok(())
    }

    // ============================================================================
    // Action Processing Methods
    // ============================================================================

    /// Process a dotfile action and return the result.
    ///
    /// This is the main dispatcher for all dotfile-related actions.
    pub fn process_action(
        &mut self,
        action: DotfileAction,
        config: &mut Config,
        config_path: &Path,
    ) -> Result<ActionResult> {
        debug!("Processing dotfile action: {:?}", action);

        match action {
            DotfileAction::ScanDotfiles => {
                self.scan_dotfiles(config)?;
                Ok(ActionResult::None)
            }
            DotfileAction::RefreshFileBrowser => {
                self.refresh_file_browser(config)?;
                Ok(ActionResult::None)
            }
            DotfileAction::ToggleFileSync {
                file_index,
                is_synced,
            } => self.toggle_file_sync(config, file_index, is_synced),
            DotfileAction::AddCustomFileToSync {
                full_path,
                relative_path,
            } => self.add_custom_file_to_sync(config, config_path, full_path, relative_path),
            DotfileAction::SetBackupEnabled { enabled } => {
                self.state.backup_enabled = enabled;
                Ok(ActionResult::None)
            }
            DotfileAction::MoveToCommon {
                file_index,
                is_common,
                profiles_to_cleanup,
            } => self.move_to_common(config, file_index, is_common, profiles_to_cleanup),
        }
    }

    /// Scan for dotfiles and update the state.
    pub fn scan_dotfiles(&mut self, config: &Config) -> Result<()> {
        info!("Scanning for dotfiles...");

        let dotfiles = SyncService::scan_dotfiles(config)?;
        debug!("Found {} dotfiles", dotfiles.len());

        // Update state
        self.state.dotfiles = dotfiles;
        self.state.selected_for_sync.clear();

        // Mark synced files as selected
        for (i, dotfile) in self.state.dotfiles.iter().enumerate() {
            if dotfile.synced {
                self.state.selected_for_sync.insert(i);
            }
        }

        // Update scrollbar
        self.state.dotfile_list_scrollbar = self
            .state
            .dotfile_list_scrollbar
            .content_length(self.state.dotfiles.len());

        // Select first file if list is not empty
        if !self.state.dotfiles.is_empty() && self.state.dotfile_list_state.selected().is_none() {
            // Find first non-header item
            let display_items = self.get_display_items(&config.active_profile);
            for (i, item) in display_items.iter().enumerate() {
                if matches!(item, DisplayItem::File(_)) {
                    self.state.dotfile_list_state.select(Some(i));
                    break;
                }
            }
        }

        info!("Dotfile scan complete");
        Ok(())
    }

    /// Refresh the file browser entries.
    pub fn refresh_file_browser(&mut self, _config: &Config) -> Result<()> {
        debug!("Refreshing file browser entries");
        // The file browser component handles its own refresh
        // This is a placeholder for any additional refresh logic needed
        Ok(())
    }

    /// Toggle file sync status (add or remove from sync).
    pub fn toggle_file_sync(
        &mut self,
        config: &Config,
        file_index: usize,
        is_synced: bool,
    ) -> Result<ActionResult> {
        if file_index >= self.state.dotfiles.len() {
            warn!("Invalid file index: {}", file_index);
            return Ok(ActionResult::ShowToast {
                message: "Invalid file selection".to_string(),
                variant: crate::widgets::ToastVariant::Error,
            });
        }

        if is_synced {
            self.remove_file_from_sync(config, file_index)
        } else {
            self.add_file_to_sync(config, file_index)
        }
    }

    /// Add a file to sync.
    fn add_file_to_sync(&mut self, config: &Config, file_index: usize) -> Result<ActionResult> {
        let dotfile = &self.state.dotfiles[file_index];
        let relative_path = dotfile.relative_path.to_string_lossy().to_string();
        let full_path = dotfile.original_path.clone();

        info!("Adding file to sync: {}", relative_path);

        match SyncService::add_file_to_sync(
            config,
            &full_path,
            &relative_path,
            self.state.backup_enabled,
        ) {
            Ok(crate::services::AddFileResult::Success) => {
                // Update state
                self.state.selected_for_sync.insert(file_index);
                self.state.dotfiles[file_index].synced = true;

                info!("Successfully added file to sync: {}", relative_path);
                Ok(ActionResult::ShowToast {
                    message: format!("Added {relative_path} to sync"),
                    variant: crate::widgets::ToastVariant::Success,
                })
            }
            Ok(crate::services::AddFileResult::AlreadySynced) => {
                // Already synced - just update state to be consistent
                self.state.selected_for_sync.insert(file_index);
                self.state.dotfiles[file_index].synced = true;

                Ok(ActionResult::ShowToast {
                    message: format!("{relative_path} is already synced"),
                    variant: crate::widgets::ToastVariant::Info,
                })
            }
            Ok(crate::services::AddFileResult::ValidationFailed(msg)) => {
                warn!("Validation failed for {}: {}", relative_path, msg);
                Ok(ActionResult::ShowDialog {
                    title: "Cannot Add File".to_string(),
                    content: msg,
                    variant: crate::widgets::DialogVariant::Error,
                })
            }
            Err(e) => {
                warn!("Error adding file to sync: {}", e);
                Ok(ActionResult::ShowToast {
                    message: format!("Error: {e}"),
                    variant: crate::widgets::ToastVariant::Error,
                })
            }
        }
    }

    /// Remove a file from sync.
    fn remove_file_from_sync(
        &mut self,
        config: &Config,
        file_index: usize,
    ) -> Result<ActionResult> {
        let dotfile = &self.state.dotfiles[file_index];
        let relative_path = dotfile.relative_path.to_string_lossy().to_string();

        // Check if this is a common file
        if dotfile.is_common {
            info!("Removing common file from sync: {}", relative_path);

            match SyncService::remove_common_file_from_sync(config, &relative_path) {
                Ok(crate::services::RemoveFileResult::Success) => {
                    // Update state
                    self.state.selected_for_sync.remove(&file_index);
                    self.state.dotfiles[file_index].synced = false;
                    self.state.dotfiles[file_index].is_common = false;

                    info!(
                        "Successfully removed common file from sync: {}",
                        relative_path
                    );
                    Ok(ActionResult::ShowToast {
                        message: format!("Removed {relative_path} from sync"),
                        variant: crate::widgets::ToastVariant::Success,
                    })
                }
                Ok(crate::services::RemoveFileResult::NotSynced) => {
                    // Not synced - just update state to be consistent
                    self.state.selected_for_sync.remove(&file_index);
                    self.state.dotfiles[file_index].synced = false;

                    Ok(ActionResult::ShowToast {
                        message: format!("{relative_path} is not synced"),
                        variant: crate::widgets::ToastVariant::Info,
                    })
                }
                Err(e) => {
                    warn!("Error removing common file from sync: {}", e);
                    Ok(ActionResult::ShowToast {
                        message: format!("Error: {e}"),
                        variant: crate::widgets::ToastVariant::Error,
                    })
                }
            }
        } else {
            info!("Removing file from sync: {}", relative_path);

            match SyncService::remove_file_from_sync(config, &relative_path) {
                Ok(crate::services::RemoveFileResult::Success) => {
                    // Update state
                    self.state.selected_for_sync.remove(&file_index);
                    self.state.dotfiles[file_index].synced = false;

                    info!("Successfully removed file from sync: {}", relative_path);
                    Ok(ActionResult::ShowToast {
                        message: format!("Removed {relative_path} from sync"),
                        variant: crate::widgets::ToastVariant::Success,
                    })
                }
                Ok(crate::services::RemoveFileResult::NotSynced) => {
                    // Not synced - just update state to be consistent
                    self.state.selected_for_sync.remove(&file_index);
                    self.state.dotfiles[file_index].synced = false;

                    Ok(ActionResult::ShowToast {
                        message: format!("{relative_path} is not synced"),
                        variant: crate::widgets::ToastVariant::Info,
                    })
                }
                Err(e) => {
                    warn!("Error removing file from sync: {}", e);
                    Ok(ActionResult::ShowToast {
                        message: format!("Error: {e}"),
                        variant: crate::widgets::ToastVariant::Error,
                    })
                }
            }
        }
    }

    /// Add a custom file to sync.
    pub fn add_custom_file_to_sync(
        &mut self,
        config: &mut Config,
        config_path: &Path,
        full_path: PathBuf,
        relative_path: String,
    ) -> Result<ActionResult> {
        info!("Adding custom file to sync: {}", relative_path);

        // Validate the file exists
        if !full_path.exists() {
            return Ok(ActionResult::ShowDialog {
                title: "File Not Found".to_string(),
                content: format!("The file {} does not exist", full_path.display()),
                variant: crate::widgets::DialogVariant::Error,
            });
        }

        // Validate the file is safe to add
        let (is_safe, reason) = crate::utils::is_safe_to_add(&full_path, &config.repo_path);
        if !is_safe {
            return Ok(ActionResult::ShowDialog {
                title: "Cannot Add File".to_string(),
                content: reason.unwrap_or_else(|| "Cannot add this file".to_string()),
                variant: crate::widgets::DialogVariant::Error,
            });
        }

        // Add to sync using SyncService
        match SyncService::add_file_to_sync(
            config,
            &full_path,
            &relative_path,
            self.state.backup_enabled,
        ) {
            Ok(crate::services::AddFileResult::Success) => {
                // Add to custom files in config if not already present
                if !config.custom_files.contains(&relative_path) {
                    config.custom_files.push(relative_path.clone());
                    // Save config
                    if let Err(e) = config.save(config_path) {
                        warn!("Failed to save config: {}", e);
                    }
                }

                // Refresh dotfile list
                self.scan_dotfiles(config)?;

                info!("Successfully added custom file to sync: {}", relative_path);
                Ok(ActionResult::ShowToast {
                    message: format!("Added {relative_path} to sync"),
                    variant: crate::widgets::ToastVariant::Success,
                })
            }
            Ok(crate::services::AddFileResult::AlreadySynced) => Ok(ActionResult::ShowToast {
                message: format!("{relative_path} is already synced"),
                variant: crate::widgets::ToastVariant::Info,
            }),
            Ok(crate::services::AddFileResult::ValidationFailed(msg)) => {
                warn!(
                    "Validation failed for custom file {}: {}",
                    relative_path, msg
                );
                Ok(ActionResult::ShowDialog {
                    title: "Cannot Add File".to_string(),
                    content: msg,
                    variant: crate::widgets::DialogVariant::Error,
                })
            }
            Err(e) => {
                warn!("Error adding custom file to sync: {}", e);
                Ok(ActionResult::ShowToast {
                    message: format!("Error: {e}"),
                    variant: crate::widgets::ToastVariant::Error,
                })
            }
        }
    }

    /// Move a file to/from common.
    pub fn move_to_common(
        &mut self,
        config: &Config,
        file_index: usize,
        is_common: bool,
        profiles_to_cleanup: Vec<String>,
    ) -> Result<ActionResult> {
        if file_index >= self.state.dotfiles.len() {
            warn!("Invalid file index: {}", file_index);
            return Ok(ActionResult::ShowToast {
                message: "Invalid file selection".to_string(),
                variant: crate::widgets::ToastVariant::Error,
            });
        }

        let dotfile = &self.state.dotfiles[file_index];
        let relative_path = dotfile.relative_path.to_string_lossy().to_string();

        if is_common {
            // Move from common to profile
            info!("Moving {} from common to profile", relative_path);

            match SyncService::move_from_common(config, &relative_path) {
                Ok(()) => {
                    // Update state
                    self.state.dotfiles[file_index].is_common = false;

                    info!("Successfully moved {} to profile", relative_path);
                    Ok(ActionResult::ShowToast {
                        message: format!("Moved {relative_path} to profile"),
                        variant: crate::widgets::ToastVariant::Success,
                    })
                }
                Err(e) => {
                    warn!("Error moving file from common: {}", e);
                    Ok(ActionResult::ShowToast {
                        message: format!("Error: {e}"),
                        variant: crate::widgets::ToastVariant::Error,
                    })
                }
            }
        } else {
            // Move from profile to common
            info!(
                "Moving {} from profile to common (cleanup: {} profiles)",
                relative_path,
                profiles_to_cleanup.len()
            );

            let result = if profiles_to_cleanup.is_empty() {
                SyncService::move_to_common(config, &relative_path)
            } else {
                SyncService::move_to_common_with_cleanup(
                    config,
                    &relative_path,
                    &profiles_to_cleanup,
                )
            };

            match result {
                Ok(()) => {
                    // Update state
                    self.state.dotfiles[file_index].is_common = true;

                    info!("Successfully moved {} to common", relative_path);
                    Ok(ActionResult::ShowToast {
                        message: format!("Moved {relative_path} to common"),
                        variant: crate::widgets::ToastVariant::Success,
                    })
                }
                Err(e) => {
                    warn!("Error moving file to common: {}", e);
                    Ok(ActionResult::ShowToast {
                        message: format!("Error: {e}"),
                        variant: crate::widgets::ToastVariant::Error,
                    })
                }
            }
        }
    }
}

// Helper function to format file sizes
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
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
        let t = ui_theme();
        let background = Block::default().style(t.background_style());
        frame.render_widget(background, area);

        // Layout: Title/Description, Content (list + preview), Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 3);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Manage Files",
            "Add or remove files to your repository. You can also add custom files. We have automatically detected some common dotfiles for you."
        )?;

        // Render main content (either custom file input or dotfile list)
        if self.state.adding_custom_file && !self.file_browser.is_open() {
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

        // Render file browser as overlay on top (Popup handles dimming)
        if self.file_browser.is_open() {
            self.file_browser
                .render(frame, area, ctx.config, ctx.syntax_set, ctx.syntax_theme)?;
        }

        // Render modals on top of the content (not instead of it)
        if self.state.show_custom_file_confirm {
            self.render_custom_file_confirm(frame, area, ctx.config)?;
        } else if self.state.confirm_move.is_some() {
            // Move confirmation modals render on top of the main content
            self.render_move_confirm(frame, area, ctx.config)?;
        } else if self.state.confirm_unsync_common.is_some() {
            // Unsync common file confirmation
            self.render_unsync_common_confirm(frame, area, ctx.config)?;
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

        if self.state.confirm_unsync_common.is_some() {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    return self.handle_unsync_common_confirm(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 2. File browser mode - delegate to component
        if self.file_browser.is_open() {
            let result = self.file_browser.handle_event(event, ctx.config)?;
            match result {
                FileBrowserResult::None | FileBrowserResult::RefreshNeeded => {
                    return Ok(ScreenAction::None);
                }
                FileBrowserResult::Cancelled => {
                    self.state.adding_custom_file = false;
                    self.state.focus = DotfileSelectionFocus::FilesList;
                    return Ok(ScreenAction::None);
                }
                FileBrowserResult::Selected {
                    full_path,
                    relative_path,
                } => {
                    self.state.adding_custom_file = false;
                    self.state.focus = DotfileSelectionFocus::FilesList;
                    return Ok(ScreenAction::AddCustomFileToSync {
                        full_path,
                        relative_path,
                    });
                }
            }
        }

        // 3. Custom file input mode (legacy)
        if self.state.adding_custom_file && !self.file_browser.is_open() {
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Press {
                    // For plain character keys, ALWAYS insert the character first
                    // This ensures vim bindings like h/l don't interfere with typing
                    if let KeyCode::Char(c) = key.code {
                        if !key.modifiers.intersects(
                            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                        ) {
                            self.state.custom_file_input.insert_char(c);
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                    return self.handle_custom_file_input(key.code, ctx.config);
                }
            }
            return Ok(ScreenAction::None);
        }

        // 4. Normal navigation based on focus
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
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

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        if self.file_browser.is_open() {
            self.file_browser.is_input_focused()
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
