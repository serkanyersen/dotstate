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
pub struct SymlinkTracking {
    pub version: u32,
    pub active_profile: String,
    pub symlinks: Vec<TrackedSymlink>,
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
    pub tracking: SymlinkTracking,
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
        Self::new_with_config_dir(repo_path, backup_enabled, config_dir)
    }

    /// Create a new SymlinkManager with a custom config directory.
    ///
    /// This is primarily used for testing to avoid polluting the real user's
    /// config directory with test data.
    pub fn new_with_config_dir(
        repo_path: PathBuf,
        backup_enabled: bool,
        config_dir: PathBuf,
    ) -> Result<Self> {
        // Ensure config directory exists
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        }

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

            let operation = self.create_symlink(&source, &target, file)?;
            operations.push(operation);
        }

        // Update tracking
        self.tracking.active_profile = profile_name.to_string();
        for op in &operations {
            // Track both Success AND Skipped (Skipped = symlink already correct, still ours)
            if matches!(op.status, OperationStatus::Success | OperationStatus::Skipped(_)) {
                // Check if already tracked (avoid duplicates)
                let already_tracked = self.tracking.symlinks.iter().any(|s| s.target == op.target);
                if !already_tracked {
                    self.tracking.symlinks.push(TrackedSymlink {
                        target: op.target.clone(),
                        source: op.source.clone(),
                        created_at: op.timestamp,
                        backup: op.backup.clone(),
                    });
                }
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

    /// Deactivate all symlinks, optionally restoring original files.
    ///
    /// This deactivates the ENTIRE app - all profile symlinks AND common file symlinks.
    /// Useful for temporarily disabling dotstate or as a pre-uninstall step.
    ///
    /// When `restore_files` is true, each symlink is replaced with a copy of the file
    /// from the repository, making it appear as if dotstate was never installed.
    pub fn deactivate_profile_with_restore(
        &mut self,
        _profile_name: &str, // Kept for API compatibility, but we deactivate ALL symlinks
        restore_files: bool,
    ) -> Result<Vec<SymlinkOperation>> {
        info!(
            "Deactivating all symlinks (restore_files: {})",
            restore_files
        );
        let mut operations = Vec::new();

        // Deactivate ALL tracked symlinks (profile + common)
        let all_symlinks: Vec<_> = self.tracking.symlinks.clone();

        for symlink in all_symlinks {
            debug!("Removing symlink: {:?}", symlink.target);
            let operation = if restore_files {
                self.remove_symlink_with_restore(&symlink)?
            } else {
                self.remove_symlink_completely(&symlink)?
            };
            operations.push(operation);
        }

        // Also scan for and remove untracked common file symlinks
        // (These might exist from before tracking was implemented)
        // We need to recursively walk the common directory for nested paths like .config/atuin/config.toml
        let common_path = self.repo_path.join("common");
        if common_path.exists() {
            let home_dir = crate::utils::get_home_dir();

            // Recursively collect all files in common directory
            fn collect_common_files(dir: &Path, base: &Path, files: &mut Vec<PathBuf>) {
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            // Recurse into subdirectories
                            collect_common_files(&path, base, files);
                        } else {
                            // Add file with relative path from common
                            if let Ok(relative) = path.strip_prefix(base) {
                                files.push(relative.to_path_buf());
                            }
                        }
                    }
                }
            }

            let mut common_files = Vec::new();
            collect_common_files(&common_path, &common_path, &mut common_files);

            for relative_path in common_files {
                let target_path = home_dir.join(&relative_path);

                // Check if this is a symlink pointing to our common folder
                if target_path.is_symlink() {
                    if let Ok(link_target) = fs::read_link(&target_path) {
                        let resolved = if link_target.is_absolute() {
                            link_target
                        } else if let Some(parent) = target_path.parent() {
                            parent.join(&link_target)
                        } else {
                            link_target
                        };

                        if resolved.starts_with(&common_path) {
                            // Check if we already processed this in tracked symlinks
                            let already_processed = operations.iter().any(|op| op.target == target_path);

                            if !already_processed {
                                info!("Found untracked common symlink: {:?}", target_path);
                                let tracked = TrackedSymlink {
                                    target: target_path.clone(),
                                    source: common_path.join(&relative_path),
                                    created_at: Utc::now(),
                                    backup: None,
                                };
                                let operation = if restore_files {
                                    self.remove_symlink_with_restore(&tracked)?
                                } else {
                                    self.remove_symlink_completely(&tracked)?
                                };
                                operations.push(operation);
                            }
                        }
                    }
                }
            }
        }

        // Clear all tracking
        self.tracking.symlinks.clear();
        self.tracking.active_profile.clear();

        self.save_tracking()?;
        info!(
            "Deactivated all symlinks ({} symlinks removed)",
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

        // Step 1: Deactivate old profile WITHOUT restoring files
        // We don't restore because we're about to activate a new profile
        // which will either create new symlinks or leave files unmanaged
        match self.deactivate_profile_with_restore(from, false) {
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

    /// Check if a path is a symlink that we created (points to our repo)
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

        // First check: is it tracked?
        if self.tracking.symlinks.iter().any(|s| s.target == path) {
            return Ok(true);
        }

        // Second check: does it point to our repo?
        // This catches untracked symlinks that were created before tracking was implemented
        if let Ok(link_target) = fs::read_link(path) {
            let resolved = if link_target.is_absolute() {
                link_target
            } else if let Some(parent) = path.parent() {
                parent.join(&link_target)
            } else {
                link_target
            };

            if resolved.starts_with(&self.repo_path) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Create a symlink, backing up any existing file
    fn create_symlink(
        &self,
        source: &Path,
        target: &Path,
        relative_name: &str,
    ) -> Result<SymlinkOperation> {
        let timestamp = Utc::now();
        info!("Creating symlink: {:?} -> {:?}", target, source);

        // Check if source exists
        if !source.exists() {
            warn!("Cannot create symlink: source does not exist: {:?}", source);
            return Ok(SymlinkOperation {
                source: source.to_path_buf(),
                target: target.to_path_buf(),
                backup: None,
                status: OperationStatus::Failed("Source file does not exist".to_string()),
                timestamp,
            });
        }

        debug!("Source exists: {:?}", source);

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
                            debug!("Symlink already exists and points to correct location: {:?} -> {:?}", target, source);
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
            if !parent.exists() {
                debug!("Creating parent directory for symlink: {:?}", parent);
                fs::create_dir_all(parent).context("Failed to create parent directories")?;
                info!("Created parent directory: {:?}", parent);
            }
        }

        // Create the symlink
        #[cfg(unix)]
        {
            debug!("Creating Unix symlink: {:?} -> {:?}", target, source);
            std::os::unix::fs::symlink(source, target).with_context(|| {
                format!("Failed to create symlink: {:?} -> {:?}", target, source)
            })?;
        }

        #[cfg(windows)]
        {
            debug!("Creating Windows symlink: {:?} -> {:?}", target, source);
            std::os::windows::fs::symlink_file(source, target).with_context(|| {
                format!("Failed to create symlink: {:?} -> {:?}", target, source)
            })?;
        }

        info!("Successfully created symlink: {:?} -> {:?}", target, source);
        if let Some(ref backup) = backup_path {
            debug!("Backup available at: {:?}", backup);
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
        info!("Removing symlink: {:?}", tracked.target);

        // Check if the symlink still exists
        if !tracked.target.exists() && tracked.target.symlink_metadata().is_err() {
            debug!(
                "Symlink does not exist, skipping removal: {:?}",
                tracked.target
            );
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
            warn!(
                "Target is not our symlink, skipping removal: {:?}",
                tracked.target
            );
            return Ok(SymlinkOperation {
                source: tracked.source.clone(),
                target: tracked.target.clone(),
                backup: tracked.backup.clone(),
                status: OperationStatus::Skipped("Not our symlink".to_string()),
                timestamp,
            });
        }

        // Remove the symlink
        debug!("Removing symlink: {:?}", tracked.target);
        fs::remove_file(&tracked.target).context("Failed to remove symlink")?;
        info!("Removed symlink: {:?}", tracked.target);

        // Restore from repo source first (source of truth)
        // Only fall back to backup if repo file doesn't exist
        let restored = if tracked.source.exists() {
            info!(
                "Restoring from repo source: {:?} -> {:?}",
                tracked.source, tracked.target
            );
            // Create parent directories if needed
            if let Some(parent) = tracked.target.parent() {
                if !parent.exists() {
                    debug!("Creating parent directory for restored file: {:?}", parent);
                    fs::create_dir_all(parent)
                        .context("Failed to create parent directory for restored file")?;
                }
            }

            // Copy file or directory from repo (source of truth)
            let metadata = tracked
                .source
                .metadata()
                .context("Failed to read source metadata")?;

            if metadata.is_dir() {
                debug!(
                    "Restoring directory from repo: {:?} -> {:?}",
                    tracked.source, tracked.target
                );
                crate::file_manager::copy_dir_all(&tracked.source, &tracked.target)
                    .context("Failed to copy directory from repo")?;
                info!("Restored directory from repo: {:?}", tracked.target);
            } else {
                let file_size = metadata.len();
                debug!(
                    "Restoring file from repo ({} bytes): {:?} -> {:?}",
                    file_size, tracked.source, tracked.target
                );
                fs::copy(&tracked.source, &tracked.target)
                    .context("Failed to copy file from repo")?;
                info!("Restored file from repo: {:?}", tracked.target);
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
                    debug!(
                        "Restoring from backup: {:?} -> {:?}",
                        backup, tracked.target
                    );
                    fs::rename(backup, &tracked.target).context("Failed to restore backup")?;
                    info!("Restored from backup: {:?}", tracked.target);
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
    pub fn save_tracking(&self) -> Result<()> {
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

    /// Add a single symlink to an existing profile.
    ///
    /// This is more efficient than calling activate_profile with a single file,
    /// as it doesn't need to iterate or create unnecessary data structures.
    ///
    /// # Arguments
    ///
    /// * `profile_name` - Name of the profile
    /// * `relative_path` - Path relative to home directory (e.g., ".zshrc")
    ///
    /// # Returns
    ///
    /// The symlink operation result
    pub fn add_symlink_to_profile(
        &mut self,
        profile_name: &str,
        relative_path: &str,
    ) -> Result<SymlinkOperation> {
        let profile_path = self.repo_path.join(profile_name);
        let source = profile_path.join(relative_path);
        let home_dir = crate::utils::get_home_dir();
        let target = home_dir.join(relative_path);

        info!(
            "Adding symlink to profile {}: {} -> {:?}",
            profile_name, relative_path, source
        );

        // Create backup session if backups are enabled and not already created
        if self.backup_enabled && self.backup_session.is_none() {
            if let Some(ref backup_mgr) = self.backup_manager {
                self.backup_session = Some(backup_mgr.create_backup_session()?);
                debug!("Created backup session: {:?}", self.backup_session);
            }
        }

        // Create the symlink
        let operation = self.create_symlink(&source, &target, relative_path)?;

        // Update tracking if successful
        if matches!(operation.status, OperationStatus::Success) {
            self.tracking.symlinks.push(TrackedSymlink {
                target: operation.target.clone(),
                source: operation.source.clone(),
                created_at: operation.timestamp,
                backup: operation.backup.clone(),
            });

            // Update active profile if not set
            if self.tracking.active_profile.is_empty() {
                self.tracking.active_profile = profile_name.to_string();
            }

            self.save_tracking()?;
            info!("Successfully added symlink for {}", relative_path);
        }

        Ok(operation)
    }

    /// Ensure all files in a profile have their symlinks created.
    ///
    /// This is an efficient "reconciliation" method that only creates symlinks for files
    /// that are missing them. It's perfect for after pulling changes from remote, where
    /// new files may have been added but their symlinks don't exist locally yet.
    ///
    /// Unlike `activate_profile`, this does NOT remove any existing symlinks - it only
    /// adds missing ones.
    ///
    /// # Arguments
    ///
    /// * `profile_name` - Name of the profile
    /// * `files` - List of files that should have symlinks (relative paths)
    ///
    /// # Returns
    ///
    /// A tuple of (created_count, skipped_count, errors)
    pub fn ensure_profile_symlinks(
        &mut self,
        profile_name: &str,
        files: &[String],
    ) -> Result<(usize, usize, Vec<String>)> {
        info!(
            "Ensuring symlinks for profile '{}' ({} files)",
            profile_name,
            files.len()
        );

        let profile_path = self.repo_path.join(profile_name);
        let home_dir = crate::utils::get_home_dir();

        let mut created_count = 0;
        let mut skipped_count = 0;
        let mut errors = Vec::new();

        // Create backup session if backups are enabled
        if self.backup_enabled && self.backup_session.is_none() {
            if let Some(ref backup_mgr) = self.backup_manager {
                self.backup_session = Some(backup_mgr.create_backup_session()?);
                debug!("Created backup session: {:?}", self.backup_session);
            }
        }

        for relative_path in files {
            let source = profile_path.join(relative_path);
            let target = home_dir.join(relative_path);

            // Check if source exists in repo
            if !source.exists() {
                debug!("Source file does not exist in repo, skipping: {:?}", source);
                skipped_count += 1;
                continue;
            }

            // Check if symlink already exists and points to the right place
            if target.symlink_metadata().is_ok() {
                if let Ok(metadata) = target.symlink_metadata() {
                    if metadata.is_symlink() {
                        // It's a symlink - check if it points to the right place
                        if let Ok(existing_target) = fs::read_link(&target) {
                            // Normalize paths for comparison
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

                            let source_normalized = source.canonicalize().unwrap_or(source.clone());

                            if existing_normalized == source_normalized {
                                debug!(
                                    "Symlink already exists and is correct, skipping: {:?}",
                                    target
                                );
                                skipped_count += 1;
                                continue;
                            } else {
                                debug!(
                                    "Symlink exists but points to wrong location: {:?} -> {:?} (expected: {:?})",
                                    target, existing_normalized, source_normalized
                                );
                            }
                        }
                    } else {
                        // Regular file exists at target location
                        debug!(
                            "Regular file exists at target location (not a symlink): {:?}",
                            target
                        );
                        errors.push(format!("File exists at {} (not a symlink)", relative_path));
                        continue;
                    }
                }
            }

            // Symlink doesn't exist or is incorrect - create it
            debug!("Creating symlink: {:?} -> {:?}", target, source);
            match self.create_symlink(&source, &target, relative_path) {
                Ok(operation) => {
                    if matches!(operation.status, OperationStatus::Success) {
                        // Update tracking
                        self.tracking.symlinks.push(TrackedSymlink {
                            target: operation.target.clone(),
                            source: operation.source.clone(),
                            created_at: operation.timestamp,
                            backup: operation.backup.clone(),
                        });

                        // Update active profile if not set
                        if self.tracking.active_profile.is_empty() {
                            self.tracking.active_profile = profile_name.to_string();
                        }

                        created_count += 1;
                        info!("Created symlink for {}", relative_path);
                    } else {
                        warn!(
                            "Failed to create symlink for {}: {:?}",
                            relative_path, operation.status
                        );
                        errors.push(format!("Failed to create symlink for {}", relative_path));
                    }
                }
                Err(e) => {
                    error!("Error creating symlink for {}: {}", relative_path, e);
                    errors.push(format!("Error for {}: {}", relative_path, e));
                }
            }
        }

        // Save tracking if we created any symlinks
        if created_count > 0 {
            self.save_tracking()?;
        }

        info!(
            "Symlink reconciliation complete: {} created, {} skipped, {} errors",
            created_count,
            skipped_count,
            errors.len()
        );

        Ok((created_count, skipped_count, errors))
    }

    /// Remove a specific symlink from tracking without affecting other symlinks.
    ///
    /// This is a surgical operation that only updates the tracking data for a single file,
    /// unlike deactivate_profile which removes all symlinks for a profile.
    ///
    /// # Arguments
    ///
    /// * `profile_name` - Name of the profile
    /// * `relative_path` - Path relative to home directory (e.g., ".zshrc")
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    pub fn remove_symlink_from_tracking(
        &mut self,
        profile_name: &str,
        relative_path: &str,
    ) -> Result<()> {
        let profile_path = self.repo_path.join(profile_name);
        let source_path = profile_path.join(relative_path);

        debug!(
            "Removing symlink from tracking: profile={}, path={}",
            profile_name, relative_path
        );

        // Remove the specific symlink from tracking
        let initial_count = self.tracking.symlinks.len();
        self.tracking.symlinks.retain(|s| s.source != source_path);
        let removed_count = initial_count - self.tracking.symlinks.len();

        if removed_count > 0 {
            info!(
                "Removed {} symlink(s) from tracking for {}",
                removed_count, relative_path
            );
            self.save_tracking()?;
        } else {
            debug!(
                "No symlink found in tracking for {} (may have already been removed)",
                relative_path
            );
        }

        Ok(())
    }

    // ============================================================================
    // Common File Methods - For files shared across all profiles
    // ============================================================================

    /// Add a symlink for a common file (shared across all profiles).
    ///
    /// Common files are stored in the "common" folder at the repository root
    /// and are symlinked regardless of which profile is active.
    ///
    /// # Arguments
    ///
    /// * `relative_path` - Path relative to home directory (e.g., ".gitconfig")
    ///
    /// # Returns
    ///
    /// The symlink operation result
    pub fn add_common_symlink(&mut self, relative_path: &str) -> Result<SymlinkOperation> {
        let common_path = self.repo_path.join("common");
        let source = common_path.join(relative_path);
        let home_dir = crate::utils::get_home_dir();
        let target = home_dir.join(relative_path);

        info!("Adding common symlink: {} -> {:?}", relative_path, source);

        // Ensure common directory exists
        if !common_path.exists() {
            fs::create_dir_all(&common_path).context("Failed to create common directory")?;
        }

        // Create backup session if backups are enabled and not already created
        if self.backup_enabled && self.backup_session.is_none() {
            if let Some(ref backup_mgr) = self.backup_manager {
                self.backup_session = Some(backup_mgr.create_backup_session()?);
                debug!("Created backup session: {:?}", self.backup_session);
            }
        }

        // Create the symlink
        let operation = self.create_symlink(&source, &target, relative_path)?;

        // Update tracking if successful
        if matches!(operation.status, OperationStatus::Success) {
            self.tracking.symlinks.push(TrackedSymlink {
                target: operation.target.clone(),
                source: operation.source.clone(),
                created_at: operation.timestamp,
                backup: operation.backup.clone(),
            });

            self.save_tracking()?;
            info!("Successfully added common symlink for {}", relative_path);
        }

        Ok(operation)
    }

    /// Remove a symlink for a common file and restore original if exists.
    ///
    /// # Arguments
    ///
    /// * `relative_path` - Path relative to home directory (e.g., ".gitconfig")
    ///
    /// # Returns
    ///
    /// The symlink operation result
    pub fn remove_common_symlink(&mut self, relative_path: &str) -> Result<SymlinkOperation> {
        let common_path = self.repo_path.join("common");
        let source_path = common_path.join(relative_path);

        info!("Removing common symlink: {}", relative_path);

        // Find the tracked symlink
        let symlink = self
            .tracking
            .symlinks
            .iter()
            .find(|s| s.source == source_path)
            .cloned();

        let operation = if let Some(tracked) = symlink {
            self.remove_symlink_with_restore(&tracked)?
        } else {
            // Not tracked, but try to remove if it exists
            let home_dir = crate::utils::get_home_dir();
            let target = home_dir.join(relative_path);

            if target.symlink_metadata().is_ok() {
                fs::remove_file(&target)
                    .with_context(|| format!("Failed to remove symlink: {:?}", target))?;
            }

            SymlinkOperation {
                source: source_path.clone(),
                target,
                backup: None,
                status: OperationStatus::Success,
                timestamp: Utc::now(),
            }
        };

        // Remove from tracking
        self.tracking.symlinks.retain(|s| s.source != source_path);
        self.save_tracking()?;

        Ok(operation)
    }

    /// Remove a common symlink from tracking only (without touching the actual symlink).
    ///
    /// # Arguments
    ///
    /// * `relative_path` - Path relative to home directory
    pub fn remove_common_symlink_from_tracking(&mut self, relative_path: &str) -> Result<()> {
        let common_path = self.repo_path.join("common");
        let source_path = common_path.join(relative_path);

        debug!(
            "Removing common symlink from tracking: path={}",
            relative_path
        );

        let initial_count = self.tracking.symlinks.len();
        self.tracking.symlinks.retain(|s| s.source != source_path);
        let removed_count = initial_count - self.tracking.symlinks.len();

        if removed_count > 0 {
            info!(
                "Removed {} common symlink(s) from tracking for {}",
                removed_count, relative_path
            );
            self.save_tracking()?;
        }

        Ok(())
    }

    /// Ensure all common files have their symlinks created.
    ///
    /// This is an efficient "reconciliation" method for common files.
    ///
    /// # Arguments
    ///
    /// * `files` - List of common files that should have symlinks (relative paths)
    ///
    /// # Returns
    ///
    /// A tuple of (created_count, skipped_count, errors)
    pub fn ensure_common_symlinks(
        &mut self,
        files: &[String],
    ) -> Result<(usize, usize, Vec<String>)> {
        info!("Ensuring common symlinks ({} files)", files.len());

        let common_path = self.repo_path.join("common");
        let home_dir = crate::utils::get_home_dir();

        let mut created_count = 0;
        let mut skipped_count = 0;
        let mut errors = Vec::new();

        // Create backup session if backups are enabled
        if self.backup_enabled && self.backup_session.is_none() {
            if let Some(ref backup_mgr) = self.backup_manager {
                self.backup_session = Some(backup_mgr.create_backup_session()?);
                debug!("Created backup session: {:?}", self.backup_session);
            }
        }

        for relative_path in files {
            let source = common_path.join(relative_path);
            let target = home_dir.join(relative_path);

            // Check if source exists in repo
            if !source.exists() {
                debug!("Common source file does not exist, skipping: {:?}", source);
                skipped_count += 1;
                continue;
            }

            // Check if symlink already exists and points to the right place
            if target.symlink_metadata().is_ok() {
                if let Ok(metadata) = target.symlink_metadata() {
                    if metadata.is_symlink() {
                        if let Ok(existing_target) = fs::read_link(&target) {
                            let existing_normalized = if existing_target.is_absolute() {
                                existing_target.canonicalize().unwrap_or(existing_target)
                            } else if let Some(parent) = target.parent() {
                                parent
                                    .join(&existing_target)
                                    .canonicalize()
                                    .unwrap_or_else(|_| parent.join(&existing_target))
                            } else {
                                existing_target
                            };

                            let source_normalized = source.canonicalize().unwrap_or(source.clone());

                            if existing_normalized == source_normalized {
                                debug!(
                                    "Common symlink already exists and is correct, skipping: {:?}",
                                    target
                                );
                                skipped_count += 1;
                                continue;
                            }
                        }
                    } else {
                        errors.push(format!("File exists at {} (not a symlink)", relative_path));
                        continue;
                    }
                }
            }

            // Create the symlink
            match self.create_symlink(&source, &target, relative_path) {
                Ok(op) => {
                    if matches!(op.status, OperationStatus::Success) {
                        created_count += 1;
                        self.tracking.symlinks.push(TrackedSymlink {
                            target: op.target,
                            source: op.source,
                            created_at: op.timestamp,
                            backup: op.backup,
                        });
                    }
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to create common symlink for {}: {}",
                        relative_path, e
                    ));
                }
            }
        }

        if created_count > 0 {
            self.save_tracking()?;
        }

        info!(
            "Common symlinks: {} created, {} skipped, {} errors",
            created_count,
            skipped_count,
            errors.len()
        );

        Ok((created_count, skipped_count, errors))
    }

    /// Activate all common files by creating their symlinks.
    ///
    /// # Arguments
    ///
    /// * `files` - List of common files to activate
    ///
    /// # Returns
    ///
    /// List of symlink operations
    pub fn activate_common_files(&mut self, files: &[String]) -> Result<Vec<SymlinkOperation>> {
        info!("Activating common files ({} files)", files.len());

        let common_path = self.repo_path.join("common");

        // Ensure common directory exists
        if !common_path.exists() {
            fs::create_dir_all(&common_path).context("Failed to create common directory")?;
        }

        // Create backup session if backups are enabled
        if self.backup_enabled && self.backup_session.is_none() {
            if let Some(ref backup_mgr) = self.backup_manager {
                self.backup_session = Some(backup_mgr.create_backup_session()?);
            }
        }

        let mut operations = Vec::new();
        let home_dir = crate::utils::get_home_dir();

        for file in files {
            let source = common_path.join(file);
            let target = home_dir.join(file);

            let operation = self.create_symlink(&source, &target, file)?;

            // Track both Success AND Skipped (Skipped = symlink already correct, still ours)
            if matches!(operation.status, OperationStatus::Success | OperationStatus::Skipped(_)) {
                // Check if already tracked (avoid duplicates)
                let already_tracked = self.tracking.symlinks.iter().any(|s| s.target == operation.target);
                if !already_tracked {
                    self.tracking.symlinks.push(TrackedSymlink {
                        target: operation.target.clone(),
                        source: operation.source.clone(),
                        created_at: operation.timestamp,
                        backup: operation.backup.clone(),
                    });
                }
            }

            operations.push(operation);
        }

        self.save_tracking()?;
        info!("Activated {} common files", operations.len());

        Ok(operations)
    }

    /// Check if a symlink is for a common file.
    ///
    /// # Arguments
    ///
    /// * `source_path` - The source path of the symlink
    ///
    /// # Returns
    ///
    /// True if the symlink is for a common file
    pub fn is_common_symlink(&self, source_path: &Path) -> bool {
        let common_path = self.repo_path.join("common");
        source_path.starts_with(&common_path)
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
        let config_dir = temp_dir.path().join("config"); // Isolated config directory
        fs::create_dir_all(&repo_path).unwrap();
        fs::create_dir_all(&config_dir).unwrap();

        // Use isolated config directory to avoid polluting real user config
        let manager = SymlinkManager::new_with_config_dir(repo_path, false, config_dir).unwrap();
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
