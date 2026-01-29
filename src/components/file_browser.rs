//! File Browser component for selecting files and directories.
//!
//! This component provides a modal file browser with:
//! - Path input field for direct navigation
//! - File/directory list with navigation
//! - Preview pane for selected files
//! - Keyboard navigation between panes

use crate::components::file_preview::FilePreview;
use crate::components::popup::Popup;
use crate::config::Config;
use crate::keymap::Action;
use crate::styles::{theme as ui_theme, LIST_HIGHLIGHT_SYMBOL};
use crate::utils::list_navigation::ListStateExt;
use crate::utils::style::{focused_border_style, unfocused_border_style};
use crate::utils::text_input::TextInput;
use crate::widgets::text_input::{TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, StatefulWidget,
};
use ratatui::Frame;
use std::path::{Path, PathBuf};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

/// Focus area within the file browser
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileBrowserFocus {
    /// Path input field is focused
    PathInput,
    /// File/directory list is focused
    #[default]
    List,
    /// Preview pane is focused
    Preview,
}

/// Result of file browser interaction
#[derive(Debug, Clone)]
pub enum FileBrowserResult {
    /// No action taken
    None,
    /// User cancelled/closed the browser
    Cancelled,
    /// User selected a file or folder to add
    Selected {
        full_path: PathBuf,
        relative_path: String,
    },
    /// Directory changed, entries need refresh
    RefreshNeeded,
}

/// File browser component state
#[derive(Debug)]
pub struct FileBrowser {
    /// Whether the browser is currently open/active
    pub is_active: bool,
    /// Current directory path
    pub current_path: PathBuf,
    /// Entries in the current directory (files and folders)
    pub entries: Vec<PathBuf>,
    /// List selection state
    pub list_state: ListState,
    /// Scrollbar state for the list
    pub scrollbar_state: ScrollbarState,
    /// Path input field
    pub path_input: TextInput,
    /// Preview pane scroll offset
    pub preview_scroll: usize,
    /// Which pane currently has focus
    pub focus: FileBrowserFocus,
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowser {
    /// Create a new file browser
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_active: false,
            current_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            entries: Vec::new(),
            list_state: ListState::default(),
            scrollbar_state: ScrollbarState::new(0),
            path_input: TextInput::new(),
            preview_scroll: 0,
            focus: FileBrowserFocus::List,
        }
    }

    /// Open the file browser at the specified path
    pub fn open(&mut self, path: PathBuf) {
        self.is_active = true;
        self.current_path = path.clone();
        self.path_input.set_text(path.to_string_lossy().to_string());
        self.list_state.select(Some(0));
        self.preview_scroll = 0;
        self.focus = FileBrowserFocus::List;
        self.refresh_entries();
    }

    /// Close the file browser
    pub fn close(&mut self) {
        self.is_active = false;
        self.path_input.clear();
        self.entries.clear();
    }

    /// Check if the browser is currently open
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.is_active
    }

    /// Refresh directory entries for the current path
    pub fn refresh_entries(&mut self) {
        self.entries.clear();

        // Add parent directory entry
        if self.current_path.parent().is_some() {
            self.entries.push(PathBuf::from(".."));
        }

        // Add current directory entry (for selecting the folder itself)
        self.entries.push(PathBuf::from("."));

        // List directory contents
        if let Ok(read_dir) = std::fs::read_dir(&self.current_path) {
            let mut dirs: Vec<PathBuf> = Vec::new();
            let mut files: Vec<PathBuf> = Vec::new();

            for entry in read_dir.flatten() {
                let path = entry.path();

                // Include all files (including dotfiles, since this is a dotfile manager)
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    files.push(path);
                }
            }

            // Sort directories and files
            dirs.sort();
            files.sort();

            // Add directories first, then files
            for dir in dirs {
                self.entries.push(dir);
            }
            for file in files {
                self.entries.push(file);
            }
        }

        // Update scrollbar
        self.scrollbar_state = self.scrollbar_state.content_length(self.entries.len());

        // Ensure selection is valid
        if self.list_state.selected().is_none() && !self.entries.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    /// Check if input field is focused (for blocking global keybindings)
    #[must_use]
    pub fn is_input_focused(&self) -> bool {
        self.is_active && self.focus == FileBrowserFocus::PathInput
    }

    /// Handle events for the file browser
    pub fn handle_event(&mut self, event: Event, config: &Config) -> Result<FileBrowserResult> {
        if !self.is_active {
            return Ok(FileBrowserResult::None);
        }

        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(FileBrowserResult::None);
            }

            match self.focus {
                FileBrowserFocus::PathInput => {
                    return self.handle_path_input(key.code, config);
                }
                FileBrowserFocus::List => {
                    return self.handle_list_navigation(key.code, config);
                }
                FileBrowserFocus::Preview => {
                    return self.handle_preview_navigation(key.code, config);
                }
            }
        }

        Ok(FileBrowserResult::None)
    }

    /// Handle path input field events
    fn handle_path_input(
        &mut self,
        key_code: KeyCode,
        _config: &Config,
    ) -> Result<FileBrowserResult> {
        match key_code {
            KeyCode::Char(c) => {
                self.path_input.insert_char(c);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.path_input.handle_key(key_code);
            }
            KeyCode::Backspace => {
                self.path_input.backspace();
            }
            KeyCode::Delete => {
                self.path_input.delete();
            }
            KeyCode::Enter => {
                // Load path from input
                let path_str = self.path_input.text_trimmed();
                if !path_str.is_empty() {
                    let full_path = crate::utils::expand_path(path_str);

                    if full_path.exists() {
                        if full_path.is_dir() {
                            self.current_path = full_path.clone();
                            self.path_input
                                .set_text(self.current_path.to_string_lossy().to_string());
                            self.list_state.select(Some(0));
                            self.focus = FileBrowserFocus::List;
                            self.refresh_entries();
                            return Ok(FileBrowserResult::RefreshNeeded);
                        } else {
                            // It's a file - select it directly
                            let home_dir = crate::utils::get_home_dir();
                            let relative_path = full_path.strip_prefix(&home_dir).map_or_else(
                                |_| full_path.to_string_lossy().to_string(),
                                |p| p.to_string_lossy().to_string(),
                            );

                            self.close();
                            return Ok(FileBrowserResult::Selected {
                                full_path,
                                relative_path,
                            });
                        }
                    }
                }
            }
            KeyCode::Tab => {
                self.focus = FileBrowserFocus::List;
            }
            KeyCode::Esc => {
                self.close();
                return Ok(FileBrowserResult::Cancelled);
            }
            _ => {}
        }

        Ok(FileBrowserResult::None)
    }

    /// Handle list navigation events
    fn handle_list_navigation(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<FileBrowserResult> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    self.list_state.select_previous();
                    self.preview_scroll = 0; // Reset preview scroll on selection change
                }
                Action::MoveDown => {
                    self.list_state.select_next();
                    self.preview_scroll = 0;
                }
                Action::Confirm => {
                    return self.handle_selection(config);
                }
                Action::NextTab => {
                    self.focus = FileBrowserFocus::Preview;
                }
                Action::PageUp => {
                    self.list_state.page_up(10, self.entries.len());
                }
                Action::PageDown => {
                    self.list_state.page_down(10, self.entries.len());
                }
                Action::GoToTop => {
                    self.list_state.select_first();
                }
                Action::GoToEnd => {
                    self.list_state.select_last();
                }
                Action::Cancel | Action::Quit => {
                    self.close();
                    return Ok(FileBrowserResult::Cancelled);
                }
                _ => {}
            }
        }

        // Update scrollbar position
        if let Some(selected) = self.list_state.selected() {
            self.scrollbar_state = self.scrollbar_state.position(selected);
        }

        Ok(FileBrowserResult::None)
    }

    /// Handle selection/confirmation in the list
    fn handle_selection(&mut self, config: &Config) -> Result<FileBrowserResult> {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.entries.len() {
                let entry = self.entries[idx].clone();

                // Handle special entries
                if entry == Path::new("..") {
                    // Go to parent directory
                    if let Some(parent) = self.current_path.parent() {
                        self.current_path = parent.to_path_buf();
                        self.path_input
                            .set_text(self.current_path.to_string_lossy().to_string());
                        self.list_state.select(Some(0));
                        self.refresh_entries();
                        return Ok(FileBrowserResult::RefreshNeeded);
                    }
                } else if entry == Path::new(".") {
                    // Add current folder
                    let current_folder = self.current_path.clone();
                    let home_dir = crate::utils::get_home_dir();
                    let relative_path = current_folder.strip_prefix(&home_dir).map_or_else(
                        |_| current_folder.to_string_lossy().to_string(),
                        |p| p.to_string_lossy().to_string(),
                    );

                    // Validate
                    let repo_path = &config.repo_path;
                    let (is_safe, _reason) =
                        crate::utils::is_safe_to_add(&current_folder, repo_path);
                    if !is_safe {
                        // Return error via status message (caller handles this)
                        return Ok(FileBrowserResult::None); // TODO: Add error result variant
                    }

                    // Check if git repo
                    if crate::utils::is_git_repo(&current_folder) {
                        return Ok(FileBrowserResult::None); // TODO: Add error result variant
                    }

                    self.close();
                    return Ok(FileBrowserResult::Selected {
                        full_path: current_folder,
                        relative_path,
                    });
                } else {
                    // Regular file or directory
                    let full_path = if entry.is_absolute() {
                        entry.clone()
                    } else {
                        self.current_path.join(&entry)
                    };

                    if full_path.is_dir() {
                        // Navigate into directory
                        self.current_path = full_path.clone();
                        self.path_input
                            .set_text(full_path.to_string_lossy().to_string());
                        self.list_state.select(Some(0));
                        self.refresh_entries();
                        return Ok(FileBrowserResult::RefreshNeeded);
                    } else if full_path.is_file() {
                        // Select file
                        let home_dir = crate::utils::get_home_dir();
                        let relative_path = full_path.strip_prefix(&home_dir).map_or_else(
                            |_| full_path.to_string_lossy().to_string(),
                            |p| p.to_string_lossy().to_string(),
                        );

                        self.close();
                        return Ok(FileBrowserResult::Selected {
                            full_path,
                            relative_path,
                        });
                    }
                }
            }
        }

        Ok(FileBrowserResult::None)
    }

    /// Handle preview pane navigation
    fn handle_preview_navigation(
        &mut self,
        key_code: KeyCode,
        config: &Config,
    ) -> Result<FileBrowserResult> {
        let action = config
            .keymap
            .get_action(key_code, crossterm::event::KeyModifiers::NONE);

        if let Some(action) = action {
            match action {
                Action::MoveUp | Action::ScrollUp => {
                    self.preview_scroll = self.preview_scroll.saturating_sub(1);
                }
                Action::MoveDown | Action::ScrollDown => {
                    self.preview_scroll = self.preview_scroll.saturating_add(1);
                }
                Action::PageUp => {
                    self.preview_scroll = self.preview_scroll.saturating_sub(20);
                }
                Action::PageDown => {
                    self.preview_scroll = self.preview_scroll.saturating_add(20);
                }
                Action::GoToTop => {
                    self.preview_scroll = 0;
                }
                Action::GoToEnd => {
                    self.preview_scroll = 10000; // Will be clamped during render
                }
                Action::NextTab => {
                    self.focus = FileBrowserFocus::PathInput;
                }
                Action::Cancel | Action::Quit => {
                    self.close();
                    return Ok(FileBrowserResult::Cancelled);
                }
                _ => {}
            }
        }

        Ok(FileBrowserResult::None)
    }

    /// Render the file browser
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        if !self.is_active {
            return Ok(());
        }

        let t = ui_theme();

        // Use Popup component for consistent styling
        let k = |a| config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Switch Focus | {}: Navigate | {}: Select | {}: Cancel",
            k(Action::NextTab),
            config.keymap.navigation_display(),
            k(Action::Confirm),
            k(Action::Cancel)
        );

        let popup_result = Popup::new()
            .width(80)
            .height(70)
            .title("Select File or Directory")
            .footer(&footer_text)
            .render(frame, area);

        let content_area = popup_result.content_area;

        // Split content area into path, list+preview
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Current path display
                Constraint::Length(3), // Path input
                Constraint::Min(0),    // List and preview
            ])
            .split(content_area);

        // Current path display
        let path_display = Paragraph::new(self.current_path.to_string_lossy().to_string()).block(
            Block::default()
                .title(" Current Directory ")
                .title_alignment(Alignment::Center)
                .title_style(t.title_style())
                .style(t.background_style()),
        );
        frame.render_widget(path_display, chunks[0]);

        // Path input field
        let widget = TextInputWidget::new(&self.path_input)
            .title("Path Input")
            .focused(self.focus == FileBrowserFocus::PathInput);
        frame.render_text_input_widget(widget, chunks[1]);

        // Split list and preview horizontally
        let list_preview_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[2]);

        // Render file list
        self.render_list(frame, list_preview_chunks[0], config);

        // Render preview
        self.render_preview(frame, list_preview_chunks[1], syntax_set, theme, config)?;

        Ok(())
    }

    /// Render the file/directory list
    fn render_list(&mut self, frame: &mut Frame, area: Rect, config: &Config) {
        let t = ui_theme();
        let icons = crate::icons::Icons::from_config(config);

        let items: Vec<ListItem> = self
            .entries
            .iter()
            .map(|path| {
                let is_dir = if path == Path::new("..") || path == Path::new(".") {
                    true
                } else {
                    path.is_dir()
                };

                let name = if path == Path::new("..") {
                    ".. (parent)".to_string()
                } else if path == Path::new(".") {
                    ". (add this folder)".to_string()
                } else {
                    path.file_name().and_then(|n| n.to_str()).map_or_else(
                        || path.to_string_lossy().to_string(),
                        std::string::ToString::to_string,
                    )
                };

                let prefix = if is_dir {
                    format!("{} ", icons.folder())
                } else {
                    format!("{} ", icons.file())
                };

                ListItem::new(format!("{prefix}{name}")).style(t.text_style())
            })
            .collect();

        let is_focused = self.focus == FileBrowserFocus::List;
        let border_style = if is_focused {
            focused_border_style().bg(t.background)
        } else {
            unfocused_border_style().bg(t.background)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Files ")
                    .border_type(t.border_type(is_focused))
                    .title_alignment(Alignment::Center)
                    .border_style(border_style)
                    .style(t.background_style()),
            )
            .highlight_style(t.highlight_style())
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        StatefulWidget::render(list, area, frame.buffer_mut(), &mut self.list_state);

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            area,
            &mut self.scrollbar_state,
        );
    }

    /// Render the preview pane
    fn render_preview(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        syntax_set: &SyntaxSet,
        theme: &Theme,
        config: &crate::config::Config,
    ) -> Result<()> {
        let t = ui_theme();

        if let Some(selected_index) = self.list_state.selected() {
            if selected_index < self.entries.len() {
                let selected = &self.entries[selected_index];
                let full_path = if selected == Path::new("..") {
                    self.current_path
                        .parent()
                        .map_or_else(|| PathBuf::from("/"), std::path::Path::to_path_buf)
                } else if selected == Path::new(".") {
                    self.current_path.clone()
                } else if selected.is_absolute() {
                    selected.clone()
                } else {
                    self.current_path.join(selected)
                };

                let is_focused = self.focus == FileBrowserFocus::Preview;

                FilePreview::render(
                    frame,
                    area,
                    &full_path,
                    self.preview_scroll,
                    is_focused,
                    Some("Preview"),
                    None,
                    syntax_set,
                    theme,
                    config,
                )?;

                return Ok(());
            }
        }

        // No selection - show empty preview
        let empty_preview = Paragraph::new("No selection")
            .style(t.muted_style())
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(t.border_type(false))
                    .title(" Preview ")
                    .title_alignment(Alignment::Center)
                    .style(t.background_style()),
            );
        frame.render_widget(empty_preview, area);

        Ok(())
    }
}
