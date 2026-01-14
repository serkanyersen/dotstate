//! Preset keymaps: Standard, Vim, Emacs
//!
//! Each preset provides a complete set of key bindings for all actions.

use super::{Action, KeyBinding};
use serde::{Deserialize, Serialize};

/// Available keymap presets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeymapPreset {
    /// Standard keyboard navigation (arrows, Enter, Esc)
    #[default]
    Standard,
    /// Vim-style navigation (hjkl, etc.)
    Vim,
    /// Emacs-style navigation (Ctrl+N/P, etc.)
    Emacs,
}

impl KeymapPreset {
    /// Get all key bindings for this preset
    pub fn bindings(&self) -> Vec<KeyBinding> {
        match self {
            KeymapPreset::Standard => standard_bindings(),
            KeymapPreset::Vim => vim_bindings(),
            KeymapPreset::Emacs => emacs_bindings(),
        }
    }

    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            KeymapPreset::Standard => "Standard",
            KeymapPreset::Vim => "Vim",
            KeymapPreset::Emacs => "Emacs",
        }
    }
}

/// Standard keyboard bindings (arrows, Enter, Esc)
fn standard_bindings() -> Vec<KeyBinding> {
    vec![
        // Navigation
        KeyBinding::new("up", Action::MoveUp),
        KeyBinding::new("down", Action::MoveDown),
        KeyBinding::new("left", Action::MoveLeft),
        KeyBinding::new("right", Action::MoveRight),
        KeyBinding::new("pageup", Action::PageUp),
        KeyBinding::new("pagedown", Action::PageDown),
        KeyBinding::new("home", Action::GoToTop),
        KeyBinding::new("end", Action::GoToEnd),
        // Selection
        KeyBinding::new("enter", Action::Confirm),
        KeyBinding::new("esc", Action::Cancel),
        KeyBinding::new("space", Action::ToggleSelect),
        KeyBinding::new("ctrl+a", Action::SelectAll),
        KeyBinding::new("ctrl+shift+a", Action::DeselectAll),
        // Global
        KeyBinding::new("q", Action::Quit),
        KeyBinding::new("ctrl+c", Action::Quit),
        KeyBinding::new("?", Action::Help),
        // Actions
        KeyBinding::new("d", Action::Delete),
        KeyBinding::new("e", Action::Edit),
        KeyBinding::new("c", Action::Create),
        KeyBinding::new("/", Action::Search),
        KeyBinding::new("r", Action::Refresh),
        KeyBinding::new("s", Action::CheckStatus),
        KeyBinding::new("shift+s", Action::Sync),
        KeyBinding::new("i", Action::Install),
        KeyBinding::new("ctrl+s", Action::Save),
        KeyBinding::new("b", Action::ToggleBackup),
        // Text editing
        KeyBinding::new("backspace", Action::Backspace),
        KeyBinding::new("delete", Action::DeleteChar),
        // Tab navigation
        KeyBinding::new("tab", Action::NextTab),
        KeyBinding::new("shift+backtab", Action::PrevTab),
        // Scroll (with shift modifier for preview panes)
        KeyBinding::new("shift+up", Action::ScrollUp),
        KeyBinding::new("shift+down", Action::ScrollDown),
        // Yes/No prompts
        KeyBinding::new("y", Action::Yes),
        KeyBinding::new("n", Action::No),
    ]
}

/// Vim-style keyboard bindings (hjkl navigation)
fn vim_bindings() -> Vec<KeyBinding> {
    vec![
        // Navigation - vim style + arrows
        KeyBinding::new("k", Action::MoveUp),
        KeyBinding::new("up", Action::MoveUp),
        KeyBinding::new("j", Action::MoveDown),
        KeyBinding::new("down", Action::MoveDown),
        KeyBinding::new("h", Action::MoveLeft),
        KeyBinding::new("left", Action::MoveLeft),
        KeyBinding::new("l", Action::MoveRight),
        KeyBinding::new("right", Action::MoveRight),
        KeyBinding::new("ctrl+u", Action::PageUp),
        KeyBinding::new("pageup", Action::PageUp),
        KeyBinding::new("ctrl+d", Action::PageDown),
        KeyBinding::new("pagedown", Action::PageDown),
        KeyBinding::new("g", Action::GoToTop), // gg in real vim, but single g works
        KeyBinding::new("home", Action::GoToTop),
        KeyBinding::new("shift+g", Action::GoToEnd),
        KeyBinding::new("end", Action::GoToEnd),
        // Selection
        KeyBinding::new("enter", Action::Confirm),
        KeyBinding::new("esc", Action::Cancel),
        KeyBinding::new("space", Action::ToggleSelect),
        KeyBinding::new("ctrl+a", Action::SelectAll),
        KeyBinding::new("ctrl+shift+a", Action::DeselectAll),
        // Global - vim uses q to quit
        KeyBinding::new("q", Action::Quit),
        KeyBinding::new("ctrl+c", Action::Quit),
        KeyBinding::new("?", Action::Help),
        // Actions
        KeyBinding::new("d", Action::Delete),
        KeyBinding::new("e", Action::Edit),
        KeyBinding::new("o", Action::Create), // 'o' for open/new in vim style
        KeyBinding::new("/", Action::Search),
        KeyBinding::new("r", Action::Refresh),
        KeyBinding::new("s", Action::CheckStatus),
        KeyBinding::new("shift+s", Action::Sync),
        KeyBinding::new("i", Action::Install),
        KeyBinding::new("ctrl+s", Action::Save),
        KeyBinding::new("b", Action::ToggleBackup),
        // Text editing
        KeyBinding::new("backspace", Action::Backspace),
        KeyBinding::new("x", Action::DeleteChar), // vim style delete char
        KeyBinding::new("delete", Action::DeleteChar),
        // Tab navigation
        KeyBinding::new("tab", Action::NextTab),
        KeyBinding::new("shift+backtab", Action::PrevTab),
        // Scroll (vim style)
        KeyBinding::new("ctrl+y", Action::ScrollUp),
        KeyBinding::new("ctrl+e", Action::ScrollDown),
        // Yes/No prompts
        KeyBinding::new("y", Action::Yes),
        KeyBinding::new("n", Action::No),
    ]
}

/// Emacs-style keyboard bindings (Ctrl+N/P navigation)
fn emacs_bindings() -> Vec<KeyBinding> {
    vec![
        // Navigation - emacs style + arrows
        KeyBinding::new("ctrl+p", Action::MoveUp),
        KeyBinding::new("up", Action::MoveUp),
        KeyBinding::new("ctrl+n", Action::MoveDown),
        KeyBinding::new("down", Action::MoveDown),
        KeyBinding::new("ctrl+b", Action::MoveLeft),
        KeyBinding::new("left", Action::MoveLeft),
        KeyBinding::new("ctrl+f", Action::MoveRight),
        KeyBinding::new("right", Action::MoveRight),
        KeyBinding::new("alt+v", Action::PageUp),
        KeyBinding::new("pageup", Action::PageUp),
        KeyBinding::new("ctrl+v", Action::PageDown),
        KeyBinding::new("pagedown", Action::PageDown),
        KeyBinding::new("alt+shift+,", Action::GoToTop), // M-< in emacs
        KeyBinding::new("home", Action::GoToTop),
        KeyBinding::new("alt+shift+.", Action::GoToEnd), // M-> in emacs
        KeyBinding::new("end", Action::GoToEnd),
        // Selection
        KeyBinding::new("enter", Action::Confirm),
        KeyBinding::new("ctrl+g", Action::Cancel), // C-g is cancel in emacs
        KeyBinding::new("esc", Action::Cancel),
        KeyBinding::new("space", Action::ToggleSelect),
        KeyBinding::new("ctrl+a", Action::SelectAll),
        KeyBinding::new("ctrl+shift+a", Action::DeselectAll),
        // Global
        KeyBinding::new("ctrl+x ctrl+c", Action::Quit), // Note: multi-key not supported yet
        KeyBinding::new("q", Action::Quit),
        KeyBinding::new("ctrl+c", Action::Quit),
        KeyBinding::new("ctrl+h", Action::Help),
        KeyBinding::new("?", Action::Help),
        // Actions
        KeyBinding::new("d", Action::Delete), // Use 'd' since Ctrl+D is DeleteChar in Emacs
        KeyBinding::new("ctrl+e", Action::Edit),
        KeyBinding::new("ctrl+o", Action::Create),
        KeyBinding::new("/", Action::Search), // Use / for search (Ctrl+S is used for Save)
        KeyBinding::new("ctrl+r", Action::Refresh),
        KeyBinding::new("ctrl+r", Action::Refresh),
        KeyBinding::new("ctrl+x s", Action::Sync), // Note: multi-key not supported yet
        KeyBinding::new("s", Action::CheckStatus),
        KeyBinding::new("i", Action::Install),
        KeyBinding::new("ctrl+s", Action::Save),
        KeyBinding::new("b", Action::ToggleBackup), // Use 'b' since Ctrl+B is MoveLeft in Emacs
        // Text editing
        KeyBinding::new("backspace", Action::Backspace),
        KeyBinding::new("ctrl+d", Action::DeleteChar), // Forward delete (Emacs standard)
        KeyBinding::new("delete", Action::DeleteChar),
        // Tab navigation
        KeyBinding::new("tab", Action::NextTab),
        KeyBinding::new("shift+backtab", Action::PrevTab),
        // Scroll
        KeyBinding::new("alt+p", Action::ScrollUp),
        KeyBinding::new("alt+n", Action::ScrollDown),
        // Yes/No prompts
        KeyBinding::new("y", Action::Yes),
        KeyBinding::new("n", Action::No),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_names() {
        assert_eq!(KeymapPreset::Standard.name(), "Standard");
        assert_eq!(KeymapPreset::Vim.name(), "Vim");
        assert_eq!(KeymapPreset::Emacs.name(), "Emacs");
    }

    #[test]
    fn test_standard_bindings_complete() {
        let bindings = KeymapPreset::Standard.bindings();
        // Should have navigation bindings
        assert!(bindings.iter().any(|b| b.action == Action::MoveUp));
        assert!(bindings.iter().any(|b| b.action == Action::MoveDown));
        assert!(bindings.iter().any(|b| b.action == Action::Confirm));
        assert!(bindings.iter().any(|b| b.action == Action::Cancel));
        assert!(bindings.iter().any(|b| b.action == Action::Quit));
        assert!(bindings.iter().any(|b| b.action == Action::Help));
    }

    #[test]
    fn test_vim_has_hjkl() {
        let bindings = KeymapPreset::Vim.bindings();
        assert!(bindings
            .iter()
            .any(|b| b.key == "j" && b.action == Action::MoveDown));
        assert!(bindings
            .iter()
            .any(|b| b.key == "k" && b.action == Action::MoveUp));
        assert!(bindings
            .iter()
            .any(|b| b.key == "h" && b.action == Action::MoveLeft));
        assert!(bindings
            .iter()
            .any(|b| b.key == "l" && b.action == Action::MoveRight));
    }

    #[test]
    fn test_emacs_has_ctrl_np() {
        let bindings = KeymapPreset::Emacs.bindings();
        assert!(bindings
            .iter()
            .any(|b| b.key == "ctrl+n" && b.action == Action::MoveDown));
        assert!(bindings
            .iter()
            .any(|b| b.key == "ctrl+p" && b.action == Action::MoveUp));
    }

    #[test]
    fn test_preset_serialization() {
        let preset = KeymapPreset::Vim;
        let json = serde_json::to_string(&preset).unwrap();
        assert_eq!(json, "\"vim\"");
    }

    #[test]
    fn test_preset_deserialization() {
        let preset: KeymapPreset = serde_json::from_str("\"emacs\"").unwrap();
        assert_eq!(preset, KeymapPreset::Emacs);
    }
}
