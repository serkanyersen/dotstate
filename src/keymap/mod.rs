//! `DotState` keymap surface re-exported from `tui-forge`.

pub use tui_forge::keymap::{
    format_key_display, parse_key_string, Action, KeyBinding, Keymap, KeymapPreset, ParsedKey,
};

use crossterm::event::{KeyCode, KeyModifiers};

#[must_use]
pub fn dotstate_action_sync() -> Action {
    Action::Custom("sync".to_string())
}

#[must_use]
pub fn dotstate_action_check_status() -> Action {
    Action::Custom("check_status".to_string())
}

#[must_use]
pub fn dotstate_action_install() -> Action {
    Action::Custom("install".to_string())
}

#[must_use]
pub fn dotstate_action_import() -> Action {
    Action::Custom("import".to_string())
}

#[must_use]
pub fn dotstate_action_move() -> Action {
    Action::Custom("move".to_string())
}

#[must_use]
pub fn dotstate_action_toggle_backup() -> Action {
    Action::Custom("toggle_backup".to_string())
}

#[must_use]
pub fn dotstate_extra_bindings(preset: KeymapPreset) -> Vec<KeyBinding> {
    match preset {
        KeymapPreset::Standard => vec![
            KeyBinding::new("s", dotstate_action_check_status()),
            KeyBinding::new("shift+s", dotstate_action_sync()),
            KeyBinding::new("i", dotstate_action_install()),
            KeyBinding::new("shift+i", dotstate_action_import()),
            KeyBinding::new("b", dotstate_action_toggle_backup()),
            KeyBinding::new("m", dotstate_action_move()),
        ],
        KeymapPreset::Vim => vec![
            KeyBinding::new("s", dotstate_action_check_status()),
            KeyBinding::new("shift+s", dotstate_action_sync()),
            KeyBinding::new("i", dotstate_action_install()),
            KeyBinding::new("shift+i", dotstate_action_import()),
            KeyBinding::new("b", dotstate_action_toggle_backup()),
            KeyBinding::new("m", dotstate_action_move()),
        ],
        KeymapPreset::Emacs => vec![
            KeyBinding::new("s", dotstate_action_check_status()),
            KeyBinding::new("ctrl+x s", dotstate_action_sync()),
            KeyBinding::new("i", dotstate_action_install()),
            KeyBinding::new("shift+i", dotstate_action_import()),
            KeyBinding::new("b", dotstate_action_toggle_backup()),
            KeyBinding::new("m", dotstate_action_move()),
        ],
    }
}

#[must_use]
pub fn get_action(keymap: &Keymap, code: KeyCode, modifiers: KeyModifiers) -> Option<Action> {
    let extra = dotstate_extra_bindings(keymap.preset);
    keymap.get_action_with_bindings(code, modifiers, &extra)
}

#[must_use]
pub fn get_key_display_for_action(keymap: &Keymap, action: &Action) -> String {
    let extra = dotstate_extra_bindings(keymap.preset);
    keymap.get_key_display_for_action_with_bindings(action, &extra)
}
