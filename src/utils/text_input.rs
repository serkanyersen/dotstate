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

