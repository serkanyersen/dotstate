//! Icon provider system for TUI applications.
//!
//! Supports multiple icon sets: `NerdFonts`, Unicode, Emoji, and ASCII fallback.
//! Auto-detects terminal capabilities and allows user override via environment variable.
//!
//! # Example
//!
//! ```rust
//! use tui_forge::icons::{Icons, IconSet};
//!
//! // Auto-detect from TUI_FORGE_ICONS env var or terminal capabilities
//! let icons = Icons::new();
//!
//! // Use a specific icon set
//! let icons = Icons::with_icon_set(IconSet::Ascii);
//!
//! // Use a custom env var name (e.g. for your own app: MYAPP_ICONS)
//! let icons = Icons::with_env_var("MYAPP_ICONS");
//!
//! println!("Success: {}", icons.success());
//! ```

use std::env;

/// The default environment variable name used to override the icon set.
const DEFAULT_ENV_VAR: &str = "TUI_FORGE_ICONS";

/// Available icon sets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSet {
    /// `NerdFonts` icons (requires NerdFont-patched font)
    NerdFonts,
    /// Unicode icons (works in most modern terminals)
    Unicode,
    /// Emoji icons (works in most modern terminals)
    Emoji,
    /// ASCII-only fallback (maximum compatibility)
    Ascii,
}

impl IconSet {
    /// Detect the best icon set for the current terminal,
    /// checking the given environment variable for an explicit override.
    #[must_use]
    pub fn detect() -> Self {
        Self::detect_with_env(DEFAULT_ENV_VAR)
    }

    /// Detect the best icon set, using a custom environment variable name
    /// for the user override.
    #[must_use]
    pub fn detect_with_env(env_var: &str) -> Self {
        // Check for explicit user override
        if let Ok(icons) = env::var(env_var) {
            return match icons.to_lowercase().as_str() {
                "nerd" | "nerdfont" | "nerdfonts" => IconSet::NerdFonts,
                "unicode" => IconSet::Unicode,
                "emoji" => IconSet::Emoji,
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

    /// Heuristic to detect if terminal likely supports `NerdFonts`
    #[must_use]
    pub fn likely_supports_nerd_fonts() -> bool {
        // Check TERM_PROGRAM for known terminals with good font support
        if let Ok(term_program) = env::var("TERM_PROGRAM") {
            matches!(
                term_program.as_str(),
                "iTerm.app" | "WezTerm" | "Alacritty" | "kitty" | "Ghostty" | "Hyper" | "Tabby"
            )
        } else {
            false
        }
    }

    /// Get the name of this icon set
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            IconSet::NerdFonts => "NerdFonts",
            IconSet::Unicode => "Unicode",
            IconSet::Emoji => "Emoji",
            IconSet::Ascii => "ASCII",
        }
    }
}

/// Icon provider that returns appropriate icons based on the selected icon set.
///
/// Construct via [`Icons::new`] for auto-detection (using the default
/// `TUI_FORGE_ICONS` env var), [`Icons::with_icon_set`] for a specific set,
/// or [`Icons::with_env_var`] to auto-detect using a custom env var name.
pub struct Icons {
    icon_set: IconSet,
}

impl Icons {
    /// Create a new icon provider with auto-detection.
    ///
    /// Checks the `TUI_FORGE_ICONS` environment variable first, then falls
    /// back to terminal capability detection.
    #[must_use]
    pub fn new() -> Self {
        Self {
            icon_set: IconSet::detect(),
        }
    }

    /// Create an icon provider with a specific icon set.
    #[must_use]
    pub fn with_icon_set(icon_set: IconSet) -> Self {
        Self { icon_set }
    }

    /// Create an icon provider that reads the override from a custom
    /// environment variable instead of the default `TUI_FORGE_ICONS`.
    ///
    /// This lets applications define their own env var name, e.g.
    /// `Icons::with_env_var("MYAPP_ICONS")`.
    #[must_use]
    pub fn with_env_var(env_var: &str) -> Self {
        Self {
            icon_set: IconSet::detect_with_env(env_var),
        }
    }

    /// Get the current icon set.
    #[must_use]
    pub fn icon_set(&self) -> IconSet {
        self.icon_set
    }

    // === Object Icons ===

    #[must_use]
    pub fn folder(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{ea83}", //
            IconSet::Unicode => "\u{25b6}",   // ▶
            IconSet::Emoji => "\u{1f4c1}",    // 📁
            IconSet::Ascii => "[DIR]",
        }
    }

    #[must_use]
    pub fn file(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f15b}", //
            IconSet::Unicode => "\u{25c7}",   // ◇
            IconSet::Emoji => "\u{1f4c4}",    // 📄
            IconSet::Ascii => "[FILE]",
        }
    }

    #[must_use]
    pub fn sync(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f14ce}", //
            IconSet::Unicode => "\u{21bb}",    // ↻
            IconSet::Emoji => "\u{1f504}",     // 🔄
            IconSet::Ascii => "[SYNC]",
        }
    }

    #[must_use]
    pub fn loading(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f021}", //
            IconSet::Unicode => "\u{25cc}",   // ◌
            IconSet::Emoji => "\u{231b}",     // ⏳
            IconSet::Ascii => "[LD]",
        }
    }

    #[must_use]
    pub fn profile(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f007}", //
            IconSet::Unicode => "\u{25c9}",   // ◉
            IconSet::Emoji => "\u{1f464}",    // 👤
            IconSet::Ascii => "[USR]",
        }
    }

    #[must_use]
    pub fn package(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{eb29}", //
            IconSet::Unicode => "\u{25c6}",   // ◆
            IconSet::Emoji => "\u{1f4e6}",    // 📦
            IconSet::Ascii => "[PKG]",
        }
    }

    #[must_use]
    pub fn git(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f1d2}", //
            IconSet::Unicode => "\u{2387}",   // ⎇
            IconSet::Emoji => "\u{1f527}",    // 🔧
            IconSet::Ascii => "[GIT]",
        }
    }

    #[must_use]
    pub fn update(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f06b0}", //
            IconSet::Unicode => "\u{2191}",    // ↑
            IconSet::Emoji => "\u{1f389}",     // 🎉
            IconSet::Ascii => "[UPD]",
        }
    }

    #[must_use]
    pub fn menu(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f0c9}", //
            IconSet::Unicode => "\u{2261}",   // ≡
            IconSet::Emoji => "\u{1f4cb}",    // 📋
            IconSet::Ascii => "[MENU]",
        }
    }

    // === Status Icons ===

    #[must_use]
    pub fn success(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f00c}", //
            IconSet::Unicode => "\u{2713}",   // ✓
            IconSet::Emoji => "\u{2705}",     // ✅
            IconSet::Ascii => "[OK]",
        }
    }

    #[must_use]
    pub fn warning(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f071}",     //
            IconSet::Unicode => "\u{26a0}",       // ⚠
            IconSet::Emoji => "\u{26a0}\u{fe0f}", // ⚠️
            IconSet::Ascii => "[!]",
        }
    }

    #[must_use]
    pub fn error(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{ebfb}", //
            IconSet::Unicode => "\u{2717}",   // ✗
            IconSet::Emoji => "\u{274c}",     // ❌
            IconSet::Ascii => "[X]",
        }
    }

    #[must_use]
    pub fn info(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f05a}",     //
            IconSet::Unicode => "\u{2139}",       // ℹ
            IconSet::Emoji => "\u{2139}\u{fe0f}", // ℹ️
            IconSet::Ascii => "[i]",
        }
    }

    #[must_use]
    pub fn lightbulb(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f0eb}", //
            IconSet::Unicode => "\u{2606}",   // ☆
            IconSet::Emoji => "\u{1f4a1}",    // 💡
            IconSet::Ascii => "[IDEA]",
        }
    }

    #[must_use]
    pub fn active_profile(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f005}", // Star
            IconSet::Unicode => "\u{2605}",   // ★
            IconSet::Emoji => "\u{2b50}",     // ⭐
            IconSet::Ascii => "[*]",
        }
    }

    #[must_use]
    pub fn inactive_profile(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f111}", // Circle
            IconSet::Unicode => "\u{25cb}",   // ○
            IconSet::Emoji => "\u{25cb}",     // ○
            IconSet::Ascii => "[ ]",
        }
    }

    #[must_use]
    pub fn check(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f42e}",
            IconSet::Unicode => "\u{2713}", // ✓
            IconSet::Emoji => "\u{2705}",   // ✅
            IconSet::Ascii => "[x]",
        }
    }

    #[must_use]
    pub fn uncheck(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => " ",
            IconSet::Unicode => " ",
            IconSet::Emoji => " ",
            IconSet::Ascii => "[ ]",
        }
    }

    #[must_use]
    pub fn create(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f067}", // Plus
            IconSet::Unicode => "+",
            IconSet::Emoji => "\u{1f195}", // 🆕
            IconSet::Ascii => "[+]",
        }
    }

    #[must_use]
    pub fn github(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f09b}", // GitHub logo
            IconSet::Unicode => "\u{2387}",   // ⎇
            IconSet::Emoji => "\u{1f527}",    // 🔧
            IconSet::Ascii => "[GH]",
        }
    }

    #[must_use]
    pub fn wrench(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f0ad}", // Wrench
            IconSet::Unicode => "\u{2692}",   // ⚒
            IconSet::Emoji => "\u{1f527}",    // 🔧
            IconSet::Ascii => "[TOOL]",
        }
    }

    #[must_use]
    pub fn plug(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f1e6}", // Plug
            IconSet::Unicode => "\u{2318}",   // ⌘
            IconSet::Emoji => "\u{1f50c}",    // 🔌
            IconSet::Ascii => "[CONN]",
        }
    }

    #[must_use]
    pub fn circle_filled(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f111}", // Circle
            IconSet::Unicode => "\u{25cf}",   // ●
            IconSet::Emoji => "\u{25cf}",     // ●
            IconSet::Ascii => "[x]",
        }
    }

    #[must_use]
    pub fn circle_empty(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f1db}", // Circle
            IconSet::Unicode => "\u{25cb}",   // ○
            IconSet::Emoji => "\u{25cb}",     // ○
            IconSet::Ascii => "[ ]",
        }
    }

    #[must_use]
    pub fn cog(&self) -> &'static str {
        match self.icon_set {
            IconSet::NerdFonts => "\u{f013}",     // Cog/gear icon
            IconSet::Unicode => "\u{2699}",       // ⚙
            IconSet::Emoji => "\u{2699}\u{fe0f}", // ⚙️
            IconSet::Ascii => "[*]",
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
            IconSet::NerdFonts | IconSet::Unicode | IconSet::Emoji | IconSet::Ascii
        ));
    }

    #[test]
    fn test_icons_creation() {
        let icons = Icons::new();
        assert!(!icons.folder().is_empty());
        assert!(!icons.sync().is_empty());
    }

    #[test]
    fn test_with_env_var_custom() {
        // When the custom env var is not set, should fall back to detection
        let icons = Icons::with_env_var("TEST_NONEXISTENT_ICONS_VAR_12345");
        assert!(!icons.folder().is_empty());
    }

    #[test]
    fn test_all_icon_sets_have_values() {
        for icon_set in [
            IconSet::NerdFonts,
            IconSet::Unicode,
            IconSet::Emoji,
            IconSet::Ascii,
        ] {
            let icons = Icons::with_icon_set(icon_set);
            assert!(!icons.folder().is_empty());
            assert!(!icons.file().is_empty());
            assert!(!icons.sync().is_empty());
            assert!(!icons.loading().is_empty());
            assert!(!icons.profile().is_empty());
            assert!(!icons.package().is_empty());
            assert!(!icons.git().is_empty());
            assert!(!icons.update().is_empty());
            assert!(!icons.menu().is_empty());
            assert!(!icons.success().is_empty());
            assert!(!icons.warning().is_empty());
            assert!(!icons.error().is_empty());
            assert!(!icons.info().is_empty());
            assert!(!icons.lightbulb().is_empty());
            assert!(!icons.active_profile().is_empty());
            assert!(!icons.check().is_empty());
            assert!(!icons.create().is_empty());
            assert!(!icons.github().is_empty());
            assert!(!icons.wrench().is_empty());
            assert!(!icons.plug().is_empty());
            assert!(!icons.circle_filled().is_empty());
            assert!(!icons.circle_empty().is_empty());
            assert!(!icons.cog().is_empty());
        }
    }

    #[test]
    fn test_icon_set_names() {
        assert_eq!(IconSet::NerdFonts.name(), "NerdFonts");
        assert_eq!(IconSet::Unicode.name(), "Unicode");
        assert_eq!(IconSet::Emoji.name(), "Emoji");
        assert_eq!(IconSet::Ascii.name(), "ASCII");
    }

    #[test]
    fn test_default_impl() {
        let icons = Icons::default();
        assert!(!icons.success().is_empty());
    }
}
