//! Theme and style system for DotState
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
    let mut theme = THEME.write().unwrap();
    *theme = Theme::new(theme_type);
}

/// Get the current theme
pub fn theme() -> Theme {
    THEME.read().unwrap().clone()
}

/// Theme type selector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeType {
    #[default]
    Dark,
    Light,
    /// Disable all UI colors (equivalent to `NO_COLOR=1` / `--no-colors`)
    NoColor,
    /// Fixed colors regardless of Terminal color presets, RGB values only
    Fixed,
}

impl FromStr for ThemeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "light" => ThemeType::Light,
            "fixed" => ThemeType::Fixed,
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
    pub fn new(theme_type: ThemeType) -> Self {
        match theme_type {
            ThemeType::Dark => Self::dark(),
            ThemeType::Light => Self::light(),
            ThemeType::NoColor => Self::no_color(),
            ThemeType::Fixed => Self::fixed(),
        }
    }

    /// Fixed theme - unified color palette
    pub fn fixed() -> Self {
        Self {
            theme_type: ThemeType::Fixed,

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

            border_type: BorderType::Plain,
            border_focused_type: BorderType::Thick,
            dialog_border_type: BorderType::Double,
        }
    }

    /// Dark theme - for dark terminal backgrounds
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
    pub fn title_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    /// Style for regular text
    pub fn text_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().fg(self.text)
    }

    /// Style for muted/secondary text
    pub fn muted_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::DIM);
        }
        Style::default().fg(self.text_muted)
    }

    /// Style for emphasized text (commands, code)
    pub fn emphasis_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.text_emphasis)
    }

    /// Style for success states
    pub fn success_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.success)
    }

    /// Style for warning states
    #[allow(dead_code)]
    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Style for error states
    #[allow(dead_code)]
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }

    /// Style for list item highlight (selected row)
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
    pub fn border_style(&self, focused: bool) -> Style {
        if focused {
            self.border_focused_style()
        } else {
            self.unfocused_border_style()
        }
    }

    /// Get the border type based on focus
    pub fn border_type(&self, focused: bool) -> BorderType {
        if focused {
            self.border_focused_type
        } else {
            self.border_type
        }
    }

    /// Style for focused borders
    pub fn border_focused_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.border_focused)
    }

    /// Style for unfocused borders
    pub fn unfocused_border_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().fg(self.border)
    }

    /// Style for disabled items
    pub fn disabled_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::DIM);
        }
        Style::default().fg(self.text_muted)
    }

    /// Background style
    pub fn background_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().bg(self.background)
    }

    /// Dimmed background style for modals
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
