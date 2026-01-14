// Component-based architecture for dotstate TUI

pub mod component;

pub mod file_preview;
pub mod footer;
pub mod github_auth;
pub mod header;
pub mod help_overlay;
pub mod main_menu;
pub mod message;
pub mod message_box;

pub mod synced_files;

pub use component::{Component, ComponentAction};
// Footer and InputField are used directly via their module paths
// pub use footer::Footer;
// pub use input_field::InputField;

pub use github_auth::GitHubAuthComponent;
pub use main_menu::{MainMenuComponent, MenuItem};
pub use message::MessageComponent;

pub use synced_files::SyncedFilesComponent;
