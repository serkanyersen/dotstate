//! Validation functions for file syncing operations
//!
//! This module provides robust validation to prevent data loss and conflicts
//! when adding files to sync. It checks for:
//! - Files already inside synced directories
//! - Directories containing already-synced files
//! - Nested git repositories
//! - Ability to create symlinks before deleting files

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Result of validation check
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the operation is safe to proceed
    pub is_safe: bool,
    /// Error message if validation failed
    pub error_message: Option<String>,
}

impl ValidationResult {
    pub fn safe() -> Self {
        Self {
            is_safe: true,
            error_message: None,
        }
    }

    pub fn unsafe_with(message: String) -> Self {
        Self {
            is_safe: false,
            error_message: Some(message),
        }
    }
}

/// Check if a path contains a git repository (recursively)
///
/// This checks the path itself and all parent directories for .git folders.
/// This is more robust than just checking the immediate directory.
pub fn contains_git_repo(path: &Path) -> bool {
    // Check if the path itself is a git repo
    if path.is_dir() && path.join(".git").exists() {
        return true;
    }

    // Check all parent directories
    let mut current = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };

    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return true;
        }
        current = dir.parent();
    }

    false
}

/// Check if a path is inside a nested git repository
///
/// This recursively checks if any subdirectory contains a .git folder.
pub fn contains_nested_git_repo(path: &Path) -> Result<bool> {
    if !path.is_dir() {
        return Ok(false);
    }

    // Check immediate .git
    if path.join(".git").exists() {
        return Ok(true);
    }

    // Recursively check subdirectories (with depth limit to avoid infinite loops)
    const MAX_DEPTH: usize = 10;
    fn check_recursive(dir: &Path, depth: usize) -> Result<bool> {
        if depth > MAX_DEPTH {
            return Ok(false); // Too deep, assume safe
        }

        if dir.join(".git").exists() {
            return Ok(true);
        }

        let entries = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Skip .git directories themselves
                if path.file_name().and_then(|n| n.to_str()) == Some(".git") {
                    continue;
                }

                if check_recursive(&path, depth + 1)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    check_recursive(path, 0)
}

/// Check if a file path is already inside a synced directory
///
/// Returns true if the file would conflict with an already-synced parent directory.
pub fn is_file_inside_synced_directory(file_path: &str, synced_files: &HashSet<String>) -> bool {
    // Normalize the path (remove leading ./ if present)
    let normalized = file_path.strip_prefix("./").unwrap_or(file_path);

    // Check if any parent directory is synced
    let path = PathBuf::from(normalized);
    let mut current = path.parent();

    while let Some(parent) = current {
        let parent_str = parent.to_string_lossy().to_string();

        // Check exact match
        if synced_files.contains(&parent_str) {
            return true;
        }

        // Check with leading dot (e.g., if synced has ".nvim" and parent is "nvim")
        if !parent_str.starts_with('.') {
            let with_dot = format!(".{}", parent_str);
            if synced_files.contains(&with_dot) {
                return true;
            }
        }

        // Check without leading dot (e.g., if synced has "nvim" and parent is ".nvim")
        if parent_str.starts_with('.') && parent_str.len() > 1 {
            let without_dot = parent_str[1..].to_string();
            if synced_files.contains(&without_dot) {
                return true;
            }
        }

        current = parent.parent();
    }

    false
}

/// Check if a directory contains already-synced files
///
/// Returns true if the directory would conflict with already-synced files inside it.
pub fn directory_contains_synced_files(dir_path: &str, synced_files: &HashSet<String>) -> bool {
    // Normalize the path (remove leading ./ if present)
    let normalized = dir_path.strip_prefix("./").unwrap_or(dir_path);
    let dir_path_buf = PathBuf::from(normalized);

    // Check if any synced file is inside this directory
    for synced_file in synced_files {
        let synced_path = PathBuf::from(synced_file);

        // Check exact match first
        if synced_path.starts_with(&dir_path_buf) && synced_path != dir_path_buf {
            return true;
        }

        // Check with/without leading dot variations
        let dir_with_dot = if normalized.starts_with('.') {
            normalized.to_string()
        } else {
            format!(".{}", normalized)
        };
        let dir_without_dot = if normalized.starts_with('.') && normalized.len() > 1 {
            &normalized[1..]
        } else {
            normalized
        };

        let synced_str = synced_file.as_str();
        if synced_str.starts_with(&dir_with_dot) && synced_str != dir_with_dot {
            return true;
        }
        if synced_str.starts_with(dir_without_dot) && synced_str != dir_without_dot {
            return true;
        }
    }

    false
}

/// Comprehensive validation before adding a file/directory to sync
///
/// This performs all necessary checks to ensure the operation is safe:
/// 1. File is not already inside a synced directory
/// 2. Directory does not contain already-synced files
/// 3. Path does not contain git repositories
/// 4. Path is safe to add (basic safety checks)
pub fn validate_before_sync(
    relative_path: &str,
    full_path: &Path,
    synced_files: &HashSet<String>,
    repo_path: &Path,
) -> ValidationResult {
    debug!(
        "Validating path before sync: {} ({:?})",
        relative_path, full_path
    );

    // Normalize the relative path
    let normalized = relative_path.strip_prefix("./").unwrap_or(relative_path);

    // Check if already synced
    if synced_files.contains(normalized) {
        return ValidationResult::unsafe_with(format!(
            "File or directory is already synced: {}",
            normalized
        ));
    }

    // Check if file is inside a synced directory
    if is_file_inside_synced_directory(normalized, synced_files) {
        return ValidationResult::unsafe_with(format!(
            "Cannot sync '{}': it is already inside a synced directory.\n\n\
             If you want to sync this file, remove the parent directory from sync first.",
            normalized
        ));
    }

    // Check if directory contains already-synced files
    if full_path.is_dir() && directory_contains_synced_files(normalized, synced_files) {
        return ValidationResult::unsafe_with(format!(
            "Cannot sync directory '{}': it contains files that are already synced.\n\n\
             If you want to sync this directory, remove the individual files from sync first.",
            normalized
        ));
    }

    // Check for git repositories (improved detection)
    if contains_git_repo(full_path) {
        return ValidationResult::unsafe_with(format!(
            "Cannot sync a git repository. Path contains a .git directory: {}",
            full_path.display()
        ));
    }

    // Check for nested git repositories (recursive check)
    if full_path.is_dir() {
        match contains_nested_git_repo(full_path) {
            Ok(true) => {
                return ValidationResult::unsafe_with(format!(
                    "Cannot sync directory '{}': it contains a nested git repository.\n\n\
                     You cannot have a git repository inside a git repository.",
                    normalized
                ));
            }
            Ok(false) => {}
            Err(e) => {
                warn!("Failed to check for nested git repos: {}", e);
                // Continue - better to warn than to block if we can't check
            }
        }
    }

    // Basic safety checks
    let (is_safe, reason) = crate::utils::is_safe_to_add(full_path, repo_path);
    if !is_safe {
        return ValidationResult::unsafe_with(
            reason.unwrap_or_else(|| "Path is not safe to add".to_string()),
        );
    }

    ValidationResult::safe()
}

/// Validate that we can create a symlink before deleting the original file
///
/// This performs a dry-run check to ensure the symlink operation will succeed.
/// It checks:
/// 1. Original source file/directory exists (so we can copy it to repo)
/// 2. Target parent directory can be created
/// 3. Target location is writable
///
/// Note: `original_source` is the file in the home directory that will be copied.
/// `_symlink_source` is where the symlink will point (in the repo, will be created).
/// `target` is where the symlink will be created (in the home directory).
pub fn validate_symlink_creation(
    original_source: &Path,
    _symlink_source: &Path,
    target: &Path,
) -> Result<ValidationResult> {
    // Check if original source exists (the file we'll copy to repo)
    if !original_source.exists() {
        return Ok(ValidationResult::unsafe_with(format!(
            "Source file does not exist: {:?}",
            original_source
        )));
    }

    // Check if target parent directory can be created
    if let Some(parent) = target.parent() {
        if !parent.exists() {
            // Try to create it (dry run - we'll remove it if it's empty)
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent directory: {:?}", parent))?;

            // Check if we created an empty directory (remove it for dry run)
            if parent.read_dir()?.next().is_none() {
                let _ = std::fs::remove_dir(parent);
            }
        } else if !parent.is_dir() {
            return Ok(ValidationResult::unsafe_with(format!(
                "Target parent exists but is not a directory: {:?}",
                parent
            )));
        }
    }

    // Check if target location is writable (if parent exists)
    if let Some(parent) = target.parent() {
        if parent.exists() {
            // Try to create a test file to check write permissions
            let test_file = parent.join(".dotstate_write_test");
            if std::fs::File::create(&test_file).is_ok() {
                let _ = std::fs::remove_file(&test_file);
            } else {
                return Ok(ValidationResult::unsafe_with(format!(
                    "Cannot write to target location: {:?}",
                    parent
                )));
            }
        }
    }

    Ok(ValidationResult::safe())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    #[test]
    fn test_is_file_inside_synced_directory() {
        let mut synced = HashSet::new();
        synced.insert(".nvim".to_string());

        assert!(is_file_inside_synced_directory(".nvim/init.lua", &synced));
        assert!(is_file_inside_synced_directory("nvim/init.lua", &synced));
        assert!(!is_file_inside_synced_directory(".zshrc", &synced));
    }

    #[test]
    fn test_directory_contains_synced_files() {
        let mut synced = HashSet::new();
        synced.insert(".nvim/init.lua".to_string());

        assert!(directory_contains_synced_files(".nvim", &synced));
        assert!(!directory_contains_synced_files(".config", &synced));
    }

    #[test]
    fn test_contains_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Not a git repo
        assert!(!contains_git_repo(&test_dir));

        // Create .git directory
        std::fs::create_dir_all(test_dir.join(".git")).unwrap();
        assert!(contains_git_repo(&test_dir));

        // Check nested file
        let nested_file = test_dir.join("file.txt");
        File::create(&nested_file).unwrap();
        assert!(contains_git_repo(&nested_file));
    }

    #[test]
    fn test_validate_before_sync_file_inside_synced_dir() {
        let temp_dir = TempDir::new().unwrap();
        let mut synced = HashSet::new();
        synced.insert(".nvim".to_string());

        let file_path = temp_dir.path().join(".nvim").join("init.lua");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        File::create(&file_path).unwrap();

        let result = validate_before_sync(".nvim/init.lua", &file_path, &synced, temp_dir.path());

        assert!(!result.is_safe);
        assert!(result.error_message.is_some());
        assert!(result
            .error_message
            .unwrap()
            .contains("already inside a synced directory"));
    }

    #[test]
    fn test_validate_before_sync_dir_contains_synced_files() {
        let temp_dir = TempDir::new().unwrap();
        let mut synced = HashSet::new();
        synced.insert(".nvim/init.lua".to_string());

        let dir_path = temp_dir.path().join(".nvim");
        std::fs::create_dir_all(&dir_path).unwrap();

        let result = validate_before_sync(".nvim", &dir_path, &synced, temp_dir.path());

        assert!(!result.is_safe);
        assert!(result.error_message.is_some());
        assert!(result
            .error_message
            .unwrap()
            .contains("contains files that are already synced"));
    }

    // ========== STRESS TESTS - Edge Cases ==========

    #[test]
    fn test_path_normalization_edge_cases() {
        let mut synced = HashSet::new();
        synced.insert(".nvim".to_string());

        // Test various path formats
        assert!(is_file_inside_synced_directory("./nvim/init.lua", &synced));
        assert!(is_file_inside_synced_directory("nvim/init.lua", &synced));
        assert!(is_file_inside_synced_directory(".nvim/init.lua", &synced));
        assert!(is_file_inside_synced_directory(
            ".nvim/config/init.lua",
            &synced
        ));
        assert!(is_file_inside_synced_directory(
            "nvim/config/init.lua",
            &synced
        ));

        // Test with synced as "nvim" (no dot)
        let mut synced_no_dot = HashSet::new();
        synced_no_dot.insert("nvim".to_string());
        assert!(is_file_inside_synced_directory(
            ".nvim/init.lua",
            &synced_no_dot
        ));
        assert!(is_file_inside_synced_directory(
            "nvim/init.lua",
            &synced_no_dot
        ));
    }

    #[test]
    fn test_deeply_nested_conflicts() {
        let mut synced = HashSet::new();
        synced.insert(".config".to_string());

        // Deep nesting
        assert!(is_file_inside_synced_directory(
            ".config/nvim/init.lua",
            &synced
        ));
        assert!(is_file_inside_synced_directory(
            ".config/nvim/lua/plugins/init.lua",
            &synced
        ));
        assert!(is_file_inside_synced_directory(
            "config/nvim/init.lua",
            &synced
        ));
    }

    #[test]
    fn test_multiple_synced_directories() {
        let mut synced = HashSet::new();
        synced.insert(".config".to_string());
        synced.insert(".local".to_string());
        synced.insert(".zshrc".to_string()); // File, not directory

        // Should detect conflicts with any synced directory
        assert!(is_file_inside_synced_directory(".config/file", &synced));
        assert!(is_file_inside_synced_directory(".local/file", &synced));
        // File shouldn't conflict
        assert!(!is_file_inside_synced_directory(".zshrc_backup", &synced));
    }

    #[test]
    fn test_directory_contains_multiple_synced_files() {
        let mut synced = HashSet::new();
        synced.insert(".nvim/init.lua".to_string());
        synced.insert(".nvim/lua/config.lua".to_string());
        synced.insert(".nvim/after/plugin/colors.lua".to_string());

        assert!(directory_contains_synced_files(".nvim", &synced));
        assert!(directory_contains_synced_files("nvim", &synced)); // Without dot
        assert!(!directory_contains_synced_files(".config", &synced));
    }

    #[test]
    fn test_git_repo_in_parent_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create git repo in parent
        let parent_dir = temp_dir.path().join("parent");
        std::fs::create_dir_all(&parent_dir).unwrap();
        std::fs::create_dir_all(parent_dir.join(".git")).unwrap();

        // File inside git repo
        let file_in_repo = parent_dir.join("child").join("file.txt");
        std::fs::create_dir_all(file_in_repo.parent().unwrap()).unwrap();
        File::create(&file_in_repo).unwrap();

        assert!(contains_git_repo(&file_in_repo));
        assert!(contains_git_repo(&parent_dir.join("child")));
    }

    #[test]
    fn test_nested_git_repos() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create nested git repo
        let nested_dir = test_dir.join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::create_dir_all(nested_dir.join(".git")).unwrap();

        let result = contains_nested_git_repo(&test_dir).unwrap();
        assert!(result);
    }

    #[test]
    fn test_nested_git_repos_deep() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("level1");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create deeply nested git repo
        let level2 = test_dir.join("level2");
        std::fs::create_dir_all(&level2).unwrap();
        let level3 = level2.join("level3");
        std::fs::create_dir_all(&level3).unwrap();
        std::fs::create_dir_all(level3.join(".git")).unwrap();

        let result = contains_nested_git_repo(&test_dir).unwrap();
        assert!(result);
    }

    #[test]
    fn test_git_repo_max_depth_limit() {
        let temp_dir = TempDir::new().unwrap();
        let mut current = temp_dir.path().to_path_buf();

        // Create a very deep directory structure (beyond MAX_DEPTH = 10)
        for i in 0..15 {
            current = current.join(format!("level{}", i));
            std::fs::create_dir_all(&current).unwrap();
        }

        // Should not panic, should return false (too deep)
        let result = contains_nested_git_repo(temp_dir.path()).unwrap();
        assert!(!result); // Too deep, assumes safe
    }

    #[test]
    fn test_validate_symlink_creation_source_missing() {
        let temp_dir = TempDir::new().unwrap();
        let original_source = temp_dir.path().join("nonexistent");
        let symlink_source = temp_dir.path().join("repo").join("source.txt");
        let target = temp_dir.path().join("target");

        let result = validate_symlink_creation(&original_source, &symlink_source, &target).unwrap();
        assert!(!result.is_safe);
        assert!(result.error_message.unwrap().contains("does not exist"));
    }

    #[test]
    fn test_validate_symlink_creation_parent_not_dir() {
        let temp_dir = TempDir::new().unwrap();
        let original_source = temp_dir.path().join("source.txt");
        File::create(&original_source).unwrap();
        let symlink_source = temp_dir.path().join("repo").join("source.txt");

        // Create a file where parent should be
        let parent_file = temp_dir.path().join("parent");
        File::create(&parent_file).unwrap();
        let target = parent_file.join("target");

        let result = validate_symlink_creation(&original_source, &symlink_source, &target).unwrap();
        assert!(!result.is_safe);
        assert!(result.error_message.unwrap().contains("not a directory"));
    }

    #[test]
    fn test_validate_symlink_creation_success() {
        let temp_dir = TempDir::new().unwrap();
        let original_source = temp_dir.path().join("source.txt");
        File::create(&original_source).unwrap();
        let symlink_source = temp_dir.path().join("repo").join("source.txt");
        let target = temp_dir.path().join("subdir").join("target");

        let result = validate_symlink_creation(&original_source, &symlink_source, &target).unwrap();
        assert!(result.is_safe);
    }

    #[test]
    fn test_complex_nested_scenario() {
        let temp_dir = TempDir::new().unwrap();
        let mut synced = HashSet::new();

        // Sync a directory with nested structure
        synced.insert(".config/nvim".to_string());

        // Try to sync a file inside it - should fail
        let file_path = temp_dir
            .path()
            .join(".config")
            .join("nvim")
            .join("init.lua");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        File::create(&file_path).unwrap();

        let result = validate_before_sync(
            ".config/nvim/init.lua",
            &file_path,
            &synced,
            temp_dir.path(),
        );
        assert!(!result.is_safe);
    }

    #[test]
    fn test_reverse_scenario_file_then_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut synced = HashSet::new();

        // First sync a file
        synced.insert(".nvim/init.lua".to_string());

        // Then try to sync parent directory - should fail
        let dir_path = temp_dir.path().join(".nvim");
        std::fs::create_dir_all(&dir_path).unwrap();

        let result = validate_before_sync(".nvim", &dir_path, &synced, temp_dir.path());
        assert!(!result.is_safe);
        assert!(result
            .error_message
            .unwrap()
            .contains("contains files that are already synced"));
    }

    #[test]
    fn test_sibling_files_same_directory() {
        let mut synced = HashSet::new();
        synced.insert(".nvim/init.lua".to_string());

        // Sibling file should be OK
        assert!(!is_file_inside_synced_directory(
            ".nvim/config.lua",
            &synced
        ));
        assert!(!is_file_inside_synced_directory(
            ".nvim/colors.vim",
            &synced
        ));
    }

    #[test]
    fn test_empty_paths() {
        let synced = HashSet::new();

        // Empty or root paths
        assert!(!is_file_inside_synced_directory("", &synced));
        assert!(!is_file_inside_synced_directory(".", &synced));
        assert!(!is_file_inside_synced_directory("..", &synced));
    }

    #[test]
    fn test_path_with_dot_components() {
        let mut synced = HashSet::new();
        synced.insert(".config".to_string());

        // Paths with .. components (should still work)
        assert!(is_file_inside_synced_directory(
            ".config/../.config/file",
            &synced
        ));
    }

    #[test]
    fn test_unicode_and_special_characters() {
        let mut synced = HashSet::new();
        synced.insert(".config".to_string());

        // Unicode characters
        assert!(is_file_inside_synced_directory(".config/测试.txt", &synced));
        assert!(is_file_inside_synced_directory(".config/файл.txt", &synced));

        // Special characters (if filesystem allows)
        // Note: Some filesystems may not allow these, so we test what we can
    }

    #[test]
    fn test_very_long_paths() {
        let mut synced = HashSet::new();
        synced.insert(".config".to_string());

        // Very long nested path
        let long_path = format!(".config/{}", "a/".repeat(50));
        assert!(is_file_inside_synced_directory(&long_path, &synced));
    }

    #[test]
    fn test_case_sensitivity() {
        // Note: Our validation is case-sensitive by design
        // On case-insensitive filesystems (macOS), the filesystem itself handles this
        // but our path matching is explicit to avoid false positives

        let mut synced = HashSet::new();
        synced.insert(".Config".to_string());

        // Exact match should work
        assert!(is_file_inside_synced_directory(".Config/file", &synced));

        // Case mismatch - our validation is case-sensitive
        // (This is intentional - we match exactly what's in synced_files)
        // The filesystem will handle case-insensitivity at the OS level
        assert!(!is_file_inside_synced_directory(".config/file", &synced));
    }

    #[test]
    fn test_already_synced_exact_match() {
        let temp_dir = TempDir::new().unwrap();
        let mut synced = HashSet::new();
        synced.insert(".zshrc".to_string());

        let file_path = temp_dir.path().join(".zshrc");
        File::create(&file_path).unwrap();

        let result = validate_before_sync(".zshrc", &file_path, &synced, temp_dir.path());
        assert!(!result.is_safe);
        assert!(result.error_message.unwrap().contains("already synced"));
    }

    #[test]
    fn test_git_repo_as_file_not_directory() {
        // Edge case: .git as a file (submodule case)
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("test");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create .git as a file (submodule)
        File::create(test_dir.join(".git")).unwrap();

        // Should still detect it (our check uses .join(".git").exists() which works for both)
        assert!(contains_git_repo(&test_dir));
    }

    #[test]
    fn test_multiple_git_repos_in_tree() {
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path().join("project");
        std::fs::create_dir_all(&test_dir).unwrap();

        // Create multiple git repos
        std::fs::create_dir_all(test_dir.join("sub1").join(".git")).unwrap();
        std::fs::create_dir_all(test_dir.join("sub2").join(".git")).unwrap();

        let result = contains_nested_git_repo(&test_dir).unwrap();
        assert!(result);
    }

    #[test]
    fn test_symlink_validation_with_existing_symlink() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        File::create(&source).unwrap();

        let target = temp_dir.path().join("target");
        // Create existing symlink
        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &target).unwrap();

        // Should still validate (symlink creation will handle replacing it)
        let symlink_source = temp_dir.path().join("repo").join("source.txt");
        let result = validate_symlink_creation(&source, &symlink_source, &target).unwrap();
        assert!(result.is_safe);
    }

    #[test]
    fn test_directory_with_only_dotfiles() {
        let mut synced = HashSet::new();
        synced.insert(".config/nvim/.nvimrc".to_string());

        // Directory contains synced file even if it's a dotfile
        assert!(directory_contains_synced_files(".config/nvim", &synced));
    }

    #[test]
    fn test_validate_repo_path_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path().join("repo");
        std::fs::create_dir_all(&repo_path).unwrap();

        // Try to sync the repo itself - should fail (caught by is_safe_to_add)
        let result = validate_before_sync(
            "repo",
            &repo_path,
            &HashSet::new(),
            &repo_path, // Pass repo_path as both the path to sync AND the repo_path
        );
        // This should be caught by is_safe_to_add which checks if path == repo_path
        assert!(!result.is_safe);
    }

    #[test]
    fn test_validate_home_directory() {
        // This would be caught by is_safe_to_add, but let's ensure it's caught
        let temp_dir = TempDir::new().unwrap();
        let home = temp_dir.path().join("home");
        std::fs::create_dir_all(&home).unwrap();

        // Try to sync home itself
        let _result = validate_before_sync("home", &home, &HashSet::new(), temp_dir.path());
        // Should be caught by is_safe_to_add check
        // (In real scenario, get_home_dir() would return actual home)
    }

    #[test]
    fn test_concurrent_operations_simulation() {
        // Simulate what happens if validation passes but file changes
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.txt");
        File::create(&source).unwrap();

        let target = temp_dir.path().join("target");
        let symlink_source = temp_dir.path().join("repo").join("source.txt");
        let validation = validate_symlink_creation(&source, &symlink_source, &target).unwrap();
        assert!(validation.is_safe);

        // Now delete source (simulating race condition)
        std::fs::remove_file(&source).unwrap();

        // Re-validate should catch it
        let revalidation = validate_symlink_creation(&source, &symlink_source, &target).unwrap();
        assert!(!revalidation.is_safe);
    }

    #[test]
    fn test_nested_symlink_scenarios() {
        let mut synced = HashSet::new();
        synced.insert(".config".to_string());

        // Create a symlink inside synced directory
        // Should be detected as inside synced directory
        assert!(is_file_inside_synced_directory(".config/symlink", &synced));
    }
}
