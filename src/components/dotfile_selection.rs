use crate::components::component::{Component, ComponentAction};
use crate::components::file_preview::FilePreview;
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::input_field::InputField;
use crate::styles::{theme as ui_theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::{DotfileSelectionFocus, UiState};
use crate::utils::{
    center_popup, create_split_layout, create_standard_layout, focused_border_style,
    unfocused_border_style,
};
use anyhow::Result;
use crossterm::event::Event;
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
    StatefulWidget, Wrap,
};
use std::path::{Path, PathBuf};
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

/// Dotfile selection component
/// Note: Event handling is done in app.rs due to complex state dependencies
/// This component handles rendering with Clear widget and can be extended with mouse support
pub struct DotfileSelectionComponent;

impl DotfileSelectionComponent {
    pub fn new() -> Self {
        Self
    }

    /// Render with state - this is the main render method that takes UiState
    pub fn render_with_state(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &mut UiState,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        // Clear the entire area first to prevent background bleed-through
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let background = Block::default().style(Style::default().bg(Color::Reset));
        frame.render_widget(background, area);

        let selection_state = &mut state.dotfile_selection;

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
        if selection_state.show_custom_file_confirm {
            self.render_custom_file_confirm(frame, area, selection_state)?;
        }
        // Check if file browser is active - render as popup
        else if selection_state.file_browser_mode {
            self.render_file_browser(
                frame,
                area,
                selection_state,
                footer_chunk,
                syntax_set,
                theme,
            )?;
        } else if selection_state.adding_custom_file {
            self.render_custom_file_input(frame, content_chunk, footer_chunk, selection_state)?;
        } else {
            self.render_dotfile_list(
                frame,
                content_chunk,
                footer_chunk,
                selection_state,
                syntax_set,
                theme,
            )?;
        }

        Ok(())
    }

    fn render_file_browser(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        selection_state: &mut crate::ui::DotfileSelectionState,
        footer_chunk: Rect,
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
            selection_state
                .file_browser_path
                .to_string_lossy()
                .to_string(),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Current Directory")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(Color::Reset)),
        );
        frame.render_widget(path_display, browser_chunks[0]);

        // Path input field - use InputField component
        let path_input_text = if selection_state.file_browser_path_input.is_empty() {
            selection_state
                .file_browser_path
                .to_string_lossy()
                .to_string()
        } else {
            selection_state.file_browser_path_input.clone()
        };

        let cursor_pos = if selection_state.file_browser_path_input.is_empty() {
            path_input_text.chars().count()
        } else {
            selection_state
                .file_browser_path_cursor
                .min(path_input_text.chars().count())
        };

        InputField::render(
            frame,
            browser_chunks[1],
            &path_input_text,
            cursor_pos,
            selection_state.file_browser_path_focused,
            "Path Input",
            None,
            Alignment::Left,
            false, // Not disabled
        )?;

        // Split list and preview
        let list_preview_chunks = create_split_layout(browser_chunks[2], &[50, 50]);

        // File list using ListState
        let items: Vec<ListItem> = selection_state
            .file_browser_entries
            .iter()
            .map(|path| {
                let is_dir = if path == Path::new("..") {
                    true
                } else {
                    let full_path = if path.is_absolute() {
                        path.to_path_buf()
                    } else {
                        selection_state.file_browser_path.join(path)
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
        let total_items = selection_state.file_browser_entries.len();
        let selected_index = selection_state
            .file_browser_list_state
            .selected()
            .unwrap_or(0);
        selection_state.file_browser_scrollbar = selection_state
            .file_browser_scrollbar
            .content_length(total_items)
            .position(selected_index);

        // Add focus indicator to file browser list
        let t = ui_theme();
        let list_title = "Select File or Directory (Enter to load path)";
        let list_border_style = if selection_state.focus == DotfileSelectionFocus::FileBrowserList {
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
            &mut selection_state.file_browser_list_state,
        );

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_preview_chunks[0],
            &mut selection_state.file_browser_scrollbar,
        );

        // Preview panel
        if let Some(selected_index) = selection_state.file_browser_list_state.selected() {
            if selected_index < selection_state.file_browser_entries.len() {
                let selected = &selection_state.file_browser_entries[selected_index];
                let full_path = if selected == Path::new("..") {
                    selection_state
                        .file_browser_path
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| PathBuf::from("/"))
                } else if selected == Path::new(".") {
                    // Current folder
                    selection_state.file_browser_path.clone()
                } else if selected.is_absolute() {
                    selected.to_path_buf()
                } else {
                    selection_state.file_browser_path.join(selected)
                };

                let is_focused = selection_state.focus == DotfileSelectionFocus::FileBrowserPreview;
                let preview_title = if is_focused {
                    "Preview (u/d: Scroll)"
                } else {
                    "Preview"
                };

                FilePreview::render(
                    frame,
                    list_preview_chunks[1],
                    &full_path,
                    selection_state.file_browser_preview_scroll,
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
            let footer = Paragraph::new("Tab: Switch Focus | ↑↓: Navigate List | u/d: Scroll Preview | Enter: Load Path | Esc: Cancel")
                .style(Style::default().fg(t.text_muted))
                .alignment(Alignment::Center);
            frame.render_widget(footer_block, browser_chunks[3]);
            frame.render_widget(footer, footer_inner);
        }

        // Also render main footer (outside popup, at bottom of screen)
        let _ = Footer::render(frame, footer_chunk, "File Browser Active - Esc: Cancel")?;

        Ok(())
    }

    fn render_custom_file_input(
        &mut self,
        frame: &mut Frame,
        content_chunk: Rect,
        footer_chunk: Rect,
        selection_state: &mut crate::ui::DotfileSelectionState,
    ) -> Result<()> {
        let input_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(3), // Input field
            ])
            .split(content_chunk);

        let input_text = &selection_state.custom_file_input;
        let cursor_pos = selection_state
            .custom_file_cursor
            .min(input_text.chars().count());

        InputField::render(
            frame,
            input_chunks[1],
            input_text,
            cursor_pos,
            selection_state.custom_file_focused,
            "Custom File Path",
            Some("Enter file path (e.g., ~/.myconfig or /path/to/file)"),
            Alignment::Center,
            false, // Not disabled
        )?;

        let _ = Footer::render(
            frame,
            footer_chunk,
            "Enter: Add File | Esc: Cancel | Tab: Focus/Unfocus",
        )?;

        Ok(())
    }

    fn render_dotfile_list(
        &mut self,
        frame: &mut Frame,
        content_chunk: Rect,
        footer_chunk: Rect,
        selection_state: &mut crate::ui::DotfileSelectionState,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        // Split content into left (list + description) and right (preview)
        let (left_area, preview_area_opt) = if selection_state.status_message.is_some() {
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
        let items: Vec<ListItem> = selection_state
            .dotfiles
            .iter()
            .enumerate()
            .map(|(i, dotfile)| {
                let is_selected = selection_state.selected_for_sync.contains(&i);
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
        let total_dotfiles = selection_state.dotfiles.len();
        let selected_index = selection_state.dotfile_list_state.selected().unwrap_or(0);
        selection_state.dotfile_list_scrollbar = selection_state
            .dotfile_list_scrollbar
            .content_length(total_dotfiles)
            .position(selected_index);

        // Add focus indicator to files list
        let list_title = format!("Found {} dotfiles", selection_state.dotfiles.len());
        let list_border_style = if selection_state.focus == DotfileSelectionFocus::FilesList {
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
            &mut selection_state.dotfile_list_state,
        );

        // Render scrollbar
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            list_area,
            &mut selection_state.dotfile_list_scrollbar,
        );

        // Description block
        if let Some(selected_index) = selection_state.dotfile_list_state.selected() {
            if selected_index < selection_state.dotfiles.len() {
                let dotfile = &selection_state.dotfiles[selected_index];
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
            if let Some(selected_index) = selection_state.dotfile_list_state.selected() {
                if selected_index < selection_state.dotfiles.len() {
                    let dotfile = &selection_state.dotfiles[selected_index];
                    let is_focused = selection_state.focus == DotfileSelectionFocus::Preview;
                    let preview_title = format!(
                        "Preview: {} (u/d: Scroll)",
                        dotfile.relative_path.to_string_lossy()
                    );

                    FilePreview::render(
                        frame,
                        preview_rect,
                        &dotfile.original_path,
                        selection_state.preview_scroll,
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
        if let Some(status) = &selection_state.status_message {
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
        let backup_status = if selection_state.backup_enabled {
            "ON"
        } else {
            "OFF"
        };
        let footer_text = if selection_state.status_message.is_some() {
            "Enter: Continue".to_string()
        } else {
            format!("Tab: Switch Focus | ↑↓: Navigate | Space/Enter: Toggle Selection | a: Add Custom File | u/d: Scroll Preview | b: Backup ({}) | q/Esc: Back", backup_status)
        };

        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    fn render_custom_file_confirm(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        selection_state: &crate::ui::DotfileSelectionState,
    ) -> Result<()> {
        let t = ui_theme();
        // Dim the background
        let dim = Block::default().style(Style::default().bg(Color::Reset).fg(t.text_muted));
        frame.render_widget(dim, area);

        // Create centered popup
        let popup_area = crate::utils::center_popup(area, 70, 40);
        frame.render_widget(Clear, popup_area);

        let path = selection_state
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
        let instructions = Paragraph::new("Press Y/Enter to confirm, N/Esc to cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(t.text_muted));
        frame.render_widget(instructions, chunks[7]);

        Ok(())
    }
}

impl Component for DotfileSelectionComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // This method is required by the trait but we use render_with_state instead
        // Clear the area as a fallback
        frame.render_widget(Clear, area);
        let background = Block::default().style(Style::default().bg(Color::Reset));
        frame.render_widget(background, area);
        Ok(())
    }

    fn handle_event(&mut self, _event: Event) -> Result<ComponentAction> {
        // Event handling is done in app.rs due to complex dependencies
        Ok(ComponentAction::None)
    }
}
