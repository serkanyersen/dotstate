//! Keymap configuration module
//!
//! Provides customizable keyboard shortcuts with preset keymaps (standard, vim, emacs).

#![allow(dead_code)] // Types are used via Config in the binary, but compiler doesn't see direct usage

mod actions;
mod binding;
mod presets;

pub use actions::Action;
pub use binding::KeyBinding;
pub use presets::KeymapPreset;

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

/// Keymap configuration with preset and optional overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keymap {
    /// Base preset keymap
    #[serde(default)]
    pub preset: KeymapPreset,

    /// User-defined overrides (checked before preset)
    #[serde(default)]
    pub overrides: Vec<KeyBinding>,
}

impl Default for Keymap {
    fn default() -> Self {
        Self {
            preset: KeymapPreset::Standard,
            overrides: Vec::new(),
        }
    }
}

impl Keymap {
    /// Get the action for a key event, checking overrides first then preset
    /// Note: If an action is overridden, preset bindings for that action are ignored
    pub fn get_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        // Use all_bindings which already handles override shadowing
        for binding in self.all_bindings() {
            if binding.matches(code, modifiers) {
                return Some(binding.action);
            }
        }
        None
    }

    /// Get all bindings (overrides + preset) for display in help
    /// Overrides shadow preset bindings for the same action
    pub fn all_bindings(&self) -> Vec<KeyBinding> {
        let mut bindings = self.overrides.clone();
        let preset_bindings = self.preset.bindings();

        // Add preset bindings that aren't overridden
        for preset_binding in preset_bindings {
            let is_overridden = self
                .overrides
                .iter()
                .any(|o| o.action == preset_binding.action);
            if !is_overridden {
                bindings.push(preset_binding);
            }
        }

        bindings
    }

    /// Get the display string for navigation keys (up/down)
    /// Reflects actual bindings including overrides
    pub fn navigation_display(&self) -> String {
        let up_key = self.get_key_display_for_action(Action::MoveUp);
        let down_key = self.get_key_display_for_action(Action::MoveDown);
        format!("{}/{}", up_key, down_key)
    }

    /// Get the display string for quit/cancel key
    /// Reflects actual bindings including overrides
    pub fn quit_display(&self) -> String {
        let quit_key = self.get_key_display_for_action(Action::Quit);
        let cancel_key = self.get_key_display_for_action(Action::Cancel);
        if quit_key == cancel_key {
            quit_key
        } else {
            format!("{}/{}", quit_key, cancel_key)
        }
    }

    /// Get the display string for confirm key
    /// Reflects actual bindings including overrides
    pub fn confirm_display(&self) -> String {
        self.get_key_display_for_action(Action::Confirm)
    }

    /// Get the display string for a specific action (e.g., Action::Quit -> "q")
    /// Checks overrides first, then preset. Returns generic fallback if not found.
    pub fn get_key_display_for_action(&self, action: Action) -> String {
        // Check overrides
        if let Some(binding) = self.overrides.iter().find(|b| b.action == action) {
            return binding.display();
        }

        // Check preset
        if let Some(binding) = self
            .preset
            .bindings()
            .into_iter()
            .find(|b| b.action == action)
        {
            return binding.display();
        }

        // Fallback for actions not in current map (shouldn't happen for core actions)
        format!("{:?}", action)
    }

    /// Generate footer text for common navigation screens
    pub fn footer_navigation(&self) -> String {
        format!(
            "{}: Navigate | {}: Select | {}: Back | ?: Help",
            self.navigation_display(),
            self.confirm_display(),
            self.quit_display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keymap() {
        let keymap = Keymap::default();
        assert_eq!(keymap.preset, KeymapPreset::Standard);
        assert!(keymap.overrides.is_empty());
    }

    #[test]
    fn test_get_action_from_preset() {
        let keymap = Keymap::default();
        // 'q' should map to Quit in standard preset
        let action = keymap.get_action(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(action, Some(Action::Quit));
    }

    #[test]
    fn test_override_takes_precedence() {
        let keymap = Keymap {
            preset: KeymapPreset::Standard,
            overrides: vec![KeyBinding::new("q", Action::Help)],
        };
        // Override should win
        let action = keymap.get_action(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(action, Some(Action::Help));
    }

    #[test]
    fn test_vim_preset() {
        let keymap = Keymap {
            preset: KeymapPreset::Vim,
            overrides: Vec::new(),
        };
        // 'j' should map to MoveDown in vim preset
        let action = keymap.get_action(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(action, Some(Action::MoveDown));
    }
}
