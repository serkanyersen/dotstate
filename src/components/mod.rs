// Component-based architecture for dotstate TUI

pub mod component;
pub mod dialog;
pub mod file_preview;
pub mod footer;
pub mod header;
pub mod help_overlay;
pub mod message;
pub mod message_box;
pub mod popup;

pub use component::{Component, ComponentAction};
pub use dialog::{Dialog, DialogVariant};
pub use message::MessageComponent;
pub use popup::{Popup, PopupRenderResult};
