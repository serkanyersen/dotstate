use anyhow::{Context, Result};
use std::fs;
// Note: symlink and MetadataExt are used via std::os::unix::fs:: paths
use std::path::{Path, PathBuf};

/// Manages dotfile operations: scanning, backing up, symlinking
pub struct FileManager {
    home_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Dotfile {
    /// Original path of the file
    pub original_path: PathBuf,
    /// Path relative to home directory
    pub relative_path: PathBuf,
    /// Whether this file is currently synced
    pub synced: bool,
}

impl FileManager {
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir()
            .context("Could not determine home directory")?;

        Ok(Self { home_dir })
    }

    /// Scan for dotfiles based on the provided list
    pub fn scan_dotfiles(&self, dotfile_names: &[String]) -> Vec<Dotfile> {
        let mut found = Vec::new();

        for name in dotfile_names {
            let path = self.home_dir.join(name);
            // Only include if the path actually exists (file or directory)
            if path.exists() {
                let relative = path
                    .strip_prefix(&self.home_dir)
                    .unwrap_or(&path)
                    .to_path_buf();

                found.push(Dotfile {
                    original_path: path.clone(),
                    relative_path: relative,
                    synced: false,
                });
            }
        }

        found
    }

    /// Create a backup of a file by adding .bak extension
    pub fn backup_file(&self, file_path: &Path) -> Result<PathBuf> {
        // Create backup path by appending .bak to the filename
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("backup");

        let backup_name = format!("{}.bak", file_name);
        let backup_path = file_path.parent()
            .map(|p| p.join(&backup_name))
            .unwrap_or_else(|| PathBuf::from(&backup_name));

        if file_path.is_file() {
            fs::copy(file_path, &backup_path)
                .with_context(|| format!("Failed to backup file: {:?}", file_path))?;
        } else if file_path.is_dir() {
            // For directories, create a backup with .bak suffix
            copy_dir_all(file_path, &backup_path)?;
        }

        Ok(backup_path)
    }

    /// Resolve a symlink to its target, following multiple levels if needed
    pub fn resolve_symlink(&self, path: &Path) -> Result<PathBuf> {
        let mut current = path.to_path_buf();
        let mut depth = 0;
        const MAX_SYMLINK_DEPTH: usize = 20; // Prevent infinite loops

        while current.is_symlink() && depth < MAX_SYMLINK_DEPTH {
            current = fs::read_link(&current)
                .with_context(|| format!("Failed to read symlink: {:?}", current))?;

            // If the symlink is relative, resolve it relative to the parent
            if current.is_relative() {
                if let Some(parent) = path.parent() {
                    current = parent.join(&current);
                }
            }

            depth += 1;
        }

        if depth >= MAX_SYMLINK_DEPTH {
            return Err(anyhow::anyhow!("Symlink depth exceeded for: {:?}", path));
        }

        Ok(current)
    }

    /// Check if a path is a symlink
    pub fn is_symlink(&self, path: &Path) -> bool {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            metadata.file_type().is_symlink()
        } else {
            false
        }
    }

    /// Copy file or directory recursively
    pub fn copy_to_repo(&self, source: &Path, dest: &Path) -> Result<()> {
        // Create parent directory
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
        }

        if source.is_file() {
            fs::copy(source, dest)
                .with_context(|| format!("Failed to copy file from {:?} to {:?}", source, dest))?;
        } else if source.is_dir() {
            copy_dir_all(source, dest)
                .with_context(|| format!("Failed to copy directory from {:?} to {:?}", source, dest))?;
        } else {
            return Err(anyhow::anyhow!("Source path is neither file nor directory: {:?}", source));
        }

        Ok(())
    }

    /// Restore original file from backup or repo
    pub fn restore_original(&self, target: &Path, repo_path: &Path, relative_path: &Path) -> Result<()> {
        // Remove symlink if it exists
        if self.is_symlink(target) || target.exists() {
            if target.is_dir() {
                fs::remove_dir_all(target)
                    .with_context(|| format!("Failed to remove directory: {:?}", target))?;
            } else {
                fs::remove_file(target)
                    .with_context(|| format!("Failed to remove file: {:?}", target))?;
            }
        }

        // Try to restore from backup first
        let backup_path = if target.is_file() {
            let file_name = target.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("backup");
            let backup_name = format!("{}.bak", file_name);
            target.parent().map(|p| p.join(&backup_name))
        } else {
            None
        };

        if let Some(backup) = backup_path {
            if backup.exists() {
                if backup.is_file() {
                    fs::copy(&backup, target)?;
                    fs::remove_file(&backup)?; // Clean up backup
                } else if backup.is_dir() {
                    copy_dir_all(&backup, target)?;
                    fs::remove_dir_all(&backup)?; // Clean up backup
                }
                return Ok(());
            }
        }

        // If no backup, try to restore from repo
        let repo_file = repo_path.join(relative_path);
        if repo_file.exists() {
            self.copy_to_repo(&repo_file, target)?;
        } else {
            // If neither backup nor repo file exists, create empty file/dir
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            if target.extension().is_some() {
                // Likely a file
                fs::File::create(target)?;
            } else {
                // Likely a directory
                fs::create_dir_all(target)?;
            }
        }

        Ok(())
    }

    /// Create a symlink from target to source
    pub fn create_symlink(&self, source: &Path, target: &Path) -> Result<()> {
        // Remove existing file/directory if it exists (after backup)
        if target.exists() {
            self.backup_file(target)?;
            if target.is_dir() {
                fs::remove_dir_all(target)
                    .with_context(|| format!("Failed to remove directory: {:?}", target))?;
            } else {
                fs::remove_file(target)
                    .with_context(|| format!("Failed to remove file: {:?}", target))?;
            }
        }

        // Create parent directories if needed
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;
        }

        // Create symlink
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(source, target)
                .with_context(|| format!("Failed to create symlink from {:?} to {:?}", source, target))?;
        }

        #[cfg(windows)]
        {
            std::os::windows::fs::symlink_file(source, target)
                .with_context(|| format!("Failed to create symlink from {:?} to {:?}", source, target))?;
        }

        Ok(())
    }

    /// Sync file to repository: handle symlinks, copy original, create new symlink
    pub fn sync_file(&self, dotfile: &Dotfile, repo_path: &Path, profile: &str) -> Result<()> {
        let repo_file_path = repo_path
            .join(profile)
            .join(&dotfile.relative_path);

        // Create parent directories in repo
        if let Some(parent) = repo_file_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create repo directory: {:?}", parent))?;
        }

        // Handle symlinks: resolve to original file
        let source_path = if self.is_symlink(&dotfile.original_path) {
            self.resolve_symlink(&dotfile.original_path)?
        } else {
            dotfile.original_path.clone()
        };

        // Copy original file/directory to repo
        self.copy_to_repo(&source_path, &repo_file_path)?;

        // Create symlink from original location to repo
        self.create_symlink(&repo_file_path, &dotfile.original_path)?;

        Ok(())
    }

    /// Unsync file: remove from repo and restore original
    pub fn unsync_file(&self, dotfile: &Dotfile, repo_path: &Path, profile: &str) -> Result<()> {
        let repo_file_path = repo_path
            .join(profile)
            .join(&dotfile.relative_path);

        // Restore original file (pass the profile path for restore_original)
        let repo_relative_path = PathBuf::from(profile).join(&dotfile.relative_path);
        self.restore_original(&dotfile.original_path, repo_path, &repo_relative_path)?;

        // Remove from repo
        if repo_file_path.exists() {
            if repo_file_path.is_dir() {
                fs::remove_dir_all(&repo_file_path)
                    .with_context(|| format!("Failed to remove directory from repo: {:?}", repo_file_path))?;
            } else {
                fs::remove_file(&repo_file_path)
                    .with_context(|| format!("Failed to remove file from repo: {:?}", repo_file_path))?;
            }
        }

        Ok(())
    }

    /// Get home directory
    #[allow(dead_code)]
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }
}

/// Recursively copy a directory
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create destination directory: {:?}", dst))?;

    for entry in fs::read_dir(src)
        .with_context(|| format!("Failed to read directory: {:?}", src))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_all(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)
                .with_context(|| format!("Failed to copy file: {:?}", path))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_manager_creation() {
        let fm = FileManager::new().unwrap();
        assert!(fm.home_dir().exists());
    }

    #[test]
    fn test_scan_dotfiles() {
        let fm = FileManager::new().unwrap();
        let dotfiles = fm.scan_dotfiles(&[".bashrc".to_string(), ".nonexistent".to_string()]);
        // Results depend on what exists in home directory
        assert!(dotfiles.len() <= 1);
    }
}

