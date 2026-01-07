use crossterm::event::KeyCode;

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
}
