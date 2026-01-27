//! Application services layer.
//!
//! This module contains service modules that encapsulate business logic
//! separated from the UI layer. Services provide a clean interface for
//! performing operations like file syncing, profile management, and
//! package management.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │                   UI Layer                      │
//! │  (App, Screens, Components)                     │
//! └─────────────────────┬───────────────────────────┘
//!                       │
//!                       ▼
//! ┌────────────────────────────────────────────────────┐
//! │               Services Layer                       │
//! │  ┌─────────────┐ ┌───────────────┐ ┌──────────────┐│
//! │  │ SyncService │ │ProfileService │ │PackageService││
//! │  └─────────────┘ └───────────────┘ └──────────────┘│
//! │  ┌───────────────┐                                 │
//! │  │ GitService    │                                 │
//! │  └───────────────┘                                 │
//! └─────────────────────┬──────────────────────────────┘
//!                       │
//!                       ▼
//! ┌──────────────────────────────────────────────────┐
//! │             Infrastructure Layer                 │
//! │  (GitManager, FileManager, Config, etc.)         │
//! └──────────────────────────────────────────────────┘
//! ```

pub mod git_service;
pub mod package_service;
pub mod profile_service;
pub mod storage_setup_service;
pub mod sync_service;

// Re-export common types
pub use git_service::GitService;
pub use package_service::{PackageCheckStatus, PackageCreationParams, PackageService};
pub use profile_service::ProfileService;
pub use storage_setup_service::{StepHandle, StepResult, StorageSetupService};
pub use sync_service::{AddFileResult, RemoveFileResult, SyncService};
