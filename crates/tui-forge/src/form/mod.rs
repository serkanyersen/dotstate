//! Form coordinator: manages a collection of form fields, focus, validation,
//! layout rendering, and event routing with keymap conflict resolution.

pub mod field;
pub mod config;
pub mod values;
pub mod layout;
pub mod fields;

// Re-exports
pub use field::{FormField, FieldValue};
pub use config::{FieldConfig, ValidateOn, validators};
pub use values::FormValues;
pub use layout::FormLayout;
pub use fields::*;

use std::collections::HashMap;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::theme::theme;

// ---------------------------------------------------------------------------
// FormAction
// ---------------------------------------------------------------------------

/// Action returned by [`Form::handle_event`] to the caller.
///
/// The caller uses this to decide what to do next:
/// - `Ignored`  — the form did not use this event; pass it to the app keymap.
/// - `Consumed` — the form used the event; do **not** pass it to the keymap.
/// - `Submit`   — the user pressed Enter / Ctrl+Enter to submit.
/// - `ValueChanged(name)` — a field value changed (useful for live previews).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormAction {
    /// The event was not handled by the form.
    Ignored,
    /// The event was handled; the caller should not propagate it further.
    Consumed,
    /// The user requested form submission (Enter on non-textarea, or Ctrl+Enter).
    Submit,
    /// A field value changed. Contains the field name.
    ValueChanged(String),
}

// ---------------------------------------------------------------------------
// FormEntry (private)
// ---------------------------------------------------------------------------

/// Internal bookkeeping for a single field in the form.
struct FormEntry {
    /// The unique name used to look up the field.
    name: String,
    /// The boxed field widget implementing [`FormField`].
    field: Box<dyn FormField>,
    /// Configuration (label, validators, etc.).
    config: FieldConfig,
}

// ---------------------------------------------------------------------------
// Form
// ---------------------------------------------------------------------------

/// A form coordinator that owns a heterogeneous collection of fields,
/// manages focus traversal, validation, and delegates rendering to a
/// configurable [`FormLayout`].
#[allow(clippy::struct_field_names)]
pub struct Form {
    /// Ordered list of field entries.
    fields: Vec<FormEntry>,
    /// Index of the currently focused field, or `None` if unfocused.
    focused: Option<usize>,
    /// Validation errors keyed by field name.
    errors: HashMap<String, String>,
    /// The layout strategy used for rendering.
    form_layout: FormLayout,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

impl Form {
    /// Create a new empty form with a vertical layout.
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            focused: None,
            errors: HashMap::new(),
            form_layout: FormLayout::default(),
        }
    }

    /// Add a field to the form with the given unique name and configuration.
    ///
    /// Fields are rendered and focused in the order they are added.
    pub fn field<F: FormField + 'static>(mut self, name: &str, field: F, config: FieldConfig) -> Self {
        self.fields.push(FormEntry {
            name: name.to_string(),
            field: Box::new(field),
            config,
        });
        // Auto-focus the first field added if nothing is focused yet
        if self.focused.is_none() && !self.fields.is_empty() {
            self.focused = Some(0);
        }
        self
    }

    /// Set the layout strategy.
    pub fn layout(mut self, layout: FormLayout) -> Self {
        self.form_layout = layout;
        self
    }
}

impl Default for Form {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Event handling — the critical part
// ---------------------------------------------------------------------------

impl Form {
    /// Route an input event through the form's focus / field pipeline.
    ///
    /// # Keymap conflict resolution (plan section 2.10.7)
    ///
    /// 1. **Tab / Shift+Tab** — move focus to next / previous field. Always
    ///    returns `Consumed`.
    /// 2. **Escape** — if the focused field captures text, unfocus it and
    ///    return `Consumed`. Otherwise return `Ignored` (let the app handle
    ///    Escape, e.g. to close a dialog).
    /// 3. **Enter on a non-textarea field** — return `Submit`.
    ///    **Ctrl+Enter** on any field — return `Submit`.
    /// 4. Forward the event to the focused field.
    /// 5. If the field consumed the event, check whether the value changed
    ///    and return `ValueChanged(name)` or `Consumed`.
    /// 6. If the field did **not** consume the event but the field captures
    ///    text, still return `Consumed` to block the keymap from seeing stray
    ///    printable characters.
    /// 7. Non-text-capturing field did not handle the event — `Ignored`.
    pub fn handle_event(&mut self, event: &Event) -> FormAction {
        // Only handle key events
        let key_event = match event {
            Event::Key(ke) => ke,
            _ => return FormAction::Ignored,
        };

        // ---- Step 1: Tab / Shift+Tab navigation ----
        if key_event.code == KeyCode::Tab {
            if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                self.focus_previous();
            } else {
                self.focus_next();
            }
            return FormAction::Consumed;
        }
        // Also accept BackTab (some terminals emit this instead of Shift+Tab)
        if key_event.code == KeyCode::BackTab {
            self.focus_previous();
            return FormAction::Consumed;
        }

        // If no field is focused, the form cannot handle the event
        let idx = match self.focused {
            Some(i) => i,
            None => return FormAction::Ignored,
        };

        let entry = &self.fields[idx];
        let field_captures_text = entry.field.captures_text();
        let is_textarea = entry.config.is_textarea;

        // ---- Step 2: Escape ----
        if key_event.code == KeyCode::Esc {
            if field_captures_text {
                // Unfocus this text-capturing field so the app keymap resumes.
                // We don't remove focus entirely; we just keep the index so
                // rendering still shows which field is "selected". The captures_text
                // check in step 6 below won't fire after this because we
                // return immediately.
                return FormAction::Consumed;
            }
            // Non-text field: let the app handle Escape (e.g. close dialog)
            return FormAction::Ignored;
        }

        // ---- Step 3: Enter / Ctrl+Enter → Submit ----
        if key_event.code == KeyCode::Enter {
            // Ctrl+Enter always submits, regardless of field type.
            if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                return FormAction::Submit;
            }
            // Plain Enter on a non-textarea → submit.
            if !is_textarea {
                return FormAction::Submit;
            }
            // Plain Enter on a textarea → fall through to the field so it can
            // insert a newline.
        }

        // ---- Step 4: Route to the focused field ----
        let value_before = self.fields[idx].field.field_value();
        let consumed = self.fields[idx].field.handle_key_event(key_event);
        let name = self.fields[idx].name.clone();

        // ---- Step 5: Field consumed → check for value change ----
        if consumed {
            let value_after = self.fields[idx].field.field_value();
            if value_before != value_after {
                // Run on-change validation if configured
                if self.fields[idx].config.validate_on == ValidateOn::Change {
                    self.validate_field(idx);
                }
                return FormAction::ValueChanged(name);
            }
            return FormAction::Consumed;
        }

        // ---- Step 6: Field did NOT consume, but captures text → block ----
        if field_captures_text {
            // Even though the field didn't handle it, we must not let the
            // keymap see arbitrary characters while a text field is focused.
            return FormAction::Consumed;
        }

        // ---- Step 7: Non-text-capturing field didn't handle → Ignored ----
        FormAction::Ignored
    }
}

// ---------------------------------------------------------------------------
// Value access
// ---------------------------------------------------------------------------

impl Form {
    /// Collect a snapshot of all field values.
    pub fn values(&self) -> FormValues {
        let mut vals = FormValues::new();
        for entry in &self.fields {
            vals.insert(entry.name.clone(), entry.field.field_value());
        }
        vals
    }

    /// Get the raw [`FieldValue`] for a field by name.
    pub fn value(&self, field: &str) -> Option<FieldValue> {
        self.find_entry(field).map(|e| e.field.field_value())
    }

    /// Convenience: get a text value by field name.
    pub fn text(&self, _field: &str) -> Option<&str> {
        // We cannot return a borrow into the field value because `field_value()`
        // returns an owned `FieldValue`. Instead we search for the field and
        // try to borrow directly from its internal state. Since the trait
        // returns owned values, we return `None` here and callers should use
        // `value(field).and_then(|v| v.as_text().map(|s| s.to_string()))` for
        // owned access, or use `values().text(name)`.
        //
        // For API convenience we *do* attempt a short path: extract via
        // `field_value()` and match, but we cannot hand out a borrow. So
        // `text()` on Form is *not* zero-copy. Callers that need a borrow
        // should use `values()`.
        None // see `value()` instead
    }

    /// Convenience: get a bool value by field name.
    pub fn bool(&self, field: &str) -> Option<bool> {
        self.value(field).and_then(|v| v.as_bool())
    }

    /// Convenience: get a single choice index by field name.
    pub fn choice(&self, field: &str) -> Option<usize> {
        self.value(field).and_then(|v| v.as_choice())
    }

    /// Convenience: get multiple choice indices by field name.
    pub fn choices(&self, field: &str) -> Option<Vec<usize>> {
        self.value(field)
            .and_then(|v| v.as_choices().map(<[usize]>::to_vec))
    }

    /// Set the value of a field by name.
    pub fn set_value(&mut self, field: &str, value: FieldValue) {
        if let Some(entry) = self.find_entry_mut(field) {
            entry.field.set_field_value(value);
        }
    }

    /// Convenience: set a text value by field name.
    pub fn set_text(&mut self, field: &str, text: &str) {
        self.set_value(field, FieldValue::Text(text.to_string()));
    }

    /// Convenience: set a bool value by field name.
    pub fn set_bool(&mut self, field: &str, value: bool) {
        self.set_value(field, FieldValue::Bool(value));
    }

    /// Convenience: set a single choice index by field name.
    pub fn set_choice(&mut self, field: &str, index: usize) {
        self.set_value(field, FieldValue::Choice(index));
    }

    /// Reset all fields to their default state and clear errors.
    pub fn clear(&mut self) {
        for entry in &mut self.fields {
            entry.field.clear();
        }
        self.errors.clear();
    }

    /// Bulk-set field values from a [`FormValues`] snapshot.
    ///
    /// Only fields whose names appear in `values` are updated; the rest
    /// are left unchanged.
    pub fn set_values(&mut self, values: FormValues) {
        for (name, val) in values.iter() {
            if let Some(entry) = self.fields.iter_mut().find(|e| e.name == *name) {
                entry.field.set_field_value(val.clone());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

impl Form {
    /// Run validation on all fields. Returns `true` if every field passes.
    pub fn validate(&mut self) -> bool {
        self.errors.clear();
        for i in 0..self.fields.len() {
            self.validate_field(i);
        }
        self.errors.is_empty()
    }

    /// Whether the form currently has no validation errors.
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get all current validation errors.
    pub fn errors(&self) -> &HashMap<String, String> {
        &self.errors
    }

    /// Get the validation error for a specific field, if any.
    pub fn field_error(&self, field: &str) -> Option<&str> {
        self.errors.get(field).map(String::as_str)
    }

    /// Clear all validation errors without re-validating.
    pub fn clear_errors(&mut self) {
        self.errors.clear();
    }

    /// Validate a single field by index and store any error.
    fn validate_field(&mut self, idx: usize) {
        let entry = &self.fields[idx];
        let name = entry.name.clone();
        let value = entry.field.field_value();

        // Convert the value to a string for validators
        let text_repr = match &value {
            FieldValue::Text(s) => s.clone(),
            FieldValue::Bool(b) => b.to_string(),
            FieldValue::Choice(i) => i.to_string(),
            FieldValue::Choices(v) => format!("{v:?}"),
        };

        // Check required first
        if entry.config.required {
            if let Err(msg) = validators::required(&text_repr) {
                self.errors.insert(name, msg);
                return;
            }
        }

        // Run custom validators in order; first failure wins
        for validator in &entry.config.validators {
            if let Err(msg) = validator(&value) {
                self.errors.insert(name, msg);
                return;
            }
        }

        // If we get here, the field is valid — remove any stale error
        self.errors.remove(&name);
    }
}

// ---------------------------------------------------------------------------
// Focus management
// ---------------------------------------------------------------------------

impl Form {
    /// Focus a field by name. No-op if the name is not found.
    pub fn focus_field(&mut self, name: &str) {
        if let Some(idx) = self.fields.iter().position(|e| e.name == name) {
            self.focused = Some(idx);
        }
    }

    /// Move focus to the next field (wrapping around).
    pub fn focus_next(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        // Run blur validation on the field we are leaving
        if let Some(prev) = self.focused {
            if self.fields[prev].config.validate_on == ValidateOn::Blur {
                self.validate_field(prev);
            }
        }
        self.focused = Some(match self.focused {
            Some(i) => (i + 1) % self.fields.len(),
            None => 0,
        });
    }

    /// Move focus to the previous field (wrapping around).
    pub fn focus_previous(&mut self) {
        if self.fields.is_empty() {
            return;
        }
        // Run blur validation on the field we are leaving
        if let Some(prev) = self.focused {
            if self.fields[prev].config.validate_on == ValidateOn::Blur {
                self.validate_field(prev);
            }
        }
        self.focused = Some(match self.focused {
            Some(0) | None => self.fields.len().saturating_sub(1),
            Some(i) => i - 1,
        });
    }

    /// Focus the first field.
    pub fn focus_first(&mut self) {
        if !self.fields.is_empty() {
            self.focused = Some(0);
        }
    }

    /// Remove focus from all fields.
    pub fn unfocus(&mut self) {
        // Run blur validation on the field we are leaving
        if let Some(prev) = self.focused {
            if self.fields[prev].config.validate_on == ValidateOn::Blur {
                self.validate_field(prev);
            }
        }
        self.focused = None;
    }

    /// Get the name of the currently focused field, if any.
    pub fn focused_field(&self) -> Option<&str> {
        self.focused.map(|i| self.fields[i].name.as_str())
    }

    /// Whether the currently focused field captures text input.
    ///
    /// Callers can use this to decide whether to suppress the application
    /// keymap while the form is active.
    pub fn captures_text(&self) -> bool {
        self.focused
            .map(|i| self.fields[i].field.captures_text())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl Form {
    /// Render the entire form into the given area using the configured layout.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let t = theme();

        match &self.form_layout {
            FormLayout::Vertical => self.render_vertical(frame, area, &t),
            FormLayout::Grid { columns } => self.render_grid(frame, area, *columns, &t),
        }
    }

    /// Render a single field by name into the given area.
    ///
    /// This is useful when the caller manages its own layout and wants to
    /// render individual fields into specific areas.
    pub fn render_field(&self, frame: &mut Frame, area: Rect, field: &str) {
        let t = theme();
        if let Some((idx, entry)) = self
            .fields
            .iter()
            .enumerate()
            .find(|(_, e)| e.name == field)
        {
            let focused = self.focused == Some(idx);
            let error = self.errors.get(&entry.name).map(String::as_str);
            self.render_single_field(frame, area, entry, focused, error, &t);
        }
    }

    /// The total height the form would need for rendering.
    pub fn height(&self) -> u16 {
        let entries: Vec<(Option<&str>, u16, bool)> = self
            .fields
            .iter()
            .map(|e| {
                let label = e.config.label.as_deref();
                let fh = e.field.height();
                let has_err = self.errors.contains_key(&e.name);
                (label, fh, has_err)
            })
            .collect();

        layout::calculate_total_height(&entries, &self.form_layout)
    }

    // -- private rendering helpers --

    fn render_vertical(
        &self,
        frame: &mut Frame,
        area: Rect,
        t: &crate::theme::Theme,
    ) {
        let mut y = area.y;

        for (idx, entry) in self.fields.iter().enumerate() {
            let focused = self.focused == Some(idx);
            let error = self.errors.get(&entry.name).map(String::as_str);
            let label = entry.config.label.as_deref();
            let field_h = entry.field.height();
            let total_h = layout::calculate_field_height(label, field_h, error.is_some());

            if y + total_h > area.y + area.height {
                break; // no more room
            }

            let field_area = Rect::new(area.x, y, area.width, total_h);
            self.render_single_field(frame, field_area, entry, focused, error, t);

            y += total_h;

            // Spacing between fields
            if idx + 1 < self.fields.len() {
                y += 1;
            }
        }
    }

    fn render_grid(
        &self,
        frame: &mut Frame,
        area: Rect,
        columns: u16,
        t: &crate::theme::Theme,
    ) {
        let cols = columns.max(1) as usize;
        let col_width = area.width / columns.max(1);
        let mut y = area.y;

        for (row_idx, chunk) in self.fields.chunks(cols).enumerate() {
            // Calculate the tallest field in this row
            let row_height = chunk
                .iter()
                .map(|e| {
                    let label = e.config.label.as_deref();
                    let fh = e.field.height();
                    let has_err = self.errors.contains_key(&e.name);
                    layout::calculate_field_height(label, fh, has_err)
                })
                .max()
                .unwrap_or(0);

            if y + row_height > area.y + area.height {
                break;
            }

            for (col_idx, entry) in chunk.iter().enumerate() {
                let global_idx = row_idx * cols + col_idx;
                let focused = self.focused == Some(global_idx);
                let error = self.errors.get(&entry.name).map(String::as_str);

                let x = area.x + (col_idx as u16) * col_width;
                // Last column gets remaining width to avoid rounding gaps
                let w = if col_idx + 1 == cols || col_idx + 1 == chunk.len() {
                    (area.x + area.width).saturating_sub(x)
                } else {
                    col_width
                };

                let cell_area = Rect::new(x, y, w, row_height);
                self.render_single_field(frame, cell_area, entry, focused, error, t);
            }

            y += row_height;

            // Spacing between rows
            if row_idx + 1 < self.fields.len().div_ceil(cols) {
                y += 1;
            }
        }
    }

    /// Render a label + field widget + error message into `area`.
    fn render_single_field(
        &self,
        frame: &mut Frame,
        area: Rect,
        entry: &FormEntry,
        focused: bool,
        error: Option<&str>,
        t: &crate::theme::Theme,
    ) {
        let mut y = area.y;

        // -- Label --
        if let Some(label_text) = &entry.config.label {
            if y < area.y + area.height {
                let label_style = if focused {
                    t.title_style()
                } else {
                    t.text_style()
                };
                let required_marker = if entry.config.required { " *" } else { "" };
                let label = Paragraph::new(format!("{label_text}{required_marker}"))
                    .style(label_style);
                let label_area = Rect::new(area.x, y, area.width, 1);
                frame.render_widget(label, label_area);
                y += 1;
            }
        }

        // -- Field widget --
        let field_h = entry.field.height();
        if y + field_h <= area.y + area.height {
            let widget_area = Rect::new(area.x, y, area.width, field_h);
            entry.field.render(frame, widget_area, focused);
            y += field_h;
        }

        // -- Error message --
        if let Some(err_text) = error {
            if y < area.y + area.height {
                let err = Paragraph::new(err_text).style(t.error_style());
                let err_area = Rect::new(area.x, y, area.width, 1);
                frame.render_widget(err, err_area);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl Form {
    fn find_entry(&self, name: &str) -> Option<&FormEntry> {
        self.fields.iter().find(|e| e.name == name)
    }

    fn find_entry_mut(&mut self, name: &str) -> Option<&mut FormEntry> {
        self.fields.iter_mut().find(|e| e.name == name)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// A minimal mock field for testing the Form coordinator.
    struct MockTextField {
        text: String,
    }

    impl MockTextField {
        fn new(initial: &str) -> Self {
            Self {
                text: initial.to_string(),
            }
        }
    }

    impl FormField for MockTextField {
        fn field_value(&self) -> FieldValue {
            FieldValue::Text(self.text.clone())
        }

        fn set_field_value(&mut self, value: FieldValue) {
            if let FieldValue::Text(t) = value {
                self.text = t;
            }
        }

        fn clear(&mut self) {
            self.text.clear();
        }

        fn captures_text(&self) -> bool {
            true
        }

        fn handle_key_event(&mut self, event: &KeyEvent) -> bool {
            if let KeyCode::Char(c) = event.code {
                self.text.push(c);
                return true;
            }
            false
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {}

        fn height(&self) -> u16 {
            1
        }
    }

    /// A mock non-text field (like a checkbox).
    struct MockToggle {
        on: bool,
    }

    impl MockToggle {
        fn new(initial: bool) -> Self {
            Self { on: initial }
        }
    }

    impl FormField for MockToggle {
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
            if event.code == KeyCode::Char(' ') {
                self.on = !self.on;
                return true;
            }
            false
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _focused: bool) {}

        fn height(&self) -> u16 {
            1
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn test_new_form_auto_focuses_first_field() {
        let form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new());
        assert_eq!(form.focused_field(), Some("name"));
    }

    #[test]
    fn test_tab_cycles_focus() {
        let mut form = Form::new()
            .field("a", MockTextField::new(""), FieldConfig::new())
            .field("b", MockToggle::new(false), FieldConfig::new());

        assert_eq!(form.focused_field(), Some("a"));

        let action = form.handle_event(&key(KeyCode::Tab));
        assert_eq!(action, FormAction::Consumed);
        assert_eq!(form.focused_field(), Some("b"));

        // Tab again wraps
        let action = form.handle_event(&key(KeyCode::Tab));
        assert_eq!(action, FormAction::Consumed);
        assert_eq!(form.focused_field(), Some("a"));
    }

    #[test]
    fn test_shift_tab_cycles_backwards() {
        let mut form = Form::new()
            .field("a", MockTextField::new(""), FieldConfig::new())
            .field("b", MockToggle::new(false), FieldConfig::new());

        assert_eq!(form.focused_field(), Some("a"));

        // Shift+Tab from first wraps to last
        let action = form.handle_event(&key_mod(KeyCode::Tab, KeyModifiers::SHIFT));
        assert_eq!(action, FormAction::Consumed);
        assert_eq!(form.focused_field(), Some("b"));
    }

    #[test]
    fn test_backtab_cycles_backwards() {
        let mut form = Form::new()
            .field("a", MockTextField::new(""), FieldConfig::new())
            .field("b", MockToggle::new(false), FieldConfig::new());

        let action = form.handle_event(&key(KeyCode::BackTab));
        assert_eq!(action, FormAction::Consumed);
        assert_eq!(form.focused_field(), Some("b"));
    }

    #[test]
    fn test_escape_consumed_for_text_field() {
        let mut form = Form::new()
            .field("name", MockTextField::new("hello"), FieldConfig::new());

        let action = form.handle_event(&key(KeyCode::Esc));
        assert_eq!(action, FormAction::Consumed);
    }

    #[test]
    fn test_escape_ignored_for_non_text_field() {
        let mut form = Form::new()
            .field("toggle", MockToggle::new(false), FieldConfig::new());

        let action = form.handle_event(&key(KeyCode::Esc));
        assert_eq!(action, FormAction::Ignored);
    }

    #[test]
    fn test_enter_submits_on_non_textarea() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new());

        let action = form.handle_event(&key(KeyCode::Enter));
        assert_eq!(action, FormAction::Submit);
    }

    #[test]
    fn test_enter_does_not_submit_on_textarea() {
        let mut form = Form::new()
            .field("bio", MockTextField::new(""), FieldConfig::new().textarea());

        // Enter on textarea should be forwarded to the field, not submit
        let action = form.handle_event(&key(KeyCode::Enter));
        // MockTextField doesn't handle Enter, so it falls through to step 6
        // (captures_text = true, not consumed) -> Consumed
        assert_eq!(action, FormAction::Consumed);
    }

    #[test]
    fn test_ctrl_enter_always_submits() {
        let mut form = Form::new()
            .field("bio", MockTextField::new(""), FieldConfig::new().textarea());

        let action = form.handle_event(&key_mod(KeyCode::Enter, KeyModifiers::CONTROL));
        assert_eq!(action, FormAction::Submit);
    }

    #[test]
    fn test_char_routed_to_text_field_returns_value_changed() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new());

        let action = form.handle_event(&key(KeyCode::Char('a')));
        assert_eq!(action, FormAction::ValueChanged("name".to_string()));

        // Verify the value was updated
        let val = form.value("name").unwrap();
        assert_eq!(val, FieldValue::Text("a".to_string()));
    }

    #[test]
    fn test_unhandled_key_on_text_field_still_consumed() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new());

        // F1 is not handled by MockTextField but field captures text
        let action = form.handle_event(&key(KeyCode::F(1)));
        assert_eq!(action, FormAction::Consumed);
    }

    #[test]
    fn test_unhandled_key_on_non_text_field_is_ignored() {
        let mut form = Form::new()
            .field("toggle", MockToggle::new(false), FieldConfig::new());

        // 'a' is not handled by MockToggle and it doesn't capture text
        let action = form.handle_event(&key(KeyCode::Char('a')));
        assert_eq!(action, FormAction::Ignored);
    }

    #[test]
    fn test_space_on_toggle_returns_value_changed() {
        let mut form = Form::new()
            .field("toggle", MockToggle::new(false), FieldConfig::new());

        let action = form.handle_event(&key(KeyCode::Char(' ')));
        assert_eq!(action, FormAction::ValueChanged("toggle".to_string()));
        assert_eq!(form.bool("toggle"), Some(true));
    }

    #[test]
    fn test_values_snapshot() {
        let form = Form::new()
            .field("name", MockTextField::new("Alice"), FieldConfig::new())
            .field("active", MockToggle::new(true), FieldConfig::new());

        let vals = form.values();
        assert_eq!(vals.text("name"), Some("Alice"));
        assert_eq!(vals.bool("active"), Some(true));
    }

    #[test]
    fn test_set_value_and_clear() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new())
            .field("active", MockToggle::new(false), FieldConfig::new());

        form.set_text("name", "Bob");
        form.set_bool("active", true);

        assert_eq!(
            form.value("name"),
            Some(FieldValue::Text("Bob".to_string()))
        );
        assert_eq!(form.bool("active"), Some(true));

        form.clear();
        assert_eq!(
            form.value("name"),
            Some(FieldValue::Text(String::new()))
        );
        assert_eq!(form.bool("active"), Some(false));
    }

    #[test]
    fn test_set_values_bulk() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new())
            .field("active", MockToggle::new(false), FieldConfig::new());

        let mut vals = FormValues::new();
        vals.insert("name".to_string(), FieldValue::Text("Eve".to_string()));
        vals.insert("active".to_string(), FieldValue::Bool(true));
        form.set_values(vals);

        assert_eq!(
            form.value("name"),
            Some(FieldValue::Text("Eve".to_string()))
        );
        assert_eq!(form.bool("active"), Some(true));
    }

    #[test]
    fn test_validation_required() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new().required());

        assert!(!form.validate());
        assert!(form.field_error("name").is_some());

        form.set_text("name", "Alice");
        assert!(form.validate());
        assert!(form.is_valid());
        assert!(form.field_error("name").is_none());
    }

    #[test]
    fn test_validation_custom_validator() {
        let mut form = Form::new().field(
            "email",
            MockTextField::new("bad"),
            FieldConfig::new().validator(|v| {
                if v.contains('@') {
                    Ok(())
                } else {
                    Err("Must contain @".to_string())
                }
            }),
        );

        assert!(!form.validate());
        assert_eq!(form.field_error("email"), Some("Must contain @"));

        form.set_text("email", "a@b.com");
        assert!(form.validate());
    }

    #[test]
    fn test_clear_errors() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new().required());

        form.validate();
        assert!(!form.is_valid());

        form.clear_errors();
        assert!(form.is_valid());
    }

    #[test]
    fn test_focus_field_by_name() {
        let mut form = Form::new()
            .field("a", MockTextField::new(""), FieldConfig::new())
            .field("b", MockToggle::new(false), FieldConfig::new());

        form.focus_field("b");
        assert_eq!(form.focused_field(), Some("b"));

        form.focus_field("nonexistent");
        assert_eq!(form.focused_field(), Some("b")); // unchanged
    }

    #[test]
    fn test_focus_first_and_unfocus() {
        let mut form = Form::new()
            .field("a", MockTextField::new(""), FieldConfig::new())
            .field("b", MockToggle::new(false), FieldConfig::new());

        form.focus_field("b");
        form.focus_first();
        assert_eq!(form.focused_field(), Some("a"));

        form.unfocus();
        assert_eq!(form.focused_field(), None);
    }

    #[test]
    fn test_captures_text_reflects_focused_field() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new())
            .field("toggle", MockToggle::new(false), FieldConfig::new());

        // Focused on text field
        assert!(form.captures_text());

        form.focus_field("toggle");
        assert!(!form.captures_text());

        form.unfocus();
        assert!(!form.captures_text());
    }

    #[test]
    fn test_no_fields_handle_event_ignored() {
        let mut form = Form::new();
        let action = form.handle_event(&key(KeyCode::Char('a')));
        assert_eq!(action, FormAction::Ignored);
    }

    #[test]
    fn test_non_key_event_ignored() {
        let mut form = Form::new()
            .field("name", MockTextField::new(""), FieldConfig::new());

        let action = form.handle_event(&Event::Resize(80, 24));
        assert_eq!(action, FormAction::Ignored);
    }

    #[test]
    fn test_height_calculation() {
        let form = Form::new()
            .field("a", MockTextField::new(""), FieldConfig::new().label("Name"))
            .field("b", MockToggle::new(false), FieldConfig::new());

        // "a": label(1) + field(1) = 2
        // spacing: 1
        // "b": no label, field(1) = 1
        // total = 4
        assert_eq!(form.height(), 4);
    }
}
