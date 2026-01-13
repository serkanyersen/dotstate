use crate::keymap::Action;
use crossterm::event::{KeyCode, KeyModifiers};

/// A text input field with encapsulated state.
///
/// This struct wraps the text and cursor position, providing a cleaner API
/// for managing text input in forms and screens.
///
/// # Example
/// ```
/// use dotstate::utils::text_input::TextInput;
///
/// let mut input = TextInput::new();
/// input.insert_char('h');
/// input.insert_char('i');
/// assert_eq!(input.text(), "hi");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInput {
    text: String,
    cursor: usize,
}

impl TextInput {
    /// Create a new empty text input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a text input with initial text.
    pub fn with_text(text: impl Into<String>) -> Self {
        let text = text.into();
        let cursor = text.chars().count();
        Self { text, cursor }
    }

    /// Get the current text as a string slice.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the current cursor position.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Get a mutable reference to the text (for backward compatibility).
    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    /// Get a mutable reference to the cursor (for backward compatibility).
    pub fn cursor_mut(&mut self) -> &mut usize {
        &mut self.cursor
    }

    /// Get the trimmed text.
    pub fn text_trimmed(&self) -> &str {
        self.text.trim()
    }

    /// Check if the text is empty (ignoring whitespace).
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Set the text and move cursor to end.
    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.cursor = self.text.chars().count();
    }

    /// Clear the text and reset cursor.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        handle_char_insertion(&mut self.text, &mut self.cursor, c);
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        handle_backspace(&mut self.text, &mut self.cursor);
    }

    /// Delete the character at the cursor position.
    pub fn delete(&mut self) {
        handle_delete(&mut self.text, &mut self.cursor);
    }

    /// Move the cursor left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the cursor right.
    pub fn move_right(&mut self) {
        let char_count = self.text.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }

    /// Move the cursor to the start.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move the cursor to the end.
    pub fn move_end(&mut self) {
        self.cursor = self.text.chars().count();
    }

    /// Handle a key code event.
    ///
    /// Returns true if the key was handled.
    pub fn handle_key(&mut self, key_code: KeyCode) -> bool {
        handle_input(&mut self.text, &mut self.cursor, key_code);
        matches!(
            key_code,
            KeyCode::Char(_)
                | KeyCode::Backspace
                | KeyCode::Delete
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Home
                | KeyCode::End
        )
    }

    /// Handle an action from the keymap.
    ///
    /// Returns true if the action was handled.
    pub fn handle_action(&mut self, action: Action) -> bool {
        match action {
            Action::MoveLeft => {
                self.move_left();
                true
            }
            Action::MoveRight => {
                self.move_right();
                true
            }
            Action::Home => {
                self.move_home();
                true
            }
            Action::End => {
                self.move_end();
                true
            }
            Action::Backspace => {
                self.backspace();
                true
            }
            Action::DeleteChar => {
                self.delete();
                true
            }
            _ => false,
        }
    }

    /// Handle a key event with modifiers and action mapping.
    ///
    /// This is useful when you want to handle both raw keys and mapped actions.
    /// Regular characters are inserted directly, while special keys go through
    /// action mapping if available.
    pub fn handle_key_with_action(
        &mut self,
        key_code: KeyCode,
        _modifiers: KeyModifiers,
        action: Option<Action>,
    ) -> bool {
        // Try action first
        if let Some(action) = action {
            if self.handle_action(action) {
                return true;
            }
        }

        // Fall back to raw key handling
        self.handle_key(key_code)
    }
}

/// Handle text input for a single character insertion
///
/// # Arguments
/// * `text` - Mutable reference to the text string
/// * `cursor_pos` - Mutable reference to cursor position
/// * `c` - Character to insert
pub fn handle_char_insertion(text: &mut String, cursor_pos: &mut usize, c: char) {
    if c.is_ascii() && !c.is_control() {
        let byte_index = text
            .char_indices()
            .map(|(i, _)| i)
            .nth(*cursor_pos)
            .unwrap_or(text.len());
        text.insert(byte_index, c);
        *cursor_pos = (*cursor_pos + 1).min(text.chars().count());
    }
}

/// Handle cursor movement
///
/// # Arguments
/// * `text` - The text string
/// * `cursor_pos` - Mutable reference to cursor position
/// * `key_code` - Key code (Left, Right, Home, End)
pub fn handle_cursor_movement(text: &str, cursor_pos: &mut usize, key_code: KeyCode) {
    match key_code {
        KeyCode::Left => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
            }
        }
        KeyCode::Right => {
            let char_count = text.chars().count();
            if *cursor_pos < char_count {
                *cursor_pos += 1;
            }
        }
        KeyCode::Home => {
            *cursor_pos = 0;
        }
        KeyCode::End => {
            *cursor_pos = text.chars().count();
        }
        _ => {}
    }
}

/// Handle character deletion (backspace)
///
/// # Arguments
/// * `text` - Mutable reference to the text string
/// * `cursor_pos` - Mutable reference to cursor position
pub fn handle_backspace(text: &mut String, cursor_pos: &mut usize) {
    if *cursor_pos > 0 {
        let before_cursor = text.chars().take(*cursor_pos - 1);
        let after_cursor = text.chars().skip(*cursor_pos);
        *text = before_cursor.chain(after_cursor).collect();
        *cursor_pos -= 1;
    }
}

/// Handle character deletion (delete key)
///
/// # Arguments
/// * `text` - Mutable reference to the text string
/// * `cursor_pos` - Mutable reference to cursor position
pub fn handle_delete(text: &mut String, cursor_pos: &mut usize) {
    let char_count = text.chars().count();
    if *cursor_pos < char_count {
        let before_cursor = text.chars().take(*cursor_pos);
        let after_cursor = text.chars().skip(*cursor_pos + 1);
        *text = before_cursor.chain(after_cursor).collect();
    }
}
/// Handle generic input for text fields (movement, editing)
///
/// # Arguments
/// * `text` - Mutable reference to the text string
/// * `cursor_pos` - Mutable reference to cursor position
/// * `key_code` - Key code from event
pub fn handle_input(text: &mut String, cursor_pos: &mut usize, key_code: KeyCode) {
    match key_code {
        KeyCode::Char(c) => handle_char_insertion(text, cursor_pos, c),
        KeyCode::Backspace => handle_backspace(text, cursor_pos),
        KeyCode::Delete => handle_delete(text, cursor_pos),
        KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
            handle_cursor_movement(text, cursor_pos, key_code)
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_insertion() {
        let mut text = String::from("hello");
        let mut cursor = 2;

        handle_char_insertion(&mut text, &mut cursor, 'x');
        assert_eq!(text, "hexllo");
        assert_eq!(cursor, 3);
    }

    #[test]
    fn test_char_insertion_at_end() {
        let mut text = String::from("hello");
        let mut cursor = 5;

        handle_char_insertion(&mut text, &mut cursor, '!');
        assert_eq!(text, "hello!");
        assert_eq!(cursor, 6);
    }

    #[test]
    fn test_cursor_movement_left() {
        let text = "hello";
        let mut cursor = 3;

        handle_cursor_movement(text, &mut cursor, KeyCode::Left);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn test_cursor_movement_right() {
        let text = "hello";
        let mut cursor = 2;

        handle_cursor_movement(text, &mut cursor, KeyCode::Right);
        assert_eq!(cursor, 3);
    }

    #[test]
    fn test_cursor_movement_home() {
        let text = "hello";
        let mut cursor = 3;

        handle_cursor_movement(text, &mut cursor, KeyCode::Home);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_cursor_movement_end() {
        let text = "hello";
        let mut cursor = 2;

        handle_cursor_movement(text, &mut cursor, KeyCode::End);
        assert_eq!(cursor, 5);
    }

    #[test]
    fn test_backspace() {
        let mut text = String::from("hello");
        let mut cursor = 3;

        handle_backspace(&mut text, &mut cursor);
        assert_eq!(text, "helo"); // Deletes 'l' at position 2
        assert_eq!(cursor, 2);
    }

    #[test]
    fn test_backspace_at_start() {
        let mut text = String::from("hello");
        let mut cursor = 0;

        handle_backspace(&mut text, &mut cursor);
        assert_eq!(text, "hello"); // No change
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_delete() {
        let mut text = String::from("hello");
        let mut cursor = 2;

        handle_delete(&mut text, &mut cursor);
        assert_eq!(text, "helo");
        assert_eq!(cursor, 2); // Cursor doesn't move
    }

    #[test]
    fn test_delete_at_end() {
        let mut text = String::from("hello");
        let mut cursor = 5;

        handle_delete(&mut text, &mut cursor);
        assert_eq!(text, "hello"); // No change
        assert_eq!(cursor, 5);
    }

    #[test]
    fn test_unicode_handling() {
        let mut text = String::from("héllo");
        let mut cursor = 2;

        handle_char_insertion(&mut text, &mut cursor, 'x');
        assert_eq!(text, "héxllo");
        assert_eq!(cursor, 3);
    }

    // TextInput struct tests
    #[test]
    fn test_text_input_new() {
        let input = TextInput::new();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
        assert!(input.is_empty());
    }

    #[test]
    fn test_text_input_with_text() {
        let input = TextInput::with_text("hello");
        assert_eq!(input.text(), "hello");
        assert_eq!(input.cursor(), 5);
        assert!(!input.is_empty());
    }

    #[test]
    fn test_text_input_set_text() {
        let mut input = TextInput::new();
        input.set_text("world");
        assert_eq!(input.text(), "world");
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn test_text_input_clear() {
        let mut input = TextInput::with_text("hello");
        input.clear();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
        assert!(input.is_empty());
    }

    #[test]
    fn test_text_input_insert_char() {
        let mut input = TextInput::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.text(), "hi");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn test_text_input_backspace() {
        let mut input = TextInput::with_text("hello");
        input.backspace();
        assert_eq!(input.text(), "hell");
        assert_eq!(input.cursor(), 4);
    }

    #[test]
    fn test_text_input_delete() {
        let mut input = TextInput::with_text("hello");
        input.move_home();
        input.delete();
        assert_eq!(input.text(), "ello");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn test_text_input_cursor_movement() {
        let mut input = TextInput::with_text("hello");

        input.move_home();
        assert_eq!(input.cursor(), 0);

        input.move_right();
        assert_eq!(input.cursor(), 1);

        input.move_left();
        assert_eq!(input.cursor(), 0);

        input.move_end();
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn test_text_input_handle_key() {
        let mut input = TextInput::new();

        assert!(input.handle_key(KeyCode::Char('a')));
        assert_eq!(input.text(), "a");

        assert!(input.handle_key(KeyCode::Char('b')));
        assert_eq!(input.text(), "ab");

        assert!(input.handle_key(KeyCode::Backspace));
        assert_eq!(input.text(), "a");
    }

    #[test]
    fn test_text_input_handle_action() {
        let mut input = TextInput::with_text("hello");

        assert!(input.handle_action(Action::Home));
        assert_eq!(input.cursor(), 0);

        assert!(input.handle_action(Action::MoveRight));
        assert_eq!(input.cursor(), 1);

        assert!(input.handle_action(Action::DeleteChar));
        assert_eq!(input.text(), "hllo");
    }

    #[test]
    fn test_text_input_trimmed() {
        let input = TextInput::with_text("  hello  ");
        assert_eq!(input.text_trimmed(), "hello");
        assert!(!input.is_empty());
    }

    #[test]
    fn test_text_input_is_empty_whitespace() {
        let input = TextInput::with_text("   ");
        assert!(input.is_empty());
    }

    #[test]
    fn test_text_input_backward_compat() {
        let mut input = TextInput::with_text("hello");

        // Test backward compatibility methods
        *input.text_mut() = "world".to_string();
        *input.cursor_mut() = 3;

        assert_eq!(input.text(), "world");
        assert_eq!(input.cursor(), 3);
    }

    #[test]
    fn test_text_input_clone() {
        let input1 = TextInput::with_text("hello");
        let input2 = input1.clone();

        assert_eq!(input1.text(), input2.text());
        assert_eq!(input1.cursor(), input2.cursor());
    }

    #[test]
    fn test_text_input_default() {
        let input: TextInput = Default::default();
        assert_eq!(input.text(), "");
        assert_eq!(input.cursor(), 0);
    }
}
