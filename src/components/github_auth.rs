use crate::components::component::{Component, ComponentAction};
use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::components::input_field::InputField;
use crate::styles::{theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::{GitHubAuthField, GitHubAuthState, SetupMode};
use crate::utils::{
    create_standard_layout, disabled_border_style, disabled_text_style, focused_border_style,
    unfocused_border_style,
};
use anyhow::Result;
use crossterm::event::{Event, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Paragraph, Wrap,
};

/// GitHub authentication component (also handles local repo setup)
/// Note: Event handling is done in app.rs due to complex state dependencies
pub struct GitHubAuthComponent {
    pub auth_state: GitHubAuthState,
    /// Clickable areas for input fields (for mouse support)
    token_area: Option<Rect>,
    repo_name_area: Option<Rect>,
    repo_location_area: Option<Rect>,
    visibility_area: Option<Rect>,
    /// Clickable areas for mode selection
    mode_selection_areas: Vec<(Rect, usize)>, // (area, mode_index)
    /// List state for mode selection
    mode_list_state: ListState,
    /// Clickable area for local repo path input
    local_repo_path_area: Option<Rect>,
}

impl Default for GitHubAuthComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubAuthComponent {
    pub fn new() -> Self {
        let mut mode_list_state = ListState::default();
        mode_list_state.select(Some(0)); // Select first option by default

        Self {
            auth_state: GitHubAuthState::default(),
            token_area: None,
            repo_name_area: None,
            repo_location_area: None,
            visibility_area: None,
            mode_selection_areas: Vec::new(),
            mode_list_state,
            local_repo_path_area: None,
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
        let display_text =
            if self.auth_state.repo_already_configured && !self.auth_state.is_editing_token {
                "••••••••••••••••••••••••••••••••••••••••"
            } else {
                &self.auth_state.token_input
            };

        let cursor_pos = if is_focused
            && (!self.auth_state.repo_already_configured || self.auth_state.is_editing_token)
        {
            self.auth_state
                .cursor_position
                .min(self.auth_state.token_input.chars().count())
        } else {
            0
        };

        // Disable token field if repo configured and not in edit mode
        let is_disabled =
            self.auth_state.repo_already_configured && !self.auth_state.is_editing_token;

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
            self.auth_state
                .cursor_position
                .min(self.auth_state.repo_name_input.chars().count())
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
            Some(crate::config::default_repo_name().as_str()),
            Alignment::Left,
            is_disabled,
        )?;
        Ok(())
    }

    fn render_repo_location_field(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let is_focused = self.auth_state.focused_field == GitHubAuthField::RepoLocation;
        let is_disabled = self.auth_state.repo_already_configured;

        let cursor_pos = if is_focused && !is_disabled {
            self.auth_state
                .cursor_position
                .min(self.auth_state.repo_location_input.chars().count())
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
            Some("~/.config/dotstate/storage"),
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

        let t = theme();
        let text_style = if is_disabled {
            disabled_text_style()
        } else if is_focused {
            t.text_style()
        } else {
            Style::default().fg(t.text_muted)
        };

        let paragraph = Paragraph::new(visibility_text)
            .block(block)
            .style(text_style);

        frame.render_widget(paragraph, area);
        Ok(())
    }

    /// Render the mode selection screen (choosing between GitHub and Local modes)
    fn render_mode_selection(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let t = theme();
        // Layout: Header, Content, Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Repository Setup",
            "Choose how you want to set up your dotfiles repository.",
        )?;

        // Split content into left and right panels
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Left panel - options
                Constraint::Percentage(50), // Right panel - explanation
            ])
            .split(content_chunk);

        // Left panel: Mode selection list
        let options = vec![
            ListItem::new(vec![Line::from(vec![
                Span::styled("🔧 ", Style::default()),
                Span::styled("Create repository for me (GitHub)", t.success_style()),
            ])]),
            ListItem::new(vec![Line::from(vec![
                Span::styled("📁 ", Style::default()),
                Span::styled("Use my own repository", Style::default().fg(t.tertiary)),
            ])]),
        ];

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(focused_border_style())
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("📋 Choose Setup Method")
            .title_style(t.title_style())
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        // Store clickable areas
        self.mode_selection_areas.clear();
        let list_inner = list_block.inner(main_layout[0]);

        for i in 0..options.len() {
            let y = list_inner.y + i as u16;
            if y < list_inner.y + list_inner.height {
                self.mode_selection_areas
                    .push((Rect::new(list_inner.x, y, list_inner.width, 1), i));
            }
        }

        // Update list state selection
        self.mode_list_state
            .select(Some(self.auth_state.mode_selection_index));

        let list = List::new(options)
            .block(list_block)
            .highlight_style(t.highlight_style())
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        frame.render_stateful_widget(list, main_layout[0], &mut self.mode_list_state);

        // Right panel: Context-sensitive explanation
        let (title, explanation_lines) = if self.auth_state.mode_selection_index == 0 {
            (
                "GitHub Setup",
                vec![
                    Line::from(vec![Span::styled(
                        "Automatic GitHub Setup",
                        Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from("DotState will:"),
                    Line::from(vec![
                        Span::styled("  1. ", Style::default().fg(t.text_emphasis)),
                        Span::raw("Connect to GitHub using your token"),
                    ]),
                    Line::from(vec![
                        Span::styled("  2. ", Style::default().fg(t.text_emphasis)),
                        Span::raw("Create a repository for you"),
                    ]),
                    Line::from(vec![
                        Span::styled("  3. ", Style::default().fg(t.text_emphasis)),
                        Span::raw("Set up syncing automatically"),
                    ]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Requirements:",
                        Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                    )]),
                    Line::from("• GitHub account"),
                    Line::from("• Personal Access Token (PAT)"),
                    Line::from("  with 'repo' scope"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 ", Style::default().fg(t.secondary)),
                        Span::raw("Best for: Users who want a quick,"),
                    ]),
                    Line::from("   automated setup on GitHub."),
                ],
            )
        } else {
            (
                "Local Repository",
                vec![
                    Line::from(vec![Span::styled(
                        "Use Your Own Repository",
                        Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD),
                    )]),
                    Line::from(""),
                    Line::from("You set up the repository yourself:"),
                    Line::from(vec![
                        Span::styled("  1. ", Style::default().fg(t.text_emphasis)),
                        Span::raw("Create a repo on any git host"),
                    ]),
                    Line::from(vec![
                        Span::styled("  2. ", Style::default().fg(t.text_emphasis)),
                        Span::raw("Clone it to your machine"),
                    ]),
                    Line::from(vec![
                        Span::styled("  3. ", Style::default().fg(t.text_emphasis)),
                        Span::raw("Tell DotState where it is"),
                    ]),
                    Line::from(""),
                    Line::from(vec![Span::styled(
                        "Supports:",
                        Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                    )]),
                    Line::from("• GitHub, GitLab, Bitbucket"),
                    Line::from("• Self-hosted git servers"),
                    Line::from("• Any git remote"),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("💡 ", Style::default().fg(t.secondary)),
                        Span::raw("Best for: Users who already have"),
                    ]),
                    Line::from("   a repo or use non-GitHub hosts."),
                ],
            )
        };

        let explanation_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(t.primary))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title(format!("💡 {}", title))
            .title_style(t.title_style())
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let explanation_para = Paragraph::new(explanation_lines)
            .block(explanation_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(explanation_para, main_layout[1]);

        // Footer
        let _ = Footer::render(
            frame,
            footer_chunk,
            "↑↓: Navigate | Enter: Select | Esc: Cancel",
        )?;

        Ok(())
    }

    /// Render the local repository setup screen
    fn render_local_setup(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let t = theme();
        // Layout: Header, Content, Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - Local Repository Setup",
            "Point DotState to your existing git repository.",
        )?;

        // Split content into left and right panels
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Left panel - instructions + input
                Constraint::Percentage(40), // Right panel - help
            ])
            .split(content_chunk);

        // Left side: instructions and input
        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(14), // Instructions block
                Constraint::Length(1),  // Spacer
                Constraint::Length(3),  // Path input
                Constraint::Min(0),     // Spacer
            ])
            .split(main_layout[0]);

        // Instructions
        let instructions_lines = vec![
            Line::from(vec![Span::styled(
                "Set up your own git repository:",
                Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("1. ", Style::default().fg(t.text_emphasis)),
                Span::raw("Create a repository on any git host"),
            ]),
            Line::from("   (GitHub, GitLab, Bitbucket, etc.)"),
            Line::from(""),
            Line::from(vec![
                Span::styled("2. ", Style::default().fg(t.text_emphasis)),
                Span::raw("Clone it locally:"),
            ]),
            Line::from(vec![Span::styled(
                "   git clone <url> ~/.config/dotstate/storage",
                Style::default().fg(t.text_muted),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("3. ", Style::default().fg(t.text_emphasis)),
                Span::raw("Ensure you can push:"),
            ]),
            Line::from(vec![Span::styled(
                "   git push origin main",
                Style::default().fg(t.text_muted),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("4. ", Style::default().fg(t.text_emphasis)),
                Span::raw("Enter the local path below"),
            ]),
        ];

        let instructions_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(t.tertiary))
            .border_type(ratatui::widgets::BorderType::Rounded)
            .title("📋 Setup Instructions")
            .title_style(Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center);

        let instructions = Paragraph::new(instructions_lines)
            .block(instructions_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(instructions, left_layout[0]);

        // Path input field
        let is_disabled = self.auth_state.repo_already_configured;
        let cursor_pos = if !is_disabled {
            self.auth_state
                .local_repo_path_cursor
                .min(self.auth_state.local_repo_path_input.chars().count())
        } else {
            0
        };

        // Store area for mouse support
        let input_block = Block::bordered();
        self.local_repo_path_area = Some(input_block.inner(left_layout[2]));

        InputField::render(
            frame,
            left_layout[2],
            &self.auth_state.local_repo_path_input,
            cursor_pos,
            self.auth_state.input_focused && !is_disabled,
            "Local Repository Path",
            Some("~/.config/dotstate/storage"),
            Alignment::Left,
            is_disabled,
        )?;

        // Right side: Help panel
        self.render_local_help_panel(frame, main_layout[1])?;

        // Footer
        let footer_text = if self.auth_state.repo_already_configured {
            "Esc: Back"
        } else {
            "Ctrl+S: Validate & Save | Esc: Back to mode selection"
        };
        let _ = Footer::render(frame, footer_chunk, footer_text)?;

        Ok(())
    }

    /// Render help panel for local setup
    fn render_local_help_panel(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let t = theme();
        if let Some(status) = &self.auth_state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .title("Status")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, area);
        } else if let Some(error) = &self.auth_state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.error));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(error_para, area);
        } else if self.auth_state.repo_already_configured {
            // Show current configuration details when already configured
            let path = &self.auth_state.local_repo_path_input;
            let expanded_path = crate::git::expand_path(path);
            let validation = crate::git::validate_local_repo(&expanded_path);

            let remote_url = validation.remote_url.as_deref().unwrap_or("unknown");

            let help_lines = vec![
                Line::from(vec![Span::styled(
                    "Current Configuration",
                    Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Mode: ", Style::default().fg(t.primary)),
                    Span::raw("Local Repository"),
                ]),
                Line::from(""),
                Line::from(vec![Span::styled("Path: ", Style::default().fg(t.primary))]),
                Line::from(format!("  {}", path)),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Remote: ",
                    Style::default().fg(t.primary),
                )]),
                Line::from(format!("  {}", remote_url)),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(t.primary)),
                    Span::styled("✅ Configured", Style::default().fg(t.success)),
                ]),
            ];

            let help_block = Block::default()
                .borders(Borders::ALL)
                .title("📋 Repository Info")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .padding(ratatui::widgets::Padding::new(1, 1, 0, 0));

            let help_para = Paragraph::new(help_lines)
                .block(help_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(help_para, area);
        } else {
            let help_lines = vec![
                Line::from(vec![Span::styled(
                    "Repository Path",
                    Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from("Enter the path to your cloned git repository."),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Requirements:",
                    Style::default().fg(t.text_emphasis),
                )]),
                Line::from("• Must be a valid git repository"),
                Line::from("• Must have a remote named 'origin'"),
                Line::from("• Must be able to push to remote"),
                Line::from(""),
                Line::from(vec![Span::styled("Tips:", Style::default().fg(t.success))]),
                Line::from("• Use ~ for home directory"),
                Line::from("• SSH or HTTPS remotes both work"),
                Line::from("• Ensure your SSH keys or"),
                Line::from("  credentials are configured"),
            ];

            let help_block = Block::default()
                .borders(Borders::ALL)
                .title("💡 Help")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.primary))
                .border_type(ratatui::widgets::BorderType::Rounded)
                .padding(ratatui::widgets::Padding::new(1, 1, 0, 0));

            let help_para = Paragraph::new(help_lines)
                .block(help_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(help_para, area);
        }
        Ok(())
    }

    fn render_help_panel(&self, frame: &mut Frame, area: Rect) -> Result<()> {
        let t = theme();
        if let Some(status) = &self.auth_state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .title("Status")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, area);
        } else if let Some(error) = &self.auth_state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.error));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(error_para, area);
        } else {
            // Context-sensitive help based on focused field
            // Pre-compute default repo name for use in help text
            let default_repo_name = crate::config::default_repo_name();
            let default_repo_name_text = format!("Default: {}", default_repo_name);

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
                        &default_repo_name_text,
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
                        "Default: ~/.config/dotstate/storage",
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
                .border_style(Style::default().fg(t.primary));
            let help_para = Paragraph::new(help_lines.join("\n"))
                .block(help_block)
                .wrap(Wrap { trim: true })
                .scroll((self.auth_state.help_scroll as u16, 0));
            frame.render_widget(help_para, area);
        }
        Ok(())
    }

    /// Render progress screen when processing GitHub setup
    fn render_progress_screen(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        use ratatui::layout::Constraint;
        use ratatui::layout::Direction;
        use ratatui::layout::Layout;

        let t = theme();

        // Layout: Header, Content, Footer
        // Header component needs more height (6) to accommodate logo and description
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 6, 2);

        // Header
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - GitHub Setup",
            "Setting up your GitHub repository...",
        )?;

        // Center the progress message
        let progress_area = crate::utils::center_popup(content_chunk, 60, 15);

        // Progress block
        let progress_block = Block::default()
            .borders(Borders::ALL)
            .title("Progress")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(t.primary))
            .border_type(ratatui::widgets::BorderType::Rounded);

        // Status message with styling
        let status_text = if let Some(status) = &self.auth_state.status_message {
            status.clone()
        } else {
            "Processing...".to_string()
        };

        let status_para = Paragraph::new(status_text)
            .block(progress_block)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center)
            .style(t.text_style());

        frame.render_widget(status_para, progress_area);

        // Show error if any
        if let Some(error) = &self.auth_state.error_message {
            let error_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(15), // Progress area
                    Constraint::Length(8),  // Error area
                ])
                .split(content_chunk)[1];

            let error_block = Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.error));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(t.error));
            frame.render_widget(error_para, error_area);
        }

        // Footer
        let footer_text = if self.auth_state.error_message.is_some() {
            "Press Esc to go back and fix the error"
        } else if self
            .auth_state
            .status_message
            .as_ref()
            .map(|s| s.contains("✅"))
            .unwrap_or(false)
        {
            "Press Enter to continue"
        } else {
            "Please wait..."
        };
        let _ = Footer::render(frame, footer_chunk, footer_text)?;

        Ok(())
    }
}

impl Component for GitHubAuthComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let background = Block::default().style(Style::default().bg(Color::Reset));
        frame.render_widget(background, area);

        // If processing or in setup, show progress screen instead of input form
        if matches!(
            self.auth_state.step,
            crate::ui::GitHubAuthStep::Processing | crate::ui::GitHubAuthStep::SetupStep(_)
        ) {
            return self.render_progress_screen(frame, area);
        }

        // Check setup mode and render appropriate screen
        match self.auth_state.setup_mode {
            SetupMode::Choosing => {
                // Show mode selection screen (only for new setups)
                return self.render_mode_selection(frame, area);
            }
            SetupMode::Local => {
                // Show local setup screen
                return self.render_local_setup(frame, area);
            }
            SetupMode::GitHub => {
                // Continue to GitHub setup form below
            }
        }

        // Layout: Title/Description, Content, Footer
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 2);

        // Header: Use common header component
        let _ = Header::render(
            frame,
            header_chunk,
            "DotState - GitHub Setup",
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

        let t = theme();

        // Instructions
        let instructions_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(t.tertiary))
            .border_type(ratatui::widgets::BorderType::Rounded);
        let instructions = Paragraph::new("Fill in the details below to set up your dotfiles repository. Use Tab to navigate between fields.")
            .block(instructions_block)
            .style(t.text_style())
            .alignment(Alignment::Center);
        frame.render_widget(instructions, left_layout[0]);

        // Render each field with spacers
        self.render_token_field(frame, left_layout[2])?;
        self.render_repo_name_field(frame, left_layout[4])?;

        // Show reminder message for new installs (when repo is not already configured)
        if !self.auth_state.repo_already_configured {
            let reminder_text = "⚠️  If you already had a repo with a different name, make sure to enter it here, otherwise a new repo with this name will be created";
            let reminder = Paragraph::new(reminder_text)
                .style(Style::default().fg(t.warning))
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
        if let Event::Mouse(mouse) = event {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let x = mouse.column;
                let y = mouse.row;

                // Handle mode selection screen
                if matches!(self.auth_state.setup_mode, SetupMode::Choosing) {
                    for (rect, mode_index) in &self.mode_selection_areas {
                        if x >= rect.x
                            && x < rect.x + rect.width
                            && y >= rect.y
                            && y < rect.y + rect.height
                        {
                            self.auth_state.mode_selection_index = *mode_index;
                            self.mode_list_state.select(Some(*mode_index));
                            return Ok(ComponentAction::Update);
                        }
                    }
                    return Ok(ComponentAction::None);
                }

                // Handle local setup screen
                if matches!(self.auth_state.setup_mode, SetupMode::Local) {
                    if self.is_click_in_area(self.local_repo_path_area, x, y) {
                        if !self.auth_state.repo_already_configured {
                            self.auth_state.input_focused = true;
                            return Ok(ComponentAction::Update);
                        }
                    } else {
                        self.auth_state.input_focused = false;
                        return Ok(ComponentAction::Update);
                    }
                    return Ok(ComponentAction::None);
                }

                // Handle GitHub setup screen (existing logic)
                // Check which field was clicked
                if self.is_click_in_area(self.token_area, x, y) {
                    // Token is only editable if repo not configured OR in edit mode
                    if !self.auth_state.repo_already_configured || self.auth_state.is_editing_token
                    {
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
        }
        Ok(ComponentAction::None)
    }
}
