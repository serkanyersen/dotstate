//! Dotfile selection screen controller.
//!
//! This screen handles selecting and managing dotfiles for syncing.
//! It owns all state and rendering logic (self-contained screen).

use crate::file_manager::Dotfile;
use crate::components::file_preview::FilePreview;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::input_field::InputField;
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{theme as ui_theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::Screen as ScreenId;
use crate::utils::{
    center_popup, create_split_layout, create_standard_layout, focused_border_style,
    unfocused_border_style, list_navigation::ListStateExt, TextInput,
};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};

use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, StatefulWidget, Wrap,
};
use ratatui::Frame;
use std::path::{Path, PathBuf};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

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

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    self.state.dotfile_list_state.select_previous();
                    self.state.preview_scroll = 0;
                }
                Action::MoveDown => {
                    self.state.dotfile_list_state.select_next();
                    self.state.preview_scroll = 0;
                }
                Action::Confirm => {
                    if self.state.status_message.is_some() {
                        self.state.status_message = None;
                    } else if let Some(idx) = self.state.dotfile_list_state.selected() {
                        let is_synced = self.state.selected_for_sync.contains(&idx);
                        return Ok(ScreenAction::ToggleFileSync {
                            file_index: idx,
                            is_synced,
                        });
                    }
                }
                Action::NextTab => {
                    self.state.focus = DotfileSelectionFocus::Preview;
                }
                Action::PageUp => {
                    self.state
                        .dotfile_list_state
                        .page_up(10, self.state.dotfiles.len());
                    self.state.preview_scroll = 0;
                }
                Action::PageDown => {
                    self.state
                        .dotfile_list_state
                        .page_down(10, self.state.dotfiles.len());
                    self.state.preview_scroll = 0;
                }
                Action::GoToTop => {
                    self.state.dotfile_list_state.select_first();
                    self.state.preview_scroll = 0;
                }
                Action::GoToEnd => {
                    self.state.dotfile_list_state.select_last();
                    self.state.preview_scroll = 0;
                }
                Action::Create => {
                    // Open file browser
                    self.state.adding_custom_file = true;
                    self.state.file_browser_mode = true;
                    self.state.file_browser_path = crate::utils::get_home_dir();
                    self.state.file_browser_selected = 0;
                    self.state.file_browser_path_input.set_text(
                        self.state.file_browser_path.to_string_lossy().to_string(),
                    );
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
        let path_display = Paragraph::new(
            self.state
                .file_browser_path
                .to_string_lossy()
                .to_string(),
        )
        .block(
            Block::default()
                // .borders(Borders::ALL)
                .title("Current Directory")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(Color::Reset)),
        );
        frame.render_widget(path_display, browser_chunks[0]);

        // Path input field - use InputField component
        let path_input_text = if self.state.file_browser_path_input.is_empty() {
            self.state
                .file_browser_path
                .to_string_lossy()
                .to_string()
        } else {
            self.state.file_browser_path_input.text().to_string()
        };

        let cursor_pos = if self.state.file_browser_path_input.is_empty() {
            path_input_text.chars().count()
        } else {
            self.state
                .file_browser_path_input
                .cursor()
                .min(path_input_text.chars().count())
        };

        InputField::render(
            frame,
            browser_chunks[1],
            &path_input_text,
            cursor_pos,
            self.state.file_browser_path_focused,
            "Path Input",
            None,
            Alignment::Left,
            false, // Not disabled
        )?;

        // Split list and preview
        let list_preview_chunks = create_split_layout(browser_chunks[2], &[50, 50]);

        // File list using ListState
        let items: Vec<ListItem> = self.state
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

                let prefix = if is_dir { "📁 " } else { "📄 " };
                let display = format!("{}{}", prefix, name);

                ListItem::new(display)
            })
            .collect();

        // Update scrollbar state
        let total_items = self.state.file_browser_entries.len();
        let selected_index = self.state
            .file_browser_list_state
            .selected()
            .unwrap_or(0);
        self.state.file_browser_scrollbar = self.state
            .file_browser_scrollbar
            .content_length(total_items)
            .position(selected_index);

        // Add focus indicator to file browser list
        let t = ui_theme();
        let list_title = "Select File or Directory (Enter to load path)";
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
                        .title("Preview")
                        .title_alignment(Alignment::Center),
                );
                frame.render_widget(empty_preview, list_preview_chunks[1]);
            }
        } else {
            let empty_preview = Paragraph::new("No selection").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Preview")
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

        let input_text = self.state.custom_file_input.text();
        let cursor_pos = self
            .state
            .custom_file_input
            .cursor()
            .min(input_text.chars().count());

        InputField::render(
            frame,
            input_chunks[1],
            input_text,
            cursor_pos,
            self.state.custom_file_focused,
            "Custom File Path",
            Some("Enter file path (e.g., ~/.myconfig or /path/to/file)"),
            Alignment::Center,
            false, // Not disabled
        )?;

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
        let items: Vec<ListItem> = self.state
            .dotfiles
            .iter()
            .enumerate()
            .map(|(i, dotfile)| {
                let is_selected = self.state.selected_for_sync.contains(&i);
                let prefix = if is_selected { "✓ " } else { "  " };
                let style = if is_selected {
                    Style::default().fg(t.success)
                } else {
                    t.text_style()
                };
                let path_str = dotfile.relative_path.to_string_lossy();
                ListItem::new(format!("{}{}", prefix, path_str)).style(style)
            })
            .collect();

        // Update scrollbar state
        let total_dotfiles = self.state.dotfiles.len();
        let selected_index = self.state.dotfile_list_state.selected().unwrap_or(0);
        self.state.dotfile_list_scrollbar = self.state
            .dotfile_list_scrollbar
            .content_length(total_dotfiles)
            .position(selected_index);

        // Add focus indicator to files list
        let list_title = format!("Found {} dotfiles", self.state.dotfiles.len());
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

        // Description block
        if let Some(selected_index) = self.state.dotfile_list_state.selected() {
            if selected_index < self.state.dotfiles.len() {
                let dotfile = &self.state.dotfiles[selected_index];
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
                            .title("Description")
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
                        .title("Description")
                        .title_alignment(Alignment::Center)
                        .border_style(unfocused_border_style()),
                );
                frame.render_widget(empty_desc, description_area);
            }
        } else {
            let empty_desc = Paragraph::new("No file selected").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Description")
                    .title_alignment(Alignment::Center)
                    .border_style(unfocused_border_style()),
            );
            frame.render_widget(empty_desc, description_area);
        }

        // Preview panel
        if let Some(preview_rect) = preview_area_opt {
            if let Some(selected_index) = self.state.dotfile_list_state.selected() {
                if selected_index < self.state.dotfiles.len() {
                    let dotfile = &self.state.dotfiles[selected_index];
                    let is_focused = self.state.focus == DotfileSelectionFocus::Preview;
                    let preview_title =
                        format!("Preview: {}", dotfile.relative_path.to_string_lossy());

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
                            .title("Preview")
                            .title_alignment(Alignment::Center),
                    );
                    frame.render_widget(empty_preview, preview_rect);
                }
            } else {
                let empty_preview = Paragraph::new("No file selected").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Preview")
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
                .title("Sync Summary")
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
            format!(
                "Tab: Focus | {}: Navigate | Space/{}: Toggle | {}: Add Custom | {}: Backup ({}) | {}: Back",
                 config.keymap.navigation_display(),
                 k(crate::keymap::Action::Confirm),
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
        let t = ui_theme();
        // Dim the background
        let dim = Block::default().style(Style::default().bg(Color::Reset).fg(t.text_muted));
        frame.render_widget(dim, area);

        // Create centered popup
        let popup_area = crate::utils::center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let path = self.state
            .custom_file_confirm_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Title
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Path label
                Constraint::Length(3), // Path value (highlighted)
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Warning message
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Instructions
            ])
            .split(popup_area);

        // Title
        let title = Paragraph::new("Confirm Add Custom File")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Confirmation")
                    .title_alignment(Alignment::Center)
                    .style(Style::default().bg(Color::Reset)),
            )
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(t.text_emphasis)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(title, chunks[0]);

        // Path label
        let path_label = Paragraph::new("Path:").style(t.text_style());
        frame.render_widget(path_label, chunks[2]);

        // Path value (highlighted in different color)
        let path_value = Paragraph::new(path.as_str())
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(t.primary).add_modifier(Modifier::BOLD));
        frame.render_widget(path_value, chunks[3]);

        // Warning message
        let warning = Paragraph::new("⚠️  This will move this path to the storage repo and replace it with a symlink.\nMake sure you know what you are doing.")
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(t.warning));
        frame.render_widget(warning, chunks[5]);

        // Instructions
        let k = |a| config.keymap.get_key_display_for_action(a);
        let instruction_text = format!(
            "Press Y/{} to confirm, N/{} to cancel",
            k(crate::keymap::Action::Confirm),
            k(crate::keymap::Action::Quit)
        );
        let instructions = Paragraph::new(instruction_text)
            .alignment(Alignment::Center)
            .style(Style::default().fg(t.text_muted));
        frame.render_widget(instructions, chunks[7]);

        Ok(())
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

        // Check if confirmation modal is showing
        if self.state.show_custom_file_confirm {
            self.render_custom_file_confirm(frame, area, ctx.config)?;
        }
        // Check if file browser is active - render as popup
        else if self.state.file_browser_mode {
            self.render_file_browser(
                frame,
                area,
                footer_chunk,
                ctx.config,
                ctx.syntax_set,
                ctx.syntax_theme,
            )?;
        } else if self.state.adding_custom_file {
            self.render_custom_file_input(
                frame,
                content_chunk,
                footer_chunk,
                ctx.config,
            )?;
        } else {
            self.render_dotfile_list(
                frame,
                content_chunk,
                footer_chunk,
                ctx.config,
                ctx.syntax_set,
                ctx.syntax_theme,
            )?;
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
