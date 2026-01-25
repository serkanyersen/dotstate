//! CLI module for DotState command-line interface.
//!
//! This module provides a modular structure for CLI commands:
//! - `common` - Shared utilities (CliContext, prompts, output helpers)
//! - `sync` - Sync with remote repository
//! - `files` - File management (list, add, remove)
//! - `profiles` - Profile activation/deactivation
//! - `packages` - Package management
//! - `doctor` - Diagnostics
//! - `info` - Help, logs, config, repository info
//! - `upgrade` - Update checker

mod common;
mod completions;
mod doctor;
mod files;
mod info;
pub mod packages;
mod profiles;
mod sync;
mod upgrade;

// Re-export common utilities for use by CLI commands
pub use common::*;

// Re-export packages command enum for external use
pub use packages::PackagesCommand;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

/// A friendly TUI tool for managing dotfiles with GitHub sync
#[derive(Parser, Debug)]
#[command(name = "dotstate", version, about = "A friendly TUI tool for managing dotfiles with GitHub sync", long_about = None, disable_help_subcommand = true, arg_required_else_help = false)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Disable colors in the TUI (also respects NO_COLOR env var)
    #[arg(long, global = true)]
    pub no_colors: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Sync with remote: commit, pull (with rebase), and push
    Sync {
        /// Custom commit message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List all synced files
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
    /// Add a file to sync
    Add {
        /// Path to the file to add
        path: PathBuf,
        /// Add as a common file (shared across all profiles)
        #[arg(long)]
        common: bool,
    },
    /// Remove a file from sync
    Remove {
        /// Path to the file to remove (relative to home directory, e.g., ".zshrc")
        path: String,
        /// Remove from common files (shared across all profiles)
        #[arg(long)]
        common: bool,
    },
    /// Activate the symlinks, restores app state after deactivation.
    Activate,
    /// Deactivate symlinks. this might be useful if you are going to uninstall dotstate or you need the original files.
    Deactivate,
    /// Run diagnostics and optionally fix issues
    Doctor {
        /// Attempt to auto-fix detected issues
        #[arg(long)]
        fix: bool,
        /// Show detailed diagnostic information
        #[arg(short, long)]
        verbose: bool,
        /// Output results as JSON for scripting
        #[arg(long)]
        json: bool,
    },
    /// Shows logs location and how to view them
    Logs,
    /// Configuration file location
    Config,
    /// Repository location
    Repository,
    /// Show help for a specific command
    Help {
        /// Command to show help for
        command: Option<String>,
    },
    /// Check for updates and optionally upgrade DotState
    Upgrade {
        /// Check for updates without prompting to install
        #[arg(long)]
        check: bool,
    },
    /// Manage packages for profiles
    Packages {
        #[command(subcommand)]
        command: PackagesCommand,
    },
    /// Generate command-line completions
    #[clap(alias = "completion")]
    Completions {
        /// The shell to generate completions for
        shell: Option<Shell>,
    },
}

impl Cli {
    /// Execute the CLI command
    pub fn execute(self) -> Result<()> {
        match self.command {
            Some(Commands::Sync { message }) => sync::execute(message),
            Some(Commands::List { verbose }) => files::cmd_list(verbose),
            Some(Commands::Add { path, common }) => files::cmd_add(path, common),
            Some(Commands::Remove { path, common }) => files::cmd_remove(path, common),
            Some(Commands::Activate) => profiles::cmd_activate(),
            Some(Commands::Deactivate) => profiles::cmd_deactivate(),
            Some(Commands::Doctor { fix, verbose, json }) => doctor::execute(fix, verbose, json),
            Some(Commands::Help { command }) => info::cmd_help(command),
            Some(Commands::Logs) => info::cmd_logs(),
            Some(Commands::Config) => info::cmd_config(),
            Some(Commands::Repository) => info::cmd_repository(),
            Some(Commands::Upgrade { check }) => upgrade::execute(check),
            Some(Commands::Packages { command }) => packages::execute(command),
            Some(Commands::Completions { shell }) => completions::generate(shell),
            None => {
                // No command provided, launch TUI
                Ok(())
            }
        }
    }
}
