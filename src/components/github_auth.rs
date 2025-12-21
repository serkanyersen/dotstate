use anyhow::Result;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use crate::components::component::{Component, ComponentAction};
use crate::components::header::Header;
use crate::components::footer::Footer;
use crate::components::input_field::InputField;
use crate::ui::{GitHubAuthState, GitHubAuthField};
use crate::utils::{create_standard_layout, focused_border_style, unfocused_border_style, disabled_border_style, disabled_text_style};

/// GitHub authentication component
/// Note: Event handling is done in app.rs due to complex state dependencies
pub struct GitHubAuthComponent {
    pub auth_state: GitHubAuthState,
    /// Clickable areas for input fields (for mouse support)
    token_area: Option<Rect>,
    repo_name_area: Option<Rect>,
    repo_location_area: Option<Rect>,
    visibility_area: Option<Rect>,
}

impl GitHubAuthComponent {
    pub fn new() -> Self {
        Self {
            auth_state: GitHubAuthState::default(),
            token_area: None,
            repo_name_area: None,
            repo_location_area: None,
            visibility_area: None,
        }
    }

    pub fn get_auth_state(&self) -> &GitHubAuthState {
        &self.auth_state
    }

    pub fn get_auth_state_mut(&mut self) -> &mut GitHubAuthState {
        &mut self.auth_state
    }

    /// Check if mouse click is in a specific field area
    fn is_click_in_area(&self, area: Option<Rect>, x: u16, y: u16) -> bool {
        if let Some(rect) = area {
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
        } else {
            false
        }
    }

    fn render_token_field(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let is_focused = self.auth_state.focused_field == GitHubAuthField::Token;

        // Show masked token if repo is already configured and not editing
        let display_text = if self.auth_state.repo_already_configured && !self.auth_state.is_editing_token {
            "••••••••••••••••••••••••••••••••••••••••"
        } else {
            &self.auth_state.token_input
        };

        let cursor_pos = if is_focused && (!self.auth_state.repo_already_configured || self.auth_state.is_editing_token) {
            self.auth_state.cursor_position.min(self.auth_state.token_input.chars().count())
        } else {
            0
        };

        // Disable token field if repo configured and not in edit mode
        let is_disabled = self.auth_state.repo_already_configured && !self.auth_state.is_editing_token;

        // Store area for mouse support
        let input_block = Block::bordered();
        self.token_area = Some(input_block.inner(area));

        InputField::render(
            frame,
            area,
            display_text,
            cursor_pos,
            is_focused && !is_disabled,
            "GitHub Token",
            Some("ghp_..."),
            Alignment::Left,
            is_disabled,
        )?;
        Ok(())
    }

    fn render_repo_name_field(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let is_focused = self.auth_state.focused_field == GitHubAuthField::RepoName;
        let is_disabled = self.auth_state.repo_already_configured;

        let cursor_pos = if is_focused && !is_disabled {
            self.auth_state.cursor_position.min(self.auth_state.repo_name_input.chars().count())
        } else {
            0
        };

        // Store area for mouse support
        let input_block = Block::bordered();
        self.repo_name_area = Some(input_block.inner(area));

        InputField::render(
            frame,
            area,
            &self.auth_state.repo_name_input,
            cursor_pos,
            is_focused && !is_disabled,
            "Repository Name",
            Some("dotstate-storage"),
            Alignment::Left,
            is_disabled,
        )?;
        Ok(())
    }

    fn render_repo_location_field(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let is_focused = self.auth_state.focused_field == GitHubAuthField::RepoLocation;
        let is_disabled = self.auth_state.repo_already_configured;

        let cursor_pos = if is_focused && !is_disabled {
            self.auth_state.cursor_position.min(self.auth_state.repo_location_input.chars().count())
        } else {
            0
        };

        // Store area for mouse support
        let input_block = Block::bordered();
        self.repo_location_area = Some(input_block.inner(area));

        InputField::render(
            frame,
            area,
            &self.auth_state.repo_location_input,
            cursor_pos,
            is_focused && !is_disabled,
            "Local Path",
            Some("~/.dotstate"),
            Alignment::Left,
            is_disabled,
        )?;
        Ok(())
    }

    fn render_visibility_field(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let is_focused = self.auth_state.focused_field == GitHubAuthField::IsPrivate;
        let is_disabled = self.auth_state.repo_already_configured;

        let border_style = if is_disabled {
            disabled_border_style()
        } else if is_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title("Repository Visibility")
            .title_alignment(Alignment::Left);

        // Store area for mouse support
        self.visibility_area = Some(block.inner(area));

        let visibility_text = if self.auth_state.is_private {
            "[✓] Private    [ ] Public"
        } else {
            "[ ] Private    [✓] Public"
        };

        let text_style = if is_disabled {
            disabled_text_style()
        } else if is_focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let paragraph = Paragraph::new(visibility_text)
            .block(block)
            .style(text_style);

        frame.render_widget(paragraph, area);
        Ok(())
    }

    fn render_help_panel(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        if let Some(status) = &self.auth_state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .title("Status")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Green));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, area);
        } else if let Some(error) = &self.auth_state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Red));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(error_para, area);
        } else {
            // Context-sensitive help based on focused field
            let (title, help_lines) = match self.auth_state.focused_field {
                GitHubAuthField::Token => (
                    "GitHub Token",
                    vec![
                        "Your Personal Access Token (PAT) for GitHub authentication.",
                        "",
                        "How to create:",
                        "1. Go to: github.com/settings/tokens",
                        "2. Click: Generate new token (classic)",
                        "3. Select scope: 'repo' (full control)",
                        "4. Generate and copy the token",
                        "",
                        "Format:",
                        "• Must start with 'ghp_'",
                        "• Length: 40+ characters",
                        "",
                        "Security:",
                        "• Stored in ~/.config/dotstate/config.toml",
                        "• File permissions: 600 (owner only)",
                        "• Never share your token!",
                    ],
                ),
                GitHubAuthField::RepoName => (
                    "Repository Name",
                    vec![
                        "The name of your dotfiles repository on GitHub.",
                        "",
                        "This repository will:",
                        "• Store your configuration files",
                        "• Be created if it doesn't exist",
                        "• Sync across all your computers",
                        "",
                        "Default: dotstate-storage",
                        "",
                        "⚠️ Important for returning users:",
                        "If you already have a repo with a different",
                        "name, make sure to enter it here, otherwise",
                        "a new repo with this name will be created.",
                        "",
                        "Requirements:",
                        "• Letters, numbers, hyphens, underscores",
                        "• No spaces or special characters",
                        "• Must be unique to your GitHub account",
                    ],
                ),
                GitHubAuthField::RepoLocation => (
                    "Local Repository Path",
                    vec![
                        "Where dotfiles will be stored on your computer.",
                        "",
                        "This directory will contain:",
                        "• Copies of your selected dotfiles",
                        "• Git repository data (.git folder)",
                        "• Profile-specific configurations",
                        "",
                        "Default: ~/.dotstate",
                        "",
                        "Tips:",
                        "• Use ~ for home directory",
                        "• Path will be created if it doesn't exist",
                        "• Should be in your home directory",
                        "• Don't use system directories",
                    ],
                ),
                GitHubAuthField::IsPrivate => (
                    "Repository Visibility",
                    vec![
                        "Control who can see your dotfiles repository.",
                        "",
                        "Private Repository (Recommended):",
                        "• Only you can access it",
                        "• Keeps your configs confidential",
                        "• May contain sensitive information",
                        "• API tokens, SSH keys, etc. stay private",
                        "",
                        "Public Repository:",
                        "• Anyone can view your dotfiles",
                        "• Good for sharing configurations",
                        "• ⚠️ Be careful with sensitive data!",
                        "",
                        "Toggle: Press Space",
                    ],
                ),
            };

            let help_block = Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(Color::Cyan));
            let help_para = Paragraph::new(help_lines.join("\n"))
                .block(help_block)
                .wrap(Wrap { trim: true })
                .scroll((self.auth_state.help_scroll as u16, 0));
            frame.render_widget(help_para, area);
        }
        Ok(())
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
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "dotstate - GitHub Setup",
            "Configure your GitHub repository for syncing dotfiles. All settings will be saved securely."
        )?;

        // Main content layout: instructions, fields on left, help on right
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Instructions + Input fields
                Constraint::Percentage(40), // Help panel
            ])
            .split(content_chunk);

        // Left side: instructions and fields
        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3), // Instructions
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Token input
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Repo name input
                Constraint::Length(2), // Reminder message (only shown for new installs)
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Repo location input
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Visibility toggle
                Constraint::Min(0),    // Spacer
            ])
            .split(main_layout[0]);

        // Instructions
        let instructions_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .border_type(ratatui::widgets::BorderType::Rounded);
        let instructions = Paragraph::new("Fill in the details below to set up your dotfiles repository. Use Tab to navigate between fields.")
            .block(instructions_block)
            .style(Style::default().fg(Color::White))
            .alignment(Alignment::Center);
        frame.render_widget(instructions, left_layout[0]);

        // Render each field with spacers
        self.render_token_field(frame, left_layout[2])?;
        self.render_repo_name_field(frame, left_layout[4])?;

        // Show reminder message for new installs (when repo is not already configured)
        if !self.auth_state.repo_already_configured {
            let reminder_text = "⚠️  If you already had a repo with a different name, make sure to enter it here, otherwise a new repo with this name will be created";
            let reminder = Paragraph::new(reminder_text)
                .style(Style::default().fg(Color::Yellow))
                .wrap(Wrap { trim: true });
            frame.render_widget(reminder, left_layout[5]);
        }

        self.render_repo_location_field(frame, left_layout[7])?;
        self.render_visibility_field(frame, left_layout[9])?;

        // Right side: Context-sensitive help
        self.render_help_panel(frame, main_layout[1])?;

        // Footer
        let footer_text = if self.auth_state.repo_already_configured {
            if self.auth_state.is_editing_token {
                "Ctrl+S: Save Token | Esc: Cancel"
            } else {
                "U: Update Token | Esc: Back"
            }
        } else {
            "Tab: Next Field | Shift+Tab: Previous | Space: Toggle (on visibility) | Ctrl+S: Save & Create | Esc: Cancel"
        };
        let _ = Footer::render(frame, footer_chunk, footer_text)?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<ComponentAction> {
        // Basic mouse support - clicking fields focuses them
        // Full event handling is done in app.rs due to complex dependencies
        match event {
            Event::Mouse(mouse) => {
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let x = mouse.column;
                        let y = mouse.row;

                        // Check which field was clicked
                        if self.is_click_in_area(self.token_area, x, y) {
                            // Token is only editable if repo not configured OR in edit mode
                            if !self.auth_state.repo_already_configured || self.auth_state.is_editing_token {
                                self.auth_state.focused_field = GitHubAuthField::Token;
                                self.auth_state.input_focused = true;
                                return Ok(ComponentAction::Update);
                            }
                        } else if self.is_click_in_area(self.repo_name_area, x, y) {
                            // Repo name is only editable if repo not configured
                            if !self.auth_state.repo_already_configured {
                                self.auth_state.focused_field = GitHubAuthField::RepoName;
                                self.auth_state.input_focused = true;
                                return Ok(ComponentAction::Update);
                            }
                        } else if self.is_click_in_area(self.repo_location_area, x, y) {
                            // Repo location is only editable if repo not configured
                            if !self.auth_state.repo_already_configured {
                                self.auth_state.focused_field = GitHubAuthField::RepoLocation;
                                self.auth_state.input_focused = true;
                                return Ok(ComponentAction::Update);
                            }
                        } else if self.is_click_in_area(self.visibility_area, x, y) {
                            // Only allow interaction if repo is not already configured
                            if !self.auth_state.repo_already_configured {
                                self.auth_state.focused_field = GitHubAuthField::IsPrivate;
                                self.auth_state.input_focused = true;
                                // Toggle visibility on click
                                self.auth_state.is_private = !self.auth_state.is_private;
                                return Ok(ComponentAction::Update);
                            }
                        } else {
                            // Click outside - unfocus
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
