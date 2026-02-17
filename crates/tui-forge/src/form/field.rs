//! Core form field trait and universal value type.

use crossterm::event::KeyEvent;
use ratatui::prelude::*;

/// Universal value type for form fields.
///
/// Each variant maps to one or more field types:
/// - `Text` — [`TextInput`], [`TextArea`], [`Password`]
/// - `Bool` — [`ToggleSwitch`]
/// - `Choice` — [`RadioBox`] (single selected index)
/// - `Choices` — [`Checkbox`] (multiple selected indices)
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// A text value (used by `TextInput`, `TextArea`, `Password`).
    Text(String),
    /// A boolean value (used by `ToggleSwitch`).
    Bool(bool),
    /// A single selected index (used by `RadioBox`).
    Choice(usize),
    /// Multiple selected indices (used by `Checkbox`).
    Choices(Vec<usize>),
}

impl FieldValue {
    /// Attempt to extract a text reference.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            FieldValue::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Attempt to extract a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            FieldValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempt to extract a single choice index.
    pub fn as_choice(&self) -> Option<usize> {
        match self {
            FieldValue::Choice(i) => Some(*i),
            _ => None,
        }
    }

    /// Attempt to extract multiple choice indices.
    pub fn as_choices(&self) -> Option<&[usize]> {
        match self {
            FieldValue::Choices(v) => Some(v),
            _ => None,
        }
    }
}

/// Trait implemented by every form field type.
///
/// This provides a uniform interface for the form container to interact with
/// heterogeneous fields (text inputs, checkboxes, radio buttons, etc.).
pub trait FormField {
    /// Get the current value of the field as a [`FieldValue`].
    fn field_value(&self) -> FieldValue;

    /// Set the field value from a [`FieldValue`].
    ///
    /// If the variant does not match the expected type for this field,
    /// the implementation should silently ignore the call.
    fn set_field_value(&mut self, value: FieldValue);

    /// Reset the field to its empty / default state.
    fn clear(&mut self);

    /// Whether this field captures raw text input.
    ///
    /// When `true`, the form container should forward printable key events
    /// to this field rather than interpreting them as navigation commands.
    fn captures_text(&self) -> bool;

    /// Handle a key event, returning `true` if the event was consumed.
    fn handle_key_event(&mut self, event: &KeyEvent) -> bool;

    /// Render the field into the given area.
    fn render(&self, frame: &mut Frame, area: Rect, focused: bool);

    /// The preferred height (in rows) this field wants.
    fn height(&self) -> u16;
}
