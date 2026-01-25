//! GitHub authentication screen controller.
//!
//! This screen handles the GitHub authentication and repository setup flow.
//! It supports two modes:
//! - GitHub mode: Creates/uses a GitHub repository with token authentication
//! - Local mode: Uses an existing local git repository

use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{theme, LIST_HIGHLIGHT_SYMBOL};
use crate::ui::{GitHubAuthField, GitHubAuthState, GitHubAuthStep, SetupMode};
use crate::utils::{
    create_standard_layout, disabled_border_style, disabled_text_style, focused_border_style,
    unfocused_border_style,
};
use crate::widgets::{TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use ratatui::Frame;

/// GitHub authentication screen controller.
///
/// This screen owns its state and handles both rendering and events.
pub struct GitHubAuthScreen {
    /// Screen owns its state
    state: GitHubAuthState,
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

impl GitHubAuthScreen {
    /// Create a new GitHub auth screen.
    pub fn new() -> Self {
        let mut mode_list_state = ListState::default();
        mode_list_state.select(Some(0)); // Select first option by default

        Self {
            state: GitHubAuthState::default(),
            token_area: None,
            repo_name_area: None,
            repo_location_area: None,
            visibility_area: None,
            mode_selection_areas: Vec::new(),
            mode_list_state,
            local_repo_path_area: None,
        }
    }

    /// Get icon set from config
    fn icons(&self, ctx: &RenderContext) -> crate::icons::Icons {
        crate::icons::Icons::from_config(ctx.config)
    }

    /// Get key display for an action
    fn get_key(&self, ctx: &RenderContext, action: crate::keymap::Action) -> String {
        ctx.config.keymap.get_key_display_for_action(action)
    }

    /// Get the current auth state.
    pub fn get_auth_state(&self) -> &GitHubAuthState {
        &self.state
    }

    /// Get mutable auth state.
    pub fn get_auth_state_mut(&mut self) -> &mut GitHubAuthState {
        &mut self.state
    }

    /// Reset the screen state to default.
    pub fn reset(&mut self) {
        self.state = GitHubAuthState::default();
    }

    /// Check if the async setup step needs processing.
    /// Returns true if there's a setup step in progress that needs ticking.
    pub fn needs_tick(&self) -> bool {
        matches!(self.state.step, GitHubAuthStep::SetupStep(_))
    }

    /// Check if mouse click is in a specific field area
    fn is_click_in_area(&self, area: Option<Rect>, x: u16, y: u16) -> bool {
        if let Some(rect) = area {
            x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
        } else {
            false
        }
    }

    // Debug methods (only available in debug builds)

    /// Cycle through sub-screens for preview/testing
    /// Note: Skips Processing/SetupStep states as they trigger async processing
    #[cfg(debug_assertions)]
    fn debug_cycle_screen(&mut self) -> ScreenAction {
        // Cycle through static screens only (no async processing states)
        // Choosing -> Local (new) -> GitHub (new) -> Local (configured) -> GitHub (configured) -> back
        match (&self.state.setup_mode, self.state.repo_already_configured) {
            // New user flow
            (SetupMode::Choosing, _) => {
                self.state.setup_mode = SetupMode::Local;
                self.state.step = GitHubAuthStep::Input;
                self.state.repo_already_configured = false;
                self.state.status_message = None;
            }
            (SetupMode::Local, false) => {
                self.state.setup_mode = SetupMode::GitHub;
                self.state.step = GitHubAuthStep::Input;
                self.state.repo_already_configured = false;
            }
            (SetupMode::GitHub, false) => {
                // Switch to "already configured" flow
                self.state.setup_mode = SetupMode::Local;
                self.state.step = GitHubAuthStep::Input;
                self.state.repo_already_configured = true;
                self.state.local_repo_path_input =
                    crate::utils::TextInput::with_text("~/.config/dotstate/storage");
            }
            // Configured user flow
            (SetupMode::Local, true) => {
                self.state.setup_mode = SetupMode::GitHub;
                self.state.step = GitHubAuthStep::Input;
                self.state.repo_already_configured = true;
                self.state.token_input =
                    crate::utils::TextInput::with_text("ghp_xxxxxxxxxxxxxxxxxxxx");
                self.state.repo_name_input = crate::utils::TextInput::with_text("my-dotfiles");
            }
            (SetupMode::GitHub, true) => {
                // Reset back to start
                self.state.setup_mode = SetupMode::Choosing;
                self.state.step = GitHubAuthStep::Input;
                self.state.repo_already_configured = false;
                self.state.status_message = None;
                self.state.token_input.clear();
                self.state.repo_name_input.clear();
                self.state.local_repo_path_input.clear();
            }
        }
        ScreenAction::Refresh
    }

    // Rendering methods

    fn render_token_field(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _ctx: &RenderContext,
    ) -> Result<()> {
        let is_focused = self.state.focused_field == GitHubAuthField::Token;

        // Disable token field if repo configured and not in edit mode
        let is_disabled = self.state.repo_already_configured && !self.state.is_editing_token;

        // Show masked token if repo is already configured and not editing
        let masked = self.state.repo_already_configured && !self.state.is_editing_token;

        // Store area for mouse support
        let input_block = Block::bordered();
        self.token_area = Some(input_block.inner(area));

        let widget = TextInputWidget::new(&self.state.token_input)
            .title("GitHub Token")
            .placeholder("ghp_...")
            .focused(is_focused && !is_disabled)
            .disabled(is_disabled)
            .masked(masked);

        frame.render_text_input_widget(widget, area);
        Ok(())
    }

    fn render_repo_name_field(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _ctx: &RenderContext,
    ) -> Result<()> {
        let is_focused = self.state.focused_field == GitHubAuthField::RepoName;
        let is_disabled = self.state.repo_already_configured;

        // Store area for mouse support
        let input_block = Block::bordered();
        self.repo_name_area = Some(input_block.inner(area));

        let widget = TextInputWidget::new(&self.state.repo_name_input)
            .title("Repository Name")
            .placeholder("dotstate-dotfiles")
            .focused(is_focused && !is_disabled)
            .disabled(is_disabled);

        frame.render_text_input_widget(widget, area);
        Ok(())
    }

    fn render_repo_location_field(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        _ctx: &RenderContext,
    ) -> Result<()> {
        let is_focused = self.state.focused_field == GitHubAuthField::RepoLocation;
        let is_disabled = self.state.repo_already_configured;

        // Store area for mouse support
        let input_block = Block::bordered();
        self.repo_location_area = Some(input_block.inner(area));

        let widget = TextInputWidget::new(&self.state.repo_location_input)
            .title("Local Path")
            .placeholder("~/.config/dotstate/storage")
            .focused(is_focused && !is_disabled)
            .disabled(is_disabled);

        frame.render_text_input_widget(widget, area);
        Ok(())
    }

    fn render_visibility_field(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let is_focused = self.state.focused_field == GitHubAuthField::IsPrivate;
        let is_disabled = self.state.repo_already_configured;
        let icons = self.icons(ctx);

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
            .title(" Repository Visibility ")
            .title_alignment(Alignment::Left);

        // Store area for mouse support
        self.visibility_area = Some(block.inner(area));

        let visibility_text = if self.state.is_private {
            format!(
                "[{}] Private    [{}] Public",
                icons.check(),
                icons.uncheck()
            )
        } else {
            format!(
                "[{}] Private    [{}] Public",
                icons.uncheck(),
                icons.check()
            )
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
    fn render_mode_selection(
        &mut self,
        frame: &mut Frame,
        header_chunk: Rect,
        content_chunk: Rect,
        footer_chunk: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let t = theme();

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
        let icons = self.icons(ctx);
        let options = vec![
            ListItem::new(vec![Line::from(vec![
                Span::styled(format!("{} ", icons.github()), Style::default()),
                Span::styled("Create repository for me (GitHub)", t.success_style()),
            ])]),
            ListItem::new(vec![Line::from(vec![
                Span::styled(format!("{} ", icons.folder()), Style::default()),
                Span::styled("Use my own repository", Style::default().fg(t.tertiary)),
            ])]),
        ];

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_style(focused_border_style())
            .border_type(theme().border_type(false))
            .title(format!(" {} Choose Setup Method ", icons.menu()))
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
            .select(Some(self.state.mode_selection_index));

        let list = List::new(options)
            .block(list_block)
            .highlight_style(t.highlight_style())
            .highlight_spacing(HighlightSpacing::Always)
            .highlight_symbol(LIST_HIGHLIGHT_SYMBOL);

        frame.render_stateful_widget(list, main_layout[0], &mut self.mode_list_state);

        // Right panel: Context-sensitive explanation
        let (title, explanation_lines) = if self.state.mode_selection_index == 0 {
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
                        Span::styled(
                            format!("{} ", icons.lightbulb()),
                            Style::default().fg(t.secondary),
                        ),
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
                        Span::styled(
                            format!("{} ", icons.lightbulb()),
                            Style::default().fg(t.secondary),
                        ),
                        Span::raw("Best for: Users who already have"),
                    ]),
                    Line::from("   a repo or use non-GitHub hosts."),
                ],
            )
        };

        let explanation_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(t.primary))
            .border_type(theme().border_type(false))
            .title(format!(" {} {} ", icons.lightbulb(), title))
            .title_style(t.title_style())
            .title_alignment(Alignment::Center)
            .padding(ratatui::widgets::Padding::new(1, 1, 1, 1));

        let explanation_para = Paragraph::new(explanation_lines)
            .block(explanation_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(explanation_para, main_layout[1]);

        // Footer
        let footer_text = format!(
            "{}/{} : Navigate | {}: Select | {}: Cancel",
            self.get_key(ctx, crate::keymap::Action::MoveUp),
            self.get_key(ctx, crate::keymap::Action::MoveDown),
            self.get_key(ctx, crate::keymap::Action::Confirm),
            self.get_key(ctx, crate::keymap::Action::Quit)
        );
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    /// Render the local repository setup screen
    fn render_local_setup(
        &mut self,
        frame: &mut Frame,
        header_chunk: Rect,
        content_chunk: Rect,
        footer_chunk: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let t = theme();
        let icons = self.icons(ctx);

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

        // Render setup form block
        let setup_block = Block::bordered()
            .title(" Local Repository Setup ")
            .border_style(Style::default().fg(t.primary))
            .border_type(theme().border_type(false));
        let setup_area = setup_block.inner(main_layout[0]);
        frame.render_widget(setup_block, main_layout[0]);

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
            .split(setup_area);

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
            .border_type(theme().border_type(false))
            .title(format!(" {} Setup Instructions ", icons.menu()))
            .title_style(Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD))
            .title_alignment(Alignment::Center);

        let instructions = Paragraph::new(instructions_lines)
            .block(instructions_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(instructions, left_layout[0]);

        // Path input field
        let is_disabled = self.state.repo_already_configured;

        // Store area for mouse support
        let input_block = Block::bordered();
        self.local_repo_path_area = Some(input_block.inner(left_layout[2]));

        let widget = TextInputWidget::new(&self.state.local_repo_path_input)
            .title("Local Repository Path")
            .placeholder("~/.config/dotstate/storage")
            .focused(self.state.input_focused && !is_disabled)
            .disabled(is_disabled);

        frame.render_text_input_widget(widget, left_layout[2]);

        // Right side: Help panel
        self.render_local_help_panel(frame, main_layout[1], ctx)?;

        // Footer
        let footer_text = if self.state.repo_already_configured {
            format!("{}: Back", self.get_key(ctx, crate::keymap::Action::Quit))
        } else {
            format!(
                "{}: Validate & Save | {}: Back to mode selection",
                self.get_key(ctx, crate::keymap::Action::Confirm),
                self.get_key(ctx, crate::keymap::Action::Quit)
            )
        };
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    /// Render help panel for local setup
    fn render_local_help_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let t = theme();
        let icons = self.icons(ctx);
        if let Some(status) = &self.state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .border_type(theme().border_type(false))
                .title(" Status ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, area);
        } else if let Some(error) = &self.state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .border_type(theme().border_type(false))
                .title(" Error ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.error));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(error_para, area);
        } else if self.state.repo_already_configured {
            // Show current configuration details when already configured
            let path = self.state.local_repo_path_input.text();
            let expanded_path = crate::git::expand_path(path);
            let validation = crate::git::validate_local_repo(&expanded_path);

            let remote_url = validation
                .remote_url
                .as_deref()
                .map(crate::git::redact_credentials)
                .unwrap_or_else(|| "unknown".to_string());

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
                    Span::styled(
                        format!("{} Configured", icons.check()),
                        Style::default().fg(t.success),
                    ),
                ]),
            ];

            let help_block = Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} Repository Info ", icons.menu()))
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success))
                .border_type(theme().border_type(false))
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
                .title(format!(" {} Help ", icons.lightbulb()))
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.primary))
                .border_type(theme().border_type(false))
                .padding(ratatui::widgets::Padding::new(1, 1, 0, 0));

            let help_para = Paragraph::new(help_lines)
                .block(help_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(help_para, area);
        }
        Ok(())
    }

    fn render_help_panel(&self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        let t = theme();
        let icons = self.icons(ctx);
        if let Some(status) = &self.state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .border_type(theme().border_type(false))
                .title(" Status ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, area);
        } else if let Some(error) = &self.state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .border_type(theme().border_type(false))
                .title(" Error ")
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

            let (title, help_lines) = match self.state.focused_field {
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
                .title(format!(" {} {} ", icons.info(), title))
                .border_type(theme().border_type(false))
                .padding(Padding::proportional(1))
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.primary));
            let help_para = Paragraph::new(help_lines.join("\n"))
                .block(help_block)
                .wrap(Wrap { trim: true })
                .scroll((self.state.help_scroll as u16, 0));
            frame.render_widget(help_para, area);
        }
        Ok(())
    }

    /// Render progress screen when processing GitHub setup
    fn render_progress_screen(
        &mut self,
        frame: &mut Frame,
        header_chunk: Rect,
        content_chunk: Rect,
        footer_chunk: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let t = theme();

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
            .border_type(theme().border_type(false))
            .title(" Progress ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(t.primary));

        // Status message with styling
        let status_text = if let Some(status) = &self.state.status_message {
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
        if let Some(error) = &self.state.error_message {
            let error_area = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(15), // Progress area
                    Constraint::Length(8),  // Error area
                ])
                .split(content_chunk)[1];

            let error_block = Block::default()
                .borders(Borders::ALL)
                .border_type(theme().border_type(false))
                .title(" Error ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.error));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(t.error));
            frame.render_widget(error_para, error_area);
        }

        // Footer
        let footer_text = if self.state.error_message.is_some() {
            "Press Esc to go back and fix the error".to_string()
        } else if self
            .state
            .status_message
            .as_ref()
            .map(|s| s.contains("✅"))
            .unwrap_or(false)
        {
            format!(
                "Press {} to continue",
                self.get_key(ctx, crate::keymap::Action::Confirm)
            )
        } else {
            "Please wait...".to_string()
        };
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    /// Render the GitHub setup form
    fn render_github_form(
        &mut self,
        frame: &mut Frame,
        header_chunk: Rect,
        content_chunk: Rect,
        footer_chunk: Rect,
        ctx: &RenderContext,
    ) -> Result<()> {
        let t = theme();

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

        // Render setup form block
        let setup_block = Block::bordered()
            .title(" Setup Form ")
            .border_style(Style::default().fg(t.primary))
            .border_type(theme().border_type(false));
        let setup_area = setup_block.inner(main_layout[0]);
        frame.render_widget(setup_block, main_layout[0]);

        // Left side: instructions and fields
        let left_layout = Layout::default()
            .direction(Direction::Vertical)
            .margin(1) // Reduced margin inside the block
            .constraints([
                Constraint::Length(4), // Instructions (height increased for block border?)
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Token input
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Repo name input
                if !self.state.repo_already_configured {
                    Constraint::Length(4)
                } else {
                    Constraint::Length(0)
                }, // Reminder message (only shown for new installs)
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Repo location input
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Visibility toggle
                Constraint::Min(0),    // Spacer
            ])
            .split(setup_area);

        // Instructions
        let instructions_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(t.tertiary))
            .border_type(theme().border_type(false));
        let instructions = Paragraph::new("Fill in the details below to set up your dotfiles repository. Use Tab to navigate between fields.")
            .block(instructions_block)
            .style(t.text_style())
            .alignment(Alignment::Center);
        frame.render_widget(instructions, left_layout[0]);

        // Render each field with spacers
        self.render_token_field(frame, left_layout[2], ctx)?;
        self.render_repo_name_field(frame, left_layout[4], ctx)?;

        // Show reminder message for new installs (when repo is not already configured)
        if !self.state.repo_already_configured {
            let reminder_text = "⚠️  If you already had a repo with a different name, make sure to enter it here, otherwise a new repo with this name will be created";
            let reminder = Paragraph::new(reminder_text)
                .style(t.warning_style())
                .block(Block::default().padding(Padding::proportional(1)))
                .wrap(Wrap { trim: false });
            frame.render_widget(reminder, left_layout[5]);
        }

        self.render_repo_location_field(frame, left_layout[7], ctx)?;
        self.render_visibility_field(frame, left_layout[9], ctx)?;

        // Right side: Context-sensitive help
        self.render_help_panel(frame, main_layout[1], ctx)?;

        // Footer
        let footer_text = if self.state.repo_already_configured {
            if self.state.is_editing_token {
                format!(
                    "{}: Save Token | {}: Cancel",
                    self.get_key(ctx, crate::keymap::Action::Confirm),
                    self.get_key(ctx, crate::keymap::Action::Quit)
                )
            } else {
                format!(
                    "{}: Update Token | {}: Back",
                    self.get_key(ctx, crate::keymap::Action::Edit),
                    self.get_key(ctx, crate::keymap::Action::Quit)
                )
            }
        } else {
            format!(
                "{}: Next Field | {}: Previous | {}: Toggle | {}: Save & Create | {}: Cancel",
                self.get_key(ctx, crate::keymap::Action::NextTab),
                self.get_key(ctx, crate::keymap::Action::PrevTab),
                self.get_key(ctx, crate::keymap::Action::ToggleSelect),
                self.get_key(ctx, crate::keymap::Action::Confirm),
                self.get_key(ctx, crate::keymap::Action::Cancel)
            )
        };
        let _ = Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    // Event handling methods

    /// Handle mode selection (Choosing mode).
    fn handle_mode_selection(
        &mut self,
        action: Option<crate::keymap::Action>,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    if self.state.mode_selection_index > 0 {
                        self.state.mode_selection_index -= 1;
                    }
                }
                Action::MoveDown => {
                    if self.state.mode_selection_index < 1 {
                        self.state.mode_selection_index += 1;
                    }
                }
                Action::Confirm => {
                    if self.state.mode_selection_index == 0 {
                        self.state.setup_mode = SetupMode::GitHub;
                    } else {
                        self.state.setup_mode = SetupMode::Local;
                        self.state.input_focused = true;
                    }
                }
                Action::Cancel | Action::Quit => {
                    self.reset();
                    return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
                }
                _ => {}
            }
        }
        Ok(ScreenAction::None)
    }

    /// Handle local setup input.
    fn handle_local_setup_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        ctx: &ScreenContext,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        let action = ctx.config.keymap.get_action(key.code, key.modifiers);
        self.state.error_message = None;

        // If already configured, only allow Esc/Cancel to go back
        if self.state.repo_already_configured {
            if let Some(Action::Cancel | Action::Quit) = action {
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
            return Ok(ScreenAction::None);
        }

        // Check for Action::Confirm or Action::Save to validate and save
        if matches!(action, Some(Action::Confirm | Action::Save)) {
            let path_str = self.state.local_repo_path_input.text_trimmed();
            if path_str.is_empty() {
                self.state.error_message = Some("Please enter a repository path".to_string());
                return Ok(ScreenAction::None);
            }

            let expanded_path = crate::git::expand_path(path_str);
            let validation = crate::git::validate_local_repo(&expanded_path);

            if !validation.is_valid {
                self.state.error_message = validation.error_message;
                return Ok(ScreenAction::None);
            }

            // Validation passed - signal to app to save config
            let safe_url = validation
                .remote_url
                .as_deref()
                .map(crate::git::redact_credentials)
                .unwrap_or_else(|| "unknown".to_string());
            self.state.status_message = Some(format!(
                "✅ Valid repository found!\n\nRemote: {}\n\nSaving configuration...",
                safe_url
            ));

            // Load profiles from the repository
            let profiles = crate::utils::ProfileManifest::load_or_backfill(&expanded_path)
                .map(|m| m.profiles.iter().map(|p| p.name.clone()).collect())
                .unwrap_or_default();

            return Ok(ScreenAction::SaveLocalRepoConfig {
                repo_path: expanded_path,
                profiles,
            });
        }

        if let Some(Action::Cancel | Action::Quit) = action {
            self.state.setup_mode = SetupMode::Choosing;
            self.state.error_message = None;
            self.state.status_message = None;
            return Ok(ScreenAction::None);
        }

        // Handle text input
        match key.code {
            KeyCode::Esc => {
                self.state.setup_mode = SetupMode::Choosing;
                self.state.error_message = None;
                self.state.status_message = None;
            }
            KeyCode::Char(c) => {
                self.state.local_repo_path_input.insert_char(c);
            }
            KeyCode::Backspace => {
                self.state.local_repo_path_input.backspace();
            }
            KeyCode::Delete => {
                self.state.local_repo_path_input.delete();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.state.local_repo_path_input.handle_key(key.code);
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    /// Handle GitHub setup input (Input step).
    fn handle_github_input(
        &mut self,
        key: crossterm::event::KeyEvent,
        ctx: &ScreenContext,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        let action = ctx.config.keymap.get_action(key.code, key.modifiers);

        // Handle "Update Token" action if repo is configured
        if self.state.repo_already_configured && !self.state.is_editing_token {
            if let Some(Action::Edit) = action {
                self.state.is_editing_token = true;
                self.state.token_input.clear();
                self.state.focused_field = GitHubAuthField::Token;
                return Ok(ScreenAction::None);
            }
            if let Some(Action::Cancel | Action::Quit) = action {
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
        }

        // Check for Save/Confirm action
        if matches!(action, Some(Action::Save) | Some(Action::Confirm)) {
            if self.state.repo_already_configured && self.state.is_editing_token {
                // Just update the token
                let token = self.state.token_input.text_trimmed().to_string();
                return Ok(ScreenAction::UpdateGitHubToken { token });
            } else if !self.state.repo_already_configured {
                // Full setup - validate and start setup
                let token = self.state.token_input.text_trimmed().to_string();
                let repo_name = self.state.repo_name_input.text_trimmed().to_string();

                // Validate token format
                if !token.starts_with("ghp_") {
                    let actual_start = if token.len() >= 4 {
                        &token[..4]
                    } else {
                        "too short"
                    };
                    self.state.error_message = Some(format!(
                        "❌ Invalid token format: Must start with 'ghp_' but starts with '{}'.\n\
                         See help for more details.",
                        actual_start
                    ));
                    return Ok(ScreenAction::None);
                }

                if token.len() < 40 {
                    self.state.error_message = Some(format!(
                        "❌ Token appears incomplete: {} characters (expected 40+).",
                        token.len()
                    ));
                    return Ok(ScreenAction::None);
                }

                // Return action to start the setup
                return Ok(ScreenAction::StartGitHubSetup {
                    token,
                    repo_name,
                    is_private: self.state.is_private,
                });
            }
            return Ok(ScreenAction::None);
        }

        // For plain character keys, ALWAYS insert the character first
        // This ensures vim bindings like h/l don't interfere with typing
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                match self.state.focused_field {
                    GitHubAuthField::Token => self.state.token_input.insert_char(c),
                    GitHubAuthField::RepoName => self.state.repo_name_input.insert_char(c),
                    GitHubAuthField::RepoLocation => self.state.repo_location_input.insert_char(c),
                    GitHubAuthField::IsPrivate => {}
                }
                return Ok(ScreenAction::None);
            }
        }

        // Handle navigation and editing actions for non-character keys
        if let Some(act) = action {
            match act {
                Action::Cancel | Action::Quit => {
                    if !self.state.repo_already_configured {
                        self.state.setup_mode = SetupMode::Choosing;
                        self.state.error_message = None;
                        return Ok(ScreenAction::None);
                    }
                    self.reset();
                    return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
                }
                Action::NextTab if !self.state.repo_already_configured => {
                    self.state.focused_field = match self.state.focused_field {
                        GitHubAuthField::Token => GitHubAuthField::RepoName,
                        GitHubAuthField::RepoName => GitHubAuthField::RepoLocation,
                        GitHubAuthField::RepoLocation => GitHubAuthField::IsPrivate,
                        GitHubAuthField::IsPrivate => GitHubAuthField::Token,
                    };
                    return Ok(ScreenAction::None);
                }
                Action::PrevTab if !self.state.repo_already_configured => {
                    self.state.focused_field = match self.state.focused_field {
                        GitHubAuthField::Token => GitHubAuthField::IsPrivate,
                        GitHubAuthField::RepoName => GitHubAuthField::Token,
                        GitHubAuthField::RepoLocation => GitHubAuthField::RepoName,
                        GitHubAuthField::IsPrivate => GitHubAuthField::RepoLocation,
                    };
                    return Ok(ScreenAction::None);
                }
                Action::MoveLeft => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_left(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_left(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_left(),
                        GitHubAuthField::IsPrivate if !self.state.repo_already_configured => {
                            self.state.is_private = !self.state.is_private;
                        }
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::MoveRight => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_right(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_right(),
                        GitHubAuthField::RepoLocation => {
                            self.state.repo_location_input.move_right()
                        }
                        GitHubAuthField::IsPrivate if !self.state.repo_already_configured => {
                            self.state.is_private = !self.state.is_private;
                        }
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::Home => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_home(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_home(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_home(),
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::End => {
                    match self.state.focused_field {
                        GitHubAuthField::Token => self.state.token_input.move_end(),
                        GitHubAuthField::RepoName => self.state.repo_name_input.move_end(),
                        GitHubAuthField::RepoLocation => self.state.repo_location_input.move_end(),
                        GitHubAuthField::IsPrivate => {}
                    }
                    return Ok(ScreenAction::None);
                }
                Action::Backspace => {
                    self.handle_backspace();
                    return Ok(ScreenAction::None);
                }
                Action::DeleteChar => {
                    self.handle_delete();
                    return Ok(ScreenAction::None);
                }
                Action::ToggleSelect => {
                    if self.state.focused_field == GitHubAuthField::IsPrivate
                        && !self.state.repo_already_configured
                    {
                        self.state.is_private = !self.state.is_private;
                    }
                    return Ok(ScreenAction::None);
                }
                _ => {}
            }
        }

        // Handle character input
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                self.handle_char_input(c);
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle Processing step events.
    fn handle_processing_input(
        &mut self,
        action: Option<crate::keymap::Action>,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        match action {
            Some(Action::Confirm) => {
                // Processing is done, navigate based on profiles
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
            Some(Action::Cancel | Action::Quit) => {
                self.reset();
                return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }

    /// Handle SetupStep events (during async operations).
    fn handle_setup_step_input(
        &mut self,
        action: Option<crate::keymap::Action>,
    ) -> Result<ScreenAction> {
        use crate::keymap::Action;

        if let Some(Action::Cancel | Action::Quit) = action {
            self.reset();
            return Ok(ScreenAction::Navigate(crate::ui::Screen::MainMenu));
        }
        Ok(ScreenAction::None)
    }

    /// Handle mouse events
    fn handle_mouse_event(&mut self, event: Event) -> Result<ScreenAction> {
        if let Event::Mouse(mouse) = event {
            if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                let x = mouse.column;
                let y = mouse.row;

                // Handle mode selection screen
                if matches!(self.state.setup_mode, SetupMode::Choosing) {
                    for (rect, mode_index) in &self.mode_selection_areas {
                        if x >= rect.x
                            && x < rect.x + rect.width
                            && y >= rect.y
                            && y < rect.y + rect.height
                        {
                            self.state.mode_selection_index = *mode_index;
                            self.mode_list_state.select(Some(*mode_index));
                            return Ok(ScreenAction::Refresh);
                        }
                    }
                    return Ok(ScreenAction::None);
                }

                // Handle local setup screen
                if matches!(self.state.setup_mode, SetupMode::Local) {
                    if self.is_click_in_area(self.local_repo_path_area, x, y) {
                        if !self.state.repo_already_configured {
                            self.state.input_focused = true;
                            return Ok(ScreenAction::Refresh);
                        }
                    } else {
                        self.state.input_focused = false;
                        return Ok(ScreenAction::Refresh);
                    }
                    return Ok(ScreenAction::None);
                }

                // Handle GitHub setup screen
                if self.is_click_in_area(self.token_area, x, y) {
                    if !self.state.repo_already_configured || self.state.is_editing_token {
                        self.state.focused_field = GitHubAuthField::Token;
                        self.state.input_focused = true;
                        return Ok(ScreenAction::Refresh);
                    }
                } else if self.is_click_in_area(self.repo_name_area, x, y) {
                    if !self.state.repo_already_configured {
                        self.state.focused_field = GitHubAuthField::RepoName;
                        self.state.input_focused = true;
                        return Ok(ScreenAction::Refresh);
                    }
                } else if self.is_click_in_area(self.repo_location_area, x, y) {
                    if !self.state.repo_already_configured {
                        self.state.focused_field = GitHubAuthField::RepoLocation;
                        self.state.input_focused = true;
                        return Ok(ScreenAction::Refresh);
                    }
                } else if self.is_click_in_area(self.visibility_area, x, y) {
                    if !self.state.repo_already_configured {
                        self.state.focused_field = GitHubAuthField::IsPrivate;
                        self.state.input_focused = true;
                        self.state.is_private = !self.state.is_private;
                        return Ok(ScreenAction::Refresh);
                    }
                } else {
                    // Click outside - unfocus
                    self.state.input_focused = false;
                    return Ok(ScreenAction::Refresh);
                }
            }
        }
        Ok(ScreenAction::None)
    }

    // Helper methods

    fn handle_backspace(&mut self) {
        match self.state.focused_field {
            GitHubAuthField::Token => self.state.token_input.backspace(),
            GitHubAuthField::RepoName => self.state.repo_name_input.backspace(),
            GitHubAuthField::RepoLocation => self.state.repo_location_input.backspace(),
            GitHubAuthField::IsPrivate => {}
        }
    }

    fn handle_delete(&mut self) {
        match self.state.focused_field {
            GitHubAuthField::Token => self.state.token_input.delete(),
            GitHubAuthField::RepoName => self.state.repo_name_input.delete(),
            GitHubAuthField::RepoLocation => self.state.repo_location_input.delete(),
            GitHubAuthField::IsPrivate => {}
        }
    }

    fn handle_char_input(&mut self, c: char) {
        let can_edit_token = !self.state.repo_already_configured || self.state.is_editing_token;

        match self.state.focused_field {
            GitHubAuthField::Token if can_edit_token => {
                self.state.token_input.insert_char(c);
            }
            GitHubAuthField::RepoName if !self.state.repo_already_configured => {
                self.state.repo_name_input.insert_char(c);
            }
            GitHubAuthField::RepoLocation if !self.state.repo_already_configured => {
                self.state.repo_location_input.insert_char(c);
            }
            _ => {}
        }
    }
}

impl Default for GitHubAuthScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for GitHubAuthScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Clear the entire area first
        frame.render_widget(Clear, area);

        // Background - use Reset to inherit terminal's native background
        let t = theme();
        let background = Block::default().style(t.background_style());
        frame.render_widget(background, area);

        // Standard layout for all sub-screens
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 3);

        // If processing or in setup, show progress screen instead of input form
        if matches!(
            self.state.step,
            GitHubAuthStep::Processing | GitHubAuthStep::SetupStep(_)
        ) {
            return self.render_progress_screen(
                frame,
                header_chunk,
                content_chunk,
                footer_chunk,
                ctx,
            );
        }

        // Check setup mode and render appropriate screen
        match self.state.setup_mode {
            SetupMode::Choosing => {
                self.render_mode_selection(frame, header_chunk, content_chunk, footer_chunk, ctx)
            }
            SetupMode::Local => {
                self.render_local_setup(frame, header_chunk, content_chunk, footer_chunk, ctx)
            }
            SetupMode::GitHub => {
                self.render_github_form(frame, header_chunk, content_chunk, footer_chunk, ctx)
            }
        }
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        self.state.error_message = None;

        // Handle keyboard events
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ScreenAction::None);
            }

            // Debug mode: F12 cycles through sub-screens (debug builds only)
            #[cfg(debug_assertions)]
            if key.code == KeyCode::F(12) {
                return Ok(self.debug_cycle_screen());
            }

            let action = ctx.config.keymap.get_action(key.code, key.modifiers);

            // Handle based on current mode and step
            match self.state.setup_mode {
                SetupMode::Choosing => {
                    return self.handle_mode_selection(action);
                }
                SetupMode::Local => {
                    return self.handle_local_setup_input(key, ctx);
                }
                SetupMode::GitHub => {
                    // Handle based on step
                    match self.state.step {
                        GitHubAuthStep::Input => {
                            return self.handle_github_input(key, ctx);
                        }
                        GitHubAuthStep::Processing => {
                            return self.handle_processing_input(action);
                        }
                        GitHubAuthStep::SetupStep(_) => {
                            return self.handle_setup_step_input(action);
                        }
                    }
                }
            }
        }

        // Handle mouse events
        if matches!(event, Event::Mouse(_)) {
            return self.handle_mouse_event(event);
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        self.state.input_focused
    }

    fn on_enter(&mut self, _ctx: &ScreenContext) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_auth_screen_creation() {
        let screen = GitHubAuthScreen::new();
        assert!(screen.is_input_focused());
    }

    #[test]
    fn test_get_auth_state() {
        let screen = GitHubAuthScreen::new();
        let state = screen.get_auth_state();
        assert_eq!(state.setup_mode, SetupMode::Choosing);
    }

    #[test]
    fn test_reset() {
        let mut screen = GitHubAuthScreen::new();
        screen.state.token_input = crate::utils::TextInput::with_text("test_token");
        screen.state.setup_mode = SetupMode::GitHub;
        screen.reset();
        assert!(screen.state.token_input.is_empty());
        assert_eq!(screen.state.setup_mode, SetupMode::Choosing);
    }

    #[test]
    fn test_needs_tick() {
        let mut screen = GitHubAuthScreen::new();
        assert!(!screen.needs_tick());

        screen.state.step = GitHubAuthStep::SetupStep(crate::ui::GitHubSetupStep::Connecting);
        assert!(screen.needs_tick());
    }
}
