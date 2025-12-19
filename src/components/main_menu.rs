use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, StatefulWidget};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::utils::create_standard_layout;

/// Main menu component
pub struct MainMenuComponent {
    selected_index: usize,
    has_changes_to_push: bool,
    list_state: ListState,
    /// Clickable areas: (rect, menu_index)
    clickable_areas: Vec<(Rect, usize)>,
}

impl MainMenuComponent {
    pub fn new(has_changes_to_push: bool) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        Self {
            selected_index: 0,
            has_changes_to_push,
            list_state,
            clickable_areas: Vec::new(),
        }
    }

    pub fn set_selected(&mut self, index: usize) {
        self.selected_index = index;
        self.list_state.select(Some(index));
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_has_changes_to_push(&mut self, has_changes: bool) {
        self.has_changes_to_push = has_changes;
    }
}

impl Component for MainMenuComponent {
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
            "dotzz - Dotfile Manager",
            "Manage your dotfiles with ease. Sync to GitHub, organize by profiles, and keep your configuration files safe."
        )?;

        // Menu items
        let mut menu_items = vec![
            "Setup GitHub Repository",
            "Scan & Select Dotfiles",
            "View Synced Files",
            "Push Changes",
            "Pull Changes",
            "Manage Profiles",
        ];

        if self.has_changes_to_push {
            menu_items[3] = "Push Changes ⚠";
        }

        let items: Vec<ListItem> = menu_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == self.selected_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                } else if i == 3 && self.has_changes_to_push {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(*item).style(style)
            })
            .collect();

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title("Select an option")
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let list = List::new(items)
            .block(list_block.clone())
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
            )
            .highlight_symbol("▶ ");

        // Store clickable area for mouse support
        self.clickable_areas.clear();
        let list_inner = list_block.inner(content_chunk);
        let item_height = 1;
        for (i, _) in menu_items.iter().enumerate() {
            let y = list_inner.y + i as u16;
            if y < list_inner.y + list_inner.height {
                self.clickable_areas.push((
                    Rect::new(list_inner.x, y, list_inner.width, item_height),
                    i,
                ));
            }
        }

        // Use StatefulWidget for proper list rendering
        StatefulWidget::render(list, content_chunk, frame.buffer_mut(), &mut self.list_state);

        // Footer
        let _ = Footer::render(frame, footer_chunk, "↑↓: Navigate | Enter/Click: Select | q: Quit")?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Up => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                            self.list_state.select_previous();
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    KeyCode::Down => {
                        if self.selected_index < 5 {
                            self.selected_index += 1;
                            self.list_state.select_next();
                            Ok(ComponentAction::Update)
                        } else {
                            Ok(ComponentAction::None)
                        }
                    }
                    KeyCode::Enter => {
                        Ok(ComponentAction::Update) // App will handle selection
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        Ok(ComponentAction::Quit)
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Check if click is in any clickable area
                        for (rect, index) in &self.clickable_areas {
                            if mouse.column >= rect.x
                                && mouse.column < rect.x + rect.width
                                && mouse.row >= rect.y
                                && mouse.row < rect.y + rect.height {
                                self.selected_index = *index;
                                self.list_state.select(Some(*index));
                                return Ok(ComponentAction::Update);
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        if self.selected_index > 0 {
                            self.selected_index -= 1;
                            self.list_state.select_previous();
                            return Ok(ComponentAction::Update);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        if self.selected_index < 5 {
                            self.selected_index += 1;
                            self.list_state.select_next();
                            return Ok(ComponentAction::Update);
                        }
                    }
                    _ => {}
                }
                Ok(ComponentAction::None)
            }
            _ => Ok(ComponentAction::None),
        }
    }

}

