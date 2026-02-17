use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};

/// Manages centralized backups in ~/.dotstate-backups
pub struct BackupManager {
    backup_root: PathBuf,
}

impl BackupManager {
    /// Create a new `BackupManager`
    ///
    /// In tests, set `DOTSTATE_TEST_BACKUP_DIR` env var to override the backup location.
    pub fn new() -> Result<Self> {
        let backup_root = if let Ok(test_backup) = std::env::var("DOTSTATE_TEST_BACKUP_DIR") {
            PathBuf::from(test_backup)
        } else {
            let home_dir = crate::utils::get_home_dir();
            home_dir.join(".dotstate-backups")
        };

        // Ensure backup directory exists
        fs::create_dir_all(&backup_root).context("Failed to create backup directory")?;

        Ok(Self { backup_root })
    }

    /// Create a new timestamped backup directory for a sync operation
    pub fn create_backup_session(&self) -> Result<PathBuf> {
        // Use filesystem-safe timestamp (':' is invalid on some platforms/filesystems).
        let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S").to_string();
        let session_dir = self.backup_root.join(&timestamp);

        fs::create_dir_all(&session_dir).context("Failed to create backup session directory")?;

        Ok(session_dir)
    }

    /// Backup a file or directory to the backup session directory
    /// Returns the path where the backup was created
    pub fn backup_path(
        &self,
        session_dir: &Path,
        source: &Path,
        relative_name: &str,
    ) -> Result<PathBuf> {
        let backup_dest = session_dir.join(relative_name);

        // Create parent directories if needed
        if let Some(parent) = backup_dest.parent() {
            fs::create_dir_all(parent).context("Failed to create backup parent directory")?;
        }

        // Get metadata to determine if it's a directory
        let metadata = source
            .metadata()
            .context("Failed to read source metadata for backup")?;

        if metadata.is_dir() {
            crate::file_manager::copy_dir_all(source, &backup_dest).with_context(|| {
                format!("Failed to backup directory {source:?} to {backup_dest:?}")
            })?;
        } else {
            fs::copy(source, &backup_dest)
                .with_context(|| format!("Failed to backup file {source:?} to {backup_dest:?}"))?;
        }

        Ok(backup_dest)
    }

    /// Get the backup root directory
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    #[must_use]
    pub fn backup_root(&self) -> &Path {
        &self.backup_root
    }
}

impl Default for BackupManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to a safe location if we can't create the backup directory
            Self {
                backup_root: PathBuf::from("/tmp/.dotstate-backups"),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_manager_creation() {
        // Use a test-local writable backup root.
        let temp_dir = TempDir::new().unwrap();
        let backup_root = temp_dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let manager = BackupManager { backup_root };
        assert!(manager.backup_root().exists());
    }

    #[test]
    fn test_backup_session_creation() {
        // Use a test-local writable backup root.
        let temp_dir = TempDir::new().unwrap();
        let backup_root = temp_dir.path().join("backups");
        fs::create_dir_all(&backup_root).unwrap();
        let manager = BackupManager { backup_root };
        let session = manager.create_backup_session();
        assert!(session.is_ok(), "session error: {:?}", session.err());

        let session_path = session.unwrap();
        assert!(session_path.exists());
        assert!(session_path.is_dir());

        // Check that the directory name matches the timestamp format
        let dir_name = session_path.file_name().unwrap().to_str().unwrap();
        assert!(dir_name.len() == 19); // YYYY-MM-DDTHH-MM-SS
        assert!(dir_name.contains('T'));
    }
}
