//! Syntax theme selection utilities for syntax highlighting.
//!
//! This module provides a unified way to select syntax themes based on
//! the current UI theme type, avoiding duplication across the codebase.

use crate::styles::ThemeType;
use syntect::highlighting::{Theme, ThemeSet};

/// Get the appropriate syntax highlighting theme based on the current UI theme.
///
/// This function selects a syntax theme that matches the current UI theme type
/// (light, dark, or no-color mode). It tries preferred themes in order and
/// falls back to any available theme if none of the preferred ones are found.
///
/// # Arguments
///
/// * `theme_set` - The syntect `ThemeSet` containing available themes
/// * `theme_type` - The current UI theme type (Light, Dark, or `NoColor`)
///
/// # Returns
///
/// A reference to the selected syntax highlighting theme.
///
/// # Panics
///
/// Panics if no syntect themes are available at all (should never happen
/// with default themes loaded).
#[must_use]
pub fn get_syntax_theme(theme_set: &ThemeSet, theme_type: ThemeType) -> &Theme {
    let preferred_names = match theme_type {
        ThemeType::Light => vec!["base16-ocean.light", "Solarized (light)", "GitHub"],
        ThemeType::SolarizedDark => vec!["Solarized (dark)", "base16-ocean.dark"],
        ThemeType::SolarizedLight => vec!["Solarized (light)", "base16-ocean.light"],
        ThemeType::Dark | ThemeType::NoColor | ThemeType::Midnight => vec![
            "base16-ocean.dark",
            "base16-eighties.dark",
            "base16-mocha.dark",
            "InspiredGitHub",
        ],
    };

    for name in &preferred_names {
        if let Some(theme) = theme_set.themes.get(*name) {
            return theme;
        }
    }

    theme_set
        .themes
        .values()
        .next()
        .expect("No syntect themes available")
}

/// Get the syntax theme using the current global UI theme.
///
/// This is a convenience function that automatically retrieves the current
/// UI theme type from the global theme settings.
///
/// # Arguments
///
/// * `theme_set` - The syntect `ThemeSet` containing available themes
///
/// # Returns
///
/// A reference to the selected syntax highlighting theme.
#[must_use]
pub fn get_current_syntax_theme(theme_set: &ThemeSet) -> &Theme {
    use crate::styles::theme as ui_theme;
    let theme_type = ui_theme().theme_type;
    get_syntax_theme(theme_set, theme_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_syntax_theme_dark() {
        let theme_set = ThemeSet::load_defaults();
        let theme = get_syntax_theme(&theme_set, ThemeType::Dark);
        // Should not panic and return a valid theme
        assert!(!theme
            .name
            .as_ref()
            .is_none_or(std::string::String::is_empty));
    }

    #[test]
    fn test_get_syntax_theme_light() {
        let theme_set = ThemeSet::load_defaults();
        let theme = get_syntax_theme(&theme_set, ThemeType::Light);
        // Should not panic and return a valid theme
        assert!(!theme
            .name
            .as_ref()
            .is_none_or(std::string::String::is_empty));
    }

    #[test]
    fn test_get_syntax_theme_nocolor() {
        let theme_set = ThemeSet::load_defaults();
        let theme = get_syntax_theme(&theme_set, ThemeType::NoColor);
        // NoColor should fall back to dark theme preferences
        assert!(!theme
            .name
            .as_ref()
            .is_none_or(std::string::String::is_empty));
    }

    #[test]
    fn test_get_syntax_theme_solarized() {
        let theme_set = ThemeSet::load_defaults();

        let theme_dark = get_syntax_theme(&theme_set, ThemeType::SolarizedDark);
        assert!(!theme_dark
            .name
            .as_ref()
            .is_none_or(std::string::String::is_empty));

        let theme_light = get_syntax_theme(&theme_set, ThemeType::SolarizedLight);
        assert!(!theme_light
            .name
            .as_ref()
            .is_none_or(std::string::String::is_empty));
    }
}
