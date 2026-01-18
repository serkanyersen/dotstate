//! Settings screen for configuring application options.
//!
//! Provides a two-pane interface:
//! - Left: List of settings
//! - Right: Current value, options, and explanation

use crate::components::footer::Footer;
use crate::components::header::Header;
use crate::config::Config;
use crate::icons::Icons;
use crate::keymap::{Action, KeymapPreset};
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::styles::{init_theme, theme, ThemeType};
use crate::ui::Screen as ScreenId;
use crate::utils::{
    create_split_layout, create_standard_layout, focused_border_style, unfocused_border_style,
};
use anyhow::Result;
use crossterm::event::{Event, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Wrap,
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
}

impl SettingItem {
    pub fn all() -> Vec<SettingItem> {
        vec![
            SettingItem::Theme,
            SettingItem::IconSet,
            SettingItem::KeymapPreset,
            SettingItem::Backups,
            SettingItem::CheckForUpdates,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            SettingItem::Theme => "Theme",
            SettingItem::IconSet => "Icon Set",
            SettingItem::KeymapPreset => "Keymap Preset",
            SettingItem::Backups => "Backups",
            SettingItem::CheckForUpdates => "Check for Updates",
        }
    }

    pub fn from_index(index: usize) -> Option<SettingItem> {
        Self::all().get(index).copied()
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
}

impl Default for SettingsScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsScreen {
    pub fn new() -> Self {
        Self {
            state: SettingsState::default(),
        }
    }

    fn selected_setting(&self) -> Option<SettingItem> {
        self.state
            .list_state
            .selected()
            .and_then(SettingItem::from_index)
    }

    /// Get available options for the current setting
    fn get_options(&self, config: &Config) -> Vec<(String, bool)> {
        match self.selected_setting() {
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
            None => vec![],
        }
    }

    /// Get explanation text for the current setting
    fn get_explanation(&self, config: &Config) -> Text<'static> {
        let t = theme();
        let icons = Icons::from_config(config);

        match self.selected_setting() {
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

        let items: Vec<ListItem> = SettingItem::all()
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
                };

                let line = Line::from(vec![
                    Span::styled(
                        format!("{} ", icons.cog()),
                        Style::default().fg(t.secondary),
                    ),
                    Span::styled(item.name(), t.text_style()),
                    Span::styled(format!(" ({})", current_value), t.muted_style()),
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

    fn render_options_pane(&self, frame: &mut Frame, area: Rect, config: &Config) {
        let t = theme();
        let is_focused = self.state.focus == SettingsFocus::Options;

        // Split into options (top) and explanation (bottom)
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Render options
        let options = self.get_options(config);
        let icons = Icons::from_config(config);

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
                    Span::styled(format!("  {} ", marker), style),
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
                    .style(t.background_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(explanation_block, chunks[1]);
    }
}

impl Screen for SettingsScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()> {
        // Standard layout (header=6, footer=2)
        let (header_chunk, content_chunk, footer_chunk) = create_standard_layout(area, 6, 2);

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
        if let Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                return Ok(ScreenAction::None);
            }

            let action = ctx.config.keymap.get_action(key.code, key.modifiers);

            if let Some(action) = action {
                match self.state.focus {
                    SettingsFocus::List => match action {
                        Action::MoveUp => {
                            self.state.list_state.select_previous();
                            // Update option index to current selection
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
                                // Apply the selected option
                                return Ok(ScreenAction::UpdateSetting {
                                    setting: self
                                        .selected_setting()
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
