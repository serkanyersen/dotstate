use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, ListState, StatefulWidget, Wrap};
use crate::file_manager::Dotfile;
use crate::components::input_field::InputField;
use crate::components::footer::Footer;
use crate::utils::{create_standard_layout, create_split_layout, center_popup};
use std::path::{Path, PathBuf};

/// Application screens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    MainMenu,
    DotfileSelection,
    GitHubAuth,
    ViewSyncedFiles,
    PushChanges,
    PullChanges,
}

/// GitHub auth state
#[derive(Debug, Clone)]
pub struct GitHubAuthState {
    pub token_input: String,
    pub step: GitHubAuthStep,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub show_help: bool,
    pub help_scroll: usize,
    pub cursor_position: usize, // For token input
    pub input_focused: bool, // Whether input is currently focused
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubAuthStep {
    TokenInput,
    Processing,
}

impl Default for GitHubAuthState {
    fn default() -> Self {
        Self {
            token_input: String::new(),
            step: GitHubAuthStep::TokenInput,
            error_message: None,
            status_message: None,
            show_help: true,
            help_scroll: 0,
            cursor_position: 0,
            input_focused: true, // Input starts focused
        }
    }
}

/// Focus area in dotfile selection screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotfileSelectionFocus {
    FilesList,           // Files list pane is focused
    Preview,             // Preview pane is focused
    FileBrowserList,     // File browser list pane is focused
    FileBrowserPreview,  // File browser preview pane is focused
    FileBrowserInput,    // File browser path input is focused
    CustomInput,         // Custom file input is focused
}

/// Dotfile selection state
#[derive(Debug)]
pub struct DotfileSelectionState {
    pub dotfiles: Vec<Dotfile>,
    pub selected_index: usize, // Deprecated, using dotfile_list_state now
    pub preview_index: Option<usize>,
    pub scroll_offset: usize, // Deprecated, using dotfile_list_state now
    pub preview_scroll: usize,
    pub selected_for_sync: std::collections::HashSet<usize>, // Indices of selected files
    pub dotfile_list_scrollbar: ScrollbarState, // Scrollbar state for dotfile list
    pub dotfile_list_state: ListState, // ListState for main dotfile list (handles selection and scrolling)
    pub status_message: Option<String>, // For sync summary
    pub adding_custom_file: bool, // Whether we're in "add custom file" mode
    pub custom_file_input: String, // Input for custom file path
    pub custom_file_cursor: usize, // Cursor position for custom file input
    pub custom_file_focused: bool, // Whether custom file input is focused
    pub file_browser_mode: bool, // Whether we're in file browser mode
    pub file_browser_path: PathBuf, // Current directory in file browser
    pub file_browser_selected: usize, // Selected file index in browser
    pub file_browser_entries: Vec<PathBuf>, // Files/dirs in current directory
    #[allow(dead_code)]
    pub file_browser_scroll: usize, // Scroll offset for file browser list (deprecated, using ListState now)
    pub file_browser_scrollbar: ScrollbarState, // Scrollbar state for file browser
    pub file_browser_list_state: ListState, // ListState for file browser (handles selection and scrolling)
    pub file_browser_preview_scroll: usize, // Scroll offset for file browser preview
    pub file_browser_path_input: String, // Path input for file browser
    pub file_browser_path_cursor: usize, // Cursor position for path input
    pub file_browser_path_focused: bool, // Whether path input is focused
    pub focus: DotfileSelectionFocus, // Which pane currently has focus
}

impl Default for DotfileSelectionState {
    fn default() -> Self {
        Self {
            dotfiles: Vec::new(),
            selected_index: 0,
            preview_index: None,
            scroll_offset: 0,
            preview_scroll: 0,
            selected_for_sync: std::collections::HashSet::new(),
            dotfile_list_scrollbar: ScrollbarState::new(0),
            dotfile_list_state: ListState::default(),
            status_message: None,
            adding_custom_file: false,
            custom_file_input: String::new(),
            custom_file_cursor: 0,
            custom_file_focused: true,
            file_browser_mode: false,
            file_browser_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            file_browser_selected: 0,
            file_browser_entries: Vec::new(),
            file_browser_scroll: 0,
            file_browser_scrollbar: ScrollbarState::new(0),
            file_browser_list_state: ListState::default(),
            file_browser_preview_scroll: 0,
            file_browser_path_input: String::new(),
            file_browser_path_cursor: 0,
            file_browser_path_focused: false,
            focus: DotfileSelectionFocus::FilesList, // Start with files list focused
        }
    }
}

/// Application UI state
#[derive(Debug)]
pub struct UiState {
    pub current_screen: Screen,
    pub selected_index: usize,
    pub github_auth: GitHubAuthState,
    pub dotfile_selection: DotfileSelectionState,
    pub has_changes_to_push: bool, // Whether there are uncommitted or unpushed changes
}

impl UiState {
    pub fn new() -> Self {
        Self {
            current_screen: Screen::Welcome,
            selected_index: 0,
            github_auth: GitHubAuthState::default(),
            dotfile_selection: DotfileSelectionState::default(),
            has_changes_to_push: false,
        }
    }
}

// Legacy render functions removed - replaced by components:
// - render_welcome() -> WelcomeComponent
// - render_main_menu() -> MainMenuComponent
// - render_github_auth() -> GitHubAuthComponent
// - render_message() -> MessageComponent
// - render_synced_files() -> SyncedFilesComponent

/// Render the dotfile selection screen
pub fn render_dotfile_selection(frame: &mut Frame, state: &mut UiState) -> Result<(), Box<dyn std::error::Error>> {
    let area = frame.size();
    let selection_state = &mut state.dotfile_selection;

    // Layout: Title/Description, Content (list + preview), Footer
    let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 6, 2);

    // Header: Use common header component
    let _ = crate::components::header::Header::render(
        frame,
        header_chunk,
        "dotzz - Select Dotfiles to Sync",
        "Select the dotfiles you want to sync to your repository. Selected files will be copied to the repo and symlinked back to their original locations."
    );

    // Check if file browser is active - render as popup
    if selection_state.file_browser_mode {
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
                Constraint::Min(0),   // File list and preview
                Constraint::Length(2), // Footer (1 for border, 1 for text)
            ])
            .split(popup_area);

        // Current path display
        let path_display = Paragraph::new(selection_state.file_browser_path.to_string_lossy().to_string())
            .block(Block::default()
                .borders(Borders::ALL)
                .title("Current Directory")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(Color::Black)));
        frame.render_widget(path_display, browser_chunks[0]);

        // Path input field - use InputField component
        let path_input_text = if selection_state.file_browser_path_input.is_empty() {
            selection_state.file_browser_path.to_string_lossy().to_string()
        } else {
            selection_state.file_browser_path_input.clone()
        };

        let cursor_pos = if selection_state.file_browser_path_input.is_empty() {
            // If showing current directory, cursor at end
            path_input_text.chars().count()
        } else {
            // Use actual cursor position
            selection_state.file_browser_path_cursor.min(path_input_text.chars().count())
        };

        InputField::render(
            frame,
            browser_chunks[1],
            &path_input_text,
            cursor_pos,
            selection_state.file_browser_path_focused,
            "Path Input",
            None, // No placeholder since we show current directory
            Alignment::Left,
        )?;

        // Split list and preview
        let list_preview_chunks = create_split_layout(browser_chunks[2], &[50, 50]);

        // File list using ListState (like ratatui list example)
        let items: Vec<ListItem> = selection_state.file_browser_entries
            .iter()
            .map(|path| {
                let is_dir = if path == Path::new("..") {
                    true
                } else {
                    let full_path = if path.is_absolute() {
                        path.clone()
                    } else {
                        selection_state.file_browser_path.join(path)
                    };
                    full_path.is_dir()
                };

                let name = if path == Path::new("..") {
                    ".. (parent)".to_string()
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

        // Update scrollbar state based on ListState selection
        let total_items = selection_state.file_browser_entries.len();
        let selected_index = selection_state.file_browser_list_state.selected().unwrap_or(0);
        selection_state.file_browser_scrollbar = selection_state.file_browser_scrollbar
            .content_length(total_items)
            .position(selected_index);

        // Add focus indicator to file browser list (border color only)
        let list_title = "Select File or Directory (Enter to load path)";
        let list_border_style = if selection_state.focus == DotfileSelectionFocus::FileBrowserList {
            Style::default().fg(Color::Cyan).bg(Color::Black)
        } else {
            Style::default().bg(Color::Black)
        };

        let list = List::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .title_alignment(Alignment::Center)
                .border_style(list_border_style))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            )
            .highlight_symbol("> ");

        // Use StatefulWidget::render to let ListState handle scrolling automatically
        StatefulWidget::render(list, list_preview_chunks[0], frame.buffer_mut(), &mut selection_state.file_browser_list_state);

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_preview_chunks[0],
            &mut selection_state.file_browser_scrollbar,
        );

        // Preview panel with scrolling support - use common component
        if let Some(selected_index) = selection_state.file_browser_list_state.selected() {
            if selected_index < selection_state.file_browser_entries.len() {
                let selected = &selection_state.file_browser_entries[selected_index];
                let full_path = if selected == Path::new("..") {
                    selection_state.file_browser_path.parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("/"))
                } else if selected.is_absolute() {
                    selected.clone()
                } else {
                    selection_state.file_browser_path.join(selected)
                };

                let is_focused = selection_state.focus == DotfileSelectionFocus::FileBrowserPreview;
                let preview_title = if is_focused {
                    "Preview (u/d: Scroll)"
                } else {
                    "Preview"
                };

                crate::components::file_preview::FilePreview::render(
                    frame,
                    list_preview_chunks[1],
                    &full_path,
                    selection_state.file_browser_preview_scroll,
                    is_focused,
                    Some(preview_title),
                ).map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
            } else {
                // Empty state
                let empty_preview = Paragraph::new("No selection")
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title("Preview")
                        .title_alignment(Alignment::Center));
                frame.render_widget(empty_preview, list_preview_chunks[1]);
            }
        } else {
            // Empty state
            let empty_preview = Paragraph::new("No selection")
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title("Preview")
                    .title_alignment(Alignment::Center));
            frame.render_widget(empty_preview, list_preview_chunks[1]);
        }

        // Footer for file browser (inside popup)
        if browser_chunks.len() > 3 && browser_chunks[3].height > 0 {
            let footer_block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::DarkGray))
                .style(Style::default().bg(Color::Black));
            let footer_inner = footer_block.inner(browser_chunks[3]);
            let footer = Paragraph::new("Tab: Switch Focus | ↑↓: Navigate List | u/d: Scroll Preview | Enter: Load Path | Esc: Cancel")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            frame.render_widget(footer_block, browser_chunks[3]);
            frame.render_widget(footer, footer_inner);
        }

        // Also render main footer (outside popup, at bottom of screen)
        let _ = Footer::render(frame, footer_chunk, "File Browser Active - Esc: Cancel")?;
    } else if selection_state.adding_custom_file && !selection_state.file_browser_mode {
        // Custom file input mode
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3), // Input field
            ])
            .split(content_chunk);

        // Custom file input - use InputField component
        let input_text = &selection_state.custom_file_input;
        let cursor_pos = selection_state.custom_file_cursor.min(input_text.chars().count());

        InputField::render(
            frame,
            input_chunks[1],
            input_text,
            cursor_pos,
            selection_state.custom_file_focused,
            "Custom File Path",
            Some("Enter file path (e.g., ~/.myconfig or /path/to/file)"),
            Alignment::Center,
        )?;

        // Footer for custom file input - use Footer component
        let _ = Footer::render(frame, footer_chunk, "Enter: Add File | Esc: Cancel | Tab: Focus/Unfocus")?;
    } else {
        // Normal dotfile selection view
        // Split content into list and preview (always show preview for selected file)
        // If status message is showing, don't show preview
                let (list_area, preview_area_opt) = if selection_state.status_message.is_some() {
                    // When status is showing, use full width for list
                    (content_chunk, None::<Rect>)
                } else {
                    let content_chunks = create_split_layout(content_chunk, &[50, 50]);
                    (content_chunks[0], Some(content_chunks[1]))
                };

        // File list using ListState (like ratatui list example)
        let items: Vec<ListItem> = selection_state.dotfiles
            .iter()
            .enumerate()
            .map(|(i, dotfile)| {
                let is_selected = selection_state.selected_for_sync.contains(&i);

                let prefix = if is_selected {
                    "✓ "
                } else {
                    "  "
                };

                let style = if is_selected {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::White)
                };

                let path_str = dotfile.relative_path.to_string_lossy();
                ListItem::new(format!("{}{}", prefix, path_str)).style(style)
            })
            .collect();

        // Update scrollbar state for dotfile list based on ListState selection
        let total_dotfiles = selection_state.dotfiles.len();
        let selected_index = selection_state.dotfile_list_state.selected().unwrap_or(0);
        selection_state.dotfile_list_scrollbar = selection_state.dotfile_list_scrollbar
            .content_length(total_dotfiles)
            .position(selected_index);

        // Add focus indicator to files list (border color only)
        let list_title = format!("Found {} dotfiles", selection_state.dotfiles.len());
        let list_border_style = if selection_state.focus == DotfileSelectionFocus::FilesList {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };

        let list = List::new(items)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(list_title)
                .title_alignment(Alignment::Center)
                .border_style(list_border_style))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            )
            .highlight_symbol("> ");

        // Use StatefulWidget::render to let ListState handle scrolling automatically
        StatefulWidget::render(list, list_area, frame.buffer_mut(), &mut selection_state.dotfile_list_state);

        // Render scrollbar for dotfile list
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_area,
            &mut selection_state.dotfile_list_scrollbar,
        );

        // Preview panel (only show if status message is not showing)
        if let Some(preview_rect) = preview_area_opt {
            if let Some(selected_index) = selection_state.dotfile_list_state.selected() {
                if selected_index < selection_state.dotfiles.len() {
                    let dotfile = &selection_state.dotfiles[selected_index];

                    // Use common preview component
                    let is_focused = selection_state.focus == DotfileSelectionFocus::Preview;
                    let preview_title = format!("Preview: {} (u/d: Scroll)", dotfile.relative_path.to_string_lossy());

                    crate::components::file_preview::FilePreview::render(
                        frame,
                        preview_rect,
                        &dotfile.original_path,
                        selection_state.preview_scroll,
                        is_focused,
                        Some(&preview_title),
                    ).map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;
                } else {
                    // Empty state
                    let empty_preview = Paragraph::new("No file selected")
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .title("Preview")
                            .title_alignment(Alignment::Center));
                    frame.render_widget(empty_preview, preview_rect);
                }
            } else {
                // Empty state
                let empty_preview = Paragraph::new("No file selected")
                    .block(Block::default()
                        .borders(Borders::ALL)
                        .title("Preview")
                        .title_alignment(Alignment::Center));
                frame.render_widget(empty_preview, preview_rect);
            }
        }

        // Status message overlay (if present)
        if let Some(status) = &selection_state.status_message {
            let status_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(10), // Status message area
                ])
                .split(content_chunk);

            // Clear area with background
            frame.render_widget(Clear, status_chunks[1]);
            frame.render_widget(Block::default().style(Style::default().bg(Color::DarkGray)), status_chunks[1]);

            let status_block = Block::default()
                .borders(Borders::ALL)
                .title("Sync Summary")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(Color::DarkGray));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, status_chunks[1]);
        }

        // Footer - always render in main footer area
        let footer_text = if let Some(_) = &selection_state.status_message {
            "Enter: Continue".to_string()
        } else {
            if selection_state.selected_for_sync.is_empty() {
                "Tab: Switch Focus | ↑↓: Navigate | Space/Enter: Toggle | a: Add Custom File | u/d: Scroll Preview | s: Sync | q/Esc: Back".to_string()
            } else {
                format!(
                    "Tab: Switch Focus | ↑↓: Navigate | Space/Enter: Toggle | a: Add Custom File | u/d: Scroll Preview | s: Sync ({} selected) | q/Esc: Back",
                    selection_state.selected_for_sync.len()
                )
            }
        };

        // Use Footer component
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;
    }
    Ok(())
}

// Legacy render functions removed - replaced by components
// popup_area removed - use crate::utils::center_popup instead
// render_synced_files_legacy removed - replaced by SyncedFilesComponent
