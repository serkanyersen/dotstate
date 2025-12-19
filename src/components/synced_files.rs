use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::config::Config;
use crate::ui::Screen;
use crate::utils::{create_standard_layout, get_home_dir};

/// Synced files view component
pub struct SyncedFilesComponent {
    config: Config,
    list_state: ListState,
    scrollbar_state: ScrollbarState,
}

impl SyncedFilesComponent {
    pub fn new(config: Config) -> Self {
        let file_count = config.synced_files.len();
        let mut list_state = ListState::default();
        if !config.synced_files.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            config,
            list_state,
            scrollbar_state: ScrollbarState::new(file_count.saturating_sub(1)),
        }
    }

    pub fn update_config(&mut self, config: Config) {
        let was_empty = self.config.synced_files.is_empty();
        self.config = config;
        if was_empty && !self.config.synced_files.is_empty() {
            self.list_state.select(Some(0));
        }
        self.scrollbar_state = ScrollbarState::new(self.config.synced_files.len().saturating_sub(1));
    }
}

impl Component for SyncedFilesComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 6, 2);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "dotzz - View Synced Files",
            "These are the files currently synced to your repository. Files are stored in the repo and symlinked back to their original locations."
        )?;

        // Content: List of synced files
        if self.config.synced_files.is_empty() {
            let empty_message = Paragraph::new(
                "No files are currently synced.\n\nGo to 'Scan & Select Dotfiles' to start syncing your dotfiles."
            )
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true })
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title("Synced Files")
                    .title_alignment(Alignment::Center)
                    .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)));
            frame.render_widget(empty_message, content_chunk);
        } else {
            let items: Vec<ListItem> = self.config.synced_files
                .iter()
                .enumerate()
                .map(|(i, path)| {
                    // Check if file exists and is a symlink
                    let home_dir = get_home_dir();
                    let full_path = home_dir.join(path);
                    let status = if !full_path.exists() {
                        "⚠ Missing"
                    } else if std::fs::symlink_metadata(&full_path)
                        .map(|m| m.file_type().is_symlink())
                        .unwrap_or(false) {
                        "✓ Synced"
                    } else {
                        "⚠ Not symlinked"
                    };

                    let style = if self.list_state.selected() == Some(i) {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    ListItem::new(format!("{} {}", status, path))
                        .style(style)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(format!("Synced Files ({})", self.config.synced_files.len()))
                    .title_alignment(Alignment::Center)
                    .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)));

            frame.render_stateful_widget(list, content_chunk, &mut self.list_state);

            // Update scrollbar state
            if let Some(selected) = self.list_state.selected() {
                self.scrollbar_state = self.scrollbar_state.position(selected);
            }

            // Render scrollbar
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            frame.render_stateful_widget(scrollbar, content_chunk, &mut self.scrollbar_state);
        }

        // Footer
        let _ = Footer::render(frame, footer_chunk, "q/Esc/Click: Back to Main Menu")?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                    KeyCode::Up => {
                        if !self.config.synced_files.is_empty() {
                            self.list_state.select_previous();
                        }
                        Ok(ComponentAction::Update)
                    }
                    KeyCode::Down => {
                        if !self.config.synced_files.is_empty() {
                            self.list_state.select_next();
                        }
                        Ok(ComponentAction::Update)
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Click anywhere to go back (simple for now)
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                    MouseEventKind::ScrollUp => {
                        if !self.config.synced_files.is_empty() {
                            self.list_state.select_previous();
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if !self.config.synced_files.is_empty() {
                            self.list_state.select_next();
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            _ => Ok(ComponentAction::None),
        }
    }

}
