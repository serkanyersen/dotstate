// Component-based architecture for dotstate TUI

pub mod component;
pub mod file_browser;
pub mod file_preview;
pub mod message;
pub mod message_box;

pub mod profile_selection_popup;

pub use component::{Component, ComponentAction};
pub use file_browser::{FileBrowser, FileBrowserFocus, FileBrowserResult};
pub use message::MessageComponent;

pub use profile_selection_popup::{ProfileSelectionPopup, ProfileSelectionResult};
