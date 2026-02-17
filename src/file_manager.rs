use anyhow::{Context, Result};
use std::fs;
// Note: symlink and MetadataExt are used via std::os::unix::fs:: paths
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Utility for file operations: scanning dotfiles, copying files/directories, and resolving symlinks.
///
/// Note: Symlink creation/management is handled by `SymlinkManager`, not this module.
/// This module provides low-level file utilities only.
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
    /// Description of the dotfile (if available from default list)
    pub description: Option<String>,
    /// Whether this file is a common file (shared across all profiles)
    pub is_common: bool,
    /// Whether this file was manually added via `config.custom_files` (not auto-detected)
    pub is_custom: bool,
}

impl FileManager {
    pub fn new() -> Result<Self> {
        let home_dir = dirs::home_dir().context("Could not determine home directory")?;

        Ok(Self { home_dir })
    }

    /// Scan for dotfiles based on the provided list
    #[must_use]
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

                // Try to find description from default candidates
                let description = crate::dotfile_candidates::find_candidate(name)
                    .map(|c| c.description.to_string());

                found.push(Dotfile {
                    original_path: path.clone(),
                    relative_path: relative,
                    synced: false,
                    description,
                    is_common: false,
                    is_custom: false,
                });
            }
        }

        found
    }

    /// Resolve a symlink to its target, following multiple levels if needed
    pub fn resolve_symlink(&self, path: &Path) -> Result<PathBuf> {
        debug!("Resolving symlink: {:?}", path);
        let mut current = path.to_path_buf();
        let mut depth = 0;
        const MAX_SYMLINK_DEPTH: usize = 20; // Prevent infinite loops

        while current.is_symlink() && depth < MAX_SYMLINK_DEPTH {
            let target = fs::read_link(&current)
                .with_context(|| format!("Failed to read symlink: {current:?}"))?;
            debug!("Symlink at depth {}: {:?} -> {:?}", depth, current, target);

            // If the symlink is relative, resolve it relative to the parent
            if target.is_relative() {
                if let Some(parent) = current.parent() {
                    current = parent.join(&target);
                    debug!("Resolved relative symlink: {:?}", current);
                } else {
                    current = target;
                }
            } else {
                current = target;
            }

            depth += 1;
        }

        if depth >= MAX_SYMLINK_DEPTH {
            warn!(
                "Symlink depth exceeded (max {}) for: {:?}",
                MAX_SYMLINK_DEPTH, path
            );
            return Err(anyhow::anyhow!("Symlink depth exceeded for: {path:?}"));
        }

        debug!(
            "Resolved symlink: {:?} -> {:?} (depth: {})",
            path, current, depth
        );
        Ok(current)
    }

    /// Check if a path is a symlink
    #[must_use]
    pub fn is_symlink(&self, path: &Path) -> bool {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            metadata.file_type().is_symlink()
        } else {
            false
        }
    }

    /// Copy file or directory recursively
    pub fn copy_to_repo(&self, source: &Path, dest: &Path) -> Result<()> {
        info!("Starting copy operation: {:?} -> {:?}", source, dest);

        // Remove destination if it exists (to avoid conflicts)
        if dest.exists() {
            if dest.is_dir() {
                info!("Removing existing directory at destination: {:?}", dest);
                fs::remove_dir_all(dest)
                    .with_context(|| format!("Failed to remove existing directory: {dest:?}"))?;
                debug!("Successfully removed existing directory: {:?}", dest);
            } else {
                info!("Removing existing file at destination: {:?}", dest);
                fs::remove_file(dest)
                    .with_context(|| format!("Failed to remove existing file: {dest:?}"))?;
                debug!("Successfully removed existing file: {:?}", dest);
            }
        }

        // Create parent directory
        if let Some(parent) = dest.parent() {
            if !parent.exists() {
                debug!("Creating parent directory: {:?}", parent);
                fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create parent directory: {parent:?}"))?;
                info!("Created parent directory: {:?}", parent);
            }
        }

        // Use metadata to check file type (follows symlinks)
        let source_metadata = fs::metadata(source)
            .with_context(|| format!("Failed to read metadata for source: {source:?}"))?;

        if source_metadata.is_file() {
            let file_size = source_metadata.len();
            info!(
                "Copying file ({} bytes): {:?} -> {:?}",
                file_size, source, dest
            );
            let bytes_copied = fs::copy(source, dest)
                .with_context(|| format!("Failed to copy file from {source:?} to {dest:?}"))?;
            info!(
                "Successfully copied file ({} bytes): {:?}",
                bytes_copied, dest
            );
            debug!(
                "File copy complete: source={:?}, dest={:?}, size={}",
                source, dest, bytes_copied
            );
        } else if source_metadata.is_dir() {
            info!("Copying directory recursively: {:?} -> {:?}", source, dest);
            copy_dir_all(source, dest)
                .with_context(|| format!("Failed to copy directory from {source:?} to {dest:?}"))?;
            info!("Successfully copied directory: {:?} -> {:?}", source, dest);
        } else {
            warn!("Source path is neither file nor directory: {:?}", source);
            return Err(anyhow::anyhow!(
                "Source path is neither file nor directory: {source:?}"
            ));
        }

        Ok(())
    }

    /// Get home directory
    #[allow(dead_code)]
    #[must_use]
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }
}

/// Recursively copy a directory, preserving symlinks.
///
/// NOTE: This function creates symlinks directly with `std::os::unix::fs::symlink()`
/// instead of using `SymlinkManager`. This is intentional because:
/// - These are **internal symlinks within dotfile content** (e.g., `~/.config/app/current -> versions/v1`)
/// - They are part of the user's configuration, not DotState-managed infrastructure
/// - `SymlinkManager` only tracks symlinks that link home files to the repo (e.g., `~/.zshrc -> repo/Profile/.zshrc`)
/// - Internal content symlinks should be preserved as-is without tracking
pub fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    debug!("Creating destination directory: {:?}", dst);
    fs::create_dir_all(dst)
        .with_context(|| format!("Failed to create destination directory: {dst:?}"))?;

    let mut files_copied = 0;
    let mut dirs_copied = 0;
    let mut symlinks_copied = 0;
    let mut skipped = 0;

    for entry in fs::read_dir(src).with_context(|| format!("Failed to read directory: {src:?}"))? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);

        // Check for symlinks first (before is_dir/is_file which follow symlinks)
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            // Preserve symlinks as symlinks
            match fs::read_link(&path) {
                Ok(link_target) => {
                    debug!(
                        "Copying symlink: {:?} -> {:?} (target: {:?})",
                        path, dst_path, link_target
                    );
                    // Remove existing destination if it exists
                    if dst_path.symlink_metadata().is_ok() {
                        let _ = fs::remove_file(&dst_path);
                    }
                    #[cfg(unix)]
                    {
                        if let Err(e) = std::os::unix::fs::symlink(&link_target, &dst_path) {
                            warn!(
                                "Failed to create symlink {:?} -> {:?}: {}",
                                dst_path, link_target, e
                            );
                            skipped += 1;
                            continue;
                        }
                    }
                    #[cfg(windows)]
                    {
                        // On Windows, we need to know if target is file or directory
                        // For broken symlinks, default to file symlink
                        let target_is_dir = link_target.is_dir();
                        let result = if target_is_dir {
                            std::os::windows::fs::symlink_dir(&link_target, &dst_path)
                        } else {
                            std::os::windows::fs::symlink_file(&link_target, &dst_path)
                        };
                        if let Err(e) = result {
                            warn!(
                                "Failed to create symlink {:?} -> {:?}: {}",
                                dst_path, link_target, e
                            );
                            skipped += 1;
                            continue;
                        }
                    }
                    symlinks_copied += 1;
                }
                Err(e) => {
                    warn!("Failed to read symlink target for {:?}: {}", path, e);
                    skipped += 1;
                }
            }
        } else if file_type.is_dir() {
            debug!("Copying subdirectory: {:?} -> {:?}", path, dst_path);
            copy_dir_all(&path, &dst_path)?;
            dirs_copied += 1;
        } else {
            if let Ok(metadata) = path.metadata() {
                let file_size = metadata.len();
                debug!(
                    "Copying file ({} bytes): {:?} -> {:?}",
                    file_size, path, dst_path
                );
            } else {
                debug!("Copying file: {:?} -> {:?}", path, dst_path);
            }
            fs::copy(&path, &dst_path).with_context(|| format!("Failed to copy file: {path:?}"))?;
            files_copied += 1;
        }
    }

    debug!(
        "Directory copy complete: {:?} -> {:?} ({} files, {} dirs, {} symlinks, {} skipped)",
        src, dst, files_copied, dirs_copied, symlinks_copied, skipped
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_file_manager_creation() {
        let fm = FileManager::new().unwrap();
        assert!(fm.home_dir().exists());
    }

    #[test]
    fn test_scan_dotfiles() {
        let temp_dir = TempDir::new().unwrap();
        let home_dir = temp_dir.path();

        // Create a mock FileManager with temp directory as home
        let fm = FileManager {
            home_dir: home_dir.to_path_buf(),
        };

        // Create some test files
        let test_file1 = home_dir.join(".testrc");
        File::create(&test_file1)
            .unwrap()
            .write_all(b"test")
            .unwrap();

        // Don't create .nonexistent - it shouldn't be found

        let dotfiles = fm.scan_dotfiles(&[".testrc".to_string(), ".nonexistent".to_string()]);

        assert_eq!(dotfiles.len(), 1);
        assert_eq!(dotfiles[0].relative_path, PathBuf::from(".testrc"));
        assert_eq!(dotfiles[0].original_path, test_file1);
        assert!(!dotfiles[0].synced);
    }

    #[test]
    fn test_scan_dotfiles_with_subdirectory() {
        let temp_dir = TempDir::new().unwrap();
        let home_dir = temp_dir.path();

        let fm = FileManager {
            home_dir: home_dir.to_path_buf(),
        };

        // Create a nested directory
        let nested_dir = home_dir.join(".config").join("test");
        std::fs::create_dir_all(&nested_dir).unwrap();

        let dotfiles = fm.scan_dotfiles(&[".config/test".to_string()]);

        assert_eq!(dotfiles.len(), 1);
        assert_eq!(dotfiles[0].relative_path, PathBuf::from(".config/test"));
    }

    #[test]
    fn test_is_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create a real file
        let real_file = temp_dir.path().join("real_file");
        File::create(&real_file).unwrap();
        assert!(!fm.is_symlink(&real_file));

        // Create a symlink
        let symlink_target = temp_dir.path().join("target");
        File::create(&symlink_target).unwrap();
        let symlink = temp_dir.path().join("symlink");
        std::os::unix::fs::symlink(&symlink_target, &symlink).unwrap();
        assert!(fm.is_symlink(&symlink));
        assert!(!fm.is_symlink(&symlink_target));
    }

    #[test]
    fn test_resolve_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create target file
        let target = temp_dir.path().join("target");
        File::create(&target)
            .unwrap()
            .write_all(b"content")
            .unwrap();

        // Create symlink
        let symlink = temp_dir.path().join("symlink");
        std::os::unix::fs::symlink(&target, &symlink).unwrap();

        // Resolve symlink
        let resolved = fm.resolve_symlink(&symlink).unwrap();
        assert_eq!(resolved, target);
        assert!(resolved.exists());
    }

    #[test]
    fn test_resolve_symlink_chain() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create target
        let target = temp_dir.path().join("target");
        File::create(&target).unwrap();

        // Create chain: symlink1 -> symlink2 -> target
        let symlink2 = temp_dir.path().join("symlink2");
        std::os::unix::fs::symlink(&target, &symlink2).unwrap();

        let symlink1 = temp_dir.path().join("symlink1");
        std::os::unix::fs::symlink(&symlink2, &symlink1).unwrap();

        // Resolve should follow the chain
        let resolved = fm.resolve_symlink(&symlink1).unwrap();
        assert_eq!(resolved, target);
    }

    #[test]
    fn test_resolve_symlink_relative() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create target
        let target = temp_dir.path().join("target");
        File::create(&target).unwrap();

        // Create relative symlink
        let symlink = temp_dir.path().join("symlink");
        std::os::unix::fs::symlink("target", &symlink).unwrap();

        // Resolve should handle relative paths
        let resolved = fm.resolve_symlink(&symlink).unwrap();
        assert!(resolved.exists());
    }

    #[test]
    fn test_copy_to_repo_file() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create source file
        let source = temp_dir.path().join("source.txt");
        File::create(&source)
            .unwrap()
            .write_all(b"test content")
            .unwrap();

        // Copy to destination
        let dest = temp_dir.path().join("dest.txt");
        fm.copy_to_repo(&source, &dest).unwrap();

        // Verify copy
        assert!(dest.exists());
        assert!(dest.is_file());
        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "test content");
    }

    #[test]
    fn test_copy_to_repo_directory() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create source directory with files
        let source_dir = temp_dir.path().join("source_dir");
        std::fs::create_dir_all(&source_dir).unwrap();

        let file1 = source_dir.join("file1.txt");
        File::create(&file1).unwrap().write_all(b"file1").unwrap();

        let nested_dir = source_dir.join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let file2 = nested_dir.join("file2.txt");
        File::create(&file2).unwrap().write_all(b"file2").unwrap();

        // Copy directory
        let dest_dir = temp_dir.path().join("dest_dir");
        fm.copy_to_repo(&source_dir, &dest_dir).unwrap();

        // Verify copy
        assert!(dest_dir.exists());
        assert!(dest_dir.is_dir());
        assert!(dest_dir.join("file1.txt").exists());
        assert!(dest_dir.join("nested").is_dir());
        assert!(dest_dir.join("nested/file2.txt").exists());

        let content1 = std::fs::read_to_string(dest_dir.join("file1.txt")).unwrap();
        assert_eq!(content1, "file1");
        let content2 = std::fs::read_to_string(dest_dir.join("nested/file2.txt")).unwrap();
        assert_eq!(content2, "file2");
    }

    #[test]
    fn test_copy_to_repo_overwrites_existing() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create existing destination
        let dest = temp_dir.path().join("dest.txt");
        File::create(&dest)
            .unwrap()
            .write_all(b"old content")
            .unwrap();

        // Create new source
        let source = temp_dir.path().join("source.txt");
        File::create(&source)
            .unwrap()
            .write_all(b"new content")
            .unwrap();

        // Copy should overwrite
        fm.copy_to_repo(&source, &dest).unwrap();

        let content = std::fs::read_to_string(&dest).unwrap();
        assert_eq!(content, "new content");
    }

    #[test]
    fn test_copy_dir_all() {
        let temp_dir = TempDir::new().unwrap();

        // Create source structure
        let source = temp_dir.path().join("source");
        std::fs::create_dir_all(&source).unwrap();

        File::create(source.join("a.txt"))
            .unwrap()
            .write_all(b"a")
            .unwrap();
        File::create(source.join("b.txt"))
            .unwrap()
            .write_all(b"b")
            .unwrap();

        let nested = source.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        File::create(nested.join("c.txt"))
            .unwrap()
            .write_all(b"c")
            .unwrap();

        // Copy
        let dest = temp_dir.path().join("dest");
        copy_dir_all(&source, &dest).unwrap();

        // Verify
        assert!(dest.exists());
        assert!(dest.is_dir());
        assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(dest.join("b.txt")).unwrap(), "b");
        assert_eq!(
            std::fs::read_to_string(dest.join("nested/c.txt")).unwrap(),
            "c"
        );
    }

    #[test]
    fn test_resolve_symlink_max_depth() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager::new().unwrap();

        // Create a very long symlink chain (should fail at MAX_SYMLINK_DEPTH = 20)
        let mut current = temp_dir.path().join("target");
        File::create(&current).unwrap();

        // Create 25 symlinks in a chain
        for i in 0..25 {
            let next = temp_dir.path().join(format!("link{i}"));
            std::os::unix::fs::symlink(&current, &next).unwrap();
            current = next;
        }

        // Resolving should fail due to max depth
        let result = fm.resolve_symlink(&current);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Symlink depth exceeded"));
    }

    #[test]
    fn test_scan_dotfiles_empty_list() {
        let temp_dir = TempDir::new().unwrap();
        let fm = FileManager {
            home_dir: temp_dir.path().to_path_buf(),
        };

        let dotfiles = fm.scan_dotfiles(&[]);
        assert!(dotfiles.is_empty());
    }

    #[test]
    fn test_is_symlink_nonexistent() {
        let fm = FileManager::new().unwrap();
        let nonexistent = PathBuf::from("/nonexistent/path/that/does/not/exist");
        assert!(!fm.is_symlink(&nonexistent));
    }
}
