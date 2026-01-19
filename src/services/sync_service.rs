//! Sync service for file synchronization operations.
//!
//! This module provides a service layer for file sync operations,
//! abstracting the details of file copying, symlink creation, and
//! manifest management from the UI layer.

use crate::config::Config;
use crate::file_manager::{copy_dir_all, Dotfile, FileManager};
use crate::utils::{get_home_dir, sync_validation, ProfileManifest, SymlinkManager};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Result of a sync validation.
#[derive(Debug)]
pub struct SyncValidationResult {
    /// Whether the operation is safe to perform.
    pub is_safe: bool,
    /// Error message if not safe.
    pub error_message: Option<String>,
}

/// Result of adding a file to sync.
#[derive(Debug)]
pub enum AddFileResult {
    /// File was successfully synced.
    Success,
    /// File was already synced, no action taken.
    AlreadySynced,
    /// Validation failed with the given error message.
    ValidationFailed(String),
}

/// Result of removing a file from sync.
#[derive(Debug)]
pub enum RemoveFileResult {
    /// File was successfully removed from sync.
    Success,
    /// File was not synced, no action taken.
    NotSynced,
}

/// Service for file synchronization operations.
///
/// This service provides a clean interface for file sync operations without
/// direct dependencies on UI state.
pub struct SyncService;

impl SyncService {
    /// Add a file to sync.
    ///
    /// This is the unified function that handles both regular dotfiles and custom files.
    /// It performs the following operations:
    /// 1. Validates the operation is safe
    /// 2. Copies the file to the repository
    /// 3. Creates a symlink from home to repo
    /// 4. Updates the profile manifest
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `full_path` - Full path to the source file.
    /// * `relative_path` - Path relative to home directory.
    /// * `backup_enabled` - Whether to enable backups.
    ///
    /// # Returns
    ///
    /// Result indicating success, already synced, or validation failure.
    pub fn add_file_to_sync(
        config: &Config,
        full_path: &Path,
        relative_path: &str,
        backup_enabled: bool,
    ) -> Result<AddFileResult> {
        let profile_name = &config.active_profile;
        let repo_path = &config.repo_path;

        // Get previously synced files
        let previously_synced = Self::get_synced_files(repo_path, profile_name)?;

        // Check if already synced
        if previously_synced.contains(relative_path) {
            debug!("File already synced: {}", relative_path);
            return Ok(AddFileResult::AlreadySynced);
        }

        // VALIDATE BEFORE ANY OPERATIONS - prevent data loss
        let validation = sync_validation::validate_before_sync(
            relative_path,
            full_path,
            &previously_synced,
            repo_path,
        );
        if !validation.is_safe {
            let error_msg = validation
                .error_message
                .unwrap_or_else(|| "Cannot add this file or directory".to_string());
            warn!("Validation failed for {}: {}", relative_path, error_msg);
            return Ok(AddFileResult::ValidationFailed(error_msg));
        }

        // Create file manager for symlink resolution
        let file_manager = FileManager::new()?;

        // Validate symlink can be created before deleting original file
        let home_dir = get_home_dir();
        let target_path = home_dir.join(relative_path);
        let profile_path = repo_path.join(profile_name);
        let repo_file_path = profile_path.join(relative_path);

        // Handle symlinks: resolve to original file for validation
        let original_source = if file_manager.is_symlink(full_path) {
            file_manager.resolve_symlink(full_path)?
        } else {
            full_path.to_path_buf()
        };

        let symlink_validation = sync_validation::validate_symlink_creation(
            &original_source,
            &repo_file_path,
            &target_path,
        )
        .context("Failed to validate symlink creation")?;
        if !symlink_validation.is_safe {
            let error_msg = symlink_validation
                .error_message
                .unwrap_or_else(|| "Cannot create symlink".to_string());
            warn!(
                "Symlink validation failed for {}: {}",
                relative_path, error_msg
            );
            return Ok(AddFileResult::ValidationFailed(error_msg));
        }

        info!(
            "Adding file to sync: {} (profile: {})",
            relative_path, profile_name
        );
        debug!("Source path: {:?}", full_path);
        debug!("Repo destination: {:?}", repo_file_path);
        debug!("Symlink target: {:?}", target_path);

        // Create parent directories
        if let Some(parent) = repo_file_path.parent() {
            if !parent.exists() {
                debug!("Creating repo directory: {:?}", parent);
            }
            std::fs::create_dir_all(parent).context("Failed to create repo directory")?;
        }

        // Handle symlinks: resolve to original file
        let source_path = if file_manager.is_symlink(full_path) {
            debug!("Resolving symlink: {:?}", full_path);
            let resolved = file_manager.resolve_symlink(full_path)?;
            debug!("Resolved symlink to: {:?}", resolved);
            resolved
        } else {
            full_path.to_path_buf()
        };

        // Copy to repo FIRST (before deleting original)
        // This ensures we have a backup before any destructive operations
        info!("Copying file to repository...");
        file_manager
            .copy_to_repo(&source_path, &repo_file_path)
            .context("Failed to copy file to repo")?;
        info!("Successfully copied file to repository");

        // Create symlink using SymlinkManager
        info!("Creating symlink...");
        let mut symlink_mgr = SymlinkManager::new_with_backup(repo_path.clone(), backup_enabled)?;
        symlink_mgr
            .add_symlink_to_profile(profile_name, relative_path)
            .context("Failed to create symlink")?;
        info!("Successfully created symlink");

        // Update manifest
        info!("Updating profile manifest...");
        Self::add_file_to_manifest(repo_path, profile_name, relative_path)?;

        info!("Successfully added file to sync: {}", relative_path);
        Ok(AddFileResult::Success)
    }

    /// Remove a file from sync.
    ///
    /// This performs the following operations:
    /// 1. Removes the symlink from home
    /// 2. Restores the file from the repository
    /// 3. Removes the file from the repository
    /// 4. Updates the profile manifest
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `relative_path` - Path relative to home directory.
    ///
    /// # Returns
    ///
    /// Result indicating success or that file was not synced.
    pub fn remove_file_from_sync(config: &Config, relative_path: &str) -> Result<RemoveFileResult> {
        let profile_name = &config.active_profile;
        let repo_path = &config.repo_path;
        let home_dir = get_home_dir();

        // Get previously synced files
        let previously_synced = Self::get_synced_files(repo_path, profile_name)?;

        if !previously_synced.contains(relative_path) {
            debug!("File not synced, skipping removal: {}", relative_path);
            return Ok(RemoveFileResult::NotSynced);
        }

        info!(
            "Removing file from sync: {} (profile: {})",
            relative_path, profile_name
        );

        let target_path = home_dir.join(relative_path);
        let repo_file_path = repo_path.join(profile_name).join(relative_path);

        // Restore file from repo if symlink exists
        if target_path.symlink_metadata().is_ok() {
            let metadata = target_path.symlink_metadata().unwrap();
            if metadata.is_symlink() {
                // Remove symlink
                std::fs::remove_file(&target_path).context("Failed to remove symlink")?;

                // Copy file from repo back to home
                if repo_file_path.exists() {
                    if repo_file_path.is_dir() {
                        copy_dir_all(&repo_file_path, &target_path)
                            .context("Failed to restore directory from repo")?;
                    } else {
                        std::fs::copy(&repo_file_path, &target_path)
                            .context("Failed to restore file from repo")?;
                    }
                }
            }
        }

        // Update symlink tracking - remove only the specific file
        let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;

        // Remove the specific symlink from tracking
        // Note: We already removed the actual symlink and restored the file above (lines 227-244)
        // This just updates the tracking data without touching other symlinks
        symlink_mgr.remove_symlink_from_tracking(profile_name, relative_path)?;

        // Remove from repo
        if repo_file_path.exists() {
            if repo_file_path.is_dir() {
                std::fs::remove_dir_all(&repo_file_path)
                    .context("Failed to remove directory from repo")?;
            } else {
                std::fs::remove_file(&repo_file_path).context("Failed to remove file from repo")?;
            }
        }

        // Update manifest - remove the file from the synced files list
        let remaining_files: Vec<String> = previously_synced
            .iter()
            .filter(|f| *f != relative_path)
            .cloned()
            .collect();

        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;
        manifest.update_synced_files(profile_name, remaining_files)?;
        manifest.save(repo_path)?;

        info!("Successfully removed file from sync: {}", relative_path);
        Ok(RemoveFileResult::Success)
    }

    /// Get the set of synced files for a profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile.
    ///
    /// # Returns
    ///
    /// Set of synced file paths.
    pub fn get_synced_files(repo_path: &Path, profile_name: &str) -> Result<HashSet<String>> {
        let manifest = ProfileManifest::load_or_backfill(repo_path)?;
        Ok(manifest
            .profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .map(|p| p.synced_files.into_iter().collect())
            .unwrap_or_default())
    }

    /// Add a file path to the manifest for a profile.
    fn add_file_to_manifest(
        repo_path: &Path,
        profile_name: &str,
        relative_path: &str,
    ) -> Result<()> {
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;
        let current_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .map(|p| p.synced_files.clone())
            .unwrap_or_default();

        if !current_files.contains(&relative_path.to_string()) {
            debug!(
                "Adding {} to manifest for profile {}",
                relative_path, profile_name
            );
            let mut new_files = current_files;
            new_files.push(relative_path.to_string());
            manifest.update_synced_files(profile_name, new_files)?;
            manifest.save(repo_path)?;
            info!("Updated manifest with new file: {}", relative_path);
        } else {
            debug!("File already in manifest, skipping update");
        }
        Ok(())
    }

    /// Scan for dotfiles and return the list.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    ///
    /// # Returns
    ///
    /// List of dotfiles found, with sync status marked.
    pub fn scan_dotfiles(config: &Config) -> Result<Vec<Dotfile>> {
        use crate::dotfile_candidates::get_default_dotfile_paths;

        let file_manager = FileManager::new()?;
        let dotfile_names = get_default_dotfile_paths();
        let mut found = file_manager.scan_dotfiles(&dotfile_names);

        // Load the manifest to get synced files and common files
        let manifest = ProfileManifest::load_or_backfill(&config.repo_path)?;

        // Mark files that are already synced
        let synced_set: HashSet<String> =
            Self::get_synced_files(&config.repo_path, &config.active_profile)
                .unwrap_or_default()
                .iter()
                .map(|p| {
                    let p = p.replace('\\', "/");
                    p.strip_prefix("./").unwrap_or(&p).to_string()
                })
                .collect();

        // Also check if any found files are common files
        let common_files_set: HashSet<String> = manifest
            .get_common_files()
            .iter()
            .map(|p| {
                let p = p.replace('\\', "/");
                p.strip_prefix("./").unwrap_or(&p).to_string()
            })
            .collect();

        for dotfile in &mut found {
            let rel_raw = dotfile.relative_path.to_string_lossy().replace('\\', "/");
            let rel = rel_raw.strip_prefix("./").unwrap_or(&rel_raw).to_string();

            if synced_set.contains(&rel) {
                dotfile.synced = true;
            }
            if common_files_set.contains(&rel) {
                dotfile.is_common = true;
                dotfile.synced = true; // Common files are always synced
            }
        }

        // Also add custom files from config
        let home_dir = get_home_dir();
        for custom_path in &config.custom_files {
            let full_path = home_dir.join(custom_path);
            let relative_path = PathBuf::from(custom_path);

            // Skip if not a valid path or if it doesn't exist
            if !full_path.exists() && !full_path.is_symlink() {
                continue;
            }

            // Check if already in the list
            if found
                .iter()
                .any(|d| d.relative_path.to_string_lossy() == *custom_path)
            {
                continue;
            }

            let is_synced = synced_set.contains(custom_path);

            found.push(Dotfile {
                original_path: full_path,
                relative_path,
                synced: is_synced,
                description: None,
                is_common: false,
            });
        }

        // IMPORTANT: Also add synced files from manifest that aren't in the list yet
        // This ensures that custom files synced on another machine still show up
        // even if they're not in the local config.custom_files
        for synced_path in &synced_set {
            // Skip if already in the list
            if found
                .iter()
                .any(|d| d.relative_path.to_string_lossy() == *synced_path)
            {
                continue;
            }

            let full_path = home_dir.join(synced_path);
            let relative_path = PathBuf::from(synced_path);

            // Add even if file doesn't exist locally (might have been deleted)
            // This allows user to see and manage it in the UI
            found.push(Dotfile {
                original_path: full_path,
                relative_path,
                synced: true, // It's in the manifest, so it's synced
                description: None,
                is_common: false,
            });
        }

        // Add common files from manifest
        let common_files = manifest.get_common_files();
        for common_path in common_files {
            // Skip if already in the list (shouldn't happen, but be safe)
            if found.iter().any(|d| {
                let d_path_raw = d.relative_path.to_string_lossy().replace('\\', "/");
                let d_path = d_path_raw.strip_prefix("./").unwrap_or(&d_path_raw);

                let c_path_raw = common_path.replace('\\', "/");
                let c_path = c_path_raw.strip_prefix("./").unwrap_or(&c_path_raw);

                d_path == c_path
            }) {
                continue;
            }

            let full_path = home_dir.join(common_path);
            let relative_path = PathBuf::from(common_path);

            found.push(Dotfile {
                original_path: full_path,
                relative_path,
                synced: true, // Common files are always synced
                description: None,
                is_common: true,
            });
        }

        // Sort by relative path for consistent display
        found.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        Ok(found)
    }

    /// Check if a path is a custom file (not in default dotfile candidates).
    ///
    /// # Arguments
    ///
    /// * `relative_path` - Path relative to home directory.
    ///
    /// # Returns
    ///
    /// True if this is a custom file.
    pub fn is_custom_file(relative_path: &str) -> bool {
        use crate::dotfile_candidates::get_default_dotfile_paths;
        let default_paths = get_default_dotfile_paths();
        !default_paths.iter().any(|p| p == relative_path)
    }

    // ============================================================================
    // Common File Methods - For files shared across all profiles
    // ============================================================================

    /// Add a file to common (shared across all profiles).
    ///
    /// This performs the following operations:
    /// 1. Validates the operation is safe
    /// 2. Copies the file to the common folder in the repository
    /// 3. Creates a symlink from home to the common folder
    /// 4. Updates the manifest
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `full_path` - Full path to the source file.
    /// * `relative_path` - Path relative to home directory.
    /// * `backup_enabled` - Whether to enable backups.
    ///
    /// # Returns
    ///
    /// Result indicating success, already synced, or validation failure.
    pub fn add_common_file_to_sync(
        config: &Config,
        full_path: &Path,
        relative_path: &str,
        backup_enabled: bool,
    ) -> Result<AddFileResult> {
        let repo_path = &config.repo_path;

        // Load manifest to check if already in common
        let manifest = ProfileManifest::load_or_backfill(repo_path)?;
        if manifest.is_common_file(relative_path) {
            debug!("File already in common: {}", relative_path);
            return Ok(AddFileResult::AlreadySynced);
        }

        // VALIDATE BEFORE ANY OPERATIONS
        let previously_synced = Self::get_synced_files(repo_path, &config.active_profile)?;
        let validation = sync_validation::validate_before_sync(
            relative_path,
            full_path,
            &previously_synced,
            repo_path,
        );
        if !validation.is_safe {
            let error_msg = validation
                .error_message
                .unwrap_or_else(|| "Cannot add this file to common".to_string());
            warn!(
                "Validation failed for common file {}: {}",
                relative_path, error_msg
            );
            return Ok(AddFileResult::ValidationFailed(error_msg));
        }

        // Create file manager for symlink resolution
        let file_manager = FileManager::new()?;

        // Validate symlink can be created
        let home_dir = get_home_dir();
        let target_path = home_dir.join(relative_path);
        let common_path = repo_path.join("common");
        let repo_file_path = common_path.join(relative_path);

        // Handle symlinks: resolve to original file for validation
        let original_source = if file_manager.is_symlink(full_path) {
            file_manager.resolve_symlink(full_path)?
        } else {
            full_path.to_path_buf()
        };

        let symlink_validation = sync_validation::validate_symlink_creation(
            &original_source,
            &repo_file_path,
            &target_path,
        )
        .context("Failed to validate symlink creation")?;
        if !symlink_validation.is_safe {
            let error_msg = symlink_validation
                .error_message
                .unwrap_or_else(|| "Cannot create symlink".to_string());
            warn!(
                "Symlink validation failed for common file {}: {}",
                relative_path, error_msg
            );
            return Ok(AddFileResult::ValidationFailed(error_msg));
        }

        info!("Adding common file to sync: {}", relative_path);

        // Ensure common directory exists
        std::fs::create_dir_all(&common_path).context("Failed to create common directory")?;

        // Create parent directories
        if let Some(parent) = repo_file_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).context("Failed to create directory")?;
            }
        }

        // Handle symlinks: resolve to original file
        let source_path = if file_manager.is_symlink(full_path) {
            file_manager.resolve_symlink(full_path)?
        } else {
            full_path.to_path_buf()
        };

        // Copy to common folder in repo
        info!("Copying file to common folder...");
        file_manager
            .copy_to_repo(&source_path, &repo_file_path)
            .context("Failed to copy file to common folder")?;

        // Create symlink using SymlinkManager
        info!("Creating symlink...");
        let mut symlink_mgr = SymlinkManager::new_with_backup(repo_path.clone(), backup_enabled)?;
        symlink_mgr
            .add_common_symlink(relative_path)
            .context("Failed to create symlink")?;

        // Update manifest
        info!("Updating manifest...");
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;
        manifest.add_common_file(relative_path);
        manifest.save(repo_path)?;

        info!("Successfully added common file: {}", relative_path);
        Ok(AddFileResult::Success)
    }

    /// Remove a file from common.
    ///
    /// This performs the following operations:
    /// 1. Removes the symlink from home
    /// 2. Restores the file from the common folder
    /// 3. Removes the file from the common folder
    /// 4. Updates the manifest
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `relative_path` - Path relative to home directory.
    ///
    /// # Returns
    ///
    /// Result indicating success or that file was not in common.
    pub fn remove_common_file_from_sync(
        config: &Config,
        relative_path: &str,
    ) -> Result<RemoveFileResult> {
        let repo_path = &config.repo_path;
        let home_dir = get_home_dir();

        // Load manifest to check if in common
        let manifest = ProfileManifest::load_or_backfill(repo_path)?;
        if !manifest.is_common_file(relative_path) {
            debug!("File not in common, skipping removal: {}", relative_path);
            return Ok(RemoveFileResult::NotSynced);
        }

        info!("Removing common file from sync: {}", relative_path);

        let target_path = home_dir.join(relative_path);
        let common_path = repo_path.join("common");
        let repo_file_path = common_path.join(relative_path);

        // Restore file from common folder if symlink exists
        if target_path.symlink_metadata().is_ok() {
            let metadata = target_path.symlink_metadata().unwrap();
            if metadata.is_symlink() {
                // Remove symlink
                std::fs::remove_file(&target_path).context("Failed to remove symlink")?;

                // Copy file from common folder back to home
                if repo_file_path.exists() {
                    if repo_file_path.is_dir() {
                        copy_dir_all(&repo_file_path, &target_path)
                            .context("Failed to restore directory from common")?;
                    } else {
                        std::fs::copy(&repo_file_path, &target_path)
                            .context("Failed to restore file from common")?;
                    }
                }
            }
        }

        // Update symlink tracking
        let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;
        symlink_mgr.remove_common_symlink_from_tracking(relative_path)?;

        // Remove from common folder
        if repo_file_path.exists() {
            if repo_file_path.is_dir() {
                std::fs::remove_dir_all(&repo_file_path)
                    .context("Failed to remove directory from common")?;
            } else {
                std::fs::remove_file(&repo_file_path)
                    .context("Failed to remove file from common")?;
            }
        }

        // Update manifest
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;
        manifest.remove_common_file(relative_path);
        manifest.save(repo_path)?;

        info!("Successfully removed common file: {}", relative_path);
        Ok(RemoveFileResult::Success)
    }

    /// Move a file from a profile to common, cleaning up specified profiles.
    ///
    /// This is the validated version that handles cleanup of the same file
    /// in other profiles before moving to common.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `relative_path` - Path relative to home directory.
    /// * `profiles_to_cleanup` - Profiles that have the same file and should be cleaned up.
    ///
    /// # Returns
    ///
    /// Result indicating success or failure.
    pub fn move_to_common_with_cleanup(
        config: &Config,
        relative_path: &str,
        profiles_to_cleanup: &[String],
    ) -> Result<()> {
        let repo_path = &config.repo_path;
        let profile_name = &config.active_profile;

        info!(
            "Moving {} from profile '{}' to common (cleaning up {} profiles)",
            relative_path,
            profile_name,
            profiles_to_cleanup.len()
        );

        // Clean up the file from other profiles first
        for profile in profiles_to_cleanup {
            if profile == profile_name {
                continue; // Skip the source profile
            }

            info!("Cleaning up {} from profile '{}'", relative_path, profile);

            // Remove from manifest
            let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;
            if let Some(p) = manifest.profiles.iter_mut().find(|p| p.name == *profile) {
                p.synced_files.retain(|f| f != relative_path);
            }
            manifest.save(repo_path)?;

            // Remove file from profile directory if it exists
            let profile_file_path = repo_path.join(profile).join(relative_path);
            if profile_file_path.exists() {
                if profile_file_path.is_dir() {
                    std::fs::remove_dir_all(&profile_file_path)
                        .context("Failed to remove directory from profile")?;
                } else {
                    std::fs::remove_file(&profile_file_path)
                        .context("Failed to remove file from profile")?;
                }
            }

            // Remove symlink tracking if it exists
            let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;
            symlink_mgr.remove_symlink_from_tracking(profile, relative_path)?;
        }

        // Now perform the normal move
        Self::move_to_common(config, relative_path)
    }

    /// Move a file from a profile to common.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `relative_path` - Path relative to home directory.
    ///
    /// # Returns
    ///
    /// Result indicating success or failure.
    pub fn move_to_common(config: &Config, relative_path: &str) -> Result<()> {
        let repo_path = &config.repo_path;
        let profile_name = &config.active_profile;

        info!(
            "Moving {} from profile '{}' to common",
            relative_path, profile_name
        );

        // Load manifest
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;

        // Check if file is in the profile
        let profile = manifest
            .profiles
            .iter()
            .find(|p| p.name == *profile_name)
            .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", profile_name))?;

        if !profile.synced_files.contains(&relative_path.to_string()) {
            return Err(anyhow::anyhow!(
                "File '{}' is not synced in profile '{}'",
                relative_path,
                profile_name
            ));
        }

        // Move the actual file from profile folder to common folder
        let profile_path = repo_path.join(profile_name);
        let common_path = repo_path.join("common");
        let source = profile_path.join(relative_path);
        let dest = common_path.join(relative_path);

        // Ensure common directory exists
        std::fs::create_dir_all(&common_path).context("Failed to create common directory")?;

        // Create parent directories for destination
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Move the file
        if source.exists() {
            if source.is_dir() {
                copy_dir_all(&source, &dest)?;
                std::fs::remove_dir_all(&source)?;
            } else {
                std::fs::copy(&source, &dest)?;
                std::fs::remove_file(&source)?;
            }
        }

        // Update symlink to point to common folder
        let home_dir = get_home_dir();
        let target = home_dir.join(relative_path);

        // Remove old symlink if exists
        if target.symlink_metadata().is_ok() {
            std::fs::remove_file(&target)?;
        }

        // Create new symlink to common
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dest, &target)?;
        #[cfg(windows)]
        {
            if dest.is_dir() {
                std::os::windows::fs::symlink_dir(&dest, &target)?;
            } else {
                std::os::windows::fs::symlink_file(&dest, &target)?;
            }
        }

        // Update manifest
        manifest.move_to_common(profile_name, relative_path)?;
        manifest.save(repo_path)?;

        // Update symlink tracking: remove old profile tracking, add new common tracking
        let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;
        symlink_mgr.remove_symlink_from_tracking(profile_name, relative_path)?;
        // Add the new common symlink to tracking
        symlink_mgr.add_common_symlink(relative_path)?;

        info!("Successfully moved {} to common", relative_path);

        Ok(())
    }

    /// Move a file from common to the current profile.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    /// * `relative_path` - Path relative to home directory.
    ///
    /// # Returns
    ///
    /// Result indicating success or failure.
    pub fn move_from_common(config: &Config, relative_path: &str) -> Result<()> {
        let repo_path = &config.repo_path;
        let profile_name = &config.active_profile;

        info!(
            "Moving {} from common to profile '{}'",
            relative_path, profile_name
        );

        // Load manifest
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;

        // Check if file is in common
        if !manifest.is_common_file(relative_path) {
            return Err(anyhow::anyhow!("File '{}' is not in common", relative_path));
        }

        // Move the actual file from common folder to profile folder
        let common_path = repo_path.join("common");
        let profile_path = repo_path.join(profile_name);
        let source = common_path.join(relative_path);
        let dest = profile_path.join(relative_path);

        // Create parent directories for destination
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Move the file
        if source.exists() {
            if source.is_dir() {
                copy_dir_all(&source, &dest)?;
                std::fs::remove_dir_all(&source)?;
            } else {
                std::fs::copy(&source, &dest)?;
                std::fs::remove_file(&source)?;
            }
        }

        // Update symlink to point to profile folder
        let home_dir = get_home_dir();
        let target = home_dir.join(relative_path);

        // Remove old symlink if exists
        if target.symlink_metadata().is_ok() {
            std::fs::remove_file(&target)?;
        }

        // Create new symlink to profile
        #[cfg(unix)]
        std::os::unix::fs::symlink(&dest, &target)?;
        #[cfg(windows)]
        {
            if dest.is_dir() {
                std::os::windows::fs::symlink_dir(&dest, &target)?;
            } else {
                std::os::windows::fs::symlink_file(&dest, &target)?;
            }
        }

        // Update manifest
        manifest.move_from_common(profile_name, relative_path)?;
        manifest.save(repo_path)?;

        // Update symlink tracking: remove old common tracking, add new profile tracking
        let mut symlink_mgr = SymlinkManager::new(repo_path.clone())?;
        symlink_mgr.remove_common_symlink_from_tracking(relative_path)?;
        // Add the new profile symlink to tracking
        symlink_mgr.add_symlink_to_profile(profile_name, relative_path)?;

        info!(
            "Successfully moved {} to profile '{}'",
            relative_path, profile_name
        );

        Ok(())
    }

    /// Get the set of common files.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    ///
    /// # Returns
    ///
    /// Set of common file paths.
    pub fn get_common_files(repo_path: &Path) -> Result<HashSet<String>> {
        let manifest = ProfileManifest::load_or_backfill(repo_path)?;
        Ok(manifest.get_common_files().iter().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_custom_file() {
        // Common dotfiles should not be custom
        assert!(!SyncService::is_custom_file(".bashrc"));
        assert!(!SyncService::is_custom_file(".zshrc"));

        // Random files should be custom
        assert!(SyncService::is_custom_file("my_custom_config"));
        assert!(SyncService::is_custom_file(".my_app/config.toml"));
    }
    #[test]
    fn test_scan_dotfiles_path_normalization() {
        // Mock logic used in scan_dotfiles

        let local_path = "subdir/file";
        let local_path_win = "subdir\\file";
        let local_path_prefix = "./subdir/file";

        let manifest_path = "subdir/file";
        let manifest_path_win = "subdir\\file";
        let manifest_path_prefix = "./subdir/file";

        // Helper to simulate the cleanup logic
        let normalize = |p: &str| -> String {
            let p = p.replace('\\', "/");
            p.strip_prefix("./").unwrap_or(&p).to_string()
        };

        // Verify cross-platform matching
        assert_eq!(normalize(local_path), normalize(manifest_path));
        assert_eq!(normalize(local_path), normalize(manifest_path_win));
        assert_eq!(normalize(local_path), normalize(manifest_path_prefix));

        assert_eq!(normalize(local_path_win), normalize(manifest_path));
        assert_eq!(normalize(local_path_prefix), normalize(manifest_path));

        // Explicit cases
        assert_eq!(normalize("foo\\bar"), "foo/bar");
        assert_eq!(normalize("./foo/bar"), "foo/bar");
        assert_eq!(normalize(".\\foo\\bar"), "foo/bar");
    }
}
