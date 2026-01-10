//! Theme and style system for DotState
//!
//! Provides consistent styling across the application with support for
//! light and dark themes.

use ratatui::style::{Color, Modifier, Style};
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
    text_emphasis: Color::Yellow,
    border: Color::DarkGray,
    border_focused: Color::Cyan,
    highlight_bg: Color::DarkGray,
    background: Color::Reset,
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
}

impl FromStr for ThemeType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "light" => ThemeType::Light,
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
}

impl Theme {
    pub fn new(theme_type: ThemeType) -> Self {
        match theme_type {
            ThemeType::Dark => Self::dark(),
            ThemeType::Light => Self::light(),
            ThemeType::NoColor => Self::no_color(),
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
            text: Color::White,
            text_muted: Color::DarkGray,
            text_emphasis: Color::Yellow,

            // UI colors
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            highlight_bg: Color::DarkGray,
            background: Color::Reset,
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
            text: Color::Black,
            text_muted: Color::DarkGray,
            text_emphasis: Color::Blue,

            // UI colors
            border: Color::DarkGray,
            border_focused: Color::Blue,
            highlight_bg: Color::Gray,
            background: Color::Reset,
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
            text_emphasis: Color::Reset,

            border: Color::Reset,
            border_focused: Color::Reset,
            highlight_bg: Color::Reset,
            background: Color::Reset,
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

    /// Style for focused borders
    pub fn border_focused_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default().add_modifier(Modifier::BOLD);
        }
        Style::default().fg(self.border_focused)
    }

    /// Style for unfocused borders
    pub fn border_style(&self) -> Style {
        if self.theme_type == ThemeType::NoColor {
            return Style::default();
        }
        Style::default().fg(self.border)
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
