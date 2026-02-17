//! Field configuration, validation rules, and built-in validators.

use super::field::FieldValue;

/// When validation should be triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidateOn {
    /// Validate only when the form is submitted (default).
    #[default]
    Submit,
    /// Validate when the field loses focus.
    Blur,
    /// Validate on every value change.
    Change,
}

/// Configuration attached to a form field.
///
/// Holds metadata (label, required flag) and an ordered list of validator
/// closures that are run against the field's [`FieldValue`].
///
/// # Builder example
///
/// ```rust,ignore
/// use tui_forge::form::config::FieldConfig;
/// use tui_forge::form::config::validators;
///
/// let cfg = FieldConfig::new()
///     .label("Email")
///     .required()
///     .validate(validators::not_empty())
///     .validate(validators::email())
///     .validate_on(ValidateOn::Blur);
/// ```
/// A boxed validator closure: takes a [`FieldValue`] and returns `Ok(())` or
/// an error message.
pub type ValidatorFn = Box<dyn Fn(&FieldValue) -> Result<(), String>>;

pub struct FieldConfig {
    /// Optional label displayed alongside the field.
    pub label: Option<String>,
    /// Whether the field must have a non-empty value before submission.
    pub required: bool,
    /// Ordered list of validation closures.
    pub validators: Vec<ValidatorFn>,
    /// When the validators should be evaluated.
    pub validate_on: ValidateOn,
    /// Whether this field is a textarea (multi-line text).
    ///
    /// When `true`, plain Enter inserts a newline instead of submitting
    /// the form. Only Ctrl+Enter submits from a textarea field.
    pub is_textarea: bool,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldConfig {
    /// Create a new configuration with default settings.
    pub fn new() -> Self {
        Self {
            label: None,
            required: false,
            validators: Vec::new(),
            validate_on: ValidateOn::Submit,
            is_textarea: false,
        }
    }

    /// Set the field label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Mark the field as required.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Append a validator closure.
    pub fn validate(
        mut self,
        f: impl Fn(&FieldValue) -> Result<(), String> + 'static,
    ) -> Self {
        self.validators.push(Box::new(f));
        self
    }

    /// Set when validation should run.
    pub fn validate_on(mut self, when: ValidateOn) -> Self {
        self.validate_on = when;
        self
    }

    /// Mark this field as a textarea.
    ///
    /// Textarea fields treat plain Enter as a newline insertion rather
    /// than a form submission. Only Ctrl+Enter submits from a textarea.
    pub fn textarea(mut self) -> Self {
        self.is_textarea = true;
        self
    }

    /// Append a plain-string validator (convenience wrapper).
    ///
    /// The closure receives the stringified field value. This is handy when
    /// you only need to validate text and don't want to pattern-match
    /// [`FieldValue`] yourself.
    pub fn validator<F>(mut self, f: F) -> Self
    where
        F: Fn(&str) -> Result<(), String> + 'static,
    {
        self.validators.push(Box::new(move |v: &FieldValue| {
            let s = match v {
                FieldValue::Text(s) => s.clone(),
                FieldValue::Bool(b) => b.to_string(),
                FieldValue::Choice(i) => i.to_string(),
                FieldValue::Choices(v) => format!("{v:?}"),
            };
            f(&s)
        }));
        self
    }

    /// Run all validators against a value, returning the first error (if any).
    pub fn run_validators(&self, value: &FieldValue) -> Result<(), String> {
        for v in &self.validators {
            v(value)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Built-in validators
// ---------------------------------------------------------------------------

/// Ready-made validator closures for common validation patterns.
pub mod validators {
    use super::super::field::FieldValue;

    /// Validate that a string value is non-empty (after trimming).
    ///
    /// This is the plain `&str` version used internally by the form
    /// coordinator for the `required` check. Prefer [`not_empty`] for
    /// the [`FieldValue`]-based API.
    pub fn required(value: &str) -> Result<(), String> {
        if value.trim().is_empty() {
            Err("This field is required".to_string())
        } else {
            Ok(())
        }
    }

    /// Require the text to be at least `n` characters.
    pub fn min_length(n: usize) -> impl Fn(&FieldValue) -> Result<(), String> {
        move |v| match v {
            FieldValue::Text(s) if s.chars().count() >= n => Ok(()),
            FieldValue::Text(s) => Err(format!(
                "Must be at least {} characters (got {})",
                n,
                s.chars().count()
            )),
            _ => Ok(()),
        }
    }

    /// Require the text to be at most `n` characters.
    pub fn max_length(n: usize) -> impl Fn(&FieldValue) -> Result<(), String> {
        move |v| match v {
            FieldValue::Text(s) if s.chars().count() <= n => Ok(()),
            FieldValue::Text(s) => Err(format!(
                "Must be at most {} characters (got {})",
                n,
                s.chars().count()
            )),
            _ => Ok(()),
        }
    }

    /// Require the text to be non-empty (after trimming).
    pub fn not_empty() -> impl Fn(&FieldValue) -> Result<(), String> {
        |v| match v {
            FieldValue::Text(s) if !s.trim().is_empty() => Ok(()),
            FieldValue::Text(_) => Err("Field must not be empty".to_string()),
            _ => Ok(()),
        }
    }

    /// Require the text to look like an email address.
    ///
    /// This is a lightweight check (contains `@` and a `.` after the `@`),
    /// not a full RFC 5322 parser.
    pub fn email() -> impl Fn(&FieldValue) -> Result<(), String> {
        |v| match v {
            FieldValue::Text(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    return Ok(()); // empty is handled by `not_empty` / `required`
                }
                let at_pos = trimmed.find('@');
                match at_pos {
                    Some(pos) if pos > 0 => {
                        let domain = &trimmed[pos + 1..];
                        if domain.contains('.')
                            && !domain.ends_with('.')
                            && !domain.starts_with('.')
                        {
                            Ok(())
                        } else {
                            Err("Invalid email address".to_string())
                        }
                    }
                    _ => Err("Invalid email address".to_string()),
                }
            }
            _ => Ok(()),
        }
    }

    /// Create a validator from a function that operates on the raw text.
    ///
    /// Non-text values pass through without error.
    pub fn custom(
        f: impl Fn(&str) -> Result<(), String> + 'static,
    ) -> impl Fn(&FieldValue) -> Result<(), String> {
        move |v| match v {
            FieldValue::Text(s) => f(s),
            _ => Ok(()),
        }
    }
}
