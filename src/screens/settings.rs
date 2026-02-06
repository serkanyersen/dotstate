//! Settings screen for configuring application options.
//!
//! Provides a two-pane interface:
//! - Left: List of settings
//! - Right: Current value, options, and explanation

use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::config::{Config, RepoMode};
use crate::icons::Icons;
use crate::keymap::{Action, KeymapPreset};
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{init_theme, theme, ThemeType};
use crate::ui::Screen as ScreenId;
use crate::utils::{
    create_split_layout, create_standard_layout, focused_border_style, unfocused_border_style,
    MouseRegions,
};
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Padding, Paragraph, StatefulWidget, Wrap,
};
use ratatui::Frame;

/// Available settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    Theme,
    IconSet,
    KeymapPreset,
    Backups,
    CheckForUpdates,
    EmbedCredentials,
}

impl SettingItem {
    #[must_use]
    pub fn all(repo_mode: RepoMode) -> Vec<SettingItem> {
        let mut items = vec![
            SettingItem::Theme,
            SettingItem::IconSet,
            SettingItem::KeymapPreset,
            SettingItem::Backups,
            SettingItem::CheckForUpdates,
        ];
        if repo_mode == RepoMode::GitHub {
            items.push(SettingItem::EmbedCredentials);
        }
        items
    }

    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            SettingItem::Theme => "Theme",
            SettingItem::IconSet => "Icon Set",
            SettingItem::KeymapPreset => "Keymap Preset",
            SettingItem::Backups => "Backups",
            SettingItem::CheckForUpdates => "Check for Updates",
            SettingItem::EmbedCredentials => "Token in Remote URL",
        }
    }

    #[must_use]
    pub fn from_index(index: usize, repo_mode: RepoMode) -> Option<SettingItem> {
        Self::all(repo_mode).get(index).copied()
    }
}

/// Focus within the settings screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsFocus {
    #[default]
    List,
    Options,
}

/// Settings screen state
#[derive(Debug)]
pub struct SettingsState {
    pub list_state: ListState,
    pub focus: SettingsFocus,
    pub option_index: usize, // Selected option within the current setting
}

impl Default for SettingsState {
    fn default() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            focus: SettingsFocus::List,
            option_index: 0,
        }
    }
}

/// Settings screen controller
pub struct SettingsScreen {
    state: SettingsState,
    /// Clickable regions for settings list items
    settings_regions: MouseRegions<usize>,
    /// Clickable regions for option items
    option_regions: MouseRegions<usize>,
    /// Area of the settings list pane (for scroll hit-testing)
    list_pane_area: Option<Rect>,
    /// Area of the options pane (for scroll hit-testing)
    options_pane_area: Option<Rect>,
}

impl Default for SettingsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsScreen {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: SettingsState::default(),
            settings_regions: MouseRegions::new(),
            option_regions: MouseRegions::new(),
            list_pane_area: None,
            options_pane_area: None,
        }
    }

    fn selected_setting(&self, repo_mode: RepoMode) -> Option<SettingItem> {
        self.state
            .list_state
            .selected()
            .and_then(|i| SettingItem::from_index(i, repo_mode))
    }

    /// Get available options for the current setting
    fn get_options(&self, config: &Config) -> Vec<(String, bool)> {
        match self.selected_setting(config.repo_mode) {
            Some(SettingItem::Theme) => {
                let current = &config.theme;
                ThemeType::all()
                    .iter()
                    .map(|t| (t.name().to_string(), current == t.to_config_string()))
                    .collect()
            }
            Some(SettingItem::IconSet) => {
                use crate::icons::IconSet;
                let current = &config.icon_set;
                vec![
                    ("auto".to_string(), current == "auto"),
                    (IconSet::NerdFonts.name().to_string(), current == "nerd"),
                    (IconSet::Unicode.name().to_string(), current == "unicode"),
                    (IconSet::Emoji.name().to_string(), current == "emoji"),
                    (IconSet::Ascii.name().to_string(), current == "ascii"),
                ]
            }
            Some(SettingItem::KeymapPreset) => {
                let current = config.keymap.preset;
                vec![
                    ("Standard".to_string(), current == KeymapPreset::Standard),
                    ("Vim".to_string(), current == KeymapPreset::Vim),
                    ("Emacs".to_string(), current == KeymapPreset::Emacs),
                ]
            }
            Some(SettingItem::Backups) => {
                vec![
                    ("Enabled".to_string(), config.backup_enabled),
                    ("Disabled".to_string(), !config.backup_enabled),
                ]
            }
            Some(SettingItem::CheckForUpdates) => {
                vec![
                    ("Enabled".to_string(), config.updates.check_enabled),
                    ("Disabled".to_string(), !config.updates.check_enabled),
                ]
            }
            Some(SettingItem::EmbedCredentials) => {
                vec![
                    ("Enabled".to_string(), config.embed_credentials_in_url),
                    ("Disabled".to_string(), !config.embed_credentials_in_url),
                ]
            }
            None => vec![],
        }
    }

    /// Get explanation text for the current setting
    fn get_explanation(&self, config: &Config) -> Text<'static> {
        let t = theme();
        let icons = Icons::from_config(config);

        match self.selected_setting(config.repo_mode) {
            Some(SettingItem::Theme) => {
                let lines = vec![
                    Line::from(Span::styled("Color Theme", t.title_style())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Choose how DotState looks. The theme affects all colors in the UI.",
                        t.text_style(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(" Current: ", t.muted_style()),
                        Span::styled(config.theme.clone(), t.emphasis_style()),
                    ]),
                ];
                Text::from(lines)
            }
            Some(SettingItem::IconSet) => {
                let icons_preview = Icons::from_config(config);
                let lines = vec![
                    Line::from(Span::styled("Icon Set", t.title_style())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Choose which icon set to use in the interface.",
                        t.text_style(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled("Preview:", t.muted_style())),
                    Line::from(vec![
                        Span::styled(
                            format!("  {} Folder  ", icons_preview.folder()),
                            t.text_style(),
                        ),
                        Span::styled(format!("{} File  ", icons_preview.file()), t.text_style()),
                        Span::styled(format!("{} Sync", icons_preview.sync()), t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(" Tip: ", Style::default().fg(t.secondary)),
                        Span::styled(
                            "Use 'nerd' if you have a NerdFont installed",
                            t.text_style(),
                        ),
                    ]),
                ];
                Text::from(lines)
            }
            Some(SettingItem::KeymapPreset) => {
                let lines = vec![
                    Line::from(Span::styled("Keymap Preset", t.title_style())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Choose keyboard bindings that feel natural to you.",
                        t.text_style(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  • ", t.muted_style()),
                        Span::styled("Standard", t.emphasis_style()),
                        Span::styled(": Arrow keys, Enter, Escape", t.text_style()),
                    ]),
                    Line::from(vec![
                        Span::styled("  • ", t.muted_style()),
                        Span::styled("Vim", t.emphasis_style()),
                        Span::styled(": hjkl navigation, Esc to cancel", t.text_style()),
                    ]),
                    Line::from(vec![
                        Span::styled("  • ", t.muted_style()),
                        Span::styled("Emacs", t.emphasis_style()),
                        Span::styled(": Ctrl+n/p/f/b navigation", t.text_style()),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(" Override bindings in config:", t.muted_style()),
                    ]),
                    Line::from(Span::styled("  [keymap.overrides]", t.emphasis_style())),
                    Line::from(Span::styled("  confirm = \"ctrl+s\"", t.emphasis_style())),
                ];
                Text::from(lines)
            }
            Some(SettingItem::Backups) => {
                let lines = vec![
                    Line::from(Span::styled("Automatic Backups", t.title_style())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "When enabled, DotState creates .bak files before overwriting existing files during sync operations.",
                        t.text_style(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(" Current: ", t.muted_style()),
                        Span::styled(
                            if config.backup_enabled { "Enabled" } else { "Disabled" },
                            t.emphasis_style(),
                        ),
                    ]),
                ];
                Text::from(lines)
            }
            Some(SettingItem::CheckForUpdates) => {
                let lines = vec![
                    Line::from(Span::styled("Update Checks", t.title_style())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "When enabled, DotState periodically checks for new versions and shows a notification in the main menu.",
                        t.text_style(),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "You can always manually check for updates using:",
                        t.text_style(),
                    )),
                    Line::from(Span::styled("  dotstate upgrade", t.emphasis_style())),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(" Current: ", t.muted_style()),
                        Span::styled(
                            if config.updates.check_enabled { "Enabled" } else { "Disabled" },
                            t.emphasis_style(),
                        ),
                    ]),
                ];
                Text::from(lines)
            }
            Some(SettingItem::EmbedCredentials) => {
                let lines = vec![
                    Line::from(Span::styled("Token in Remote URL", t.title_style())),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Controls how DotState authenticates with GitHub when syncing your dotfiles.",
                        t.text_style(),
                    )),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  • ", t.muted_style()),
                        Span::styled("Enabled", t.emphasis_style()),
                        Span::styled(
                            ": Token is stored in the remote URL",
                            t.text_style(),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled("  • ", t.muted_style()),
                        Span::styled("Disabled", t.emphasis_style()),
                        Span::styled(
                            ": Uses your system's git credential manager",
                            t.text_style(),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(
                            " Disable if your environment blocks URLs with embedded tokens.",
                            t.muted_style(),
                        ),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(icons.lightbulb(), Style::default().fg(t.secondary)),
                        Span::styled(" Current: ", t.muted_style()),
                        Span::styled(
                            if config.embed_credentials_in_url { "Enabled" } else { "Disabled" },
                            t.emphasis_style(),
                        ),
                    ]),
                ];
                Text::from(lines)
            }
            None => Text::from(""),
        }
    }

    /// Apply a setting change by setting name (public, for use by App)
    pub fn apply_setting_to_config(
        &self,
        config: &mut Config,
        setting_name: &str,
        option_index: usize,
    ) -> bool {
        Self::apply_setting_by_name(config, setting_name, option_index)
    }

    /// Apply a setting by name and option index
    fn apply_setting_by_name(config: &mut Config, setting_name: &str, option_index: usize) -> bool {
        match setting_name {
            "Theme" => {
                let themes = ThemeType::all();
                if option_index < themes.len() {
                    let selected_theme = themes[option_index];
                    config.theme = selected_theme.to_config_string().to_string();
                    // Apply theme immediately
                    init_theme(selected_theme);
                    return true;
                }
            }
            "Icon Set" => {
                let sets = ["auto", "nerd", "unicode", "emoji", "ascii"];
                if option_index < sets.len() {
                    config.icon_set = sets[option_index].to_string();
                    return true;
                }
            }
            "Keymap Preset" => {
                let presets = [
                    KeymapPreset::Standard,
                    KeymapPreset::Vim,
                    KeymapPreset::Emacs,
                ];
                if option_index < presets.len() {
                    config.keymap.preset = presets[option_index];
                    // Clear overrides when changing preset to ensure clean bindings
                    config.keymap.overrides.clear();
                    return true;
                }
            }
            "Backups" => {
                config.backup_enabled = option_index == 0;
                return true;
            }
            "Check for Updates" => {
                config.updates.check_enabled = option_index == 0;
                return true;
            }
            "Token in Remote URL" => {
                config.embed_credentials_in_url = option_index == 0;
                return true;
            }
            _ => {}
        }
        false
    }

    /// Find the current option index for the selected setting
    fn current_option_index(&self, config: &Config) -> usize {
        let options = self.get_options(config);
        options
            .iter()
            .position(|(_, selected)| *selected)
            .unwrap_or(0)
    }

    fn render_settings_list(&mut self, frame: &mut Frame, area: Rect, config: &Config) {
        let t = theme();
        let icons = Icons::from_config(config);
        let is_focused = self.state.focus == SettingsFocus::List;

        // Store pane area and populate mouse regions
        self.list_pane_area = Some(area);
        self.settings_regions.clear();
        let inner = Block::default().borders(Borders::ALL).inner(area);
        let item_count = SettingItem::all(config.repo_mode).len();
        let scroll_offset = self.state.list_state.offset();
        for i in 0..item_count {
            let visible_idx = i.saturating_sub(scroll_offset);
            if i >= scroll_offset && (visible_idx as u16) < inner.height {
                let row = Rect::new(inner.x, inner.y + visible_idx as u16, inner.width, 1);
                self.settings_regions.add(row, i);
            }
        }

        let items: Vec<ListItem> = SettingItem::all(config.repo_mode)
            .iter()
            .map(|item| {
                let current_value = match item {
                    SettingItem::Theme => config.theme.clone(),
                    SettingItem::IconSet => config.icon_set.clone(),
                    SettingItem::KeymapPreset => format!("{:?}", config.keymap.preset),
                    SettingItem::Backups => {
                        if config.backup_enabled {
                            "On".to_string()
                        } else {
                            "Off".to_string()
                        }
                    }
                    SettingItem::CheckForUpdates => {
                        if config.updates.check_enabled {
                            "On".to_string()
                        } else {
                            "Off".to_string()
                        }
                    }
                    SettingItem::EmbedCredentials => {
                        if config.embed_credentials_in_url {
                            "On".to_string()
                        } else {
                            "Off".to_string()
                        }
                    }
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{} ", icons.cog()),
                        Style::default().fg(t.secondary),
                    ),
                    Span::styled(item.name(), t.text_style()),
                    Span::styled(format!(" ({current_value})"), t.muted_style()),
                ]);
                ListItem::new(line)
            })
            .collect();

        let border_style = if is_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Settings ")
                    .title_alignment(Alignment::Center)
                    .border_type(t.border_type(is_focused))
                    .border_style(border_style)
                    .style(t.background_style()),
            )
            .highlight_style(t.highlight_style())
            .highlight_symbol(crate::styles::LIST_HIGHLIGHT_SYMBOL);

        StatefulWidget::render(list, area, frame.buffer_mut(), &mut self.state.list_state);
    }

    fn render_options_pane(&mut self, frame: &mut Frame, area: Rect, config: &Config) {
        let t = theme();
        let is_focused = self.state.focus == SettingsFocus::Options;

        // Split into options (top) and explanation (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Store pane area and populate option regions
        self.options_pane_area = Some(chunks[0]);
        self.option_regions.clear();

        // Render options
        let options = self.get_options(config);
        let icons = Icons::from_config(config);

        // Populate option click regions (each option is 1 line inside the block)
        let options_inner = Block::default().borders(Borders::ALL).inner(chunks[0]);
        for i in 0..options.len() {
            if (i as u16) < options_inner.height {
                let row = Rect::new(
                    options_inner.x,
                    options_inner.y + i as u16,
                    options_inner.width,
                    1,
                );
                self.option_regions.add(row, i);
            }
        }

        let option_lines: Vec<Line> = options
            .iter()
            .enumerate()
            .map(|(i, (name, selected))| {
                let marker = if *selected {
                    icons.circle_filled()
                } else {
                    icons.circle_empty()
                };
                let style = if *selected {
                    Style::default().fg(t.success).add_modifier(Modifier::BOLD)
                } else if is_focused && i == self.state.option_index {
                    t.highlight_style()
                } else {
                    t.text_style()
                };
                Line::from(vec![
                    Span::styled(format!("  {marker} "), style),
                    Span::styled(name.clone(), style),
                ])
            })
            .collect();

        let border_style = if is_focused {
            focused_border_style()
        } else {
            unfocused_border_style()
        };

        let options_block = Paragraph::new(option_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Options ")
                    .title_alignment(Alignment::Center)
                    .border_type(t.border_type(is_focused))
                    .border_style(border_style)
                    .style(t.background_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(options_block, chunks[0]);

        // Render explanation
        let explanation = self.get_explanation(config);
        let explanation_block = Paragraph::new(explanation)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Details ")
                    .title_alignment(Alignment::Center)
                    .border_type(t.border_type(false))
                    .border_style(unfocused_border_style())
                    .padding(Padding::proportional(1))
                    .style(t.background_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(explanation_block, chunks[1]);
    }
}

impl Screen for SettingsScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Standard layout (header=5, footer=2)
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 5, 3);

        // Header
        Header::render(
            frame,
            header_chunk,
            "DotState - Settings",
            "Configure your preferences. Changes are applied instantly.",
        )?;

        // Content: two-pane layout
        let panes = create_split_layout(content_chunk, &[40, 60]);

        // Left: settings list
        self.render_settings_list(frame, panes[0], ctx.config);

        // Right: options and explanation
        self.render_options_pane(frame, panes[1], ctx.config);

        // Footer
        let k = |a| ctx.config.keymap.get_key_display_for_action(a);
        let footer_text = format!(
            "{}: Navigate | {}: Switch Focus | {}: Select | {}: Back",
            ctx.config.keymap.navigation_display(),
            k(Action::NextTab),
            k(Action::Confirm),
            k(Action::Cancel),
        );
        Footer::render(frame, footer_chunk, &footer_text)?;

        Ok(())
    }

    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let action = ctx.config.keymap.get_action(key.code, key.modifiers);

                if let Some(action) = action {
                    match self.state.focus {
                        SettingsFocus::List => match action {
                            Action::MoveUp => {
                                self.state.list_state.select_previous();
                                self.state.option_index = self.current_option_index(ctx.config);
                            }
                            Action::MoveDown => {
                                self.state.list_state.select_next();
                                self.state.option_index = self.current_option_index(ctx.config);
                            }
                            Action::Confirm | Action::NextTab | Action::MoveRight => {
                                self.state.focus = SettingsFocus::Options;
                                self.state.option_index = self.current_option_index(ctx.config);
                            }
                            Action::Cancel | Action::Quit => {
                                return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                            }
                            _ => {}
                        },
                        SettingsFocus::Options => {
                            let options = self.get_options(ctx.config);
                            match action {
                                Action::MoveUp => {
                                    if self.state.option_index > 0 {
                                        self.state.option_index -= 1;
                                    }
                                }
                                Action::MoveDown => {
                                    if self.state.option_index < options.len().saturating_sub(1) {
                                        self.state.option_index += 1;
                                    }
                                }
                                Action::Confirm => {
                                    return Ok(ScreenAction::UpdateSetting {
                                        setting: self
                                            .selected_setting(ctx.config.repo_mode)
                                            .map(|s| s.name().to_string())
                                            .unwrap_or_default(),
                                        option_index: self.state.option_index,
                                    });
                                }
                                Action::NextTab | Action::MoveLeft | Action::Cancel => {
                                    self.state.focus = SettingsFocus::List;
                                }
                                Action::Quit => {
                                    return Ok(ScreenAction::Navigate(ScreenId::MainMenu));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Event::Mouse(mouse) => {
                return self.handle_mouse_event(mouse, ctx);
            }
            _ => {}
        }

        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        false
    }

    fn on_enter(&mut self, _ctx: &ScreenContext) -> Result<()> {
        // Reset to first setting
        self.state.list_state.select(Some(0));
        self.state.focus = SettingsFocus::List;
        self.state.option_index = 0;
        Ok(())
    }
}

impl SettingsScreen {
    fn handle_mouse_event(
        &mut self,
        mouse: crossterm::event::MouseEvent,
        ctx: &ScreenContext,
    ) -> Result<ScreenAction> {
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check settings list click
                if let Some(&idx) = self.settings_regions.hit_test(mouse.column, mouse.row) {
                    self.state.list_state.select(Some(idx));
                    self.state.focus = SettingsFocus::List;
                    self.state.option_index = self.current_option_index(ctx.config);
                    return Ok(ScreenAction::Refresh);
                }
                // Check options click
                if let Some(&idx) = self.option_regions.hit_test(mouse.column, mouse.row) {
                    self.state.focus = SettingsFocus::Options;
                    self.state.option_index = idx;
                    // Apply immediately on click
                    return Ok(ScreenAction::UpdateSetting {
                        setting: self
                            .selected_setting(ctx.config.repo_mode)
                            .map(|s| s.name().to_string())
                            .unwrap_or_default(),
                        option_index: idx,
                    });
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(area) = self.list_pane_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        for _ in 0..3 {
                            self.state.list_state.select_previous();
                        }
                        self.state.option_index = self.current_option_index(ctx.config);
                        return Ok(ScreenAction::Refresh);
                    }
                }
                if let Some(area) = self.options_pane_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        self.state.focus = SettingsFocus::Options;
                        self.state.option_index = self.state.option_index.saturating_sub(3);
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(area) = self.list_pane_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        for _ in 0..3 {
                            self.state.list_state.select_next();
                        }
                        self.state.option_index = self.current_option_index(ctx.config);
                        return Ok(ScreenAction::Refresh);
                    }
                }
                if let Some(area) = self.options_pane_area {
                    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
                        self.state.focus = SettingsFocus::Options;
                        let max = self.get_options(ctx.config).len().saturating_sub(1);
                        self.state.option_index = (self.state.option_index + 3).min(max);
                        return Ok(ScreenAction::Refresh);
                    }
                }
            }
            _ => {}
        }
        Ok(ScreenAction::None)
    }
}
