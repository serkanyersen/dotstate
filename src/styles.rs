//! Theme and style system for `DotState`
//!
//! Provides consistent styling across the application with support for
//! light and dark themes.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::BorderType;
use std::str::FromStr;
use std::sync::RwLock;

/// List selection indicator shown next to the selected item
pub const LIST_HIGHLIGHT_SYMBOL: &str = "» ";

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
});

/// Initialize the global theme (call once at startup, or to update at runtime)
pub fn init_theme(theme_type: ThemeType) {
    // Recover from poison - if a thread panicked while holding the lock,
    // we still want to update the theme rather than propagate the panic
    let mut theme = THEME
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *theme = Theme::new(theme_type);
}

/// Get the current theme
pub fn theme() -> Theme {
    // Recover from poison - theme should always be accessible
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
    /// Midnight colors regardless of Terminal color presets, RGB values only
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

/// Color palette for the application
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme type
    pub theme_type: ThemeType,

    // === Primary Colors ===
    /// Main accent color (borders, titles, key UI elements)
    pub primary: Color,
    /// Secondary accent (profiles, categories)
    pub secondary: Color,
    /// Tertiary accent (additional variety)
    pub tertiary: Color,

    // === Semantic Colors ===
    /// Success states (installed, synced, active)
    pub success: Color,
    /// Warning states (needs attention, pending)
    pub warning: Color,
    /// Error states (not installed, failed)
    pub error: Color,

    // === Text Colors ===
    /// Main text color
    pub text: Color,
    /// Muted/secondary text
    pub text_muted: Color,
    // Dimmed/less prominent text
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
    /// Background color (use Reset for terminal default)
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
}

impl Theme {
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

    /// Midnight theme - unified color palette
    #[must_use]
    pub fn midnight() -> Self {
        Self {
            theme_type: ThemeType::Midnight,

            // Primary colors - balanced mid-tones for visibility on light/dark
            primary: Color::Rgb(0, 150, 200),    // Cerulean Blue
            secondary: Color::Rgb(170, 50, 170), // Medium Orchid
            tertiary: Color::Rgb(60, 100, 200),  // Steel Blue

            // Semantic colors
            success: Color::Rgb(40, 160, 60), // Jungle Green
            warning: Color::Rgb(200, 130, 0), // Dark Orange
            error: Color::Rgb(200, 40, 40),   // Fire Brick

            // Text colors
            text: Color::Rgb(200, 200, 200),       // Light gray
            text_muted: Color::Rgb(128, 128, 128), // Gray works on both
            text_dimmed: Color::Rgb(100, 100, 100),
            text_emphasis: Color::Rgb(220, 140, 0), // Match warning/orange

            // UI colors
            border: Color::Rgb(100, 100, 100),       // Dark Gray
            border_focused: Color::Rgb(0, 150, 200), // Match primary
            highlight_bg: Color::Rgb(60, 60, 60), // Dark gray for selection (assuming text becomes white-ish or readable)
            background: Color::Rgb(20, 20, 20),
            dim_bg: Color::Rgb(40, 40, 40),

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Solarized Dark theme
    #[must_use]
    pub fn solarized_dark() -> Self {
        Self {
            theme_type: ThemeType::SolarizedDark,

            // Solarized Palette (Dark)
            // Base03  #002b36 (Background)
            // Base02  #073642 (Background Highlights)
            // Base01  #586e75 (Content Emphasis)
            // Base00  #657b83 (Content)
            // Base0   #839496 (Content)
            // Base1   #93a1a1 (Content Emphasis)
            // Base2   #eee8d5 (Background Highlights - unused in dark)
            // Base3   #fdf6e3 (Background - unused in dark)
            // Yellow  #b58900
            // Orange  #cb4b16
            // Red     #dc322f
            // Magenta #d33682
            // Violet  #6c71c4
            // Blue    #268bd2
            // Cyan    #2aa198
            // Green   #859900

            // Primary colors
            primary: Color::Rgb(38, 139, 210),    // Blue
            secondary: Color::Rgb(108, 113, 196), // Violet
            tertiary: Color::Rgb(42, 161, 152),   // Cyan

            // Semantic colors
            success: Color::Rgb(133, 153, 0), // Green
            warning: Color::Rgb(181, 137, 0), // Yellow
            error: Color::Rgb(220, 50, 47),   // Red

            // Text colors
            text: Color::Rgb(131, 148, 150),          // Base0
            text_muted: Color::Rgb(88, 110, 117),     // Base01
            text_dimmed: Color::Rgb(7, 54, 66),       // Base02
            text_emphasis: Color::Rgb(147, 161, 161), // Base1

            // UI colors
            border: Color::Rgb(88, 110, 117),         // Base01
            border_focused: Color::Rgb(38, 139, 210), // Blue
            highlight_bg: Color::Rgb(7, 54, 66),      // Base02
            background: Color::Rgb(0, 43, 54),        // Base03
            dim_bg: Color::Rgb(7, 54, 66),            // Base02

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Solarized Light theme
    #[must_use]
    pub fn solarized_light() -> Self {
        Self {
            theme_type: ThemeType::SolarizedLight,

            // Solarized Palette (Light)
            // Base3   #fdf6e3 (Background)
            // Base2   #eee8d5 (Background Highlights)
            // Base1   #93a1a1 (Content Emphasis)
            // Base0   #839496 (Content)
            // Base00  #657b83 (Content)
            // Base01  #586e75 (Content Emphasis)
            // Base02  #073642 (Background Highlights - unused in light)
            // Base03  #002b36 (Background - unused in light)

            // Primary colors
            primary: Color::Rgb(38, 139, 210),    // Blue
            secondary: Color::Rgb(108, 113, 196), // Violet
            tertiary: Color::Rgb(42, 161, 152),   // Cyan

            // Semantic colors
            success: Color::Rgb(133, 153, 0), // Green
            warning: Color::Rgb(203, 75, 22), // Orange (better visibility on light)
            error: Color::Rgb(220, 50, 47),   // Red

            // Text colors
            text: Color::Rgb(101, 123, 131),         // Base00
            text_muted: Color::Rgb(147, 161, 161),   // Base1
            text_dimmed: Color::Rgb(238, 232, 213),  // Base2
            text_emphasis: Color::Rgb(88, 110, 117), // Base01

            // UI colors
            border: Color::Rgb(147, 161, 161),        // Base1
            border_focused: Color::Rgb(38, 139, 210), // Blue
            highlight_bg: Color::Rgb(238, 232, 213),  // Base2
            background: Color::Rgb(253, 246, 227),    // Base3
            dim_bg: Color::Rgb(238, 232, 213),        // Base2

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Gruvbox Dark theme - warm, retro colors
    #[must_use]
    pub fn gruvbox_dark() -> Self {
        Self {
            theme_type: ThemeType::GruvboxDark,

            // Gruvbox Palette (Dark)
            // bg0:  #282828  bg1: #3c3836  bg2: #504945  bg3: #665c54
            // fg0:  #fbf1c7  fg1: #ebdbb2  fg4: #a89984
            // Bright: red #fb4934  green #b8bb26  yellow #fabd2f
            //         blue #83a598  purple #d3869b  aqua #8ec07c  orange #fe8019
            primary: Color::Rgb(215, 153, 33),   // yellow #d79921
            secondary: Color::Rgb(177, 98, 134), // purple #b16286
            tertiary: Color::Rgb(69, 133, 136),  // blue #458588

            success: Color::Rgb(152, 151, 26), // green #98971a
            warning: Color::Rgb(214, 93, 14),  // orange #d65d0e
            error: Color::Rgb(204, 36, 29),    // red #cc241d

            text: Color::Rgb(235, 219, 178),         // fg1 #ebdbb2
            text_muted: Color::Rgb(168, 153, 132),   // fg4 #a89984
            text_dimmed: Color::Rgb(102, 92, 84),    // bg3 #665c54
            text_emphasis: Color::Rgb(250, 189, 47), // bright yellow #fabd2f

            border: Color::Rgb(102, 92, 84),          // bg3 #665c54
            border_focused: Color::Rgb(215, 153, 33), // yellow #d79921
            highlight_bg: Color::Rgb(60, 56, 54),     // bg1 #3c3836
            background: Color::Rgb(40, 40, 40),       // bg0 #282828
            dim_bg: Color::Rgb(50, 48, 47),           // bg0_s #32302f

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Gruvbox Light theme - warm, retro colors on light background
    #[must_use]
    pub fn gruvbox_light() -> Self {
        Self {
            theme_type: ThemeType::GruvboxLight,

            // Gruvbox Palette (Light)
            // bg0:  #fbf1c7  bg1: #ebdbb2  bg2: #d5c4a1  bg3: #bdae93
            // fg0:  #282828  fg1: #3c3836  fg4: #7c6f64
            primary: Color::Rgb(175, 58, 3), // orange-dark #af3a03
            secondary: Color::Rgb(143, 63, 113), // purple-dark #8f3f71
            tertiary: Color::Rgb(7, 102, 120), // blue-dark #076678

            success: Color::Rgb(121, 116, 14), // green-dark #79740e
            warning: Color::Rgb(181, 118, 20), // yellow-dark #b57614
            error: Color::Rgb(157, 0, 6),      // red-dark #9d0006

            text: Color::Rgb(60, 56, 54),           // fg1 #3c3836
            text_muted: Color::Rgb(124, 111, 100),  // fg4 #7c6f64
            text_dimmed: Color::Rgb(189, 174, 147), // bg3 #bdae93
            text_emphasis: Color::Rgb(175, 58, 3),  // orange-dark #af3a03

            border: Color::Rgb(189, 174, 147),       // bg3 #bdae93
            border_focused: Color::Rgb(175, 58, 3),  // orange-dark #af3a03
            highlight_bg: Color::Rgb(235, 219, 178), // bg1 #ebdbb2
            background: Color::Rgb(251, 241, 199),   // bg0 #fbf1c7
            dim_bg: Color::Rgb(242, 229, 188),       // bg0_s #f2e5bc

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Catppuccin Mocha theme - pastel dark theme
    #[must_use]
    pub fn catppuccin_mocha() -> Self {
        Self {
            theme_type: ThemeType::CatppuccinMocha,

            // Catppuccin Mocha Palette
            // Base: #1e1e2e  Mantle: #181825  Crust: #11111b
            // Surface0: #313244  Surface1: #45475a  Surface2: #585b70
            // Overlay0: #6c7086  Subtext0: #a6adc8  Subtext1: #bac2de
            // Text: #cdd6f4
            // Blue: #89b4fa  Lavender: #b4befe  Mauve: #cba6f7
            // Red: #f38ba8  Peach: #fab387  Yellow: #f9e2af
            // Green: #a6e3a1  Teal: #94e2d5  Sky: #89dceb
            primary: Color::Rgb(137, 180, 250),   // Blue #89b4fa
            secondary: Color::Rgb(203, 166, 247), // Mauve #cba6f7
            tertiary: Color::Rgb(180, 190, 254),  // Lavender #b4befe

            success: Color::Rgb(166, 227, 161), // Green #a6e3a1
            warning: Color::Rgb(249, 226, 175), // Yellow #f9e2af
            error: Color::Rgb(243, 139, 168),   // Red #f38ba8

            text: Color::Rgb(205, 214, 244),          // Text #cdd6f4
            text_muted: Color::Rgb(108, 112, 134),    // Overlay0 #6c7086
            text_dimmed: Color::Rgb(88, 91, 112),     // Surface2 #585b70
            text_emphasis: Color::Rgb(250, 179, 135), // Peach #fab387

            border: Color::Rgb(69, 71, 90), // Surface1 #45475a
            border_focused: Color::Rgb(137, 180, 250), // Blue #89b4fa
            highlight_bg: Color::Rgb(49, 50, 68), // Surface0 #313244
            background: Color::Rgb(30, 30, 46), // Base #1e1e2e
            dim_bg: Color::Rgb(24, 24, 37), // Mantle #181825

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Catppuccin Latte theme - pastel light theme
    #[must_use]
    pub fn catppuccin_latte() -> Self {
        Self {
            theme_type: ThemeType::CatppuccinLatte,

            // Catppuccin Latte Palette
            // Base: #eff1f5  Mantle: #e6e9ef  Crust: #dce0e8
            // Surface0: #ccd0da  Surface1: #bcc0cc  Surface2: #acb0be
            // Overlay0: #9ca0b0  Subtext0: #6c6f85  Subtext1: #5c5f77
            // Text: #4c4f69
            // Blue: #1e66f5  Lavender: #7287fd  Mauve: #8839ef
            // Red: #d20f39  Peach: #fe640b  Yellow: #df8e1d
            // Green: #40a02b  Teal: #179299  Sky: #04a5e5
            primary: Color::Rgb(30, 102, 245),   // Blue #1e66f5
            secondary: Color::Rgb(136, 57, 239), // Mauve #8839ef
            tertiary: Color::Rgb(114, 135, 253), // Lavender #7287fd

            success: Color::Rgb(64, 160, 43),  // Green #40a02b
            warning: Color::Rgb(223, 142, 29), // Yellow #df8e1d
            error: Color::Rgb(210, 15, 57),    // Red #d20f39

            text: Color::Rgb(76, 79, 105),           // Text #4c4f69
            text_muted: Color::Rgb(156, 160, 176),   // Overlay0 #9ca0b0
            text_dimmed: Color::Rgb(188, 192, 204),  // Surface1 #bcc0cc
            text_emphasis: Color::Rgb(254, 100, 11), // Peach #fe640b

            border: Color::Rgb(188, 192, 204), // Surface1 #bcc0cc
            border_focused: Color::Rgb(30, 102, 245), // Blue #1e66f5
            highlight_bg: Color::Rgb(204, 208, 218), // Surface0 #ccd0da
            background: Color::Rgb(239, 241, 245), // Base #eff1f5
            dim_bg: Color::Rgb(230, 233, 239), // Mantle #e6e9ef

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Tokyo Night (Storm) theme - dark, inspired by Tokyo city lights
    #[must_use]
    pub fn tokyonight_dark() -> Self {
        Self {
            theme_type: ThemeType::TokyoNightDark,

            // Tokyo Night Storm Palette
            // bg: #24283b  bg_dark: #1f2335  bg_highlight: #292e42
            // terminal_black: #414868  fg: #c0caf5  fg_dark: #a9b1d6
            // comment: #565f89
            // blue: #7aa2f7  cyan: #7dcfff  magenta: #bb9af7  orange: #ff9e64
            // red: #f7768e  yellow: #e0af68  green: #9ece6a  teal: #73daca
            primary: Color::Rgb(122, 162, 247),   // blue #7aa2f7
            secondary: Color::Rgb(187, 154, 247), // magenta #bb9af7
            tertiary: Color::Rgb(125, 207, 255),  // cyan #7dcfff

            success: Color::Rgb(158, 206, 106), // green #9ece6a
            warning: Color::Rgb(224, 175, 104), // yellow #e0af68
            error: Color::Rgb(247, 118, 142),   // red #f7768e

            text: Color::Rgb(192, 202, 245),          // fg #c0caf5
            text_muted: Color::Rgb(86, 95, 137),      // comment #565f89
            text_dimmed: Color::Rgb(65, 72, 104),     // terminal_black #414868
            text_emphasis: Color::Rgb(255, 158, 100), // orange #ff9e64

            border: Color::Rgb(65, 72, 104), // terminal_black #414868
            border_focused: Color::Rgb(122, 162, 247), // blue #7aa2f7
            highlight_bg: Color::Rgb(41, 46, 66), // bg_highlight #292e42
            background: Color::Rgb(36, 40, 59), // bg #24283b
            dim_bg: Color::Rgb(31, 35, 53),  // bg_dark #1f2335

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Tokyo Night Day theme - light variant
    #[must_use]
    pub fn tokyonight_light() -> Self {
        Self {
            theme_type: ThemeType::TokyoNightLight,

            // Tokyo Night Day Palette
            // bg: #e1e2e7  bg_dark: #d5d6db  bg_highlight: #c4c8da
            // fg: #3760bf  fg_dark: #6172b0  comment: #848cb5
            // blue: #2e7de9  cyan: #007197  magenta: #9854f1  orange: #b15c00
            // red: #f52a65  yellow: #8c6c3e  green: #587539  teal: #118c74
            primary: Color::Rgb(46, 125, 233),   // blue #2e7de9
            secondary: Color::Rgb(152, 84, 241), // magenta #9854f1
            tertiary: Color::Rgb(0, 113, 151),   // cyan #007197

            success: Color::Rgb(88, 117, 57),  // green #587539
            warning: Color::Rgb(140, 108, 62), // yellow #8c6c3e
            error: Color::Rgb(245, 42, 101),   // red #f52a65

            text: Color::Rgb(55, 96, 191),          // fg #3760bf
            text_muted: Color::Rgb(132, 140, 181),  // comment #848cb5
            text_dimmed: Color::Rgb(196, 200, 218), // bg_highlight #c4c8da
            text_emphasis: Color::Rgb(177, 92, 0),  // orange #b15c00

            border: Color::Rgb(196, 200, 218), // bg_highlight #c4c8da
            border_focused: Color::Rgb(46, 125, 233), // blue #2e7de9
            highlight_bg: Color::Rgb(196, 200, 218), // bg_highlight #c4c8da
            background: Color::Rgb(225, 226, 231), // bg #e1e2e7
            dim_bg: Color::Rgb(213, 214, 219), // bg_dark #d5d6db

            border_type: BorderType::Rounded,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Dark theme - for dark terminal backgrounds
    #[must_use]
    pub fn dark() -> Self {
        Self {
            theme_type: ThemeType::Dark,

            // Primary colors - cyan family for main accents
            primary: Color::Cyan,
            secondary: Color::Magenta,
            tertiary: Color::Blue,

            // Semantic colors
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,

            // Text colors
            text: Color::Reset,
            text_muted: Color::DarkGray,
            text_dimmed: Color::Cyan,
            text_emphasis: Color::Yellow,

            // UI colors
            border: Color::Cyan,
            border_focused: Color::LightBlue,
            highlight_bg: Color::DarkGray,
            background: Color::Reset,
            dim_bg: Color::Reset,

            border_type: BorderType::Plain,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Light theme - for light terminal backgrounds
    #[must_use]
    pub fn light() -> Self {
        Self {
            theme_type: ThemeType::Light,

            // Primary colors - darker variants for light backgrounds
            primary: Color::Blue,
            secondary: Color::Magenta,
            tertiary: Color::Cyan,

            // Semantic colors - darker/more saturated for visibility
            success: Color::Green,
            warning: Color::Rgb(180, 120, 0), // Darker yellow/orange
            error: Color::Red,

            // Text colors - dark text on light background
            text: Color::Reset,
            text_muted: Color::DarkGray,
            text_dimmed: Color::Cyan,
            text_emphasis: Color::Blue,

            // UI colors
            border: Color::DarkGray,
            border_focused: Color::Blue,
            highlight_bg: Color::Gray,
            background: Color::Reset,
            dim_bg: Color::Reset,

            border_type: BorderType::Plain,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// No-color theme - for terminals where colors should be disabled
    ///
    /// Note: In this mode, style helpers below intentionally avoid setting fg/bg
    /// so the UI uses the terminal defaults without emitting color codes.
    #[must_use]
    pub fn no_color() -> Self {
        Self {
            theme_type: ThemeType::NoColor,

            // These palette values are effectively unused by the style helpers in NoColor mode.
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
        }
    }

    // === Style Helpers ===

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
    #[allow(dead_code)]
    #[must_use]
    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for error states
    #[allow(dead_code)]
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
        // In no-color mode we rely on modifiers only, not fg/bg.
        assert!(s.fg.is_none());
        assert!(s.bg.is_none());
    }
}
