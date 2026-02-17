//! Keymap configuration module
//!
//! Provides customizable keyboard shortcuts with preset keymaps (standard, vim, emacs).

mod action;
mod binding;
mod presets;

pub use action::Action;
pub use binding::{format_key_display, parse_key_string, KeyBinding, ParsedKey};
pub use presets::KeymapPreset;

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    /// Create a new keymap from a preset with no overrides.
    #[must_use]
    pub fn new(preset: KeymapPreset) -> Self {
        Self {
            preset,
            overrides: Vec::new(),
        }
    }

    /// Get the action for a key event, checking overrides first then preset
    /// Note: If an action is overridden, preset bindings for that action are ignored
    #[must_use]
    pub fn get_action(&self, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
        // Use all_bindings which already handles override shadowing
        for binding in self.all_bindings() {
            if binding.matches(code, modifiers) {
                return Some(binding.action);
            }
        }
        None
    }

    /// Get the action for a key event while extending the preset with
    /// application-provided bindings.
    ///
    /// Precedence order:
    /// 1) `self.overrides`
    /// 2) `extra_bindings`
    /// 3) preset bindings
    #[must_use]
    pub fn get_action_with_bindings(
        &self,
        code: KeyCode,
        modifiers: KeyModifiers,
        extra_bindings: &[KeyBinding],
    ) -> Option<Action> {
        for binding in self.all_bindings_with(extra_bindings) {
            if binding.matches(code, modifiers) {
                return Some(binding.action);
            }
        }
        None
    }

    /// Get all bindings (overrides + preset) for display in help
    /// Overrides shadow preset bindings for the same action
    #[must_use]
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

    /// Get all bindings using the precedence:
    /// overrides > `extra_bindings` > preset.
    #[must_use]
    pub fn all_bindings_with(&self, extra_bindings: &[KeyBinding]) -> Vec<KeyBinding> {
        let mut bindings = self.overrides.clone();
        let mut shadowed: HashSet<Action> = self.overrides.iter().map(|b| b.action.clone()).collect();

        for extra in extra_bindings {
            if shadowed.insert(extra.action.clone()) {
                bindings.push(extra.clone());
            }
        }

        for preset in self.preset.bindings() {
            if shadowed.insert(preset.action.clone()) {
                bindings.push(preset);
            }
        }

        bindings
    }

    /// Get the display string for navigation keys (up/down)
    /// Reflects actual bindings including overrides
    #[must_use]
    pub fn navigation_display(&self) -> String {
        let up_key = self.get_key_display_for_action(Action::MoveUp);
        let down_key = self.get_key_display_for_action(Action::MoveDown);
        format!("{up_key}/{down_key}")
    }

    /// Get the display string for quit/cancel key
    /// Reflects actual bindings including overrides
    #[must_use]
    pub fn quit_display(&self) -> String {
        let quit_key = self.get_key_display_for_action(Action::Quit);
        let cancel_key = self.get_key_display_for_action(Action::Cancel);
        if quit_key == cancel_key {
            quit_key
        } else {
            format!("{quit_key}/{cancel_key}")
        }
    }

    /// Get the display string for confirm key
    /// Reflects actual bindings including overrides
    #[must_use]
    pub fn confirm_display(&self) -> String {
        self.get_key_display_for_action(Action::Confirm)
    }

    /// Get the display string for a specific action (e.g., `Action::Quit` -> "q")
    /// Checks overrides first, then preset. Returns generic fallback if not found.
    #[must_use]
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
        format!("{action:?}")
    }

    /// Get display text for an action while extending the preset with
    /// application-provided bindings.
    ///
    /// Precedence order:
    /// 1) `self.overrides`
    /// 2) `extra_bindings`
    /// 3) preset bindings
    #[must_use]
    pub fn get_key_display_for_action_with_bindings(
        &self,
        action: &Action,
        extra_bindings: &[KeyBinding],
    ) -> String {
        if let Some(binding) = self.overrides.iter().find(|b| &b.action == action) {
            return binding.display();
        }

        if let Some(binding) = extra_bindings.iter().find(|b| &b.action == action) {
            return binding.display();
        }

        if let Some(binding) = self
            .preset
            .bindings()
            .into_iter()
            .find(|b| &b.action == action)
        {
            return binding.display();
        }

        format!("{action:?}")
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

    #[test]
    fn test_navigation_display() {
        let keymap = Keymap::default();
        let display = keymap.navigation_display();
        assert!(display.contains('/'));
    }

    #[test]
    fn test_quit_display() {
        let keymap = Keymap::default();
        let display = keymap.quit_display();
        assert!(!display.is_empty());
    }

    #[test]
    fn test_confirm_display() {
        let keymap = Keymap::default();
        let display = keymap.confirm_display();
        assert_eq!(display, "Enter");
    }
}
