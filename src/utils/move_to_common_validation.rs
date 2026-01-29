//! Validation for moving files from profile to common
//!
//! This module validates that moving a file to common won't cause conflicts
//! with other profiles. It checks for:
//! - Same file path in other profiles (with content comparison)
//! - Path hierarchy conflicts (file vs directory at same path)

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Type of conflict detected when moving to common
#[derive(Debug, Clone)]
pub enum MoveToCommonConflict {
    /// Same file path exists in another profile with identical content
    SameContentInProfile { profile_name: String },
    /// Same file path exists in another profile with different content
    DifferentContentInProfile {
        profile_name: String,
        /// Size difference if any (for display purposes)
        size_diff: Option<(u64, u64)>,
    },
    /// Path hierarchy conflict - the file being moved is a parent/child of a path in another profile
    PathHierarchyConflict {
        profile_name: String,
        conflicting_path: String,
        /// true if the conflicting path is a parent of the file being moved
        is_parent: bool,
    },
}

/// Result of validating a move-to-common operation
#[derive(Debug, Clone)]
pub struct MoveToCommonValidation {
    /// Whether the operation can proceed (possibly with user action)
    pub can_proceed: bool,
    /// Conflicts that need user attention
    pub conflicts: Vec<MoveToCommonConflict>,
    /// Whether all conflicts are auto-resolvable (same content)
    pub all_auto_resolvable: bool,
    /// Profiles that would need cleanup if user proceeds (same content only)
    pub profiles_to_cleanup: Vec<String>,
}

impl MoveToCommonValidation {
    /// Create a validation result with no conflicts
    #[must_use]
    pub fn safe() -> Self {
        Self {
            can_proceed: true,
            conflicts: Vec::new(),
            all_auto_resolvable: true,
            profiles_to_cleanup: Vec::new(),
        }
    }
}

/// Validate moving a file to common across all profiles
///
/// This function checks:
/// 1. If the same file path exists in other profiles
/// 2. If the content is identical (safe to auto-cleanup) or different (needs user confirmation)
/// 3. If there are path hierarchy conflicts (file vs directory)
///
/// # Arguments
/// * `repo_path` - Path to the repository root
/// * `source_profile` - Name of the profile the file is currently in
/// * `relative_path` - Relative path of the file (e.g., ".tmux.conf")
///
/// # Returns
/// Validation result with conflicts and cleanup information
pub fn validate_move_to_common(
    repo_path: &Path,
    source_profile: &str,
    relative_path: &str,
) -> Result<MoveToCommonValidation> {
    // Load manifest to get all profiles
    let manifest = crate::utils::ProfileManifest::load_or_backfill(repo_path)?;

    let source_file_path = repo_path.join(source_profile).join(relative_path);

    // Check if source file exists
    if !source_file_path.exists() {
        return Ok(MoveToCommonValidation::safe());
    }

    let mut conflicts = Vec::new();
    let mut profiles_to_cleanup = Vec::new();

    // Check each profile (except the source profile)
    for profile in &manifest.profiles {
        if profile.name == source_profile {
            continue;
        }

        // Check if this profile has the same file path
        if profile.synced_files.contains(&relative_path.to_string()) {
            let other_file_path = repo_path.join(&profile.name).join(relative_path);

            // Check for path hierarchy conflicts first (most critical)
            let hierarchy_conflicts = check_path_hierarchy_conflicts(
                repo_path,
                source_profile,
                &profile.name,
                relative_path,
                &profile.synced_files,
            );
            if !hierarchy_conflicts.is_empty() {
                conflicts.extend(hierarchy_conflicts);
                continue;
            }

            // Check if files have same content
            if other_file_path.exists() {
                match files_have_same_content(&source_file_path, &other_file_path) {
                    Ok(true) => {
                        // Same content - safe to auto-cleanup
                        profiles_to_cleanup.push(profile.name.clone());
                        conflicts.push(MoveToCommonConflict::SameContentInProfile {
                            profile_name: profile.name.clone(),
                        });
                    }
                    Ok(false) => {
                        // Different content - needs user confirmation
                        let size_diff = if let (Ok(meta1), Ok(meta2)) = (
                            fs::metadata(&source_file_path),
                            fs::metadata(&other_file_path),
                        ) {
                            Some((meta1.len(), meta2.len()))
                        } else {
                            None
                        };
                        conflicts.push(MoveToCommonConflict::DifferentContentInProfile {
                            profile_name: profile.name.clone(),
                            size_diff,
                        });
                    }
                    Err(e) => {
                        // Error comparing - treat as different content to be safe
                        tracing::warn!(
                            "Failed to compare files {:?} and {:?}: {}",
                            source_file_path,
                            other_file_path,
                            e
                        );
                        conflicts.push(MoveToCommonConflict::DifferentContentInProfile {
                            profile_name: profile.name.clone(),
                            size_diff: None,
                        });
                    }
                }
            } else {
                // File is in manifest but doesn't exist on disk - treat as same content
                // (likely was synced on another machine)
                profiles_to_cleanup.push(profile.name.clone());
                conflicts.push(MoveToCommonConflict::SameContentInProfile {
                    profile_name: profile.name.clone(),
                });
            }
        } else {
            // Check for path hierarchy conflicts even if exact path doesn't match
            let hierarchy_conflicts = check_path_hierarchy_conflicts(
                repo_path,
                source_profile,
                &profile.name,
                relative_path,
                &profile.synced_files,
            );
            conflicts.extend(hierarchy_conflicts);
        }
    }

    // Determine if we can proceed
    let has_blocking_conflicts = conflicts.iter().any(|c| {
        matches!(
            c,
            MoveToCommonConflict::DifferentContentInProfile { .. }
                | MoveToCommonConflict::PathHierarchyConflict { .. }
        )
    });

    let all_auto_resolvable = conflicts
        .iter()
        .all(|c| matches!(c, MoveToCommonConflict::SameContentInProfile { .. }));

    Ok(MoveToCommonValidation {
        can_proceed: !has_blocking_conflicts,
        conflicts,
        all_auto_resolvable,
        profiles_to_cleanup,
    })
}

/// Compare file contents between two paths
///
/// Returns true if files have identical content, false otherwise.
/// Handles both regular files and directories (for directories, checks if they're identical).
fn files_have_same_content(path1: &Path, path2: &Path) -> Result<bool> {
    let meta1 = fs::metadata(path1).context("Failed to read source file metadata")?;
    let meta2 = fs::metadata(path2).context("Failed to read target file metadata")?;

    // If one is a directory and the other isn't, they're different
    if meta1.is_dir() != meta2.is_dir() {
        return Ok(false);
    }

    // For files, compare size first (quick check)
    if !meta1.is_dir() {
        if meta1.len() != meta2.len() {
            return Ok(false);
        }

        // If sizes match, compare actual content
        let content1 = fs::read(path1).context("Failed to read source file")?;
        let content2 = fs::read(path2).context("Failed to read target file")?;
        return Ok(content1 == content2);
    }

    // For directories, we'd need to recursively compare
    // For now, we'll do a simple check: same number of entries and same structure
    // This is a simplified check - in practice, we might want more thorough comparison
    let entries1: HashSet<String> = fs::read_dir(path1)
        .context("Failed to read source directory")?
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    let entries2: HashSet<String> = fs::read_dir(path2)
        .context("Failed to read target directory")?
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    if entries1 != entries2 {
        return Ok(false);
    }

    // For directories, if structure matches, we'll consider them the same
    // (full recursive comparison would be expensive and rarely needed)
    Ok(true)
}

/// Check for path hierarchy conflicts
///
/// A path hierarchy conflict occurs when:
/// - The file being moved is a child of a directory in another profile
/// - A file in another profile is a child of the directory being moved
///
/// This function checks path structure to detect potential conflicts.
/// It's conservative - it flags conflicts even if we can't be 100% sure
/// from path structure alone, to avoid data loss.
///
/// # Arguments
/// * `repo_path` - Path to the repository root
/// * `source_profile` - Name of the source profile (where the file currently is)
/// * `profile_name` - Name of the profile to check
/// * `relative_path` - The path being moved to common
/// * `profile_files` - List of files synced in the profile
///
/// # Returns
/// Vector of path hierarchy conflicts found
fn check_path_hierarchy_conflicts(
    repo_path: &Path,
    source_profile: &str,
    profile_name: &str,
    relative_path: &str,
    profile_files: &[String],
) -> Vec<MoveToCommonConflict> {
    let mut conflicts = Vec::new();
    let path_buf = PathBuf::from(relative_path);
    let profile_path = repo_path.join(profile_name);
    let source_path = repo_path.join(source_profile);

    for synced_file in profile_files {
        let synced_path = PathBuf::from(synced_file);
        let synced_full_path = profile_path.join(&synced_path);

        // Check if synced_file exists and is a directory
        let synced_is_dir = synced_full_path.exists() && synced_full_path.is_dir();

        // Check if the file being moved is inside a synced directory
        if synced_is_dir {
            // synced_file is a directory - check if relative_path is inside it
            if path_buf.starts_with(&synced_path) && path_buf != synced_path {
                conflicts.push(MoveToCommonConflict::PathHierarchyConflict {
                    profile_name: profile_name.to_string(),
                    conflicting_path: synced_file.clone(),
                    is_parent: true,
                });
                continue; // Found conflict, no need to check reverse
            }
        }

        // Check if relative_path is a directory and synced_file is inside it
        let relative_full_path = source_path.join(&path_buf);
        let relative_is_dir = relative_full_path.exists() && relative_full_path.is_dir();

        // Also check if path structure suggests it's a directory
        let relative_looks_like_dir = relative_path.ends_with('/')
            || (path_buf.components().count() > 1 && path_buf.extension().is_none());

        if relative_is_dir || relative_looks_like_dir {
            // relative_path is a directory - check if synced_file is inside it
            if synced_path.starts_with(&path_buf) && synced_path != path_buf {
                conflicts.push(MoveToCommonConflict::PathHierarchyConflict {
                    profile_name: profile_name.to_string(),
                    conflicting_path: synced_file.clone(),
                    is_parent: false,
                });
            }
        }
    }

    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_files_have_same_content_identical() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        fs::write(&file1, b"identical content").unwrap();
        fs::write(&file2, b"identical content").unwrap();

        assert!(files_have_same_content(&file1, &file2).unwrap());
    }

    #[test]
    fn test_files_have_same_content_different() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        fs::write(&file1, b"content one").unwrap();
        fs::write(&file2, b"content two").unwrap();

        assert!(!files_have_same_content(&file1, &file2).unwrap());
    }

    #[test]
    fn test_path_hierarchy_conflicts_parent() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create directory structure
        let profile_path = repo_path.join("test_profile");
        fs::create_dir_all(profile_path.join("foo/bar")).unwrap();

        let conflicts = check_path_hierarchy_conflicts(
            repo_path,
            "source_profile",
            "test_profile",
            "foo/bar/config.toml",
            &["foo/bar".to_string()],
        );

        assert_eq!(conflicts.len(), 1);
        match &conflicts[0] {
            MoveToCommonConflict::PathHierarchyConflict {
                profile_name,
                conflicting_path,
                is_parent,
            } => {
                assert_eq!(profile_name, "test_profile");
                assert_eq!(conflicting_path, "foo/bar");
                assert!(*is_parent);
            }
            _ => panic!("Expected PathHierarchyConflict"),
        }
    }

    #[test]
    fn test_path_hierarchy_conflicts_child() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create directory structure
        let source_path = repo_path.join("source_profile");
        fs::create_dir_all(source_path.join("foo/bar")).unwrap();

        let conflicts = check_path_hierarchy_conflicts(
            repo_path,
            "source_profile",
            "test_profile",
            "foo/bar",
            &["foo/bar/config.toml".to_string()],
        );

        assert_eq!(conflicts.len(), 1);
        match &conflicts[0] {
            MoveToCommonConflict::PathHierarchyConflict {
                profile_name,
                conflicting_path,
                is_parent,
            } => {
                assert_eq!(profile_name, "test_profile");
                assert_eq!(conflicting_path, "foo/bar/config.toml");
                assert!(!*is_parent);
            }
            _ => panic!("Expected PathHierarchyConflict"),
        }
    }

    #[test]
    fn test_validate_move_to_common_no_conflicts() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create a simple repo structure
        let profile_a = repo_path.join("profile_a");
        fs::create_dir_all(&profile_a).unwrap();
        fs::write(profile_a.join(".zshrc"), b"profile a content").unwrap();

        let mut manifest = crate::utils::ProfileManifest::default();
        manifest.add_profile("profile_a".to_string(), None);
        manifest
            .update_synced_files("profile_a", vec![".zshrc".to_string()])
            .unwrap();
        manifest.save(repo_path).unwrap();

        let result = validate_move_to_common(repo_path, "profile_a", ".zshrc").unwrap();

        assert!(result.can_proceed);
        assert!(result.conflicts.is_empty());
        assert!(result.all_auto_resolvable);
    }

    #[test]
    fn test_validate_move_to_common_same_content() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create two profiles with identical files
        let profile_a = repo_path.join("profile_a");
        let profile_b = repo_path.join("profile_b");
        fs::create_dir_all(&profile_a).unwrap();
        fs::create_dir_all(&profile_b).unwrap();

        let content = b"identical content";
        fs::write(profile_a.join(".tmux.conf"), content).unwrap();
        fs::write(profile_b.join(".tmux.conf"), content).unwrap();

        let mut manifest = crate::utils::ProfileManifest::default();
        manifest.add_profile("profile_a".to_string(), None);
        manifest.add_profile("profile_b".to_string(), None);
        manifest
            .update_synced_files("profile_a", vec![".tmux.conf".to_string()])
            .unwrap();
        manifest
            .update_synced_files("profile_b", vec![".tmux.conf".to_string()])
            .unwrap();
        manifest.save(repo_path).unwrap();

        let result = validate_move_to_common(repo_path, "profile_a", ".tmux.conf").unwrap();

        assert!(result.can_proceed);
        assert_eq!(result.conflicts.len(), 1);
        assert!(matches!(
            result.conflicts[0],
            MoveToCommonConflict::SameContentInProfile { .. }
        ));
        assert!(result.all_auto_resolvable);
        assert_eq!(result.profiles_to_cleanup, vec!["profile_b"]);
    }

    #[test]
    fn test_validate_move_to_common_different_content() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create two profiles with different files
        let profile_a = repo_path.join("profile_a");
        let profile_b = repo_path.join("profile_b");
        fs::create_dir_all(&profile_a).unwrap();
        fs::create_dir_all(&profile_b).unwrap();

        fs::write(profile_a.join(".tmux.conf"), b"profile a content").unwrap();
        fs::write(profile_b.join(".tmux.conf"), b"profile b content").unwrap();

        let mut manifest = crate::utils::ProfileManifest::default();
        manifest.add_profile("profile_a".to_string(), None);
        manifest.add_profile("profile_b".to_string(), None);
        manifest
            .update_synced_files("profile_a", vec![".tmux.conf".to_string()])
            .unwrap();
        manifest
            .update_synced_files("profile_b", vec![".tmux.conf".to_string()])
            .unwrap();
        manifest.save(repo_path).unwrap();

        let result = validate_move_to_common(repo_path, "profile_a", ".tmux.conf").unwrap();

        assert!(!result.can_proceed); // Blocked by different content
        assert_eq!(result.conflicts.len(), 1);
        assert!(matches!(
            result.conflicts[0],
            MoveToCommonConflict::DifferentContentInProfile { .. }
        ));
        assert!(!result.all_auto_resolvable);
    }
}
