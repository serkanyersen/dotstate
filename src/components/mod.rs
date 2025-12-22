// Component-based architecture for dotstate TUI

pub mod component;
pub mod header;
pub mod footer;
pub mod input_field;
pub mod file_preview;
pub mod message_box;
pub mod main_menu;
pub mod github_auth;
pub mod dotfile_selection;
pub mod synced_files;
pub mod message;
pub mod push_changes;
pub mod profile_manager;
pub mod package_manager;

pub use component::{Component, ComponentAction};
// Footer and InputField are used directly via their module paths
// pub use footer::Footer;
// pub use input_field::InputField;
pub use main_menu::{MainMenuComponent, MenuItem};
pub use github_auth::GitHubAuthComponent;
pub use dotfile_selection::DotfileSelectionComponent;
pub use synced_files::SyncedFilesComponent;
pub use message::MessageComponent;
pub use push_changes::PushChangesComponent;
pub use profile_manager::{ProfileManagerComponent};
// PackageManagerComponent is used directly via module path in app.rs

