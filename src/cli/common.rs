//! Common CLI utilities shared across all CLI commands.
//!
//! This module provides:
//! - `CliContext`: Shared context for loading config/manifest
//! - Output helpers: `print_success`, `print_error`, `print_warning`, `print_info`
//! - Prompt helpers: `prompt_string`, `prompt_string_optional`, `prompt_select`, `prompt_confirm`

use crate::config::Config;
use crate::services::PackageService;
use crate::utils::profile_manifest::PackageManager;
use crate::utils::ProfileManifest;
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;

/// Shared context for CLI commands.
///
/// Provides access to configuration, profile manifest, and common utilities.
pub struct CliContext {
    /// The loaded configuration
    pub config: Config,
    /// The profile manifest from the repository
    pub manifest: ProfileManifest,
    /// Path to the config file
    pub config_path: PathBuf,
}

impl CliContext {
    /// Load the CLI context from the configuration file.
    ///
    /// This will exit with an error message if:
    /// - Config file cannot be loaded
    /// - Repository is not configured
    /// - Profile manifest cannot be loaded
    pub fn load() -> Result<Self> {
        let config_path = crate::utils::get_config_path();

        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        // Check if repository is configured
        if !config.is_repo_configured() {
            print_error("Repository not configured. Please run 'dotstate' to set up repository.");
            std::process::exit(1);
        }

        let manifest = ProfileManifest::load_or_backfill(&config.repo_path)
            .context("Failed to load profile manifest")?;

        Ok(Self {
            config,
            manifest,
            config_path,
        })
    }

    /// Resolve a profile name: returns provided profile or falls back to active profile.
    ///
    /// # Arguments
    /// * `profile` - Optional profile name provided by the user
    ///
    /// # Returns
    /// The resolved profile name
    #[must_use]
    pub fn resolve_profile(&self, profile: Option<&str>) -> String {
        profile.map_or_else(
            || self.config.active_profile.clone(),
            std::string::ToString::to_string,
        )
    }

    /// Check if the given profile name is the active profile.
    ///
    /// # Arguments
    /// * `profile_name` - Profile name to check
    ///
    /// # Returns
    /// `true` if the profile is the active profile
    #[must_use]
    pub fn is_active_profile(&self, profile_name: &str) -> bool {
        self.config.active_profile == profile_name
    }

    /// Check if a profile exists in the manifest.
    ///
    /// # Arguments
    /// * `profile_name` - Profile name to check
    ///
    /// # Returns
    /// `true` if the profile exists
    #[must_use]
    pub fn profile_exists(&self, profile_name: &str) -> bool {
        self.manifest
            .profiles
            .iter()
            .any(|p| p.name == profile_name)
    }

    /// Get a reference to a profile by name.
    ///
    /// # Arguments
    /// * `profile_name` - Profile name to find
    ///
    /// # Returns
    /// Reference to the profile info if found
    #[must_use]
    pub fn get_profile(&self, profile_name: &str) -> Option<&crate::utils::ProfileInfo> {
        self.manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
    }

    /// Save the manifest to disk.
    pub fn save_manifest(&self) -> Result<()> {
        self.manifest
            .save(&self.config.repo_path)
            .context("Failed to save profile manifest")
    }
}

// =============================================================================
// Output Helpers
// =============================================================================

/// Print a success message with a checkmark prefix.
///
/// # Arguments
/// * `msg` - The message to print
pub fn print_success(msg: &str) {
    println!("\u{2713} {msg}");
}

/// Print an error message with an X prefix to stderr.
///
/// # Arguments
/// * `msg` - The message to print
pub fn print_error(msg: &str) {
    eprintln!("\u{2717} {msg}");
}

/// Print a warning message with a warning sign prefix.
///
/// # Arguments
/// * `msg` - The message to print
pub fn print_warning(msg: &str) {
    println!("\u{26A0}\u{FE0F} {msg}");
}

/// Print an info message with an info sign prefix.
///
/// # Arguments
/// * `msg` - The message to print
pub fn print_info(msg: &str) {
    println!("\u{2139}\u{FE0F} {msg}");
}

// =============================================================================
// Prompt Helpers
// =============================================================================

/// Prompt the user for a string input with an optional default value.
///
/// # Arguments
/// * `label` - The prompt label to display
/// * `default` - Optional default value shown in brackets
///
/// # Returns
/// The user's input, or the default if they pressed Enter
pub fn prompt_string(label: &str, default: Option<&str>) -> Result<String> {
    if let Some(def) = default {
        print!("{label} [{def}]: ");
    } else {
        print!("{label}: ");
    }
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(default.unwrap_or("").to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

/// Prompt the user for an optional string input.
///
/// # Arguments
/// * `label` - The prompt label to display
///
/// # Returns
/// `Some(input)` if the user entered text, `None` if they pressed Enter
pub fn prompt_string_optional(label: &str) -> Result<Option<String>> {
    print!("{label} (optional): ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

/// Prompt the user to select from a numbered list of options.
///
/// # Arguments
/// * `label` - The prompt label to display
/// * `options` - List of options to display (shown as 1-indexed)
///
/// # Returns
/// The 0-indexed position of the selected option
///
/// # Panics
/// Exits with error if options is empty or user enters invalid input
pub fn prompt_select(label: &str, options: &[&str]) -> Result<usize> {
    if options.is_empty() {
        print_error("No options available for selection");
        std::process::exit(1);
    }

    println!("{label}:");
    for (i, option) in options.iter().enumerate() {
        println!("  {}. {}", i + 1, option);
    }
    print!("Enter choice [1-{}]: ", options.len());
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim();
    match trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= options.len() => Ok(n - 1),
        _ => {
            print_error(&format!(
                "Invalid choice. Please enter a number between 1 and {}",
                options.len()
            ));
            std::process::exit(1);
        }
    }
}

/// Prompt the user to select from a numbered list of options with optional suffixes.
///
/// # Arguments
/// * `label` - The prompt label to display
/// * `options` - List of (name, `optional_suffix`) tuples to display (shown as 1-indexed)
///
/// # Returns
/// The 0-indexed position of the selected option
///
/// # Panics
/// Exits with error if options is empty or user enters invalid input
pub fn prompt_select_with_suffix(label: &str, options: &[(&str, Option<&str>)]) -> Result<usize> {
    if options.is_empty() {
        print_error("No options available for selection");
        std::process::exit(1);
    }

    println!("{label}:");
    for (i, (name, suffix)) in options.iter().enumerate() {
        if let Some(s) = suffix {
            println!("  {}. {} {}", i + 1, name, s);
        } else {
            println!("  {}. {}", i + 1, name);
        }
    }
    print!("Enter choice [1-{}]: ", options.len());
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim();
    match trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= options.len() => Ok(n - 1),
        _ => {
            print_error(&format!(
                "Invalid choice. Please enter a number between 1 and {}",
                options.len()
            ));
            std::process::exit(1);
        }
    }
}

/// Prompt the user for a yes/no confirmation.
///
/// # Arguments
/// * `message` - The confirmation message to display
///
/// # Returns
/// `true` if the user confirmed (y/yes), `false` otherwise
pub fn prompt_confirm(message: &str) -> Result<bool> {
    print!("{message} [y/N]: ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim().to_lowercase();
    Ok(trimmed == "y" || trimmed == "yes")
}

// =============================================================================
// Package Manager Helpers
// =============================================================================

/// Get all supported package managers (for non-active profiles).
fn all_package_managers() -> Vec<PackageManager> {
    vec![
        PackageManager::Brew,
        PackageManager::Apt,
        PackageManager::Yum,
        PackageManager::Dnf,
        PackageManager::Pacman,
        PackageManager::Snap,
        PackageManager::Cargo,
        PackageManager::Npm,
        PackageManager::Pip,
        PackageManager::Pip3,
        PackageManager::Gem,
        PackageManager::Custom,
    ]
}

/// Prompt for package manager selection.
///
/// For active profiles: shows only managers available on current OS with "(installed)" markers.
/// For non-active profiles: shows ALL supported managers (profile may be for different OS).
///
/// # Arguments
/// * `is_active_profile` - Whether this is the active profile
///
/// # Returns
/// The selected `PackageManager`
pub fn prompt_manager(is_active_profile: bool) -> Result<PackageManager> {
    // For active profile: use OS-detected managers with installed markers
    // For non-active profile: show ALL managers (may be for different OS)
    let managers = if is_active_profile {
        PackageService::get_available_managers()
    } else {
        all_package_managers()
    };

    let options: Vec<(&str, Option<&str>)> = managers
        .iter()
        .map(|m| {
            let name = match m {
                PackageManager::Brew => "brew",
                PackageManager::Apt => "apt",
                PackageManager::Yum => "yum",
                PackageManager::Dnf => "dnf",
                PackageManager::Pacman => "pacman",
                PackageManager::Snap => "snap",
                PackageManager::Cargo => "cargo",
                PackageManager::Npm => "npm",
                PackageManager::Pip => "pip",
                PackageManager::Pip3 => "pip3",
                PackageManager::Gem => "gem",
                PackageManager::Custom => "custom",
            };
            // Only show "(installed)" markers for active profile
            let suffix = if is_active_profile && PackageService::is_manager_installed(m) {
                Some("(installed)")
            } else {
                None
            };
            (name, suffix)
        })
        .collect();

    let index = prompt_select_with_suffix("Manager", &options)?;
    Ok(managers[index].clone())
}

/// Parse package manager from string.
///
/// # Arguments
/// * `s` - The string to parse
///
/// # Returns
/// The parsed `PackageManager`, or None if invalid
#[must_use]
pub fn parse_manager(s: &str) -> Option<PackageManager> {
    match s.to_lowercase().as_str() {
        "brew" | "homebrew" => Some(PackageManager::Brew),
        "apt" | "apt-get" => Some(PackageManager::Apt),
        "yum" => Some(PackageManager::Yum),
        "dnf" => Some(PackageManager::Dnf),
        "pacman" => Some(PackageManager::Pacman),
        "snap" => Some(PackageManager::Snap),
        "cargo" => Some(PackageManager::Cargo),
        "npm" => Some(PackageManager::Npm),
        "pip" => Some(PackageManager::Pip),
        "pip3" => Some(PackageManager::Pip3),
        "gem" => Some(PackageManager::Gem),
        "custom" => Some(PackageManager::Custom),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_output_helpers_compile() {
        // Just verify that the output helper unicode characters are valid
        // We can't easily test stdout/stderr in unit tests
        // but we can verify the format strings are valid
        let _ = format!("\u{2713} {}", "test"); // checkmark
        let _ = format!("\u{2717} {}", "test"); // X mark
        let _ = format!("\u{26A0}\u{FE0F} {}", "test"); // warning sign
        let _ = format!("\u{2139}\u{FE0F} {}", "test"); // info sign
    }
}
