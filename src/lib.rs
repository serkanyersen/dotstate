//! DotState - A modern, secure, and user-friendly dotfile manager
//!
//! This library provides the core functionality for managing dotfiles,
//! syncing with git repositories, and managing profiles.

// Core modules
pub mod app;
pub mod cli;
pub mod components;
pub mod config;
pub mod dotfile_candidates;
pub mod file_manager;
pub mod git;
pub mod github;
pub mod keymap;
pub mod styles;
pub mod tui;
pub mod ui;
pub mod utils;
pub mod version_check;
pub mod widgets;

// Re-exports for convenience
pub use config::Config;
pub use file_manager::FileManager;
pub use utils::ProfileManifest;
pub use utils::SymlinkManager;

// Keymap re-exports (used by Config and for external API)
pub use keymap::{Action, KeyBinding, Keymap, KeymapPreset};
