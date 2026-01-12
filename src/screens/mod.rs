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
pub mod package_manager;
pub mod profile_manager;
pub mod screen_trait;
pub mod sync_with_remote;
pub mod view_synced_files;

pub use dotfile_selection::DotfileSelectionScreen;
pub use github_auth::GitHubAuthScreen;
pub use main_menu::MainMenuScreen;
pub use package_manager::PackageManagerScreen;
pub use profile_manager::ProfileManagerScreen;
pub use screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
pub use sync_with_remote::SyncWithRemoteScreen;
pub use view_synced_files::ViewSyncedFilesScreen;
