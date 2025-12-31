use crate::utils::BackupManager;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

/// Represents a symlink operation (create or remove)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkOperation {
    /// Source file in the profile folder (e.g., ~/.config/dotstate/storage/Personal-Mac/.zshrc)
    pub source: PathBuf,
    /// Target symlink location in home directory (e.g., ~/.zshrc)
    pub target: PathBuf,
    /// Backup of the original file if it existed
    pub backup: Option<PathBuf>,
    /// Status of this operation
    pub status: OperationStatus,
    /// When this operation was performed
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OperationStatus {
    Success,
    Failed(String),
    Skipped(String),
    RolledBack,
}

/// Report of a profile switch operation
#[derive(Debug)]
pub struct SwitchReport {
    /// Symlinks that were removed (old profile)
    pub removed: Vec<SymlinkOperation>,
    /// Symlinks that were created (new profile)
    pub created: Vec<SymlinkOperation>,
    /// Any errors that occurred
    pub errors: Vec<(PathBuf, String)>,
    /// Whether a rollback was performed
    pub rollback_performed: bool,
}

/// Preview of what would happen during a switch
#[derive(Debug)]
/// Preview of what would happen during a profile switch (dry run)
#[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
pub struct SwitchPreview {
    /// Symlinks that will be removed
    pub will_remove: Vec<PathBuf>,
    /// Symlinks that will be created
    pub will_create: Vec<(PathBuf, PathBuf)>, // (target, source)
    /// Files that exist and aren't our symlinks (potential conflicts)
    pub conflicts: Vec<PathBuf>,
}

/// Tracked symlink information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedSymlink {
    pub target: PathBuf,
    pub source: PathBuf,
    pub created_at: DateTime<Utc>,
    pub backup: Option<PathBuf>,
}

/// Tracking data for all symlinks we manage
#[derive(Debug, Serialize, Deserialize)]
struct SymlinkTracking {
    version: u32,
    active_profile: String,
    symlinks: Vec<TrackedSymlink>,
}

impl Default for SymlinkTracking {
    fn default() -> Self {
        Self {
            version: 1,
            active_profile: String::new(),
            symlinks: Vec::new(),
        }
    }
}

/// Manages symlinks for dotfile profiles
pub struct SymlinkManager {
    /// Path to the dotfiles repository
    repo_path: PathBuf,
    /// Path to the tracking file
    tracking_file: PathBuf,
    /// Current tracking data
    tracking: SymlinkTracking,
    /// Whether backups are enabled
    backup_enabled: bool,
    /// Backup manager for centralized backups
    backup_manager: Option<BackupManager>,
    /// Current backup session directory (if backups are enabled and session started)
    backup_session: Option<PathBuf>,
}

impl SymlinkManager {
    /// Create a new SymlinkManager
    pub fn new(repo_path: PathBuf) -> Result<Self> {
        Self::new_with_backup(repo_path, true)
    }

    /// Create a new SymlinkManager with backup settings
    pub fn new_with_backup(repo_path: PathBuf, backup_enabled: bool) -> Result<Self> {
        let config_dir = crate::utils::get_config_dir();
        let tracking_file = config_dir.join("symlinks.json");

        // Load existing tracking data or create new
        let tracking = if tracking_file.exists() {
            let data =
                fs::read_to_string(&tracking_file).context("Failed to read tracking file")?;
            serde_json::from_str(&data).context("Failed to parse tracking file")?
        } else {
            SymlinkTracking::default()
        };

        let backup_manager = if backup_enabled {
            Some(BackupManager::new()?)
        } else {
            None
        };

        Ok(Self {
            repo_path,
            tracking_file,
            tracking,
            backup_enabled,
            backup_manager,
            backup_session: None,
        })
    }

    /// Activate a profile by creating all its symlinks
    pub fn activate_profile(
        &mut self,
        profile_name: &str,
        files: &[String],
    ) -> Result<Vec<SymlinkOperation>> {
        info!("Activating profile: {}", profile_name);

        // Create backup session if backups are enabled
        if self.backup_enabled {
            if let Some(ref backup_mgr) = self.backup_manager {
                self.backup_session = Some(backup_mgr.create_backup_session()?);
                info!("Created backup session: {:?}", self.backup_session);
            }
        }

        let mut operations = Vec::new();
        let profile_path = self.repo_path.join(profile_name);

        if !profile_path.exists() {
            return Err(anyhow::anyhow!(
                "Profile directory does not exist: {:?}",
                profile_path
            ));
        }

        let home_dir = crate::utils::get_home_dir();

        for file in files {
            let source = profile_path.join(file);
            let target = home_dir.join(file);

            debug!("Creating symlink: {:?} -> {:?}", target, source);

            let operation = self.create_symlink(&source, &target, file)?;
            operations.push(operation);
        }

        // Update tracking
        self.tracking.active_profile = profile_name.to_string();
        for op in &operations {
            if matches!(op.status, OperationStatus::Success) {
                self.tracking.symlinks.push(TrackedSymlink {
                    target: op.target.clone(),
                    source: op.source.clone(),
                    created_at: op.timestamp,
                    backup: op.backup.clone(),
                });
            }
        }

        self.save_tracking()?;
        info!(
            "Profile activated: {} ({} symlinks)",
            profile_name,
            operations.len()
        );

        Ok(operations)
    }

    /// Deactivate a profile by removing its symlinks
    pub fn deactivate_profile(&mut self, profile_name: &str) -> Result<Vec<SymlinkOperation>> {
        self.deactivate_profile_with_restore(profile_name, true)
    }

    /// Deactivate a profile, optionally restoring original files
    pub fn deactivate_profile_with_restore(
        &mut self,
        profile_name: &str,
        restore_files: bool,
    ) -> Result<Vec<SymlinkOperation>> {
        info!(
            "Deactivating profile: {} (restore_files: {})",
            profile_name, restore_files
        );
        let mut operations = Vec::new();
        let profile_path = self.repo_path.join(profile_name);

        // Find all symlinks for this profile
        let profile_symlinks: Vec<_> = self
            .tracking
            .symlinks
            .iter()
            .filter(|s| s.source.starts_with(&profile_path))
            .cloned()
            .collect();

        for symlink in profile_symlinks {
            debug!("Removing symlink: {:?}", symlink.target);
            let operation = if restore_files {
                self.remove_symlink_with_restore(&symlink)?
            } else {
                self.remove_symlink_completely(&symlink)?
            };
            operations.push(operation);
        }

        // Remove from tracking
        self.tracking
            .symlinks
            .retain(|s| !s.source.starts_with(&profile_path));

        if self.tracking.active_profile == profile_name {
            self.tracking.active_profile.clear();
        }

        self.save_tracking()?;
        info!(
            "Profile deactivated: {} ({} symlinks removed)",
            profile_name,
            operations.len()
        );

        Ok(operations)
    }

    /// Switch from one profile to another
    pub fn switch_profile(
        &mut self,
        from: &str,
        to: &str,
        to_files: &[String],
    ) -> Result<SwitchReport> {
        info!("Switching profile: {} -> {}", from, to);

        let mut report = SwitchReport {
            removed: Vec::new(),
            created: Vec::new(),
            errors: Vec::new(),
            rollback_performed: false,
        };

        // Step 1: Deactivate old profile
        match self.deactivate_profile(from) {
            Ok(ops) => report.removed = ops,
            Err(e) => {
                error!("Failed to deactivate profile {}: {}", from, e);
                report.errors.push((PathBuf::from(from), e.to_string()));
                return Ok(report);
            }
        }

        // Step 2: Activate new profile
        match self.activate_profile(to, to_files) {
            Ok(ops) => report.created = ops,
            Err(e) => {
                error!("Failed to activate profile {}: {}", to, e);
                report.errors.push((PathBuf::from(to), e.to_string()));

                // Attempt rollback - reactivate the old profile
                // Note: We don't have access to the old profile's files here,
                // so rollback is limited. The caller should handle this better.
                warn!("Attempting rollback to profile: {}", from);
                // Rollback requires the profile's file list, which we don't have here
                // This is a limitation - rollback should be handled at a higher level
                report.rollback_performed = false;
                report.errors.push((
                    PathBuf::from("rollback"),
                    format!(
                        "Rollback not fully supported - profile '{}' may need manual reactivation",
                        from
                    ),
                ));
            }
        }

        Ok(report)
    }

    /// Preview what would happen during a switch (dry run)
    #[allow(dead_code)]
    pub fn preview_switch(
        &self,
        from: &str,
        to: &str,
        to_files: &[String],
    ) -> Result<SwitchPreview> {
        let profile_path = self.repo_path.join(from);
        let new_profile_path = self.repo_path.join(to);
        let home_dir = crate::utils::get_home_dir();

        // What will be removed
        let will_remove: Vec<_> = self
            .tracking
            .symlinks
            .iter()
            .filter(|s| s.source.starts_with(&profile_path))
            .map(|s| s.target.clone())
            .collect();

        // What will be created
        let mut will_create = Vec::new();
        let mut conflicts = Vec::new();

        for file in to_files {
            let source = new_profile_path.join(file);
            let target = home_dir.join(file);

            will_create.push((target.clone(), source));

            // Check for conflicts
            if target.exists() && !self.is_our_symlink(&target)? {
                conflicts.push(target);
            }
        }

        Ok(SwitchPreview {
            will_remove,
            will_create,
            conflicts,
        })
    }

    /// Check if a path is a symlink that we created
    fn is_our_symlink(&self, path: &Path) -> Result<bool> {
        if !path.exists() && path.symlink_metadata().is_err() {
            return Ok(false);
        }

        // Check if it's a symlink
        let metadata = path
            .symlink_metadata()
            .context("Failed to read symlink metadata")?;

        if !metadata.is_symlink() {
            return Ok(false);
        }

        // Check if we're tracking it
        Ok(self.tracking.symlinks.iter().any(|s| s.target == path))
    }

    /// Create a symlink, backing up any existing file
    fn create_symlink(
        &self,
        source: &Path,
        target: &Path,
        relative_name: &str,
    ) -> Result<SymlinkOperation> {
        let timestamp = Utc::now();

        // Check if source exists
        if !source.exists() {
            return Ok(SymlinkOperation {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                backup: None,
                status: OperationStatus::Failed("Source file does not exist".to_string()),
                timestamp,
            });
        }

        let mut backup_path = None;

        // Handle existing target (file, directory, or symlink)
        if target.symlink_metadata().is_ok() {
            // Check if it's a symlink first
            if let Ok(metadata) = target.symlink_metadata() {
                if metadata.is_symlink() {
                    // It's a symlink - check if it points to the right place
                    if let Ok(existing_target) = fs::read_link(target) {
                        // Normalize paths for comparison (handle relative vs absolute)
                        let existing_normalized = if existing_target.is_absolute() {
                            existing_target.canonicalize().unwrap_or(existing_target)
                        } else {
                            // Relative symlink - resolve relative to target's parent
                            if let Some(parent) = target.parent() {
                                parent
                                    .join(&existing_target)
                                    .canonicalize()
                                    .unwrap_or_else(|_| parent.join(&existing_target))
                            } else {
                                existing_target
                            }
                        };

                        let source_normalized =
                            source.canonicalize().unwrap_or(source.to_path_buf());

                        if existing_normalized == source_normalized {
                            // Already points to the right place, skip
                            return Ok(SymlinkOperation {
                                source: source.to_path_buf(),
                                target: target.to_path_buf(),
                                backup: None,
                                status: OperationStatus::Skipped(
                                    "Symlink already exists and points to correct location"
                                        .to_string(),
                                ),
                                timestamp,
                            });
                        }

                        // Symlink points to wrong place - try to resolve and backup the actual file
                        // if it exists, then remove the symlink
                        if existing_normalized.exists() {
                            // Try to canonicalize, but if it fails (orphaned symlink), use the path as-is
                            let resolved = existing_normalized
                                .canonicalize()
                                .unwrap_or(existing_normalized);
                            if resolved.exists() {
                                // The symlink points to an existing file - back it up
                                if let Some(ref session) = self.backup_session {
                                    if let Some(ref backup_mgr) = self.backup_manager {
                                        match backup_mgr.backup_path(
                                            session,
                                            &resolved,
                                            relative_name,
                                        ) {
                                            Ok(backup) => backup_path = Some(backup),
                                            Err(e) => {
                                                warn!("Failed to backup file pointed to by symlink {:?}: {}", target, e);
                                                // Continue anyway - we'll still remove the symlink
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Remove the symlink (whether we backed up or not)
                    fs::remove_file(target).with_context(|| {
                        format!("Failed to remove existing symlink: {:?}", target)
                    })?;
                } else if metadata.is_file() || metadata.is_dir() {
                    // It's a real file or directory, back it up
                    if let Some(ref session) = self.backup_session {
                        if let Some(ref backup_mgr) = self.backup_manager {
                            match backup_mgr.backup_path(session, target, relative_name) {
                                Ok(backup) => backup_path = Some(backup),
                                Err(e) => {
                                    warn!("Failed to backup {:?}: {}", target, e);
                                    // Continue anyway - we'll still remove/replace the file
                                }
                            }
                        }
                    }
                    if metadata.is_dir() {
                        fs::remove_dir_all(target).with_context(|| {
                            format!("Failed to remove existing directory: {:?}", target)
                        })?;
                    } else {
                        fs::remove_file(target).with_context(|| {
                            format!("Failed to remove existing file: {:?}", target)
                        })?;
                    }
                }
            }
        }

        // Create parent directories if needed
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).context("Failed to create parent directories")?;
        }

        // Create the symlink
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, target).with_context(|| {
                format!("Failed to create symlink: {:?} -> {:?}", target, source)
            })?;
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(source, target).with_context(|| {
                format!("Failed to create symlink: {:?} -> {:?}", target, source)
            })?;
        }

        Ok(SymlinkOperation {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            backup: backup_path,
            status: OperationStatus::Success,
            timestamp,
        })
    }

    /// Remove a symlink, restoring backup if it exists, or copying from repo if no backup
    fn remove_symlink_with_restore(&self, tracked: &TrackedSymlink) -> Result<SymlinkOperation> {
        let timestamp = Utc::now();

        // Check if the symlink still exists
        if !tracked.target.exists() && tracked.target.symlink_metadata().is_err() {
            return Ok(SymlinkOperation {
                source: tracked.source.clone(),
                target: tracked.target.clone(),
                backup: tracked.backup.clone(),
                status: OperationStatus::Skipped("Symlink does not exist".to_string()),
                timestamp,
            });
        }

        // Verify it's still our symlink
        if !self.is_our_symlink(&tracked.target)? {
            return Ok(SymlinkOperation {
                source: tracked.source.clone(),
                target: tracked.target.clone(),
                backup: tracked.backup.clone(),
                status: OperationStatus::Skipped("Not our symlink".to_string()),
                timestamp,
            });
        }

        // Remove the symlink
        fs::remove_file(&tracked.target).context("Failed to remove symlink")?;

        // Restore from repo source first (source of truth)
        // Only fall back to backup if repo file doesn't exist
        let restored = if tracked.source.exists() {
            // Create parent directories if needed
            if let Some(parent) = tracked.target.parent() {
                fs::create_dir_all(parent)
                    .context("Failed to create parent directory for restored file")?;
            }

            // Copy file or directory from repo (source of truth)
            let metadata = tracked
                .source
                .metadata()
                .context("Failed to read source metadata")?;

            if metadata.is_dir() {
                crate::file_manager::copy_dir_all(&tracked.source, &tracked.target)
                    .context("Failed to copy directory from repo")?;
            } else {
                fs::copy(&tracked.source, &tracked.target)
                    .context("Failed to copy file from repo")?;
            }
            true
        } else {
            // Repo file doesn't exist - try backup as last resort
            if let Some(backup) = &tracked.backup {
                if backup.exists() {
                    warn!(
                        "Repo file {:?} not found, restoring from backup {:?}",
                        tracked.source, backup
                    );
                    // Restore from backup (last resort)
                    fs::rename(backup, &tracked.target).context("Failed to restore backup")?;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if !restored {
            warn!(
                "Could not restore {:?}: repo file doesn't exist and no backup available",
                tracked.target
            );
        }

        Ok(SymlinkOperation {
            source: tracked.source.clone(),
            target: tracked.target.clone(),
            backup: tracked.backup.clone(),
            status: OperationStatus::Success,
            timestamp,
        })
    }

    /// Remove a symlink completely without restoring any files
    fn remove_symlink_completely(&self, tracked: &TrackedSymlink) -> Result<SymlinkOperation> {
        let timestamp = Utc::now();

        // Check if the symlink still exists
        if !tracked.target.exists() && tracked.target.symlink_metadata().is_err() {
            return Ok(SymlinkOperation {
                source: tracked.source.clone(),
                target: tracked.target.clone(),
                backup: tracked.backup.clone(),
                status: OperationStatus::Skipped("Symlink does not exist".to_string()),
                timestamp,
            });
        }

        // Verify it's still our symlink
        if !self.is_our_symlink(&tracked.target)? {
            return Ok(SymlinkOperation {
                source: tracked.source.clone(),
                target: tracked.target.clone(),
                backup: tracked.backup.clone(),
                status: OperationStatus::Skipped("Not our symlink".to_string()),
                timestamp,
            });
        }

        // Remove the symlink (no restore)
        fs::remove_file(&tracked.target).context("Failed to remove symlink")?;

        Ok(SymlinkOperation {
            source: tracked.source.clone(),
            target: tracked.target.clone(),
            backup: tracked.backup.clone(),
            status: OperationStatus::Success,
            timestamp,
        })
    }

    /// Remove a symlink, restoring backup if it exists (legacy method, calls remove_symlink_with_restore)
    fn remove_symlink(&self, tracked: &TrackedSymlink) -> Result<SymlinkOperation> {
        self.remove_symlink_with_restore(tracked)
    }

    /// Rollback to a previous profile state
    ///
    /// This is a simplified rollback that attempts to reactivate the specified profile.
    /// In a real scenario, we'd need to restore the exact state from before the switch attempt.
    ///
    /// # Arguments
    /// * `profile_name` - Name of the profile to rollback to
    /// * `files` - List of files that should be synced for this profile
    ///
    /// # Returns
    /// * `Ok(())` if rollback was successful
    /// * `Err` if rollback failed
    ///
    /// # Note
    /// This function is currently not used because rollback requires the profile's file list,
    /// which is not available at the point where rollback would be needed.
    #[allow(dead_code)]
    fn rollback_to_profile(&mut self, profile_name: &str, files: &[String]) -> Result<()> {
        warn!("Rollback functionality is simplified - manual intervention may be needed");
        self.activate_profile(profile_name, files)?;
        Ok(())
    }

    /// Save tracking data to disk
    fn save_tracking(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.tracking)
            .context("Failed to serialize tracking data")?;

        // Ensure config directory exists
        if let Some(parent) = self.tracking_file.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        fs::write(&self.tracking_file, json).context("Failed to write tracking file")?;

        debug!("Tracking data saved to: {:?}", self.tracking_file);
        Ok(())
    }

    /// Get the currently active profile name
    /// Get the currently active profile name
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    pub fn get_active_profile(&self) -> Option<&str> {
        if self.tracking.active_profile.is_empty() {
            None
        } else {
            Some(&self.tracking.active_profile)
        }
    }

    /// Get all tracked symlinks
    /// Get all tracked symlinks
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    pub fn get_tracked_symlinks(&self) -> &[TrackedSymlink] {
        &self.tracking.symlinks
    }

    /// Rename a profile and update all associated symlinks
    /// This updates the source paths in the tracking file and recreates symlinks
    /// to point to the new profile folder location
    pub fn rename_profile(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<Vec<SymlinkOperation>> {
        info!("Renaming profile: {} -> {}", old_name, new_name);

        let old_profile_path = self.repo_path.join(old_name);
        let new_profile_path = self.repo_path.join(new_name);
        let mut operations = Vec::new();

        // Find all symlinks for this profile
        let profile_symlinks: Vec<_> = self
            .tracking
            .symlinks
            .iter()
            .filter(|s| s.source.starts_with(&old_profile_path))
            .cloned()
            .collect();

        if profile_symlinks.is_empty() {
            // No symlinks to update, but still update active_profile name if needed
            if self.tracking.active_profile == old_name {
                self.tracking.active_profile = new_name.to_string();
                self.save_tracking()?;
            }
            return Ok(operations);
        }

        // Update each symlink
        for symlink in &profile_symlinks {
            // Calculate new source path
            let relative_path = match symlink.source.strip_prefix(&old_profile_path) {
                Ok(path) => path,
                Err(e) => {
                    error!(
                        "Failed to get relative path from {:?}: {}",
                        symlink.source, e
                    );
                    continue;
                }
            };
            let new_source = new_profile_path.join(relative_path);

            // Get relative name for create_symlink (e.g., ".zshrc" from "/path/to/repo/old/.zshrc")
            let relative_name = relative_path.to_string_lossy().to_string();

            // Remove old symlink
            let remove_op = self.remove_symlink(symlink)?;
            operations.push(remove_op);

            // Create new symlink pointing to new source
            let create_op = self.create_symlink(&new_source, &symlink.target, &relative_name)?;
            operations.push(create_op);
        }

        // Update tracking: remove old entries and add new ones
        self.tracking
            .symlinks
            .retain(|s| !s.source.starts_with(&old_profile_path));

        // Add updated entries
        for symlink in &profile_symlinks {
            let relative_path = match symlink.source.strip_prefix(&old_profile_path) {
                Ok(path) => path,
                Err(e) => {
                    error!(
                        "Failed to get relative path from {:?}: {}",
                        symlink.source, e
                    );
                    continue;
                }
            };
            let new_source = new_profile_path.join(relative_path);

            self.tracking.symlinks.push(TrackedSymlink {
                target: symlink.target.clone(),
                source: new_source,
                created_at: symlink.created_at,
                backup: symlink.backup.clone(),
            });
        }

        // Update active profile name if this was the active profile
        if self.tracking.active_profile == old_name {
            self.tracking.active_profile = new_name.to_string();
        }

        self.save_tracking()?;
        info!(
            "Profile renamed: {} -> {} ({} symlinks updated)",
            old_name,
            new_name,
            profile_symlinks.len()
        );

        Ok(operations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, SymlinkManager) {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("dotstate");
        fs::create_dir_all(&repo_path).unwrap();

        // Disable backups to avoid issues with home directory in CI
        let manager = SymlinkManager::new_with_backup(repo_path, false).unwrap();
        (temp_dir, manager)
    }

    #[test]
    fn test_create_symlink_manager() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("dotstate");
        fs::create_dir_all(&repo_path).unwrap();

        // Disable backups to avoid issues with home directory in CI
        let manager = SymlinkManager::new_with_backup(repo_path.clone(), false);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_activate_profile() {
        let (temp_dir, mut manager) = setup_test_env();

        // Create a profile directory with a file
        let profile_path = temp_dir.path().join("dotstate/test-profile");
        fs::create_dir_all(&profile_path).unwrap();

        let test_file = profile_path.join(".testrc");
        File::create(&test_file)
            .unwrap()
            .write_all(b"test content")
            .unwrap();

        // Activate profile
        let result = manager.activate_profile("test-profile", &[".testrc".to_string()]);
        assert!(result.is_ok());

        let operations = result.unwrap();
        assert_eq!(operations.len(), 1);
        assert!(matches!(operations[0].status, OperationStatus::Success));
    }

    // More tests would go here...
}
