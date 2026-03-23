//! Profile service for profile management operations.
//!
//! This module provides a service layer for profile-related operations,
//! abstracting the details of the profile management from the UI layer.

use crate::utils::profile_manifest::{Package, ProfileInfo, ResolvedFile};
use crate::utils::symlink_manager::{OperationStatus, SymlinkManager};
use crate::utils::{sanitize_profile_name, validate_profile_name, ProfileManifest};
use anyhow::{Context, Result};
use std::path::Path;
use tracing::{error, info, warn};

/// Result of a profile switch operation.
#[derive(Debug)]
pub struct ProfileSwitchResult {
    /// Number of symlinks removed from old profile.
    pub removed_count: usize,
    /// Number of symlinks created for new profile.
    pub created_count: usize,
    /// Packages that need to be checked for the new profile.
    pub packages: Vec<Package>,
}

/// Result of a profile activation operation.
#[derive(Debug)]
pub struct ProfileActivationResult {
    /// Number of symlinks successfully created.
    pub success_count: usize,
    /// Packages that need to be checked for the profile.
    pub packages: Vec<Package>,
}

/// Service for profile-related operations.
///
/// This service provides a clean interface for profile operations without
/// direct dependencies on UI state.
pub struct ProfileService;

impl ProfileService {
    /// Load the profile manifest from the repository.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    ///
    /// # Returns
    ///
    /// The profile manifest.
    pub fn load_manifest(repo_path: &Path) -> Result<ProfileManifest> {
        ProfileManifest::load_or_backfill(repo_path)
    }

    /// Save the profile manifest to the repository.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `manifest` - The manifest to save.
    pub fn save_manifest(repo_path: &Path, manifest: &ProfileManifest) -> Result<()> {
        manifest.save(repo_path)
    }

    /// Get all profiles from the manifest.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    ///
    /// # Returns
    ///
    /// List of profile info.
    pub fn get_profiles(repo_path: &Path) -> Result<Vec<ProfileInfo>> {
        Ok(Self::load_manifest(repo_path)?.profiles)
    }

    /// Get information about a specific profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile to get.
    ///
    /// # Returns
    ///
    /// Profile info if found.
    pub fn get_profile_info(repo_path: &Path, profile_name: &str) -> Result<Option<ProfileInfo>> {
        let manifest = Self::load_manifest(repo_path)?;
        Ok(manifest
            .profiles
            .into_iter()
            .find(|p| p.name == profile_name))
    }

    /// Create a new profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `name` - Name for the new profile.
    /// * `description` - Optional description.
    /// * `copy_from` - Optional index of profile to copy files from.
    ///
    /// # Returns
    ///
    /// The sanitized name of the created profile.
    pub fn create_profile(
        repo_path: &Path,
        name: &str,
        description: Option<String>,
        copy_from: Option<usize>,
        inherits: Option<String>,
    ) -> Result<String> {
        // Validate and sanitize profile name
        let sanitized_name = sanitize_profile_name(name);
        if sanitized_name.is_empty() {
            return Err(anyhow::anyhow!("Profile name cannot be empty"));
        }

        // Get existing profile names from manifest
        let mut manifest = Self::load_manifest(repo_path)?;
        let existing_names: Vec<String> =
            manifest.profiles.iter().map(|p| p.name.clone()).collect();
        if let Err(e) = validate_profile_name(&sanitized_name, &existing_names) {
            return Err(anyhow::anyhow!("Invalid profile name: {e}"));
        }

        // Check if profile folder exists but is not in manifest
        let profile_path = repo_path.join(&sanitized_name);
        let folder_exists = profile_path.exists();
        let profile_in_manifest = existing_names.contains(&sanitized_name);

        if folder_exists && !profile_in_manifest {
            warn!(
                "Profile folder '{}' already exists but is not in manifest. Will use existing folder.",
                sanitized_name
            );
        } else if folder_exists && profile_in_manifest {
            return Err(anyhow::anyhow!(
                "Profile '{sanitized_name}' already exists in manifest"
            ));
        }

        // Create folder if it doesn't exist
        if !folder_exists {
            std::fs::create_dir_all(&profile_path).context("Failed to create profile directory")?;
        }

        // Copy files from source profile if specified
        let synced_files = if let Some(source_idx) = copy_from {
            if let Some(source_profile) = manifest.profiles.get(source_idx) {
                let source_profile_path = repo_path.join(&source_profile.name);

                // Copy all files from source profile
                for file in &source_profile.synced_files {
                    let source_file = source_profile_path.join(file);
                    let dest_file = profile_path.join(file);

                    if source_file.exists() {
                        // Create parent directories
                        if let Some(parent) = dest_file.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        // Copy file or directory
                        if source_file.is_dir() {
                            crate::file_manager::copy_dir_all(&source_file, &dest_file)?;
                        } else {
                            std::fs::copy(&source_file, &dest_file)?;
                        }
                    }
                }

                source_profile.synced_files.clone()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        // Add profile to manifest with synced_files and optional inheritance
        manifest.add_profile_with_inherits(sanitized_name.clone(), description, inherits);
        // Update synced_files for the newly added profile
        manifest.update_synced_files(&sanitized_name, synced_files)?;
        Self::save_manifest(repo_path, &manifest)?;

        info!("Created profile: {}", sanitized_name);
        Ok(sanitized_name)
    }

    /// Get list of common files from manifest
    pub fn get_common_files(repo_path: &Path) -> Result<Vec<String>> {
        let manifest = Self::load_manifest(repo_path)?;
        Ok(manifest.common.synced_files)
    }

    /// Switch to a different profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `old_profile_name` - Name of the current active profile.
    /// * `target_profile_name` - Name of the profile to switch to.
    /// * `backup_enabled` - Whether to enable backups during switch.
    ///
    /// # Returns
    ///
    /// Result of the switch operation.
    pub fn switch_profile(
        repo_path: &Path,
        old_profile_name: &str,
        target_profile_name: &str,
        backup_enabled: bool,
    ) -> Result<ProfileSwitchResult> {
        let manifest = Self::load_manifest(repo_path)?;

        // Verify target exists
        if !manifest
            .profiles
            .iter()
            .any(|p| p.name == target_profile_name)
        {
            return Err(anyhow::anyhow!("Profile '{target_profile_name}' not found"));
        }

        // Don't switch if already active
        if old_profile_name == target_profile_name {
            let packages = manifest.resolve_packages(target_profile_name)?;
            return Ok(ProfileSwitchResult {
                removed_count: 0,
                created_count: 0,
                packages,
            });
        }

        // Resolve the full file list for the target (inheritance + common)
        let resolved_files = manifest.resolve_files(target_profile_name)?;
        let resolved_packages = manifest.resolve_packages(target_profile_name)?;

        // Use SymlinkManager: deactivate old, activate new with resolved files
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(repo_path.to_path_buf(), backup_enabled)?;

        // Step 1: Deactivate old profile (removes ALL tracked symlinks)
        let removed = match symlink_mgr.deactivate_profile_with_restore(old_profile_name, false) {
            Ok(ops) => ops,
            Err(e) => {
                error!("Failed to deactivate profile '{}': {}", old_profile_name, e);
                return Err(anyhow::anyhow!("Failed to deactivate old profile: {e}"));
            }
        };

        // Step 2: Activate new profile with resolved files (includes inherited + common)
        let created = match symlink_mgr.activate_resolved(target_profile_name, &resolved_files) {
            Ok(ops) => ops,
            Err(e) => {
                error!(
                    "Failed to activate profile '{}': {}",
                    target_profile_name, e
                );
                return Err(anyhow::anyhow!("Failed to activate new profile: {e}"));
            }
        };

        info!(
            "Switched from '{}' to '{}'",
            old_profile_name, target_profile_name
        );
        info!(
            "Removed {} symlinks, created {} symlinks",
            removed.len(),
            created.len()
        );

        Ok(ProfileSwitchResult {
            removed_count: removed.len(),
            created_count: created.len(),
            packages: resolved_packages,
        })
    }

    /// Rename a profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `old_name` - Current name of the profile.
    /// * `new_name` - New name for the profile.
    /// * `is_active` - Whether this is the active profile.
    /// * `backup_enabled` - Whether to enable backups during rename.
    ///
    /// # Returns
    ///
    /// The sanitized new name.
    pub fn rename_profile(
        repo_path: &Path,
        old_name: &str,
        new_name: &str,
        is_active: bool,
        backup_enabled: bool,
    ) -> Result<String> {
        // Validate new name
        let sanitized_name = sanitize_profile_name(new_name);
        if sanitized_name.is_empty() {
            return Err(anyhow::anyhow!("Profile name cannot be empty"));
        }

        // Get existing profile names from manifest
        let mut manifest = Self::load_manifest(repo_path)?;
        let existing_names: Vec<String> = manifest
            .profiles
            .iter()
            .filter(|p| p.name != old_name)
            .map(|p| p.name.clone())
            .collect();
        if let Err(e) = validate_profile_name(&sanitized_name, &existing_names) {
            return Err(anyhow::anyhow!("Invalid profile name: {e}"));
        }

        // Check if profile exists in manifest
        if !manifest.has_profile(old_name) {
            return Err(anyhow::anyhow!("Profile '{old_name}' not found"));
        }

        // Rename profile folder in repo
        let old_path = repo_path.join(old_name);
        let new_path = repo_path.join(&sanitized_name);

        if old_path.exists() {
            std::fs::rename(&old_path, &new_path).context("Failed to rename profile directory")?;
        }

        // Update profile manifest (name + any inherits references)
        manifest.rename_profile(old_name, &sanitized_name)?;
        Self::save_manifest(repo_path, &manifest)?;

        // Update symlinks if profile is active
        if is_active {
            let mut symlink_mgr =
                SymlinkManager::new_with_backup(repo_path.to_path_buf(), backup_enabled)?;

            match symlink_mgr.rename_profile(old_name, &sanitized_name) {
                Ok(ops) => {
                    let success_count = ops
                        .iter()
                        .filter(|op| op.status == OperationStatus::Success)
                        .count();
                    info!("Updated {} symlinks for renamed profile", success_count);
                }
                Err(e) => {
                    error!("Failed to update symlinks after rename: {}", e);
                    // Don't fail the rename, but log the error
                }
            }
        }

        info!(
            "Renamed profile from '{}' to '{}'",
            old_name, sanitized_name
        );
        Ok(sanitized_name)
    }

    /// Delete a profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile to delete.
    /// * `active_profile_name` - Name of the currently active profile.
    ///
    /// # Errors
    ///
    /// Returns an error if the profile is active or doesn't exist.
    pub fn delete_profile(
        repo_path: &Path,
        profile_name: &str,
        active_profile_name: &str,
    ) -> Result<()> {
        // Cannot delete active profile
        if active_profile_name == profile_name {
            return Err(anyhow::anyhow!(
                "Cannot delete active profile '{profile_name}'. Please switch to another profile first."
            ));
        }

        // Check if other profiles inherit from this one
        let manifest = Self::load_manifest(repo_path)?;
        let inheriting = manifest.get_inheriting_profiles(profile_name);
        if !inheriting.is_empty() {
            let names = inheriting.join(", ");
            return Err(anyhow::anyhow!(
                "Cannot delete profile '{profile_name}' because it is inherited by: {names}. \
                 Remove the inheritance first."
            ));
        }

        // Remove profile folder from repo
        let profile_path = repo_path.join(profile_name);
        if profile_path.exists() {
            std::fs::remove_dir_all(&profile_path).context("Failed to remove profile directory")?;
        }

        // Remove from manifest
        let mut manifest = Self::load_manifest(repo_path)?;
        if !manifest.remove_profile(profile_name) {
            return Err(anyhow::anyhow!("Profile '{profile_name}' not found"));
        }
        Self::save_manifest(repo_path, &manifest)?;

        info!("Deleted profile: {}", profile_name);
        Ok(())
    }

    /// Activate a profile after setup (creates symlinks).
    ///
    /// Resolves the full inheritance chain and common files, then creates
    /// symlinks for all resolved files. Files from child profiles override
    /// parent profiles, and profile files override common files.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile to activate.
    /// * `backup_enabled` - Whether to enable backups during activation.
    ///
    /// # Returns
    ///
    /// Result of the activation.
    pub fn activate_profile(
        repo_path: &Path,
        profile_name: &str,
        backup_enabled: bool,
    ) -> Result<ProfileActivationResult> {
        info!("Activating profile '{}' after setup", profile_name);

        let manifest = Self::load_manifest(repo_path)?;

        // Resolve the full file list (inheritance chain + common, with overrides)
        let resolved_files = manifest.resolve_files(profile_name)?;
        let resolved_packages = manifest.resolve_packages(profile_name)?;

        if resolved_files.is_empty() {
            info!(
                "Profile '{}' has no files to sync (including inherited/common)",
                profile_name
            );
            return Ok(ProfileActivationResult {
                success_count: 0,
                packages: resolved_packages,
            });
        }

        // Create SymlinkManager with backup enabled
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(repo_path.to_path_buf(), backup_enabled)?;

        // Activate using resolved files (handles multi-source directories)
        let activation_result = match symlink_mgr.activate_resolved(profile_name, &resolved_files) {
            Ok(operations) => {
                let success_count = operations
                    .iter()
                    .filter(|op| matches!(op.status, OperationStatus::Success))
                    .count();
                info!(
                    "Activated profile '{}' with {} files (including inherited/common)",
                    profile_name, success_count
                );

                Ok(ProfileActivationResult {
                    success_count,
                    packages: resolved_packages,
                })
            }
            Err(e) => {
                error!("Failed to activate profile '{}': {}", profile_name, e);
                Err(anyhow::anyhow!("Failed to activate profile: {e}"))
            }
        }?;

        Ok(activation_result)
    }

    /// Ensure all files in the active profile have their symlinks created.
    ///
    /// This is an efficient reconciliation method that only creates missing symlinks.
    /// Perfect for after pulling changes from remote where new files were added but
    /// their symlinks don't exist locally yet.
    ///
    /// Resolves the full inheritance chain and common files, respecting overrides,
    /// then ensures symlinks exist for all resolved files.
    ///
    /// Unlike `activate_profile`, this does NOT remove any existing symlinks - it only
    /// adds missing ones.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile to ensure symlinks for.
    /// * `backup_enabled` - Whether to enable backups during symlink creation.
    ///
    /// # Returns
    ///
    /// A tuple of (`created_count`, `skipped_count`, errors)
    pub fn ensure_profile_symlinks(
        repo_path: &Path,
        profile_name: &str,
        backup_enabled: bool,
    ) -> Result<(usize, usize, Vec<String>)> {
        info!("Ensuring symlinks for profile '{}'", profile_name);

        let manifest = Self::load_manifest(repo_path)?;

        // Resolve the full file list (inheritance + common with overrides)
        let resolved_files = manifest.resolve_files(profile_name)?;

        if resolved_files.is_empty() {
            info!(
                "Profile '{}' has no files to sync (including inherited/common)",
                profile_name
            );
            return Ok((0, 0, Vec::new()));
        }

        // Use SymlinkManager to ensure resolved symlinks
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(repo_path.to_path_buf(), backup_enabled)?;

        symlink_mgr.ensure_resolved_symlinks(profile_name, &resolved_files)
    }

    /// Ensure all common files have their symlinks created.
    ///
    /// This is an efficient "reconciliation" method that only creates
    /// symlinks for files that don't already have them. Useful after
    /// pulling from remote when new common files may have been added.
    ///
    /// **Note:** When a profile with inheritance is active, common files that
    /// are overridden by the profile chain are skipped (the profile's version
    /// is authoritative). Use `ensure_profile_symlinks` to handle the full
    /// resolved set.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `backup_enabled` - Whether to enable backups during symlink creation.
    ///
    /// # Returns
    ///
    /// A tuple of (`created_count`, `skipped_count`, errors)
    pub fn ensure_common_symlinks(
        repo_path: &Path,
        backup_enabled: bool,
    ) -> Result<(usize, usize, Vec<String>)> {
        info!("Ensuring symlinks for common files");

        // Get the list of common files from the manifest
        let manifest = ProfileManifest::load_or_backfill(repo_path)?;
        let common_files = manifest.get_common_files();

        if common_files.is_empty() {
            info!("No common files to sync");
            return Ok((0, 0, Vec::new()));
        }

        // Convert to Vec<String> for the symlink manager
        let common_files_vec = common_files.to_vec();

        // Use SymlinkManager to ensure symlinks
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(repo_path.to_path_buf(), backup_enabled)?;

        symlink_mgr.ensure_common_symlinks(&common_files_vec)
    }

    /// Resolve the full list of files for a profile, including inherited
    /// and common files with proper override semantics.
    ///
    /// This is useful for UI/CLI that need to display what files would be
    /// active for a given profile.
    pub fn resolve_files(repo_path: &Path, profile_name: &str) -> Result<Vec<ResolvedFile>> {
        let manifest = Self::load_manifest(repo_path)?;
        manifest.resolve_files(profile_name)
    }

    /// Resolve the full list of packages for a profile, including inherited packages.
    pub fn resolve_packages(repo_path: &Path, profile_name: &str) -> Result<Vec<Package>> {
        let manifest = Self::load_manifest(repo_path)?;
        manifest.resolve_packages(profile_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sanitize_empty_name() {
        let result = ProfileService::create_profile(&PathBuf::from("/tmp"), "", None, None, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_delete_active_profile_fails() {
        let result = ProfileService::delete_profile(&PathBuf::from("/tmp"), "active", "active");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot delete active profile"));
    }
}
