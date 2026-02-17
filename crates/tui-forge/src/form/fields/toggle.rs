//! On/off toggle switch field.
//!
//! A simple boolean toggle rendered as a visual switch.
//! Implements [`FormField`].

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::form::field::{FieldValue, FormField};
use crate::theme::theme;

/// An on/off toggle switch.
///
/// # Builder example
///
/// ```rust,ignore
/// use tui_forge::form::fields::ToggleSwitch;
///
/// let toggle = ToggleSwitch::new().on(true);
/// ```
#[derive(Debug, Clone)]
pub struct ToggleSwitch {
    on: bool,
}

impl Default for ToggleSwitch {
    fn default() -> Self {
        Self::new()
    }
}

impl ToggleSwitch {
    // ----- construction / builder -----------------------------------------

    /// Create a new toggle switch (default: off).
    pub fn new() -> Self {
        Self { on: false }
    }

    /// Set the initial state.
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    // ----- typed API ------------------------------------------------------

    /// Whether the toggle is in the "on" position.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Programmatically set the state.
    pub fn set_on(&mut self, on: bool) {
        self.on = on;
    }

    /// Flip the toggle.
    pub fn toggle(&mut self) {
        self.on = !self.on;
    }
}

// ---------------------------------------------------------------------------
// FormField implementation
// ---------------------------------------------------------------------------

impl FormField for ToggleSwitch {
    fn field_value(&self) -> FieldValue {
        FieldValue::Bool(self.on)
    }

    fn set_field_value(&mut self, value: FieldValue) {
        if let FieldValue::Bool(b) = value {
            self.on = b;
        }
    }

    fn clear(&mut self) {
        self.on = false;
    }

    fn captures_text(&self) -> bool {
        false
    }

    fn handle_key_event(&mut self, event: &KeyEvent) -> bool {
        match event.code {
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.toggle();
                true
            }
            _ => false,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let t = theme();

        let (switch_text, label, color) = if self.on {
            // ON state:  ━━━●  ON
            ("\u{2501}\u{2501}\u{2501}\u{25CF}", "ON", t.success)
        } else {
            // OFF state: ○━━━  OFF
            ("\u{25CB}\u{2501}\u{2501}\u{2501}", "OFF", t.text_muted)
        };

        let style = if focused {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        let text = format!("{switch_text}  {label}");
        let paragraph = Paragraph::new(Span::styled(text, style));
        frame.render_widget(paragraph, area);
    }

    fn height(&self) -> u16 {
        1
    }
}
