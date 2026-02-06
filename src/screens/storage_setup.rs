//! Storage setup screen for configuring dotfiles storage.
//!
//! Provides a two-pane interface matching the settings screen pattern:
//! - Left: Storage method selection (GitHub or Local)
//! - Right: Form fields and context-sensitive help

use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::icons::Icons;
use crate::keymap::Action;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::theme;
use crate::ui::{GitHubSetupData, GitHubSetupStep};
use crate::utils::{
    create_split_layout, create_standard_layout, focused_border_style, unfocused_border_style,
    MouseRegions, TextInput,
};
use crate::widgets::{Menu, MenuItem, MenuState, TextInputWidget, TextInputWidgetExt};
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, StatefulWidget, Wrap};
use ratatui::Frame;

/// Focus within the storage setup screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageSetupFocus {
    #[default]
    MethodList, // Left pane - selecting storage method
    Form, // Right pane - editing form fields
}

/// Selected storage method
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageMethod {
    #[default]
    GitHub,
    Local,
}

impl StorageMethod {
    fn all() -> Vec<StorageMethod> {
        vec![StorageMethod::GitHub, StorageMethod::Local]
    }

    #[allow(dead_code)] // Utility method for potential future use
    fn name(&self) -> &'static str {
        match self {
            StorageMethod::GitHub => "GitHub Repository",
            StorageMethod::Local => "Local Repository",
        }
    }

    fn index(&self) -> usize {
        match self {
            StorageMethod::GitHub => 0,
            StorageMethod::Local => 1,
        }
    }

    fn from_index(index: usize) -> Option<StorageMethod> {
        match index {
            0 => Some(StorageMethod::GitHub),
            1 => Some(StorageMethod::Local),
            _ => None,
        }
    }
}

/// GitHub form fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitHubField {
    #[default]
    Token,
    RepoName,
    RepoPath,
    Visibility,
}

impl GitHubField {
    #[allow(dead_code)] // Utility method for potential future use
    fn all() -> Vec<GitHubField> {
        vec![
            GitHubField::Token,
            GitHubField::RepoName,
            GitHubField::RepoPath,
            GitHubField::Visibility,
        ]
    }

    fn next(&self) -> GitHubField {
        match self {
            GitHubField::Token => GitHubField::RepoName,
            GitHubField::RepoName => GitHubField::RepoPath,
            GitHubField::RepoPath => GitHubField::Visibility,
            GitHubField::Visibility => GitHubField::Token,
        }
    }

    fn prev(&self) -> GitHubField {
        match self {
            GitHubField::Token => GitHubField::Visibility,
            GitHubField::RepoName => GitHubField::Token,
            GitHubField::RepoPath => GitHubField::RepoName,
            GitHubField::Visibility => GitHubField::RepoPath,
        }
    }
}

/// Current step in the storage setup process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageSetupStep {
    #[default]
    Input,
    /// GitHub setup state machine in progress
    Processing(GitHubSetupStep),
}

/// Storage setup screen state
#[derive(Debug)]
pub struct StorageSetupState {
    // Focus and selection
    pub focus: StorageSetupFocus,
    pub method: StorageMethod,
    pub menu_state: MenuState,

    // GitHub form fields
    pub token_input: TextInput,
    pub repo_name_input: TextInput,
    pub repo_path_input: TextInput,
    pub is_private: bool,
    pub github_field: GitHubField,

    // Local form field
    pub local_path_input: TextInput,

    // Status
    pub status_message: Option<String>,
    pub error_message: Option<String>,

    // Configuration state
    pub is_reconfiguring: bool,
    pub is_editing_token: bool,

    // Setup processing state
    pub step: StorageSetupStep,
    pub setup_data: Option<GitHubSetupData>,
}

impl Default for StorageSetupState {
    fn default() -> Self {
        let mut menu_state = MenuState::new();
        menu_state.select(Some(0));

        Self {
            focus: StorageSetupFocus::MethodList,
            method: StorageMethod::GitHub,
            menu_state,
            token_input: TextInput::default(),
            repo_name_input: TextInput::with_text(crate::config::default_repo_name()),
            repo_path_input: TextInput::with_text("~/.config/dotstate/storage"),
            is_private: true,
            github_field: GitHubField::Token,
            local_path_input: TextInput::with_text("~/.config/dotstate/storage"),
            status_message: None,
            error_message: None,
            is_reconfiguring: false,
            is_editing_token: false,
            step: StorageSetupStep::Input,
            setup_data: None,
        }
    }
}

/// Storage setup screen controller
pub struct StorageSetupScreen {
    state: StorageSetupState,
    /// Clickable regions for method menu items
    method_regions: MouseRegions<usize>,
    /// Area of the method list pane (for scroll hit-testing)
    method_pane_area: Option<Rect>,
    /// Clickable regions for form fields
    form_field_regions: MouseRegions<usize>,
}

impl Default for StorageSetupScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageSetupScreen {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: StorageSetupState::default(),
            method_regions: MouseRegions::new(),
            method_pane_area: None,
            form_field_regions: MouseRegions::new(),
        }
    }

    /// Reset the screen state
    pub fn reset(&mut self) {
        self.state = StorageSetupState::default();
    }

    /// Check if the async setup step needs processing.
    /// Returns true if there's a setup step in progress that needs ticking.
    #[must_use]
    pub fn needs_tick(&self) -> bool {
        matches!(self.state.step, StorageSetupStep::Processing(_))
    }

    /// Get the current state (read-only).
    #[must_use]
    pub fn get_state(&self) -> &StorageSetupState {
        &self.state
    }

    /// Get mutable state.
    pub fn get_state_mut(&mut self) -> &mut StorageSetupState {
        &mut self.state
    }

    /// Get icons from config
    fn icons(&self, ctx: &RenderContext) -> Icons {
        Icons::from_config(ctx.config)
    }

    /// Get key display for an action
    fn key_display(&self, ctx: &RenderContext, action: Action) -> String {
        ctx.config.keymap.get_key_display_for_action(action)
    }

    /// Render the method selection menu (left pane)
    fn render_method_list(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let t = theme();
        let icons = self.icons(ctx);
        let is_focused = self.state.focus == StorageSetupFocus::MethodList;

        // Store pane area for mouse
        self.method_pane_area = Some(area);

        // Build menu items
        let items: Vec<MenuItem> = StorageMethod::all()
            .iter()
            .map(|method| {
                let (icon, text, color) = match method {
                    StorageMethod::GitHub => (icons.github(), "GitHub Repository", t.success),
                    StorageMethod::Local => (icons.folder(), "Local Repository", t.tertiary),
                };
                MenuItem::new(icon, text, color)
            })
            .collect();

        // Create bordered container
        let border_style = if is_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Storage Method ")
            .title_alignment(Alignment::Center)
            .border_type(t.border_type(is_focused))
            .border_style(border_style)
            .style(t.background_style());

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Render menu and store clickable areas
        let menu = Menu::new(items);
        self.method_regions.clear();
        for (rect, idx) in menu.clickable_areas(inner) {
            self.method_regions.add(rect, idx);
        }
        StatefulWidget::render(menu, inner, frame.buffer_mut(), &mut self.state.menu_state);
    }

    /// Render the right pane (form or explanation based on focus)
    fn render_form_pane(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let is_focused = self.state.focus == StorageSetupFocus::Form;

        // Split into form (top) and help (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        // Clear form field regions before rendering
        self.form_field_regions.clear();

        match self.state.method {
            StorageMethod::GitHub => {
                self.render_github_form(frame, chunks[0], ctx, is_focused);
            }
            StorageMethod::Local => {
                self.render_local_form(frame, chunks[0], ctx, is_focused);
            }
        }

        // Render help panel
        self.render_help_panel(frame, chunks[1], ctx);
    }

    /// Render GitHub form fields
    fn render_github_form(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
        is_pane_focused: bool,
    ) {
        let t = theme();
        let icons = self.icons(ctx);

        let border_style = if is_pane_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let form_block = Block::default()
            .borders(Borders::ALL)
            .title(" GitHub Setup ")
            .title_alignment(Alignment::Center)
            .border_type(t.border_type(is_pane_focused))
            .border_style(border_style)
            .padding(Padding::new(1, 1, 1, 1))
            .style(t.background_style());

        let inner = form_block.inner(area);
        frame.render_widget(form_block, area);

        // Form layout
        let fields = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Token
                Constraint::Length(3), // Repo name
                Constraint::Length(3), // Repo path
                Constraint::Length(3), // Visibility
                Constraint::Min(0),    // Spacer
            ])
            .split(inner);

        // Token field (field index 0)
        let token_focused = is_pane_focused && self.state.github_field == GitHubField::Token;
        let token_disabled = self.state.is_reconfiguring && !self.state.is_editing_token;
        let token_widget = TextInputWidget::new(&self.state.token_input)
            .title("GitHub Token")
            .placeholder("ghp_...")
            .focused(token_focused)
            .disabled(token_disabled)
            .masked(token_disabled);
        frame.render_text_input_widget(token_widget, fields[0]);
        self.form_field_regions.add(fields[0], 0);

        // Repo name field (field index 1)
        let repo_name_focused = is_pane_focused && self.state.github_field == GitHubField::RepoName;
        let repo_name_widget = TextInputWidget::new(&self.state.repo_name_input)
            .title("Repository Name")
            .placeholder("dotstate-dotfiles")
            .focused(repo_name_focused)
            .disabled(self.state.is_reconfiguring);
        frame.render_text_input_widget(repo_name_widget, fields[1]);
        self.form_field_regions.add(fields[1], 1);

        // Repo path field (field index 2)
        let repo_path_focused = is_pane_focused && self.state.github_field == GitHubField::RepoPath;
        let repo_path_widget = TextInputWidget::new(&self.state.repo_path_input)
            .title("Local Path")
            .placeholder("~/.config/dotstate/storage")
            .focused(repo_path_focused)
            .disabled(self.state.is_reconfiguring);
        frame.render_text_input_widget(repo_path_widget, fields[2]);
        self.form_field_regions.add(fields[2], 2);

        // Visibility toggle
        let vis_focused = is_pane_focused && self.state.github_field == GitHubField::Visibility;
        let vis_border = if vis_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let vis_text = if self.state.is_private {
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

        let vis_block = Block::default()
            .borders(Borders::ALL)
            .border_style(vis_border)
            .title(" Visibility ");

        let vis_para =
            Paragraph::new(vis_text)
                .block(vis_block)
                .style(if self.state.is_reconfiguring {
                    t.muted_style()
                } else {
                    t.text_style()
                });
        frame.render_widget(vis_para, fields[3]);
        self.form_field_regions.add(fields[3], 3);
    }

    /// Render Local form fields
    fn render_local_form(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ctx: &RenderContext,
        is_pane_focused: bool,
    ) {
        let t = theme();
        let icons = self.icons(ctx);

        let border_style = if is_pane_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let form_block = Block::default()
            .borders(Borders::ALL)
            .title(" Local Repository Setup ")
            .title_alignment(Alignment::Center)
            .border_type(t.border_type(is_pane_focused))
            .border_style(border_style)
            .padding(Padding::new(1, 1, 1, 1))
            .style(t.background_style());

        let inner = form_block.inner(area);
        frame.render_widget(form_block, area);

        // Form layout
        let fields = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5), // Instructions
                Constraint::Length(1), // Spacer
                Constraint::Length(3), // Path input
                Constraint::Min(0),    // Spacer
            ])
            .split(inner);

        // Instructions
        let instructions = vec![
            Line::from(vec![
                Span::styled(
                    format!("{} ", icons.lightbulb()),
                    Style::default().fg(t.secondary),
                ),
                Span::styled("Setup your own git repository:", t.text_style()),
            ]),
            Line::from(vec![
                Span::styled("  1. ", Style::default().fg(t.text_emphasis)),
                Span::raw("Clone a repo to your machine"),
            ]),
            Line::from(vec![
                Span::styled("  2. ", Style::default().fg(t.text_emphasis)),
                Span::raw("Enter the path below"),
            ]),
        ];
        let instructions_para = Paragraph::new(instructions).wrap(Wrap { trim: true });
        frame.render_widget(instructions_para, fields[0]);

        // Path input (field index 0 for local)
        let path_widget = TextInputWidget::new(&self.state.local_path_input)
            .title("Repository Path")
            .placeholder("~/.config/dotstate/storage")
            .focused(is_pane_focused)
            .disabled(self.state.is_reconfiguring);
        frame.render_text_input_widget(path_widget, fields[2]);
        self.form_field_regions.add(fields[2], 0);
    }

    /// Render context-sensitive help panel
    fn render_help_panel(&self, frame: &mut Frame, area: Rect, ctx: &RenderContext) {
        let t = theme();
        let icons = self.icons(ctx);

        // Show error if any
        if let Some(error) = &self.state.error_message {
            let error_block = Block::default()
                .borders(Borders::ALL)
                .border_type(t.border_type(false))
                .title(" Error ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.error))
                .padding(Padding::proportional(1));
            let error_para = Paragraph::new(error.as_str())
                .block(error_block)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(t.error));
            frame.render_widget(error_para, area);
            return;
        }

        // Show status if any
        if let Some(status) = &self.state.status_message {
            let status_block = Block::default()
                .borders(Borders::ALL)
                .border_type(t.border_type(false))
                .title(" Status ")
                .title_alignment(Alignment::Center)
                .border_style(Style::default().fg(t.success))
                .padding(Padding::proportional(1));
            let status_para = Paragraph::new(status.as_str())
                .block(status_block)
                .wrap(Wrap { trim: true });
            frame.render_widget(status_para, area);
            return;
        }

        // Context-sensitive help
        let help_text = match self.state.focus {
            StorageSetupFocus::MethodList => self.get_method_help(),
            StorageSetupFocus::Form => match self.state.method {
                StorageMethod::GitHub => self.get_github_field_help(),
                StorageMethod::Local => self.get_local_help(),
            },
        };

        let help_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {} Help ", icons.lightbulb()))
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(t.primary))
            .border_type(t.border_type(false))
            .padding(Padding::proportional(1))
            .style(t.background_style());

        let help_para = Paragraph::new(help_text)
            .block(help_block)
            .wrap(Wrap { trim: true });
        frame.render_widget(help_para, area);
    }

    /// Get help text for method selection
    fn get_method_help(&self) -> Text<'static> {
        let t = theme();

        match self.state.method {
            StorageMethod::GitHub => Text::from(vec![
                Line::from(Span::styled(
                    "GitHub Repository",
                    Style::default().fg(t.success).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("DotState will create a private repo"),
                Line::from("and set up syncing automatically."),
                Line::from(""),
                Line::from(Span::styled(
                    "You'll need a token with:",
                    Style::default().fg(t.primary),
                )),
                Line::from(vec![
                    Span::styled("  • ", t.muted_style()),
                    Span::styled("Administration", Style::default().fg(t.text_emphasis)),
                    Span::raw(" read & write"),
                ]),
                Line::from(vec![
                    Span::styled("  • ", t.muted_style()),
                    Span::styled("Contents", Style::default().fg(t.text_emphasis)),
                    Span::raw(" read & write"),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Create a token:",
                    Style::default().fg(t.primary),
                )),
                Line::from(Span::styled(
                    "  github.com/settings/tokens",
                    Style::default().fg(t.text_muted),
                )),
                Line::from(Span::styled(
                    "  (classic: select 'repo' scope)",
                    Style::default().fg(t.text_muted),
                )),
            ]),
            StorageMethod::Local => Text::from(vec![
                Line::from(Span::styled(
                    "Local Repository",
                    Style::default().fg(t.tertiary).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("Use your own git repository:"),
                Line::from(vec![
                    Span::styled("  • ", t.muted_style()),
                    Span::raw("GitHub, GitLab, Bitbucket"),
                ]),
                Line::from(vec![
                    Span::styled("  • ", t.muted_style()),
                    Span::raw("Self-hosted git servers"),
                ]),
                Line::from(""),
                Line::from(Span::styled("Requires:", Style::default().fg(t.primary))),
                Line::from("  • Pre-cloned git repository"),
                Line::from("  • Push access configured"),
            ]),
        }
    }

    /// Get help text for GitHub form fields
    fn get_github_field_help(&self) -> Text<'static> {
        let t = theme();

        match self.state.github_field {
            GitHubField::Token => {
                if self.state.is_reconfiguring && !self.state.is_editing_token {
                    // Reconfiguring but not editing - show edit prompt
                    Text::from(vec![
                        Line::from(Span::styled("GitHub Token", t.title_style())),
                        Line::from(""),
                        Line::from("Token is configured and masked."),
                        Line::from(""),
                        Line::from(Span::styled(
                            "Press Enter to update your token.",
                            Style::default().fg(t.primary),
                        )),
                    ])
                } else if self.state.is_editing_token {
                    // Editing token
                    Text::from(vec![
                        Line::from(Span::styled("Update Token", t.title_style())),
                        Line::from(""),
                        Line::from("Enter your new Personal Access Token."),
                        Line::from(""),
                        Line::from(Span::styled(
                            "Press Enter to save, Esc to cancel.",
                            Style::default().fg(t.primary),
                        )),
                    ])
                } else {
                    // Initial setup
                    Text::from(vec![
                        Line::from(Span::styled("GitHub Token", t.title_style())),
                        Line::from(""),
                        Line::from("Personal Access Token for authentication."),
                        Line::from(""),
                        Line::from(Span::styled(
                            "Classic token (ghp_):",
                            Style::default().fg(t.primary),
                        )),
                        Line::from("  github.com/settings/tokens"),
                        Line::from("  Select 'repo' scope"),
                        Line::from(""),
                        Line::from(Span::styled(
                            "Fine-grained token (github_pat_):",
                            Style::default().fg(t.primary),
                        )),
                        Line::from("  github.com/settings/personal-access-tokens"),
                        Line::from(""),
                        Line::from(Span::styled(
                            "Required permissions:",
                            Style::default().fg(t.text_emphasis),
                        )),
                        Line::from("  Administration: Read & write"),
                        Line::from("    (to create dotstate-storage repo)"),
                        Line::from("  Contents: Read & write"),
                        Line::from("    (to sync your dotfiles)"),
                        Line::from(""),
                        Line::from(Span::styled("Note:", Style::default().fg(t.text_muted))),
                        Line::from(Span::styled(
                            "  Metadata is auto-included by GitHub.",
                            Style::default().fg(t.text_muted),
                        )),
                        Line::from(""),
                        Line::from(Span::styled("Tip:", Style::default().fg(t.success))),
                        Line::from("  For initial setup, grant access to"),
                        Line::from("  'All repositories'. After setup, you"),
                        Line::from("  can restrict to only your storage repo."),
                    ])
                }
            }
            GitHubField::RepoName => Text::from(vec![
                Line::from(Span::styled("Repository Name", t.title_style())),
                Line::from(""),
                Line::from("Name for your dotfiles repository."),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Note: ", Style::default().fg(t.warning)),
                    Span::raw("If you already have a repo,"),
                ]),
                Line::from("enter its exact name here."),
            ]),
            GitHubField::RepoPath => Text::from(vec![
                Line::from(Span::styled("Local Path", t.title_style())),
                Line::from(""),
                Line::from("Where dotfiles are stored locally."),
                Line::from(""),
                Line::from("Default: ~/.config/dotstate/storage"),
            ]),
            GitHubField::Visibility => Text::from(vec![
                Line::from(Span::styled("Repository Visibility", t.title_style())),
                Line::from(""),
                Line::from(vec![
                    Span::styled("Private: ", Style::default().fg(t.success)),
                    Span::raw("Only you can access"),
                ]),
                Line::from(vec![
                    Span::styled("Public: ", Style::default().fg(t.warning)),
                    Span::raw("Anyone can view"),
                ]),
                Line::from(""),
                Line::from("Press Space to toggle"),
            ]),
        }
    }

    /// Get help text for Local form
    fn get_local_help(&self) -> Text<'static> {
        let t = theme();

        Text::from(vec![
            Line::from(Span::styled("Repository Path", t.title_style())),
            Line::from(""),
            Line::from("Path to your cloned git repository."),
            Line::from(""),
            Line::from(Span::styled(
                "Requirements:",
                Style::default().fg(t.primary),
            )),
            Line::from("  • Valid git repository"),
            Line::from("  • Has 'origin' remote"),
            Line::from("  • Can push to remote"),
        ])
    }

    /// Render the processing state with centered progress
    fn render_processing(&self, frame: &mut Frame, area: Rect, step: GitHubSetupStep) {
        let t = theme();

        // Center the progress box
        // Height: 8 steps + plus 1 unified padding = 10
        let popup_width = 50u16.min(area.width.saturating_sub(4));
        let popup_height = 30u16.min(area.height.saturating_sub(2));
        let popup_area = crate::utils::center_popup(area, popup_width, popup_height);

        // Clear the popup area
        frame.render_widget(Clear, popup_area);

        // Build progress content
        let steps = [
            (GitHubSetupStep::Connecting, "Connecting to GitHub"),
            (GitHubSetupStep::ValidatingToken, "Validating token"),
            (GitHubSetupStep::CheckingRepo, "Checking repository"),
            (GitHubSetupStep::CloningRepo, "Cloning repository"),
            (GitHubSetupStep::CreatingRepo, "Creating repository"),
            (GitHubSetupStep::InitializingRepo, "Initializing repository"),
            (GitHubSetupStep::DiscoveringProfiles, "Discovering profiles"),
            (GitHubSetupStep::Complete, "Complete"),
        ];

        let current_step_index = steps.iter().position(|(s, _)| *s == step).unwrap_or(0);

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(""));

        for (i, (_, label)) in steps.iter().enumerate() {
            let (prefix, style) = if i < current_step_index {
                ("✓ ", Style::default().fg(t.success))
            } else if i == current_step_index {
                (
                    "→ ",
                    Style::default().fg(t.primary).add_modifier(Modifier::BOLD),
                )
            } else {
                ("  ", t.muted_style())
            };

            lines.push(Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(*label, style),
            ]));
        }

        // Add status message if any
        if let Some(ref status) = self.state.status_message {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                status.as_str(),
                Style::default().fg(t.text),
            )));
        }

        let progress_block = Block::default()
            .borders(Borders::ALL)
            .title(" Setting Up Repository ")
            .title_alignment(Alignment::Center)
            .border_style(Style::default().fg(t.primary))
            .border_type(t.border_type(true))
            .padding(Padding::proportional(1))
            .style(t.background_style());

        let para = Paragraph::new(lines)
            .block(progress_block)
            .wrap(Wrap { trim: true });

        frame.render_widget(para, popup_area);
    }

    /// Handle mouse events
    fn handle_mouse_event(&mut self, mouse: crossterm::event::MouseEvent) -> Result<ScreenAction> {
        // Don't handle mouse during processing
        if matches!(self.state.step, StorageSetupStep::Processing(_)) {
            return Ok(ScreenAction::None);
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check method list click
                if let Some(&idx) = self.method_regions.hit_test(mouse.column, mouse.row) {
                    if let Some(method) = StorageMethod::from_index(idx) {
                        self.state.method = method;
                        self.state.menu_state.select(Some(idx));
                        self.state.focus = StorageSetupFocus::MethodList;
                        return Ok(ScreenAction::Refresh);
                    }
                }
                // Check form field click
                if let Some(&field_idx) = self.form_field_regions.hit_test(mouse.column, mouse.row)
                {
                    self.state.focus = StorageSetupFocus::Form;
                    match self.state.method {
                        StorageMethod::GitHub => {
                            let field = match field_idx {
                                0 => GitHubField::Token,
                                1 => GitHubField::RepoName,
                                2 => GitHubField::RepoPath,
                                3 => GitHubField::Visibility,
                                _ => return Ok(ScreenAction::None),
                            };
                            self.state.github_field = field;
                        }
                        StorageMethod::Local => {
                            // Only one field in local mode
                        }
                    }
                    return Ok(ScreenAction::Refresh);
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(area) = self.method_pane_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        // Only 2 items, just move to previous
                        if let Some(current) = self.state.menu_state.selected() {
                            if current > 0 {
                                self.state.menu_state.select(Some(current - 1));
                                if let Some(m) = StorageMethod::from_index(current - 1) {
                                    self.state.method = m;
                                }
                            }
                        }
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(area) = self.method_pane_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        if let Some(current) = self.state.menu_state.selected() {
                            let max = StorageMethod::all().len().saturating_sub(1);
                            if current < max {
                                self.state.menu_state.select(Some(current + 1));
                                if let Some(m) = StorageMethod::from_index(current + 1) {
                                    self.state.method = m;
                                }
                            }
                        }
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }

    /// Handle events when method list is focused
    fn handle_list_event(&mut self, action: Option<Action>) -> Result<ScreenAction> {
        if let Some(action) = action {
            match action {
                Action::MoveUp => {
                    let current = self.state.method.index();
                    if current > 0 {
                        self.state.method = StorageMethod::from_index(current - 1).unwrap();
                        self.state.menu_state.select(Some(current - 1));
                    }
                }
                Action::MoveDown => {
                    let current = self.state.method.index();
                    if current < StorageMethod::all().len() - 1 {
                        self.state.method = StorageMethod::from_index(current + 1).unwrap();
                        self.state.menu_state.select(Some(current + 1));
                    }
                }
                Action::Confirm | Action::NextTab | Action::MoveRight => {
                    self.state.focus = StorageSetupFocus::Form;
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

    /// Handle events when form is focused
    fn handle_form_event(
        &mut self,
        key: crossterm::event::KeyEvent,
        ctx: &ScreenContext,
    ) -> Result<ScreenAction> {
        use crossterm::event::{KeyCode, KeyModifiers};

        // For plain character keys (no modifiers), ALWAYS insert the character first
        // This ensures vim bindings like h/l/q don't interfere with typing
        if let KeyCode::Char(c) = key.code {
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
            {
                // Check if we're in an editable text field
                let is_editable = match self.state.method {
                    StorageMethod::GitHub => {
                        // Visibility is a toggle, not editable
                        if self.state.github_field == GitHubField::Visibility {
                            false
                        } else if self.state.is_reconfiguring {
                            // Only token field in edit mode is editable
                            self.state.github_field == GitHubField::Token
                                && self.state.is_editing_token
                        } else {
                            // All text fields editable in fresh setup
                            true
                        }
                    }
                    StorageMethod::Local => !self.state.is_reconfiguring,
                };

                if is_editable {
                    // Insert character into the appropriate field
                    match self.state.method {
                        StorageMethod::GitHub => match self.state.github_field {
                            GitHubField::Token => self.state.token_input.insert_char(c),
                            GitHubField::RepoName => self.state.repo_name_input.insert_char(c),
                            GitHubField::RepoPath => self.state.repo_path_input.insert_char(c),
                            GitHubField::Visibility => {} // Not a text field
                        },
                        StorageMethod::Local => self.state.local_path_input.insert_char(c),
                    }
                    return Ok(ScreenAction::None);
                }
            }
        }

        let action = ctx.config.keymap.get_action(key.code, key.modifiers);

        // Handle form submission
        if matches!(action, Some(Action::Confirm | Action::Save)) {
            // In reconfiguration mode, Enter on Token field toggles edit mode
            if self.state.is_reconfiguring
                && self.state.method == StorageMethod::GitHub
                && self.state.github_field == GitHubField::Token
                && !self.state.is_editing_token
            {
                // Enter edit token mode
                self.state.is_editing_token = true;
                self.state.token_input = TextInput::new(); // Clear for new input
                self.state.status_message = Some("Enter new token".to_string());
                return Ok(ScreenAction::None);
            }
            return self.handle_submit();
        }

        // Handle cancel/back
        if let Some(Action::Cancel | Action::Quit) = action {
            // If editing token, cancel exits edit mode (not the form)
            if self.state.is_editing_token {
                self.state.is_editing_token = false;
                self.state.status_message = None;
                self.state.error_message = None;
                // Restore the masked token display (we don't have original, just clear)
                self.state.token_input = TextInput::with_text("••••••••••••••••••••");
                return Ok(ScreenAction::None);
            }
            self.state.focus = StorageSetupFocus::MethodList;
            self.state.error_message = None;
            return Ok(ScreenAction::None);
        }

        // Handle MoveLeft to go back to menu (only for text fields at cursor position 0)
        // Note: Visibility field uses MoveLeft/MoveRight for toggling, so we skip it here
        if let Some(Action::MoveLeft) = action {
            let should_go_back = match self.state.method {
                StorageMethod::GitHub => match self.state.github_field {
                    GitHubField::Token => self.state.token_input.cursor() == 0,
                    GitHubField::RepoName => self.state.repo_name_input.cursor() == 0,
                    GitHubField::RepoPath => self.state.repo_path_input.cursor() == 0,
                    GitHubField::Visibility => false, // MoveLeft toggles visibility, doesn't exit
                },
                StorageMethod::Local => self.state.local_path_input.cursor() == 0,
            };

            if should_go_back {
                self.state.focus = StorageSetupFocus::MethodList;
                return Ok(ScreenAction::None);
            }
        }

        match self.state.method {
            StorageMethod::GitHub => self.handle_github_form_input(action),
            StorageMethod::Local => self.handle_local_form_input(action),
        }
    }

    /// Handle GitHub form input (character input handled at top of `handle_form_event`)
    fn handle_github_form_input(&mut self, action: Option<Action>) -> Result<ScreenAction> {
        // Handle field navigation
        if let Some(Action::NextTab) = action {
            self.state.github_field = self.state.github_field.next();
            return Ok(ScreenAction::None);
        }

        if let Some(Action::PrevTab) = action {
            // On first field, go back to menu; otherwise go to previous field
            if self.state.github_field == GitHubField::Token {
                self.state.focus = StorageSetupFocus::MethodList;
            } else {
                self.state.github_field = self.state.github_field.prev();
            }
            return Ok(ScreenAction::None);
        }

        // Handle visibility toggle
        if self.state.github_field == GitHubField::Visibility {
            if let Some(Action::ToggleSelect) = action {
                self.state.is_private = !self.state.is_private;
                return Ok(ScreenAction::None);
            }
            if let Some(Action::MoveLeft | Action::MoveRight) = action {
                self.state.is_private = !self.state.is_private;
                return Ok(ScreenAction::None);
            }
        }

        // Check if current field is disabled
        let is_field_disabled = match self.state.github_field {
            GitHubField::Token => self.state.is_reconfiguring && !self.state.is_editing_token,
            GitHubField::RepoName | GitHubField::RepoPath => self.state.is_reconfiguring,
            GitHubField::Visibility => self.state.is_reconfiguring,
        };

        // Don't allow input on disabled fields
        if is_field_disabled {
            return Ok(ScreenAction::None);
        }

        // Handle text input for current field (character input handled at top of handle_form_event)
        let input = match self.state.github_field {
            GitHubField::Token => &mut self.state.token_input,
            GitHubField::RepoName => &mut self.state.repo_name_input,
            GitHubField::RepoPath => &mut self.state.repo_path_input,
            GitHubField::Visibility => return Ok(ScreenAction::None),
        };

        // Handle text editing actions
        if let Some(act) = action {
            match act {
                Action::Backspace => input.backspace(),
                Action::DeleteChar => input.delete(),
                Action::MoveLeft => input.move_left(),
                Action::MoveRight => input.move_right(),
                Action::Home => input.move_home(),
                Action::End => input.move_end(),
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle Local form input (character input handled at top of `handle_form_event`)
    fn handle_local_form_input(&mut self, action: Option<Action>) -> Result<ScreenAction> {
        // PrevTab (Shift+Tab) goes back to menu
        if let Some(Action::PrevTab) = action {
            self.state.focus = StorageSetupFocus::MethodList;
            return Ok(ScreenAction::None);
        }

        // Don't allow input on disabled fields (character input handled at top of handle_form_event)
        if self.state.is_reconfiguring {
            return Ok(ScreenAction::None);
        }

        let input = &mut self.state.local_path_input;

        // Handle text editing actions
        if let Some(act) = action {
            match act {
                Action::Backspace => input.backspace(),
                Action::DeleteChar => input.delete(),
                Action::MoveLeft => input.move_left(),
                Action::MoveRight => input.move_right(),
                Action::Home => input.move_home(),
                Action::End => input.move_end(),
                _ => {}
            }
        }

        Ok(ScreenAction::None)
    }

    /// Handle form submission
    fn handle_submit(&mut self) -> Result<ScreenAction> {
        self.state.error_message = None;

        // In reconfiguration mode, only allow token updates
        if self.state.is_reconfiguring {
            if self.state.method == StorageMethod::GitHub && self.state.is_editing_token {
                // User is updating their token
                let token = self.state.token_input.text_trimmed().to_string();

                // Validate token (ghp_ for classic, github_pat_ for fine-grained)
                if !token.starts_with("ghp_") && !token.starts_with("github_pat_") {
                    self.state.error_message =
                        Some("Token must start with 'ghp_' or 'github_pat_'".to_string());
                    return Ok(ScreenAction::None);
                }

                // Update the token in config
                return Ok(ScreenAction::UpdateGitHubToken { token });
            }
            // In reconfiguration mode but not editing token - show info
            self.state.status_message =
                Some("Storage already configured. Press Esc to go back.".to_string());
            return Ok(ScreenAction::None);
        }

        match self.state.method {
            StorageMethod::GitHub => {
                let token = self.state.token_input.text_trimmed().to_string();
                let repo_name = self.state.repo_name_input.text_trimmed().to_string();

                // Validate token (ghp_ for classic, github_pat_ for fine-grained)
                if !token.starts_with("ghp_") && !token.starts_with("github_pat_") {
                    self.state.error_message =
                        Some("Token must start with 'ghp_' or 'github_pat_'".to_string());
                    return Ok(ScreenAction::None);
                }

                if repo_name.is_empty() {
                    self.state.error_message = Some("Repository name required".to_string());
                    return Ok(ScreenAction::None);
                }

                // Return action to start GitHub setup
                Ok(ScreenAction::StartGitHubSetup {
                    token,
                    repo_name,
                    is_private: self.state.is_private,
                })
            }
            StorageMethod::Local => {
                let path_str = self.state.local_path_input.text_trimmed();

                if path_str.is_empty() {
                    self.state.error_message = Some("Path required".to_string());
                    return Ok(ScreenAction::None);
                }

                let expanded_path = crate::git::expand_path(path_str);
                let validation = crate::git::validate_local_repo(&expanded_path);

                if !validation.is_valid {
                    self.state.error_message = validation.error_message;
                    return Ok(ScreenAction::None);
                }

                // Load profiles from the repository
                let profiles = crate::utils::ProfileManifest::load_or_backfill(&expanded_path)
                    .map(|m| m.profiles.iter().map(|p| p.name.clone()).collect())
                    .unwrap_or_default();

                Ok(ScreenAction::SaveLocalRepoConfig {
                    repo_path: expanded_path,
                    profiles,
                })
            }
        }
    }
}

impl Screen for StorageSetupScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Clear and set background
        frame.render_widget(Clear, area);
        let t = theme();
        let background = Block::default().style(t.background_style());
        frame.render_widget(background, area);

        // Standard layout (header=5, footer=3)
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 3);

        // Header
        Header::render(
            frame,
            header_chunk,
            "DotState - Storage Setup",
            "Choose where to store your dotfiles.",
        )?;

        // Check if we're in processing mode
        if let StorageSetupStep::Processing(step) = self.state.step {
            // Render processing overlay using full area (not just content_chunk)
            // so the popup has room to display all steps
            self.render_processing(frame, area, step);

            // Footer with processing message
            Footer::render(frame, footer_chunk, "Setting up your repository...")?;
        } else {
            // Content: two-pane layout (40/60 like settings)
            let panes = create_split_layout(content_chunk, &[40, 60]);

            // Left: method selection list
            self.render_method_list(frame, panes[0], ctx);

            // Right: form and help
            self.render_form_pane(frame, panes[1], ctx);

            // Footer - context-sensitive based on mode
            let footer_text = match self.state.focus {
                StorageSetupFocus::MethodList => format!(
                    "{}: Navigate | {}: Configure | {}: Back",
                    ctx.config.keymap.navigation_display(),
                    self.key_display(ctx, Action::Confirm),
                    self.key_display(ctx, Action::Cancel),
                ),
                StorageSetupFocus::Form => {
                    if self.state.is_reconfiguring {
                        if self.state.method == StorageMethod::GitHub {
                            if self.state.is_editing_token {
                                format!(
                                    "{}: Next Field | {}: Save Token | {}: Cancel",
                                    self.key_display(ctx, Action::NextTab),
                                    self.key_display(ctx, Action::Confirm),
                                    self.key_display(ctx, Action::Cancel),
                                )
                            } else if self.state.github_field == GitHubField::Token {
                                format!(
                                    "{}: Next Field | {}: Edit Token | {}: Back",
                                    self.key_display(ctx, Action::NextTab),
                                    self.key_display(ctx, Action::Confirm),
                                    self.key_display(ctx, Action::Cancel),
                                )
                            } else {
                                format!(
                                    "{}: Navigate Fields | {}: Back",
                                    self.key_display(ctx, Action::NextTab),
                                    self.key_display(ctx, Action::Cancel),
                                )
                            }
                        } else {
                            // Local mode in reconfiguration
                            format!(
                                "{}: Back (view only)",
                                self.key_display(ctx, Action::Cancel),
                            )
                        }
                    } else {
                        format!(
                            "{}: Next Field | {}: Submit | {}: Back",
                            self.key_display(ctx, Action::NextTab),
                            self.key_display(ctx, Action::Confirm),
                            self.key_display(ctx, Action::Cancel),
                        )
                    }
                }
            };
            Footer::render(frame, footer_chunk, &footer_text)?;
        }

        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        self.state.error_message = None;

        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = ctx.config.keymap.get_action(key.code, key.modifiers);

                match self.state.focus {
                    StorageSetupFocus::MethodList => self.handle_list_event(action),
                    StorageSetupFocus::Form => self.handle_form_event(key, ctx),
                }
            }
            Event::Mouse(mouse) => self.handle_mouse_event(mouse),
            _ => Ok(ScreenAction::None),
        }
    }

    fn is_input_focused(&self) -> bool {
        // Only return true when we're actually typing in a text input field
        if self.state.focus != StorageSetupFocus::Form {
            return false;
        }

        match self.state.method {
            StorageMethod::GitHub => {
                // Visibility is a toggle, not a text input
                if self.state.github_field == GitHubField::Visibility {
                    return false;
                }

                // In reconfiguration mode, only token field is editable (and only in edit mode)
                if self.state.is_reconfiguring {
                    return self.state.github_field == GitHubField::Token
                        && self.state.is_editing_token;
                }

                // In fresh setup, all text fields are editable
                true
            }
            StorageMethod::Local => {
                // Local path is editable only in fresh setup
                !self.state.is_reconfiguring
            }
        }
    }

    fn on_enter(&mut self, ctx: &ScreenContext) -> Result<()> {
        // Check if already configured using the proper method
        // This checks: GitHub mode with github config, OR Local mode with .git existing
        let is_configured = ctx.config.is_repo_configured();

        if is_configured {
            // Reconfiguration mode - pre-fill with existing values
            self.state.is_reconfiguring = true;
            self.state.focus = StorageSetupFocus::MethodList;

            // Determine which method was used
            if ctx.config.github.is_some() {
                // GitHub mode
                self.state.method = StorageMethod::GitHub;
                self.state.menu_state.select(Some(0));

                if let Some(ref github) = ctx.config.github {
                    // Pre-fill token (masked display - actual token not shown)
                    if github.token.is_some() {
                        self.state.token_input = TextInput::with_text("••••••••••••••••••••");
                    }
                    // Pre-fill repo name
                    self.state.repo_name_input = TextInput::with_text(github.repo.clone());
                }
                // Ensure edit mode is off
                self.state.is_editing_token = false;
                // Pre-fill repo path
                self.state.repo_path_input =
                    TextInput::with_text(ctx.config.repo_path.to_string_lossy().to_string());
            } else {
                // Local mode
                self.state.method = StorageMethod::Local;
                self.state.menu_state.select(Some(1));
                self.state.local_path_input =
                    TextInput::with_text(ctx.config.repo_path.to_string_lossy().to_string());
            }

            self.state.error_message = None;
            self.state.status_message = None;
            self.state.step = StorageSetupStep::Input;
            self.state.setup_data = None;
        } else {
            // Fresh setup - reset to defaults
            self.reset();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_method_index() {
        assert_eq!(StorageMethod::GitHub.index(), 0);
        assert_eq!(StorageMethod::Local.index(), 1);
    }

    #[test]
    fn test_storage_method_from_index() {
        assert_eq!(StorageMethod::from_index(0), Some(StorageMethod::GitHub));
        assert_eq!(StorageMethod::from_index(1), Some(StorageMethod::Local));
        assert_eq!(StorageMethod::from_index(2), None);
    }

    #[test]
    fn test_github_field_navigation() {
        assert_eq!(GitHubField::Token.next(), GitHubField::RepoName);
        assert_eq!(GitHubField::Visibility.next(), GitHubField::Token);
        assert_eq!(GitHubField::Token.prev(), GitHubField::Visibility);
    }

    #[test]
    fn test_default_state() {
        let screen = StorageSetupScreen::new();
        assert_eq!(screen.state.focus, StorageSetupFocus::MethodList);
        assert_eq!(screen.state.method, StorageMethod::GitHub);
    }
}
