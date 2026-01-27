//! CLI module for DotState command-line interface.
//!
//! This module provides a modular structure for CLI commands:
//! - `common` - Shared utilities (CliContext, prompts, output helpers)
//! - `packages` - Package management commands
//! - `legacy` - Original CLI commands (sync, list, add, etc.)

mod common;
mod legacy;
pub mod packages;

// Re-export common utilities for use by CLI commands
pub use common::*;

// Re-export legacy CLI for backwards compatibility
// This includes Cli struct, Commands enum, and all existing command implementations
pub use legacy::{Cli, Commands};

// Re-export packages command enum for use in legacy routing
pub use packages::PackagesCommand;
