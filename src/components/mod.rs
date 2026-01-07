// Component-based architecture for dotstate TUI

pub mod component;
pub mod dotfile_selection;
pub mod file_preview;
pub mod footer;
pub mod provider_setup;
pub mod header;
pub mod input_field;
pub mod main_menu;
pub mod message;
pub mod message_box;
pub mod package_manager;
pub mod profile_manager;
pub mod push_changes;
pub mod synced_files;

pub use component::{Component, ComponentAction};
// Footer and InputField are used directly via their module paths
// pub use footer::Footer;
// pub use input_field::InputField;
pub use dotfile_selection::DotfileSelectionComponent;
pub use provider_setup::ProviderSetupComponent;
pub use main_menu::{MainMenuComponent, MenuItem};
pub use message::MessageComponent;
pub use profile_manager::ProfileManagerComponent;
pub use push_changes::PushChangesComponent;
pub use synced_files::SyncedFilesComponent;
// PackageManagerComponent is used directly via module path in app.rs
