//! Action enum for all user-triggered actions
//!
//! These represent semantic actions that can be triggered by keyboard shortcuts.

use serde::{Deserialize, Serialize};

/// All possible user actions in the application
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // ============ Navigation (unified across screens) ============
    /// Move selection up in a list
    MoveUp,
    /// Move selection down in a list
    MoveDown,
    /// Move left (for horizontal navigation or tabs)
    MoveLeft,
    /// Move right (for horizontal navigation or tabs)
    MoveRight,
    /// Jump up by a page
    PageUp,
    /// Jump down by a page
    PageDown,
    /// Go to the first item
    GoToTop,
    /// Go to the last item
    GoToEnd,
    /// Jump to start of line/input
    Home,
    /// Jump to end of line/input
    End,

    // ============ Selection & Confirmation ============
    /// Confirm selection / submit form (Enter)
    Confirm,
    /// Cancel / go back (Esc)
    Cancel,
    /// Toggle selection state (Space)
    ToggleSelect,
    /// Select all items
    SelectAll,
    /// Deselect all items
    DeselectAll,

    // ============ Global ============
    /// Quit the application
    Quit,
    /// Show help overlay
    Help,

    // ============ Screen-specific actions ============
    /// Delete selected item
    Delete,
    /// Edit selected item
    Edit,
    /// Create new item
    Create,
    /// Search / filter
    Search,
    /// Refresh current view
    Refresh,

    // ============ Text editing ============
    /// Delete character before cursor
    Backspace,
    /// Delete character at cursor
    DeleteChar,

    // ============ Tab/Field navigation ============
    /// Move to next tab or field
    NextTab,
    /// Move to previous tab or field
    PrevTab,

    // ============ Scroll (for preview panes) ============
    /// Scroll preview up
    ScrollUp,
    /// Scroll preview down
    ScrollDown,

    // ============ Yes/No prompts ============
    /// Confirm yes
    Yes,
    /// Confirm no
    No,

    // ============ Form/Save actions ============
    /// Save / submit form (Ctrl+S in many contexts)
    Save,

    // ============ Extensibility ============
    /// Custom action for application-specific behavior
    Custom(String),
}

impl Action {
    /// Get a human-readable description of this action
    #[must_use]
    pub fn description(&self) -> &str {
        match self {
            Action::MoveUp => "Move up",
            Action::MoveDown => "Move down",
            Action::MoveLeft => "Move left",
            Action::MoveRight => "Move right",
            Action::PageUp => "Page up",
            Action::PageDown => "Page down",
            Action::GoToTop => "Go to top",
            Action::GoToEnd => "Go to end",
            Action::Home => "Home",
            Action::End => "End",
            Action::Confirm => "Confirm",
            Action::Cancel => "Cancel / Go back",
            Action::ToggleSelect => "Toggle selection",
            Action::SelectAll => "Select all",
            Action::DeselectAll => "Deselect all",
            Action::Quit => "Quit",
            Action::Help => "Show help",
            Action::Delete => "Delete",
            Action::Edit => "Edit",
            Action::Create => "Create new",
            Action::Search => "Search",
            Action::Refresh => "Refresh",
            Action::Backspace => "Backspace",
            Action::DeleteChar => "Delete character",
            Action::NextTab => "Next field",
            Action::PrevTab => "Previous field",
            Action::ScrollUp => "Scroll up",
            Action::ScrollDown => "Scroll down",
            Action::Yes => "Yes",
            Action::No => "No",
            Action::Save => "Save",
            Action::Custom(s) => s.as_str(),
        }
    }

    /// Get action category for grouping in help display
    #[must_use]
    pub fn category(&self) -> &str {
        match self {
            Action::MoveUp
            | Action::MoveDown
            | Action::MoveLeft
            | Action::MoveRight
            | Action::PageUp
            | Action::PageDown
            | Action::GoToTop
            | Action::GoToEnd
            | Action::Home
            | Action::End => "Navigation",

            Action::Confirm
            | Action::Cancel
            | Action::ToggleSelect
            | Action::SelectAll
            | Action::DeselectAll => "Selection",

            Action::Quit | Action::Help => "Global",

            Action::Delete
            | Action::Edit
            | Action::Create
            | Action::Search
            | Action::Refresh
            | Action::Save => "Actions",

            Action::Backspace | Action::DeleteChar => "Text Editing",

            Action::NextTab | Action::PrevTab => "Field Navigation",

            Action::ScrollUp | Action::ScrollDown => "Scroll",

            Action::Yes | Action::No => "Prompts",

            Action::Custom(_) => "Custom",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_description() {
        assert_eq!(Action::MoveUp.description(), "Move up");
        assert_eq!(Action::Quit.description(), "Quit");
    }

    #[test]
    fn test_action_category() {
        assert_eq!(Action::MoveUp.category(), "Navigation");
        assert_eq!(Action::Quit.category(), "Global");
        assert_eq!(Action::Save.category(), "Actions");
    }

    #[test]
    fn test_action_serialization() {
        let action = Action::MoveUp;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"move_up\"");
    }

    #[test]
    fn test_action_deserialization() {
        let action: Action = serde_json::from_str("\"move_down\"").unwrap();
        assert_eq!(action, Action::MoveDown);
    }

    #[test]
    fn test_custom_action() {
        let action = Action::Custom("my_action".to_string());
        assert_eq!(action.description(), "my_action");
        assert_eq!(action.category(), "Custom");
    }

    #[test]
    fn test_custom_action_serialization() {
        let action = Action::Custom("my_action".to_string());
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, action);
    }
}
