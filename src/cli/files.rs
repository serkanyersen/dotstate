//! File management commands: list, add, remove.

use crate::config::Config;
use crate::services::{AddFileResult, RemoveFileResult, SyncService};
use anyhow::{Context, Result};
use std::io::{self, Write};
use std::path::PathBuf;
use tracing::info;

/// Execute the list command.
pub fn cmd_list(verbose: bool) -> Result<()> {
    let config_path = crate::utils::get_config_path();

    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.profile_activated {
        eprintln!("⚠️  Profile is not activated. Please activate your profile first:");
        eprintln!("   dotstate activate");
        eprintln!("\n   This ensures your symlinks are active before listing files.");
        std::process::exit(1);
    }

    // Get manifest
    let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
        .context("Failed to load profile manifest")?;

    // Get common files
    let common_files = manifest.get_common_files();

    // Get active profile's synced files
    let empty_vec = Vec::new();
    let synced_files = manifest
        .profiles
        .iter()
        .find(|p| p.name == config.active_profile)
        .map_or(&empty_vec, |p| &p.synced_files);

    if common_files.is_empty() && synced_files.is_empty() {
        println!("No files are currently synced.");
        return Ok(());
    }

    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let repo_path = &config.repo_path;
    let profile_name = &config.active_profile;

    // Print common files first
    if !common_files.is_empty() {
        println!(
            "Common files ({}) - shared across all profiles:",
            common_files.len()
        );
        for file in common_files {
            let symlink_path = home_dir.join(file);
            let repo_file_path = repo_path.join("common").join(file);

            if verbose {
                let symlink_exists = symlink_path.exists();
                let repo_file_exists = repo_file_path.exists();

                println!("  {file}");
                println!("    Symlink:   {}", symlink_path.display());
                if symlink_exists {
                    if let Ok(metadata) = symlink_path.symlink_metadata() {
                        if metadata.is_symlink() {
                            println!("      Status:  ✓ Active symlink");
                        } else {
                            println!("      Status:  ⚠ File exists but is not a symlink");
                        }
                    } else {
                        println!("      Status:  ✓ Exists");
                    }
                } else {
                    println!("      Status:  ✗ Not found");
                }
                println!("    Storage:   {}", repo_file_path.display());
                if repo_file_exists {
                    println!("      Status:  ✓ Exists");
                } else {
                    println!("      Status:  ✗ Not found");
                }
            } else {
                println!("  {file}");
                println!("    Symlink:   {}", symlink_path.display());
                println!("    Storage:   {}", repo_file_path.display());
            }
        }
        println!();
    }

    // Print profile files
    if !synced_files.is_empty() {
        println!("Profile files ({}) - {}:", synced_files.len(), profile_name);
        for file in synced_files {
            let symlink_path = home_dir.join(file);
            let repo_file_path = repo_path.join(profile_name).join(file);

            if verbose {
                let symlink_exists = symlink_path.exists();
                let repo_file_exists = repo_file_path.exists();

                println!("  {file}");
                println!("    Symlink:   {}", symlink_path.display());
                if symlink_exists {
                    if let Ok(metadata) = symlink_path.symlink_metadata() {
                        if metadata.is_symlink() {
                            println!("      Status:  ✓ Active symlink");
                        } else {
                            println!("      Status:  ⚠ File exists but is not a symlink");
                        }
                    } else {
                        println!("      Status:  ✓ Exists");
                    }
                } else {
                    println!("      Status:  ✗ Not found");
                }
                println!("    Storage:   {}", repo_file_path.display());
                if repo_file_exists {
                    println!("      Status:  ✓ Exists");
                } else {
                    println!("      Status:  ✗ Not found");
                }
            } else {
                println!("  {file}");
                println!("    Symlink:   {}", symlink_path.display());
                println!("    Storage:   {}", repo_file_path.display());
            }
        }
    }

    Ok(())
}

/// Execute the add command.
pub fn cmd_add(path: PathBuf, common: bool) -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    // Resolve path relative to home directory
    let home = dirs::home_dir().context("Failed to get home directory")?;

    let resolved_path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };

    if !resolved_path.exists() {
        eprintln!("❌ File not found: {resolved_path:?}");
        std::process::exit(1);
    }

    // Get relative path from home
    let relative_path = resolved_path
        .strip_prefix(&home)
        .map_or_else(|_| resolved_path.clone(), std::path::Path::to_path_buf);
    let relative_str = relative_path.to_string_lossy().to_string();

    // Show confirmation prompt
    let destination = if common { "common files" } else { "profile" };
    println!(
        "⚠️  Warning: This will move the following path to {destination} and replace it with a symlink:"
    );
    println!("   {}", resolved_path.display());
    if common {
        println!("\n   This file will be shared across ALL profiles.");
    }
    println!("\n   Make sure you know what you are doing.");
    print!("   Continue? [y/N]: ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim().to_lowercase();
    if trimmed != "y" && trimmed != "yes" {
        println!("Cancelled.");
        return Ok(());
    }

    info!(
        "CLI: Adding file to sync: {} (common: {})",
        relative_str, common
    );

    // Use appropriate SyncService method
    let result = if common {
        SyncService::add_common_file_to_sync(
            &config,
            &resolved_path,
            &relative_str,
            config.backup_enabled,
        )?
    } else {
        SyncService::add_file_to_sync(
            &config,
            &resolved_path,
            &relative_str,
            config.backup_enabled,
        )?
    };

    match result {
        AddFileResult::Success => {
            // Check if this is a custom file (not in default dotfile candidates)
            if !common && SyncService::is_custom_file(&relative_str) {
                // Add to config.custom_files if not already there
                let mut config =
                    Config::load_or_create(&config_path).context("Failed to load configuration")?;
                if !config.custom_files.contains(&relative_str) {
                    config.custom_files.push(relative_str.clone());
                    config.save(&config_path)?;
                }
            }
            let dest_type = if common { "common files" } else { "repository" };
            println!("✅ Added {relative_str} to {dest_type} and created symlink");
        }
        AddFileResult::AlreadySynced => {
            let dest_type = if common { "common" } else { "synced" };
            println!("ℹ️  File is already {dest_type}: {relative_str}");
        }
        AddFileResult::ValidationFailed(msg) => {
            eprintln!("❌ {msg}");
            std::process::exit(1);
        }
    }

    Ok(())
}

/// Execute the remove command.
pub fn cmd_remove(path: String, common: bool) -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    // Show confirmation prompt
    let source = if common { "common files" } else { "profile" };
    println!("⚠️  Warning: This will remove {path} from {source} and restore the original file.");
    print!("   Continue? [y/N]: ");
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let trimmed = input.trim().to_lowercase();
    if trimmed != "y" && trimmed != "yes" {
        println!("Cancelled.");
        return Ok(());
    }

    info!(
        "CLI: Removing file from sync: {} (common: {})",
        path, common
    );

    // Use appropriate SyncService method
    let result = if common {
        SyncService::remove_common_file_from_sync(&config, &path)?
    } else {
        SyncService::remove_file_from_sync(&config, &path)?
    };

    match result {
        RemoveFileResult::Success => {
            // Remove from config.custom_files if present
            if !common {
                let mut config =
                    Config::load_or_create(&config_path).context("Failed to load configuration")?;
                config.custom_files.retain(|f| f != &path);
                config.save(&config_path)?;
            }
            let source_type = if common { "common files" } else { "sync" };
            println!("✅ Removed {path} from {source_type} and restored original file");
        }
        RemoveFileResult::NotSynced => {
            let source_type = if common { "common" } else { "synced" };
            println!("ℹ️  File is not {source_type}: {path}");
        }
    }

    Ok(())
}
