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
    let active_profile_files = manifest
        .profiles
        .iter()
        .find(|p| p.name == active_profile_name)
        .ok_or_else(|| anyhow::anyhow!("No active profile found"))?
        .synced_files
        .clone();

    if active_profile_files.is_empty() {
        eprintln!(
            "❌ Active profile '{}' has no synced files.",
            active_profile_name
        );
        eprintln!("💡 Run 'dotstate' to select and sync files.");
        std::process::exit(1);
    }

    println!("🔗 Activating profile '{}'...", active_profile_name);
    println!(
        "   This will create symlinks for {} files",
        active_profile_files.len()
    );

    // Create SymlinkManager
    let mut symlink_mgr =
        SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

    // Activate profile files
    let mut operations =
        symlink_mgr.activate_profile(&active_profile_name, &active_profile_files)?;

    // Also activate common files if any exist
    let common_files: Vec<String> = manifest.get_common_files().to_vec();
    if !common_files.is_empty() {
        let common_operations = symlink_mgr.activate_common_files(&common_files)?;
        operations.extend(common_operations);
    }

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
        eprintln!(
            "⚠️  Activated {} files, {} failed",
            success_count, failed_count
        );
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

        println!(
            "✅ Successfully activated profile '{}'",
            active_profile_name
        );
        println!("   {} symlinks created", success_count);
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
        eprintln!(
            "⚠️  Deactivated {} files, {} failed",
            success_count, failed_count
        );
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
        println!("   {} files restored", success_count);
        println!("💡 Dotstate is now deactivated. Use 'dotstate activate' to reactivate.");
    }

    Ok(())
}
