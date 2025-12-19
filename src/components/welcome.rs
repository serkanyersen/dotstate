use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::utils::create_standard_layout;

/// Welcome screen component
pub struct WelcomeComponent {
    /// Whether the component has been initialized
    initialized: bool,
}

impl WelcomeComponent {
    pub fn new() -> Self {
        Self {
            initialized: false,
        }
    }
}

impl Component for WelcomeComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first to prevent background bleed-through
        frame.render_widget(Clear, area);

        // Create a styled background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Title with better styling - centered in a bordered block
        let title_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let title = Paragraph::new("Welcome to dotzz")
            .style(Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(title_block);

        // Description with better formatting in a bordered block
        let message_block = Block::default()
            .borders(Borders::ALL)
            .title("About")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(Color::Blue))
            .padding(ratatui::widgets::Padding::new(2, 2, 2, 2));

        let message = Paragraph::new(
            "A friendly TUI tool for managing your dotfiles with GitHub sync\n\n\
            Get started by connecting your GitHub account and syncing your configuration files.\n\n\
            Features:\n\
            • Sync dotfiles to GitHub repositories\n\
            • Organize files by profiles\n\
            • Automatic symlink management\n\
            • Easy push/pull operations"
        )
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true })
            .block(message_block);

        frame.render_widget(title, header_chunk);
        frame.render_widget(message, content_chunk);

        // Footer
        let _ = Footer::render(frame, footer_chunk, "Press any key or click to continue")?;

        self.initialized = true;
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('\n') => {
                        // Will be handled by app to determine next screen
                        Ok(ComponentAction::Update)
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        Ok(ComponentAction::Quit)
                    }
                    _ => Ok(ComponentAction::None),
                }
            }
            Event::Mouse(mouse) => {
                if let MouseEventKind::Down(button) = mouse.kind {
                    if button == MouseButton::Left {
                        // Click anywhere to continue
                        Ok(ComponentAction::Update)
                    } else {
                        Ok(ComponentAction::None)
                    }
                } else {
                    Ok(ComponentAction::None)
                }
            }
            _ => Ok(ComponentAction::None),
        }
    }

}

