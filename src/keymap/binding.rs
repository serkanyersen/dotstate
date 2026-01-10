//! KeyBinding struct for mapping keys to actions
//!
//! Provides parsing of key strings like "ctrl+n", "shift+tab", "j"

use super::Action;
use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Serialize};

/// A single key binding mapping a key combination to an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Key string (e.g., "j", "down", "ctrl+n", "shift+tab")
    pub key: String,

    /// The action this key triggers
    pub action: Action,

    /// Optional description override (uses action description if None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parsed key representation for matching
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedKey {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    /// Create a new key binding
    pub fn new(key: &str, action: Action) -> Self {
        Self {
            key: key.to_string(),
            action,
            description: None,
        }
    }

    /// Check if this binding matches the given key event
    pub fn matches(&self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if let Ok(parsed) = self.parse() {
            parsed.code == code && parsed.modifiers == modifiers
        } else {
            false
        }
    }

    /// Parse the key string into KeyCode and KeyModifiers
    pub fn parse(&self) -> Result<ParsedKey, String> {
        parse_key_string(&self.key)
    }

    /// Get the display string for this binding (e.g., "Ctrl+N")
    pub fn display(&self) -> String {
        format_key_display(&self.key)
    }

    /// Get the description (custom or from action)
    pub fn get_description(&self) -> &str {
        self.description
            .as_deref()
            .unwrap_or_else(|| self.action.description())
    }
}

/// Parse a key string like "ctrl+shift+n" into KeyCode and KeyModifiers
pub fn parse_key_string(key: &str) -> Result<ParsedKey, String> {
    let key = key.trim().to_lowercase();
    let parts: Vec<&str> = key.split('+').collect();

    let mut modifiers = KeyModifiers::NONE;
    let mut key_part = "";

    for (i, part) in parts.iter().enumerate() {
        let part = part.trim();
        if i == parts.len() - 1 {
            // Last part is the actual key
            key_part = part;
        } else {
            // Everything else is a modifier
            match part {
                "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
                "alt" | "option" => modifiers |= KeyModifiers::ALT,
                "shift" => modifiers |= KeyModifiers::SHIFT,
                "super" | "meta" | "cmd" | "command" => modifiers |= KeyModifiers::SUPER,
                _ => return Err(format!("Unknown modifier: {}", part)),
            }
        }
    }

    let code = parse_key_code(key_part)?;
    Ok(ParsedKey { code, modifiers })
}

/// Parse a single key name into KeyCode
fn parse_key_code(key: &str) -> Result<KeyCode, String> {
    let key = key.trim().to_lowercase();

    // Special keys
    match key.as_str() {
        // Navigation keys
        "up" | "arrow_up" => return Ok(KeyCode::Up),
        "down" | "arrow_down" => return Ok(KeyCode::Down),
        "left" | "arrow_left" => return Ok(KeyCode::Left),
        "right" | "arrow_right" => return Ok(KeyCode::Right),
        "home" => return Ok(KeyCode::Home),
        "end" => return Ok(KeyCode::End),
        "pageup" | "page_up" | "pgup" => return Ok(KeyCode::PageUp),
        "pagedown" | "page_down" | "pgdn" => return Ok(KeyCode::PageDown),

        // Action keys
        "enter" | "return" => return Ok(KeyCode::Enter),
        "esc" | "escape" => return Ok(KeyCode::Esc),
        "space" | " " => return Ok(KeyCode::Char(' ')),
        "tab" => return Ok(KeyCode::Tab),
        "backtab" | "shift+tab" => return Ok(KeyCode::BackTab),
        "backspace" | "bs" => return Ok(KeyCode::Backspace),
        "delete" | "del" => return Ok(KeyCode::Delete),
        "insert" | "ins" => return Ok(KeyCode::Insert),

        // Function keys
        "f1" => return Ok(KeyCode::F(1)),
        "f2" => return Ok(KeyCode::F(2)),
        "f3" => return Ok(KeyCode::F(3)),
        "f4" => return Ok(KeyCode::F(4)),
        "f5" => return Ok(KeyCode::F(5)),
        "f6" => return Ok(KeyCode::F(6)),
        "f7" => return Ok(KeyCode::F(7)),
        "f8" => return Ok(KeyCode::F(8)),
        "f9" => return Ok(KeyCode::F(9)),
        "f10" => return Ok(KeyCode::F(10)),
        "f11" => return Ok(KeyCode::F(11)),
        "f12" => return Ok(KeyCode::F(12)),

        _ => {}
    }

    // Single character
    if key.len() == 1 {
        if let Some(c) = key.chars().next() {
            return Ok(KeyCode::Char(c));
        }
    }

    Err(format!("Unknown key: {}", key))
}

/// Format a key string for display (e.g., "ctrl+n" -> "Ctrl+N")
pub fn format_key_display(key: &str) -> String {
    let parts: Vec<&str> = key.split('+').collect();
    let formatted: Vec<String> = parts
        .iter()
        .map(|part| {
            let part = part.trim().to_lowercase();
            match part.as_str() {
                "ctrl" | "control" => "Ctrl".to_string(),
                "alt" | "option" => "Alt".to_string(),
                "shift" => "Shift".to_string(),
                "super" | "meta" | "cmd" | "command" => "Cmd".to_string(),
                "up" | "arrow_up" => "↑".to_string(),
                "down" | "arrow_down" => "↓".to_string(),
                "left" | "arrow_left" => "←".to_string(),
                "right" | "arrow_right" => "→".to_string(),
                "enter" | "return" => "Enter".to_string(),
                "esc" | "escape" => "Esc".to_string(),
                "space" => "Space".to_string(),
                "tab" => "Tab".to_string(),
                "backtab" => "Shift+Tab".to_string(),
                "backspace" | "bs" => "Backspace".to_string(),
                "delete" | "del" => "Del".to_string(),
                "pageup" | "page_up" | "pgup" => "PgUp".to_string(),
                "pagedown" | "page_down" | "pgdn" => "PgDn".to_string(),
                "home" => "Home".to_string(),
                "end" => "End".to_string(),
                _ if part.len() == 1 => part.to_uppercase(),
                _ if part.starts_with('f') && part.len() <= 3 => part.to_uppercase(),
                _ => part.to_string(),
            }
        })
        .collect();

    formatted.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_key() {
        let parsed = parse_key_string("j").unwrap();
        assert_eq!(parsed.code, KeyCode::Char('j'));
        assert_eq!(parsed.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_parse_arrow_key() {
        let parsed = parse_key_string("up").unwrap();
        assert_eq!(parsed.code, KeyCode::Up);
        assert_eq!(parsed.modifiers, KeyModifiers::NONE);

        let parsed = parse_key_string("down").unwrap();
        assert_eq!(parsed.code, KeyCode::Down);
    }

    #[test]
    fn test_parse_ctrl_key() {
        let parsed = parse_key_string("ctrl+n").unwrap();
        assert_eq!(parsed.code, KeyCode::Char('n'));
        assert_eq!(parsed.modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_parse_shift_key() {
        let parsed = parse_key_string("shift+tab").unwrap();
        assert_eq!(parsed.code, KeyCode::Tab);
        assert_eq!(parsed.modifiers, KeyModifiers::SHIFT);
    }

    #[test]
    fn test_parse_multi_modifier() {
        let parsed = parse_key_string("ctrl+shift+n").unwrap();
        assert_eq!(parsed.code, KeyCode::Char('n'));
        assert_eq!(
            parsed.modifiers,
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        );
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(parse_key_string("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key_string("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key_string("space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(parse_key_string("tab").unwrap().code, KeyCode::Tab);
        assert_eq!(
            parse_key_string("backspace").unwrap().code,
            KeyCode::Backspace
        );
    }

    #[test]
    fn test_parse_function_keys() {
        assert_eq!(parse_key_string("f1").unwrap().code, KeyCode::F(1));
        assert_eq!(parse_key_string("f12").unwrap().code, KeyCode::F(12));
    }

    #[test]
    fn test_format_key_display() {
        assert_eq!(format_key_display("ctrl+n"), "Ctrl+N");
        assert_eq!(format_key_display("up"), "↑");
        assert_eq!(format_key_display("ctrl+shift+j"), "Ctrl+Shift+J");
        assert_eq!(format_key_display("enter"), "Enter");
    }

    #[test]
    fn test_key_binding_matches() {
        let binding = KeyBinding::new("ctrl+n", Action::MoveDown);
        assert!(binding.matches(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert!(!binding.matches(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(!binding.matches(KeyCode::Char('m'), KeyModifiers::CONTROL));
    }

    #[test]
    fn test_key_binding_description() {
        let binding = KeyBinding::new("j", Action::MoveDown);
        assert_eq!(binding.get_description(), "Move down");
    }
}
