//! Icon provider system for the application.
//!
//! Supports multiple icon sets: NerdFonts, Unicode emojis, and ASCII fallback.
//! Auto-detects terminal capabilities and allows user override via environment variable.

use std::env;

/// Available icon sets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSet {
    /// NerdFonts icons (requires NerdFont-patched font)
    NerdFonts,
    /// Unicode emoji icons (works in most modern terminals)
    Unicode,
    /// ASCII-only fallback (maximum compatibility)
    Ascii,
}

impl IconSet {
    /// Detect the best icon set for the current terminal
    pub fn detect() -> Self {
        // Check for explicit user override
        if let Ok(icons) = env::var("DOTSTATE_ICONS") {
            return match icons.to_lowercase().as_str() {
                "nerd" | "nerdfont" | "nerdfonts" => IconSet::NerdFonts,
                "unicode" | "emoji" => IconSet::Unicode,
                "ascii" | "plain" => IconSet::Ascii,
                _ => IconSet::Unicode, // Default fallback
            };
        }

        // Try to detect based on terminal type
        if Self::likely_supports_nerd_fonts() {
            IconSet::NerdFonts
        } else {
            IconSet::Unicode // Safe default
        }
    }

    /// Heuristic to detect if terminal likely supports NerdFonts
    fn likely_supports_nerd_fonts() -> bool {
        // Check TERM_PROGRAM for known terminals with good font support
        if let Ok(term_program) = env::var("TERM_PROGRAM") {
            matches!(
                term_program.as_str(),
                "iTerm.app" | "WezTerm" | "Alacritty" | "kitty" | "Ghostty"
            )
        } else {
            false
        }
    }

    /// Get the name of this icon set
    pub fn name(&self) -> &'static str {
        match self {
            IconSet::NerdFonts => "NerdFonts",
            IconSet::Unicode => "Unicode",
            IconSet::Ascii => "ASCII",
        }
    }
}

/// Icon provider that returns appropriate icons based on the selected icon set
pub struct Icons {
    icon_set: IconSet,
}

impl Icons {
    /// Create a new icon provider with auto-detection
    pub fn new() -> Self {
        Self {
            icon_set: IconSet::detect(),
        }
    }

    /// Create an icon provider with a specific icon set
    pub fn with_icon_set(icon_set: IconSet) -> Self {
        Self { icon_set }
    }

    /// Create an icon provider from config
    /// Priority: DOTSTATE_ICONS env var > config value > auto-detect
    pub fn from_config(config: &crate::config::Config) -> Self {
        // Environment variable takes precedence
        if env::var("DOTSTATE_ICONS").is_ok() {
            return Self::new(); // Will use env var via detect()
        }

        // Use config value
        Self::with_icon_set(config.get_icon_set())
    }

    /// Get the current icon set
    pub fn icon_set(&self) -> IconSet {
        self.icon_set
    }

    pub fn folder(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{ea83}", //
            IconSet::Unicode => "📁",
            IconSet::Ascii => "[DIR]",
        }
    }

    pub fn file(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f15b}", //
            IconSet::Unicode => "📄",
            IconSet::Ascii => "[FILE]",
        }
    }

    pub fn sync(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f14ce}", //
            IconSet::Unicode => "🔄",
            IconSet::Ascii => "[SYNC]",
        }
    }

    pub fn loading(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f021}", //
            IconSet::Unicode => "⏳",
            IconSet::Ascii => "[LD]",
        }
    }

    pub fn profile(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f007}", //
            IconSet::Unicode => "👤",
            IconSet::Ascii => "[USER]",
        }
    }

    pub fn package(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{eb29}", //
            IconSet::Unicode => "📦",
            IconSet::Ascii => "[PKG]",
        }
    }

    pub fn git(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f1d2}", //
            IconSet::Unicode => "🔧",
            IconSet::Ascii => "[GIT]",
        }
    }

    pub fn update(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f06b0}", //
            IconSet::Unicode => "🎉",
            IconSet::Ascii => "[UPD]",
        }
    }

    pub fn menu(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f0c9}", //
            IconSet::Unicode => "📋",
            IconSet::Ascii => "[MENU]",
        }
    }

    // === Status Icons ===

    pub fn success(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f00c}", //
            IconSet::Unicode => "✅",
            IconSet::Ascii => "[OK]",
        }
    }

    pub fn warning(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f071}", //
            IconSet::Unicode => "⚠️",
            IconSet::Ascii => "[!]",
        }
    }

    pub fn error(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{ebfb}", //
            IconSet::Unicode => "❌",
            IconSet::Ascii => "[X]",
        }
    }

    pub fn info(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f05a}", //
            IconSet::Unicode => "ℹ️",
            IconSet::Ascii => "[i]",
        }
    }

    pub fn lightbulb(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f0eb}", //
            IconSet::Unicode => "💡",
            IconSet::Ascii => "[IDEA]",
        }
    }
    pub fn active_profile(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f005}", // Star or something distinct
            IconSet::Unicode => "⭐",
            IconSet::Ascii => "[*]",
        }
    }

    pub fn inactive_profile(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f111}", // Circle
            IconSet::Unicode => "○",
            IconSet::Ascii => "[ ]",
        }
    }

    pub fn check(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f00c}",
            IconSet::Unicode => "✓",
            IconSet::Ascii => "[x]",
        }
    }

    pub fn uncheck(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => " ",
            IconSet::Unicode => " ",
            IconSet::Ascii => "[ ]",
        }
    }

    pub fn create(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f067}", // Plus
            IconSet::Unicode => "+",
            IconSet::Ascii => "[+]",
        }
    }
}

impl Default for Icons {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_set_detection() {
        let icon_set = IconSet::detect();
        assert!(matches!(
            icon_set,
            IconSet::NerdFonts | IconSet::Unicode | IconSet::Ascii
        ));
    }

    #[test]
    fn test_icons_creation() {
        let icons = Icons::new();
        assert!(!icons.folder().is_empty());
        assert!(!icons.sync().is_empty());
    }

    #[test]
    fn test_all_icon_sets_have_values() {
        for icon_set in [IconSet::NerdFonts, IconSet::Unicode, IconSet::Ascii] {
            let icons = Icons::with_icon_set(icon_set);
            assert!(!icons.folder().is_empty());
            assert!(!icons.sync().is_empty());
            assert!(!icons.profile().is_empty());
            assert!(!icons.package().is_empty());
            assert!(!icons.git().is_empty());
            assert!(!icons.success().is_empty());
            assert!(!icons.warning().is_empty());
            assert!(!icons.error().is_empty());
        }
    }
}
