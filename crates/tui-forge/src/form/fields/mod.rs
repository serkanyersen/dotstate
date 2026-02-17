//! Built-in form field widget implementations.
//!
//! Each type implements the [`FormField`](super::field::FormField) trait,
//! providing a self-contained widget with state, key handling, and rendering.

pub mod text_input;
pub mod text_area;
pub mod password;
pub mod checkbox;
pub mod radio;
pub mod toggle;

pub use text_input::TextInput;
pub use text_area::TextArea;
pub use password::Password;
pub use checkbox::Checkbox;
pub use radio::RadioBox;
pub use toggle::ToggleSwitch;
