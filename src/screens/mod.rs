//! Screen controllers for the application.
//!
//! This module provides screen controllers that implement the `Screen` trait.
//! Each screen controller owns its state and handles both rendering and events.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────┐
//! │                      App                                │
//! │  ┌────────────────────────────────────────────────┐    │
//! │  │               Screen Router                     │    │
//! │  │  match current_screen {                         │    │
//! │  │    MainMenu => main_menu.handle_event(...)     │    │
//! │  │    GitHubAuth => github_auth.handle_event(...) │    │
//! │  │    ...                                          │    │
//! │  │  }                                              │    │
//! │  └────────────────────────────────────────────────┘    │
//! │                                                         │
//! │  ┌────────────────────────────────────────────────┐    │
//! │  │               Screen Trait                      │    │
//! │  │  - render(frame, area, context)                │    │
//! │  │  - handle_event(event, context) -> Action      │    │
//! │  │  - is_input_focused() -> bool                  │    │
//! │  └────────────────────────────────────────────────┘    │
//! └────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use crate::screens::{Screen, ScreenContext, ScreenAction};
//!
//! struct MyScreen {
//!     state: MyScreenState,
//! }
//!
//! impl Screen for MyScreen {
//!     fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &ScreenContext) -> Result<()> {
//!         // Render the screen
//!         Ok(())
//!     }
//!
//!     fn handle_event(&mut self, event: Event, ctx: &mut ScreenContext) -> Result<ScreenAction> {
//!         // Handle events and return navigation action
//!         Ok(ScreenAction::None)
//!     }
//! }
//! ```

pub mod dotfile_selection;
pub mod github_auth;
pub mod main_menu;
pub mod manage_packages;
pub mod manage_profiles;
pub mod profile_selection;
pub mod screen_trait;
pub mod settings;
pub mod storage_setup;
pub mod sync_with_remote;

pub use dotfile_selection::DotfileSelectionScreen;
pub use github_auth::GitHubAuthScreen;
pub use main_menu::MainMenuScreen;
pub use manage_packages::ManagePackagesScreen;
pub use manage_profiles::ManageProfilesScreen;
pub use profile_selection::ProfileSelectionScreen;
pub use screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
pub use settings::SettingsScreen;
pub use storage_setup::StorageSetupScreen;
pub use sync_with_remote::SyncWithRemoteScreen;
