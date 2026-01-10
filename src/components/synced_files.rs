use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::config::Config;
use crate::styles::{theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::Screen;
use crate::utils::{create_standard_layout, focused_border_style, get_home_dir};
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};

/// Synced files view component
pub struct SyncedFilesComponent {
    config: Config,
    list_state: ListState,
    scrollbar_state: ScrollbarState,
}

impl SyncedFilesComponent {
    pub fn new(config: Config) -> Self {
        // Get synced files from manifest
        let synced_files = Self::get_synced_files(&config);
        let file_count = synced_files.len();
        let mut list_state = ListState::default();
        if !synced_files.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            config,
            list_state,
            scrollbar_state: ScrollbarState::new(file_count.saturating_sub(1)),
        }
    }

    pub fn update_config(&mut self, config: Config) {
        let synced_files = Self::get_synced_files(&self.config);
        let was_empty = synced_files.is_empty();
        self.config = config;
        let new_synced_files = Self::get_synced_files(&self.config);
        if was_empty && !new_synced_files.is_empty() {
            self.list_state.select(Some(0));
        }
        self.scrollbar_state = ScrollbarState::new(new_synced_files.len().saturating_sub(1));
    }

    /// Get synced files from manifest for the active profile
    fn get_synced_files(config: &Config) -> Vec<String> {
        crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
            .ok()
            .and_then(|manifest| {
                manifest
                    .profiles
                    .iter()
                    .find(|p| p.name == config.active_profile)
                    .map(|p| p.synced_files.clone())
            })
            .unwrap_or_default()
    }
}

impl Component for SyncedFilesComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let background = Block::default().style(Style::default().bg(Color::Reset));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - View Synced Files",
            "These are the files currently synced to your repository. Files are stored in the repo and symlinked back to their original locations."
        )?;

        // Get synced files from manifest
        let synced_files = Self::get_synced_files(&self.config);

        let t = theme();
        // Content: List of synced files
        if synced_files.is_empty() {
            let empty_message = Paragraph::new(
                "No files are currently synced.\n\nGo to 'Scan & Select Dotfiles' to start syncing your dotfiles."
            )
                .style(t.muted_style())
                .wrap(Wrap { trim: true })
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title("Synced Files")
                    .title_alignment(Alignment::Center)
                    .padding(ratatui::widgets::Padding::new(2, 2, 2, 2)));
            frame.render_widget(empty_message, content_chunk);
        } else {
            let items: Vec<ListItem> = synced_files
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
                        .unwrap_or(false)
                    {
                        "✓ Synced"
                    } else {
                        "⚠ Not symlinked"
                    };

                    let style = if self.list_state.selected() == Some(i) {
                        t.highlight_style()
                    } else {
                        t.text_style()
                    };

                    ListItem::new(format!("{} {}", status, path)).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(focused_border_style())
                        .title(format!("Synced Files ({})", synced_files.len()))
                        .title_alignment(Alignment::Center)
                        .padding(ratatui::widgets::Padding::new(1, 1, 1, 1)),
                )
                .highlight_style(t.highlight_style())
                .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

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
        if let Event::Key(key) = event {
            if key.kind == KeyEventKind::Press {
                if let Some(action) = self.config.keymap.get_action(key.code, key.modifiers) {
                    use crate::keymap::Action;
                    match action {
                        Action::Quit | Action::Cancel => {
                            return Ok(ComponentAction::Navigate(Screen::MainMenu));
                        }
                        Action::MoveUp => {
                            let synced_files = Self::get_synced_files(&self.config);
                            if !synced_files.is_empty() {
                                self.list_state.select_previous();
                            }
                            return Ok(ComponentAction::Update);
                        }
                        Action::MoveDown => {
                            let synced_files = Self::get_synced_files(&self.config);
                            if !synced_files.is_empty() {
                                self.list_state.select_next();
                            }
                            return Ok(ComponentAction::Update);
                        }
                        _ => {}
                    }
                }
            }
        }

        match event {
            // Key events not matched by keymap (fallback or specific component handling?)
            // The original code only handled q/Esc/Up/Down. Keymap handles those now.
            // So we just fall through to mouse handling.
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Click anywhere to go back (simple for now)
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                    MouseEventKind::ScrollUp => {
                        let synced_files = Self::get_synced_files(&self.config);
                        if !synced_files.is_empty() {
                            self.list_state.select_previous();
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        let synced_files = Self::get_synced_files(&self.config);
                        if !synced_files.is_empty() {
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
