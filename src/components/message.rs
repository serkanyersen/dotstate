use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::components::message_box::MessageBox;
use crate::ui::Screen;
use crate::utils::create_standard_layout;

/// Message component for displaying status messages
pub struct MessageComponent {
    title: String,
    message: String,
    screen_type: Screen,
}

impl MessageComponent {
    pub fn new(title: String, message: String, screen_type: Screen) -> Self {
        Self {
            title,
            message,
            screen_type,
        }
    }

}

impl Component for MessageComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component
        let description = match self.screen_type {
            Screen::SyncWithRemote => "Syncing with remote repository...",
            _ => "Important Notice",
        };
        let _ = Header::render(frame, header_chunk, &self.title, description)?;

        // Message with styled block - use MessageBox component
        let message_color = match self.screen_type {
            Screen::SyncWithRemote => Some(Color::Green), // Success color for sync
            _ => Some(Color::Yellow), // Warning color for deactivation
        };

        MessageBox::render(
            frame,
            content_chunk,
            &self.message,
            None, // Let MessageBox auto-detect error from message content
            message_color,
        )?;

        // Footer
        let _ = Footer::render(frame, footer_chunk, "Press any key or click to continue")?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('q') | KeyCode::Esc => {
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Click anywhere to continue
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            _ => Ok(ComponentAction::None),
        }
    }

}
