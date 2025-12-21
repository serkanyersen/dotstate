use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use tracing::{debug, info, warn, error};
use crate::utils::BackupManager;

/// Represents a symlink operation (create or remove)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymlinkOperation {
    /// Source file in the profile folder (e.g., ~/.dotstate/Personal-Mac/.zshrc)
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
            let data = fs::read_to_string(&tracking_file)
                .context("Failed to read tracking file")?;
            serde_json::from_str(&data)
                .context("Failed to parse tracking file")?
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
    pub fn activate_profile(&mut self, profile_name: &str, files: &[String]) -> Result<Vec<SymlinkOperation>> {
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
            return Err(anyhow::anyhow!("Profile directory does not exist: {:?}", profile_path));
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
        info!("Profile activated: {} ({} symlinks)", profile_name, operations.len());

        Ok(operations)
    }

    /// Deactivate a profile by removing its symlinks
    pub fn deactivate_profile(&mut self, profile_name: &str) -> Result<Vec<SymlinkOperation>> {
        info!("Deactivating profile: {}", profile_name);
        let mut operations = Vec::new();
        let profile_path = self.repo_path.join(profile_name);

        // Find all symlinks for this profile
        let profile_symlinks: Vec<_> = self.tracking.symlinks
            .iter()
            .filter(|s| s.source.starts_with(&profile_path))
            .cloned()
            .collect();

        for symlink in profile_symlinks {
            debug!("Removing symlink: {:?}", symlink.target);
            let operation = self.remove_symlink(&symlink)?;
            operations.push(operation);
        }

        // Remove from tracking
        self.tracking.symlinks.retain(|s| !s.source.starts_with(&profile_path));

        if self.tracking.active_profile == profile_name {
            self.tracking.active_profile.clear();
        }

        self.save_tracking()?;
        info!("Profile deactivated: {} ({} symlinks removed)", profile_name, operations.len());

        Ok(operations)
    }

    /// Switch from one profile to another
    pub fn switch_profile(&mut self, from: &str, to: &str, to_files: &[String]) -> Result<SwitchReport> {
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
                warn!("Attempting rollback to profile: {}", from);
                if let Err(rollback_err) = self.rollback_to_profile(from) {
                    error!("Rollback failed: {}", rollback_err);
                    report.errors.push((PathBuf::from("rollback"), rollback_err.to_string()));
                } else {
                    info!("Rollback successful");
                    report.rollback_performed = true;
                }
            }
        }

        Ok(report)
    }

    /// Preview what would happen during a switch (dry run)
    pub fn preview_switch(&self, from: &str, to: &str, to_files: &[String]) -> Result<SwitchPreview> {
        let profile_path = self.repo_path.join(from);
        let new_profile_path = self.repo_path.join(to);
        let home_dir = crate::utils::get_home_dir();

        // What will be removed
        let will_remove: Vec<_> = self.tracking.symlinks
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
        if !path.exists() && !path.symlink_metadata().is_ok() {
            return Ok(false);
        }

        // Check if it's a symlink
        let metadata = path.symlink_metadata()
            .context("Failed to read symlink metadata")?;

        if !metadata.is_symlink() {
            return Ok(false);
        }

        // Check if we're tracking it
        Ok(self.tracking.symlinks.iter().any(|s| s.target == path))
    }

    /// Create a symlink, backing up any existing file
    fn create_symlink(&self, source: &Path, target: &Path, relative_name: &str) -> Result<SymlinkOperation> {
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
                                parent.join(&existing_target).canonicalize().unwrap_or_else(|_| parent.join(&existing_target))
                            } else {
                                existing_target
                            }
                        };

                        let source_normalized = source.canonicalize().unwrap_or(source.to_path_buf());

                        if existing_normalized == source_normalized {
                            // Already points to the right place, skip
                            return Ok(SymlinkOperation {
                                source: source.to_path_buf(),
                                target: target.to_path_buf(),
                                backup: None,
                                status: OperationStatus::Skipped("Symlink already exists and points to correct location".to_string()),
                                timestamp,
                            });
                        }

                        // Symlink points to wrong place - try to resolve and backup the actual file
                        // if it exists, then remove the symlink
                        if existing_normalized.exists() {
                            // Try to canonicalize, but if it fails (orphaned symlink), use the path as-is
                            let resolved = existing_normalized.canonicalize().unwrap_or(existing_normalized);
                            if resolved.exists() {
                                // The symlink points to an existing file - back it up
                                if let Some(ref session) = self.backup_session {
                                    if let Some(ref backup_mgr) = self.backup_manager {
                                        match backup_mgr.backup_path(session, &resolved, relative_name) {
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
                    fs::remove_file(target)
                        .with_context(|| format!("Failed to remove existing symlink: {:?}", target))?;
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
                        fs::remove_dir_all(target)
                            .with_context(|| format!("Failed to remove existing directory: {:?}", target))?;
                    } else {
                        fs::remove_file(target)
                            .with_context(|| format!("Failed to remove existing file: {:?}", target))?;
                    }
                }
            }
        }

        // Create parent directories if needed
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create parent directories")?;
        }

        // Create the symlink
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, target)
                .with_context(|| format!("Failed to create symlink: {:?} -> {:?}", target, source))?;
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(source, target)
                .with_context(|| format!("Failed to create symlink: {:?} -> {:?}", target, source))?;
        }

        Ok(SymlinkOperation {
            source: source.to_path_buf(),
            target: target.to_path_buf(),
            backup: backup_path,
            status: OperationStatus::Success,
            timestamp,
        })
    }

    /// Remove a symlink, restoring backup if it exists
    fn remove_symlink(&self, tracked: &TrackedSymlink) -> Result<SymlinkOperation> {
        let timestamp = Utc::now();

        // Check if the symlink still exists
        if !tracked.target.exists() && !tracked.target.symlink_metadata().is_ok() {
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
        fs::remove_file(&tracked.target)
            .context("Failed to remove symlink")?;

        // Restore backup if it exists
        if let Some(backup) = &tracked.backup {
            if backup.exists() {
                fs::rename(backup, &tracked.target)
                    .context("Failed to restore backup")?;
            }
        }

        Ok(SymlinkOperation {
            source: tracked.source.clone(),
            target: tracked.target.clone(),
            backup: tracked.backup.clone(),
            status: OperationStatus::Success,
            timestamp,
        })
    }


    /// Attempt to rollback to a profile (used when switch fails)
    fn rollback_to_profile(&mut self, profile_name: &str) -> Result<()> {
        // This is a simplified rollback - in a real scenario, we'd need to restore
        // the exact state from before the switch attempt
        // For now, we'll just try to reactivate the old profile

        warn!("Rollback functionality is simplified - manual intervention may be needed");

        // Try to get the files for the profile from config
        // This is a placeholder - in reality, we'd need access to the config
        let files = Vec::new(); // TODO: Get from config

        self.activate_profile(profile_name, &files)?;

        Ok(())
    }

    /// Save tracking data to disk
    fn save_tracking(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.tracking)
            .context("Failed to serialize tracking data")?;

        // Ensure config directory exists
        if let Some(parent) = self.tracking_file.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        fs::write(&self.tracking_file, json)
            .context("Failed to write tracking file")?;

        debug!("Tracking data saved to: {:?}", self.tracking_file);
        Ok(())
    }

    /// Get the currently active profile name
    pub fn get_active_profile(&self) -> Option<&str> {
        if self.tracking.active_profile.is_empty() {
            None
        } else {
            Some(&self.tracking.active_profile)
        }
    }

    /// Get all tracked symlinks
    pub fn get_tracked_symlinks(&self) -> &[TrackedSymlink] {
        &self.tracking.symlinks
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

        let manager = SymlinkManager::new(repo_path).unwrap();
        (temp_dir, manager)
    }

    #[test]
    fn test_create_symlink_manager() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("dotstate");
        fs::create_dir_all(&repo_path).unwrap();

        let manager = SymlinkManager::new(repo_path.clone());
        assert!(manager.is_ok());
    }

    #[test]
    fn test_activate_profile() {
        let (temp_dir, mut manager) = setup_test_env();

        // Create a profile directory with a file
        let profile_path = temp_dir.path().join("dotstate/test-profile");
        fs::create_dir_all(&profile_path).unwrap();

        let test_file = profile_path.join(".testrc");
        File::create(&test_file).unwrap().write_all(b"test content").unwrap();

        // Activate profile
        let result = manager.activate_profile("test-profile", &[".testrc".to_string()]);
        assert!(result.is_ok());

        let operations = result.unwrap();
        assert_eq!(operations.len(), 1);
        assert!(matches!(operations[0].status, OperationStatus::Success));
    }

    // More tests would go here...
}

