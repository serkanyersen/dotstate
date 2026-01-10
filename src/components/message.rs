use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::message_box::MessageBox;
use crate::config::Config;
use crate::keymap::Action;
use crate::ui::Screen;
use crate::utils::create_standard_layout;
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear};

/// Message component for displaying status messages
pub struct MessageComponent {
    title: String,
    message: String,
    screen_type: Screen,
    config: Option<Config>,
}

impl MessageComponent {
    pub fn new(title: String, message: String, screen_type: Screen) -> Self {
        Self {
            title,
            message,
            screen_type,
            config: None,
        }
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }
}

impl Component for MessageComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        let t = crate::styles::theme();
        // Background - use Reset to inherit terminal's native background
        let background = Block::default().style(t.background_style());
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
            Screen::SyncWithRemote => Some(t.success), // Success color for sync
            _ => Some(t.warning),                      // Warning color for deactivation
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
                // Use keymap if available, otherwise accept any key (as footer says "Press any key")
                let action = self.config.as_ref()
                    .map(|c| c.keymap.get_action(key.code, key.modifiers))
                    .flatten();

                match action {
                    Some(Action::Confirm) | Some(Action::ToggleSelect) | Some(Action::Quit) | Some(Action::Cancel) => {
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                    _ => {
                        // If no action mapped or keymap not available, accept any key press
                        // (Footer says "Press any key or click to continue")
                        Ok(ComponentAction::Navigate(Screen::MainMenu))
                    }
                }
            },
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
