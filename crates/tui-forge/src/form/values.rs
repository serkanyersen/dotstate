//! Collected form values — a typed wrapper around a `HashMap<String, FieldValue>`.

use std::collections::HashMap;

use super::field::FieldValue;

/// A snapshot of all field values in a form, keyed by field name.
///
/// Returned by the form container after submission and usable for
/// convenient typed extraction:
///
/// ```rust,ignore
/// let vals: FormValues = form.values();
/// let name = vals.text("name").unwrap_or_default();
/// let agree = vals.bool("agree").unwrap_or(false);
/// ```
pub struct FormValues(pub(crate) HashMap<String, FieldValue>);

impl FormValues {
    /// Create an empty `FormValues`.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert a value for the given key.
    pub fn insert(&mut self, key: impl Into<String>, value: FieldValue) {
        self.0.insert(key.into(), value);
    }

    /// Get the raw [`FieldValue`] for a key.
    pub fn get(&self, key: &str) -> Option<&FieldValue> {
        self.0.get(key)
    }

    /// Extract a `Text` value as a `&str`.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(FieldValue::as_text)
    }

    /// Extract a `Bool` value.
    pub fn bool(&self, key: &str) -> Option<bool> {
        self.0.get(key).and_then(FieldValue::as_bool)
    }

    /// Extract a single `Choice` index.
    pub fn choice(&self, key: &str) -> Option<usize> {
        self.0.get(key).and_then(FieldValue::as_choice)
    }

    /// Extract multiple `Choices` indices.
    pub fn choices(&self, key: &str) -> Option<&[usize]> {
        self.0.get(key).and_then(FieldValue::as_choices)
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &FieldValue)> {
        self.0.iter()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the map is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Default for FormValues {
    fn default() -> Self {
        Self::new()
    }
}
