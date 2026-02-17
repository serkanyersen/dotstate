//! Theme and style system for `tui-forge`
//!
//! Provides consistent styling across TUI applications with support for
//! 12 built-in theme variants (light, dark, and popular community palettes).
//!
//! # Usage
//!
//! ```rust,no_run
//! use tui_forge::theme::{init_theme, set_custom_theme, theme, Theme, ThemeType};
//!
//! // Initialize with a built-in theme (call once at startup)
//! init_theme(ThemeType::CatppuccinMocha);
//!
//! // Or inject a fully user-constructed theme
//! let mut custom = Theme::new(ThemeType::Dark);
//! custom.list_highlight_symbol = "> ";
//! set_custom_theme(custom);
//!
//! // Access the current theme anywhere in the application
//! let t = theme();
//! let style = t.title_style();
//! ```

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;
use std::str::FromStr;
use std::sync::RwLock;

/// Global theme instance (supports runtime updates)
static THEME: RwLock<Theme> = RwLock::new(Theme {
    theme_type: ThemeType::Dark,
    primary: Color::Cyan,
    secondary: Color::Magenta,
    tertiary: Color::Blue,
    success: Color::Green,
    warning: Color::Yellow,
    error: Color::Red,
    text: Color::White,
    text_muted: Color::DarkGray,
    text_dimmed: Color::Cyan,
    text_emphasis: Color::Yellow,
    border: Color::DarkGray,
    border_focused: Color::Cyan,
    highlight_bg: Color::DarkGray,
    background: Color::Reset,
    dim_bg: Color::Black,
    border_type: BorderType::Plain,
    border_focused_type: BorderType::Thick,
    dialog_border_type: BorderType::Double,
    list_highlight_symbol: "\u{00bb} ",
});

/// Initialize the global theme from a built-in variant.
///
/// Call once at startup, or again at runtime to switch themes.
/// Recovers from poisoned locks so the theme is always accessible.
pub fn init_theme(theme_type: ThemeType) {
    let mut theme = THEME
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *theme = Theme::new(theme_type);
}

/// Replace the global theme with a fully user-constructed [`Theme`].
///
/// Use this when the built-in variants are not sufficient and you need to
/// supply your own color palette or override individual fields.
pub fn set_custom_theme(theme: Theme) {
    let mut global = THEME
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *global = theme;
}

/// Get a clone of the current global theme.
///
/// Recovers from poisoned locks so the theme is always accessible.
pub fn theme() -> Theme {
    THEME
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Theme type selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeType {
    #[default]
    Dark,
    Light,
    /// Disable all UI colors (equivalent to `NO_COLOR=1` / `--no-colors`)
    NoColor,
    /// Midnight colors regardless of terminal color presets, RGB values only
    Midnight,
    /// Solarized Dark theme
    SolarizedDark,
    /// Solarized Light theme
    SolarizedLight,
    /// Gruvbox Dark theme
    GruvboxDark,
    /// Gruvbox Light theme
    GruvboxLight,
    /// Catppuccin Mocha (dark) theme
    CatppuccinMocha,
    /// Catppuccin Latte (light) theme
    CatppuccinLatte,
    /// Tokyo Night (dark) theme
    TokyoNightDark,
    /// Tokyo Night (light) theme
    TokyoNightLight,
}

impl ThemeType {
    /// Get the display name of this theme
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            ThemeType::Dark => "Dark",
            ThemeType::Light => "Light",
            ThemeType::NoColor => "No Color",
            ThemeType::Midnight => "Midnight",
            ThemeType::SolarizedDark => "Solarized Dark",
            ThemeType::SolarizedLight => "Solarized Light",
            ThemeType::GruvboxDark => "Gruvbox Dark",
            ThemeType::GruvboxLight => "Gruvbox Light",
            ThemeType::CatppuccinMocha => "Catppuccin Mocha",
            ThemeType::CatppuccinLatte => "Catppuccin Latte",
            ThemeType::TokyoNightDark => "Tokyo Night",
            ThemeType::TokyoNightLight => "Tokyo Night Light",
        }
    }

    /// Get the config string value for this theme
    #[must_use]
    pub fn to_config_string(&self) -> &'static str {
        match self {
            ThemeType::Dark => "dark",
            ThemeType::Light => "light",
            ThemeType::NoColor => "nocolor",
            ThemeType::Midnight => "midnight",
            ThemeType::SolarizedDark => "solarized-dark",
            ThemeType::SolarizedLight => "solarized-light",
            ThemeType::GruvboxDark => "gruvbox-dark",
            ThemeType::GruvboxLight => "gruvbox-light",
            ThemeType::CatppuccinMocha => "catppuccin-mocha",
            ThemeType::CatppuccinLatte => "catppuccin-latte",
            ThemeType::TokyoNightDark => "tokyonight-dark",
            ThemeType::TokyoNightLight => "tokyonight-light",
        }
    }

    /// Get all available themes
    #[must_use]
    pub fn all() -> &'static [ThemeType] {
        &[
            ThemeType::Dark,
            ThemeType::Light,
            ThemeType::Midnight,
            ThemeType::SolarizedDark,
            ThemeType::SolarizedLight,
            ThemeType::GruvboxDark,
            ThemeType::GruvboxLight,
            ThemeType::CatppuccinMocha,
            ThemeType::CatppuccinLatte,
            ThemeType::TokyoNightDark,
            ThemeType::TokyoNightLight,
            ThemeType::NoColor,
        ]
    }
}

impl FromStr for ThemeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "light" => ThemeType::Light,
            "midnight" => ThemeType::Midnight,
            "solarized-dark" | "solarized_dark" | "solarized" => ThemeType::SolarizedDark,
            "solarized-light" | "solarized_light" => ThemeType::SolarizedLight,
            "gruvbox-dark" | "gruvbox_dark" | "gruvbox" => ThemeType::GruvboxDark,
            "gruvbox-light" | "gruvbox_light" => ThemeType::GruvboxLight,
            "catppuccin-mocha" | "catppuccin_mocha" | "catppuccin" => ThemeType::CatppuccinMocha,
            "catppuccin-latte" | "catppuccin_latte" => ThemeType::CatppuccinLatte,
            "tokyonight-dark" | "tokyonight_dark" | "tokyonight" | "tokyo-night"
            | "tokyo_night" => ThemeType::TokyoNightDark,
            "tokyonight-light" | "tokyonight_light" | "tokyo-night-light" | "tokyo_night_light" => {
                ThemeType::TokyoNightLight
            }
            "nocolor" | "no-color" | "no_color" => ThemeType::NoColor,
            _ => ThemeType::Dark,
        })
    }
}

impl std::fmt::Display for ThemeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// Color palette and styling configuration for a TUI application.
///
/// Contains semantic color fields, border configuration, a list highlight symbol,
/// and convenience methods that produce [`Style`] values respecting the active theme.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Which built-in variant this theme was derived from
    pub theme_type: ThemeType,

    // === Primary Colors ===
    /// Main accent color (borders, titles, key UI elements)
    pub primary: Color,
    /// Secondary accent (categories, grouping)
    pub secondary: Color,
    /// Tertiary accent (additional variety)
    pub tertiary: Color,

    // === Semantic Colors ===
    /// Success states (installed, synced, active)
    pub success: Color,
    /// Warning states (needs attention, pending)
    pub warning: Color,
    /// Error states (failed, not installed)
    pub error: Color,

    // === Text Colors ===
    /// Main text color
    pub text: Color,
    /// Muted/secondary text
    pub text_muted: Color,
    /// Dimmed/less prominent text
    pub text_dimmed: Color,
    /// Emphasized text (commands, code, highlights)
    pub text_emphasis: Color,

    // === UI Colors ===
    /// Default border color
    pub border: Color,
    /// Focused/active border color
    pub border_focused: Color,
    /// Selection highlight background
    pub highlight_bg: Color,
    /// Background color (use `Color::Reset` for terminal default)
    pub background: Color,
    /// Dimmed background color for modals
    pub dim_bg: Color,

    // === Border Types ===
    /// Default border type (unfocused)
    pub border_type: BorderType,
    /// Focused border type
    pub border_focused_type: BorderType,
    /// Dialog border type
    pub dialog_border_type: BorderType,

    // === List ===
    /// Symbol shown next to the currently selected list item
    pub list_highlight_symbol: &'static str,
}

impl Theme {
    /// Create a theme from one of the built-in variants.
    #[must_use]
    pub fn new(theme_type: ThemeType) -> Self {
        match theme_type {
            ThemeType::Dark => Self::dark(),
            ThemeType::Light => Self::light(),
            ThemeType::NoColor => Self::no_color(),
            ThemeType::Midnight => Self::midnight(),
            ThemeType::SolarizedDark => Self::solarized_dark(),
            ThemeType::SolarizedLight => Self::solarized_light(),
            ThemeType::GruvboxDark => Self::gruvbox_dark(),
            ThemeType::GruvboxLight => Self::gruvbox_light(),
            ThemeType::CatppuccinMocha => Self::catppuccin_mocha(),
            ThemeType::CatppuccinLatte => Self::catppuccin_latte(),
            ThemeType::TokyoNightDark => Self::tokyonight_dark(),
            ThemeType::TokyoNightLight => Self::tokyonight_light(),
        }
    }

    // -----------------------------------------------------------------------
    // Built-in theme constructors
    // -----------------------------------------------------------------------

    /// Dark theme -- for dark terminal backgrounds (uses ANSI colors)
    #[must_use]
    pub fn dark() -> Self {
        Self {
            theme_type: ThemeType::Dark,

            primary: Color::Cyan,
            secondary: Color::Magenta,
            tertiary: Color::Blue,

            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,

            text: Color::Reset,
            text_muted: Color::DarkGray,
            text_dimmed: Color::Cyan,
            text_emphasis: Color::Yellow,

            border: Color::Cyan,
            border_focused: Color::LightBlue,
            highlight_bg: Color::DarkGray,
            background: Color::Reset,
            dim_bg: Color::Reset,

            border_type: BorderType::Plain,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Light theme -- for light terminal backgrounds (uses ANSI colors)
    #[must_use]
    pub fn light() -> Self {
        Self {
            theme_type: ThemeType::Light,

            primary: Color::Blue,
            secondary: Color::Magenta,
            tertiary: Color::Cyan,

            success: Color::Green,
            warning: Color::Rgb(180, 120, 0),
            error: Color::Red,

            text: Color::Reset,
            text_muted: Color::DarkGray,
            text_dimmed: Color::Cyan,
            text_emphasis: Color::Blue,

            border: Color::DarkGray,
            border_focused: Color::Blue,
            highlight_bg: Color::Gray,
            background: Color::Reset,
            dim_bg: Color::Reset,

            border_type: BorderType::Plain,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// No-color theme -- for terminals where colors should be disabled.
    ///
    /// Style helpers intentionally avoid setting fg/bg so the UI uses the
    /// terminal defaults without emitting color codes.
    #[must_use]
    pub fn no_color() -> Self {
        Self {
            theme_type: ThemeType::NoColor,

            primary: Color::Reset,
            secondary: Color::Reset,
            tertiary: Color::Reset,

            success: Color::Reset,
            warning: Color::Reset,
            error: Color::Reset,

            text: Color::Reset,
            text_muted: Color::Reset,
            text_dimmed: Color::Reset,
            text_emphasis: Color::Reset,

            border: Color::Reset,
            border_focused: Color::Reset,
            highlight_bg: Color::Reset,
            background: Color::Reset,
            dim_bg: Color::Reset,

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Midnight theme -- unified RGB color palette
    #[must_use]
    pub fn midnight() -> Self {
        Self {
            theme_type: ThemeType::Midnight,

            primary: Color::Rgb(0, 150, 200),
            secondary: Color::Rgb(170, 50, 170),
            tertiary: Color::Rgb(60, 100, 200),

            success: Color::Rgb(40, 160, 60),
            warning: Color::Rgb(200, 130, 0),
            error: Color::Rgb(200, 40, 40),

            text: Color::Rgb(200, 200, 200),
            text_muted: Color::Rgb(128, 128, 128),
            text_dimmed: Color::Rgb(100, 100, 100),
            text_emphasis: Color::Rgb(220, 140, 0),

            border: Color::Rgb(100, 100, 100),
            border_focused: Color::Rgb(0, 150, 200),
            highlight_bg: Color::Rgb(60, 60, 60),
            background: Color::Rgb(20, 20, 20),
            dim_bg: Color::Rgb(40, 40, 40),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Solarized Dark theme
    #[must_use]
    pub fn solarized_dark() -> Self {
        Self {
            theme_type: ThemeType::SolarizedDark,

            primary: Color::Rgb(38, 139, 210),
            secondary: Color::Rgb(108, 113, 196),
            tertiary: Color::Rgb(42, 161, 152),

            success: Color::Rgb(133, 153, 0),
            warning: Color::Rgb(181, 137, 0),
            error: Color::Rgb(220, 50, 47),

            text: Color::Rgb(131, 148, 150),
            text_muted: Color::Rgb(88, 110, 117),
            text_dimmed: Color::Rgb(7, 54, 66),
            text_emphasis: Color::Rgb(147, 161, 161),

            border: Color::Rgb(88, 110, 117),
            border_focused: Color::Rgb(38, 139, 210),
            highlight_bg: Color::Rgb(7, 54, 66),
            background: Color::Rgb(0, 43, 54),
            dim_bg: Color::Rgb(7, 54, 66),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Solarized Light theme
    #[must_use]
    pub fn solarized_light() -> Self {
        Self {
            theme_type: ThemeType::SolarizedLight,

            primary: Color::Rgb(38, 139, 210),
            secondary: Color::Rgb(108, 113, 196),
            tertiary: Color::Rgb(42, 161, 152),

            success: Color::Rgb(133, 153, 0),
            warning: Color::Rgb(203, 75, 22),
            error: Color::Rgb(220, 50, 47),

            text: Color::Rgb(101, 123, 131),
            text_muted: Color::Rgb(147, 161, 161),
            text_dimmed: Color::Rgb(238, 232, 213),
            text_emphasis: Color::Rgb(88, 110, 117),

            border: Color::Rgb(147, 161, 161),
            border_focused: Color::Rgb(38, 139, 210),
            highlight_bg: Color::Rgb(238, 232, 213),
            background: Color::Rgb(253, 246, 227),
            dim_bg: Color::Rgb(238, 232, 213),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Gruvbox Dark theme -- warm, retro colors
    #[must_use]
    pub fn gruvbox_dark() -> Self {
        Self {
            theme_type: ThemeType::GruvboxDark,

            primary: Color::Rgb(215, 153, 33),
            secondary: Color::Rgb(177, 98, 134),
            tertiary: Color::Rgb(69, 133, 136),

            success: Color::Rgb(152, 151, 26),
            warning: Color::Rgb(214, 93, 14),
            error: Color::Rgb(204, 36, 29),

            text: Color::Rgb(235, 219, 178),
            text_muted: Color::Rgb(168, 153, 132),
            text_dimmed: Color::Rgb(102, 92, 84),
            text_emphasis: Color::Rgb(250, 189, 47),

            border: Color::Rgb(102, 92, 84),
            border_focused: Color::Rgb(215, 153, 33),
            highlight_bg: Color::Rgb(60, 56, 54),
            background: Color::Rgb(40, 40, 40),
            dim_bg: Color::Rgb(50, 48, 47),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Gruvbox Light theme -- warm, retro colors on light background
    #[must_use]
    pub fn gruvbox_light() -> Self {
        Self {
            theme_type: ThemeType::GruvboxLight,

            primary: Color::Rgb(175, 58, 3),
            secondary: Color::Rgb(143, 63, 113),
            tertiary: Color::Rgb(7, 102, 120),

            success: Color::Rgb(121, 116, 14),
            warning: Color::Rgb(181, 118, 20),
            error: Color::Rgb(157, 0, 6),

            text: Color::Rgb(60, 56, 54),
            text_muted: Color::Rgb(124, 111, 100),
            text_dimmed: Color::Rgb(189, 174, 147),
            text_emphasis: Color::Rgb(175, 58, 3),

            border: Color::Rgb(189, 174, 147),
            border_focused: Color::Rgb(175, 58, 3),
            highlight_bg: Color::Rgb(235, 219, 178),
            background: Color::Rgb(251, 241, 199),
            dim_bg: Color::Rgb(242, 229, 188),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Catppuccin Mocha theme -- pastel dark theme
    #[must_use]
    pub fn catppuccin_mocha() -> Self {
        Self {
            theme_type: ThemeType::CatppuccinMocha,

            primary: Color::Rgb(137, 180, 250),
            secondary: Color::Rgb(203, 166, 247),
            tertiary: Color::Rgb(180, 190, 254),

            success: Color::Rgb(166, 227, 161),
            warning: Color::Rgb(249, 226, 175),
            error: Color::Rgb(243, 139, 168),

            text: Color::Rgb(205, 214, 244),
            text_muted: Color::Rgb(108, 112, 134),
            text_dimmed: Color::Rgb(88, 91, 112),
            text_emphasis: Color::Rgb(250, 179, 135),

            border: Color::Rgb(69, 71, 90),
            border_focused: Color::Rgb(137, 180, 250),
            highlight_bg: Color::Rgb(49, 50, 68),
            background: Color::Rgb(30, 30, 46),
            dim_bg: Color::Rgb(24, 24, 37),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Catppuccin Latte theme -- pastel light theme
    #[must_use]
    pub fn catppuccin_latte() -> Self {
        Self {
            theme_type: ThemeType::CatppuccinLatte,

            primary: Color::Rgb(30, 102, 245),
            secondary: Color::Rgb(136, 57, 239),
            tertiary: Color::Rgb(114, 135, 253),

            success: Color::Rgb(64, 160, 43),
            warning: Color::Rgb(223, 142, 29),
            error: Color::Rgb(210, 15, 57),

            text: Color::Rgb(76, 79, 105),
            text_muted: Color::Rgb(156, 160, 176),
            text_dimmed: Color::Rgb(188, 192, 204),
            text_emphasis: Color::Rgb(254, 100, 11),

            border: Color::Rgb(188, 192, 204),
            border_focused: Color::Rgb(30, 102, 245),
            highlight_bg: Color::Rgb(204, 208, 218),
            background: Color::Rgb(239, 241, 245),
            dim_bg: Color::Rgb(230, 233, 239),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Tokyo Night (Storm) theme -- dark, inspired by Tokyo city lights
    #[must_use]
    pub fn tokyonight_dark() -> Self {
        Self {
            theme_type: ThemeType::TokyoNightDark,

            primary: Color::Rgb(122, 162, 247),
            secondary: Color::Rgb(187, 154, 247),
            tertiary: Color::Rgb(125, 207, 255),

            success: Color::Rgb(158, 206, 106),
            warning: Color::Rgb(224, 175, 104),
            error: Color::Rgb(247, 118, 142),

            text: Color::Rgb(192, 202, 245),
            text_muted: Color::Rgb(86, 95, 137),
            text_dimmed: Color::Rgb(65, 72, 104),
            text_emphasis: Color::Rgb(255, 158, 100),

            border: Color::Rgb(65, 72, 104),
            border_focused: Color::Rgb(122, 162, 247),
            highlight_bg: Color::Rgb(41, 46, 66),
            background: Color::Rgb(36, 40, 59),
            dim_bg: Color::Rgb(31, 35, 53),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    /// Tokyo Night Day theme -- light variant
    #[must_use]
    pub fn tokyonight_light() -> Self {
        Self {
            theme_type: ThemeType::TokyoNightLight,

            primary: Color::Rgb(46, 125, 233),
            secondary: Color::Rgb(152, 84, 241),
            tertiary: Color::Rgb(0, 113, 151),

            success: Color::Rgb(88, 117, 57),
            warning: Color::Rgb(140, 108, 62),
            error: Color::Rgb(245, 42, 101),

            text: Color::Rgb(55, 96, 191),
            text_muted: Color::Rgb(132, 140, 181),
            text_dimmed: Color::Rgb(196, 200, 218),
            text_emphasis: Color::Rgb(177, 92, 0),

            border: Color::Rgb(196, 200, 218),
            border_focused: Color::Rgb(46, 125, 233),
            highlight_bg: Color::Rgb(196, 200, 218),
            background: Color::Rgb(225, 226, 231),
            dim_bg: Color::Rgb(213, 214, 219),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,

            list_highlight_symbol: "\u{00bb} ",
        }
    }

    // -----------------------------------------------------------------------
    // Builder methods — chainable setters for custom theme construction
    // -----------------------------------------------------------------------

    /// Set the primary accent color.
    #[must_use]
    pub fn with_primary(mut self, color: Color) -> Self {
        self.primary = color;
        self.border_focused = color;
        self
    }

    /// Set the secondary accent color.
    #[must_use]
    pub fn with_secondary(mut self, color: Color) -> Self {
        self.secondary = color;
        self
    }

    /// Set the tertiary accent color.
    #[must_use]
    pub fn with_tertiary(mut self, color: Color) -> Self {
        self.tertiary = color;
        self
    }

    /// Set the background color.
    #[must_use]
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Set the text emphasis color.
    #[must_use]
    pub fn with_emphasis(mut self, color: Color) -> Self {
        self.text_emphasis = color;
        self
    }

    /// Apply this theme as the global theme.
    pub fn apply(self) {
        set_custom_theme(self);
    }

    // -----------------------------------------------------------------------
    // Style helpers
    // -----------------------------------------------------------------------

    /// Style for primary/title text
    #[must_use]
    pub fn title_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for regular text
    #[must_use]
    pub fn text_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().fg(self.text)
    }

    /// Style for muted/secondary text
    #[must_use]
    pub fn muted_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::DIM);
        }
        Style::default().fg(self.text_muted)
    }

    /// Style for emphasized text (commands, code)
    #[must_use]
    pub fn emphasis_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.text_emphasis)
    }

    /// Style for success states
    #[must_use]
    pub fn success_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.success)
    }

    /// Style for warning states
    #[must_use]
    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for error states
    #[must_use]
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for list item highlight (selected row)
    #[must_use]
    pub fn highlight_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED);
        }
        Style::default()
            .fg(self.text_emphasis)
            .bg(self.highlight_bg)
            .add_modifier(Modifier::BOLD)
    }

    /// Get the border style based on focus
    #[must_use]
    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            self.border_focused_style()
        } else {
            self.unfocused_border_style()
        }
    }

    /// Get the border type based on focus
    #[must_use]
    pub fn border_type(&self, focused: bool) -> BorderType {
        if focused {
            self.border_focused_type
        } else {
            self.border_type
        }
    }

    /// Style for focused borders
    #[must_use]
    pub fn border_focused_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.border_focused)
    }

    /// Style for unfocused borders
    #[must_use]
    pub fn unfocused_border_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().fg(self.border)
    }

    /// Style for disabled items
    #[must_use]
    pub fn disabled_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::DIM);
        }
        Style::default().fg(self.text_muted)
    }

    /// Background style
    #[must_use]
    pub fn background_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().bg(self.background)
    }

    /// Dimmed background style for modals
    #[must_use]
    pub fn dim_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().bg(self.dim_bg).fg(self.text_muted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_type_from_str() {
        assert_eq!("dark".parse::<ThemeType>().unwrap(), ThemeType::Dark);
        assert_eq!("light".parse::<ThemeType>().unwrap(), ThemeType::Light);
        assert_eq!("nocolor".parse::<ThemeType>().unwrap(), ThemeType::NoColor);
        assert_eq!("no-color".parse::<ThemeType>().unwrap(), ThemeType::NoColor);
        assert_eq!("no_color".parse::<ThemeType>().unwrap(), ThemeType::NoColor);
        assert_eq!(
            "solarized-dark".parse::<ThemeType>().unwrap(),
            ThemeType::SolarizedDark
        );
        assert_eq!(
            "solarized_dark".parse::<ThemeType>().unwrap(),
            ThemeType::SolarizedDark
        );
        assert_eq!(
            "solarized".parse::<ThemeType>().unwrap(),
            ThemeType::SolarizedDark
        );
        assert_eq!(
            "solarized-light".parse::<ThemeType>().unwrap(),
            ThemeType::SolarizedLight
        );
        assert_eq!(
            "solarized_light".parse::<ThemeType>().unwrap(),
            ThemeType::SolarizedLight
        );
        assert_eq!(
            "gruvbox-dark".parse::<ThemeType>().unwrap(),
            ThemeType::GruvboxDark
        );
        assert_eq!(
            "gruvbox".parse::<ThemeType>().unwrap(),
            ThemeType::GruvboxDark
        );
        assert_eq!(
            "gruvbox-light".parse::<ThemeType>().unwrap(),
            ThemeType::GruvboxLight
        );
        assert_eq!(
            "catppuccin-mocha".parse::<ThemeType>().unwrap(),
            ThemeType::CatppuccinMocha
        );
        assert_eq!(
            "catppuccin".parse::<ThemeType>().unwrap(),
            ThemeType::CatppuccinMocha
        );
        assert_eq!(
            "catppuccin-latte".parse::<ThemeType>().unwrap(),
            ThemeType::CatppuccinLatte
        );
        assert_eq!(
            "tokyonight-dark".parse::<ThemeType>().unwrap(),
            ThemeType::TokyoNightDark
        );
        assert_eq!(
            "tokyonight".parse::<ThemeType>().unwrap(),
            ThemeType::TokyoNightDark
        );
        assert_eq!(
            "tokyo-night".parse::<ThemeType>().unwrap(),
            ThemeType::TokyoNightDark
        );
        assert_eq!(
            "tokyonight-light".parse::<ThemeType>().unwrap(),
            ThemeType::TokyoNightLight
        );
    }

    #[test]
    fn test_no_color_theme_styles_do_not_set_colors() {
        let t = Theme::new(ThemeType::NoColor);
        let s = t.highlight_style();
        assert!(s.fg.is_none());
        assert!(s.bg.is_none());
    }

    #[test]
    fn test_default_list_highlight_symbol() {
        let t = Theme::new(ThemeType::Dark);
        assert_eq!(t.list_highlight_symbol, "\u{00bb} ");
    }

    #[test]
    fn test_set_custom_theme() {
        let mut custom = Theme::new(ThemeType::Midnight);
        custom.list_highlight_symbol = "> ";
        custom.primary = Color::Red;
        set_custom_theme(custom);

        let t = theme();
        assert_eq!(t.list_highlight_symbol, "> ");
        assert_eq!(t.primary, Color::Red);
        assert_eq!(t.theme_type, ThemeType::Midnight);

        // Restore default so other tests are not affected
        init_theme(ThemeType::Dark);
    }

    #[test]
    fn test_all_variants_count() {
        assert_eq!(ThemeType::all().len(), 12);
    }

    #[test]
    fn test_theme_type_display() {
        assert_eq!(ThemeType::CatppuccinMocha.to_string(), "Catppuccin Mocha");
        assert_eq!(ThemeType::Dark.to_string(), "Dark");
    }

    #[test]
    fn test_config_roundtrip() {
        for variant in ThemeType::all() {
            let config = variant.to_config_string();
            let parsed: ThemeType = config.parse().unwrap();
            assert_eq!(*variant, parsed, "roundtrip failed for {config}");
        }
    }
}
