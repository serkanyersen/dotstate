use anyhow::Result;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::components::input_field::InputField;
use crate::ui::GitHubAuthState;
use crate::utils::{create_standard_layout, create_split_layout};

/// GitHub authentication component
/// Note: Event handling is done in app.rs due to complex state dependencies
pub struct GitHubAuthComponent {
    pub auth_state: GitHubAuthState,
    /// Clickable area for input field (for mouse support)
    input_area: Option<Rect>,
}

impl GitHubAuthComponent {
    pub fn new() -> Self {
        Self {
            auth_state: GitHubAuthState::default(),
            input_area: None,
        }
    }

    pub fn get_auth_state(&self) -> &GitHubAuthState {
        &self.auth_state
    }

    pub fn get_auth_state_mut(&mut self) -> &mut GitHubAuthState {
        &mut self.auth_state
    }

    /// Check if mouse click is in input area
    pub fn is_click_in_input(&self, x: u16, y: u16) -> bool {
        if let Some(rect) = self.input_area {
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
        } else {
            false
        }
    }
}

impl Component for GitHubAuthComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        // Layout: Title/Description, Content, Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 6, 2);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "dotzz - GitHub Setup",
            "Enter your GitHub Personal Access Token (PAT) to connect your repository. The token will be stored securely on your system."
        )?;

        // Content area: Input and Help/Status side by side
        let content_chunks = create_split_layout(content_chunk, &[60, 40]);

        // Split input area vertically to constrain input height
        let input_area_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Input field (compact, like ratatui example)
                Constraint::Min(0),   // Empty space below input
            ])
            .split(content_chunks[0]);

        // Token input - use InputField component
        let token = &self.auth_state.token_input;
        let cursor_pos = self.auth_state.cursor_position.min(token.chars().count());

        // Store input area for mouse support (calculate inner area)
        let input_block = Block::bordered();
        let input_inner = input_block.inner(input_area_chunks[0]);
        self.input_area = Some(input_inner);

        InputField::render(
            frame,
            input_area_chunks[0],
            token,
            cursor_pos,
            self.auth_state.input_focused,
            "GitHub Token",
            Some("Enter your GitHub Personal Access Token (ghp_...)"),
            Alignment::Center,
        )?;

        // Help/Status panel - split vertically
        let help_status_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0), // Help or Status
                Constraint::Length(1), // Toggle hint
            ])
            .split(content_chunks[1]);

        if let Some(status) = &self.auth_state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .title("Status")
                .title_alignment(Alignment::Center)
                .style(Style::default().bg(Color::DarkGray));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, help_status_chunks[0]);
        } else if let Some(error) = &self.auth_state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .title_alignment(Alignment::Center)
                .style(Style::default().fg(Color::Red));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(error_para, help_status_chunks[0]);
        } else if self.auth_state.show_help {
            let help_text = vec![
                "How to create a GitHub Token:",
                "",
                "1. Go to GitHub.com → Settings",
                "2. Developer settings → Personal access tokens",
                "3. Tokens (classic) → Generate new token",
                "4. Select scopes:",
                "   • repo (Full control of private repos)",
                "   • workflow (Update GitHub Action workflows)",
                "",
                "5. Copy the token (starts with ghp_)",
                "6. Paste it here",
                "",
                "Security:",
                "• Token stored in: ~/.config/dotzz/config.toml",
                "• File permissions: 600 (owner read/write only)",
                "• Never share your token!",
            ];

            let help_block = Block::default()
                .borders(Borders::ALL)
                .title("Help (Press ? to toggle)")
                .title_alignment(Alignment::Center);
            let help_para = Paragraph::new(help_text.join("\n"))
                .block(help_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(help_para, help_status_chunks[0]);
        }

        let toggle_hint = Paragraph::new("Press ? or F1 to toggle help")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(toggle_hint, help_status_chunks[1]);

        // Footer
        let _ = Footer::render(frame, footer_chunk, "Enter: Submit | Tab/Click: Focus/Unfocus | Esc: Cancel")?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        // Basic mouse support - clicking input area focuses it
        // Full event handling is done in app.rs due to complex dependencies
        match event {
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if self.is_click_in_input(mouse.column, mouse.row) {
                            self.auth_state.input_focused = true;
                            // Approximate cursor position based on click
                            if let Some(rect) = self.input_area {
                                let relative_x = mouse.column.saturating_sub(rect.x + 1) as usize;
                                self.auth_state.cursor_position = relative_x.min(self.auth_state.token_input.chars().count());
                            }
                            return Ok(ComponentAction::Update);
                        } else {
                            // Click outside input - unfocus
                            self.auth_state.input_focused = false;
                            return Ok(ComponentAction::Update);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(ComponentAction::None)
    }

}
