//! Profile activation/deactivation commands.

use crate::config::Config;
use crate::utils::symlink_manager::OperationStatus;
use crate::utils::SymlinkManager;
use anyhow::{Context, Result};

/// Execute the activate command.
pub fn cmd_activate() -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let mut config =
        Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        eprintln!("❌ Repository not configured. Please run 'dotstate' to set up repository.");
        std::process::exit(1);
    }

    // Check if already activated
    if config.profile_activated {
        println!(
            "ℹ️  Profile '{}' is already activated.",
            config.active_profile
        );
        println!("   No action needed. Use 'dotstate deactivate' to restore original files.");
        return Ok(());
    }

    // Get active profile info from manifest
    let active_profile_name = config.active_profile.clone();
    let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
        .context("Failed to load profile manifest")?;

    // Resolve the full file list (inheritance chain + common, with overrides)
    let resolved_files = manifest
        .resolve_files(&active_profile_name)
        .context("Failed to resolve files for active profile")?;

    if resolved_files.is_empty() {
        eprintln!("❌ Active profile '{active_profile_name}' has no synced files (including inherited/common).");
        eprintln!("💡 Run 'dotstate' to select and sync files.");
        std::process::exit(1);
    }

    // Show inheritance chain if applicable
    if let Ok(chain) = manifest.inheritance_chain(&active_profile_name) {
        if chain.len() > 1 {
            println!("   Inheritance chain: {}", chain.join(" -> "));
        }
    }

    println!("🔗 Activating profile '{active_profile_name}'...");
    println!(
        "   This will create symlinks for {} files",
        resolved_files.len()
    );

    // Create SymlinkManager and activate with resolved files
    let mut symlink_mgr =
        SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

    let operations = symlink_mgr.activate_resolved(&active_profile_name, &resolved_files)?;

    // Report results
    // Count Success and Skipped as successful (Skipped = symlink already correct)
    let success_count = operations
        .iter()
        .filter(|op| {
            matches!(
                op.status,
                OperationStatus::Success | OperationStatus::Skipped(_)
            )
        })
        .count();
    let failed_count = operations.len() - success_count;

    if failed_count > 0 {
        eprintln!("⚠️  Activated {success_count} files, {failed_count} failed");
        for op in &operations {
            if let OperationStatus::Failed(msg) = &op.status {
                eprintln!("   ❌ {}: {}", op.target.display(), msg);
            }
        }
        std::process::exit(1);
    } else {
        // Mark as activated in config
        config.profile_activated = true;
        config
            .save(&config_path)
            .context("Failed to save configuration")?;

        println!("✅ Successfully activated profile '{active_profile_name}'");
        println!("   {success_count} symlinks created");
    }

    Ok(())
}

/// Execute the deactivate command.
pub fn cmd_deactivate() -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let mut config =
        Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        eprintln!("❌ Repository not configured. Please run 'dotstate' to set up repository.");
        std::process::exit(1);
    }

    println!("🔓 Deactivating dotstate...");
    println!("   This will restore all files from the repository");

    // Create SymlinkManager
    let mut symlink_mgr =
        SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

    // Deactivate all symlinks (profile + common), always restore files
    let operations = symlink_mgr.deactivate_profile_with_restore(&config.active_profile, true)?;

    // Report results
    // Count Success and Skipped as successful (Skipped = symlink already gone or not our symlink)
    let success_count = operations
        .iter()
        .filter(|op| {
            matches!(
                op.status,
                OperationStatus::Success | OperationStatus::Skipped(_)
            )
        })
        .count();
    let failed_count = operations.len() - success_count;

    if operations.is_empty() {
        println!("ℹ️  No symlinks were tracked. Nothing to deactivate.");
    } else if failed_count > 0 {
        eprintln!("⚠️  Deactivated {success_count} files, {failed_count} failed");
        for op in &operations {
            if let OperationStatus::Failed(msg) = &op.status {
                eprintln!("   ❌ {}: {}", op.target.display(), msg);
            }
        }
        std::process::exit(1);
    } else {
        // Mark as deactivated in config
        config.profile_activated = false;
        config
            .save(&config_path)
            .context("Failed to save configuration")?;

        println!("✅ Successfully deactivated dotstate");
        println!("   {success_count} files restored");
        println!("💡 Dotstate is now deactivated. Use 'dotstate activate' to reactivate.");
    }

    Ok(())
}
